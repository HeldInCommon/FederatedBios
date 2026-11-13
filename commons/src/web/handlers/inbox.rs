//! `POST /inbox`: what a commons receives from nodes. A share (listed,
//! refused, or put on the waiting list), a vouch, a withdrawal, an identity
//! change (recovery key, key rotation), or a story and its withdrawal.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Extension;

use crate::app::{AppState, Shared};
use crate::envelope::Signed;
use crate::federation::{self, Admission, Holders};
use crate::here;
use crate::identity::{self, Identity};
use crate::model::{PendingShare, RemoteEntry};
use crate::story::{self, HeldStory, StoryAdmission};
use crate::vouching::{self, Intro};
use crate::{keys, now};

/// Plain-text reply for server-to-server endpoints.
fn reply(status: StatusCode, msg: &str) -> Response {
    (status, msg.to_string()).into_response()
}

/// Log a storage failure and answer 500.
fn store_failed(e: &dyn std::fmt::Display) -> Response {
    eprintln!("error: {e}");
    reply(StatusCode::INTERNAL_SERVER_ERROR, "could not read or write the data folder")
}

/// `POST /inbox` — receive a share, a withdrawal, or an identity change
/// (recovery key, key rotation) from another node.
pub async fn inbox(
    State(st): State<Shared>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    body: String,
) -> Response {
    // Checked before any parsing or signature work, so a flood costs little.
    let sender = peer.map(|Extension(ConnectInfo(a))| a.ip().to_string()).unwrap_or_else(|| "unknown".into());
    if !st.inbox_limit.record_if_under_limit(&sender, now()) {
        return reply(StatusCode::TOO_MANY_REQUESTS, "too many requests from this address; try again in a minute");
    }
    let Ok(activity) = serde_json::from_str::<serde_json::Value>(&body) else {
        return reply(StatusCode::BAD_REQUEST, "invalid json");
    };
    match activity.get("type").and_then(|t| t.as_str()) {
        Some("Withdraw") => return receive_withdraw(&st, &activity),
        Some("StoryShare") => return receive_story(&st, &activity),
        Some("StoryWithdraw") => return receive_story_withdraw(&st, &activity),
        _ => {}
    }
    if activity.get("envelope").is_some() {
        return receive_identity_change(&st, &activity);
    }
    let entry = match federation::verify_any_share(&activity, now()) {
        Ok(e) => e,
        Err(e) => return reply(StatusCode::BAD_REQUEST, &format!("rejected: {e}")),
    };
    let _g = st.write.lock().unwrap();
    let local = match st.store.handle(&entry.handle) {
        Ok(r) => r.map(|r| r.ed_pub),
        Err(e) => return store_failed(&e),
    };
    let existing = match st.store.identity(&entry.handle) {
        Ok(i) => i,
        Err(e) => return store_failed(&e),
    };
    let (held, withdrawn) = (st.store.remotes(), st.store.tombstones());
    let holders = Holders {
        local: local.as_deref(),
        held: &held,
        withdrawn: &withdrawn,
        claimed: existing.as_ref().map(|i| i.key.as_str()),
    };
    match federation::admit(&entry, &holders) {
        Admission::HandleTaken => {
            reply(StatusCode::CONFLICT, "that handle is already used on this commons by a different key")
        }
        Admission::Stale => reply(
            StatusCode::CONFLICT,
            "this commons already holds a newer version from this key, or a later withdrawal",
        ),
        Admission::Accept => {
            let returning = existing.is_some()
                || held.iter().any(|r| r.ed_pub == entry.ed_pub)
                || withdrawn.iter().any(|t| t.ed_pub == entry.ed_pub);
            if returning || st.config.open_commons {
                return list_entry(&st, &entry, &activity, existing, None);
            }
            admit_newcomer(&st, &entry, &activity, &held, &sender)
        }
    }
}

