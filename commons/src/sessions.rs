//! Signed-in sessions, their cookies, and a limit on sign-in attempts.
//!
//! A session is a random token in an `HttpOnly` cookie, mapped in memory to a
//! handle and a CSRF token. Sessions expire after [`SESSION_TTL`] seconds and
//! are forgotten when the server restarts (people sign in again).
//!
//! The cookie helpers are pure functions; [`Sessions`] and [`AttemptLimiter`]
//! hold the in-memory state behind a mutex.

use std::collections::HashMap;
use std::sync::Mutex;

use axum::http::{header, HeaderMap};

/// How long a session lasts, in seconds (one day).
pub const SESSION_TTL: u64 = 86_400;

/// One signed-in session.
#[derive(Clone, Debug)]
pub struct Session {
    pub handle: String,
    /// Must accompany every form that changes something (publish, share).
    pub csrf: String,
    /// When the session stops being valid (Unix seconds).
    pub expires_at: u64,
}

/// All live sessions, keyed by cookie token.
#[derive(Default)]
pub struct Sessions {
    map: Mutex<HashMap<String, Session>>,
}

impl Sessions {
    /// Start a session for `handle` at time `now`. Returns the cookie token.
    /// Expired sessions are cleared out at the same time.
    pub fn start(&self, handle: &str, now: u64) -> String {
        let token = random_hex(32);
        let mut map = self.map.lock().unwrap();
        map.retain(|_, s| s.expires_at > now);
        map.insert(
            token.clone(),
            Session { handle: handle.to_string(), csrf: random_hex(16), expires_at: now + SESSION_TTL },
        );
        token
    }

    /// The live session for `token` at time `now`, if any.
    pub fn get(&self, token: &str, now: u64) -> Option<Session> {
        self.map.lock().unwrap().get(token).filter(|s| s.expires_at > now).cloned()
    }

    /// End the session for `token`.
    pub fn end(&self, token: &str) {
        self.map.lock().unwrap().remove(token);
    }
}

/// `n` random bytes as hex.
pub fn random_hex(n: usize) -> String {
    use rand::RngCore;
    let mut b = vec![0u8; n];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

/// The session token in the cookie called `name`
/// ([`crate::config::Config::session_cookie`]), if the request has one.
pub fn token_from(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .map(str::trim)
        .find_map(|p| p.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v.to_string()))
}

/// The `Set-Cookie` value that stores `token`. `HttpOnly` and `SameSite=Lax`
/// always; `Secure` only when served over https (see
/// [`crate::config::Config::secure_cookie`]).
pub fn set_cookie(name: &str, token: &str, secure: bool) -> String {
    let sec = if secure { "; Secure" } else { "" };
    format!("{name}={token}; Path=/; HttpOnly; SameSite=Lax{sec}; Max-Age={SESSION_TTL}")
}

/// The `Set-Cookie` value that clears the session cookie.
pub fn clear_cookie(name: &str, secure: bool) -> String {
    let sec = if secure { "; Secure" } else { "" };
    format!("{name}=; Path=/; HttpOnly; SameSite=Lax{sec}; Max-Age=0")
}

/// Limits wrong passphrases per handle: after [`AttemptLimiter::MAX_FAILURES`]
/// failures, further tries wait until [`AttemptLimiter::WINDOW`] seconds have
/// passed since the first failure. This makes guessing a passphrase online
/// impractical without locking anyone out for long.
#[derive(Default)]
pub struct AttemptLimiter {
    /// handle -> (failures, time of first failure in this window)
    map: Mutex<HashMap<String, (u32, u64)>>,
}

impl AttemptLimiter {
    pub const MAX_FAILURES: u32 = 5;
    pub const WINDOW: u64 = 15 * 60;

    /// True when `key` may try again at time `now`.
    pub fn allowed(&self, key: &str, now: u64) -> bool {
        match self.map.lock().unwrap().get(key) {
            Some((n, since)) => *n < Self::MAX_FAILURES || now >= since + Self::WINDOW,
            None => true,
        }
    }

    /// Record a failed attempt for `key` at time `now`.
    pub fn failed(&self, key: &str, now: u64) {
        let mut map = self.map.lock().unwrap();
        let entry = map.entry(key.to_string()).or_insert((0, now));
        if now >= entry.1 + Self::WINDOW {
            *entry = (0, now);
        }
        entry.0 += 1;
    }

    /// Forget `key`'s failures after a successful sign-in.
    pub fn succeeded(&self, key: &str) {
        self.map.lock().unwrap().remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn sessions_expire() {
        let s = Sessions::default();
        let t = s.start("tega", 100);
        assert_eq!(s.get(&t, 101).unwrap().handle, "tega");
        assert!(s.get(&t, 100 + SESSION_TTL).is_none());
    }

    #[test]
    fn ended_session_is_gone() {
        let s = Sessions::default();
        let t = s.start("tega", 0);
        s.end(&t);
        assert!(s.get(&t, 1).is_none());
    }

    #[test]
    fn token_is_read_from_cookie_header() {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_static("a=1; hic_session=abc; hic_session_8081=def"));
        assert_eq!(token_from(&h, "hic_session").as_deref(), Some("abc"));
        assert_eq!(token_from(&h, "hic_session_8081").as_deref(), Some("def"));
        assert_eq!(token_from(&h, "hic_session_8082"), None);
    }

    #[test]
    fn cookie_flags() {
        assert!(set_cookie("hic_session", "t", true).contains("; Secure"));
        assert!(!set_cookie("hic_session", "t", false).contains("Secure"));
        assert!(set_cookie("hic_session_8081", "t", false).starts_with("hic_session_8081=t;"));
        assert!(clear_cookie("hic_session", false).contains("Max-Age=0"));
    }

    #[test]
    fn limiter_blocks_after_max_failures_then_resets() {
        let l = AttemptLimiter::default();
        for _ in 0..AttemptLimiter::MAX_FAILURES {
            assert!(l.allowed("tega", 10));
            l.failed("tega", 10);
        }
        assert!(!l.allowed("tega", 11));
        assert!(l.allowed("tega", 10 + AttemptLimiter::WINDOW));
        l.succeeded("tega");
        assert!(l.allowed("tega", 11));
    }
}
