//! The CCC harness: everything except the solutions.
//!
//! A contest repo has two crates. `solution` holds one `levels/level<N>.rs` per level and is the
//! only place solving happens. `harness` (this crate) owns the I/O contract, ingesting downloaded
//! levels, validating against the worked example, producing outputs and the platform client.
//!
//! The contract a level implements is one function, plus an optional checker for levels that
//! accept many valid answers:
//!
//! ```ignore
//! pub fn solve(input: &str) -> Vec<String>
//! pub fn check(input: &str, output: &[String]) -> Result<(), String>   // optional
//! pub fn score(input: &str, output: &[String]) -> Result<Vec<i64>, String> // optional: optimization levels
//! ```
//!
//! `input` is the whole input file with line endings normalised to `\n` and no trailing newline.
//! Return the output lines without line endings; the harness writes them with the line ending
//! style of the example `.out` (see [`io::OutputStyle`]).

pub mod best;
pub mod budget;
pub mod cache;
pub mod checker;
pub mod config;
pub mod debug;
pub mod downloads;
pub mod guard;
pub mod ingest;
pub mod io;
pub mod layout;
pub mod par;
pub mod parse;
pub mod platform;
pub mod validate;

mod cli;

/// The signature every level's `solve` has.
pub type SolveFn = fn(&str) -> Vec<String>;

/// A level's own output checker: `Err` says why `output` is not a valid answer for `input`.
pub type CheckFn = fn(&str, &[String]) -> Result<(), String>;

/// An optimization level's scorer: the cost of each case (lower is better; negate a score where
/// higher is better), or `Err` if `output` is invalid. See [`best`].
pub type ScoreFn = fn(&str, &[String]) -> Result<Vec<i64>, String>;

/// One registered level. The `solution` crate's build script generates the list of these from
/// the files in `solution/src/levels/`, so adding a level means adding one file.
#[derive(Clone, Copy)]
pub struct Level {
    pub n: u32,
    pub solve: SolveFn,
    /// Set when the level file defines `pub fn check`.
    pub check: Option<CheckFn>,
    /// Set when the level file defines `pub fn score` (an optimization level).
    pub score: Option<ScoreFn>,
}

/// Entry point for the `solution` binary: `fn main() -> ExitCode { harness::main(LEVELS) }`.
pub fn main(levels: &[Level]) -> std::process::ExitCode {
    quiet_on_closed_pipe();
    cli::main(levels)
}

/// Exit quietly when stdout is closed early (`cargo ccc ... | head`), like standard command-line
/// tools, instead of panicking with "failed printing to stdout: Broken pipe".
fn quiet_on_closed_pipe() {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn signal(signum: i32, handler: usize) -> usize;
        }
        const SIGPIPE: i32 = 13;
        const SIG_DFL: usize = 0;
        // SAFETY: restoring the default disposition of SIGPIPE at startup, before any threads run.
        unsafe {
            signal(SIGPIPE, SIG_DFL);
        }
    }
}
