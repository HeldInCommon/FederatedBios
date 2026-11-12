//! Joining a commons needs a vouch (decision record 0011).
//!
//! A new handle is listed on a commons only when someone already there vouches
//! for it, in one of three ways:
//!
//! | Route | What the newcomer has | Checked against |
//! |---|---|---|
//! | **Invite code** | a signed `Invite` a member's node made in advance | the inviter is a member; the code is for this commons, unexpired, unused |
//! | **Pending list** | nothing: their share waits until a member sends a signed `Vouch` | the voucher is a member; the vouch names this handle and key |
//! | **Founding invite** | the one-time code the commons made when it started | the stored code (for the first person, when there are no members) |
//! | **Steward invite** | a code the commons' steward made (`hic-invite-…`) | the stored, unused code |
//! | **Steward approval** | nothing: the steward approves them from the waiting list | the steward's session |
//!
//! Each member can bring in at most `limit` people per week (set by the
//! steward, `HIC_VOUCH_LIMIT`). Every admission is recorded as an [`Intro`].
//! People already on a commons, and people whose voucher later leaves, stay.
//!
//! Members can also **see who is waiting** from their own node, with a
//! signed `WaitingListRequest` (decision record 0026): [`make_waiting_request`]
//! and [`check_waiting_request`].
//!
//! The checks are pure; the inbox handler does the storage.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::envelope::{self, Envelope, Signed};
use crate::keys::ct_eq;
use crate::web::layout::host_label;

/// The envelope `type` of a member's signed invite.
pub const INVITE: &str = "Invite";
/// The envelope `type` of a member's signed vouch for someone waiting.
pub const VOUCH: &str = "Vouch";
/// The envelope `type` a member's node signs to ask a commons who is waiting
/// for a vouch there (decision record 0026). It reads; it changes nothing.
pub const WAITING_LIST_REQUEST: &str = "WaitingListRequest";

/// How far a waiting-list request's `created_at` may be from the commons'
/// clock, either way, in seconds (decision record 0026). Short, so a request
/// copied off the network soon stops working; not so short that two
/// computers whose clocks differ by a minute or two can't talk.
pub const REQUEST_FRESH_FOR: u64 = 5 * 60;

/// How long an invite code stays valid (14 days).
pub const INVITE_TTL: u64 = 14 * 86_400;
/// The period the vouching limit counts over (a week).
pub const WEEK: u64 = 7 * 86_400;
/// The default limit: people one member can bring in per week. Raised from 5
/// to 100 on 2026-11-11 so a workshop room can fill quickly (see ADR 0011).
pub const DEFAULT_LIMIT: u32 = 100;
/// Founding invites start with this, so they're easy to tell from member invites.
pub const FOUNDING_PREFIX: &str = "hic-founding-";
/// Invites made by a commons' steward start with this.
pub const STEWARD_INVITE_PREFIX: &str = "hic-invite-";

/// How someone came to be listed on a commons. Lives in `intros/<keyid>.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Intro {
    pub handle: String,
    /// Their Ed25519 public key, hex.
    pub key: String,
    /// `"invite"`, `"vouch"` or `"founding"`.
    pub route: String,
    /// The member who vouched (their key), or `None` for the founding invite.
    pub voucher: Option<String>,
    /// When they were listed, in Unix seconds.
    pub at: u64,
    /// The signed invite or vouch, kept as the record.
    pub proof: Option<Signed>,
}

/// What a commons knows when checking a vouch.
pub struct Ctx<'a> {
    /// The hosts this commons counts as itself (`Config::own_hosts`).
    pub my_hosts: &'a [String],
    /// The time to check against, in Unix seconds.
    pub now: u64,
    /// Keys of current members (people with a profile listed here).
    pub member_keys: &'a [String],
    /// Invite codes already used.
    pub used_codes: &'a [String],
    /// Earlier admissions (for the weekly limit).
    pub intros: &'a [Intro],
    /// People one member may bring in per week.
    pub limit: u32,
}

