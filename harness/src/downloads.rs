//! The browser's Downloads folder, for manual mode (and `ingest` without arguments).
//!
//! In manual mode the human downloads each level's zip in the browser and uploads the outputs
//! there; the harness picks the zip up from Downloads and copies the outputs back into Downloads.
//! Zips older than the contest repo (`created` in ccc.toml) are ignored: they belong to an earlier
//! contest or training run (`level-1.zip` exists for every game).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::config::Config;
use crate::ingest::{detect_level, level_in_name};
use crate::layout::file_name;

/// Level zips in Downloads, newest first: `*.zip` mentioning "level" or "ccc", fully downloaded
/// (no `.part` file next to it) and not older than the contest repo.
pub fn level_zips(cfg: &Config) -> Result<Vec<PathBuf>, String> {
    let dir = cfg.downloads_dir()?;
    let created = cfg.created();
    let mut zips: Vec<(SystemTime, PathBuf)> = fs::read_dir(&dir)
        .map_err(|e| format!("could not read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            let is_level_zip = name.ends_with(".zip") && (name.contains("level") || name.contains("ccc"));
            let in_progress = dir.join(format!("{}.part", e.file_name().to_string_lossy())).exists();
            let modified = e.metadata().ok()?.modified().ok()?;
            let fresh = created.is_none_or(|c| modified >= c);
            (is_level_zip && !in_progress && fresh).then(|| (modified, e.path()))
        })
        .collect();
    zips.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    Ok(zips.into_iter().map(|(_, p)| p).collect())
}

/// The newest complete zip for `level`: by its name (`level-3.zip`, `level-3(1).zip`), or, if the
/// name has no level number, by the files inside it.
pub fn find_level(cfg: &Config, level: u32) -> Result<Option<PathBuf>, String> {
    Ok(level_zips(cfg)?.into_iter().find(|zip| match level_in_name(&file_name(zip)) {
        Some(n) => n == level,
        None => zip_level(zip) == Some(level),
    }))
}

fn zip_level(zip: &Path) -> Option<u32> {
    let archive = zip::ZipArchive::new(fs::File::open(zip).ok()?).ok()?;
    let names: Vec<String> = archive.file_names().filter_map(|n| n.ok().map(|n| n.into_owned())).collect();
    detect_level(names.iter().map(String::as_str)).ok()
}

/// Wait until level `level`'s zip is in Downloads and complete (readable, size stable), up to
/// `timeout`. Prints where it is waiting once, then a reminder every minute.
pub fn wait_for_level(cfg: &Config, level: u32, timeout: Duration) -> Result<PathBuf, String> {
    let start = Instant::now();
    let mut announced = false;
    let mut last_reminder = Instant::now();
    loop {
        if let Some(zip) = find_level(cfg, level)?
            && is_complete(&zip)
        {
            return Ok(zip);
        }
        if !announced {
            println!(
                "Waiting for level {level}'s zip in {} (download it from the game page; this continues by itself)...",
                cfg.downloads_dir()?.display()
            );
            announced = true;
        } else if last_reminder.elapsed() >= Duration::from_secs(60) {
            println!("  still waiting for level {level}'s zip ({:.0} s)", start.elapsed().as_secs_f64());
            last_reminder = Instant::now();
        }
        if start.elapsed() >= timeout {
            return Err(format!(
                "no zip for level {level} appeared in {} within {} s. Download it, then run `cargo ccc fetch {level}` again",
                cfg.downloads_dir()?.display(),
                timeout.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// A zip that opens and whose size doesn't change across a short pause (the browser is done).
fn is_complete(zip: &Path) -> bool {
    let size = || fs::metadata(zip).map(|m| m.len()).ok();
    let before = size();
    std::thread::sleep(Duration::from_millis(300));
    before.is_some() && before == size() && fs::File::open(zip).ok().and_then(|f| zip::ZipArchive::new(f).ok()).is_some()
}

/// Copy files into Downloads (overwriting), so the browser's upload dialog finds them at once.
pub fn copy_into(cfg: &Config, files: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let dir = cfg.downloads_dir()?;
    files
        .iter()
        .map(|f| {
            let dest = dir.join(file_name(f));
            fs::copy(f, &dest).map_err(|e| format!("could not copy {} to {}: {e}", file_name(f), dir.display()))?;
            Ok(dest)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn downloads(tag: &str) -> (PathBuf, Config) {
        let dir = std::env::temp_dir().join(format!("ccc-test-dl-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cfg = Config::from_pairs(&[("downloads_dir", &dir.display().to_string())]);
        (dir, cfg)
    }

    fn zip_with(path: &Path, names: &[&str]) {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for name in names {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"1\n").unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn finds_the_right_level_and_skips_partial_downloads() {
        let (dir, cfg) = downloads("find");
        zip_with(&dir.join("level-1.zip"), &["in_level-1_0-example.txt"]);
        zip_with(&dir.join("level-2.zip"), &["in_level-2_0-example.txt"]);
        fs::write(dir.join("level-2.zip.part"), b"").unwrap(); // still downloading
        zip_with(&dir.join("ccc.zip"), &["in_level-3_0-example.txt"]); // level only inside
        assert_eq!(find_level(&cfg, 1).unwrap(), Some(dir.join("level-1.zip")));
        assert_eq!(find_level(&cfg, 2).unwrap(), None);
        assert_eq!(find_level(&cfg, 3).unwrap(), Some(dir.join("ccc.zip")));
        fs::remove_file(dir.join("level-2.zip.part")).unwrap();
        assert_eq!(wait_for_level(&cfg, 2, Duration::from_secs(5)).unwrap(), dir.join("level-2.zip"));
        assert!(wait_for_level(&cfg, 4, Duration::from_millis(100)).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ignores_zips_older_than_the_repo() {
        let (dir, _) = downloads("stale");
        zip_with(&dir.join("level-1.zip"), &["in_level-1_0-example.txt"]);
        let future = SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + 3600;
        let cfg = Config::from_pairs(&[("downloads_dir", &dir.display().to_string()), ("created", &future.to_string())]);
        assert_eq!(find_level(&cfg, 1).unwrap(), None);
        fs::remove_dir_all(dir).unwrap();
    }
}
