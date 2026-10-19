//! Manual mode: nothing goes over the network. Lists the files to upload in the game's browser UI.

use std::path::{Path, PathBuf};

use crate::layout::{Workspace, file_name};

pub fn submit(ws: &Workspace, level: u32, outputs: &[PathBuf]) -> Result<bool, String> {
    println!("Manual submit: upload these {} file(s) for level {level} in the game:", outputs.len());
    for out in outputs {
        println!("  {:<28} {}", file_name(out), absolute(out).display());
    }
    println!("Output folder:  {}", absolute(&ws.out_dir(level)).display());
    println!("If the game also wants source code: {}", absolute(&ws.level_source(level)).display());
    Ok(true)
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}
