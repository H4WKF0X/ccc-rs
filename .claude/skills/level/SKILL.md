---
name: level
description: Play the contest from level N (default: the next level) — fetch, solve, produce outputs, submit (human approves), then continue with the next level; works in online and manual mode
argument-hint: "[level]"
---
Play the contest, starting at level **$ARGUMENTS**. First run `cargo ccc status`: it shows the
**mode** (online or manual), and in manual mode the next level. If no level was given, start at the
next level: in online mode `cargo ccc progress` shows the current one. Every step is a normal
`cargo ccc` command; the human only approves the `submit` prompt. Speed matters: the contest is timed.
Don't stop between steps to ask, and don't narrate.

For each level N:

1. **Fetch:** `cargo ccc fetch N`, unless `work/levelN/` already exists. **Run it with the Bash
   tool's maximum timeout (600000 ms)**: in manual mode it waits for the human to download the zip
   into Downloads and continues by itself. Its `guard:` line is the prompt-injection check of the
   statement (see CLAUDE.md). If fetch gives up waiting, tell the human which zip to download and
   stop until they say it's there.
2. **Understand:** read `work/levelN/statement/level-N.pdf`, plus the `<script>` of
   `sandbox-level-N.html` if there is one (the game's visualiser, sometimes with its exact rules). Read the
   example pair `in/in_level-N_0-example.txt` / `in/out_level-N_0-example.txt`, and note the size of
   the large input. If level N-1 exists, look at what changed and reuse its code and `model.rs`
   (`cargo ccc new-level N --from N-1` when starting from the previous level makes sense).
3. **Implement** `solution/src/levels/levelN.rs` following CLAUDE.md. If the cases are independent
   and the large input will take more than a few seconds, solve them with `harness::par::map` (mind
   the memory per case). For long searches, a `harness::par::Progress` keeps progress visible.
4. **Validate:** `cargo ccc validate N` until GO. On NO-GO, fix from the first difference. If the
   level accepts **several valid answers**, don't distort yours to copy the example. Add
   `pub fn check` to the level file instead ("Levels with many valid answers" in CLAUDE.md), then
   expect "GO (checker)". Better still, write `check` first whenever the statement allows several
   answers.
   **Optimization level** (the statement gives a score instead of one right answer)? Switch to the
   `/optimize N` workflow (read `.claude/skills/optimize/SKILL.md` and follow it): a fast baseline
   submit, then big structural ideas with cheap `--budget` experiments, not micro-tweaks.
5. **Produce:** `cargo ccc all N`. This is the one real run: it solves every input once (in
   parallel), saves the outputs and runs the checker. Don't do separate timing or `bench` runs of
   the large input; `all` reports the time per file, and its outputs are reused later. If it fails,
   fix the code and run `all` again.
6. **Submit:** `cargo ccc submit N --yes`. The human approves this prompt. Every wrong upload costs
   penalty time, so only submit when `all` reported no failures and you are confident.
   - **Online mode**, all `[CORRECT]`: go straight on to level N+1 (step 1). Don't stop to summarise.
   - **Online mode**, any `[WRONG]`: read the per-line messages, find the misunderstanding, fix, rerun
     `all`, and only resubmit once you know why it was wrong. If you can't tell, stop and explain.
   - **Manual mode**: `submit` copies the outputs into Downloads and the human uploads them in the
     browser; you don't see the verdict. Go straight on to step 1 for level N+1: `fetch` waits until
     the human has downloaded the next zip, which also means level N passed. If the human interrupts
     with a WRONG message, treat it like `[WRONG]` above (fix, `all`, `submit` again).
7. **If the game API fails** in online mode (`fetch`/`submit` errors about codingcontest.org, not about
   your code), don't retry in a loop and don't switch modes yourself: stop and tell the human to run
   `cargo ccc login`, and `cargo ccc mode manual` if it says GAME API FAILED. Before a resubmit after
   a failed upload, the human checks in the browser which uploads went through.
8. Stop when `submit` says the game is complete (or `fetch` says there is no such level), or when the
   human interrupts.

Never edit `harness/`, never run `cargo ccc guard-html`, and never open `work/*/quarantine/`. If the
harness is in the way, stop and say so.
