use clap::Parser;
use octocrab::{Octocrab, params};
use std::{process::Command};
use once_cell::sync::Lazy;
use std::{error::Error};
use regex::Regex;

static RE_GIT_LS_REMOTE: Lazy<Regex> = Lazy::new(|| {
  Regex::new("^(?P<hash>\\w*)\\s*refs/pull/(?P<prid>\\d+)/head$").unwrap()
});
static RE_BODY_TASK_LIST_CHECKED: Lazy<Regex> = Lazy::new(|| {
  Regex::new("-\\s\\[x\\]\\s#").unwrap()
});

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
   /// base branch of pull request
   #[arg(short, long)]
   base: String,

   /// head branch of pull request
   #[arg(short = 'H', long)]
   head: String,

   /// merge a pull request
   #[arg(long, default_value_t = false)]
   merge: bool,

   /// merge a pull request with squash
   #[arg(long, default_value_t = false)]
   merge_squash: bool,

   /// merge a pull request with rebase
   #[arg(long, default_value_t = false)]
   merge_rebase: bool,

   /// no remote fetch before executing
   #[arg(long, default_value_t = false)]
   no_fetch: bool,
}

#[tokio::main]
async fn main()-> Result<(), Box<dyn Error>> {
  let args = Args::parse();

  // Variables
  let head = args.head;
  let base = args.base;
  let (owner, repo) = get_repo_name();
  let head_str = head.as_str();
  let base_str = base.as_str();
  let owner_str = owner.as_str();
  let repo_str = repo.as_str();

  // fetch remote
  if !args.no_fetch {
    git_fetch_all()
  }

  // Gitub PR
  let mut ret: Vec<PR> = get_diff_pr(base_str, head_str);

  // Github Client
  let github_client = get_github_client();

  for pr in ret.iter_mut() {
    let res = github_client.pulls(owner_str, repo_str).get(pr.id).await?;
    if let Some(user) = res.user {
      pr.username = user.login;
    }
    for p in pr.children.iter_mut() {
      let res = github_client.pulls(owner_str, repo_str).get(p.id).await?;
      if let Some(user) = res.user {
        p.username = user.login;
      }
    }
  }

  let mut body = "".to_owned();
  for pr in ret {
    body += &format!("- [ ] #{} @{} {}\n", pr.id, pr.username, pr.date);
    for p in pr.children {
      body += &format!("  - [ ] #{} @{} {}\n", p.id, p.username, p.date);
    }
  }

  // List Github PR
  let list_pr = github_client.pulls(owner_str, repo_str)
    .list()
    // Optional Parameters
    .state(params::State::Open)
    .head(head_str)
    .base(base_str)
    .sort(params::pulls::Sort::Created)
    .direction(params::Direction::Descending)
    .per_page(1)
    // Send the request
    .send()
    .await?
    .take_items();

  let mut target_id: u64 = 0;
  if list_pr.len() > 0 {
    target_id = list_pr[0].number;
    // keep checked task list
    if let Some(now_body) = &list_pr[0].body {
      for line in now_body.split("\n") {
        if RE_BODY_TASK_LIST_CHECKED.is_match(&line) {
          let unchecked_line = line.replace("- [x] #", "- [ ] #");
          body = body.replace(&unchecked_line, &line);
        }
      }
    }
    // Update Github PR
    github_client
      .pulls(owner_str, repo_str)
      .update(list_pr[0].number)
      .body(body)
      .send()
      .await?;
    
    if let Some(html_url) = &list_pr[0].html_url {
      println!("existing PullRequest was successfully updated: {}", html_url.as_str());
    }
  } else {
    // Create Github PR
    let title = format!("{} from {}", base_str, head_str);
    let ret = github_client
      .pulls(owner_str, repo_str)
      .create(title, head_str, base_str)
      .body(body)
      .send()
      .await?;

    if let Some(html_url) = &ret.html_url {
      println!("new PullRequest was successfully created: {}", html_url.as_str());
    }
    target_id = ret.number;
  }

  if args.merge {
    let mut method = params::pulls::MergeMethod::Merge;
    if args.merge_rebase {
      method = params::pulls::MergeMethod::Rebase;
    }
    else if args.merge_squash {
      method = params::pulls::MergeMethod::Squash;
    }
    github_client
      .pulls(owner_str, repo_str)
      .merge(target_id)
      .method(method)
      .send().await?;

      println!("  and successfully merged ({:?})", method);
  }

  Ok(())

}

