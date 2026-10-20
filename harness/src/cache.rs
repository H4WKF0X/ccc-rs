//! Output records: which build turned which input into an output, and what the checker said.
//!
//! Solving can take minutes, so an output is only recomputed when it could have changed. A record
//! (`work/level<N>/out/.records/<output>.txt`) stores hashes of the binary that produced the output,
//! the input and the output itself. The output is *current* only if all three still match. The
//! binary hash covers every line of solution and harness code, so any code change, even in a
//! shared module, makes old outputs stale. That's deliberately strict.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// FNV-1a 64: tiny, deterministic and fast enough for a few MB.
pub fn hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub fn hash_file(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|b| hash(&b))
}

/// Hash of the running executable; `None` if it can't be read (then nothing is ever reused).
pub fn build_id() -> Option<&'static str> {
    static ID: OnceLock<Option<String>> = OnceLock::new();
    ID.get_or_init(|| std::env::current_exe().ok().and_then(|exe| hash_file(&exe))).as_deref()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub build: String,
    pub input: String,
    pub output: String,
    /// "" (not checked), "ok", "rejected: <why>" or "broken: <why>".
    pub checker: String,
}

impl Record {
    /// A record for `output` as just produced from `input` by this build (not yet checked).
    pub fn new(input: &Path, output: &Path) -> Option<Record> {
        Some(Record {
            build: build_id()?.to_owned(),
            input: hash_file(input)?,
            output: hash_file(output)?,
            checker: String::new(),
        })
    }

    pub fn checker_ok(&self) -> bool {
        self.checker == "ok"
    }
}

fn record_path(output: &Path) -> PathBuf {
    let name = output.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    output.with_file_name(".records").join(format!("{name}.txt"))
}

pub fn load(output: &Path) -> Option<Record> {
    let text = fs::read_to_string(record_path(output)).ok()?;
    let field = |key: &str| {
        text.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=')).map(str::to_owned)
    };
    Some(Record { build: field("build")?, input: field("input")?, output: field("output")?, checker: field("checker")? })
}

pub fn save(output: &Path, record: &Record) -> Result<(), String> {
    let path = record_path(output);
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| format!("could not create {}: {e}", path.display()))?;
    let checker = record.checker.replace('\n', " ");
    let text = format!("build={}\ninput={}\noutput={}\nchecker={checker}\n", record.build, record.input, record.output);
    fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// The record for `output` if it is current: made by this exact build from this exact input, and
/// the output file is unchanged since.
pub fn current(input: &Path, output: &Path) -> Option<Record> {
    let record = load(output)?;
    let fresh = Some(record.build.as_str()) == build_id()
        && Some(&record.input) == hash_file(input).as_ref()
        && Some(&record.output) == hash_file(output).as_ref();
    fresh.then_some(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable() {
        assert_eq!(hash(b""), "cbf29ce484222325");
        assert_eq!(hash(b"a"), "af63dc4c8601ec8c");
    }

    #[test]
    fn records_round_trip_and_detect_changes() {
        let dir = std::env::temp_dir().join(format!("ccc-test-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (input, output) = (dir.join("in_level-1_1-small.txt"), dir.join("out_level-1_1-small.txt"));
        fs::write(&input, "1\n5\n").unwrap();
        fs::write(&output, "5\n").unwrap();
        let mut record = Record::new(&input, &output).unwrap();
        record.checker = "rejected: line 1\nmore".into();
        save(&output, &record).unwrap();
        assert_eq!(load(&output).unwrap().checker, "rejected: line 1 more");
        assert!(current(&input, &output).is_some());
        fs::write(&output, "6\n").unwrap();
        assert!(current(&input, &output).is_none(), "edited output is not current");
        fs::remove_dir_all(dir).unwrap();
    }
}
