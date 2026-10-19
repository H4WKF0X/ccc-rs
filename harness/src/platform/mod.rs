//! Getting outputs to the contest platform. Picked by `platform = "..."` in `ccc.toml`.
//!
//! - `manual` (default): print what to upload; you upload it in the browser.
//! - `codingcontest`: talk to codingcontest.org directly (not written yet, see docs/PLATFORM.md).
//!
//! The harness has already re-validated the example and checked the outputs are fresh before
//! calling in here.

mod codingcontest;
mod manual;

use std::path::PathBuf;

use crate::config::Config;
use crate::layout::Workspace;

pub fn submit(ws: &Workspace, cfg: &Config, level: u32, outputs: &[PathBuf], yes: bool) -> Result<bool, String> {
    match cfg.get("platform").unwrap_or("manual") {
        "manual" => manual::submit(ws, level, outputs),
        "codingcontest" => codingcontest::submit(cfg, level, outputs, yes),
        other => Err(format!("unknown platform {other:?} in ccc.toml (use \"manual\" or \"codingcontest\")")),
    }
}
