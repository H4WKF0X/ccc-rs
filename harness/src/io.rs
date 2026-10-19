//! The I/O contract.
//!
//! Reading is tolerant: CRLF, CR and LF are all accepted, a UTF-8 BOM is dropped, and one trailing
//! newline is removed so a solver never sees a phantom empty last line.
//!
//! Writing is strict: the line ending style (CRLF or LF, final newline or not) is copied from
//! the example `.out`, so a correct solver matches it byte for byte.

use std::fs;
use std::path::Path;

/// Normalise raw file text to `\n` line endings without a BOM or a trailing newline.
pub fn normalize(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut s = text.replace("\r\n", "\n").replace('\r', "\n");
    if s.ends_with('\n') {
        s.pop();
    }
    s
}

/// Read an input file as normalised text (see [`normalize`]).
pub fn read_input(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    Ok(normalize(&String::from_utf8_lossy(&bytes)))
}

/// Split normalised text into lines; empty text is zero lines.
pub fn lines(text: &str) -> Vec<String> {
    if text.is_empty() { Vec::new() } else { text.split('\n').map(str::to_owned).collect() }
}

/// How output files are written: line terminator and whether the last line has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputStyle {
    pub crlf: bool,
    pub final_newline: bool,
}

impl OutputStyle {
    /// What CatCoder used: CRLF after every line, including the last.
    pub const CRLF: OutputStyle = OutputStyle { crlf: true, final_newline: true };
    pub const LF: OutputStyle = OutputStyle { crlf: false, final_newline: true };

    /// Copy the style of an expected output file.
    pub fn detect(expected: &[u8]) -> OutputStyle {
        let crlf = expected.windows(2).any(|w| w == b"\r\n");
        let final_newline = expected.is_empty() || expected.ends_with(b"\n");
        OutputStyle { crlf, final_newline }
    }

    pub fn describe(self) -> String {
        format!(
            "{}{}",
            if self.crlf { "CRLF" } else { "LF" },
            if self.final_newline { "" } else { ", no final newline" }
        )
    }

    /// Join lines into file content in this style.
    pub fn join(self, lines: &[String]) -> String {
        let eol = if self.crlf { "\r\n" } else { "\n" };
        let mut s = String::with_capacity(lines.iter().map(|l| l.len() + 2).sum());
        for (i, line) in lines.iter().enumerate() {
            s.push_str(line);
            if i + 1 < lines.len() || self.final_newline {
                s.push_str(eol);
            }
        }
        s
    }
}

/// Write output lines in the given style, creating parent directories as needed.
pub fn write_lines(path: &Path, lines: &[String], style: OutputStyle) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    fs::write(path, style.join(lines)).map_err(|e| format!("could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn normalize_accepts_every_line_ending_and_drops_one_trailing_newline() {
        assert_eq!(normalize("a\r\nb\rc\nd\r\n"), "a\nb\nc\nd");
        assert_eq!(normalize("\u{feff}3\n"), "3");
        assert_eq!(normalize("a\n\n"), "a\n");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn lines_of_empty_text_is_empty() {
        assert!(lines("").is_empty());
        assert_eq!(lines("a\n\nb"), v(&["a", "", "b"]));
    }

    #[test]
    fn detect_and_join_round_trip() {
        for raw in ["1\r\n2\r\n", "1\n2\n", "1\n2", "1\r\n2"] {
            let style = OutputStyle::detect(raw.as_bytes());
            assert_eq!(style.join(&v(&["1", "2"])), raw, "style {style:?}");
        }
        assert_eq!(OutputStyle::CRLF.join(&[]), "");
    }
}