impl Ctx<'_> {
    /// True when `voucher` has brought in fewer than the weekly limit of people
    /// in the last seven days.
    fn under_limit(&self, voucher: &str) -> bool {
        let recent =
            self.intros.iter().filter(|i| i.voucher.as_deref() == Some(voucher) && i.at + WEEK > self.now).count();
        (recent as u32) < self.limit
    }

    /// The checks every invite and vouch share: signed, the right type, for
    /// this commons, by a member who hasn't reached the limit.
    fn check_common(&self, s: &Signed, kind: &str) -> Result<String, String> {
        let voucher = check_signed_by_member(s, kind, self.my_hosts, self.member_keys)?;
        if !self.under_limit(&voucher) {
            return Err("that member has vouched for as many people as this commons allows this week".into());
        }
        Ok(voucher)
    }
}

/// The checks shared by everything a member signs for a commons: the
/// signature holds, the object is of type `kind`, it names one of this
/// commons' hosts, and its author is listed here. Returns the author's key.
///
/// Used by invites and vouches (through [`Ctx`]) and by waiting-list
/// requests ([`check_waiting_request`]), so all three agree on who counts as
/// a member. Pure: it reads only what it is given.
fn check_signed_by_member(
    s: &Signed,
    kind: &str,
    my_hosts: &[String],
    member_keys: &[String],
) -> Result<String, String> {
    // The signature covers the type as well as the content (`envelope.rs`),
    // so a vouch can't be passed off as a waiting-list request, or the
    // other way round.
    envelope::verify(s)?;
    if s.envelope.kind != kind {
        return Err(format!("expected {kind}"));
    }
    // Naming the commons stops something signed for one commons being
    // replayed at another where the same person is also a member.
    let commons = s.envelope.body.get("commons").and_then(|v| v.as_str()).unwrap_or("");
    if !my_hosts.iter().any(|h| h == commons) {
        return Err(format!("this {} is for a different commons", kind_in_words(kind)));
    }
    let author = s.envelope.author.clone();
    if !member_keys.contains(&author) {
        return Err("whoever made this isn't a member of this commons".into());
    }
    Ok(author)
}

/// A signed object's type as words for a refusal: "invite", "vouch" or
/// "waiting-list request".
fn kind_in_words(kind: &str) -> String {
    match kind {
        WAITING_LIST_REQUEST => "waiting-list request".into(),
        other => other.to_lowercase(),
    }
}

/// A member's signed request to see who is waiting for a vouch on a commons
/// (decision record 0026). The member's node makes a fresh one each time the
/// People tab is opened. It proves the request comes from whoever holds a
/// listed member's key, without any new secret to keep.
pub fn make_waiting_request(
    member_pub: &str,
    member_priv: &str,
    commons_url: &str,
    now: u64,
) -> Result<Signed, String> {
    let body = json!({ "commons": host_label(commons_url) });
    envelope::sign(Envelope::new(WAITING_LIST_REQUEST, member_pub, now, None, body), member_priv)
}

/// Check a waiting-list request (decision record 0026). It must be signed by
/// a key listed on this commons, name this commons, and be fresh: made within
/// [`REQUEST_FRESH_FOR`] seconds of `now`, either way. Returns the member's
/// key.
///
/// The weekly vouching limit doesn't apply here: looking is not vouching.
pub fn check_waiting_request(
    s: &Signed,
    my_hosts: &[String],
    member_keys: &[String],
    now: u64,
) -> Result<String, String> {
    let member = check_signed_by_member(s, WAITING_LIST_REQUEST, my_hosts, member_keys)?;
    // A request is a one-off proof, not a pass: refusing old ones means a
    // copy picked up on the network (over plain http, say) soon stops
    // working. `abs_diff` is the distance between two times, whichever is
    // later, without the subtraction going below zero.
    if s.envelope.created_at.abs_diff(now) > REQUEST_FRESH_FOR {
        return Err("this request is too old, or the two computers' clocks differ by more than five minutes".into());
    }
    Ok(member)
}

