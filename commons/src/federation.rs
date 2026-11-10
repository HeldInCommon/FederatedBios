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
use crate::keys::{self, verify_cid};
use crate::model::{HandleRec, Profile, Reach, RemoteEntry, Tombstone, Version};
use crate::web::layout::host_label;

/// Who is sending a share: this node's public address and name.
pub struct Sender<'a> {
    pub base_url: &'a str,
    pub instance: &'a str,
}

/// Build the share a node posts to a commons' `/inbox`.
pub fn build_share(sender: &Sender, rec: &HandleRec, v: &Version) -> Value {
    let profile = v.payload.clone().unwrap_or_default();
    let mut share = json!({
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
    });
    // The signed recovery-key declaration travels alongside, so a commons can
    // record it (see crate::identity). It is a separate signed object.
    if let Some(d) = &rec.recovery_decl {
        share["recovery"] = serde_json::to_value(d).expect("signed objects serialise");
    }
    share
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
        signed: None,
    })
}

// ---------------------------------------------------------------------------
// Profile shares in format hic/v1 (every field signed)
// ---------------------------------------------------------------------------

/// The `hic/v1` type for a shared profile.
pub const PROFILE: &str = "Profile";

/// Build a `hic/v1` share: a signed `Profile` envelope covering the profile
/// (picture included), display name, reach, licence, signals, encryption key,
/// and the node's address and name. Dated with the version's own time, so the
/// same version always gives the same object. The recovery-key declaration,
/// if any, travels alongside.
pub fn build_share_v1(sender: &Sender, rec: &HandleRec, v: &Version) -> Result<Value, String> {
    let body = json!({
        "handle": rec.handle,
        "display_name": rec.display_name,
        "visibility": v.visibility,
        "license": v.license,
        "signals": v.signals,
        "profile": v.payload.clone().unwrap_or_default(),
        "box_key": rec.box_pub,
        "origin": sender.base_url,
        "origin_label": sender.instance,
        "version_cid": v.cid,
    });
    let signed = crate::envelope::sign(
        crate::envelope::Envelope::new(PROFILE, &rec.ed_pub, v.created_at, None, body),
        &rec.ed_priv,
    )?;
    let mut share = json!({ "type": "Share", "schema": crate::envelope::SCHEMA, "profile": signed });
    if let Some(d) = &rec.recovery_decl {
        share["recovery"] = serde_json::to_value(d).expect("signed objects serialise");
    }
    Ok(share)
}

/// Check a `hic/v1` share and turn it into a [`RemoteEntry`].
pub fn verify_share_v1(msg: &Value, received_at: u64) -> Result<RemoteEntry, String> {
    let signed: crate::envelope::Signed =
        serde_json::from_value(msg.get("profile").cloned().ok_or("missing profile")?).map_err(|_| "bad profile")?;
    crate::envelope::verify(&signed)?;
    let env = &signed.envelope;
    if env.kind != PROFILE {
        return Err(format!("expected a Profile, got {:?}", env.kind));
    }
    let b = &env.body;
    let field = |k: &str| b.get(k).and_then(|x| x.as_str()).map(String::from).ok_or_else(|| format!("missing {k}"));
    let handle_ = field("handle")?;
    if !handle::is_valid(&handle_) {
        return Err("invalid handle".into());
    }
    let visibility = field("visibility")?;
    match Reach::parse(&visibility) {
        Some(Reach::Members) => return Err("members-only profiles are not federated".into()),
        None => return Err("unknown visibility".into()),
        Some(_) => {}
    }
    let profile: Profile =
        serde_json::from_value(b.get("profile").cloned().ok_or("missing profile")?).map_err(|_| "bad profile")?;
    let signals = b
        .get("signals")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok(RemoteEntry {
        ed_pub: env.author.clone(),
        box_pub: field("box_key")?,
        handle: handle_,
        display_name: field("display_name")?,
        origin: field("origin")?,
        origin_label: field("origin_label")?,
        cid: signed.cid.clone(),
        visibility,
        license: field("license")?,
        signals,
        created_at: env.created_at,
        profile,
        signature: signed.sig.clone(),
        received_at,
        signed: Some(signed.clone()),
    })
}

/// Read a received share in either format.
pub fn verify_any_share(msg: &Value, received_at: u64) -> Result<RemoteEntry, String> {
    if msg.get("type").and_then(|t| t.as_str()) == Some("Share") {
        verify_share_v1(msg, received_at)
    } else {
        verify_share(msg, received_at)
    }
}

