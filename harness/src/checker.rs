//! An external output checker, for levels that accept many valid answers.
//!
//! Configured in `ccc.toml`:
//!
//! ```toml
//! checker = "node checker/sandbox-check.js {sandbox} {input} {output}"
//! ```
//!
//! The command runs from the repo root with the placeholders replaced by absolute paths. Exit code 0
//! means the output is accepted. Anything else means rejected, and the command's output explains why.
//! `{sandbox}` is the level's `statement/sandbox*.html` (the game's own checker). If the command uses it
//! and a level has none, that level is not checked. Arguments are split on whitespace (no quoting).
//!
//! Prefer the game's own checker over one written alongside the solver: a checker that shares the
//! solver's misunderstanding of the rules proves nothing.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::layout::{Workspace, file_name};

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

    /// `Ok` if the checker accepts `output` for `input`; `Err` explains the rejection (or why the
    /// checker could not run).
    pub fn check(&self, root: &Path, input: &Path, output: &Path) -> Result<(), String> {
        let abs = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()).display().to_string();
        let sandbox = self.sandbox.as_deref().map(abs).unwrap_or_default();
        let args: Vec<String> = self
            .argv
            .iter()
            .map(|a| a.replace("{sandbox}", &sandbox).replace("{input}", &abs(input)).replace("{output}", &abs(output)))
            .collect();
        let result = Command::new(&args[0])
            .args(&args[1..])
            .current_dir(root)
            .output()
            .map_err(|e| format!("checker `{}` could not start: {e}", args[0]))?;
        if result.status.success() {
            return Ok(());
        }
        let text = format!("{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let tail = lines[lines.len().saturating_sub(5)..].join(" | ");
        Err(format!("rejected ({}): {}", result.status, if tail.is_empty() { "no message" } else { &tail }))
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
