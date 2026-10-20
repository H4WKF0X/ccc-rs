//! Settings, later files overriding earlier ones (empty values never override):
//!
//! 1. `~/.config/ccc/config.toml` (Windows: `%APPDATA%\ccc\config.toml`): per machine, for every
//!    contest repo, e.g. credentials.
//! 2. `ccc.toml`: the contest's settings (committed).
//! 3. `ccc.local.toml`: personal overrides for this repo (git-ignored).
//!
//! The format is a flat subset of TOML (`key = "value"` lines and `#` comments), which keeps the
//! harness free of a TOML dependency. Section headers are ignored.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct Config {
    values: BTreeMap<String, String>,
}

impl Config {
    pub fn load(root: &Path) -> Result<Config, String> {
        let mut values = BTreeMap::new();
        let files = global_path().into_iter().chain([root.join("ccc.toml"), root.join("ccc.local.toml")]);
        for path in files {
            if let Ok(text) = fs::read_to_string(&path) {
                let parsed = parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
                values.extend(parsed.into_iter().filter(|(_, v)| !v.is_empty()));
            }
        }
        Ok(Config { values })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str).filter(|v| !v.is_empty())
    }

    #[cfg(test)]
    pub(crate) fn from_pairs(pairs: &[(&str, &str)]) -> Config {
        Config { values: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    }

    /// "online" (codingcontest.org) or "manual" (browser downloads/uploads via ~/Downloads).
    pub fn mode(&self) -> &str {
        match self.get("platform").unwrap_or("codingcontest") {
            "manual" => "manual",
            _ => "online",
        }
    }

    /// Where the browser saves downloads: `downloads_dir`, else `~/Downloads`.
    pub fn downloads_dir(&self) -> Result<PathBuf, String> {
        match self.get("downloads_dir") {
            Some(d) => Ok(PathBuf::from(d)),
            None => std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join("Downloads"))
                .ok_or_else(|| "no HOME directory; set downloads_dir in ccc.local.toml".into()),
        }
    }

    /// When the contest repo was created (`created`, Unix seconds, set by new-contest). Downloads
    /// older than that belong to an earlier contest.
    pub fn created(&self) -> Option<std::time::SystemTime> {
        let secs: u64 = self.get("created")?.parse().ok()?;
        Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
    }
}

/// Set `key = "value"` in `<root>/ccc.local.toml` (personal, git-ignored), replacing an existing line.
pub fn set_local(root: &Path, key: &str, value: &str) -> Result<(), String> {
    let path = root.join("ccc.local.toml");
    let text = fs::read_to_string(&path).unwrap_or_default();
    let line = format!("{key} = \"{value}\"");
    let mut replaced = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| {
            let is_key = l.split_once('=').is_some_and(|(k, _)| k.trim() == key) && !l.trim_start().starts_with('#');
            if is_key && !replaced {
                replaced = true;
                line.clone()
            } else {
                l.to_owned()
            }
        })
        .collect();
    if !replaced {
        lines.push(line);
    }
    fs::write(&path, lines.join("\n") + "\n").map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// The per-machine config file (it may not exist).
pub fn global_path() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = if cfg!(windows) {
        env("APPDATA")?
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?
    };
    Some(dir.join("ccc").join("config.toml"))
}

fn parse(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {}: expected `key = value`, got {line:?}", i + 1))?;
        let value = value.trim();
        let value = match value.strip_prefix('"') {
            Some(rest) => rest
                .split_once('"')
                .ok_or_else(|| format!("line {}: unterminated string", i + 1))?
                .0,
            None => value.split('#').next().unwrap_or("").trim(),
        };
        out.insert(key.trim().to_owned(), value.to_owned());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_toml() {
        let m = parse("# c\n[x]\nname = \"spaceship # 1\"  # note\nline_ending = auto # c\nempty = \"\"\n").unwrap();
        assert_eq!(m["name"], "spaceship # 1");
        assert_eq!(m["line_ending"], "auto");
        assert_eq!(m["empty"], "");
    }

    #[test]
    fn set_local_replaces_or_appends() {
        let dir = std::env::temp_dir().join(format!("ccc-test-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("ccc.local.toml"), "# platform = \"x\"\nplatform = \"codingcontest\"\ncookie = \"c\"\n").unwrap();
        set_local(&dir, "platform", "manual").unwrap();
        set_local(&dir, "downloads_dir", "/tmp/d").unwrap();
        let text = fs::read_to_string(dir.join("ccc.local.toml")).unwrap();
        assert_eq!(text, "# platform = \"x\"\nplatform = \"manual\"\ncookie = \"c\"\ndownloads_dir = \"/tmp/d\"\n");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("nonsense").is_err());
        assert!(parse("a = \"open").is_err());
    }
}
