//! Hashing and checking the passphrase a member signs in with.
//!
//! New hashes use **Argon2id**, a deliberately slow hash, stored as a PHC
//! string (`$argon2id$v=19$…`) that carries its own salt and settings. Handles
//! created before November 2026 have a hex SHA-256 of `salt|passphrase`;
//! those still check, and [`Check::Upgrade`] tells the caller to replace them
//! with an Argon2id hash after a successful sign-in.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use sha2::{Digest, Sha256};

use crate::keys::ct_eq;

/// Shortest passphrase accepted at sign-up, in characters.
pub const MIN_LEN: usize = 8;

/// The result of checking a passphrase.
#[derive(Debug, PartialEq, Eq)]
pub enum Check {
    /// Correct, and the stored hash is current.
    Ok,
    /// Correct, but stored in the old format: store [`hash`] of it now.
    Upgrade,
    /// Wrong passphrase (or an unreadable stored hash).
    Wrong,
}

/// Hash a passphrase for storage. Returns `(salt, hash)` for the record's
/// `secret_salt` and `secret_hash`; with Argon2id the salt lives inside the
/// hash, so `salt` is empty.
pub fn hash(passphrase: &str) -> (String, String) {
    let salt = SaltString::generate(&mut OsRng);
    let phc = Argon2::default()
        .hash_password(passphrase.as_bytes(), &salt)
        .expect("argon2 with default settings cannot fail")
        .to_string();
    (String::new(), phc)
}

/// Check `passphrase` against a stored `(salt, hash)`.
pub fn check(stored_salt: &str, stored_hash: &str, passphrase: &str) -> Check {
    if stored_hash.starts_with("$argon2") {
        return match PasswordHash::new(stored_hash) {
            Ok(parsed) if Argon2::default().verify_password(passphrase.as_bytes(), &parsed).is_ok() => Check::Ok,
            _ => Check::Wrong,
        };
    }
    if ct_eq(&legacy_sha256(stored_salt, passphrase), stored_hash) {
        Check::Upgrade
    } else {
        Check::Wrong
    }
}

/// The pre-November-2026 format: hex SHA-256 of `salt|passphrase`.
fn legacy_sha256(salt: &str, passphrase: &str) -> String {
    let mut h = Sha256::new();
    h.update(salt.as_bytes());
    h.update(b"|");
    h.update(passphrase.as_bytes());
    hex::encode(h.finalize())
}

/// True when a new passphrase is long enough.
pub fn long_enough(passphrase: &str) -> bool {
    passphrase.chars().count() >= MIN_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2_round_trip() {
        let (salt, h) = hash("correct horse");
        assert!(h.starts_with("$argon2id$"));
        assert_eq!(check(&salt, &h, "correct horse"), Check::Ok);
        assert_eq!(check(&salt, &h, "wrong horse"), Check::Wrong);
    }

    #[test]
    fn legacy_hash_still_signs_in_and_asks_for_upgrade() {
        let salt = "00ff";
        let old = legacy_sha256(salt, "supersecret1");
        assert_eq!(check(salt, &old, "supersecret1"), Check::Upgrade);
        assert_eq!(check(salt, &old, "nope"), Check::Wrong);
    }

    #[test]
    fn garbage_stored_hash_is_wrong_not_a_panic() {
        assert_eq!(check("", "$argon2id$garbage", "x"), Check::Wrong);
    }

    #[test]
    fn length_rule() {
        assert!(!long_enough("short"));
        assert!(long_enough("eight888"));
    }
}
