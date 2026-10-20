// Runs a level's sandbox checker (the inline <script> of sandbox-level-N.html) headlessly under node,
// on every case of one input/output file pair. Contract: checker/README.md.
//
// usage: sh checker/node-safe.sh checker/sandbox-check.js <sandbox.html> <input.txt> <output.txt>
//        (never plain `node`: the sandbox code is untrusted; node-safe.sh only lets it read checker/ and work/)
// exit:  0 = every case SUCCESS, 1 = an output is rejected, 2 = this checker can't judge (usage/IO
//        errors, the sandbox script doesn't run, or the files don't fit splitCases below).
//
// Written against the Spaceship game (CCC 41, platform base 0.36). The DOM stub and the sandbox entry
// points (initGameState, parseInputAndSolution, levelInput/levelSolution/spanStatus*) are the
// platform's shared sandbox framework. Only splitCases is game-specific.
const fs = require('fs');

// ======================= GAME-SPECIFIC: adapt this for a new game =======================
// The sandbox judges ONE case at a time; the files hold N cases. Return [[caseInput, caseOutput], ...]
// or a string explaining why the files don't fit (-> exit 2). Spaceship: the input is N, then per case
// a station line `x,y limit` followed by zero or more asteroid lines; the output has two non-empty
// lines (X and Y paces) per case.
function splitCases(inLines, outLines) {
  const n = Number(inLines[0]);
  const STATION = /^-?\d+,-?\d+ \d+$/;
  const inputs = [];
  for (const line of inLines.slice(1)) {
    if (STATION.test(line)) inputs.push([line]);
    else if (inputs.length && line) inputs[inputs.length - 1].push(line);
  }
  if (inputs.length !== n) return `input declares ${n} cases but ${inputs.length} station lines were found`;
  const answers = outLines.filter(l => l);
  if (answers.length !== 2 * n) {
    return { rejected: `expected ${2 * n} sequence lines (2 per case), output has ${answers.length}` };
  }
  return inputs.map((lines, i) => [lines.join('\n'), answers.slice(2 * i, 2 * i + 2).join('\n')]);
}
// ========================================================================================

function exit(code, msg) {
  console.log(msg);
  process.exit(code);
}

const [html, inFile, outFile] = process.argv.slice(2);
if (!html || !inFile || !outFile) exit(2, 'usage: node sandbox-check.js <sandbox.html> <input> <output>');

let script, inText, outText;
try {
  const m = fs.readFileSync(html, 'utf8').match(/<script>([\s\S]*)<\/script>/);
  if (!m) exit(2, `no inline <script> in ${html}`);
  script = m[1];
  inText = fs.readFileSync(inFile, 'utf8');
  outText = fs.readFileSync(outFile, 'utf8');
} catch (e) {
  exit(2, `cannot read files: ${e.message}`);
}

// --- Minimal DOM stub: every element accepts any property/method; we only read the status spans. ---
const el = () => new Proxy({ style: {}, value: '', textContent: '', attrs: {} }, {
  get: (t, k) => (k in t ? t[k]
    : k === 'setAttribute' ? (a, v) => (t.attrs[a] = v)
    : k === 'getContext' ? () => new Proxy({}, { get: () => () => ({}) })
    : () => {}),
});
const els = {};
global.document = { getElementById: id => (els[id] ??= el()), addEventListener() {} };
global.window = { requestAnimationFrame() {}, addEventListener() {}, devicePixelRatio: 1 };
global.addEventListener = () => {};
global.Path2D = class {};
global.ResizeObserver = class { observe() {} };

const cases = splitCases(inText.trim().split(/\r?\n/).map(l => l.trim()), outText.split(/\r?\n/).map(l => l.trim()));
if (typeof cases === 'string') exit(2, `files don't fit this checker: ${cases}`);
if (cases.rejected) exit(1, `REJECTED: ${cases.rejected}`);

// --- Run the sandbox's own parse + simulation per case (skip its init(): no canvas, no UI loop). ---
let results;
try {
  const run = new Function('cases', script.replace(/^\s*init\(\);?\s*$/m, '') + `
    initGameState();
    const results = [];
    for (const [input, solution] of cases) {
      levelInput.value = input;
      levelSolution.value = solution;
      spanStatus.textContent = 'NO STATUS';
      spanStatusMessage.textContent = 'the sandbox reported nothing';
      try { parseInputAndSolution(); } catch (e) { spanStatus.textContent = 'CRASH'; spanStatusMessage.textContent = String(e); }
      results.push([spanStatus.textContent, spanStatusMessage.textContent]);
    }
    return results;`);
  results = run(cases);
} catch (e) {
  exit(2, `the sandbox script does not run headlessly: ${e}`);
}

const bad = results.map(([status, msg], i) => ({ i, status, msg })).filter(r => r.status !== 'SUCCESS');
if (bad.length === 0) exit(0, `OK: ${cases.length}/${cases.length} cases SUCCESS`);
if (bad.every(r => r.status === 'NO STATUS' || r.status === 'CRASH')) {
  exit(2, `the sandbox gave no verdict (first: case ${bad[0].i}: ${bad[0].status} ${bad[0].msg})`);
}
// The harness shows the last lines, so list a few failures and end with a one-line verdict.
bad.slice(0, 3).forEach(r => console.log(`case ${r.i}: ${r.status} ${r.msg}`));
exit(1, `REJECTED: ${bad.length}/${cases.length} cases failed; first: case ${bad[0].i}: ${bad[0].status} ${bad[0].msg}`);
