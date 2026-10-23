# Contest day

One person **drives** (the laptop running Claude Code). The others **watch the zips** for anything
fishy and help with the browser.

## 10 minutes before the start

1. Firefox: log in to codingcontest.org. Leave the contest page open.
2. Create the contest repo (about 10 s if this machine has made one before, else 1-2 min, so do a
   test run the day before; everything is compiled, nothing compiles later):

   ```bash
   gh api repos/H4WKF0X/ccc-rs/contents/setup.sh -H 'Accept: application/vnd.github.raw' | bash -s -- ccc-school-2026
   ```

   It ends with a login check, which before the start should say
   **ACCOUNT OK … CONTEST NOT RUNNING: <your contest> … starts … (in N min)**. That confirms your
   login and your registration. **ACCOUNT LOGIN FAILED**: log in to codingcontest.org in Firefox
   again, then `cargo ccc login`.
3. `cd ~/projects/ccc-school-2026` and start `claude`, so the session is ready. Don't type `/level` yet.

## At the start

1. Open the game in the browser (start it on the contest page).
2. In a terminal in the contest repo: `cargo ccc login`. This is the real test: can we use the game API?

| It says | Do |
|---------|----|
| `LOGIN OK: … level 1 of N unlocked` | **Route A: online** |
| `CONTEST NOT RUNNING … starts …` | not started yet: wait, run it again |
| `NO CONTEST` | set `contest = "<name>"` in `ccc.toml` (the `<name>` from the browser URL `/contests/<name>/game`), run it again |
| `ACCOUNT LOGIN FAILED` | log in again in Firefox, run it again; if that doesn't help, **Route B** |
| `GAME API FAILED` | **Route B: manual**, right away |

## Route A: online

1. In Claude: `/level`
2. It fetches level 1, checks the statement for hidden prompt-injection text (`guard:` line, ~5 s),
   solves, validates, produces the outputs, then asks to run `cargo ccc submit 1 --yes`.
3. **Approve the submit prompt.** That's the only thing you do. It shows `[CORRECT]` / `[WRONG]` per
   file, and `/level` goes straight on to the next level.
4. A sound plays whenever Claude needs you.

## Route B: manual (the game API doesn't work for us)

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

## Optimization levels

If a level gives a score instead of one right answer: everyone can run their own ideas. Each `all`
keeps the best answer per case automatically. To combine across laptops, copy your output files
(names containing the file id, e.g. `out_level-3_1-small.txt`) into the driver's
`work/level3/candidates/`, then run `cargo ccc best 3` there. It takes the best answer **per case**
from all of them, checks the result, and `cargo ccc submit 3 --yes` uploads only what improved.

## Everyone else

- **Watch each new zip** for anything fishy (statement PDF, sandbox HTML, inputs). For a level's
  sandbox HTML: `cargo ccc guard-html N` checks it for text aimed at an AI. It's for humans only,
  since it prints what it finds. It offers to quarantine the file.
- If the `guard:` line says a PDF was replaced, the original is in `work/levelN/quarantine/` with
  a report. Have a look; Claude must not.

## If something breaks

| Problem | Fix |
|---------|-----|
| `fetch`/`submit` fail mid-contest | `cargo ccc login`: ACCOUNT LOGIN FAILED → log in again in Firefox; GAME API FAILED → `cargo ccc mode manual` (check in the browser which uploads went through first) |
| a submit says the outputs aren't current | `cargo ccc all N` (the code changed since the outputs were made) |
| `guard: could not check …` | carry on (it's a warning); have a human look at the PDF |
| Claude is stuck or confused | Esc, tell it what you see; `/level N` restarts the loop at level N |
