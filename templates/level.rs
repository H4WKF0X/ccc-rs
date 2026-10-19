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
