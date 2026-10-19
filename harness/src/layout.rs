//! Where things live in a contest repo.
//!
//! ```text
//! <root>/ccc.toml                         marks the repo root
//! <root>/solution/src/levels/level<N>.rs  the solver for level N
//! <root>/work/level<N>/in/                inputs + the example's expected output
//! <root>/work/level<N>/out/               outputs we produce
//! <root>/work/level<N>/statement/         everything else from the download (PDF, images, ...)
//! ```
//!
//! Two file naming schemes are understood:
//! - codingcontest.org (2026+): `in_level-1_1-small.txt`, expected/produced `out_level-1_1-small.txt`
//! - CatCoder (until 2025):     `level1_1_small.in`, expected/produced `level1_1_small.out`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Workspace {
    pub root: PathBuf,
}

/// An example input and its expected output, if the download had one.
pub struct Example {
    pub input: PathBuf,
    pub expected: Option<PathBuf>,
}

impl Workspace {
    /// The nearest directory at or above the current one that contains `ccc.toml`.
    pub fn find() -> Result<Workspace, String> {
        let cwd = env::current_dir().map_err(|e| format!("no current directory: {e}"))?;
        cwd.ancestors()
            .find(|dir| dir.join("ccc.toml").is_file())
            .map(|dir| Workspace { root: dir.to_path_buf() })
            .ok_or_else(|| "not inside a contest repo (no ccc.toml here or in any parent directory)".into())
    }

    pub fn level_dir(&self, n: u32) -> PathBuf {
        self.root.join("work").join(format!("level{n}"))
    }

    pub fn in_dir(&self, n: u32) -> PathBuf {
        self.level_dir(n).join("in")
    }

    pub fn out_dir(&self, n: u32) -> PathBuf {
        self.level_dir(n).join("out")
    }

    pub fn statement_dir(&self, n: u32) -> PathBuf {
        self.level_dir(n).join("statement")
    }

    pub fn level_source(&self, n: u32) -> PathBuf {
        self.root.join("solution").join("src").join("levels").join(format!("level{n}.rs"))
    }

    pub fn level_template(&self) -> PathBuf {
        self.root.join("templates").join("level.rs")
    }

    /// All input files for a level, sorted by name.
    pub fn inputs(&self, n: u32) -> Result<Vec<PathBuf>, String> {
        let dir = self.in_dir(n);
        let entries = fs::read_dir(&dir).map_err(|_| {
            format!("no ingested inputs for level {n} (expected {}); run `cargo ccc ingest` first", dir.display())
        })?;
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| is_input_name(&file_name(p)))
            .collect();
        files.sort();
        Ok(files)
    }

    pub fn examples(&self, n: u32) -> Result<Vec<Example>, String> {
        Ok(self
            .inputs(n)?
            .into_iter()
            .filter(|p| is_example(p))
            .map(|input| {
                let expected = Some(input.with_file_name(output_name(&file_name(&input)))).filter(|p| p.is_file());
                Example { input, expected }
            })
            .collect())
    }

    /// The inputs whose answers are unknown, i.e. everything except the examples.
    pub fn real_inputs(&self, n: u32) -> Result<Vec<PathBuf>, String> {
        Ok(self.inputs(n)?.into_iter().filter(|p| !is_example(p)).collect())
    }

    /// Where the output for `input` goes: `work/level<N>/out/<output_name>`.
    pub fn output_for(&self, n: u32, input: &Path) -> PathBuf {
        self.out_dir(n).join(output_name(&file_name(input)))
    }

    /// Levels that have a `work/level<N>/` directory, ascending.
    pub fn ingested_levels(&self) -> Vec<u32> {
        let mut levels: Vec<u32> = fs::read_dir(self.root.join("work"))
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok()?.file_name().to_str()?.strip_prefix("level")?.parse().ok())
            .collect();
        levels.sort_unstable();
        levels
    }
}

pub fn is_example(path: &Path) -> bool {
    file_name(path).to_lowercase().contains("example")
}

pub fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `in_*.txt` (codingcontest.org) or `*.in` (CatCoder).
pub fn is_input_name(name: &str) -> bool {
    let path = Path::new(name);
    has_extension(path, "in") || (has_extension(path, "txt") && starts_with_ci(name, "in_"))
}

/// Inputs and expected/produced outputs, in either naming scheme.
pub fn is_data_name(name: &str) -> bool {
    let path = Path::new(name);
    is_input_name(name) || has_extension(path, "out") || (has_extension(path, "txt") && starts_with_ci(name, "out_"))
}

/// The output file name for an input: `in_X.txt` -> `out_X.txt`, `X.in` -> `X.out`.
pub fn output_name(input_name: &str) -> String {
    if starts_with_ci(input_name, "in_") {
        format!("out_{}", &input_name[3..])
    } else {
        let stem = Path::new(input_name).file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
        format!("{stem}.out")
    }
}

/// The platform's id for an input/output file: `in_level-1_1-small.txt` -> `1-small`.
pub fn file_id(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let rest = lower.strip_prefix("in_").or_else(|| lower.strip_prefix("out_"))?;
    let stem = rest.strip_suffix(".txt")?;
    let (_level, id) = stem.split_once('_')?;
    Some(id.to_owned()).filter(|id| !id.is_empty())
}

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.get(..prefix.len()).is_some_and(|p| p.eq_ignore_ascii_case(prefix))
}

pub fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn understands_both_naming_schemes() {
        assert!(is_input_name("in_level-1_0-example.txt") && is_input_name("level1_1_small.in"));
        assert!(!is_input_name("out_level-1_0-example.txt") && !is_input_name("notes.txt"));
        assert!(is_data_name("out_level-1_0-example.txt") && is_data_name("level1_0_example.out"));
        assert!(!is_data_name("level-1.pdf"));
        assert_eq!(output_name("in_level-1_2-large.txt"), "out_level-1_2-large.txt");
        assert_eq!(output_name("level1_2_large.in"), "level1_2_large.out");
    }

    #[test]
    fn file_ids() {
        assert_eq!(file_id("in_level-1_1-small.txt").as_deref(), Some("1-small"));
        assert_eq!(file_id("out_level-12_2-large.txt").as_deref(), Some("2-large"));
        assert_eq!(file_id("level1_1_small.out"), None);
    }
}
