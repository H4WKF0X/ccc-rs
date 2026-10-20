//! Getting levels from and outputs to the contest platform. Picked by `platform = "..."` in `ccc.toml`.
//!
//! - `codingcontest` (default): talks to codingcontest.org directly (`login`, `fetch`, `progress`, `submit`).
//! - `manual`: you download/upload in the browser; `submit` lists what to upload.
//!
//! The harness has already re-validated the example and checked the outputs are fresh before
//! calling `submit`.

mod codingcontest;
mod firefox;
mod manual;

use std::path::PathBuf;

use crate::config::Config;
use crate::layout::Workspace;

fn platform(cfg: &Config) -> Result<&str, String> {
    match cfg.get("platform").unwrap_or("codingcontest") {
        p @ ("manual" | "codingcontest") => Ok(p),
        other => Err(format!("unknown platform {other:?} in ccc.toml (use \"manual\" or \"codingcontest\")")),
    }
}

fn needs_client(cfg: &Config, what: &str) -> Result<(), String> {
    match platform(cfg)? {
        "codingcontest" => Ok(()),
        _ => Err(format!("`{what}` talks to codingcontest.org: set platform = \"codingcontest\" in ccc.toml")),
    }
}

pub fn submit(ws: &Workspace, cfg: &Config, level: u32, outputs: &[PathBuf], yes: bool) -> Result<bool, String> {
    match platform(cfg)? {
        "codingcontest" => codingcontest::submit(cfg, level, outputs, yes),
        _ => manual::submit(ws, level, outputs),
    }
}

/// Download level `level`'s zip and ingest it.
pub fn fetch(ws: &Workspace, cfg: &Config, level: u32) -> Result<bool, String> {
    needs_client(cfg, "fetch")?;
    codingcontest::fetch(ws, cfg, level)
}

/// Check the login and show who you are, which contest was found, and your progress.
pub fn login(cfg: &Config) -> Result<bool, String> {
    needs_client(cfg, "login")?;
    codingcontest::login(cfg)
}

/// Show the team's progress as the platform sees it (also a cheap check that auth works).
pub fn progress(cfg: &Config) -> Result<bool, String> {
    needs_client(cfg, "progress")?;
    codingcontest::progress(cfg)
}
