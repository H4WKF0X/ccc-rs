//! Optimization levels: keep the best answer per case, from every run and every source.
//!
//! A level with `pub fn score` reports the cost of each case (lower is better). The harness then
//! never lets an output get worse. Each new output (from `all`, or candidates passed to `best`) is
//! merged case by case with the best so far, which lives in `work/level<N>/best/<output>`. That
//! makes trying a risky idea free, and combines teammates' files per case, not just per file.
//!
//! Cases in an output are paragraphs separated by blank lines (the CCC convention). A file that
//! doesn't split into one paragraph per scored case can still win as a whole. Every merge is
//! re-scored as a whole before it's used, so dependent cases can't produce a wrong result.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::ScoreFn;

/// One valid candidate output for an input, with its cost per case.
#[derive(Clone, Debug)]
pub struct Scored {
    pub lines: Vec<String>,
    pub costs: Vec<i64>,
    pub source: String,
}

impl Scored {
    pub fn total(&self) -> i64 {
        self.costs.iter().sum()
    }
}

/// The result of a merge: the lines to write and where each case came from.
#[derive(Debug)]
pub struct Merged {
    pub best: Scored,
    /// Per case, the source it was taken from (empty when a whole file won).
    pub picks: Vec<String>,
}

/// Score `lines` for `input`; `Err` if invalid (or the scorer panics).
pub fn evaluate(score: ScoreFn, input: &str, lines: Vec<String>, source: &str) -> Result<Scored, String> {
    match catch_unwind(AssertUnwindSafe(|| score(input, &lines))) {
        Ok(Ok(costs)) => Ok(Scored { lines, costs, source: source.to_owned() }),
        Ok(Err(why)) => Err(why),
        Err(payload) => Err(format!(
            "score panicked: {}",
            payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default()
        )),
    }
}

/// Paragraphs separated by blank lines (runs of blank lines count once; leading/trailing ignored).
pub fn split_cases(lines: &[String]) -> Vec<Vec<String>> {
    let mut cases = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            if !current.is_empty() {
                cases.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line.clone());
        }
    }
    if !current.is_empty() {
        cases.push(current);
    }
    cases
}

/// The inverse of [`split_cases`]: one blank line between cases.
pub fn join_cases(cases: &[Vec<String>]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        if i > 0 {
            out.push(String::new());
        }
        out.extend(case.iter().cloned());
    }
    out
}

/// Best per case across candidates that split into one paragraph per case; a candidate that
/// doesn't split that way can still win as a whole. `verify` re-scores the merged lines (the
/// merge is used only if its score matches). Ties keep the earlier candidate.
pub fn merge(candidates: &[Scored], verify: impl Fn(Vec<String>) -> Option<Scored>) -> Option<Merged> {
    let whole_best = candidates.iter().min_by_key(|c| c.total())?;
    let cases = whole_best.costs.len();
    let splittable: Vec<(&Scored, Vec<Vec<String>>)> = candidates
        .iter()
        .filter(|c| c.costs.len() == cases)
        .map(|c| (c, split_cases(&c.lines)))
        .filter(|(_, parts)| parts.len() == cases)
        .collect();
    let whole = || Merged { best: whole_best.clone(), picks: Vec::new() };
    if splittable.len() < 2 {
        return Some(whole());
    }
    let mut parts = Vec::with_capacity(cases);
    let mut picks = Vec::with_capacity(cases);
    let mut expected = 0;
    for i in 0..cases {
        let (winner, winner_parts) = splittable.iter().min_by_key(|(c, _)| c.costs[i]).unwrap();
        parts.push(winner_parts[i].clone());
        picks.push(winner.source.clone());
        expected += winner.costs[i];
    }
    if expected >= whole_best.total() {
        return Some(whole());
    }
    match verify(join_cases(&parts)) {
        Some(merged) if merged.total() == expected => Some(Merged { best: merged, picks }),
        _ => Some(whole()), // cases weren't independent after all: keep the best whole file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    /// Toy scorer: each case paragraph costs its number of lines.
    fn toy(_input: &str, out: &[String]) -> Result<Vec<i64>, String> {
        Ok(split_cases(out).iter().map(|c| c.len() as i64).collect())
    }

    #[test]
    fn splits_and_joins_paragraphs() {
        let lines = v(&["", "a", "b", "", "", "c", ""]);
        let cases = split_cases(&lines);
        assert_eq!(cases, vec![v(&["a", "b"]), v(&["c"])]);
        assert_eq!(join_cases(&cases), v(&["a", "b", "", "c"]));
    }

    #[test]
    fn takes_the_best_case_from_each_candidate() {
        let a = evaluate(toy, "", v(&["x", "x", "x", "", "y"]), "a").unwrap(); // 3 + 1
        let b = evaluate(toy, "", v(&["x", "", "y", "y", "y"]), "b").unwrap(); // 1 + 3
        let merged = merge(&[a, b], |lines| evaluate(toy, "", lines, "merged").ok()).unwrap();
        assert_eq!(merged.best.costs, vec![1, 1]);
        assert_eq!(merged.best.lines, v(&["x", "", "y"]));
        assert_eq!(merged.picks, vec!["b", "a"]);
    }

    #[test]
    fn falls_back_to_the_best_whole_file_when_the_merge_does_not_verify() {
        let a = evaluate(toy, "", v(&["x", "x", "", "y"]), "a").unwrap(); // 2 + 1
        let b = evaluate(toy, "", v(&["x", "", "y", "y"]), "b").unwrap(); // 1 + 2
        let merged = merge(&[a, b], |_| None).unwrap();
        assert_eq!(merged.best.source, "a");
        assert!(merged.picks.is_empty());
    }

    #[test]
    fn invalid_outputs_are_errors() {
        let bad = |_: &str, _: &[String]| -> Result<Vec<i64>, String> { Err("case 2: not enough C".into()) };
        assert!(evaluate(bad, "", v(&["x"]), "a").is_err());
    }
}
