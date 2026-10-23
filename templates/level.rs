//! Level {{N}}: <one-line summary of the task>.
//!
//! Input:  <format, e.g. "N, then N lines of `x y`">
//! Output: <format, e.g. "one line per record: the answer">
//! Rule:   <the rule, in your own words, as worked out from the example>

#[allow(unused_imports)]
use harness::{parse::Tokens, trace};

pub fn solve(input: &str) -> Vec<String> {
    let mut t = Tokens::new(input);
    let n: usize = t.next();
    let out: Vec<String> = (0..n).map(|_| solve_one(&mut t)).collect();
    assert_eq!(out.len(), n, "one output line per record");
    out
}

fn solve_one(t: &mut Tokens) -> String {
    let _ = t;
    todo!("level {{N}}: read one record and return its output line")
}

// Only if this level accepts several valid answers: uncomment and write the statement's validity
// rules here, independently of `solve` (shared rules go in model.rs). The harness picks it up
// automatically, requires it to accept the official example answer, then uses it in validate, all
// and submit. Return Err with the case and reason, e.g. "case 3: path leaves the lawn at step 17".
//
// pub fn check(input: &str, output: &[String]) -> Result<(), String> {
//     todo!("level {{N}}: validity rules")
// }

// Only for an optimization level (the statement gives a score): the cost of each case, lower is
// better (negate a score where higher is better), Err if invalid. With it, `all` prints the score
// per file and keeps the best answer per case across runs; `cargo ccc best N` merges other outputs.
// Make searches anytime: `while !harness::budget::expired() { ... }`, then experiment cheaply with
// `cargo ccc all N --budget 5` (see /optimize).
//
// pub fn score(input: &str, output: &[String]) -> Result<Vec<i64>, String> {
//     todo!("level {{N}}: cost per case")
// }
