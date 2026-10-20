---
name: level
description: Play the contest from level N (default: the current level) — fetch, solve, produce outputs, submit (human approves), then continue with the next level
argument-hint: "[level]"
---
Play the contest, starting at level **$ARGUMENTS** (if that is empty, run `cargo ccc progress` and
start at the current level). Every step is a normal `cargo ccc` command; the human only approves the
`submit` prompt. Speed matters: the contest is timed. Don't stop between steps to ask, and don't narrate.

For each level N:

1. **Fetch:** `cargo ccc fetch N`, unless `work/levelN/` already exists.
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
5. **Produce:** `cargo ccc all N`. This is the one real run: it solves every input once (in
   parallel), saves the outputs and runs the checker. Don't do separate timing or `bench` runs of
   the large input; `all` reports the time per file, and its outputs are reused later. If it fails,
   fix the code and run `all` again.
6. **Submit:** `cargo ccc submit N --yes`. The human approves this prompt. Every wrong upload costs
   penalty time, so only submit when `all` reported no failures and you are confident.
   - All `[CORRECT]`: go straight on to level N+1 (step 1). Don't stop to summarise.
   - Any `[WRONG]`: read the per-line messages, find the misunderstanding, fix, rerun `all`, and only
     resubmit once you know why it was wrong. If you can't tell, stop and explain to the human.
7. Stop when `submit` says the game is complete (or `fetch` says there is no such level), or when the
   human interrupts.

Never edit `harness/`. If the harness is in the way, stop and say so.
