//! Keys: generating them, signing and verifying `cid`s, and encrypting
//! members-only profiles.
//!
//! Each handle has two keypairs:
//! - **Ed25519** signs every version (the signature is over the `cid` string);
//! - **X25519** receives members-only versions, sealed with a one-time key per
//!   recipient so no sender identity is attached.
//!
//! Keys are passed around as lowercase hex, which is how they are stored.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use crypto_box::{
    aead::{Aead, AeadCore, OsRng},
    ChaChaBox, PublicKey as BoxPublic, SecretKey as BoxSecret,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

use crate::model::{MemberBox, Profile};

/// A new set of keys for a handle, all hex-encoded.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct NewKeys {
    pub ed_pub: String,
    pub ed_priv: String,
    pub box_pub: String,
    pub box_priv: String,
}

/// Generate a fresh signing keypair and encryption keypair.
pub fn generate() -> NewKeys {
    let ed = SigningKey::generate(&mut OsRng);
    let bx = BoxSecret::generate(&mut OsRng);
    NewKeys {
        ed_pub: hex::encode(ed.verifying_key().to_bytes()),
        ed_priv: hex::encode(ed.to_bytes()),
        box_pub: hex::encode(bx.public_key().as_bytes()),
        box_priv: hex::encode(bx.to_bytes()),
    }
}

/// Exactly 32 bytes from hex, or `None`. A wrong length is an error, never
/// padded: a padded private key would silently sign with a different key.
fn hex32(h: &str) -> Option<[u8; 32]> {
    hex::decode(h).ok()?.try_into().ok()
}

/// Sign `msg` with a hex Ed25519 private key. Returns the hex signature, or
/// an error if the stored key is damaged.
pub fn sign_message(ed_priv_hex: &str, msg: &[u8]) -> Result<String, &'static str> {
    let key = SigningKey::from_bytes(&hex32(ed_priv_hex).ok_or("the signing key is damaged")?);
    Ok(hex::encode(key.sign(msg).to_bytes()))
}

/// Check a hex signature over `msg` against a hex Ed25519 public key.
pub fn verify_message(ed_pub_hex: &str, msg: &[u8], sig_hex: &str) -> Result<(), &'static str> {
    let vk = VerifyingKey::from_bytes(&hex32(ed_pub_hex).ok_or("bad key")?).map_err(|_| "bad key")?;
    let sig_bytes: [u8; 64] = hex::decode(sig_hex).ok().and_then(|b| b.try_into().ok()).ok_or("bad signature")?;
    vk.verify(msg, &Signature::from_bytes(&sig_bytes)).map_err(|_| "signature does not verify")
}

/// The hex public key for a hex Ed25519 private key, or `None` if damaged.
pub fn public_from_private(ed_priv_hex: &str) -> Option<String> {
    Some(hex::encode(SigningKey::from_bytes(&hex32(ed_priv_hex)?).verifying_key().to_bytes()))
}

/// A new Ed25519 keypair alone, as `(public, private)` hex. Used for recovery keys.
pub fn generate_signing() -> (String, String) {
    let k = SigningKey::generate(&mut OsRng);
    (hex::encode(k.verifying_key().to_bytes()), hex::encode(k.to_bytes()))
}

/// A private key written for printing: 64 hex characters in eight groups of
/// eight, e.g. `1a2b3c4d-…`. Easy to copy by hand and to read back.
pub fn recovery_code(priv_hex: &str) -> String {
    priv_hex.as_bytes().chunks(8).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join("-")
}

/// Read a recovery code back (spaces, dashes and case are ignored). `None`
/// if it isn't 64 hex characters.
pub fn parse_recovery_code(text: &str) -> Option<String> {
    let hex: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>().to_lowercase();
    let other = text.chars().any(|c| !(c.is_ascii_hexdigit() || c == '-' || c.is_whitespace()));
    (hex.len() == 64 && !other).then_some(hex)
}

/// Sign a version's `cid` (the signature is over the `cid` string itself).
pub fn sign_cid(ed_priv_hex: &str, cid: &str) -> Result<String, &'static str> {
    sign_message(ed_priv_hex, cid.as_bytes())
}

/// Check a signature over a version's `cid`.
pub fn verify_cid(ed_pub_hex: &str, cid: &str, sig_hex: &str) -> Result<(), &'static str> {
    verify_message(ed_pub_hex, cid.as_bytes(), sig_hex)
}

