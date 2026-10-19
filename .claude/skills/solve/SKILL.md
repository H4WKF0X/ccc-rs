---
name: solve
description: Solve a CCC level — read the statement + example, then implement solution/src/levels/level<N>.rs
argument-hint: <level>
---
Solve level **$ARGUMENTS** of this contest.

1. Read the problem statement in `work/level$ARGUMENTS/statement/` (PDF, HTML, or whatever was
   downloaded). If an earlier level's statement exists, skim what changed.
2. Read the worked example: `work/level$ARGUMENTS/in/*example*.in` and its matching `.out`. Work out
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
   iterate until GO. Then reason about edge cases from the statement that the example doesn't
   cover (empty records, zeros/negatives, ties, off-by-one, overflow).

Do not run `cargo ccc submit` and do not edit `harness/`. When it's GO, tell me to run
`cargo ccc all $ARGUMENTS`, and report `cargo ccc bench $ARGUMENTS` timings if the inputs are large.