/// Who already holds handles on this server.
pub struct Holders<'a> {
    /// The Ed25519 key of a member of this server with the handle in question.
    pub local: Option<&'a str>,
    /// Every shared profile this server holds.
    pub held: &'a [RemoteEntry],
    /// Every withdrawal recorded here.
    pub withdrawn: &'a [Tombstone],
    /// The key this server's identity record names for the handle, if any.
    pub claimed: Option<&'a str>,
}

/// Whether a commons accepts a verified share.
#[derive(Debug, PartialEq, Eq)]
pub enum Admission {
    /// Store it.
    Accept,
    /// Another key already holds this handle here (HTTP 409).
    HandleTaken,
    /// Not newer than what this server already holds, or than the owner's
    /// withdrawal: a replay or rollback (HTTP 409).
    Stale,
}

/// Decide whether to accept `incoming`.
///
/// A handle belongs to the first key that used it here, and stays theirs
/// after they withdraw. (Where a share came from isn't part of this, because
/// the sender writes that itself.) A key may re-share, but only a version
/// newer than the one held and newer than its last withdrawal: otherwise
/// anyone holding an old signed share could roll a profile back or undo a
/// withdrawal.
pub fn admit(incoming: &RemoteEntry, h: &Holders) -> Admission {
    if handle_taken(&incoming.handle, Some(&incoming.ed_pub), h) {
        return Admission::HandleTaken;
    }
    let withdrawn_at = h.withdrawn.iter().filter(|t| t.ed_pub == incoming.ed_pub).map(|t| t.withdrawn_at).max();
    if withdrawn_at.is_some_and(|w| incoming.created_at <= w) {
        return Admission::Stale;
    }
    match h.held.iter().find(|r| r.ed_pub == incoming.ed_pub) {
        Some(c) if c.cid == incoming.cid => Admission::Accept, // same version again: harmless
        Some(c) if incoming.created_at <= c.created_at => Admission::Stale,
        _ => Admission::Accept,
    }
}

/// True when `handle` is held here (by a member, a shared profile or a
/// withdrawal) by a key other than `except_key`.
pub fn handle_taken(handle: &str, except_key: Option<&str>, h: &Holders) -> bool {
    let other = |k: &str| except_key != Some(k);
    h.local.is_some_and(other)
        || h.claimed.is_some_and(other)
        || h.held.iter().any(|r| r.handle == handle && other(&r.ed_pub))
        || h.withdrawn.iter().any(|t| t.handle == handle && other(&t.ed_pub))
}

// ---------------------------------------------------------------------------
// Withdrawal
// ---------------------------------------------------------------------------

/// The exact bytes a withdrawal signature covers. The prefix keeps this
/// signature from being mistaken for any other kind; `audience` (the
/// commons' host) means a withdrawal sent to one commons can't be replayed
/// to another.
pub fn withdraw_message(handle: &str, audience: &str, created_at: u64) -> String {
    format!("hic/v1/withdraw\n{handle}\n{audience}\n{created_at}")
}

/// Build the signed withdrawal a node sends to a commons it shared to.
pub fn build_withdraw(rec: &HandleRec, commons_url: &str, created_at: u64) -> Result<Value, &'static str> {
    let audience = host_label(commons_url);
    let sig = keys::sign_message(&rec.ed_priv, withdraw_message(&rec.handle, &audience, created_at).as_bytes())?;
    Ok(json!({
        "type": "Withdraw",
        "actor": { "preferredUsername": rec.handle, "ed25519PublicKey": rec.ed_pub },
        "object": { "handle": rec.handle, "audience": audience, "createdAt": created_at },
        "signature": { "type": "ed25519", "value": sig },
    }))
}

/// A verified withdrawal request.
#[derive(Debug, PartialEq, Eq)]
pub struct Withdrawal {
    pub handle: String,
    pub ed_pub: String,
    pub created_at: u64,
}

