---
name: review
description: Sanity-check a level's produced outputs before the human submits
argument-hint: <level>
---
Level **$ARGUMENTS** passed the example. Before I submit, scrutinise it:

1. Re-read the statement for any rule the example didn't exercise (boundaries, large values,
   special cases) and check that `solution/src/levels/level$ARGUMENTS.rs` actually handles them.
2. Look at the produced files in `work/level$ARGUMENTS/out/`. Do the values look sane (right
   number of lines for the input's N, no overflow, no negatives where impossible, correct
   formatting)? Compare the shape with the example `.out`.
3. Flag anything that could make the platform reject it. If you find a real bug, fix the level
   file and tell me to rerun `cargo ccc all $ARGUMENTS`. Otherwise tell me it looks safe to submit.
