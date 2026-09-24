//! Counting how often something is asked, to say no after too many.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Requests per key within a window, in memory: a restart forgets, which is
/// fine for keeping a mailbox from being flooded.
#[derive(Debug, Clone, Default)]
pub struct Limiter(Arc<Mutex<HashMap<String, Vec<Instant>>>>);

impl Limiter {
    /// Counts one more for `key`, or says no when `most` were already asked
    /// within `window`.
    ///
    /// # Panics
    ///
    /// If another thread panicked while holding the counts.
    pub fn allow(&self, key: &str, most: usize, window: Duration) -> bool {
        let now = Instant::now();
        let mut held = self.0.lock().expect("limits");
        // Forget what no window can see any more, now and then.
        if held.len() > 10_000 {
            held.retain(|_, times| times.iter().any(|t| now.duration_since(*t) < window));
        }
        let times = held.entry(key.to_owned()).or_default();
        times.retain(|t| now.duration_since(*t) < window);
        if times.len() >= most {
            return false;
        }
        times.push(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limit_is_per_key() {
        let limits = Limiter::default();
        let hour = Duration::from_secs(3600);
        assert!(limits.allow("a", 2, hour));
        assert!(limits.allow("a", 2, hour));
        assert!(!limits.allow("a", 2, hour));
        assert!(limits.allow("b", 2, hour));
    }
}
