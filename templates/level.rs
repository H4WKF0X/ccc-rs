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
