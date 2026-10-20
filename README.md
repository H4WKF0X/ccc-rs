# ccc-rs: Cloudflight Coding Contest pipeline (Rust template)

A fast, game-agnostic pipeline for the **Cloudflight Coding Contest**, built to benchmark an AI
coding agent (Claude Code). It knows nothing about any puzzle. You point it at a downloaded level,
the agent writes the solver, and the pipeline validates against the worked example **before**
anything is uploaded, because a wrong upload costs penalty minutes.

This repo is a **template**. Each contest gets its own repo, made with `new-contest.sh`.

```
new-contest.sh -> download level -> cargo ccc ingest -> /solve N -> cargo ccc all N -> cargo ccc submit N
```

## Start a contest

1. Log in to codingcontest.org in **Firefox** and start the contest (or a training run).
2. Create the repo and play:

```bash
./new-contest.sh ccc42                 # -> ../ccc-ccc42: copied, git-initialised, built, login checked
cd ../ccc-ccc42 && claude              # then in Claude Code:
/level                                 # fetch, solve, `all`, submit (you approve each submit), next level, ...
# Windows: ./new-contest.ps1 ccc42 [dest]
```

All contest repos next to the template share one build directory (`../.ccc-target`): the first
build on a machine compiles the dependencies once (~1 min), after that a new repo is ready in ~2 s.

`/level` drives everything with ordinary commands, which you can also run yourself:

```bash
cargo ccc login                  # who you are (login read from Firefox) + which contest was found
cargo ccc fetch 3                # download + ingest level 3
cargo ccc validate 3             # example: GO / GO (checker) / NO-GO with the first difference
cargo ccc all 3                  # solve all real inputs once (in parallel), save + check the outputs
cargo ccc submit 3               # re-validate, require current + checked outputs, confirm, upload
```

Other commands: `progress`, `ingest <zip|dir|files> [--level N]`, `run N <in> [out]`, `bench N`,
`new-level N [--from M]`, `status`, `guard N` (re-run the PDF check).

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
| `CLAUDE.md`, `.claude/skills/` | Project memory and `/solve`, `/review` for Claude Code. |
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
