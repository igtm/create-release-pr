# create-release-pr

PullRequest generation tool for Git Flow

![image](./docs/pr-example.png "image")

# Installation

### curl

```sh
sudo curl -sfL https://raw.githubusercontent.com/igtm/create-release-pr/master/install.sh | sudo sh -s -- -b=/usr/local/bin
```

if you want to download old version, pass `-v` argument.

```sh
sudo curl -sfL https://raw.githubusercontent.com/igtm/create-release-pr/master/install.sh | sudo sh -s -- -b=/usr/local/bin -v=v0.0.2
```

# Usage

```
Usage: create-release-pr [OPTIONS] --base <BASE> --head <HEAD>

Options:
  -b, --base <BASE>   base branch of pull request
  -H, --head <HEAD>   head branch of pull request
      --merge         merge a pull request
      --merge-squash  merge a pull request with squash
      --merge-rebase  merge a pull request with rebase
      --no-fetch      no remote fetch before executing
  -h, --help          Print help information
  -V, --version       Print version information
```

# Useful tool for CI

- [igtm/create-release-pr-action](https://github.com/igtm/create-release-pr-action)