/// A member's invite to this commons, made in advance.
pub fn make_invite(member_pub: &str, member_priv: &str, commons_url: &str, now: u64) -> Result<Signed, String> {
    let body = json!({
        "commons": host_label(commons_url),
        "code_id": crate::sessions::random_hex(16),
        "expires_at": now + INVITE_TTL,
    });
    envelope::sign(Envelope::new(INVITE, member_pub, now, None, body), member_priv)
}

/// A member's vouch for someone waiting on the pending list.
pub fn make_vouch(
    member_pub: &str,
    member_priv: &str,
    commons_url: &str,
    handle: &str,
    newcomer_key: &str,
    now: u64,
) -> Result<Signed, String> {
    let body = json!({ "commons": host_label(commons_url), "handle": handle, "key": newcomer_key });
    envelope::sign(Envelope::new(VOUCH, member_pub, now, None, body), member_priv)
}

/// An invite as text to hand to someone (URL-safe base64 of the signed JSON).
pub fn invite_to_code(inv: &Signed) -> String {
    B64URL.encode(serde_json::to_vec(inv).expect("signed objects serialise"))
}

/// Read an invite code back. `None` if it isn't one.
pub fn code_to_invite(code: &str) -> Option<Signed> {
    let bytes = B64URL.decode(code.trim()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Check a member's invite. Returns `(voucher key, code id)` to record.
pub fn check_invite(inv: &Signed, ctx: &Ctx) -> Result<(String, String), String> {
    let voucher = ctx.check_common(inv, INVITE)?;
    let body = &inv.envelope.body;
    let code_id = body.get("code_id").and_then(|v| v.as_str()).ok_or("invite has no id")?.to_string();
    if ctx.used_codes.contains(&code_id) {
        return Err("this invite has already been used".into());
    }
    if body.get("expires_at").and_then(|v| v.as_u64()).unwrap_or(0) <= ctx.now {
        return Err("this invite has expired".into());
    }
    Ok((voucher, code_id))
}

/// Check a vouch for `handle` with `key` (someone on the pending list).
/// Returns the voucher's key.
pub fn check_vouch(v: &Signed, handle: &str, key: &str, ctx: &Ctx) -> Result<String, String> {
    let voucher = ctx.check_common(v, VOUCH)?;
    let body = &v.envelope.body;
    if body.get("handle").and_then(|x| x.as_str()) != Some(handle)
        || body.get("key").and_then(|x| x.as_str()) != Some(key)
    {
        return Err("this vouch is for someone else".into());
    }
    if voucher == key {
        return Err("you can't vouch for yourself".into());
    }
    Ok(voucher)
}

/// True when `given` is the (unused) founding invite.
pub fn founding_matches(given: &str, stored: Option<&str>) -> bool {
    stored.is_some_and(|s| ct_eq(given.trim(), s))
}

/// Should a commons make a founding invite now? Only a commons that has
/// never had one and lists nobody yet (an existing commons' members can
/// invite instead).
pub fn needs_founding_invite(ever_had_one: bool, lists_anyone: bool) -> bool {
    !ever_had_one && !lists_anyone
}

/// A fresh steward invite code (single use, made from the steward view).
pub fn new_steward_invite_code() -> String {
    format!("{STEWARD_INVITE_PREFIX}{}", crate::sessions::random_hex(12))
}

/// A fresh steward key: the secret in the steward link a commons prints when
/// it starts (see `docs/adr/0017-steward-approval.md`).
pub fn new_steward_key() -> String {
    crate::sessions::random_hex(16)
}

/// A fresh founding invite.
pub fn new_founding_code() -> String {
    format!("{FOUNDING_PREFIX}{}", crate::sessions::random_hex(12))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;

    /// The commons these tests run as.
    const URL: &str = "http://commons.example";

    /// The commons' view for a test: its members, used invite codes and intros,
    /// at time `now`, with a limit of two a week.
    ///
    /// Learner note: `Box::leak` gives up ownership of a value so it lives for
    /// the rest of the program and can be borrowed as `'static`. Fine in a test;
    /// in real code it would be a memory leak.
    fn ctx<'a>(members: &'a [String], used: &'a [String], intros: &'a [Intro], now: u64) -> Ctx<'a> {
        let hosts: &'static [String] = Box::leak(vec![host_label(URL)].into_boxed_slice());
        Ctx { my_hosts: hosts, now, member_keys: members, used_codes: used, intros, limit: 2 }
    }

    #[test]
    fn a_members_invite_admits() {
        let m = keys::generate();
        let inv = make_invite(&m.ed_pub, &m.ed_priv, URL, 100).unwrap();
        let back = code_to_invite(&invite_to_code(&inv)).unwrap();
        let members = vec![m.ed_pub.clone()];
        let (voucher, _) = check_invite(&back, &ctx(&members, &[], &[], 200)).unwrap();
        assert_eq!(voucher, m.ed_pub);
    }

    #[test]
    fn invites_are_refused_when_wrong() {
        let m = keys::generate();
        let inv = make_invite(&m.ed_pub, &m.ed_priv, URL, 100).unwrap();
        let members = vec![m.ed_pub.clone()];
        let code_id = inv.envelope.body["code_id"].as_str().unwrap().to_string();
        assert!(check_invite(&inv, &ctx(&[], &[], &[], 200)).unwrap_err().contains("isn't a member"));
        assert!(check_invite(&inv, &ctx(&members, &[code_id], &[], 200)).unwrap_err().contains("already been used"));
        assert!(check_invite(&inv, &ctx(&members, &[], &[], 100 + INVITE_TTL)).unwrap_err().contains("expired"));
        let elsewhere = make_invite(&m.ed_pub, &m.ed_priv, "http://other.example", 100).unwrap();
        assert!(check_invite(&elsewhere, &ctx(&members, &[], &[], 200)).unwrap_err().contains("different commons"));
        assert!(code_to_invite("not a code").is_none());
    }

    #[test]
    fn the_weekly_limit_counts_recent_intros() {
        let m = keys::generate();
        let members = vec![m.ed_pub.clone()];
        let intro = |at| Intro {
            handle: "x".into(),
            key: "k".into(),
            route: "invite".into(),
            voucher: Some(m.ed_pub.clone()),
            at,
            proof: None,
        };
        let two_recent = vec![intro(150), intro(160)];
        let inv = make_invite(&m.ed_pub, &m.ed_priv, URL, 100).unwrap();
        assert!(check_invite(&inv, &ctx(&members, &[], &two_recent, 200)).unwrap_err().contains("as many people"));
        let two_old = vec![intro(1), intro(2)];
        assert!(check_invite(&inv, &ctx(&members, &[], &two_old, WEEK + 10)).is_ok());
    }

    #[test]
    fn a_vouch_names_one_person() {
        let m = keys::generate();
        let newcomer = keys::generate();
        let members = vec![m.ed_pub.clone()];
        let v = make_vouch(&m.ed_pub, &m.ed_priv, URL, "newbie", &newcomer.ed_pub, 100).unwrap();
        assert!(check_vouch(&v, "newbie", &newcomer.ed_pub, &ctx(&members, &[], &[], 100)).is_ok());
        assert!(check_vouch(&v, "other", &newcomer.ed_pub, &ctx(&members, &[], &[], 100)).is_err());
        let own = make_vouch(&m.ed_pub, &m.ed_priv, URL, "me", &m.ed_pub, 100).unwrap();
        assert!(check_vouch(&own, "me", &m.ed_pub, &ctx(&members, &[], &[], 100)).unwrap_err().contains("yourself"));
    }

    #[test]
    fn a_listed_member_may_ask_who_is_waiting() {
        let m = keys::generate();
        let members = vec![m.ed_pub.clone()];
        let hosts = vec![host_label(URL)];
        let req = make_waiting_request(&m.ed_pub, &m.ed_priv, URL, 1_000).unwrap();
        assert_eq!(check_waiting_request(&req, &hosts, &members, 1_000).unwrap(), m.ed_pub);
        // A little clock difference either way is fine.
        assert!(check_waiting_request(&req, &hosts, &members, 1_000 + REQUEST_FRESH_FOR).is_ok());
        assert!(check_waiting_request(&req, &hosts, &members, 1_000 - REQUEST_FRESH_FOR).is_ok());
    }

    #[test]
    fn someone_not_listed_here_cannot_ask_who_is_waiting() {
        let (m, stranger) = (keys::generate(), keys::generate());
        let members = vec![m.ed_pub.clone()];
        let hosts = vec![host_label(URL)];
        let req = make_waiting_request(&stranger.ed_pub, &stranger.ed_priv, URL, 1_000).unwrap();
        assert!(check_waiting_request(&req, &hosts, &members, 1_000).unwrap_err().contains("isn't a member"));
    }

    #[test]
    fn a_waiting_list_request_for_another_commons_is_refused() {
        // A member of both commons: their request to one can't be replayed at the other.
        let m = keys::generate();
        let members = vec![m.ed_pub.clone()];
        let hosts = vec![host_label(URL)];
        let req = make_waiting_request(&m.ed_pub, &m.ed_priv, "http://other.example", 1_000).unwrap();
        let why = check_waiting_request(&req, &hosts, &members, 1_000).unwrap_err();
        assert!(why.contains("different commons"), "{why}");
    }

    #[test]
    fn an_old_or_future_dated_waiting_list_request_is_refused() {
        let m = keys::generate();
        let members = vec![m.ed_pub.clone()];
        let hosts = vec![host_label(URL)];
        let req = make_waiting_request(&m.ed_pub, &m.ed_priv, URL, 10_000).unwrap();
        let late = 10_000 + REQUEST_FRESH_FOR + 1;
        assert!(check_waiting_request(&req, &hosts, &members, late).unwrap_err().contains("too old"));
        assert!(check_waiting_request(&req, &hosts, &members, 10_000 - REQUEST_FRESH_FOR - 1).is_err());
    }

    #[test]
    fn a_forged_or_retyped_waiting_list_request_is_refused() {
        let (m, forger) = (keys::generate(), keys::generate());
        let members = vec![m.ed_pub.clone()];
        let hosts = vec![host_label(URL)];
        // Signed by someone else, but claiming the member as its author.
        let mut forged = make_waiting_request(&forger.ed_pub, &forger.ed_priv, URL, 1_000).unwrap();
        forged.envelope.author = m.ed_pub.clone();
        assert!(check_waiting_request(&forged, &hosts, &members, 1_000).is_err());
        // A member's vouch relabelled as a request fails its signature,
        // because the type is part of what was signed.
        let vouch = make_vouch(&m.ed_pub, &m.ed_priv, URL, "x", &forger.ed_pub, 1_000).unwrap();
        let mut retyped = vouch.clone();
        retyped.envelope.kind = WAITING_LIST_REQUEST.into();
        assert!(check_waiting_request(&retyped, &hosts, &members, 1_000).is_err());
        // And a vouch as it is is the wrong type.
        assert!(check_waiting_request(&vouch, &hosts, &members, 1_000).unwrap_err().contains("expected"));
    }

    #[test]
    fn founding_invite_only_for_a_brand_new_commons() {
        assert!(needs_founding_invite(false, false));
        assert!(!needs_founding_invite(true, false));
        assert!(!needs_founding_invite(false, true));
    }

    #[test]
    fn steward_codes_have_their_own_prefix() {
        assert!(new_steward_invite_code().starts_with(STEWARD_INVITE_PREFIX));
        assert_eq!(new_steward_key().len(), 32);
    }

    #[test]
    fn founding_code() {
        let c = new_founding_code();
        assert!(c.starts_with(FOUNDING_PREFIX));
        assert!(founding_matches(&format!(" {c} "), Some(&c)));
        assert!(!founding_matches("hic-founding-guess", Some(&c)));
        assert!(!founding_matches(&c, None));
    }
}
