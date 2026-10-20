//! `cargo ccc ingest [paths...] [--level N]`: unpack a downloaded level into `work/level<N>/`.
//!
//! Accepts zip files (nested zips too), directories and loose files, so it keeps working whatever
//! shape the new platform's downloads take. With no paths it takes the newest level-looking zip
//! from the Downloads folder. Inputs and expected outputs (`in_*.txt`/`out_*.txt`, or `*.in`/`*.out`)
//! go to `in/`, everything else (the statement PDF, images, ...) to `statement/`.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::layout::{Workspace, has_extension, is_data_name};

pub(crate) struct File {
    pub name: String,
    pub bytes: Vec<u8>,
}

pub fn run(ws: &Workspace, cfg: &Config, args: &[String]) -> Result<bool, String> {
    let mut paths = Vec::new();
    let mut level_override = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--level" {
            let n = args.next().ok_or("--level needs a number")?;
            level_override = Some(n.parse::<u32>().map_err(|_| format!("bad level {n:?}"))?);
        } else {
            paths.push(PathBuf::from(arg));
        }
    }
    if paths.is_empty() {
        let zip = newest_download(cfg)?;
        println!("Using {}", zip.display());
        paths.push(zip);
    }

    let mut files = Vec::new();
    for path in &paths {
        collect(path, &mut files)?;
    }
    store(ws, cfg, &files, level_override)
}

/// Write unpacked files into `work/level<N>/` and print what arrived.
pub(crate) fn store(ws: &Workspace, cfg: &Config, files: &[File], level_override: Option<u32>) -> Result<bool, String> {
    if files.is_empty() {
        return Err("nothing to ingest: no files found".into());
    }
    let level = match level_override {
        Some(n) => n,
        None => detect_level(files.iter().map(|f| f.name.as_str()))?,
    };

    let (in_dir, statement_dir) = (ws.in_dir(level), ws.statement_dir(level));
    for dir in [&in_dir, &statement_dir, &ws.out_dir(level)] {
        fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    for file in files {
        let dest = if is_data_name(&file.name) { &in_dir } else { &statement_dir }.join(&file.name);
        fs::write(&dest, &file.bytes).map_err(|e| format!("could not write {}: {e}", dest.display()))?;
    }

    crate::guard::run(ws, cfg, level);
    let examples = ws.examples(level)?;
    let real = ws.real_inputs(level)?;
    let names = |ps: Vec<PathBuf>| ps.iter().map(|p| crate::layout::file_name(p)).collect::<Vec<_>>().join(", ");
    println!("Ingested level {level} -> {}", ws.level_dir(level).display());
    println!("  statement: {}", names(list(&statement_dir)));
    println!("  example:   {}", names(examples.into_iter().map(|e| e.input).collect()));
    println!("  real:      {}", names(real));
    if !ws.level_source(level).exists() {
        let from = if level > 1 && ws.level_source(level - 1).exists() {
            format!(" --from {}", level - 1)
        } else {
            String::new()
        };
        println!("\nNo solver yet. In Claude Code: /solve {level}   (or scaffold: cargo ccc new-level {level}{from})");
    } else {
        println!("\nNext: /solve {level} in Claude Code, then `cargo ccc all {level}`");
    }
    Ok(true)
}

fn list(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().filter_map(|e| Some(e.ok()?.path())).collect();
    v.sort();
    v
}

/// The newest complete level zip in Downloads that isn't older than this contest repo.
fn newest_download(cfg: &Config) -> Result<PathBuf, String> {
    crate::downloads::level_zips(cfg)?.into_iter().next().ok_or_else(|| {
        format!("no (new) level zip in {}; pass one: cargo ccc ingest <zip>", cfg.downloads_dir().unwrap_or_default().display())
    })
}

fn collect(path: &Path, out: &mut Vec<File>) -> Result<(), String> {
    if path.is_dir() {
        for child in list(path) {
            collect(&child, out)?;
        }
        return Ok(());
    }
    let bytes = fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    add(crate::layout::file_name(path), bytes, out)
}

/// Add one file, unpacking it if it is a zip. Only the base name is kept, so archive paths can't
/// escape the target directory.
pub(crate) fn add(name: String, bytes: Vec<u8>, out: &mut Vec<File>) -> Result<(), String> {
    if name.starts_with('.') {
        return Ok(());
    }
    if has_extension(Path::new(&name), "zip") {
        return read_zip(Cursor::new(bytes), &name, out);
    }
    if let Some(dup) = out.iter_mut().find(|f| f.name == name) {
        eprintln!("warning: {name} appears twice in the download; keeping the last one");
        dup.bytes = bytes;
    } else {
        out.push(File { name, bytes });
    }
    Ok(())
}

fn read_zip<R: Read + Seek>(reader: R, zip_name: &str, out: &mut Vec<File>) -> Result<(), String> {
    let bad = |e: zip::result::ZipError| format!("{zip_name} is not a readable zip: {e}");
    let mut archive = zip::ZipArchive::new(reader).map_err(bad)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(bad)?;
        let full_name = entry.name().map_err(bad)?.into_owned();
        if entry.is_dir() || full_name.contains("__MACOSX") {
            continue;
        }
        let Some(name) = Path::new(&full_name).file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(|e| format!("{zip_name}: could not unpack {name}: {e}"))?;
        add(name, bytes, out)?;
    }
    Ok(())
}

