//! Getting levels from and outputs to the contest platform. Picked by `platform = "..."` in `ccc.toml`.
//!
//! - `manual` (default): you download/upload in the browser; `submit` lists what to upload.
//! - `codingcontest`: talks to codingcontest.org directly (`fetch`, `progress`, `submit`).
//!
//! The harness has already re-validated the example and checked the outputs are fresh before
//! calling `submit`.

mod codingcontest;
mod manual;

use std::path::PathBuf;

use crate::config::Config;
use crate::layout::Workspace;

fn platform(cfg: &Config) -> Result<&str, String> {
    match cfg.get("platform").unwrap_or("manual") {
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

/// Show the team's progress as the platform sees it (also a cheap check that auth works).
pub fn progress(cfg: &Config) -> Result<bool, String> {
    needs_client(cfg, "progress")?;
    codingcontest::progress(cfg)
}
