//! An external output checker, for levels that accept many valid answers.
//!
//! Configured in `ccc.toml`:
//!
//! ```toml
//! checker = "node checker/sandbox-check.js {sandbox} {input} {output}"
//! ```
//!
//! The command runs from the repo root with the placeholders replaced by absolute paths. Exit code 0
//! means accepted, 1 means rejected (its output says why), and anything else (or failing to start)
//! means the checker could not judge. That last case only warns: a broken checker never blocks.
//! `{sandbox}` is the level's `statement/sandbox*.html` (the game's own checker). If the command uses it
//! and a level has none, that level is not checked. Arguments are split on whitespace (no quoting).
//!
//! Before the checker is trusted for a level, it must accept that level's official example answer
//! (`self_test`). A checker that rejects the organisers' own answer is wrong for this game
//! (e.g. it splits cases differently), so it is set aside with a warning instead of blocking.
//!
//! Prefer the game's own checker over one written alongside the solver: a checker that shares the
//! solver's misunderstanding of the rules proves nothing.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::layout::{Workspace, file_name};

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Accepted,
    Rejected(String),
    /// The checker could not judge (crashed, missing, doesn't fit this game).
    Broken(String),
}

pub struct Checker {
    argv: Vec<String>,
    sandbox: Option<PathBuf>,
}

impl Checker {
    /// The checker for level `n`, or `None` if none is configured or it doesn't apply to this level.
    pub fn for_level(ws: &Workspace, cfg: &Config, n: u32) -> Option<Checker> {
        let argv: Vec<String> = cfg.get("checker")?.split_whitespace().map(str::to_owned).collect();
        let sandbox = sandbox_file(ws, n);
        if argv.iter().any(|a| a.contains("{sandbox}")) && sandbox.is_none() {
            return None;
        }
        Some(Checker { argv, sandbox })
    }

    /// Run the checker on one input/output pair.
    pub fn check(&self, root: &Path, input: &Path, output: &Path) -> Verdict {
        let abs = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()).display().to_string();
        let sandbox = self.sandbox.as_deref().map(abs).unwrap_or_default();
        let args: Vec<String> = self
            .argv
            .iter()
            .map(|a| a.replace("{sandbox}", &sandbox).replace("{input}", &abs(input)).replace("{output}", &abs(output)))
            .collect();
        let result = match Command::new(&args[0]).args(&args[1..]).current_dir(root).output() {
            Ok(result) => result,
            Err(e) => return Verdict::Broken(format!("`{}` could not start: {e}", args[0])),
        };
        let text = format!("{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let tail = lines[lines.len().saturating_sub(5)..].join(" | ");
        let message = if tail.is_empty() { "no message".to_owned() } else { tail };
        match result.status.code() {
            Some(0) => Verdict::Accepted,
            Some(1) => Verdict::Rejected(message),
            _ => Verdict::Broken(format!("{}: {message}", result.status)),
        }
    }

    /// Does the checker accept the official answer for every example of level `n`? `Err` says why not.
    /// Levels without an expected example output pass (nothing to test against).
    pub fn self_test(&self, ws: &Workspace, n: u32) -> Result<(), String> {
        for example in ws.examples(n).unwrap_or_default() {
            let Some(expected) = example.expected else { continue };
            match self.check(&ws.root, &example.input, &expected) {
                Verdict::Accepted => {}
                Verdict::Rejected(why) | Verdict::Broken(why) => {
                    return Err(format!("it does not accept the official answer {}: {why}", file_name(&expected)));
                }
            }
        }
        Ok(())
    }

    pub fn describe(&self) -> String {
        match &self.sandbox {
            Some(s) => format!("{} with {}", self.argv[..2.min(self.argv.len())].join(" "), file_name(s)),
            None => self.argv[..2.min(self.argv.len())].join(" "),
        }
    }
}

/// `work/level<N>/statement/sandbox*.html`, if the level shipped one.
pub fn sandbox_file(ws: &Workspace, n: u32) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(ws.statement_dir(n))
        .ok()?
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| {
            let name = file_name(p).to_lowercase();
            name.starts_with("sandbox") && name.ends_with(".html")
        })
        .collect();
    found.sort();
    found.into_iter().next()
}
