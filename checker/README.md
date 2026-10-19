# checker/

Optional output checker for levels that accept **many valid answers**. A byte-for-byte match with the
example's answer can't work there, so the game's own checker decides instead. Enable it in `ccc.toml`:

```toml
checker = "node checker/sandbox-check.js {sandbox} {input} {output}"
```

**Contract:** the command runs from the repo root with absolute paths substituted:
- `{sandbox}`: `work/level<N>/statement/sandbox-level-<N>.html`. Levels without one are not checked.
- `{input}`, `{output}`: the input and the produced output.

Exit 0 means accepted. Anything else means rejected; print the reason (the last lines are shown).

When configured, the harness:
- passes `validate` for a different-but-accepted example answer
- checks every real output in `all`
- refuses `submit` if any real output is rejected

**Where it comes from:** a level's `sandbox-level-<N>.html` is the game's offline visualiser, and its
inline `<script>` holds the real validation and simulation. `sandbox-check.js` loads that script under
node with a stubbed DOM, feeds it the input and output, and reports the verdict. Sandbox scripts are
the same across a game's levels (and likely across games on the same platform base), so adapt the
script once per game. Prefer this over a checker written alongside the solver: the game's code is
independent ground truth.

Requires `node` on the PATH.
