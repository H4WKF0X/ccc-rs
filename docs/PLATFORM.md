# The contest platform: codingcontest.org

From the 42nd CCC (2026-10-23) contests run on **codingcontest.org**. CatCoder
(`catcoder.codingcontest.org`), which the old Java pipeline's `CatCoder.java` talked to, is gone.
The notes below come from the portal's public JavaScript and API, plus a HAR captured during a
Spaceship training run (2026-10-19), sanitised with `tools/sanitize_har.py`.

## Using it

```toml
# ccc.toml
platform = "codingcontest"
contest = "training-ccc-2025.11-classic-spaceship-23882-ff454653"   # optional, see below

# ccc.local.toml (git-ignored), option A: your portal cookies
cookie = "…"          # DevTools → Network → any codingcontest.org/api request → Request Headers → Cookie
```

```bash
cargo ccc progress     # checks the credentials and shows your levels/submissions
cargo ccc fetch 3      # downloads level 3's zip and ingests it (instead of ingest from Downloads)
cargo ccc submit 3     # validates, checks freshness, asks "yes", uploads each real output
```

- `contest` is the `<name>` in the portal URL `codingcontest.org/contests/<name>/game`. If it is
  unset, the client uses your one RUNNING registration. Training runs may not appear there, so set it
  for training.
- **Option B** if the cookie route fails: copy the game's own credentials from DevTools (any request to
  `https://<game>.codingcontest.org/api/...`). Set `game_token` (the `Authorization` header value),
  `game_url` (e.g. `https://spaceship.codingcontest.org`) and `contest_slug` (the `x-ccc-slug`
  header value).
- Credentials expire. On HTTP 401/403, copy fresh ones.
- `platform = "manual"` still works: `submit` then just lists the files to upload in the browser.

**Not yet verified live:** the cookie → game-token step (XSRF header). `cargo ccc progress` tests it
without side effects. Run it once in a training game before Friday. (Confirmed in DevTools:
`x-ccc-slug` is the contest `slug`, the dashed form like `training-ccc-2025-11-…`.)

## The API

**Portal `https://codingcontest.org`** (Angular SPA, cookie session, Spring-style XSRF: the
`XSRF-TOKEN` cookie is echoed in an `X-XSRF-TOKEN` header)
- `GET /api/contests/{name}` → `{slug, gameSlug, gameBaseUrl, status, startTime, durationMinutes, totalLevels, ...}`.
  Note that `slug` uses `-` where the URL name has `.` (`…2025-11…` vs `…2025.11…`).
- `GET /api/contests/my-registrations` → `[{contestSlug, contestName, status, ...}]`
- `POST /api/game-token {"contestSlug": slug}` → `{"token": "<uuid>"}`
- `POST /api/training/{gameSlug}/start?mode=challenge|exploration` starts a training run
  (`challenge` is timed with levels unlocking in order, `exploration` is untimed).
- Public: `GET /api/games` (every game's `slug`, `baseUrl`, `totalLevels`), `GET /api/editions/active`.

**Game app `https://<game>.codingcontest.org`**, embedded in the portal as an iframe. The portal sends
it the token via `postMessage({type: "ccc:auth-token", token, contestSlug, ...})`. API requests carry
`Authorization: <token>` (raw, no `Bearer`) and `x-ccc-slug`.
- `GET /game/game-info` (no auth): per level `inputFiles: ["0-example","1-small","2-large"]`,
  `unscoredFiles: ["0-example"]`, `scoreDirection`, `optimization`, `sandbox`.
- `GET /api/contestant/contestant-info` → `{score: {gameScore: {level, timeMs}, state: {level1: {submissions: [{fileId, submittedAt, result: {correct}}], passedFiles}}}}`
- `GET /api/contestant/level/{L}/files` → `{url}`: a short-lived signed link to
  `files.codingcontest.org/game-files/<game>/level-{L}.zip`.
- `GET /api/contestant/level/{L}/input/{fileId}` → `{url}` for a single input file. The UI loads the
  small one up front and the large one when you click it. Inputs are not gated on each other.
- `POST /api/contestant/submit-{L}-{fileId}` with multipart field `solution` (the file name doesn't
  matter) → `{evaluation: {isCorrect, cases: [{isCorrect, caseState, startLineNumber, endLineNumberExcl, caseMessage}]}}`.
  A wrong answer is graded per case with a message like `"Wrong value on line 1, got '…'"`.

**Level zip** (`level-1.zip`):
```
in_level-1_0-example.txt   out_level-1_0-example.txt   ← worked example (LF line endings)
in_level-1_1-small.txt     in_level-1_2-large.txt      ← real inputs
level-1.pdf                                            ← statement
```
Levels with `sandbox: true` (Spaceship 4–8) also ship `sandbox-level-N.html`: an offline visualiser
whose inline `<script>` is the game's full checker (input/solution format validation, simulation,
collision rules, exact error messages). Open it in a browser to check an output by hand, and let Claude
read the script to get the exact rules. Ingest puts it in `statement/`.

Outputs are written as `out_level-1_1-small.txt` etc. The harness still understands CatCoder's
`level1_1_small.in/.out` naming.

## Still unknown

- **The "0000" code** a Cloudflight person mentioned. It wasn't in the captured traffic.
- Whether wrong submissions cost penalty time. `submit` asks for confirmation either way.

## Capturing another HAR

1. DevTools (F12) → Network, tick **Preserve log**, filter **All**, then **reload** the game page so
   the game's startup requests are captured.
2. Do the thing you want to learn (download, upload, sandbox, ...).
3. Right-click → **Save all as HAR with content** into `docs/` (`*.har` is git-ignored).
4. `python3 tools/sanitize_har.py` → `docs/har-summary.json`. Review it, then share that instead of the HAR.
5. Delete the HAR afterwards. It holds live session tokens.
