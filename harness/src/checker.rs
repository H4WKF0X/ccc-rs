//! Output checkers, for levels that accept many valid answers (so the example's answer is just one
//! of them and a byte-for-byte match is the wrong test). A level is judged by every usable checker:
//!
//! 1. **The level's own `check`**: `pub fn check(input: &str, output: &[String]) -> Result<(), String>`
//!    in `level<N>.rs`, the statement's validity rules written independently of `solve`. Usually the
//!    main one: most games' sandboxes only visualise.
//! 2. **An external command** from `ccc.toml`, for running the game's own sandbox code. The template
//!    default runs `sandbox-level-N.html`'s script.
//!
//! Any rejection rejects. An output is accepted if at least one checker accepts it and none reject.
//!
//! The external command is configured in `ccc.toml`:
//!
//! ```toml
//! checker = "node checker/sandbox-check.js {sandbox} {input} {output}"
//! ```
//!
//! The command runs from the repo root with the placeholders replaced by absolute paths. Exit code 0
//! means accepted, 1 means rejected (its output says why), and anything else (or failing to start)
//! means the checker could not judge. That last case only warns: a broken checker never blocks.
//! `{sandbox}` is the level's `statement/sandbox*.html` (the game's own checker). If the command uses it
//! and a level has none, that level is not checked. Arguments are split on whitespace (no quoting).
//!
//! Before the checker is trusted for a level, it must accept that level's official example answer
//! (`self_test`). A checker that rejects the organisers' own answer is wrong for this game
//! (e.g. it splits cases differently), so it is set aside with a warning instead of blocking.
//!
//! Prefer the game's own checker over one written alongside the solver: a checker that shares the
//! solver's misunderstanding of the rules proves nothing.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::config::Config;
use crate::layout::{Workspace, file_name};
use crate::{CheckFn, io};

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Accepted,
    Rejected(String),
    /// The checker could not judge (crashed, missing, doesn't fit this game).
    Broken(String),
}

pub struct Checker {
    argv: Vec<String>,
    sandbox: Option<PathBuf>,
}

impl Checker {
    /// The checker for level `n`, or `None` if none is configured or it doesn't apply to this level.
    pub fn for_level(ws: &Workspace, cfg: &Config, n: u32) -> Option<Checker> {
        let argv: Vec<String> = cfg.get("checker")?.split_whitespace().map(str::to_owned).collect();
        let sandbox = sandbox_file(ws, n);
        if argv.iter().any(|a| a.contains("{sandbox}")) && sandbox.is_none() {
            return None;
        }
        Some(Checker { argv, sandbox })
    }

    /// Run the checker on one input/output pair.
    pub fn check(&self, root: &Path, input: &Path, output: &Path) -> Verdict {
        let abs = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()).display().to_string();
        let sandbox = self.sandbox.as_deref().map(abs).unwrap_or_default();
        let args: Vec<String> = self
            .argv
            .iter()
            .map(|a| a.replace("{sandbox}", &sandbox).replace("{input}", &abs(input)).replace("{output}", &abs(output)))
            .collect();
        let result = match Command::new(&args[0]).args(&args[1..]).current_dir(root).output() {
            Ok(result) => result,
            Err(e) => return Verdict::Broken(format!("`{}` could not start: {e}", args[0])),
        };
        let text = format!("{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let tail = lines[lines.len().saturating_sub(5)..].join(" | ");
        let message = if tail.is_empty() { "no message".to_owned() } else { tail };
        match result.status.code() {
            Some(0) => Verdict::Accepted,
            Some(1) => Verdict::Rejected(message),
            _ => Verdict::Broken(format!("{}: {message}", result.status)),
        }
    }

    /// The checker script's name (e.g. `sandbox-check.js`), not the wrapper running it.
    pub fn describe(&self) -> String {
        let script = self
            .argv
            .iter()
            .find(|a| [".js", ".py", ".rb", ".pl"].iter().any(|ext| a.ends_with(ext)))
            .map(|a| file_name(Path::new(a)))
            .unwrap_or_else(|| self.argv[..2.min(self.argv.len())].join(" "));
        match &self.sandbox {
            Some(s) => format!("{script} with {}", file_name(s)),
            None => script,
        }
    }
}