/// A new handle on a commons that needs vouching (decision record 0011):
/// list it with a valid invite, the founding invite or (on a local commons
/// open to the room) the room code; otherwise put it on the pending list
/// until a member vouches for it. `sender` is the address the share came
/// from, which wrong room codes are counted by.
fn admit_newcomer(
    st: &AppState,
    entry: &RemoteEntry,
    message: &serde_json::Value,
    held: &[RemoteEntry],
    sender: &str,
) -> Response {
    let code = message.get("invite").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if code.is_empty() {
        let pending = PendingShare { entry: entry.clone(), message: message.clone() };
        return match st.store.save_pending(&pending) {
            Ok(()) => {
                reply(StatusCode::ACCEPTED, "pending: this commons lists newcomers once a member vouches for them")
            }
            Err(e) => store_failed(&e),
        };
    }
    let intro = |route: &str, voucher: Option<String>, proof: Option<Signed>| Intro {
        handle: entry.handle.clone(),
        key: entry.ed_pub.clone(),
        route: route.into(),
        voucher,
        at: now(),
        proof,
    };
    // Decision record 0027 (D4): on a local commons open to the room, the
    // code on the host's screen admits like an invite. Wrong guesses are
    // limited per address, as passphrases are (decision record 0005): five,
    // then fifteen minutes' wait.
    if st.config.room_code.is_some() && vouching::looks_like_room_code(&code) {
        use std::sync::atomic::Ordering;
        if st.room_code_wrong.load(Ordering::SeqCst) >= crate::app::ROOM_CODE_MAX_WRONG {
            return reply(
                StatusCode::TOO_MANY_REQUESTS,
                "too many wrong room codes have been tried here; ask the host to close the room and open it again",
            );
        }
        // Only a signed hic/v1 share: in the older format some of what the
        // commons shows isn't signed, and everyone in the room knows the code.
        if entry.signed.is_none() {
            return reply(StatusCode::FORBIDDEN, "update the app to join with a room code");
        }
        if !st.room_code_attempts.allowed(sender, now()) {
            return reply(
                StatusCode::TOO_MANY_REQUESTS,
                "too many wrong room codes from this computer; try again later",
            );
        }
        if !vouching::room_code_matches(&code, st.config.room_code.as_deref()) {
            st.room_code_attempts.failed(sender, now());
            st.room_code_wrong.fetch_add(1, Ordering::SeqCst);
            return reply(StatusCode::FORBIDDEN, "that room code isn't right; check it on the host's screen");
        }
        st.room_code_attempts.succeeded(sender);
        return list_entry(st, entry, message, None, Some(intro("room-code", None, None)));
    }
    if code.starts_with(vouching::FOUNDING_PREFIX) {
        let stored = match st.store.founding_code() {
            Ok(c) => c,
            Err(e) => return store_failed(&e),
        };
        if !vouching::founding_matches(&code, stored.as_deref()) {
            return reply(
                StatusCode::FORBIDDEN,
                "that founding invite isn't valid here (it may already have been used)",
            );
        }
        if let Err(e) = st.store.use_founding_code() {
            return store_failed(&e);
        }
        return list_entry(st, entry, message, None, Some(intro("founding", None, None)));
    }
    if code.starts_with(vouching::STEWARD_INVITE_PREFIX) {
        let unused = match st.store.commons_invite_unused(&code) {
            Ok(u) => u,
            Err(e) => return store_failed(&e),
        };
        if !unused {
            return reply(StatusCode::FORBIDDEN, "that invite code isn't valid here (it may already have been used)");
        }
        if let Err(e) = st.store.use_commons_invite(&code, now()) {
            return store_failed(&e);
        }
        return list_entry(st, entry, message, None, Some(intro("steward-invite", None, None)));
    }
    let Some(invite) = vouching::code_to_invite(&code) else {
        return reply(StatusCode::FORBIDDEN, "that isn't an invite code");
    };
    let (members, used, intros) = (member_keys(held), st.store.used_codes(), st.store.intros());
    let ctx = vouch_ctx(st, &members, &used, &intros);
    match vouching::check_invite(&invite, &ctx) {
        Ok((voucher, code_id)) => {
            if let Err(e) = st.store.mark_code_used(&code_id, now()) {
                return store_failed(&e);
            }
            list_entry(st, entry, message, None, Some(intro("invite", Some(voucher), Some(invite))))
        }
        Err(e) => reply(StatusCode::FORBIDDEN, &e),
    }
}

/// The keys of everyone listed on this commons: who may vouch.
fn member_keys(held: &[RemoteEntry]) -> Vec<String> {
    held.iter().map(|r| r.ed_pub.clone()).collect()
}

/// What the vouching rules need to know about this commons.
fn vouch_ctx<'a>(
    st: &'a AppState,
    members: &'a [String],
    used: &'a [String],
    intros: &'a [Intro],
) -> vouching::Ctx<'a> {
    vouching::Ctx {
        my_hosts: &st.own_hosts,
        now: now(),
        member_keys: members,
        used_codes: used,
        intros,
        limit: st.config.vouch_limit,
    }
}

/// Plan and write a listing (see [`list_entry`]); for callers that answer
/// with a page rather than to a node. **Writes the listing.**
pub(crate) fn list_entry_response(
    st: &AppState,
    entry: &RemoteEntry,
    message: &serde_json::Value,
    intro: Option<Intro>,
) -> Result<(), String> {
    let existing = st.store.identity(&entry.handle).map_err(|e| e.to_string())?;
    let plan = federation::plan_listing(entry, message, existing, intro, now());
    st.store.apply_listing(&plan).map_err(|e| e.to_string())
}

