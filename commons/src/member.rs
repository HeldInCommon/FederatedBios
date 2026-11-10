//! Pure changes to a member's record on their own node.
//!
//! Each function takes the record (and anything else it needs) and returns
//! the changed record and anything to save alongside it. Nothing here touches
//! the disk: the request handlers save what these return, so every write is
//! visible where it happens.

use crate::identity;
use crate::keys;
use crate::model::{HandleRec, PendingRotation, Profile, Version};
use crate::publishing::{self, Draft};

/// The result of moving a member to new keys.
#[derive(Debug)]
pub struct Rekeyed {
    /// The record with the new keys, a fresh recovery declaration, and no
    /// pending rotation.
    pub record: HandleRec,
    /// The current profile re-signed with the new key, if there was one.
    /// Save it, and point `record.current_cid` at it (already done here).
    pub version: Option<Version>,
    /// For a recovery: the next recovery code, to show once.
    pub new_recovery_code: Option<String>,
}

/// Move `rec` to the keys in `pending`.
///
/// - `current`: the current version and its readable profile (read with the
///   *old* keys before calling this), to be re-signed with the new key;
/// - `members`: every member of this node as `(handle, box_pub)`, for
///   re-encrypting a members-only profile (this member's entry is replaced
///   with the new encryption key).
///
/// Pure apart from the randomness used when encrypting members-only profiles.
pub fn rekey(
    mut rec: HandleRec,
    pending: &PendingRotation,
    current: Option<(Version, Profile)>,
    members: &[(String, String)],
    now: u64,
) -> Result<Rekeyed, String> {
    let k = &pending.keys;
    (rec.ed_pub, rec.ed_priv, rec.box_pub, rec.box_priv) =
        (k.ed_pub.clone(), k.ed_priv.clone(), k.box_pub.clone(), k.box_priv.clone());
    let mut new_recovery_code = None;
    if let Some((next_pub, next_priv)) = &pending.next_recovery {
        rec.recovery_pub = Some(next_pub.clone());
        new_recovery_code = Some(keys::recovery_code(next_priv));
    }
    if let Some(rp) = rec.recovery_pub.clone() {
        rec.recovery_decl = Some(identity::declare_recovery(&rec.handle, &rec.ed_pub, &rec.ed_priv, &rp, now)?);
    }
    let version = match current {
        Some((v, profile)) => {
            let recipients: Vec<(String, String)> = members
                .iter()
                .map(|(h, b)| if *h == rec.handle { (h.clone(), rec.box_pub.clone()) } else { (h.clone(), b.clone()) })
                .collect();
            let draft = Draft { profile, reach: v.reach(), license: v.license, signals: v.signals, created_at: now };
            let signed = publishing::sign_version(&rec, draft, &recipients)?;
            rec.current_cid = Some(signed.cid.clone());
            Some(signed)
        }
        None => None,
    };
    rec.pending_rotation = None;
    Ok(Rekeyed { record: rec, version, new_recovery_code })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Reach;

    fn member_with_version() -> (HandleRec, Version, Profile) {
        let rec = HandleRec::new("tega", "Tega", 0, keys::generate(), String::new(), String::new());
        let profile = Profile { name: "Tega".into(), ..Default::default() };
        let draft = Draft {
            profile: profile.clone(),
            reach: Reach::Public,
            license: "cc-by".into(),
            signals: vec![],
            created_at: 1,
        };
        let v = publishing::sign_version(&rec, draft, &[]).unwrap();
        (rec, v, profile)
    }

    fn pending_for(rec: &HandleRec, next_recovery: Option<(String, String)>) -> PendingRotation {
        let new = keys::generate();
        let change = identity::rotate(&rec.handle, &rec.ed_pub, &rec.ed_priv, &new, 5).unwrap();
        PendingRotation { keys: new, change, next_recovery, done: vec![] }
    }

    #[test]
    fn rekey_switches_keys_and_resigns_the_profile() {
        let (rec, v, profile) = member_with_version();
        let pending = pending_for(&rec, None);
        let out = rekey(rec.clone(), &pending, Some((v, profile)), &[], 10).unwrap();
        assert_eq!(out.record.ed_pub, pending.keys.ed_pub);
        assert!(out.record.pending_rotation.is_none());
        let nv = out.version.unwrap();
        assert_eq!(out.record.current_cid.as_deref(), Some(nv.cid.as_str()));
        assert!(keys::verify_cid(&pending.keys.ed_pub, &nv.cid, &nv.signature).is_ok());
        assert!(out.new_recovery_code.is_none());
    }

    #[test]
    fn a_recovery_sets_the_next_recovery_key_and_code() {
        let (rec, _, _) = member_with_version();
        let next = keys::generate_signing();
        let pending = pending_for(&rec, Some(next.clone()));
        let out = rekey(rec, &pending, None, &[], 10).unwrap();
        assert_eq!(out.record.recovery_pub.as_deref(), Some(next.0.as_str()));
        assert_eq!(out.new_recovery_code, Some(keys::recovery_code(&next.1)));
        assert!(out.record.recovery_decl.is_some(), "re-declared with the new key");
    }
}
