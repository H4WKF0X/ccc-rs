#!/usr/bin/env bash
# Create a new contest repo from this local template (for working on the template; on any other
# machine use setup.sh, which needs no clone of the template).
#   ./new-contest.sh <name> [dest] [--game <slug>]
# Copies the template (tracked + untracked-but-not-ignored files; no target/, work/ or secrets) to
# dest (default: ../<name>, prefixed with ccc- if it isn't already), then tools/init-contest.sh does
# the rest (name, git, build, login check).
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
case "$name" in ccc-*) folder="$name" ;; *) folder="ccc-$name" ;; esac
dest="${dest:-$(dirname "$src")/$folder}"
if [ -e "$dest" ] && [ -n "$(ls -A "$dest" 2>/dev/null)" ]; then echo "$dest already exists and is not empty" >&2; exit 1; fi
mkdir -p "$dest"
dest="$(cd "$dest" && pwd)"

# Copy what git would commit (respects .gitignore, so no target/, work/, ccc.local.toml, *.har).
git -C "$src" ls-files -z --cached --others --exclude-standard \
  | (cd "$src" && tar --null -T - -cf -) | tar -xf - -C "$dest"

cd "$dest"
if [ -n "$game" ]; then bash tools/init-contest.sh "$name" --game "$game"; else bash tools/init-contest.sh "$name"; fi
