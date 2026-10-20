---
name: solve
description: Solve a CCC level — read the statement + example, then implement solution/src/levels/level<N>.rs
argument-hint: <level>
---
Solve level **$ARGUMENTS** of this contest.

1. Read the problem statement in `work/level$ARGUMENTS/statement/` (`level-$ARGUMENTS.pdf`). If an
   earlier level's statement exists, skim what changed. If there is a `sandbox-level-$ARGUMENTS.html`,
   read its `<script>`: it is the game's visualiser, and often contains the exact rules (format
   validation, simulation) where the PDF is vague.
2. Read the worked example in `work/level$ARGUMENTS/in/`: `in_level-$ARGUMENTS_0-example.txt` and
   `out_level-$ARGUMENTS_0-example.txt` (old CatCoder naming: `*example*.in` + `.out`). Work out
   by hand the exact rule that turns the input into that output, including the precise output
   format (numbers per line, spacing, separators, blank lines).
3. Look at the sizes in `work/level$ARGUMENTS/in/`. The largest real input tells you whether
   performance matters.
4. If this level reuses parsing or a data model from an earlier level, reuse or extend
   `solution/src/model.rs` instead of duplicating it. To start from the previous level's code, run
   `cargo ccc new-level $ARGUMENTS --from <previous>`, or `cargo ccc new-level $ARGUMENTS` for the stub.
5. Implement `solution/src/levels/level$ARGUMENTS.rs` (`pub fn solve(input: &str) -> Vec<String>`),
   following the I/O contract and coding standards in CLAUDE.md. Fill in the doc comment at the
   top (input, output, rule).
6. Run `cargo ccc validate $ARGUMENTS`. On NO-GO, debug from the first difference it prints and
   iterate until GO. If the level allows several valid answers, a different-but-valid answer gives
   NO-GO. Don't contort the solution to copy the example. Add `pub fn check` to the level file
   instead ("Levels with many valid answers" in CLAUDE.md). Then `validate` reports "GO (checker)". Then reason about edge cases from the statement that the example doesn't
   cover (empty records, zeros/negatives, ties, off-by-one, overflow).

When it's GO, run `cargo ccc all $ARGUMENTS`: it solves every real input once (in parallel), saves
the outputs (reused later, so no need to run them again) and runs the checker. Don't do separate
timing runs of the large input; `all` reports the times. Use `harness::par::map` for slow independent
cases. Do not run `cargo ccc submit` (use `/level` for the full loop) and do not edit `harness/`.
