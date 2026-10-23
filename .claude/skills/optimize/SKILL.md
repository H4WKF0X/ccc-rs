---
name: optimize
description: Work an optimization (scored) level — fast valid baseline, then big structural improvements tested with cheap, time-boxed experiments
argument-hint: <level>
---
Level **$ARGUMENTS** is an optimization level: every file gets a score, and a better score beats
other teams. Time is the scarce resource, so aim for **the biggest score gain per minute**. At the
school contest, an hour went into small tweaks while big improvements were missed. Don't repeat that.

1. **Baseline (aim for 15 min):** `pub fn check` (validity), `pub fn score` (cost per case, lower is
   better; negate if higher is better) and a simple valid `solve`. Then `cargo ccc all $ARGUMENTS` and
   `cargo ccc submit $ARGUMENTS --yes`. This secures the level.
2. **If another level is unlocked now, ask the human:** optimize now, or move on and come back?
   That's a team strategy call.
3. **Think in big steps before coding (a few minutes, no code):** what is this problem
   mathematically? Write down **2–3 fundamentally different approaches** (a different model or
   algorithm, not variants of the current one) with a rough guess of how much each could gain and
   why. Pick the one with the largest expected gain and tell the human the plan in three lines.
   They may know better from the leaderboard or teammates.
   **If the problem has linear costs and counts (buy/produce/assign/route), the first big idea is
   an exact MILP model solved by a real solver**: HiGHS is ready (`~/.ccc-venv/bin/python`,
   `import highspy`). Model ordering/time explicitly (e.g. phases or time-indexed inventory) if
   things must exist before they are used. Never write your own branch-and-bound. Python outputs go
   to `work/level$ARGUMENTS/candidates/`, then `cargo ccc best $ARGUMENTS` (validated with `check`
   and `score`, merged per case).
4. **Make every experiment cheap:** test an idea on the small file or a few cases with a short
   budget, `cargo ccc all $ARGUMENTS --budget 5`, and read the per-file score lines. Searches must be
   anytime (`while !harness::budget::expired() { ... }` keeps the best so far). Never wait minutes
   for an experiment, and never write ad-hoc scoring scripts: `score` and `all` already do it.
5. **Each iteration is a different idea, not a parameter tweak.** Keep a short scoreboard in your
   messages: idea → score per file. If an idea gains under 1%, don't tune it further. Go back to
   step 3 for the next big idea.
6. **When an idea wins, do the long run** (`cargo ccc all $ARGUMENTS --budget 120`; `all` keeps the
   best answer per case, so nothing is lost) and submit. Only changed files are uploaded.
7. **Pool everyone's work:** teammates' or other sessions' outputs go into
   `work/level$ARGUMENTS/candidates/`; `cargo ccc best $ARGUMENTS` takes the best answer per case.

Don't: spend time on lower bounds, hard-code outputs into the source, or grind seeds and
parameters before the next structural idea has been tried.
