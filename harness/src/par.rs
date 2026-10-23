//! Solve independent cases on all cores, with progress on stderr for long runs.
//!
//! ```ignore
//! let cases: Vec<Case> = parse(input);
//! let answers: Vec<String> = harness::par::map(&cases, solve_case);   // same order as `cases`
//! ```
//!
//! Results come back in input order, so output stays deterministic. Threads default to the
//! number of cores; set `CCC_THREADS` to override. **Mind memory**: N cases in flight need N times
//! the per-case peak. For memory-hungry searches use `map_threads(k, ...)` with a small `k`.

use std::any::Any;
use std::cell::RefCell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

thread_local! {
    static LABEL: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Name shown in progress lines from this thread (the harness sets it to the input file name).
pub fn set_label(label: &str) {
    LABEL.with(|l| *l.borrow_mut() = label.to_owned());
}

fn label() -> String {
    LABEL.with(|l| l.borrow().clone())
}

pub fn threads() -> usize {
    std::env::var("CCC_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
}

/// `items.iter().map(f).collect()`, on [`threads`] threads.
pub fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    map_threads(threads(), items, f)
}

/// [`map`] with an explicit thread count (e.g. to bound memory).
pub fn map_threads<T: Sync, R: Send>(threads: usize, items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let n = items.len();
    let next = AtomicUsize::new(0);
    let progress = Progress::new(n);
    let slots: Vec<Mutex<Option<R>>> = (0..n).map(|_| Mutex::new(None)).collect();
    let panic: Mutex<Option<Box<dyn Any + Send>>> = Mutex::new(None);
    let deadline = crate::budget::deadline();
    std::thread::scope(|s| {
        for _ in 0..threads.clamp(1, n.max(1)) {
            s.spawn(|| {
                crate::budget::inherit(deadline);
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&items[i]))) {
                        Ok(r) => *slots[i].lock().unwrap() = Some(r),
                        Err(payload) => {
                            panic.lock().unwrap().get_or_insert(payload);
                            next.store(n, Ordering::Relaxed); // stop handing out work
                            break;
                        }
                    }
                    progress.tick();
                }
            });
        }
    });
    if let Some(payload) = panic.into_inner().unwrap() {
        std::panic::resume_unwind(payload); // keep the solver's own panic message
    }
    progress.finish();
    slots.into_iter().map(|s| s.into_inner().unwrap().expect("every item was processed")).collect()
}

/// Progress for a loop over `total` cases. Prints to stderr at most every 2 s, and only once a
/// run has taken over 2 s, so fast runs stay quiet. Usable directly in sequential solvers too.
pub struct Progress {
    total: usize,
    done: AtomicUsize,
    start: Instant,
    last: Mutex<Instant>,
    label: String,
}

impl Progress {
    const EVERY: Duration = Duration::from_secs(2);

    pub fn new(total: usize) -> Progress {
        let now = Instant::now();
        Progress { total, done: AtomicUsize::new(0), start: now, last: Mutex::new(now), label: label() }
    }

    pub fn tick(&self) {
        let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;
        if self.start.elapsed() < Self::EVERY {
            return;
        }
        if let Ok(mut last) = self.last.try_lock()
            && last.elapsed() >= Self::EVERY
        {
            *last = Instant::now();
            self.print(done);
        }
    }

    fn finish(&self) {
        if self.start.elapsed() >= Self::EVERY {
            self.print(self.done.load(Ordering::Relaxed));
        }
    }

    fn print(&self, done: usize) {
        let label = if self.label.is_empty() { String::new() } else { format!("[{}] ", self.label) };
        eprintln!("  {label}{done}/{} cases, {:.0} s", self.total, self.start.elapsed().as_secs_f64());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order() {
        let items: Vec<u64> = (0..1000).collect();
        assert_eq!(map_threads(8, &items, |x| x * x), items.iter().map(|x| x * x).collect::<Vec<_>>());
        assert!(map(&[] as &[u8], |x| *x).is_empty());
    }

    #[test]
    fn propagates_the_solvers_panic_message() {
        let err = std::panic::catch_unwind(|| map_threads(4, &[1, 2, 3], |&x| if x == 2 { panic!("case 2 broke") } else { x }))
            .unwrap_err();
        assert_eq!(err.downcast_ref::<&str>(), Some(&"case 2 broke"));
    }
}
