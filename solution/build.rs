//! Registers every `src/levels/level<N>.rs` automatically, so adding a level means adding one file.
//! Generates `$OUT_DIR/levels.rs` with one `mod level<N>;` per file plus the `LEVELS` table. A level
//! file that defines `pub fn check(` (at the start of a line) also registers its checker.

use std::fmt::Write;
use std::path::Path;
use std::{env, fs};

fn main() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("src").join("levels");
    println!("cargo::rerun-if-changed={}", dir.display());

    let mut levels: Vec<(u32, String, bool)> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let n = path.file_name()?.to_str()?.strip_prefix("level")?.strip_suffix(".rs")?.parse().ok()?;
            let source = fs::read_to_string(&path).unwrap_or_default();
            let has_check = source.lines().any(|l| l.starts_with("pub fn check("));
            Some((n, path.display().to_string(), has_check))
        })
        .collect();
    levels.sort();

    let mut code = String::new();
    for (n, path, _) in &levels {
        writeln!(code, "#[path = {path:?}]\npub mod level{n};").unwrap();
    }
    code.push_str("\npub const LEVELS: &[harness::Level] = &[\n");
    for (n, _, has_check) in &levels {
        let check = if *has_check { format!("Some(level{n}::check)") } else { "None".into() };
        writeln!(code, "    harness::Level {{ n: {n}, solve: level{n}::solve, check: {check} }},").unwrap();
    }
    code.push_str("];\n");
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("levels.rs"), code).unwrap();
}
