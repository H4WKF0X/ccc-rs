//! Compare produced output against an expected `.out`: the GO / NO-GO gate.

use crate::io::{self, OutputStyle};

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Byte-for-byte identical to the expected file.
    Go,
    /// Same values, but whitespace or line endings differ.
    GoWarn,
    /// Different; the string explains where.
    NoGo(String),
}

pub fn compare(produced: &[String], expected_raw: &[u8], style: OutputStyle) -> Verdict {
    if style.join(produced).as_bytes() == expected_raw {
        return Verdict::Go;
    }
    let expected = io::lines(&io::normalize(&String::from_utf8_lossy(expected_raw)));
    if normalized(produced) == normalized(&expected) {
        return Verdict::GoWarn;
    }
    Verdict::NoGo(first_difference(produced, &expected))
}

/// Trim every line and drop trailing blank lines: the differences a judge usually forgives.
fn normalized(lines: &[String]) -> Vec<&str> {
    let mut out: Vec<&str> = lines.iter().map(|l| l.trim()).collect();
    while out.last() == Some(&"") {
        out.pop();
    }
    out
}

fn first_difference(produced: &[String], expected: &[String]) -> String {
    let counts = format!("    (produced {} lines, expected {})", produced.len(), expected.len());
    for i in 0..produced.len().max(expected.len()) {
        let got = produced.get(i).map_or("<missing>", String::as_str);
        let want = expected.get(i).map_or("<missing>", String::as_str);
        if got.trim() != want.trim() {
            return format!(
                "    first difference at line {}:\n      expected: {}\n      got:      {}\n{counts}",
                i + 1,
                clip(want),
                clip(got)
            );
        }
    }
    counts
}

/// Keep diff output readable when a line is huge.
fn clip(line: &str) -> String {
    const MAX: usize = 200;
    match line.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}… ({} chars)", &line[..cut], line.chars().count()),
        None => line.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_match_is_go() {
        assert_eq!(compare(&v(&["1 2", "3"]), b"1 2\r\n3\r\n", OutputStyle::CRLF), Verdict::Go);
    }

    #[test]
    fn whitespace_only_difference_is_warning() {
        assert_eq!(compare(&v(&["1 2 ", "3", ""]), b"1 2\r\n3\r\n", OutputStyle::CRLF), Verdict::GoWarn);
        assert_eq!(compare(&v(&["1"]), b"1\n", OutputStyle::CRLF), Verdict::GoWarn);
    }

    #[test]
    fn value_difference_reports_first_line() {
        let Verdict::NoGo(msg) = compare(&v(&["1", "5", "9"]), b"1\r\n4\r\n9\r\n", OutputStyle::CRLF) else {
            panic!("expected NO-GO");
        };
        assert!(msg.contains("line 2") && msg.contains("expected: 4") && msg.contains("got:      5"), "{msg}");
    }

    #[test]
    fn missing_lines_are_reported() {
        let Verdict::NoGo(msg) = compare(&v(&["1"]), b"1\r\n2\r\n", OutputStyle::CRLF) else {
            panic!("expected NO-GO");
        };
        assert!(msg.contains("got:      <missing>") && msg.contains("produced 1 lines, expected 2"), "{msg}");
    }
}