/// List a share: plan it (pure), then write it with
/// [`crate::store::Store::apply_listing`], and answer the sender.
fn list_entry(
    st: &AppState,
    entry: &RemoteEntry,
    message: &serde_json::Value,
    existing: Option<Identity>,
    intro: Option<Intro>,
) -> Response {
    let plan = federation::plan_listing(entry, message, existing, intro, now());
    match st.store.apply_listing(&plan) {
        Ok(()) => reply(StatusCode::ACCEPTED, "accepted"),
        Err(e) => store_failed(&e),
    }
}

/// A member vouches for someone on the pending list.
fn receive_vouch(st: &AppState, vouch: Signed) -> Response {
    let body = &vouch.envelope.body;
    let (Some(h), Some(key)) = (body.get("handle").and_then(|v| v.as_str()), body.get("key").and_then(|v| v.as_str()))
    else {
        return reply(StatusCode::BAD_REQUEST, "rejected: a vouch names a handle and a key");
    };
    let _g = st.write.lock().unwrap();
    let Some(waiting) = st.store.pending().into_iter().find(|p| p.entry.handle == h && p.entry.ed_pub == key) else {
        return reply(StatusCode::NOT_FOUND, "nobody by that handle and key is waiting here");
    };
    let held = st.store.remotes();
    let (members, used, intros) = (member_keys(&held), st.store.used_codes(), st.store.intros());
    let voucher = match vouching::check_vouch(&vouch, h, key, &vouch_ctx(st, &members, &used, &intros)) {
        Ok(v) => v,
        Err(e) => return reply(StatusCode::FORBIDDEN, &e),
    };
    // Someone else may have taken the handle while this share waited.
    let withdrawn = st.store.tombstones();
    let claimed = st.store.identity(h).ok().flatten().map(|i| i.key);
    let holders = Holders { local: None, held: &held, withdrawn: &withdrawn, claimed: claimed.as_deref() };
    if federation::admit(&waiting.entry, &holders) != Admission::Accept {
        let _ = st.store.delete_pending(key);
        return reply(StatusCode::CONFLICT, "that handle was taken while this share was waiting");
    }
    let intro = Intro {
        handle: h.into(),
        key: key.into(),
        route: "vouch".into(),
        voucher: Some(voucher),
        at: now(),
        proof: Some(vouch.clone()),
    };
    list_entry(st, &waiting.entry, &waiting.message, None, Some(intro))
}

/// Apply a signed identity change (a `RecoveryKey`, `KeyRotation` or
/// `RecoveryRotation`, see [`crate::identity`]) to the handle's record, and
/// move what this server holds to the new key.
fn receive_identity_change(st: &AppState, activity: &serde_json::Value) -> Response {
    let Ok(change) = serde_json::from_value::<Signed>(activity.clone()) else {
        return reply(StatusCode::BAD_REQUEST, "rejected: not a signed object");
    };
    if change.envelope.kind == vouching::VOUCH {
        return receive_vouch(st, change);
    }
    let Some(h) = change.envelope.body.get("handle").and_then(|v| v.as_str()).map(String::from) else {
        return reply(StatusCode::BAD_REQUEST, "rejected: missing handle");
    };
    let _g = st.write.lock().unwrap();
    // Profiles shared before identity records existed get one on first use.
    let current = match st.store.identity(&h) {
        Ok(Some(i)) => i,
        Ok(None) => match st.store.remotes().into_iter().find(|r| r.handle == h) {
            Some(r) => Identity::new(&h, &r.ed_pub, &r.box_pub),
            None => return reply(StatusCode::NOT_FOUND, "this commons doesn't hold that handle"),
        },
        Err(e) => return store_failed(&e),
    };
    let next = match identity::apply(&current, &change, now()) {
        Ok(n) => n,
        Err(e) => return reply(StatusCode::CONFLICT, &format!("rejected: {e}")),
    };
    if next.key != current.key {
        if let Err(e) = st.store.rekey(&current.key, &next.key, &next.box_key) {
            return store_failed(&e);
        }
    }
    match st.store.save_identity(&next) {
        Ok(()) => reply(StatusCode::OK, "applied"),
        Err(e) => store_failed(&e),
    }
}

