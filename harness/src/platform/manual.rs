//! Manual mode: the human downloads and uploads in the browser, via the Downloads folder.
//!
//! - `fetch N` waits for level N's zip to land in Downloads, then ingests it.
//! - `submit N` copies the outputs into Downloads (the upload dialog opens there) and records that
//!   level N was handed over, so `status` knows the next level. The verdict is only in the browser.

use std::path::PathBuf;
use std::time::Duration;

use crate::config::Config;
use crate::layout::{Workspace, file_name};
use crate::{downloads, ingest};

/// Default wait for a download: just under the 10-minute limit on a single Claude Code command.
const FETCH_WAIT_SECS: u64 = 570;

pub fn fetch(ws: &Workspace, cfg: &Config, level: u32) -> Result<bool, String> {
    let wait = cfg.get("fetch_wait").and_then(|v| v.parse().ok()).unwrap_or(FETCH_WAIT_SECS);
    let zip = downloads::wait_for_level(cfg, level, Duration::from_secs(wait))?;
    println!("Using {}", zip.display());
    let bytes = std::fs::read(&zip).map_err(|e| format!("could not read {}: {e}", zip.display()))?;
    let mut files = Vec::new();
    ingest::add(file_name(&zip), bytes, &mut files)?;
    ingest::store(ws, cfg, &files, Some(level))
}

pub fn submit(ws: &Workspace, cfg: &Config, level: u32, outputs: &[PathBuf]) -> Result<bool, String> {
    let copied = downloads::copy_into(cfg, outputs)?;
    std::fs::write(handed_over(ws, level), "").map_err(|e| e.to_string())?;
    println!("MANUAL SUBMIT: the {} output(s) for level {level} are in your Downloads folder:", copied.len());
    for path in &copied {
        println!("  {}", path.display());
    }
    println!(
        "Upload each one in the game (the upload dialog opens in Downloads). If one is WRONG, tell Claude the \
         message. When all are correct, download level {}'s zip: `fetch {}` picks it up by itself.",
        level + 1,
        level + 1
    );
    Ok(true)
}

/// Marker: level N's outputs were handed to the human for upload.
pub fn handed_over(ws: &Workspace, level: u32) -> PathBuf {
    ws.level_dir(level).join(".handed-over")
}
