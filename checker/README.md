# checker/

Optional output checker for levels that accept **many valid answers**. A byte-for-byte match with the
example's answer can't work there, so the game's own checker decides instead. Enable it in `ccc.toml`:

```toml
checker = "node checker/sandbox-check.js {sandbox} {input} {output}"
```

**Contract:** the command runs from the repo root with absolute paths substituted:
- `{sandbox}`: `work/level<N>/statement/sandbox-level-<N>.html`. Levels without one are not checked.
- `{input}`, `{output}`: the input and the produced output.

Exit codes:
- `0`: accepted
- `1`: rejected; print the reason (the last lines are shown)
- anything else: the checker couldn't judge. This **warns only**, it never blocks.

**Self-test:** before the checker is trusted on a level, it must accept that level's official example
answer. A checker that rejects the organisers' own answer doesn't fit the game (e.g. it splits cases
wrongly), so it is ignored with a warning. Turning it on by default (as the template does) is safe.

When configured and self-tested, the harness:
- passes `validate` for a different-but-accepted example answer
- checks every real output in `all`
- refuses `submit` if any real output is rejected

Verdicts are stored with each output, so a file is checked once per build.

**Where it comes from:** a level's `sandbox-level-<N>.html` is the game's offline visualiser, and its
inline `<script>` holds the real validation and simulation. `sandbox-check.js` loads that script under
node with a stubbed DOM, feeds it the input and output, and reports the verdict. Sandbox scripts are
the same across a game's levels (and likely across games on the same platform base), so adapt the
script once per game. Only `splitCases` at the top of `sandbox-check.js` is game-specific. Prefer this over a checker written alongside the solver: the game's code is
independent ground truth.

Requires `node` on the PATH.
