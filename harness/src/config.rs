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
    fn rejects_garbage() {
        assert!(parse("nonsense").is_err());
        assert!(parse("a = \"open").is_err());
    }
}