/// Check a received withdrawal: well-formed, addressed to this server
/// (`my_base_url`), and signed by the key it names.
pub fn verify_withdraw(v: &Value, my_base_url: &str) -> Result<Withdrawal, String> {
    let actor = v.get("actor").ok_or("missing actor")?;
    let object = v.get("object").ok_or("missing object")?;
    let ed_pub = str_at(actor, "ed25519PublicKey").ok_or("missing key")?;
    let handle_ = str_at(object, "handle").ok_or("missing handle")?;
    if !handle::is_valid(handle_) || str_at(actor, "preferredUsername") != Some(handle_) {
        return Err("invalid handle".into());
    }
    let audience = str_at(object, "audience").ok_or("missing audience")?;
    if audience != host_label(my_base_url) {
        return Err("this withdrawal was addressed to a different commons".into());
    }
    let created_at = object.get("createdAt").and_then(|x| x.as_u64()).ok_or("missing createdAt")?;
    let sig = v.get("signature").and_then(|s| s.get("value")).and_then(|x| x.as_str()).ok_or("missing signature")?;
    keys::verify_message(ed_pub, withdraw_message(handle_, audience, created_at).as_bytes(), sig)?;
    Ok(Withdrawal { handle: handle_.into(), ed_pub: ed_pub.into(), created_at })
}

