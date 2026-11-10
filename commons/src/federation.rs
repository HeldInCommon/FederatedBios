//! Sharing between servers: the signed share a node sends, the checks a
//! commons runs on it, and the rule for whether to accept it.
//!
//! A share is an ActivityStreams-shaped `Create` carrying one version and the
//! sender's public keys. The commons recomputes the `cid` from the content it
//! received (so tampering shows), checks the Ed25519 signature over that `cid`,
//! then applies [`admit`]: on one commons a handle belongs to one key, and an
//! older version may not replace a newer one.
//!
//! Everything here is pure; the request handlers do the I/O.

use serde_json::{json, Value};

use crate::canonical::{content_id, hash_body};
use crate::handle;
use crate::keys::verify_cid;
use crate::model::{HandleRec, Profile, Reach, RemoteEntry, Version};

/// Who is sending a share: this node's public address and name.
pub struct Sender<'a> {
    pub base_url: &'a str,
    pub instance: &'a str,
}

/// Build the share a node posts to a commons' `/inbox`.
pub fn build_share(sender: &Sender, rec: &HandleRec, v: &Version) -> Value {
    let profile = v.payload.clone().unwrap_or_default();
    json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "actor": {
            "type": "Person",
            "preferredUsername": rec.handle,
            "name": rec.display_name,
            "id": format!("{}/ap/@{}", sender.base_url, rec.handle),
            "origin": sender.base_url,
            "originLabel": sender.instance,
            "ed25519PublicKey": rec.ed_pub,
            "boxPublicKey": rec.box_pub,
        },
        "object": {
            "type": "Profile",
            "cid": v.cid,
            "visibility": v.visibility,
            "license": v.license,
            "signals": v.signals,
            "createdAt": v.created_at,
            "profile": profile,
        },
        "signature": { "type": "ed25519", "value": v.signature },
    })
}

/// A string field, or `None`.
fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}

/// Check a received share and turn it into a [`RemoteEntry`] stamped with
/// `received_at`. Fails if a field is missing, the reach is members-only or
/// unknown, the `cid` doesn't match the content, or the signature is invalid.
pub fn verify_share(activity: &Value, received_at: u64) -> Result<RemoteEntry, String> {
    let actor = activity.get("actor").ok_or("missing actor")?;
    let object = activity.get("object").ok_or("missing object")?;
    let ed_pub = str_at(actor, "ed25519PublicKey").ok_or("missing key")?;
    let handle_ = str_at(actor, "preferredUsername").ok_or("missing handle")?;
    if !handle::is_valid(handle_) {
        return Err("invalid handle".into());
    }
    let visibility = str_at(object, "visibility").unwrap_or("public").to_string();
    match Reach::parse(&visibility) {
        Some(Reach::Members) => return Err("members-only profiles are not federated".into()),
        None => return Err("unknown visibility".into()),
        Some(_) => {}
    }
    let license = str_at(object, "license").unwrap_or("arr").to_string();
    let signals: Vec<String> = object
        .get("signals")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let created_at = object.get("createdAt").and_then(|v| v.as_u64()).unwrap_or(0);
    let profile: Profile =
        serde_json::from_value(object.get("profile").cloned().unwrap_or_default()).map_err(|_| "bad profile")?;
    let claimed_cid = str_at(object, "cid").ok_or("missing cid")?;
    let sig_hex =
        activity.get("signature").and_then(|s| s.get("value")).and_then(|v| v.as_str()).ok_or("missing signature")?;

    // Recompute the content address from what was actually sent.
    let cid = content_id(handle_, &visibility, &license, &signals, created_at, &hash_body(&profile));
    if cid != claimed_cid {
        return Err("content hash mismatch".into());
    }
    verify_cid(ed_pub, &cid, sig_hex)?;

    Ok(RemoteEntry {
        ed_pub: ed_pub.to_string(),
        box_pub: str_at(actor, "boxPublicKey").unwrap_or("").to_string(),
        handle: handle_.to_string(),
        display_name: str_at(actor, "name").unwrap_or(handle_).to_string(),
        origin: str_at(actor, "origin").unwrap_or("").to_string(),
        origin_label: str_at(actor, "originLabel").unwrap_or("a node").to_string(),
        cid,
        visibility,
        license,
        signals,
        created_at,
        profile,
        signature: sig_hex.to_string(),
        received_at,
    })
}

/// Whether a commons accepts a verified share.
#[derive(Debug, PartialEq, Eq)]
pub enum Admission {
    /// Store it.
    Accept,
    /// Another key already holds this handle here (HTTP 409).
    HandleTaken,
    /// Older than (or the same age as, but different from) the version
    /// already held for this key: a replay or rollback (HTTP 409).
    Stale,
}

