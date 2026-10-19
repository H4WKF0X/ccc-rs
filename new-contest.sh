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

cat <<MSG
Created contest repo: $dest

Next:
  cd "$dest"
  claude                       # start Claude Code here
  cargo ccc ingest <zip>       # after downloading a level (or no args: newest zip in Downloads)
  /solve 1                     # in Claude Code
  cargo ccc all 1              # validate on the example, then write the real outputs
  cargo ccc submit 1           # manual mode: lists the files to upload in the browser
MSG
