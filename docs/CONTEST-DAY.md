# Contest day

One person **drives** (the laptop running Claude Code). The others **watch the zips** for anything
fishy and help with the browser.

## 10 minutes before the start

1. Firefox: log in to codingcontest.org. Leave the contest page open.
2. Create the contest repo (about 2 s; everything is compiled, nothing compiles later):

   ```bash
   cd ~/projects/ccc-rs && ./new-contest.sh ccc-school-2026
   ```

   It ends with a login check. Before the start it says **LOGGED IN as …, but NO CONTEST FOUND**.
   That's expected. **LOGIN FAILED** here means: log in to codingcontest.org in Firefox again, then
   `cargo ccc login`.
3. `cd ../ccc-school-2026` and start `claude`, so the session is ready. Don't type `/level` yet.

## At the start

1. Open the game in the browser (start it on the contest page).
2. In a terminal in the contest repo: `cargo ccc login`

| It says | Route |
|---------|-------|
| `LOGIN OK: <you> @ <contest> … level 1 of N unlocked` | **A: online** |
| `LOGGED IN …, but NO CONTEST FOUND` | wait a few seconds, `cargo ccc login` again; if it persists, set `contest = "<name>"` in `ccc.toml` (the `<name>` from the browser URL `/contests/<name>/game`) |
| `LOGIN FAILED: …` | **B: manual** |

## Route A: online

1. In Claude: `/level`
2. It fetches level 1, checks the statement for hidden prompt-injection text (`guard:` line, ~5 s),
   solves, validates, produces the outputs, then asks to run `cargo ccc submit 1 --yes`.
3. **Approve the submit prompt.** That's the only thing you do. It shows `[CORRECT]` / `[WRONG]` per
   file, and `/level` goes straight on to the next level.
4. A sound plays whenever Claude needs you.

## Route B: manual (login doesn't work)

1. `cargo ccc mode manual` (only for you; switch back any time with `cargo ccc mode online`).
2. In the browser, **download level 1's zip** into Downloads (the default download folder).
3. In Claude: `/level`. `fetch` waits for the zip, picks it up as soon as it's complete, and goes on.
4. When Claude submits (approve the prompt), the outputs are copied into **Downloads**. Upload them in
   the game; the upload dialog opens there.
   - All correct: **download the next level's zip**. Claude is already waiting for it and continues by itself.
   - Something WRONG: press Esc in Claude and paste the game's message. Claude fixes, reruns, and
     asks to submit again.

Notes:
- Zips in Downloads older than the contest repo are ignored (old `level-1.zip` files from training
  can't be picked up by mistake). Half-finished downloads (`.part`) are ignored too.
- If `fetch` gives up waiting (~9.5 min), Claude tells you which zip it needs. Download it and say "go".
- `cargo ccc status` shows the mode and the next level.

## Everyone else

- **Watch each new zip** for anything fishy (statement PDF, sandbox HTML, inputs). For a level's
  sandbox HTML: `cargo ccc guard-html N` checks it for text aimed at an AI. It's for humans only,
  since it prints what it finds. It offers to quarantine the file.
- If the `guard:` line says a PDF was replaced, the original is in `work/levelN/quarantine/` with
  a report. Have a look; Claude must not.

## If something breaks

| Problem | Fix |
|---------|-----|
| `LOGIN FAILED` mid-contest (login expired) | log in again in Firefox, `cargo ccc login`; or `cargo ccc mode manual` |
| a submit says the outputs aren't current | `cargo ccc all N` (the code changed since the outputs were made) |
| `guard: could not check …` | carry on (it's a warning); have a human look at the PDF |
| Claude is stuck or confused | Esc, tell it what you see; `/level N` restarts the loop at level N |
