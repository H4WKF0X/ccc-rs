//! Contest settings from `ccc.toml` (committed), overridden by `ccc.local.toml` (git-ignored).
//!
//! The format is a flat subset of TOML (`key = "value"` lines and `#` comments), which keeps the
//! harness free of a TOML dependency. Section headers are ignored.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Config {
    values: BTreeMap<String, String>,
}

impl Config {
    pub fn load(root: &Path) -> Result<Config, String> {
        let mut values = BTreeMap::new();
        for name in ["ccc.toml", "ccc.local.toml"] {
            let path = root.join(name);
            if let Ok(text) = fs::read_to_string(&path) {
                let parsed = parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
                values.extend(parsed);
            }
        }
        Ok(Config { values })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str).filter(|v| !v.is_empty())
    }
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
