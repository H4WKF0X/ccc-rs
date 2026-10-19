//! Registers every `src/levels/level<N>.rs` automatically, so adding a level means adding one file.
//! Generates `$OUT_DIR/levels.rs` with one `mod level<N>;` per file plus the `LEVELS` table.

use std::fmt::Write;
use std::path::Path;
use std::{env, fs};

fn main() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("src").join("levels");
    println!("cargo::rerun-if-changed={}", dir.display());

    let mut levels: Vec<(u32, String)> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let n = path.file_name()?.to_str()?.strip_prefix("level")?.strip_suffix(".rs")?.parse().ok()?;
            Some((n, path.display().to_string()))
        })
        .collect();
    levels.sort();

    let mut code = String::new();
    for (n, path) in &levels {
        writeln!(code, "#[path = {path:?}]\npub mod level{n};").unwrap();
    }
    code.push_str("\npub const LEVELS: &[harness::Level] = &[\n");
    for (n, _) in &levels {
        writeln!(code, "    harness::Level {{ n: {n}, solve: level{n}::solve }},").unwrap();
    }
    code.push_str("];\n");
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("levels.rs"), code).unwrap();
}
