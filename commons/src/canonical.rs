//! The content address (`cid`) of a version.
//!
//! A `cid` is the SHA-256, in lowercase hex, of a fixed JSON layout of the
//! version: handle, reach, licence, signals, creation time and body, in that
//! order. The same content always gives the same `cid`, and any change gives a
//! different one, so a signature over the `cid` covers the content.
//!
//! **The exact bytes are a contract.** Every signature ever made depends on
//! them, so the golden test at the bottom of this file must never change. See
//! `docs/REVIEW-2026-11.md` (M1) for the plan to move to a written
//! canonicalisation standard (RFC 8785) under a new schema version.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::model::Profile;

/// The fields that go into the hash. Field order is part of the format.
#[derive(Serialize)]
struct Canonical<'a> {
    handle: &'a str,
    visibility: &'a str,
    license: &'a str,
    signals: &'a [String],
    created_at: u64,
    body: &'a serde_json::Value,
}

/// The body hashed for a public or commons version: the profile without its
/// avatar.
///
/// The avatar is display metadata, not signed content. Leaving it out keeps
/// the `cid` stable when a photo changes, and lets a node share to a commons
/// that predates the avatar field. (Members-only versions hash their
/// ciphertext instead; see [`crate::keys::seal_for_members`].)
pub fn hash_body(profile: &Profile) -> serde_json::Value {
    let mut v = serde_json::to_value(profile).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.remove("avatar");
    }
    v
}

/// The `cid` for a version with these fields.
pub fn content_id(
    handle: &str,
    visibility: &str,
    license: &str,
    signals: &[String],
    created_at: u64,
    body: &serde_json::Value,
) -> String {
    let c = Canonical { handle, visibility, license, signals, created_at, body };
    // Serialising a struct of strings, numbers and a JSON value cannot fail.
    let bytes = serde_json::to_vec(&c).expect("canonical form serialises");
    hex::encode(Sha256::digest(&bytes))
}

/// True when `s` looks like a `cid`: 64 lowercase hex characters. Used before
/// a `cid` becomes a file name.
pub fn is_cid(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// The first 12 characters of a `cid` or key, for display.
pub fn short(id: &str) -> String {
    id.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Place;

    fn sample() -> Profile {
        Profile {
            name: "Tega Brain".into(),
            pronouns: "she/her".into(),
            role: "Artist".into(),
            bio: "Works with environmental data.".into(),
            places: vec![Place { name: "Studio".into(), city: "Sydney".into() }],
            avatar: String::new(),
        }
    }

    /// Golden value: if this changes, every existing signature breaks.
    #[test]
    fn content_id_is_stable() {
        let body = hash_body(&sample());
        let cid = content_id("tega", "public", "cc-by", &["credit".to_string()], 1_700_000_000, &body);
        assert_eq!(cid, GOLDEN_CID);
    }

    /// Reproducible outside Rust: SHA-256 of the compact JSON
    /// `{"handle":…,"visibility":…,"license":…,"signals":[…],"created_at":…,"body":{…}}`
    /// with the body's keys sorted (checked with Python's
    /// `json.dumps(separators=(",", ":"), ensure_ascii=False)`).
    const GOLDEN_CID: &str = "1f4e0af70ee2d0a728728e0f045a139cbf7cfd34df02a0ea71372ad93b40f659";

    #[test]
    fn avatar_does_not_change_the_cid() {
        let mut with_photo = sample();
        with_photo.avatar = "data:image/png;base64,AAAA".into();
        let a = content_id("tega", "public", "cc-by", &[], 1, &hash_body(&sample()));
        let b = content_id("tega", "public", "cc-by", &[], 1, &hash_body(&with_photo));
        assert_eq!(a, b);
    }

    #[test]
    fn any_field_change_changes_the_cid() {
        let body = hash_body(&sample());
        let base = content_id("tega", "public", "cc-by", &[], 1, &body);
        assert_ne!(base, content_id("tegb", "public", "cc-by", &[], 1, &body));
        assert_ne!(base, content_id("tega", "commons", "cc-by", &[], 1, &body));
        assert_ne!(base, content_id("tega", "public", "cc0", &[], 1, &body));
        assert_ne!(base, content_id("tega", "public", "cc-by", &["open".into()], 1, &body));
        assert_ne!(base, content_id("tega", "public", "cc-by", &[], 2, &body));
    }

    #[test]
    fn is_cid_checks_shape() {
        assert!(is_cid(&"a".repeat(64)));
        assert!(!is_cid(&"A".repeat(64)));
        assert!(!is_cid("../../etc/passwd"));
    }
}
