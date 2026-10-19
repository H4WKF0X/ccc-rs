# CCC contest repo — project memory for Claude Code

> In the `ccc-rs` template itself, this file describes the contest repos it creates. Template
> work means changing `harness/`, `templates/`, `new-contest.*` or these docs.

This repo solves one **Cloudflight Coding Contest** game. A game is a series of levels of increasing
difficulty; later levels reuse the input format and data model of earlier ones. The repo was
created from the `ccc-rs` template; the harness is shared, the solutions are this contest's.

**Your job, when asked to solve a level:** read the statement, infer the rule from the worked
example, and implement one file, `solution/src/levels/level<N>.rs`, that passes the example
byte for byte. A human runs validation and submission. You never upload anything.

## The loop (who does what)

1. Human runs `cargo ccc fetch <N>` (or downloads in the browser + `cargo ccc ingest`). That fills `work/level<N>/`:
   - `in/in_level-<N>_0-example.txt` + `in/out_level-<N>_0-example.txt`: the worked example (known answer)
   - `in/in_level-<N>_1-small.txt`, `in/in_level-<N>_2-large.txt`: the real inputs (answers unknown)
   - `statement/level-<N>.pdf`: the problem description. Read this first.
   - `statement/sandbox-level-<N>.html` (some levels): an offline visualiser whose `<script>` is the
     game's checker, so it has the exact rules.
   (Old CatCoder naming `level<N>_0_example.in/.out` also works.)
2. **You** read the statement and the example pair, then write or extend `level<N>.rs`.
3. You may run `cargo ccc validate <N>` yourself and iterate until it says GO.
4. Human runs `cargo ccc all <N>` (validate, then write the real outputs), reviews, then
   `cargo ccc submit <N>`. Wrong uploads cost penalty minutes.

## The I/O contract — do not break it

```rust
pub fn solve(input: &str) -> Vec<String>
```

- `input` is one whole input file, line endings normalised to `\n`, no trailing newline.
- Return the output lines **without** line endings. The harness writes them in the example's style.
- The first line is usually a count `N`, then `N` records. Confirm against the statement.
- **Match the example's output format exactly**: same numbers per line, same separators, blank
  lines where the example has them. If `validate` says NO-GO, the format or logic is wrong, not
  the harness. **Exception:** levels that accept many valid answers (usually sandbox levels). There the
  game's checker decides (`checker` in `ccc.toml`, `checker/README.md`). Don't distort a valid
  solution to copy the example's arbitrary choices.
- Levels are registered automatically from file names (`solution/build.rs`). Adding a level means
  adding one file. `cargo ccc new-level <N> [--from <M>]` scaffolds it.
- Do **not** edit `harness/` to make a level work. If the harness is truly wrong, say so.

## Reusing earlier levels (this matters a lot)

Levels build on each other. When parsing or a data model recurs, move it into
`solution/src/model.rs` (or another module next to it) and use it from both levels. Don't
copy-paste. A clean model from level 2 makes level 5 tractable. Starting level N from
level N-1's code (`--from`) is often right.

## Coding standards

- **Debuggable without a debugger.** Build `solve` from small, pure functions. Trace with
  `harness::trace!(...)`, which goes to stderr only when `CCC_DEBUG=1`. Never print to stdout. Add sanity
  checks (`assert_eq!(out.len(), n)`), because a panic names the input file.
- **Parsing:** `harness::parse::Tokens` (`t.next::<i64>()`, `t.vec::<u32>(n)`) for whitespace
  formats. `harness::parse::ints(line)` pulls every integer out of a line whatever the separators.
- **Correctness first, then speed.** `_large` inputs can be big and later levels are scored on
  performance: prefer O(n) / O(n log n), and use `i64`/`u64` wherever sums or products can grow.
  Release builds keep overflow checks on, so an overflow panics instead of silently wrapping.
- **Deterministic.** No randomness, and never depend on `HashMap` iteration order; use
  `BTreeMap` or sort when order reaches the output.
- Plain std Rust. No new crates without asking. Clear names over cleverness.
- Unit tests for tricky helpers go in the level file (`#[cfg(test)] mod tests`); `cargo test` runs them.

## Commands

`cargo ccc fetch N` · `cargo ccc ingest [zip|dir|files] [--level N]` · `cargo ccc validate N` · `cargo ccc all N` ·
`cargo ccc run N <in> [out]` · `cargo ccc bench N` · `cargo ccc new-level N [--from M]` ·
`cargo ccc status` · `cargo ccc progress` · `cargo ccc submit N` (human only).

The platform is codingcontest.org (CatCoder is gone); see `docs/PLATFORM.md`. Output line endings
are copied from the example (codingcontest.org uses LF).
