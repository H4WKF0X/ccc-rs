# The contest platform: CatCoder → codingcontest.org

From the 42nd CCC (2026-10-23) the contest runs on a new platform. CatCoder
(`catcoder.codingcontest.org`) and its `/api/game/...` endpoints, which the old Java pipeline's
`CatCoder.java` used, are no longer how games are played. The notes below come from the new portal's public
JavaScript bundles and public API (checked 2026-10-05), not from a logged-in session.

## What we know

**Portal: `https://codingcontest.org`** (Angular SPA)
- Auth: `POST /api/auth/login`, `GET /api/auth/current-user`. The session lives in a cookie.
- `GET /api/editions/active` (public): this year's contests (`slug`, `startTime`, `durationMinutes`,
  `maxTeamSize`). 2026: classic 13:00Z, 240 min, teams ≤3; school 08:00Z, 120 min; AI 13:00Z, 240 min.
- `GET /api/games` (public): every game, with `slug`, `baseUrl`, `totalLevels`. Past games are
  playable for training, e.g.:

  | slug | baseUrl | levels |
  |------|---------|--------|
  | `ccc-2025.11-classic-spaceship` | `https://spaceship.codingcontest.org/` | 8 |
  | `ccc-2024.10-classic-desks` | `https://desks.codingcontest.org/` | 7 |
  | `ccc-2024.04-classic-lawn-mower` | `https://lawn-mower.codingcontest.org/` | 7 |

- Training: `POST /api/training/{gameSlug}/start?mode=challenge|exploration`. `challenge` is a timed run
  with levels unlocking in order, like the real contest. `exploration` is untimed.
  `GET /api/training/active` shows the current run.

**Games: one app per game, embedded in the portal**
- The portal shows the game in a full-screen iframe ("Game Visualizer") loaded from the game's
  `baseUrl`.
- Auth handoff: the portal calls `POST /api/game-token {"contestSlug": "..."}` → `{"token": "..."}`.
  It waits for the iframe to post `{type: "ccc:auth-ready"}`, then sends it
  `{type: "ccc:auth-token", token, contestSlug, contestStartMs, serverClockOffsetMs, contestDurationMinutes, exitUrl}`.
- Game apps return 403 `"This game can only be accessed through the CCC platform"` unless they are
  loaded inside the portal's iframe. We can't read their JS from outside.
- The game's CSP allows `connect-src https://files.codingcontest.org`, so inputs and statements are
  probably downloaded from there.
- Scores reach the portal through SSE at `/api/contests/scores/stream?contestSlug=...`.

## What we don't know yet (needs a HAR)

- The game's endpoints for downloading a level and uploading outputs, and how the token is sent
  (header, cookie or query parameter).
- The file names and layout of a download (still `level3_1_small.in`?), and the statement format
  (PDF, or rendered in the app).
- Whether source code must be uploaded alongside outputs.
- The upload response (does it say per file whether it's valid?).
- **The "0000" code** a Cloudflight person mentioned. It's probably entered somewhere in the game app.
  Note where.

Until then, `platform = "manual"`: `cargo ccc submit N` re-validates, checks freshness, and lists
the files to upload in the browser. Ingest already accepts zips, folders and loose files, and
detects the level from names (or `--level N`), so manual downloads of any shape work.

## Capturing the HAR (about 10 minutes)

1. Log in at codingcontest.org and pick a past game (spaceship is good: our old Level 1–4 code
   solved it). Choose **challenge** mode.
2. Before clicking start: open DevTools (F12) → **Network**, tick **Preserve log**, and turn
   off "Disable cache". Clear the log.
3. Start the game. Download level 1's files. Look out for any place you type a code like "0000".
4. Upload one output. A deliberately wrong one is fine in training; it shows the rejection response.
5. If the game asks for source code, upload a file too.
6. Network panel → right-click → **Save all as HAR with content**. Save it outside the repo or as
   `*.har` (git-ignored). **The HAR contains your session tokens**, so don't commit or share it.
7. In Claude Code: "read <path>.har and implement `harness/src/platform/codingcontest.rs`".

## Porting notes (Java `ccc-pipeline` → `ccc-rs`)

| Java | Rust | Change |
|------|------|--------|
| `ccc/core/Solver.java` + reflection | `pub fn solve(&str) -> Vec<String>` + `solution/build.rs` | still one file per level, no registry |
| `ccc/core/Io.java` (always CRLF) | `harness/src/io.rs` | output style copied from the example `.out` (or forced in `ccc.toml`) |
| `Cli.ingest` (zip, `level\d_` names) | `harness/src/ingest.rs` | also folders, loose files, nested zips, `--level N` |
| `Cli.validate/all/run` | `harness/src/cli.rs` | multiple examples, panics report the file; plus `bench`, `new-level`, `status` |
| `CatCoder.java` + `Json.java` | `harness/src/platform/` | `manual` now; `codingcontest` after the HAR |
| `ccc.local.properties` | `ccc.toml` + `ccc.local.toml` | flat key = value |
| `.claude/commands/*.md` | `.claude/skills/*/SKILL.md` | same steps, Rust paths, Claude may run `validate` itself |
| one repo for all years | template + `new-contest.sh` | one repo per contest |
