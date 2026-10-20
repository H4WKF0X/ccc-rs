//! Getting levels from and outputs to the contest platform. Two modes, per person
//! (`cargo ccc mode online|manual`, stored in the git-ignored ccc.local.toml):
//!
//! - **online** (default): talks to codingcontest.org directly (`login`, `fetch`, `progress`, `submit`);
//!   the login is read from Firefox.
//! - **manual**: the fallback when the login doesn't work. The human downloads each level's zip in
//!   the browser (`fetch` waits for it in Downloads), and `submit` copies the outputs into Downloads
//!   for upload.
//!
//! The harness has already re-validated the example and checked the outputs before `submit`.

mod codingcontest;
mod firefox;
mod manual;

use std::path::PathBuf;

use crate::config::{self, Config};
use crate::layout::Workspace;

const SWITCH_TO_MANUAL: &str = "cargo ccc mode manual";

pub fn submit(ws: &Workspace, cfg: &Config, level: u32, outputs: &[PathBuf], yes: bool) -> Result<bool, String> {
    match cfg.mode() {
        "manual" => manual::submit(ws, cfg, level, outputs),
        _ => codingcontest::submit(cfg, level, outputs, yes).map_err(|e| {
            format!(
                "{e}\n  If the game API keeps failing: check in the browser which uploads went through, then {SWITCH_TO_MANUAL}"
            )
        }),
    }
}

/// Get level `level`'s zip (online: download it; manual: wait for it in Downloads) and ingest it.
pub fn fetch(ws: &Workspace, cfg: &Config, level: u32) -> Result<bool, String> {
    match cfg.mode() {
        "manual" => manual::fetch(ws, cfg, level),
        _ => codingcontest::fetch(ws, cfg, level).map_err(|e| {
            format!("{e}\n  If the game API keeps failing (`cargo ccc login` says GAME API FAILED): {SWITCH_TO_MANUAL}")
        }),
    }
}

/// Check the account login and the game API, with one unmistakable state (see codingcontest::login).
pub fn login(cfg: &Config) -> Result<bool, String> {
    match cfg.mode() {
        "manual" => {
            println!("MANUAL MODE: no login is used. Download each level's zip into Downloads; outputs are copied");
            println!("there for upload. To go back to online mode: cargo ccc mode online");
            Ok(true)
        }
        _ => codingcontest::login(cfg, SWITCH_TO_MANUAL),
    }
}

/// Show the team's progress as the platform sees it.
pub fn progress(cfg: &Config) -> Result<bool, String> {
    match cfg.mode() {
        "manual" => {
            println!("Manual mode: progress is only visible in the browser. `cargo ccc status` shows the next level.");
            Ok(true)
        }
        _ => codingcontest::progress(cfg),
    }
}

/// `cargo ccc mode [online|manual]`: show or switch the mode (for this person, in ccc.local.toml).
pub fn mode(ws: &Workspace, cfg: &Config, wanted: Option<&str>) -> Result<bool, String> {
    let Some(wanted) = wanted else {
        println!("Mode: {}", cfg.mode());
        return Ok(true);
    };
    let platform = match wanted {
        "manual" => "manual",
        "online" | "codingcontest" => "codingcontest",
        other => return Err(format!("unknown mode {other:?}: use `online` or `manual`")),
    };
    config::set_local(&ws.root, "platform", platform)?;
    match platform {
        "manual" => {
            let downloads = cfg.downloads_dir()?;
            println!("MANUAL MODE is on (for you, in ccc.local.toml). From now on:");
            println!("  - download each level's zip in the browser into {}", downloads.display());
            println!("    (`fetch N` and /level wait for it and continue by themselves)");
            println!("  - after each level, the outputs to upload are copied into {}", downloads.display());
            println!("  - upload them in the game; if one is WRONG, tell Claude the message");
            println!("Back to online mode: cargo ccc mode online");
        }
        _ => println!("ONLINE MODE is on. Check it with: cargo ccc login"),
    }
    Ok(true)
}

/// Manual mode only: the level after the last one handed over for upload (1 if none).
pub fn next_manual_level(ws: &Workspace) -> u32 {
    ws.ingested_levels().into_iter().filter(|&n| manual::handed_over(ws, n).exists()).max().map_or(1, |n| n + 1)
}
