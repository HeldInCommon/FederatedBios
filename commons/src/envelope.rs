//! Signed objects, format `hic/v1` (decision record 0009, `docs/spec/README.md`).
//!
//! Every new kind of signed object uses one envelope:
//!
//! ```text
//! { "schema": "hic/v1", "type": "KeyRotation", "author": "<ed25519 public key, hex>",
//!   "created_at": 1700000000, "prev": "<cid of the author's previous object, or null>",
//!   "body": { … } }
//! ```
//!
//! - `cid` = lowercase hex SHA-256 of the envelope written as canonical JSON
//!   (RFC 8785, see [`crate::jcs`]).
//! - The author signs the UTF-8 bytes of `hic/v1/<type>\n<cid>`. Putting the
//!   type in what is signed means a signature for one kind of object can't be
//!   passed off as another.
//! - Optional **cosignatures** by other keys sign the same bytes (a key
//!   rotation is cosigned by the new key, proving its owner holds it).
//!
//! Profiles still use the earlier format ("version 0", [`crate::canonical`]),
//! which stays verifiable.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{jcs, keys};

/// The format this module writes and accepts.
pub const SCHEMA: &str = "hic/v1";

/// The unsigned object.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Envelope {
    /// Always `hic/v1` today.
    pub schema: String,
    /// What this object is (`Profile`, `Story`, `Invite` …); `type` on the wire.
    #[serde(rename = "type")]
    pub kind: String,
    /// The signer's Ed25519 public key, hex.
    pub author: String,
    /// When it was signed, in Unix seconds.
    pub created_at: u64,
    /// The `cid` of the author's previous object in the same chain, if any.
    pub prev: Option<String>,
    /// The type's own fields.
    pub body: Value,
}

impl Envelope {
    /// A new `hic/v1` envelope.
    pub fn new(kind: &str, author: &str, created_at: u64, prev: Option<String>, body: Value) -> Envelope {
        Envelope { schema: SCHEMA.into(), kind: kind.into(), author: author.into(), created_at, prev, body }
    }
}

/// An extra signature over the same bytes, by another key.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Cosig {
    /// The cosigner's Ed25519 public key, hex.
    pub key: String,
    /// Their signature, hex.
    pub sig: String,
}

/// An envelope with its `cid` and signatures, as stored and sent.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Signed {
    pub envelope: Envelope,
    /// Hex SHA-256 of the canonical envelope (see the module notes).
    pub cid: String,
    /// The author's signature, hex.
    pub sig: String,
    /// Signatures by other keys, e.g. the new key on a key rotation, proving
    /// whoever made the rotation holds it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cosigs: Vec<Cosig>,
}

/// The canonical bytes of an envelope.
pub fn canonical_bytes(env: &Envelope) -> Result<Vec<u8>, String> {
    jcs::to_vec(&serde_json::to_value(env).map_err(|e| e.to_string())?)
}

/// The `cid` of an envelope.
pub fn cid(env: &Envelope) -> Result<String, String> {
    Ok(hex::encode(Sha256::digest(canonical_bytes(env)?)))
}

/// The exact text every signature on this object covers.
pub fn signing_input(kind: &str, cid: &str) -> String {
    format!("{SCHEMA}/{kind}\n{cid}")
}

/// Sign an envelope with the author's private key (hex). The key must match
/// `env.author`.
pub fn sign(env: Envelope, author_priv: &str) -> Result<Signed, String> {
    if keys::public_from_private(author_priv).as_deref() != Some(env.author.as_str()) {
        return Err("the signing key doesn't match the author".into());
    }
    let cid = cid(&env)?;
    let sig = keys::sign_message(author_priv, signing_input(&env.kind, &cid).as_bytes())?;
    Ok(Signed { envelope: env, cid, sig, cosigs: Vec::new() })
}

/// Add a cosignature by another key.
pub fn cosign(mut s: Signed, priv_hex: &str) -> Result<Signed, String> {
    let key = keys::public_from_private(priv_hex).ok_or("damaged key")?;
    let sig = keys::sign_message(priv_hex, signing_input(&s.envelope.kind, &s.cid).as_bytes())?;
    s.cosigs.push(Cosig { key, sig });
    Ok(s)
}

/// Check a signed object: the schema, that the `cid` matches the content, the
/// author's signature, and every cosignature.
pub fn verify(s: &Signed) -> Result<(), String> {
    if s.envelope.schema != SCHEMA {
        return Err(format!("unsupported schema {:?}", s.envelope.schema));
    }
    if cid(&s.envelope)? != s.cid {
        return Err("content hash mismatch".into());
    }
    let input = signing_input(&s.envelope.kind, &s.cid);
    keys::verify_message(&s.envelope.author, input.as_bytes(), &s.sig)?;
    for c in &s.cosigs {
        keys::verify_message(&c.key, input.as_bytes(), &c.sig)?;
    }
    Ok(())
}

/// True when `s` carries a valid cosignature by `key`. (Call [`verify`] first.)
pub fn cosigned_by(s: &Signed, key: &str) -> bool {
    s.cosigs.iter().any(|c| c.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sign_and_verify() {
        let k = keys::generate();
        let s = sign(Envelope::new("Note", &k.ed_pub, 1, None, json!({"text": "hi"})), &k.ed_priv).unwrap();
        assert!(verify(&s).is_ok());
    }

    #[test]
    fn any_change_breaks_it() {
        let k = keys::generate();
        let s = sign(Envelope::new("Note", &k.ed_pub, 1, None, json!({"text": "hi"})), &k.ed_priv).unwrap();
        let mut body = s.clone();
        body.envelope.body = json!({"text": "bye"});
        assert!(verify(&body).is_err());
        let mut kind = s.clone();
        kind.envelope.kind = "Other".into();
        assert!(verify(&kind).is_err(), "changing the type changes the cid");
    }

    #[test]
    fn a_signature_cannot_be_reused_for_another_type() {
        let k = keys::generate();
        let a = sign(Envelope::new("A", &k.ed_pub, 1, None, json!({})), &k.ed_priv).unwrap();
        let mut b = sign(Envelope::new("B", &k.ed_pub, 1, None, json!({})), &k.ed_priv).unwrap();
        b.sig = a.sig.clone();
        assert!(verify(&b).is_err());
    }

    #[test]
    fn cosignatures_are_checked() {
        let (a, b) = (keys::generate(), keys::generate());
        let s = sign(Envelope::new("Note", &a.ed_pub, 1, None, json!({})), &a.ed_priv).unwrap();
        let s = cosign(s, &b.ed_priv).unwrap();
        assert!(verify(&s).is_ok() && cosigned_by(&s, &b.ed_pub));
        let mut bad = s.clone();
        bad.cosigs[0].sig = s.sig.clone();
        assert!(verify(&bad).is_err());
    }

    #[test]
    fn author_must_match_the_signing_key() {
        let (a, b) = (keys::generate(), keys::generate());
        assert!(sign(Envelope::new("Note", &a.ed_pub, 1, None, json!({})), &b.ed_priv).is_err());
    }

    #[test]
    fn floats_cannot_be_signed() {
        let k = keys::generate();
        assert!(sign(Envelope::new("Note", &k.ed_pub, 1, None, json!({"x": 0.5})), &k.ed_priv).is_err());
    }
}