/// The level named in the file names (`in_level-3_1-small.txt`, `level3_1_small.in`, `Level 3.pdf`).
/// Data files decide when they name a level; it's an error if they disagree.
pub fn detect_level<'a>(names: impl Iterator<Item = &'a str> + Clone) -> Result<u32, String> {
    let is_data = |n: &&str| is_data_name(n);
    let found = |names: Vec<&str>| names.into_iter().filter_map(level_in_name).collect::<BTreeSet<u32>>();
    let mut levels = found(names.clone().filter(is_data).collect());
    if levels.is_empty() {
        levels = found(names.collect());
    }
    match levels.len() {
        1 => Ok(*levels.first().unwrap()),
        0 => Err("could not tell the level from the file names; pass --level N".into()),
        _ => Err(format!("file names mention several levels {levels:?}; pass --level N")),
    }
}

/// `level` (any case) followed by optional spaces, `_` or `-`, then digits.
pub fn level_in_name(name: &str) -> Option<u32> {
    let lower = name.to_lowercase();
    lower.match_indices("level").find_map(|(i, _)| {
        let rest = lower[i + "level".len()..].trim_start_matches([' ', '_', '-']);
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_level_in_names() {
        assert_eq!(level_in_name("level3_1_small.in"), Some(3));
        assert_eq!(level_in_name("Level 12.pdf"), Some(12));
        assert_eq!(level_in_name("ccc-level_4.zip"), Some(4));
        assert_eq!(level_in_name("levels.txt"), None);
        assert_eq!(level_in_name("small.in"), None);
    }

    #[test]
    fn data_files_decide_the_level() {
        let names = ["level2_0_example.in", "level2_1_small.in", "overview level1.pdf"];
        assert_eq!(detect_level(names.iter().copied()), Ok(2));
        assert_eq!(detect_level(["Level 5.pdf", "a.in"].iter().copied()), Ok(5));
        let new_style = ["in_level-4_0-example.txt", "out_level-4_0-example.txt", "level-4.pdf"];
        assert_eq!(detect_level(new_style.iter().copied()), Ok(4));
        assert!(detect_level(["a.in", "b.pdf"].iter().copied()).is_err());
        assert!(detect_level(["level1_a.in", "level2_a.in"].iter().copied()).is_err());
    }

    #[test]
    fn unpacks_nested_zips_and_flattens_paths() {
        let inner = zip_of(&[("deep/level7_0_example.in", b"1\n")]);
        let outer = zip_of(&[("x/inner.zip", &inner), ("Level 7.pdf", b"%PDF"), ("__MACOSX/._junk", b"")]);
        let mut files = Vec::new();
        add("level7.zip".into(), outer, &mut files).unwrap();
        let mut names: Vec<_> = files.iter().map(|f| f.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Level 7.pdf", "level7_0_example.in"]);
    }

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(bytes).unwrap();
        }
        w.finish().unwrap().into_inner()
    }
}
