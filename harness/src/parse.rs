//! Small, fast parsing helpers for contest input. No regex, no allocation per token.
//!
//! ```ignore
//! let mut t = Tokens::new(input);
//! let n: usize = t.next();
//! for _ in 0..n {
//!     let (x, y): (i64, i64) = (t.next(), t.next());
//! }
//! ```

use std::fmt::Debug;
use std::str::{FromStr, SplitAsciiWhitespace};

/// Whitespace-separated tokens across the whole input (line breaks are just whitespace).
pub struct Tokens<'a> {
    it: SplitAsciiWhitespace<'a>,
    taken: usize,
}

impl<'a> Tokens<'a> {
    pub fn new(input: &'a str) -> Self {
        Tokens { it: input.split_ascii_whitespace(), taken: 0 }
    }

    /// Next token parsed as `T`. Panics with the token index and text if it is missing or invalid.
    /// (Named like `Iterator::next` on purpose: `t.next::<i64>()` is the contest idiom.)
    #[allow(clippy::should_implement_trait)]
    pub fn next<T: FromStr>(&mut self) -> T
    where
        T::Err: Debug,
    {
        let tok = self.next_str();
        tok.parse().unwrap_or_else(|e| {
            panic!(
                "token #{} {tok:?} is not a valid {}: {e:?}",
                self.taken,
                std::any::type_name::<T>()
            )
        })
    }

    /// Next raw token. Panics if the input ran out.
    pub fn next_str(&mut self) -> &'a str {
        self.taken += 1;
        self.it
            .next()
            .unwrap_or_else(|| panic!("input ended early: wanted token #{}", self.taken))
    }

    /// Next `n` tokens parsed as `T`.
    pub fn vec<T: FromStr>(&mut self, n: usize) -> Vec<T>
    where
        T::Err: Debug,
    {
        (0..n).map(|_| self.next()).collect()
    }

    /// Next token if there is one (for "read until the end" formats).
    pub fn try_next<T: FromStr>(&mut self) -> Option<T>
    where
        T::Err: Debug,
    {
        let tok = self.it.next()?;
        self.taken += 1;
        Some(tok.parse().unwrap_or_else(|e| panic!("token #{} {tok:?} is invalid: {e:?}", self.taken)))
    }
}

/// Every integer in a line, whatever the separators (`"3,-4 x=5"` gives `[3, -4, 5]`).
pub fn ints(line: &str) -> Vec<i64> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let negative = bytes[i] == b'-' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit);
        if bytes[i].is_ascii_digit() || negative {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            out.push(line[start..i].parse().expect("digit run fits in i64"));
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_span_lines() {
        let mut t = Tokens::new("2\n10 -3\nabc");
        assert_eq!(t.next::<usize>(), 2);
        assert_eq!(t.vec::<i64>(2), vec![10, -3]);
        assert_eq!(t.next_str(), "abc");
        assert_eq!(t.try_next::<i64>(), None);
    }

    #[test]
    #[should_panic(expected = "input ended early")]
    fn tokens_panic_when_exhausted() {
        Tokens::new("").next::<i32>();
    }

    #[test]
    fn ints_ignores_separators() {
        assert_eq!(ints("3,-4 x=5 a-b 7"), vec![3, -4, 5, 7]);
        assert!(ints("none").is_empty());
    }
}
