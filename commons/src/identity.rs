//! Who holds a handle on a commons, and how that changes: recovery keys and
//! key rotation (decision record 0010).
//!
//! A commons keeps one [`Identity`] per handle: the key that currently holds
//! it, that key's encryption key, and (once declared) a **recovery key**.
//! Three signed `hic/v1` objects change it:
//!
//! | Type | Signed by | Effect |
//! |---|---|---|
//! | `RecoveryKey` | the current key | records the recovery key (once; it can't be replaced this way) |
//! | `KeyRotation` | the current key, cosigned by the new key | moves the handle to the new key |
//! | `RecoveryRotation` | the recovery key, cosigned by the new key | moves the handle to the new key and sets a new recovery key |
//!
//! A recovery rotation wins over ordinary rotations: if a thief rotates with a
//! stolen key, the owner's recovery key (kept offline) still controls the
//! handle, and a thief can't change the recovery key without it.
//!
//! Everything here is pure; [`apply`] takes the current identity and returns
//! the new one.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::envelope::{self, Envelope, Signed};
use crate::keys::NewKeys;

/// The envelope `type` that declares a handle's recovery key.
pub const RECOVERY_KEY: &str = "RecoveryKey";
/// The envelope `type` that moves a handle to a new main key, signed by the
/// current one.
pub const KEY_ROTATION: &str = "KeyRotation";
/// The envelope `type` that, signed by the recovery key, moves a handle to a
/// new main key and sets a new recovery key (when the main key is lost or stolen).
pub const RECOVERY_ROTATION: &str = "RecoveryRotation";

/// How far ahead of the commons' clock a rotation may be dated (one day).
/// Refusing later dates stops a thief from dating a rotation far in the future
/// so that no later one can follow it.
pub const MAX_CLOCK_SKEW: u64 = 86_400;

/// A commons' record of who holds a handle. Lives in `identities/<handle>.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Identity {
    pub handle: String,
    /// The Ed25519 key that currently holds the handle.
    pub key: String,
    /// That key's X25519 encryption key.
    pub box_key: String,
    /// The recovery key's public half, once declared.
    pub recovery_key: Option<String>,
    /// Time of the last ordinary rotation (0 if none).
    pub last_rotation_at: u64,
    /// Time of the last recovery rotation (0 if none).
    pub last_recovery_at: u64,
    /// Every accepted change, in order: the identity's audit trail.
    pub history: Vec<Signed>,
}

