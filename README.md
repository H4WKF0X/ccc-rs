# ccc-rs: Cloudflight Coding Contest pipeline (Rust template)

A fast, game-agnostic pipeline for the **Cloudflight Coding Contest**, built to benchmark an AI
coding agent (Claude Code). It knows nothing about any puzzle. You point it at a downloaded level,
the agent writes the solver, and the pipeline validates against the worked example **before**
anything is uploaded, because a wrong upload costs penalty minutes.

This repo is a **template**. Each contest gets its own repo, made with `setup.sh` (any machine, no
clone needed) or `new-contest.sh` (from a local clone).

```
setup.sh <name> -> cargo ccc login -> claude: /level  (fetch, guard, solve, all, submit, next level, ...)
```

## Start a contest

**Contest day: follow [docs/CONTEST-DAY.md](docs/CONTEST-DAY.md)** (setup 10 minutes before, the
online route, and the manual fallback).

On any machine (Linux/macOS; Windows: WSL), straight from this private repo, **without cloning it**.
Needs `gh` logged in (`gh auth login`) with access to the repo:

```bash
gh api repos/H4WKF0X/ccc-rs/contents/setup.sh -H 'Accept: application/vnd.github.raw' | bash -s -- ccc-school-2026
```

That checks the tools (with install hints), downloads a snapshot of the template into
`~/projects/ccc-school-2026`, makes it the contest repo (git, build, login check), and prints the
contest-day steps. Then: `cd ~/projects/ccc-school-2026 && claude`, and `/level`.

Working on the template itself: `./new-contest.sh <name>` does the same from this local clone.

`/level` drives everything with ordinary commands, which you can also run yourself:

```bash
cargo ccc login                  # who you are (login read from Firefox) + which contest was found
cargo ccc fetch 3                # download + ingest level 3
cargo ccc validate 3             # example: GO / GO (checker) / NO-GO with the first difference
cargo ccc all 3                  # solve all real inputs once (in parallel), save + check the outputs
cargo ccc submit 3               # re-validate, require current + checked outputs, confirm, upload
```

Other commands: `progress`, `mode [online|manual]`, `ingest <zip|dir|files> [--level N]`, `run N <in> [out]`,
`bench N`, `new-level N [--from M]`, `status`, `guard N` (re-run the PDF check).

**For humans only:** `cargo ccc guard-html N` checks a level's sandbox HTML for text aimed at an AI
(Claude reads its source; you only see it run). It prints what it finds and offers to quarantine the
file. Claude is blocked from running it, because its output contains the untrusted text. `cargo ccc` is a cargo alias, so it works the same everywhere.

## Layout

| Path | Purpose |
|------|---------|
| `harness/` | The engine: I/O contract, ingest, validation, CLI, platform client. Levels don't edit it. |
| `solution/src/levels/level<N>.rs` | One solver per level, registered automatically by `solution/build.rs`. |
| `solution/src/model.rs` | Parsing/data model shared between levels. |
| `templates/level.rs` | Stub for `cargo ccc new-level N`. |
| `ccc.toml` / `ccc.local.toml` | Contest settings / personal overrides (git-ignored). |
| `CLAUDE.md`, `.claude/skills/` | Project memory; `/level` (the loop), `/optimize` (scored levels), `/solve`, `/review`. |
| `docs/PLATFORM.md` | The codingcontest.org API, credentials setup, HAR capture guide. |
| `checker/` | Judges many-valid-answer levels with the game's own sandbox script (on by default, self-tested). |
| `tools/sanitize_har.py` | Strip secrets from a browser HAR before sharing it. |
| `harness/src/guard.rs` | Prompt-injection guard: hidden text in statement PDFs (runs on every fetch/ingest, ~5 s). |
| `tools/notify.sh` | Sound when Claude needs you (Claude Code hook in `.claude/settings.json`). |
| `work/` | Runtime only (git-ignored): ingested inputs, produced outputs, statements. |

## Setup

1. Rust (stable, edition 2024): `rustup` or your distro's package. Check with `cargo --version`.
2. Claude Code: `npm install -g @anthropic-ai/claude-code`, then `claude` once to sign in.
3. Node.js (for the sandbox checker), Firefox (the login is read from it) and Ghostscript `gs` (for the
   prompt-injection guard; usually preinstalled on Linux, `brew install ghostscript` on macOS).
4. Optional: open a contest repo in RustRover / IntelliJ with the Rust plugin.

## Team workflow (one contest repo)

- `main` is the source of truth. Pull before each level: `git pull --rebase`.
- One person drives the current level with Claude Code. The others ingest the next level, run
  `/review`, or build a quick visualiser. Commit each cleared level: `git commit -am "level 3"`.
- **Agree who submits** each level so nobody double-uploads and eats penalties.