/// Handle a signed withdrawal: remove the key's shared profile and record a
/// tombstone (so the handle stays theirs and the old copy can't be replayed).
fn receive_withdraw(st: &AppState, activity: &serde_json::Value) -> Response {
    let w = match federation::verify_withdraw(activity, &st.own_hosts) {
        Ok(w) => w,
        Err(e) => return reply(StatusCode::BAD_REQUEST, &format!("rejected: {e}")),
    };
    let _g = st.write.lock().unwrap();
    let tomb = match federation::apply_withdraw(&w, &st.store.remotes(), &st.store.tombstones()) {
        Ok(t) => t,
        Err(e) => return reply(StatusCode::CONFLICT, e),
    };
    // Someone who leaves takes their stories with them.
    let removed = st
        .store
        .save_tombstone(&tomb)
        .and_then(|_| st.store.delete_remote(&keys::keyid(&w.ed_pub)))
        .and_then(|_| st.store.delete_stories_by(&w.ed_pub));
    if let Err(e) = removed {
        return store_failed(&e);
    }
    reply(StatusCode::OK, "withdrawn")
}

/// A story shared to this commons: checked, then kept if its author is
/// listed here under its handle (see [`story::admit`]).
fn receive_story(st: &AppState, activity: &serde_json::Value) -> Response {
    if !st.site.is_commons() {
        return reply(StatusCode::NOT_FOUND, "this is a node, not a commons");
    }
    let (s, signed) = match story::verify_share(activity) {
        Ok(v) => v,
        Err(e) => return reply(StatusCode::BAD_REQUEST, &format!("rejected: {e}")),
    };
    let _g = st.write.lock().unwrap();
    let held = st.store.held_stories();
    // An "Only while I'm here" card as new or newer wins (decision record 0021).
    let author = signed.envelope.author.clone();
    let created_at = signed.envelope.created_at;
    let admission = match story::admit(&s, &signed, &st.store.remotes(), &held, &st.store.story_tombstones()) {
        StoryAdmission::Accept if here::card_blocks_copy(&st.store.story_cards(), &author, &s.id, created_at) => {
            StoryAdmission::Stale
        }
        other => other,
    };
    match admission {
        StoryAdmission::NotListed => reply(
            StatusCode::FORBIDDEN,
            "stories can be shared here once you're listed: share your profile to this commons first",
        ),
        StoryAdmission::Stale => {
            reply(StatusCode::CONFLICT, "this commons already holds a newer version of that story, or it was withdrawn")
        }
        StoryAdmission::Accept => {
            let id = s.id.clone();
            // Keep the copy, then remove any older card for the same story
            // (the author switched from "Only while I'm here" to a copy).
            let saved = st
                .store
                .save_held_story(&HeldStory { story: s, signed, received_at: now() })
                .and_then(|_| st.store.delete_story_card(&author, &id));
            st.relay.remove_story(&author, &id);
            match saved {
                Ok(()) => reply(StatusCode::OK, "story shared"),
                Err(e) => store_failed(&e),
            }
        }
    }
}

/// A story's withdrawal from this commons: of its copy, or of its "Only
/// while I'm here" card (decision record 0021). A card's withdrawal takes
/// effect at once: the card is deleted, and readers waiting for its text
/// are told it's gone.
fn receive_story_withdraw(st: &AppState, activity: &serde_json::Value) -> Response {
    let w = match story::verify_withdraw(activity, &st.own_hosts) {
        Ok(w) => w,
        Err(e) => return reply(StatusCode::BAD_REQUEST, &format!("rejected: {e}")),
    };
    let _g = st.write.lock().unwrap();
    let card = st.store.story_cards().into_iter().find(|c| c.author() == w.author && c.card.id == w.id);
    if let Some(card) = card {
        let tomb = match here::apply_card_withdraw(&w, &card) {
            Ok(t) => t,
            Err(e) => return reply(StatusCode::CONFLICT, e),
        };
        let removed = st.store.save_story_tombstone(&tomb).and_then(|_| st.store.delete_story_card(&w.author, &w.id));
        st.relay.remove_story(&w.author, &w.id);
        return match removed {
            Ok(()) => reply(StatusCode::OK, "story withdrawn"),
            Err(e) => store_failed(&e),
        };
    }
    let tomb = match story::apply_withdraw(&w, &st.store.held_stories(), &st.store.story_tombstones()) {
        Ok(t) => t,
        Err(e) => return reply(StatusCode::CONFLICT, e),
    };
    let removed = st.store.save_story_tombstone(&tomb).and_then(|_| st.store.delete_held_story(&w.author, &w.id));
    match removed {
        Ok(()) => reply(StatusCode::OK, "story withdrawn"),
        Err(e) => store_failed(&e),
    }
}