/// Encrypt a profile once per recipient. `recipients` is `(handle, box_pub hex)`;
/// recipients with an unusable key are skipped. Each copy uses its own one-time
/// key and nonce.
pub fn seal_for_members(profile: &Profile, recipients: &[(String, String)]) -> Vec<MemberBox> {
    let plain = serde_json::to_vec(profile).expect("profile serialises");
    recipients
        .iter()
        .filter_map(|(handle, box_pub)| {
            let their_pub = BoxPublic::from(hex32(box_pub)?);
            let one_time = BoxSecret::generate(&mut OsRng);
            let nonce = ChaChaBox::generate_nonce(&mut OsRng);
            let ct = ChaChaBox::new(&their_pub, &one_time).encrypt(&nonce, plain.as_ref()).ok()?;
            Some(MemberBox {
                handle: handle.clone(),
                eph_pub: hex::encode(one_time.public_key().as_bytes()),
                nonce: hex::encode(nonce),
                ct: B64.encode(ct),
            })
        })
        .collect()
}

/// Decrypt the copy addressed to `viewer`, using their hex X25519 private key.
/// `None` if there is no copy for them or it doesn't decrypt.
pub fn open_for(boxes: &[MemberBox], viewer: &str, box_priv_hex: &str) -> Option<Profile> {
    let mb = boxes.iter().find(|b| b.handle == viewer)?;
    let secret = BoxSecret::from(hex32(box_priv_hex)?);
    let one_time_pub = BoxPublic::from(hex32(&mb.eph_pub)?);
    let nonce_bytes = hex::decode(&mb.nonce).ok()?;
    if nonce_bytes.len() != 24 {
        return None;
    }
    let ct = B64.decode(&mb.ct).ok()?;
    let plain = ChaChaBox::new(&one_time_pub, &secret)
        .decrypt(crypto_box::Nonce::from_slice(&nonce_bytes), ct.as_ref())
        .ok()?;
    serde_json::from_slice(&plain).ok()
}

/// Compare two strings in time that doesn't depend on where they differ.
/// Used for CSRF tokens and legacy passphrase hashes.
pub fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Short, stable id for a federated identity: the first 16 hex characters of
/// `sha256(ed_pub)`. Used as the file name and URL for a shared profile.
pub fn keyid(ed_pub: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(ed_pub.as_bytes()))[..16].to_string()
}

/// True when `s` has the shape of a [`keyid`].
pub fn is_keyid(s: &str) -> bool {
    s.len() == 16 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_round_trip() {
        let k = generate();
        let sig = sign_cid(&k.ed_priv, "abc").unwrap();
        assert!(verify_cid(&k.ed_pub, "abc", &sig).is_ok());
        assert!(verify_cid(&k.ed_pub, "abd", &sig).is_err());
    }

    #[test]
    fn damaged_private_key_is_an_error_not_a_zero_key() {
        assert!(sign_cid("abcd", "cid").is_err());
        assert!(sign_cid("zz", "cid").is_err());
    }

    #[test]
    fn members_box_opens_only_for_its_member() {
        let a = generate();
        let b = generate();
        let p = Profile { name: "Mo".into(), ..Default::default() };
        let boxes = seal_for_members(&p, &[("mo".into(), a.box_pub.clone())]);
        assert_eq!(open_for(&boxes, "mo", &a.box_priv), Some(p));
        assert_eq!(open_for(&boxes, "mo", &b.box_priv), None);
        assert_eq!(open_for(&boxes, "someone", &a.box_priv), None);
    }

    #[test]
    fn recovery_code_round_trip() {
        let (public, private) = generate_signing();
        let code = recovery_code(&private);
        assert_eq!(code.len(), 64 + 7);
        assert_eq!(parse_recovery_code(&code.to_uppercase().replace('-', " ")), Some(private.clone()));
        assert_eq!(public_from_private(&private), Some(public));
        assert_eq!(parse_recovery_code("not a code"), None);
        assert_eq!(parse_recovery_code(&code[..20]), None);
    }

    #[test]
    fn ct_eq_matches_equality() {
        assert!(ct_eq("abc", "abc"));
        assert!(!ct_eq("abc", "abd"));
        assert!(!ct_eq("abc", "abcd"));
    }

    #[test]
    fn keyid_shape() {
        let id = keyid("00");
        assert!(is_keyid(&id));
        assert!(!is_keyid("../x"));
    }
}
