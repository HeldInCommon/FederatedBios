//! Encrypted identity backups.
//!
//! On a commons a handle belongs to the key that first shared it, so losing
//! the keys means losing the handle everywhere. A backup lets a member move
//! their identity to a new computer: their record (keys included) and current
//! version, encrypted with their passphrase.
//!
//! Format (JSON, safe to email to yourself; useless without the passphrase):
//!
//! ```text
//! {
//!   "format": "hic-identity-backup/1",
//!   "handle": "tega",                     // in the clear, so you can tell files apart
//!   "kdf": "argon2id", "salt": "<hex>",   // key = Argon2id(passphrase, salt), 32 bytes
//!   "cipher": "xchacha20poly1305", "nonce": "<hex>",
//!   "ct": "<base64 of the encrypted JSON {record, version}>"
//! }
//! ```
//!
//! The handle is also authenticated (it is the cipher's associated data), so
//! it can't be edited without the backup failing to open.

use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};

use crate::model::{HandleRec, Version};

/// The format tag. A future, incompatible format gets a new number.
pub const FORMAT: &str = "hic-identity-backup/1";

/// The backup file.
#[derive(Serialize, Deserialize)]
struct BackupFile {
    format: String,
    handle: String,
    kdf: String,
    salt: String,
    cipher: String,
    nonce: String,
    ct: String,
}

/// What is inside, once decrypted.
#[derive(Serialize, Deserialize)]
struct Contents {
    record: HandleRec,
    version: Option<Version>,
}

/// The 32-byte key for `passphrase` and `salt`.
fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    Argon2::default().hash_password_into(passphrase.as_bytes(), salt, &mut key).map_err(|e| e.to_string())?;
    Ok(key)
}

/// Seal a member's record and current version into a backup file (as text).
pub fn seal(record: &HandleRec, version: Option<&Version>, passphrase: &str) -> Result<String, String> {
    let mut salt = [0u8; 16];
    rand::RngCore::fill_bytes(&mut OsRng, &mut salt);
    let cipher = XChaCha20Poly1305::new(&derive_key(passphrase, &salt)?.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let plain = serde_json::to_vec(&Contents { record: record.clone(), version: version.cloned() })
        .map_err(|e| e.to_string())?;
    let ct = cipher
        .encrypt(&nonce, Payload { msg: &plain, aad: record.handle.as_bytes() })
        .map_err(|_| "encryption failed".to_string())?;
    let file = BackupFile {
        format: FORMAT.into(),
        handle: record.handle.clone(),
        kdf: "argon2id".into(),
        salt: hex::encode(salt),
        cipher: "xchacha20poly1305".into(),
        nonce: hex::encode(nonce),
        ct: B64.encode(ct),
    };
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

/// Open a backup file with its passphrase. The error is a message for the
/// person restoring.
pub fn open(text: &str, passphrase: &str) -> Result<(HandleRec, Option<Version>), String> {
    let file: BackupFile = serde_json::from_str(text.trim())
        .map_err(|_| "That doesn't look like a Held in Common backup file.".to_string())?;
    if file.format != FORMAT || file.kdf != "argon2id" || file.cipher != "xchacha20poly1305" {
        return Err("This backup was made by a different version of Held in Common.".into());
    }
    let damaged = || "The backup file is damaged.".to_string();
    let salt = hex::decode(&file.salt).map_err(|_| damaged())?;
    let nonce_bytes = hex::decode(&file.nonce).map_err(|_| damaged())?;
    if nonce_bytes.len() != 24 {
        return Err(damaged());
    }
    let ct = B64.decode(&file.ct).map_err(|_| damaged())?;
    let cipher = XChaCha20Poly1305::new(&derive_key(passphrase, &salt)?.into());
    let plain = cipher
        .decrypt(XNonce::from_slice(&nonce_bytes), Payload { msg: &ct, aad: file.handle.as_bytes() })
        .map_err(|_| "Wrong passphrase, or the backup file was changed.".to_string())?;
    let c: Contents = serde_json::from_slice(&plain).map_err(|_| damaged())?;
    if c.record.handle != file.handle {
        return Err(damaged());
    }
    Ok((c.record, c.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A member record with fresh keys, to back up and restore.
    fn rec() -> HandleRec {
        let k = crate::keys::generate();
        HandleRec {
            handle: "tega".into(),
            display_name: "Tega".into(),
            created_at: 1,
            ed_pub: k.ed_pub,
            ed_priv: k.ed_priv,
            box_pub: k.box_pub,
            box_priv: k.box_priv,
            secret_salt: String::new(),
            secret_hash: "x".into(),
            current_cid: None,
            shared_to: vec!["http://c".into()],
            recovery_pub: None,
            recovery_decl: None,
            pending_rotation: None,
            backup_done: true,
            waiting_on: vec![],
        }
    }

    #[test]
    fn round_trip() {
        let r = rec();
        let file = seal(&r, None, "supersecret1").unwrap();
        assert!(file.contains(FORMAT) && file.contains("\"tega\""));
        assert!(!file.contains(&r.ed_priv), "private key must not appear in the clear");
        let (back, v) = open(&file, "supersecret1").unwrap();
        assert_eq!(back.ed_priv, r.ed_priv);
        assert!(v.is_none());
    }

    #[test]
    fn wrong_passphrase_fails() {
        let file = seal(&rec(), None, "supersecret1").unwrap();
        assert!(open(&file, "nope-nope").unwrap_err().contains("Wrong passphrase"));
    }

    #[test]
    fn edited_handle_fails() {
        let file = seal(&rec(), None, "supersecret1").unwrap().replace("\"tega\"", "\"mo\"");
        assert!(open(&file, "supersecret1").is_err());
    }

    #[test]
    fn not_a_backup() {
        assert!(open("hello", "x").unwrap_err().contains("doesn't look like"));
    }
}