impl Identity {
    /// A new record for a handle first seen with `key`.
    pub fn new(handle: &str, key: &str, box_key: &str) -> Identity {
        Identity {
            handle: handle.into(),
            key: key.into(),
            box_key: box_key.into(),
            recovery_key: None,
            last_rotation_at: 0,
            last_recovery_at: 0,
            history: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Building the objects (on a node)
// ---------------------------------------------------------------------------

/// Declare a recovery key, signed by the handle's current key.
pub fn declare_recovery(
    handle: &str,
    key_pub: &str,
    key_priv: &str,
    recovery_pub: &str,
    t: u64,
) -> Result<Signed, String> {
    let env = Envelope::new(RECOVERY_KEY, key_pub, t, None, json!({ "handle": handle, "recovery_key": recovery_pub }));
    envelope::sign(env, key_priv)
}

/// Move a handle to `new` keys: signed by the old key, cosigned by the new one.
pub fn rotate(handle: &str, old_pub: &str, old_priv: &str, new: &NewKeys, t: u64) -> Result<Signed, String> {
    let body = json!({ "handle": handle, "new_key": new.ed_pub, "new_box_key": new.box_pub });
    let s = envelope::sign(Envelope::new(KEY_ROTATION, old_pub, t, None, body), old_priv)?;
    envelope::cosign(s, &new.ed_priv)
}

/// Move a handle to `new` keys using the recovery key, and name the next
/// recovery key. Signed by the recovery key, cosigned by the new key.
pub fn recover(
    handle: &str,
    recovery_pub: &str,
    recovery_priv: &str,
    new: &NewKeys,
    new_recovery_pub: &str,
    t: u64,
) -> Result<Signed, String> {
    let body = json!({
        "handle": handle, "new_key": new.ed_pub, "new_box_key": new.box_pub, "new_recovery_key": new_recovery_pub,
    });
    let s = envelope::sign(Envelope::new(RECOVERY_ROTATION, recovery_pub, t, None, body), recovery_priv)?;
    envelope::cosign(s, &new.ed_priv)
}

// ---------------------------------------------------------------------------
// Applying them (on a commons)
// ---------------------------------------------------------------------------

/// The string field `key` of a signed change's body, or a "missing …" error.
///
/// Learner note: `'a` says the returned `&str` borrows from `body`, so it
/// lives as long as `body` does and no copy is made.
fn body_str<'a>(body: &'a Value, key: &str) -> Result<&'a str, String> {
    body.get(key).and_then(|v| v.as_str()).ok_or_else(|| format!("missing {key}"))
}

/// Apply a signed change to an identity at time `now`, returning the updated
/// identity, or why it was refused. Applying a change already in the history
/// returns the identity unchanged (so a node can safely retry).
pub fn apply(id: &Identity, s: &Signed, now: u64) -> Result<Identity, String> {
    envelope::verify(s)?;
    if id.history.iter().any(|h| h.cid == s.cid) {
        return Ok(id.clone());
    }
    let env = &s.envelope;
    if body_str(&env.body, "handle")? != id.handle {
        return Err("this change is for a different handle".into());
    }
    if env.created_at > now + MAX_CLOCK_SKEW {
        return Err("this change is dated in the future".into());
    }
    let mut next = id.clone();
    match env.kind.as_str() {
        RECOVERY_KEY => {
            if env.author != id.key {
                return Err("only the key that holds this handle can declare its recovery key".into());
            }
            if id.recovery_key.is_some() {
                return Err("a recovery key is already recorded; only a recovery rotation can change it".into());
            }
            next.recovery_key = Some(body_str(&env.body, "recovery_key")?.to_string());
        }
        KEY_ROTATION => {
            if env.author != id.key {
                return Err("only the key that holds this handle can rotate it".into());
            }
            if env.created_at <= id.last_rotation_at.max(id.last_recovery_at) {
                return Err("this rotation is older than the current key".into());
            }
            next.key = new_key_cosigned(s)?;
            next.box_key = body_str(&env.body, "new_box_key")?.to_string();
            next.last_rotation_at = env.created_at;
        }
        RECOVERY_ROTATION => {
            if id.recovery_key.as_deref() != Some(env.author.as_str()) {
                return Err("that isn't this handle's recovery key".into());
            }
            // Compared only with earlier recovery rotations: ordinary rotations
            // may have been made by a thief, and must not be able to block this.
            if env.created_at <= id.last_recovery_at {
                return Err("this recovery is older than the last one".into());
            }
            next.key = new_key_cosigned(s)?;
            next.box_key = body_str(&env.body, "new_box_key")?.to_string();
            next.recovery_key = Some(body_str(&env.body, "new_recovery_key")?.to_string());
            next.last_recovery_at = env.created_at;
        }
        other => return Err(format!("unknown change {other:?}")),
    }
    next.history.push(s.clone());
    Ok(next)
}

/// The new key named in a rotation, provided it cosigned the rotation.
fn new_key_cosigned(s: &Signed) -> Result<String, String> {
    let new_key = body_str(&s.envelope.body, "new_key")?;
    if !envelope::cosigned_by(s, new_key) {
        return Err("the new key must cosign the rotation".into());
    }
    Ok(new_key.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;

    struct Owner {
        main: NewKeys,
        recovery: (String, String),
    }

    /// A new owner: a main key pair and a separate recovery key.
    fn owner() -> Owner {
        Owner { main: keys::generate(), recovery: keys::generate_signing() }
    }

    /// A commons' identity record for `o`, with their recovery key declared.
    fn registered(o: &Owner) -> Identity {
        let id = Identity::new("tega", &o.main.ed_pub, &o.main.box_pub);
        let d = declare_recovery("tega", &o.main.ed_pub, &o.main.ed_priv, &o.recovery.0, 10).unwrap();
        apply(&id, &d, 10).unwrap()
    }

    #[test]
    fn recovery_key_is_recorded_once() {
        let o = owner();
        let id = registered(&o);
        assert_eq!(id.recovery_key.as_deref(), Some(o.recovery.0.as_str()));
        let other = keys::generate_signing();
        let again = declare_recovery("tega", &o.main.ed_pub, &o.main.ed_priv, &other.0, 11).unwrap();
        assert!(apply(&id, &again, 11).unwrap_err().contains("already recorded"));
    }

    #[test]
    fn ordinary_rotation_moves_the_handle() {
        let o = owner();
        let id = registered(&o);
        let new = keys::generate();
        let r = rotate("tega", &o.main.ed_pub, &o.main.ed_priv, &new, 20).unwrap();
        let id2 = apply(&id, &r, 20).unwrap();
        assert_eq!(id2.key, new.ed_pub);
        assert_eq!(id2.recovery_key, id.recovery_key, "recovery key unchanged");
        assert_eq!(apply(&id2, &r, 21).unwrap(), id2, "retrying is harmless");
    }

    #[test]
    fn rotation_needs_the_current_key_and_the_new_keys_cosignature() {
        let o = owner();
        let id = registered(&o);
        let stranger = keys::generate();
        let r = rotate("tega", &stranger.ed_pub, &stranger.ed_priv, &keys::generate(), 20).unwrap();
        assert!(apply(&id, &r, 20).is_err());
        let mut r = rotate("tega", &o.main.ed_pub, &o.main.ed_priv, &keys::generate(), 20).unwrap();
        r.cosigs.clear();
        assert!(apply(&id, &r, 20).unwrap_err().contains("cosign"));
    }

    #[test]
    fn recovery_beats_a_thief() {
        let o = owner();
        let id = registered(&o);
        // A thief with the stolen main key rotates to their own key, dated far ahead.
        let thief = keys::generate();
        let far = rotate("tega", &o.main.ed_pub, &o.main.ed_priv, &thief, 10_000_000).unwrap();
        assert!(apply(&id, &far, 30).unwrap_err().contains("future"));
        let t = rotate("tega", &o.main.ed_pub, &o.main.ed_priv, &thief, 30).unwrap();
        let stolen = apply(&id, &t, 30).unwrap();
        assert_eq!(stolen.key, thief.ed_pub);
        // The thief can't change the recovery key.
        let fake = keys::generate_signing();
        let d = declare_recovery("tega", &thief.ed_pub, &thief.ed_priv, &fake.0, 31).unwrap();
        assert!(apply(&stolen, &d, 31).is_err());
        // The owner recovers with the offline recovery key.
        let fresh = keys::generate();
        let next_recovery = keys::generate_signing();
        let rr = recover("tega", &o.recovery.0, &o.recovery.1, &fresh, &next_recovery.0, 40).unwrap();
        let back = apply(&stolen, &rr, 40).unwrap();
        assert_eq!(back.key, fresh.ed_pub);
        assert_eq!(back.recovery_key.as_deref(), Some(next_recovery.0.as_str()));
        // The thief's key no longer works.
        let again = rotate("tega", &thief.ed_pub, &thief.ed_priv, &keys::generate(), 50).unwrap();
        assert!(apply(&back, &again, 50).is_err());
        assert_eq!(back.history.len(), 3);
    }

    #[test]
    fn recovery_needs_the_right_recovery_key() {
        let o = owner();
        let id = registered(&o);
        let wrong = keys::generate_signing();
        let rr = recover("tega", &wrong.0, &wrong.1, &keys::generate(), &wrong.0, 40).unwrap();
        assert!(apply(&id, &rr, 40).unwrap_err().contains("recovery key"));
    }

    #[test]
    fn changes_for_another_handle_are_refused() {
        let o = owner();
        let id = registered(&o);
        let r = rotate("mo", &o.main.ed_pub, &o.main.ed_priv, &keys::generate(), 20).unwrap();
        assert!(apply(&id, &r, 20).unwrap_err().contains("different handle"));
    }
}
