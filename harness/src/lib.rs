//! The CCC harness: everything except the solutions.
//!
//! A contest repo has two crates. `solution` holds one `levels/level<N>.rs` per level and is the
//! only place solving happens. `harness` (this crate) owns the I/O contract, ingesting downloaded
//! levels, validating against the worked example, producing outputs and the platform client.
//!
//! The contract a level implements is one function:
//!
//! ```ignore
//! pub fn solve(input: &str) -> Vec<String>
//! ```
//!
//! `input` is the whole input file with line endings normalised to `\n` and no trailing newline.
//! Return the output lines without line endings; the harness writes them with the line ending
//! style of the example `.out` (see [`io::OutputStyle`]).

pub mod config;
pub mod debug;
pub mod ingest;
pub mod io;
pub mod layout;
pub mod parse;
pub mod platform;
pub mod validate;

mod cli;

/// The signature every level's `solve` has.
pub type SolveFn = fn(&str) -> Vec<String>;

/// One registered level. The `solution` crate's build script generates the list of these from
/// the files in `solution/src/levels/`, so adding a level means adding one file.
#[derive(Clone, Copy)]
pub struct Level {
    pub n: u32,
    pub solve: SolveFn,
}

/// Entry point for the `solution` binary: `fn main() -> ExitCode { harness::main(LEVELS) }`.
pub fn main(levels: &[Level]) -> std::process::ExitCode {
    cli::main(levels)
}
