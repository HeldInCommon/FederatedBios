//! A simple rate limit: at most `max` events per `window` seconds per key.
//!
//! Used on a commons' `/inbox`, keyed by the sender's network address, as a
//! cheap first layer against junk traffic (review item M7). It doesn't stop
//! a determined sender with many addresses; vouching (decision record 0011)
//! is the real answer to mass sign-ups.
//!
//! Behind a reverse proxy every request may appear to come from the proxy,
//! so the default limit is generous enough for a full workshop room sharing
//! at once.

use std::collections::HashMap;
use std::sync::Mutex;

/// Counts events per key in fixed time windows.
pub struct RateLimiter {
    max: u32,
    window: u64,
    /// key -> (events in this window, when the window started)
    map: Mutex<HashMap<String, (u32, u64)>>,
}

impl RateLimiter {
    /// Allow `max` events per `window` seconds per key.
    pub fn new(max: u32, window: u64) -> RateLimiter {
        RateLimiter { max, window, map: Mutex::new(HashMap::new()) }
    }

    /// Count an event for `key` at time `now`, if it is under the limit.
    /// Returns false (and counts nothing) when the limit is reached. This
    /// changes the limiter's counts, as the name says.
    pub fn record_if_under_limit(&self, key: &str, now: u64) -> bool {
        let mut map = self.map.lock().unwrap();
        // Forget windows that have ended, so the map can't grow without bound.
        if map.len() > 10_000 {
            let window = self.window;
            map.retain(|_, (_, start)| now < *start + window);
        }
        let entry = map.entry(key.to_string()).or_insert((0, now));
        if now >= entry.1 + self.window {
            *entry = (0, now);
        }
        if entry.0 >= self.max {
            return false;
        }
        entry.0 += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_the_limit_then_refuses_until_the_window_ends() {
        let l = RateLimiter::new(3, 60);
        assert!((0..3).all(|_| l.record_if_under_limit("a", 100)));
        assert!(!l.record_if_under_limit("a", 120));
        assert!(l.record_if_under_limit("b", 120), "other keys are counted separately");
        assert!(l.record_if_under_limit("a", 160), "a new window starts");
    }
}
