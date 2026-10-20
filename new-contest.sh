#!/usr/bin/env bash
# Create a new contest repo from this template.
#   ./new-contest.sh <name> [dest] [--game <slug>]
# Copies the template (tracked + untracked-but-not-ignored files; no target/, work/ or secrets) to
# dest (default: ../ccc-<name>), sets the name in ccc.toml, and makes the first commit.
set -euo pipefail

usage() { echo "usage: $0 <name> [dest] [--game <slug>]" >&2; exit 2; }
name="" dest="" game=""
while [ $# -gt 0 ]; do
  case "$1" in
    --game) [ $# -ge 2 ] || usage; game="$2"; shift 2 ;;
    -h|--help) usage ;;
    *) if [ -z "$name" ]; then name="$1"; elif [ -z "$dest" ]; then dest="$1"; else usage; fi; shift ;;
  esac
done
[ -n "$name" ] || usage
case "$name" in *[!A-Za-z0-9._-]*) echo "name may only contain letters, digits, '.', '_' and '-'" >&2; exit 2 ;; esac

src="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dest="${dest:-$(dirname "$src")/ccc-$name}"
if [ -e "$dest" ] && [ -n "$(ls -A "$dest" 2>/dev/null)" ]; then echo "$dest already exists and is not empty" >&2; exit 1; fi
mkdir -p "$dest"
dest="$(cd "$dest" && pwd)"

# Copy what git would commit (respects .gitignore, so no target/, work/, ccc.local.toml, *.har).
git -C "$src" ls-files -z --cached --others --exclude-standard \
  | (cd "$src" && tar --null -T - -cf -) | tar -xf - -C "$dest"
rm -f "$dest/new-contest.sh" "$dest/new-contest.ps1"

# Fill in ccc.toml (portable: no `sed -i`).
tmp="$(mktemp)"
sed -e "s/^name = .*/name = \"$name\"/" -e "s/^game_slug = .*/game_slug = \"$game\"/" "$dest/ccc.toml" > "$tmp"
mv "$tmp" "$dest/ccc.toml"

cd "$dest"
git init -q -b main
git add -A
git commit -q -m "contest $name (from ccc-rs template)"
cargo build --release -q

echo "Created contest repo: $dest"
echo
echo "Checking the codingcontest.org login and contest:"
if cargo ccc login; then
  ready="Ready."
else
  ready="Not ready yet: fix the login/contest issue above (log in to codingcontest.org in Firefox, start the
contest or training there), then run \`cargo ccc login\` again."
fi

cat <<MSG

$ready Next:
  cd "$dest" && claude          # start Claude Code in the contest repo
  /level                        # plays from the current level: fetch, solve, all, submit (you approve), next
MSG
