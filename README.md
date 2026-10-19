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

```bash
./new-contest.sh spaceship-training          # -> ../ccc-spaceship-training, git-initialised and built
./new-contest.sh ccc42 ~/contests/ccc42 --game <slug>    # explicit destination + game slug
# Windows: ./new-contest.ps1 ccc42 [dest] [-Game <slug>]
```

## The per-level loop (inside a contest repo)

```bash
cargo ccc fetch 3                # download + ingest from codingcontest.org (platform = "codingcontest")
cargo ccc ingest                 # or: newest level zip in ~/Downloads (or: ingest <zip|dir|files> [--level N])
claude                           # then: /solve 3   (reads statement + example, writes level3.rs, validates)
cargo ccc all 3                  # GO only if level 3 matches the example; then writes work/level3/out/
cargo ccc submit 3               # re-validates, checks freshness, uploads (or lists files in manual mode)
```

Other commands: `validate N`, `run N <in> [out]`, `bench N`, `new-level N [--from M]`, `status`, `progress`.
`cargo ccc` is a cargo alias (`.cargo/config.toml`), so it works the same on Linux, macOS and Windows.

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
| `tools/sanitize_har.py` | Strip secrets from a browser HAR before sharing it. |
| `work/` | Runtime only (git-ignored): ingested inputs, produced outputs, statements. |

## Setup

1. Rust (stable, edition 2024): `rustup` or your distro's package. Check with `cargo --version`.
2. Claude Code: `npm install -g @anthropic-ai/claude-code`, then `claude` once to sign in.
3. Optional: open a contest repo in RustRover / IntelliJ with the Rust plugin.

## Team workflow (one contest repo)

- `main` is the source of truth. Pull before each level: `git pull --rebase`.
- One person drives the current level with Claude Code. The others ingest the next level, run
  `/review`, or build a quick visualiser. Commit each cleared level: `git commit -am "level 3"`.
- **Agree who submits** each level so nobody double-uploads and eats penalties.
