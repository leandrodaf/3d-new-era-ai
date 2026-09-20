//! What a long piece of work is doing, for whoever is waiting on it.
//!
//! Opening a project, saving a bundle or exporting a model takes seconds on a
//! real home, and an interface that says nothing while it happens looks stuck
//! — the system itself starts calling it unresponsive. So the work says what
//! it is doing with [`step`] as it goes, and whoever started it listens with
//! [`watched`].
//!
//! The listener is per thread, not passed from call to call: reporting where
//! the time actually goes costs one thread-local read, so the calls can sit in
//! the middle of the work without dragging a parameter through every signature
//! between. When nobody is listening they do nothing at all.
//!
//! Work that can be given up on checks [`cancelled`] wherever stopping is safe
//! — between files, between entries — and returns `Err` saying so, rather than
//! leaving whoever waits with no way out.

use std::cell::RefCell;
use std::sync::Arc;

/// Someone waiting on a piece of work.
pub trait Watcher: Send + Sync {
    /// What the work is doing now, and how far along it is. `of` is 0 while
    /// the size of the job is not known yet.
    fn step(&self, what: &str, done: u64, of: u64);

    /// Whether whoever is waiting has given up.
    fn cancelled(&self) -> bool {
        false
    }
}

thread_local! {
    static LISTENING: RefCell<Option<Arc<dyn Watcher>>> = const { RefCell::new(None) };
}

/// Puts back whoever was listening before, panic or not.
struct Restore(Option<Arc<dyn Watcher>>);

impl Drop for Restore {
    fn drop(&mut self) {
        let previous = self.0.take();
        LISTENING.with(|slot| *slot.borrow_mut() = previous);
    }
}

/// Runs `work` with `watcher` listening to what it reports.
///
/// Only this thread is watched: work handed to other threads reports to
/// nobody, which is what makes the listener cheap.
pub fn watched<T>(watcher: &Arc<dyn Watcher>, work: impl FnOnce() -> T) -> T {
    let _restore = Restore(LISTENING.with(|slot| slot.borrow_mut().replace(Arc::clone(watcher))));
    work()
}

/// Copies the listener when a bounded worker needs to report to the same UI.
pub fn listener() -> Option<Arc<dyn Watcher>> {
    LISTENING.with(|slot| slot.borrow().clone())
}

/// Says what this thread is working on: `what` it is doing, how many parts are
/// `done` and how many there are `of`, or 0 when that is not known.
pub fn step(what: &str, done: u64, of: u64) {
    LISTENING.with(|slot| {
        if let Some(watcher) = slot.borrow().as_ref() {
            watcher.step(what, done, of);
        }
    });
}

/// Whether whoever is waiting on this thread has given up. Loops that can stop
/// between one item and the next should ask, and stop.
#[must_use]
pub fn cancelled() -> bool {
    LISTENING.with(|slot| slot.borrow().as_ref().is_some_and(|w| w.cancelled()))
}

/// What work says when it was told to stop.
pub const CANCELLED: &str = "cancelado";

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[derive(Default)]
    struct Notes {
        steps: Mutex<Vec<(String, u64, u64)>>,
        give_up: AtomicBool,
    }

    impl Watcher for Notes {
        fn step(&self, what: &str, done: u64, of: u64) {
            self.steps
                .lock()
                .expect("notes")
                .push((what.to_owned(), done, of));
        }
        fn cancelled(&self) -> bool {
            self.give_up.load(Ordering::Relaxed)
        }
    }

    #[test]
    fn work_reports_only_while_someone_listens() {
        step("nobody hears this", 0, 0);
        let notes = Arc::new(Notes::default());
        let watcher: Arc<dyn Watcher> = notes.clone();
        watched(&watcher, || {
            step("lendo", 1, 3);
            step("lendo", 2, 3);
        });
        step("nor this", 0, 0);
        assert_eq!(
            *notes.steps.lock().expect("notes"),
            vec![("lendo".to_owned(), 1, 3), ("lendo".to_owned(), 2, 3)]
        );
    }

    #[test]
    fn giving_up_reaches_the_work() {
        let notes = Arc::new(Notes::default());
        let watcher: Arc<dyn Watcher> = notes.clone();
        assert!(!cancelled());
        watched(&watcher, || {
            assert!(!cancelled());
            notes.give_up.store(true, Ordering::Relaxed);
            assert!(cancelled());
        });
        assert!(!cancelled(), "nobody is listening any more");
    }

    #[test]
    fn a_panic_does_not_leave_the_listener_behind() {
        let notes = Arc::new(Notes::default());
        let watcher: Arc<dyn Watcher> = notes.clone();
        let fell_over = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            watched(&watcher, || panic!("boom"));
        }));
        assert!(fell_over.is_err());
        assert!(!cancelled());
        step("nobody hears this", 0, 0);
        assert!(notes.steps.lock().expect("notes").is_empty());
    }
}