#[derive(Debug, Clone)]
struct PR {
  id: u64,
  date: String,
  username: String,
  hash: String,
  children: Vec<PR>,
}

fn get_diff_pr(base: &str, head: &str) -> Vec<PR> {
  let mut prs: Vec<PR> = Vec::new();

  // get feature branch commit hash of merge commits
  let merges_all = Command::new("git")
    .arg("log")
    .arg(format!("origin/{}..origin/{}", base, head))
    .arg("--merges")
    .arg("--pretty=format:'%P %cI'")
    .output().expect("failed to execute process");

  // get feature branch commit hash of merge commits
  let merges_first_parent = Command::new("git")
    .arg("log")
    .arg(format!("origin/{}..origin/{}", base, head))
    .arg("--merges")
    .arg("--pretty=format:'%P %cI'")
    .arg("--first-parent")
    .output().expect("failed to execute process");
  
  let merges_first_parent_list = std::str::from_utf8(&merges_first_parent.stdout).unwrap().split_terminator("\n").collect::<Vec<&str>>();
  let merges_all_list = std::str::from_utf8(&merges_all.stdout).unwrap().split_terminator("\n").collect::<Vec<&str>>();
  for a in merges_all_list {
    let line_a = a.split_whitespace().collect::<Vec<&str>>();
    let mut found = false;
    for b in &merges_first_parent_list {
      if a == b.to_owned() {
        found = true;
        break;
      }
    }
    if found {
      prs.push(PR{
        hash: line_a[1].to_owned(),
        id: 0,
        date: line_a[2].to_owned().to_owned().replace("'", ""),
        username: "".to_owned(),
        children: vec![],
      });
    } else {
      if prs.len() > 0 {
        let len = prs.len();
        prs[len-1].children.push(PR{
          hash: line_a[1].to_owned(),
          id: 0,
          date: line_a[2].to_owned().to_owned().replace("'", ""),
          username: "".to_owned(),
          children: vec![],
        });
      } else {
        prs.push(PR{
          hash: line_a[1].to_owned(),
          id: 0,
          date: line_a[2].to_owned().to_owned().replace("'", ""),
          username: "".to_owned(),
          children: vec![],
        });
      }
    }
  }

  // get pull requests
  let ls_remotes = Command::new("git")
    .arg("ls-remote")
    .arg("origin")
    .arg("pull/*/head")
    .output().expect("failed to execute process");

  for a in std::str::from_utf8(&ls_remotes.stdout).unwrap().split_terminator("\n").collect::<Vec<&str>>() {
    let parts = RE_GIT_LS_REMOTE.captures(&a).unwrap();
    for pr in prs.iter_mut() {
      if &parts["hash"] == pr.hash.to_owned() {
        pr.id = parts["prid"].parse().unwrap();
        break;
      }

      for pr in pr.children.iter_mut() {
        if &parts["hash"] == pr.hash.to_owned() {
          pr.id = parts["prid"].parse().unwrap();
        }
      }
    }
  }

  // filter
  prs.retain(|x| x.id != 0);
  for pr in prs.iter_mut() {
    pr.children.retain(|x| x.id != 0);
  }

  return prs;
}

fn get_repo_name() -> (String, String) {

  // get feature branch commit hash of merge commits
  let url = Command::new("git")
    .arg("remote")
    .arg("get-url")
    .arg("origin")
    .output()
    .expect("failed to execute process");

  let out = std::str::from_utf8(&url.stdout).unwrap();
  let s1 = out.split(":").collect::<Vec<&str>>();
  if s1.len() < 2 {
    panic!("git remote url is invalid");
  }
  let s2 = s1[1].replace("//github.com/", "").replace(".git", "");
  let names = s2.split("/").collect::<Vec<&str>>();
  if s2.len() < 2 {
    panic!("git remote url is invalid");
  }
  return (names[0].to_owned(), names[1].to_owned());
}

fn git_fetch_all() {
  // fetch remote
  Command::new("git")
    .arg("fetch")
    .arg("--all")
    .output()
    .expect("failed to execute process");
}

fn get_github_client() -> Octocrab {
  let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN env variable is required");
  return octocrab::OctocrabBuilder::new()
    .personal_token(token)
    .build()
    .unwrap();
}
