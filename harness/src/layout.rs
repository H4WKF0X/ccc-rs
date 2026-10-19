//! Where things live in a contest repo.
//!
//! ```text
//! <root>/ccc.toml                         marks the repo root
//! <root>/solution/src/levels/level<N>.rs  the solver for level N
//! <root>/work/level<N>/in/                *.in inputs + the example's expected *.out
//! <root>/work/level<N>/out/               outputs we produce
//! <root>/work/level<N>/statement/         everything else from the download (PDF, images, ...)
//! ```

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

    /// All `*.in` files for a level, sorted by name.
    pub fn inputs(&self, n: u32) -> Result<Vec<PathBuf>, String> {
        let dir = self.in_dir(n);
        let entries = fs::read_dir(&dir).map_err(|_| {
            format!("no ingested inputs for level {n} (expected {}); run `cargo ccc ingest` first", dir.display())
        })?;
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| has_extension(p, "in"))
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
                let expected = Some(input.with_extension("out")).filter(|p| p.is_file());
                Example { input, expected }
            })
            .collect())
    }

    /// The inputs whose answers are unknown, i.e. everything except the examples.
    pub fn real_inputs(&self, n: u32) -> Result<Vec<PathBuf>, String> {
        Ok(self.inputs(n)?.into_iter().filter(|p| !is_example(p)).collect())
    }

    /// Where the output for `input` goes: `work/level<N>/out/<stem>.out`.
    pub fn output_for(&self, n: u32, input: &Path) -> PathBuf {
        let stem = input.file_stem().unwrap_or_default();
        self.out_dir(n).join(stem).with_extension("out")
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

pub fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))
}