/// What a commons does with a verified withdrawal: the tombstone to record,
/// or why not. The caller then deletes the key's shared profile.
///
/// Only the key that currently holds the handle here may withdraw it. (A
/// thief with an old, rotated-away key must not be able to leave a tombstone
/// that blocks the real owner.) Withdrawing twice is harmless. An older
/// withdrawal can't remove a later re-share.
pub fn apply_withdraw(
    w: &Withdrawal,
    held: &[RemoteEntry],
    withdrawn: &[Tombstone],
) -> Result<Tombstone, &'static str> {
    let tomb = Tombstone { ed_pub: w.ed_pub.clone(), handle: w.handle.clone(), withdrawn_at: w.created_at };
    match held.iter().find(|r| r.ed_pub == w.ed_pub) {
        Some(r) if r.handle != w.handle => Err("that key holds a different handle here"),
        Some(r) if r.created_at > w.created_at => Err("a newer version was shared after this withdrawal"),
        Some(_) => Ok(tomb),
        None if withdrawn.iter().any(|t| t.ed_pub == w.ed_pub && t.handle == w.handle) => Ok(tomb),
        None => Err("nothing is shared here under that key"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;

    fn holders<'a>(local: Option<&'a str>, held: &'a [RemoteEntry], withdrawn: &'a [Tombstone]) -> Holders<'a> {
        Holders { local, held, withdrawn, claimed: None }
    }

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
            recovery_pub: None,
            recovery_decl: None,
            pending_rotation: None,
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
        assert_eq!(admit(&verify_share(&b, 0).unwrap(), &holders(None, &held, &[])), Admission::HandleTaken);
    }

    #[test]
    fn local_member_keeps_their_handle() {
        let (a, _) = signed_share("tega", 10, "x");
        assert_eq!(
            admit(&verify_share(&a, 0).unwrap(), &holders(Some("someone-else"), &[], &[])),
            Admission::HandleTaken
        );
    }

    #[test]
    fn older_version_from_same_key_is_stale() {
        let (new, _) = signed_share("tega", 20, "new");
        let held = vec![verify_share(&new, 0).unwrap()];
        let mut old = held[0].clone();
        old.created_at = 10;
        old.cid = "older".into();
        assert_eq!(admit(&old, &holders(None, &held, &[])), Admission::Stale);
        assert_eq!(
            admit(&held[0], &holders(None, &held, &[])),
            Admission::Accept,
            "re-sending the same version is fine"
        );
    }

    #[test]
    fn handle_taken_ignores_the_owner() {
        let (a, k) = signed_share("tega", 10, "x");
        let held = vec![verify_share(&a, 0).unwrap()];
        assert!(handle_taken("tega", None, &holders(None, &held, &[])));
        assert!(!handle_taken("tega", Some(&k.ed_pub), &holders(None, &held, &[])));
        assert!(!handle_taken("mo", None, &holders(None, &held, &[])));
    }

    /// A record with real keys, for withdrawal tests.
    fn rec_with_keys(handle: &str) -> HandleRec {
        let k = keys::generate();
        HandleRec {
            handle: handle.into(),
            display_name: handle.into(),
            created_at: 0,
            ed_pub: k.ed_pub,
            ed_priv: k.ed_priv,
            box_pub: k.box_pub,
            box_priv: k.box_priv,
            secret_salt: String::new(),
            secret_hash: String::new(),
            current_cid: None,
            shared_to: vec![],
            recovery_pub: None,
            recovery_decl: None,
            pending_rotation: None,
        }
    }

    #[test]
    fn withdrawal_round_trip() {
        let rec = rec_with_keys("tega");
        let w = build_withdraw(&rec, "https://c.example/", 50).unwrap();
        let v = verify_withdraw(&w, "https://c.example").unwrap();
        assert_eq!(v, Withdrawal { handle: "tega".into(), ed_pub: rec.ed_pub.clone(), created_at: 50 });
    }

    #[test]
    fn withdrawal_for_another_commons_is_refused() {
        let rec = rec_with_keys("tega");
        let w = build_withdraw(&rec, "https://other.example", 50).unwrap();
        assert!(verify_withdraw(&w, "https://c.example").unwrap_err().contains("different commons"));
    }

    #[test]
    fn tampered_withdrawal_is_refused() {
        let rec = rec_with_keys("tega");
        let mut w = build_withdraw(&rec, "https://c.example", 50).unwrap();
        w["object"]["createdAt"] = json!(99);
        assert!(verify_withdraw(&w, "https://c.example").is_err());
    }

    #[test]
    fn old_withdrawal_cannot_remove_a_later_reshare() {
        let (share, k) = signed_share("tega", 100, "back again");
        let held = vec![verify_share(&share, 0).unwrap()];
        let w = Withdrawal { handle: "tega".into(), ed_pub: k.ed_pub.clone(), created_at: 60 };
        assert!(apply_withdraw(&w, &held, &[]).is_err());
        let w = Withdrawal { created_at: 150, ..w };
        assert_eq!(apply_withdraw(&w, &held, &[]).unwrap().withdrawn_at, 150);
        assert!(
            apply_withdraw(&w, &[], &[]).unwrap_err().contains("nothing is shared"),
            "a key holding nothing can't leave a tombstone"
        );
    }

    #[test]
    fn withdrawn_versions_cannot_be_replayed_and_the_handle_stays_theirs() {
        let (share, k) = signed_share("tega", 100, "x");
        let entry = verify_share(&share, 0).unwrap();
        let tomb = vec![Tombstone { ed_pub: k.ed_pub.clone(), handle: "tega".into(), withdrawn_at: 120 }];
        assert_eq!(admit(&entry, &holders(None, &[], &tomb)), Admission::Stale);
        let (impostor, _) = signed_share("tega", 200, "mine now");
        assert_eq!(admit(&verify_share(&impostor, 0).unwrap(), &holders(None, &[], &tomb)), Admission::HandleTaken);
        let mut newer = entry.clone();
        newer.created_at = 130;
        newer.cid = "newer".into();
        assert_eq!(admit(&newer, &holders(None, &[], &tomb)), Admission::Accept, "owner may share again later");
    }

    fn published(handle: &str, avatar: &str) -> (HandleRec, Version) {
        let rec = rec_with_keys(handle);
        let draft = crate::publishing::Draft {
            profile: Profile { name: "Tega".into(), avatar: avatar.into(), ..Default::default() },
            reach: Reach::Public,
            license: "cc-by".into(),
            signals: vec!["credit".into()],
            created_at: 10,
        };
        let v = crate::publishing::sign_version(&rec, draft, &[]).unwrap();
        (rec, v)
    }

    #[test]
    fn v1_share_round_trip_signs_every_field() {
        let (rec, v) = published("tega", "data:image/png;base64,AAAA");
        let msg = build_share_v1(&Sender { base_url: "http://n", instance: "Tega's node" }, &rec, &v).unwrap();
        let e = verify_any_share(&msg, 5).unwrap();
        assert_eq!(e.profile.avatar, "data:image/png;base64,AAAA");
        assert_eq!(e.origin_label, "Tega's node");
        assert!(e.signed.is_some());
        // Changing anything that used to travel unsigned now breaks the signature.
        let mut m = msg.clone();
        m["profile"]["envelope"]["body"]["profile"]["avatar"] = json!("data:image/png;base64,BBBB");
        assert!(verify_any_share(&m, 5).is_err(), "the picture is signed");
        for key in ["display_name", "origin", "origin_label", "box_key"] {
            let mut m = msg.clone();
            m["profile"]["envelope"]["body"][key] = json!("changed");
            assert!(verify_any_share(&m, 5).is_err(), "{key} should be signed");
        }
    }

    #[test]
    fn v1_share_is_deterministic_for_one_version() {
        let (rec, v) = published("tega", "");
        let s = Sender { base_url: "http://n", instance: "n" };
        assert_eq!(build_share_v1(&s, &rec, &v).unwrap(), build_share_v1(&s, &rec, &v).unwrap());
    }

    #[test]
    fn old_format_still_accepted() {
        let (a, _) = signed_share("tega", 10, "hi");
        assert!(verify_any_share(&a, 0).unwrap().signed.is_none());
    }
}