/// Decide whether to accept `incoming`, given who holds handles here.
///
/// - `local_holder`: the Ed25519 key of a member of this server with the same
///   handle, if any.
/// - `held`: every shared profile this server already holds.
///
/// A handle belongs to the first key that used it here (it is not part of
/// the pin where a share came from, because the sender writes that itself).
/// A key may re-share, but only a newer version: otherwise anyone holding an
/// old signed share could roll a profile back.
pub fn admit(incoming: &RemoteEntry, local_holder: Option<&str>, held: &[RemoteEntry]) -> Admission {
    if local_holder.is_some_and(|k| k != incoming.ed_pub) {
        return Admission::HandleTaken;
    }
    if held.iter().any(|r| r.handle == incoming.handle && r.ed_pub != incoming.ed_pub) {
        return Admission::HandleTaken;
    }
    let current = held.iter().find(|r| r.ed_pub == incoming.ed_pub);
    match current {
        Some(c) if c.cid == incoming.cid => Admission::Accept, // same version again: harmless
        Some(c) if incoming.created_at <= c.created_at => Admission::Stale,
        _ => Admission::Accept,
    }
}

/// True when `handle` is held here by someone other than `except_key`.
/// `local_holder` and `held` are as for [`admit`].
pub fn handle_taken(handle: &str, except_key: Option<&str>, local_holder: Option<&str>, held: &[RemoteEntry]) -> bool {
    let other = |k: &str| except_key != Some(k);
    local_holder.is_some_and(other) || held.iter().any(|r| r.handle == handle && other(&r.ed_pub))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;

    /// A signed share for `handle` at time `t`, plus the sender's keys.
    fn signed_share(handle: &str, t: u64, bio: &str) -> (Value, keys::NewKeys) {
        let k = keys::generate();
        let profile = Profile { name: handle.into(), bio: bio.into(), ..Default::default() };
        let cid = content_id(handle, "public", "cc-by", &[], t, &hash_body(&profile));
        let sig = keys::sign_cid(&k.ed_priv, &cid).unwrap();
        let rec = HandleRec {
            handle: handle.into(),
            display_name: handle.into(),
            created_at: 0,
            ed_pub: k.ed_pub.clone(),
            ed_priv: k.ed_priv.clone(),
            box_pub: k.box_pub.clone(),
            box_priv: k.box_priv.clone(),
            secret_salt: String::new(),
            secret_hash: String::new(),
            current_cid: Some(cid.clone()),
            shared_to: vec![],
        };
        let v = Version {
            cid,
            handle: handle.into(),
            visibility: "public".into(),
            license: "cc-by".into(),
            signals: vec![],
            created_at: t,
            signature: sig,
            payload: Some(profile),
            enc: None,
        };
        (build_share(&Sender { base_url: "http://n", instance: "n" }, &rec, &v), k)
    }

    #[test]
    fn a_valid_share_verifies() {
        let (a, k) = signed_share("tega", 10, "hi");
        let e = verify_share(&a, 99).unwrap();
        assert_eq!(e.handle, "tega");
        assert_eq!(e.ed_pub, k.ed_pub);
        assert_eq!(e.received_at, 99);
    }

    #[test]
    fn tampered_content_is_rejected() {
        let (mut a, _) = signed_share("tega", 10, "hi");
        a["object"]["profile"]["bio"] = json!("changed");
        assert_eq!(verify_share(&a, 0).unwrap_err(), "content hash mismatch");
    }

    #[test]
    fn members_only_is_rejected() {
        let (mut a, _) = signed_share("tega", 10, "hi");
        a["object"]["visibility"] = json!("members");
        assert!(verify_share(&a, 0).unwrap_err().contains("members-only"));
    }

    #[test]
    fn same_handle_other_key_is_refused() {
        let (a, _) = signed_share("tega", 10, "real");
        let (b, _) = signed_share("tega", 11, "impostor");
        let held = vec![verify_share(&a, 0).unwrap()];
        assert_eq!(admit(&verify_share(&b, 0).unwrap(), None, &held), Admission::HandleTaken);
    }

    #[test]
    fn local_member_keeps_their_handle() {
        let (a, _) = signed_share("tega", 10, "x");
        assert_eq!(admit(&verify_share(&a, 0).unwrap(), Some("someone-else"), &[]), Admission::HandleTaken);
    }

    #[test]
    fn older_version_from_same_key_is_stale() {
        let (new, _) = signed_share("tega", 20, "new");
        let held = vec![verify_share(&new, 0).unwrap()];
        let mut old = held[0].clone();
        old.created_at = 10;
        old.cid = "older".into();
        assert_eq!(admit(&old, None, &held), Admission::Stale);
        assert_eq!(admit(&held[0], None, &held), Admission::Accept, "re-sending the same version is fine");
    }

    #[test]
    fn handle_taken_ignores_the_owner() {
        let (a, k) = signed_share("tega", 10, "x");
        let held = vec![verify_share(&a, 0).unwrap()];
        assert!(handle_taken("tega", None, None, &held));
        assert!(!handle_taken("tega", Some(&k.ed_pub), None, &held));
        assert!(!handle_taken("mo", None, None, &held));
    }
}
