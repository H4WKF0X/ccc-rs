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
case "$name" in ccc-*) folder="$name" ;; *) folder="ccc-$name" ;; esac
dest="${dest:-$(dirname "$src")/$folder}"
if [ -e "$dest" ] && [ -n "$(ls -A "$dest" 2>/dev/null)" ]; then echo "$dest already exists and is not empty" >&2; exit 1; fi
mkdir -p "$dest"
dest="$(cd "$dest" && pwd)"

# Copy what git would commit (respects .gitignore, so no target/, work/, ccc.local.toml, *.har).
git -C "$src" ls-files -z --cached --others --exclude-standard \
  | (cd "$src" && tar --null -T - -cf -) | tar -xf - -C "$dest"
rm -f "$dest/new-contest.sh" "$dest/new-contest.ps1"

# Fill in ccc.toml (portable: no `sed -i`).
tmp="$(mktemp)"
sed -e "s/^name = .*/name = \"$name\"/" -e "s/^game_slug = .*/game_slug = \"$game\"/" \
    -e "s/^created = .*/created = \"$(date +%s)\"/" "$dest/ccc.toml" > "$tmp"
mv "$tmp" "$dest/ccc.toml"
# Unique package and binary names: contest repos share one build directory (see .cargo/config.toml).
id="$(printf %s "$name" | tr . -)"
for f in harness/Cargo.toml solution/Cargo.toml; do
  tmp="$(mktemp)"
  sed -e "s/harness-template/harness-$id/" -e "s/solution-template/solution-$id/" -e "s/\"ccc-template\"/\"ccc-$id\"/" "$dest/$f" > "$tmp"
  mv "$tmp" "$dest/$f"
done

cd "$dest"
git init -q -b main
git add -A
git commit -q -m "contest $name (from ccc-rs template)"
# Build everything now (release + tests), so nothing compiles at contest time.
cargo build --release -q
cargo test --no-run -q 2>/dev/null

echo "Created contest repo: $dest (built, nothing left to compile)"
echo
cargo ccc login || true

cat <<MSG

When the contest starts (runbook: docs/CONTEST-DAY.md):
  1. Open the game in the browser (Firefox, logged in to codingcontest.org).
  2. cd "$dest" && cargo ccc login     -> LOGIN OK: go on.  GAME API FAILED: cargo ccc mode manual
  3. claude, then /level                -> fetch, check, solve, submit (you approve), next level ...
MSG
