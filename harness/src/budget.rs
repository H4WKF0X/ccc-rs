//! Time budgets for searches: `cargo ccc all N --budget 5` gives each input file 5 seconds.
//!
//! A search loop asks [`expired`] and stops with its best answer so far:
//!
//! ```ignore
//! while !harness::budget::expired() { improve(&mut best); }
//! ```
//!
//! Without `--budget` there is no deadline and `expired()` is always false, so solvers must also
//! stop on their own. The deadline is per input file and reaches `harness::par::map` workers too.
//! Short budgets make experiments cheap (seconds, not minutes). On optimization levels, `all` keeps
//! the best answer per case anyway, so a short run never loses anything.

use std::cell::Cell;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static BUDGET: OnceLock<Duration> = OnceLock::new();

thread_local! {
    static DEADLINE: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Set once by the CLI (`--budget S`).
pub fn set(budget: Duration) {
    let _ = BUDGET.set(budget);
}

/// The budget per input file, if any.
pub fn per_file() -> Option<Duration> {
    BUDGET.get().copied()
}

/// Start the clock for one input file on this thread (the harness does this before `solve`).
pub fn start() {
    DEADLINE.with(|d| d.set(per_file().map(|b| Instant::now() + b)));
}

/// Start a short clock for an example check: a tenth of the budget, at most 1 s. The example only
/// has to be valid, so it shouldn't eat the time meant for the real inputs.
pub fn start_example() {
    DEADLINE.with(|d| d.set(per_file().map(|b| Instant::now() + (b / 10).min(Duration::from_secs(1)))));
}

/// This thread's deadline (carried into `par::map` workers).
pub fn deadline() -> Option<Instant> {
    DEADLINE.with(Cell::get)
}

pub(crate) fn inherit(deadline: Option<Instant>) {
    DEADLINE.with(|d| d.set(deadline));
}

/// True once the current file's budget is used up (never without `--budget`).
pub fn expired() -> bool {
    deadline().is_some_and(|d| Instant::now() >= d)
}

/// Time left for the current file (`None` without `--budget`).
pub fn left() -> Option<Duration> {
    deadline().map(|d| d.saturating_duration_since(Instant::now()))
}
