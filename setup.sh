#!/usr/bin/env bash
# Set up a contest repo on any machine (Linux/macOS) straight from the private template on GitHub,
# without cloning the template. Needs `gh` logged in with access to the template repo.
#
#   gh api repos/H4WKF0X/ccc-rs/contents/setup.sh -H 'Accept: application/vnd.github.raw' | bash -s -- <name> [dest]
#
# <name> e.g. ccc-school-2026; dest defaults to ~/projects/<name> (CCC_PROJECTS changes ~/projects).
# Checks the tools, downloads a snapshot of the template, then tools/init-contest.sh makes it the
# contest repo (git, build, login check). The first repo on a machine compiles dependencies once
# (~1-2 min); later repos in the same folder are ready in seconds. Run it once before contest day.
set -euo pipefail

# Everything is inside main, called on the last line: when this script is piped into bash, no
# command can accidentally read the rest of the script from stdin.
main() {
  local template="${CCC_TEMPLATE:-H4WKF0X/ccc-rs}" ref="${CCC_REF:-main}"
  local name="${1:-}" dest="${2:-}"
  if [ -z "$name" ] || [ "$name" = "-h" ] || [ "$name" = "--help" ]; then
    echo "usage: setup.sh <name> [dest]   e.g. setup.sh ccc-school-2026" >&2
    exit 2
  fi
  case "$name" in *[!A-Za-z0-9._-]*) echo "name may only contain letters, digits, '.', '_' and '-'" >&2; exit 2 ;; esac
  local folder="$name"
  case "$name" in ccc-*) ;; *) folder="ccc-$name" ;; esac
  dest="${dest:-${CCC_PROJECTS:-$HOME/projects}/$folder}"

  check_tools
  if [ -e "$dest" ] && [ -n "$(ls -A "$dest" 2>/dev/null)" ]; then
    echo "$dest already exists and is not empty" >&2
    exit 1
  fi

  echo "Downloading the template ($template@$ref)..."
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  gh api "repos/$template/tarball/$ref" > "$tmp/template.tar.gz" < /dev/null
  mkdir -p "$dest"
  tar -xzf "$tmp/template.tar.gz" --strip-components=1 -C "$dest"
  cd "$dest"
  bash tools/init-contest.sh "$name" < /dev/null
}

check_tools() {
  local os install missing=0
  os="$(uname -s)"
  if [ "$os" = Darwin ]; then install="brew install"
  elif command -v pacman >/dev/null 2>&1; then install="sudo pacman -S"
  elif command -v apt-get >/dev/null 2>&1; then install="sudo apt install"
  else install="install"; fi

  need() { # need <command> <what it's for> <how to install>
    if ! command -v "$1" >/dev/null 2>&1; then
      echo "MISSING $1 ($2): $3" >&2
      missing=1
    fi
  }
  want() {
    if ! command -v "$1" >/dev/null 2>&1; then
      echo "warning: $1 not found ($2): $3" >&2
    fi
  }
  need git "version control" "$install git"
  need tar "unpacking the template" "$install tar"
  need gh "downloading the private template" "$install $( [ "$os" = Darwin ] && echo gh || (command -v pacman >/dev/null && echo github-cli || echo gh) ), then: gh auth login"
  need cargo "Rust" "https://rustup.rs (or: $install rustup), then: rustup default stable"
  need claude "Claude Code" "npm install -g @anthropic-ai/claude-code, then run claude once to log in"
  want node "the sandbox checker" "$install $( [ "$os" = Darwin ] && echo node || echo nodejs )"
  want gs "the prompt-injection guard" "$install ghostscript"
  if [ "$os" = Darwin ]; then
    [ -d "/Applications/Firefox.app" ] || echo "warning: Firefox not found (the login is read from it): brew install --cask firefox" >&2
  else
    want firefox "the login is read from it" "$install firefox"
  fi
  if [ "$missing" = 1 ]; then
    echo "Install the missing tools above, then run this again." >&2
    exit 1
  fi
  if ! gh auth status >/dev/null 2>&1; then
    echo "gh is not logged in: run gh auth login, then this again." >&2
    exit 1
  fi
}

main "$@"