/// `work/level<N>/statement/sandbox*.html`, if the level shipped one.
pub fn sandbox_file(ws: &Workspace, n: u32) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(ws.statement_dir(n))
        .ok()?
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| {
            let name = file_name(p).to_lowercase();
            name.starts_with("sandbox") && name.ends_with(".html")
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

/// One checker for a level: the level's `check` function, or the external command.
enum Judge {
    Level(u32, CheckFn),
    Command(Checker),
}

impl Judge {
    fn name(&self) -> String {
        match self {
            Judge::Level(n, _) => format!("level{n}::check"),
            Judge::Command(c) => c.describe(),
        }
    }

    fn judge(&self, root: &Path, input: &Path, output: &Path) -> Verdict {
        match self {
            Judge::Command(c) => c.check(root, input, output),
            Judge::Level(_, check) => {
                let (Ok(input), Ok(output)) = (io::read_input(input), io::read_input(output)) else {
                    return Verdict::Broken("could not read the input or output".into());
                };
                let lines = io::lines(&output);
                match catch_unwind(AssertUnwindSafe(|| check(&input, &lines))) {
                    Ok(Ok(())) => Verdict::Accepted,
                    Ok(Err(why)) => Verdict::Rejected(why),
                    Err(payload) => {
                        let msg = payload
                            .downcast_ref::<&str>()
                            .map(|s| s.to_string())
                            .or_else(|| payload.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "unknown panic".into());
                        Verdict::Broken(format!("check panicked: {msg}"))
                    }
                }
            }
        }
    }

    /// Does it accept the official answer of every example? `Err` says why not.
    fn self_test(&self, ws: &Workspace, n: u32) -> Result<(), String> {
        for example in ws.examples(n).unwrap_or_default() {
            let Some(expected) = example.expected else { continue };
            match self.judge(&ws.root, &example.input, &expected) {
                Verdict::Accepted => {}
                Verdict::Rejected(why) | Verdict::Broken(why) => {
                    return Err(format!("it does not accept the official answer {}: {why}", file_name(&expected)));
                }
            }
        }
        Ok(())
    }
}

/// The usable checkers for one level (see the module docs).
pub struct Judges {
    judges: Vec<Judge>,
}

impl Judges {
    /// Every checker for level `n` that passes its self-test. Failures are returned as warnings.
    pub fn for_level(ws: &Workspace, cfg: &Config, n: u32, check: Option<CheckFn>) -> (Judges, Vec<String>) {
        let candidates = check
            .map(|f| Judge::Level(n, f))
            .into_iter()
            .chain(Checker::for_level(ws, cfg, n).map(Judge::Command));
        let mut judges = Vec::new();
        let mut warnings = Vec::new();
        for judge in candidates {
            match judge.self_test(ws, n) {
                Ok(()) => judges.push(judge),
                Err(why) => warnings.push(match judge {
                    Judge::Level(..) => format!("ignoring level{n}::check, {why}. Fix check() (or the rules it encodes)."),
                    Judge::Command(_) => format!("ignoring {}, {why}. It doesn't fit this game.", judge.name()),
                }),
            }
        }
        (Judges { judges }, warnings)
    }

    pub fn is_empty(&self) -> bool {
        self.judges.is_empty()
    }

    pub fn describe(&self) -> String {
        self.judges.iter().map(Judge::name).collect::<Vec<_>>().join(" + ")
    }

    /// Combined verdict: any rejection rejects; else any acceptance accepts; else can't judge.
    pub fn judge(&self, root: &Path, input: &Path, output: &Path) -> Verdict {
        let verdicts: Vec<(String, Verdict)> = self.judges.iter().map(|j| (j.name(), j.judge(root, input, output))).collect();
        if let Some((name, Verdict::Rejected(why))) = verdicts.iter().find(|(_, v)| matches!(v, Verdict::Rejected(_))) {
            return Verdict::Rejected(format!("{name}: {why}"));
        }
        if verdicts.iter().any(|(_, v)| *v == Verdict::Accepted) {
            return Verdict::Accepted;
        }
        let why: Vec<String> = verdicts
            .into_iter()
            .filter_map(|(name, v)| match v {
                Verdict::Broken(why) => Some(format!("{name}: {why}")),
                _ => None,
            })
            .collect();
        Verdict::Broken(why.join("; "))
    }
}
