//! Tracing for debugging without a debugger. Off by default; set `CCC_DEBUG=1` to turn it on.
//!
//! ```ignore
//! harness::trace!("record {i}: best={best}");
//! ```
//!
//! Output goes to stderr only. Solvers return lines; they never print to stdout.

use std::sync::OnceLock;

/// True when the `CCC_DEBUG` env var is set to something other than empty or `0`.
pub fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("CCC_DEBUG").is_ok_and(|v| !v.is_empty() && v != "0"))
}

/// `eprintln!` that only runs when [`on`] is true. The arguments are not evaluated otherwise.
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        if $crate::debug::on() {
            eprintln!($($arg)*);
        }
    };
}
