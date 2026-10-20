#!/usr/bin/env bash
# Turn a fresh copy of the ccc-rs template (the current directory) into the contest repo <name>.
# Used by setup.sh (snapshot from GitHub) and new-contest.sh (copy of a local template); not meant
# to be run by hand.
#   bash tools/init-contest.sh <name> [--game <slug>]
# Sets the name, creation time and unique package names, makes the first commit, builds everything
# (release + tests) so nothing compiles at contest time, and checks the login.
set -euo pipefail

usage() { echo "usage: $0 <name> [--game <slug>]" >&2; exit 2; }
name="" game=""
while [ $# -gt 0 ]; do
  case "$1" in
    --game) [ $# -ge 2 ] || usage; game="$2"; shift 2 ;;
    -h|--help) usage ;;
    *) [ -z "$name" ] || usage; name="$1"; shift ;;
  esac
done
[ -n "$name" ] || usage
case "$name" in *[!A-Za-z0-9._-]*) echo "name may only contain letters, digits, '.', '_' and '-'" >&2; exit 2 ;; esac
grep -q '^name = "template"' ccc.toml 2>/dev/null || { echo "run this in a fresh copy of the ccc-rs template" >&2; exit 1; }

# Template-only files don't belong in a contest repo.
rm -f new-contest.sh new-contest.ps1 setup.sh tools/init-contest.sh

# Fill in ccc.toml (portable: no `sed -i`).
tmp="$(mktemp)"
sed -e "s/^name = .*/name = \"$name\"/" -e "s/^game_slug = .*/game_slug = \"$game\"/" \
    -e "s/^created = .*/created = \"$(date +%s)\"/" ccc.toml > "$tmp"
mv "$tmp" ccc.toml
# Unique package and binary names: contest repos share one build directory (see .cargo/config.toml).
id="$(printf %s "$name" | tr . -)"
for f in harness/Cargo.toml solution/Cargo.toml; do
  tmp="$(mktemp)"
  sed -e "s/harness-template/harness-$id/" -e "s/solution-template/solution-$id/" -e "s/\"ccc-template\"/\"ccc-$id\"/" "$f" > "$tmp"
  mv "$tmp" "$f"
done
chmod +x tools/*.sh 2>/dev/null || true

git init -q -b main
git add -A
git commit -q -m "contest $name (from ccc-rs template)"

# Build everything now (release + tests), so nothing compiles at contest time.
if [ ! -d ../.ccc-target ]; then
  echo "First contest repo in $(cd .. && pwd): compiling the dependencies once (~1-2 min)..."
fi
cargo build --release -q
cargo test --no-run -q 2>/dev/null

dest="$(pwd)"
echo "Created contest repo: $dest (built, nothing left to compile)"
echo
cargo ccc login || true

cat <<MSG

When the contest starts (runbook: docs/CONTEST-DAY.md):
  1. Open the game in the browser (Firefox, logged in to codingcontest.org).
  2. cd "$dest" && cargo ccc login     -> LOGIN OK: go on.  GAME API FAILED: cargo ccc mode manual
  3. claude, then /level                -> fetch, check, solve, submit (you approve), next level ...
MSG
