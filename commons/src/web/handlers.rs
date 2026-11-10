//! Request handlers for the HTML side of the server.
//!
//! Each handler does the same three things: load what it needs (store,
//! session), call pure functions for every decision, then render. Writes that
//! read a record, change it and save it back happen while holding
//! [`AppState::write`], so two requests can't interleave.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;

use crate::app::{AppState, Shared};
use crate::envelope::Signed;
use crate::federation::{self, Admission, Holders, Sender};
use crate::form::Form;
use crate::governance::{license_from_form, signals_from_form, DEFAULT_LICENSE};
use crate::identity::Identity;
use crate::model::{HandleRec, PendingShare, Place, Profile, Reach, RemoteEntry, Version};
use crate::passphrase::{self, Check};
use crate::publishing::{self, Draft};
use crate::sessions::{self, Session};
use crate::vouching::{self, Intro};
use crate::web::layout::{error_page, not_found, redirect, redirect_with, render, server_error, Page};
use crate::web::pages::{self, Card, OwnerPanel};
use crate::{backup, handle, identity, keys, now, outbound};

/// Largest accepted profile picture, as a data URI, in bytes.
const AVATAR_MAX: usize = 600_000;

// ---------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------

/// The signed-in session for this request, if any.
pub(crate) fn session(st: &AppState, headers: &HeaderMap) -> Option<Session> {
    sessions::token_from(headers).and_then(|t| st.sessions.get(&t, now()))
}

/// True when the request is signed in as `handle` and carries that session's
/// CSRF token in the form.
pub(crate) fn authorised_as(st: &AppState, headers: &HeaderMap, form: &Form, handle: &str) -> bool {
    session(st, headers)
        .is_some_and(|s| s.handle == handle && !s.csrf.is_empty() && keys::ct_eq(&s.csrf, &form.one("csrf")))
}

// ---------------------------------------------------------------------------
// Pure helpers for the publish form
// ---------------------------------------------------------------------------

/// The submitted picture if it is a small JPEG, PNG or WebP data URI;
/// otherwise empty (no picture).
pub fn clean_avatar(a: &str) -> String {
    let ok = ["data:image/jpeg;base64,", "data:image/png;base64,", "data:image/webp;base64,"]
        .iter()
        .any(|p| a.starts_with(p));
    if ok && a.len() <= AVATAR_MAX {
        a.to_string()
    } else {
        String::new()
    }
}

/// The profile described by a publish form.
pub fn profile_from_form(f: &Form) -> Profile {
    let place_name = f.clamped("place_name", 120);
    let places = if place_name.is_empty() {
        vec![]
    } else {
        vec![Place { name: place_name, city: f.clamped("place_city", 80) }]
    };
    Profile {
        name: f.clamped("name", 120),
        pronouns: f.clamped("pronouns", 40),
        role: f.clamped("role", 160),
        bio: f.clamped("bio", 600),
        places,
        avatar: clean_avatar(&f.one("avatar")),
    }
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

/// `GET /` — the directory.
pub async fn directory(
    State(st): State<Shared>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    let mut cards = Vec::new();
    for rec in st.store.handles() {
        let version = match st.store.current_version(&rec) {
            Ok(Some(v)) => v,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("warning: {e}");
                continue;
            }
        };
        // Members-only profiles appear as a locked card, and only to members.
        if version.reach() == Reach::Members && me.is_none() {
            continue;
        }
        cards.push(Card { rec, version });
    }
    let remotes = st.store.remotes();
    let pending = if st.site.is_commons() { st.store.pending() } else { Vec::new() };
    // The steward sees their tools (and the founding invite); someone at the
    // commons' own computer gets a link to the steward view.
    let steward = if st.site.is_commons() { crate::web::steward::steward_session(&st, &headers) } else { None };
    let view = steward.map(|s| pages::StewardView { csrf: s.csrf, founding: st.store.founding_code().ok().flatten() });
    let viewer = match &view {
        Some(v) => pages::Viewer::Steward(v),
        None if st.site.is_commons() && crate::web::steward::from_this_computer(&peer) => pages::Viewer::AtThisComputer,
        None => pages::Viewer::Visitor,
    };
    let body = pages::directory(&st.site, &cards, &remotes, &pending, &viewer);
    let notices = notices_for(&st, &headers);
    render(&st.site, Page::new("Directory", me.as_deref(), body).notices(notices))
}

/// `GET /@{handle}` — a member's profile.
pub async fn profile(State(st): State<Shared>, Path(h): Path<String>, headers: HeaderMap) -> Response {
    let sess = session(&st, &headers);
    let me = sess.as_ref().map(|s| s.handle.as_str());
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return not_found(&st.site, me),
        Err(e) => return server_error(&st.site, &e),
    };
    let version = match st.store.current_version(&rec) {
        Ok(v) => v,
        Err(e) => return server_error(&st.site, &e),
    };
    let readable = version.as_ref().and_then(|v| readable_profile(&st, v, me));
    let owner = match &sess {
        Some(s) if s.handle == rec.handle => Some(OwnerPanel {
            csrf: s.csrf.clone(),
            shareable: version.as_ref().is_some_and(|v| v.reach() != Reach::Members),
            known: pages::known_commons(&st.site.share_default, &rec.shared_to),
            shared_to: rec.shared_to.clone(),
            has_recovery: rec.recovery_pub.is_some(),
            rotation_pending: rec.pending_rotation.is_some(),
            waiting_on: rec.waiting_on.clone(),
        }),
        _ => None,
    };
    let noindex = version.as_ref().is_some_and(|v| v.reach() == Reach::Commons);
    let body = pages::profile(&rec, version.as_ref(), readable.as_ref(), owner.as_ref());
    let title = format!("@{}", rec.handle);
    let notices = notices_for(&st, &headers);
    render(&st.site, Page::new(&title, me, body).noindex(noindex).notices(notices))
}

/// The signed-in member's unseen notices, for the banner. Reads only.
fn notices_for(st: &AppState, headers: &HeaderMap) -> Option<crate::web::layout::NoticeBar> {
    let s = session(st, headers)?;
    let texts: Vec<String> =
        st.store.notices().ok()?.into_iter().filter(|n| n.handle == s.handle && !n.seen).map(|n| n.text).collect();
    (!texts.is_empty()).then_some(crate::web::layout::NoticeBar { texts, csrf: s.csrf })
}

/// `POST /waiting/check` — ask the commons we're waiting on, now.
pub async fn check_waiting(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let Some(s) = session(&st, &headers) else { return redirect("/login") };
    if !keys::ct_eq(&s.csrf, &f.one("csrf")) {
        return error_page(&st.site, "Please try again.");
    }
    crate::waiting::check_once(&st).await;
    redirect(&format!("/@{}", s.handle))
}

/// `POST /notices/dismiss` — mark the signed-in member's notices as seen.
pub async fn dismiss_notices(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let Some(s) = session(&st, &headers) else { return redirect("/login") };
    if !keys::ct_eq(&s.csrf, &f.one("csrf")) {
        return error_page(&st.site, "Please try again.");
    }
    if let Err(e) = st.store.mark_notices_seen(&s.handle) {
        return server_error(&st.site, &e);
    }
    redirect(&format!("/@{}", s.handle))
}

/// The profile in `v` that `viewer` may read: the payload for public and
/// commons versions; for members-only, the copy encrypted to the viewer.
pub(crate) fn readable_profile(st: &AppState, v: &Version, viewer: Option<&str>) -> Option<Profile> {
    match v.reach() {
        Reach::Members => {
            let viewer = viewer?;
            let vrec = st.store.handle(viewer).ok()??;
            keys::open_for(v.enc.as_deref()?, viewer, &vrec.box_priv)
        }
        _ => v.payload.clone(),
    }
}

/// `GET /join` — the sign-up form (a commons shows its gate page instead).
pub async fn join_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    let body = if st.site.is_commons() { pages::commons_gate(&st.site) } else { pages::join_form() };
    render(&st.site, Page::new(if st.site.is_commons() { "Shared commons" } else { "Join" }, me.as_deref(), body))
}

/// Ask the node's default commons whether `h` is free there. `None` when
/// there is no default commons or it can't be reached.
async fn free_on_default_commons(st: &AppState, h: &str) -> Option<bool> {
    let default = &st.site.share_default;
    if default.is_empty() {
        return None;
    }
    let reply = st
        .http
        .get(format!("{default}/api/handle/{h}"))
        .timeout(std::time::Duration::from_secs(4))
        .send()
        .await
        .ok()?;
    let j: serde_json::Value = reply.json().await.ok()?;
    j.get("available").and_then(|v| v.as_bool())
}

/// `POST /join` — create a handle and its keys, then sign in.
pub async fn join_submit(State(st): State<Shared>, body: String) -> Response {
    if st.site.is_commons() {
        return render(&st.site, Page::new("Shared commons", None, pages::commons_gate(&st.site)));
    }
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    let display = f.clamped("display_name", 80);
    let pass = f.one("passphrase");
    if !handle::is_valid(&h) {
        return error_page(&st.site, "That handle isn't valid — use 2–30 lowercase letters, numbers or underscores.");
    }
    if display.is_empty() {
        return error_page(&st.site, "Please add a display name.");
    }
    if !passphrase::long_enough(&pass) {
        return error_page(&st.site, "Please choose a passphrase of at least 8 characters.");
    }
    // Handles are unique per commons. Ask the default commons first, so a clash
    // shows now rather than at the first share. If it can't be reached, carry
    // on: the commons still refuses a clash when the profile is shared.
    if free_on_default_commons(&st, &h).await == Some(false) {
        return error_page(
            &st.site,
            &format!(
                "@{h} is already used on {}. Each commons keeps its handles unique — try another.",
                crate::web::layout::host_label(&st.site.share_default)
            ),
        );
    }
    let (salt, hash) = passphrase::hash(&pass);
    let mut rec = HandleRec::new(&h, &display, now(), keys::generate(), salt, hash);
    // The recovery key: its public half is kept and declared to commons; its
    // private half is shown once below and never stored (decision record 0010).
    let (recovery_pub, recovery_priv) = keys::generate_signing();
    match identity::declare_recovery(&h, &rec.ed_pub, &rec.ed_priv, &recovery_pub, now()) {
        Ok(d) => (rec.recovery_pub, rec.recovery_decl) = (Some(recovery_pub), Some(d)),
        Err(e) => return server_error(&st.site, &e),
    }
    {
        let _g = st.write.lock().unwrap();
        match st.store.handle(&h) {
            Ok(None) => {}
            Ok(Some(_)) => return error_page(&st.site, "That handle is taken on this node. Try another."),
            // A damaged record must never be overwritten by a new sign-up.
            Err(e) => return server_error(&st.site, &e),
        }
        if let Err(e) = st.store.save_handle(&rec) {
            return server_error(&st.site, &e);
        }
    }
    let token = st.sessions.start(&h, now());
    let page = render(
        &st.site,
        Page::new(
            "Your recovery code",
            Some(&h),
            pages::recovery_code(&h, &keys::recovery_code(&recovery_priv), "/new"),
        ),
    );
    with_cookie(page, sessions::set_cookie(&token, st.config.secure_cookie()))
}

/// `GET /new` — the publish form for the signed-in member.
pub async fn publish_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let Some(s) = session(&st, &headers) else { return redirect("/login") };
    let rec = match st.store.handle(&s.handle) {
        Ok(Some(r)) => r,
        Ok(None) => return redirect("/login"),
        Err(e) => return server_error(&st.site, &e),
    };
    let current = match st.store.current_version(&rec) {
        Ok(v) => v,
        Err(e) => return server_error(&st.site, &e),
    };
    let body = match &current {
        Some(v) => {
            let p = readable_profile(&st, v, Some(&s.handle)).unwrap_or_default();
            pages::publish_form(&s.handle, &s.csrf, &p, v.reach(), &v.license, &v.signals, false)
        }
        None => pages::publish_form(&s.handle, &s.csrf, &Profile::default(), Reach::Public, DEFAULT_LICENSE, &[], true),
    };
    render(&st.site, Page::new("Publish", Some(&s.handle), body))
}

/// `POST /publish` — sign and store a new version, and make it current.
pub async fn publish(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(&st, &headers, &f, &h) {
        return error_page(&st.site, "Please sign in as this handle to publish.");
    }
    let profile = profile_from_form(&f);
    let reach = Reach::from_form(&f.one("visibility"));
    let license = license_from_form(&f.one("license"));
    let signals = signals_from_form(&f.many("signals"));
    let created_at = now();

    let _g = st.write.lock().unwrap();
    let mut rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return error_page(&st.site, "Unknown handle."),
        Err(e) => return server_error(&st.site, &e),
    };
    let recipients: Vec<(String, String)> = st.store.handles().into_iter().map(|m| (m.handle, m.box_pub)).collect();
    let draft = Draft { profile, reach, license, signals, created_at };
    let version = match publishing::sign_version(&rec, draft, &recipients) {
        Ok(v) => v,
        Err(e) => return server_error(&st.site, &format!("@{h}: {e}")),
    };
    if let Err(e) = st.store.save_version(&version) {
        return server_error(&st.site, &e);
    }
    rec.current_cid = Some(version.cid.clone());
    if let Err(e) = st.store.save_handle(&rec) {
        return server_error(&st.site, &e);
    }
    redirect(&format!("/@{h}"))
}

/// `GET /login` — the sign-in form (a commons shows its gate page instead).
pub async fn login_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    let body = if st.site.is_commons() { pages::commons_gate(&st.site) } else { pages::login_form() };
    render(&st.site, Page::new(if st.site.is_commons() { "Shared commons" } else { "Sign in" }, me.as_deref(), body))
}

/// `POST /login` — check the passphrase and start a session.
pub async fn login_submit(State(st): State<Shared>, body: String) -> Response {
    if st.site.is_commons() {
        return render(&st.site, Page::new("Shared commons", None, pages::commons_gate(&st.site)));
    }
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    let pass = f.one("passphrase");
    let t = now();
    if !st.attempts.allowed(&h, t) {
        return error_page(&st.site, "Too many wrong passphrases for this handle. Wait 15 minutes and try again.")
            .into_response_with(StatusCode::TOO_MANY_REQUESTS);
    }
    const WRONG: &str = "No such handle, or the passphrase is wrong.";
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => {
            st.attempts.failed(&h, t);
            return error_page(&st.site, WRONG);
        }
        Err(e) => return server_error(&st.site, &e),
    };
    match passphrase::check(&rec.secret_salt, &rec.secret_hash, &pass) {
        Check::Wrong => {
            st.attempts.failed(&h, t);
            return error_page(&st.site, WRONG);
        }
        Check::Upgrade => {
            // Replace the old-format hash now that we know the passphrase.
            // A failure only delays the upgrade, so sign-in still succeeds.
            if let Err(e) = update_member(&st, &h, |r| r.with_passphrase_hash(passphrase::hash(&pass))) {
                eprintln!("warning: couldn't upgrade the passphrase hash for @{h}: {e}");
            }
        }
        Check::Ok => {}
    }
    st.attempts.succeeded(&h);
    let token = st.sessions.start(&h, t);
    redirect_with(&format!("/@{h}"), Some(sessions::set_cookie(&token, st.config.secure_cookie())))
}

/// `POST /logout` — end the session.
pub async fn logout(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Some(t) = sessions::token_from(&headers) {
        st.sessions.end(&t);
    }
    redirect_with("/", Some(sessions::clear_cookie(st.config.secure_cookie())))
}

/// `POST /share` — send the current version to a commons.
pub async fn share(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(&st, &headers, &f, &h) {
        return error_page(&st.site, "Please sign in as this handle to share.");
    }
    let commons = f.one("commons_url").trim_end_matches('/').to_string();
    if !(commons.starts_with("http://") || commons.starts_with("https://")) {
        return error_page(&st.site, "Enter the full address of a commons, e.g. https://example.proxy.runpod.net");
    }
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return error_page(&st.site, "Unknown handle."),
        Err(e) => return server_error(&st.site, &e),
    };
    let v = match st.store.current_version(&rec) {
        Ok(Some(v)) => v,
        Ok(None) => return error_page(&st.site, "Publish a profile before sharing it."),
        Err(e) => return server_error(&st.site, &e),
    };
    if v.reach() == Reach::Members {
        return error_page(&st.site, "Members-only profiles stay on your node — they are never shared.");
    }
    let invite = f.one("invite");
    let sent = send_share(&st, &commons, &rec, &v, Some(&invite)).await;
    let label = crate::web::layout::host_label(&commons);
    match sent {
        Ok(r) if r.status().is_success() => {
            let pending = r.text().await.unwrap_or_default().starts_with("pending");
            // Remember the commons, and whether we're waiting to be listed
            // there (the node follows up; see crate::waiting).
            let remembered = update_member(&st, &h, |r| {
                let r = r.with_shared_to(&commons);
                if pending {
                    r.with_waiting_on(&commons)
                } else {
                    r.without_waiting_on(&commons)
                }
            });
            if let Err(e) = remembered {
                eprintln!("warning: shared, but couldn't remember it for @{h}: {e}");
            }
            let body = if pending {
                pages::shared_pending(&h, &commons, &rec.ed_pub, &f.one("csrf"))
            } else {
                pages::shared(&h, &commons)
            };
            render(&st.site, Page::new("Shared", Some(&h), body))
        }
        Ok(r) if r.status() == StatusCode::FORBIDDEN => {
            let why = r.text().await.unwrap_or_default();
            error_page(&st.site, &format!("{label} didn't accept the invite code: {why}"))
        }
        Ok(r) if r.status() == StatusCode::CONFLICT => {
            let reason = r.text().await.unwrap_or_default();
            if reason.contains("newer") {
                error_page(
                    &st.site,
                    &format!("{label} already has a newer version of @{h}. Publish again, then share."),
                )
            } else {
                error_page(&st.site, &format!(
                    "@{h} is already used on {label} by someone else. Each commons keeps its handles unique, so this one can't join there — but you can still share it to other commons, or make a new handle for this one."
                ))
            }
        }
        Ok(r) => error_page(
            &st.site,
            &format!("That commons rejected the share (HTTP {}). It may not accept federation.", r.status().as_u16()),
        ),
        Err(e) => error_page(&st.site, &e),
    }
}

/// Load member `h`, apply the pure `change` to their record, and save the
/// result, all under the write lock. **Writes the member's file.** Returns
/// why it couldn't, for the caller to handle.
pub(crate) fn update_member(st: &AppState, h: &str, change: impl FnOnce(HandleRec) -> HandleRec) -> Result<(), String> {
    let _g = st.write.lock().unwrap();
    let rec = st.store.handle(h).map_err(|e| e.to_string())?.ok_or("no such member")?;
    st.store.save_handle(&change(rec)).map_err(|e| e.to_string())
}

/// POST `body` to a commons' `/inbox`, checking first that this server may
/// contact that address (see [`crate::outbound`]). Errors are messages for
/// the member.
pub(crate) async fn post_to_commons(
    st: &AppState,
    commons: &str,
    body: &serde_json::Value,
) -> Result<reqwest::Response, String> {
    let policy = outbound::Policy {
        personal: st.config.bind_addr(st.site.role).is_loopback(),
        allow_private: st.config.allow_private_share,
    };
    let client = outbound::client_for(commons, policy).await?;
    client
        .post(format!("{commons}/inbox"))
        .json(body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Could not reach that commons: {e}"))
}

/// Share a version to a commons in format `hic/v1` (every field signed). A
/// commons that predates `hic/v1` answers 400; then send the older format so
/// sharing keeps working while commons are upgraded.
pub(crate) async fn send_share(
    st: &AppState,
    commons: &str,
    rec: &HandleRec,
    v: &Version,
    invite: Option<&str>,
) -> Result<reqwest::Response, String> {
    let sender = Sender { base_url: &st.config.base_url, instance: &st.site.instance };
    // An invite code, if the member has one, travels alongside (decision record 0011).
    let with_invite = |mut m: serde_json::Value| {
        if let Some(code) = invite.filter(|c| !c.trim().is_empty()) {
            m["invite"] = serde_json::json!(code.trim());
        }
        m
    };
    let v1 = with_invite(federation::build_share_v1(&sender, rec, v)?);
    let r = post_to_commons(st, commons, &v1).await?;
    if r.status() == StatusCode::BAD_REQUEST {
        return post_to_commons(st, commons, &with_invite(federation::build_share(&sender, rec, v))).await;
    }
    Ok(r)
}

/// Plain-text reply for server-to-server endpoints.
pub(crate) fn reply(status: StatusCode, msg: &str) -> Response {
    (status, msg.to_string()).into_response()
}

/// Log a storage failure and answer 500.
pub(crate) fn store_failed(e: &dyn std::fmt::Display) -> Response {
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
    if activity.get("type").and_then(|t| t.as_str()) == Some("Withdraw") {
        return receive_withdraw(&st, &activity);
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
            admit_newcomer(&st, &entry, &activity, &held)
        }
    }
}

/// A new handle on a commons that needs vouching (decision record 0011):
/// list it with a valid invite or the founding invite, otherwise put it on
/// the pending list until a member vouches for it.
fn admit_newcomer(st: &AppState, entry: &RemoteEntry, message: &serde_json::Value, held: &[RemoteEntry]) -> Response {
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
        my_url: &st.config.base_url,
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
    let w = match federation::verify_withdraw(activity, &st.config.base_url) {
        Ok(w) => w,
        Err(e) => return reply(StatusCode::BAD_REQUEST, &format!("rejected: {e}")),
    };
    let _g = st.write.lock().unwrap();
    let tomb = match federation::apply_withdraw(&w, &st.store.remotes(), &st.store.tombstones()) {
        Ok(t) => t,
        Err(e) => return reply(StatusCode::CONFLICT, e),
    };
    if let Err(e) = st.store.save_tombstone(&tomb).and_then(|_| st.store.delete_remote(&keys::keyid(&w.ed_pub))) {
        return store_failed(&e);
    }
    reply(StatusCode::OK, "withdrawn")
}

/// `POST /withdraw` — ask a commons to remove this member's shared copy.
pub async fn withdraw(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(&st, &headers, &f, &h) {
        return error_page(&st.site, "Please sign in as this handle to withdraw.");
    }
    let commons = f.one("commons_url").trim_end_matches('/').to_string();
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return error_page(&st.site, "Unknown handle."),
        Err(e) => return server_error(&st.site, &e),
    };
    if !rec.shared_to.contains(&commons) {
        return error_page(&st.site, "You haven't shared to that commons from here.");
    }
    let message = match federation::build_withdraw(&rec, &commons, now()) {
        Ok(m) => m,
        Err(e) => return server_error(&st.site, &format!("@{h}: {e}")),
    };
    let sent = post_to_commons(&st, &commons, &message).await;
    let label = crate::web::layout::host_label(&commons);
    match sent {
        Ok(r) if r.status().is_success() => {
            if let Err(e) = update_member(&st, &h, |r| r.without_shared_to(&commons)) {
                eprintln!("warning: withdrawn, but couldn't update the list for @{h}: {e}");
            }
            render(&st.site, Page::new("Withdrawn", Some(&h), pages::withdrawn(&h, &label)))
        }
        Ok(r) => {
            let why = r.text().await.unwrap_or_default();
            error_page(&st.site, &format!("{label} didn't accept the withdrawal: {why}"))
        }
        Err(e) => error_page(&st.site, &e),
    }
}

/// `POST /backup` — download an encrypted backup of the signed-in member's
/// identity. Asks for the passphrase again, which also encrypts the file.
pub async fn backup(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(&st, &headers, &f, &h) {
        return error_page(&st.site, "Please sign in as this handle to back it up.");
    }
    let pass = f.one("passphrase");
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return error_page(&st.site, "Unknown handle."),
        Err(e) => return server_error(&st.site, &e),
    };
    if passphrase::check(&rec.secret_salt, &rec.secret_hash, &pass) == Check::Wrong {
        return error_page(
            &st.site,
            "That passphrase is wrong. The backup is locked with your passphrase, so it must match.",
        );
    }
    let version = st.store.current_version(&rec).ok().flatten();
    let file = match backup::seal(&rec, version.as_ref(), &pass) {
        Ok(t) => t,
        Err(e) => return server_error(&st.site, &e),
    };
    let disposition = format!("attachment; filename=\"held-in-common-{h}-backup.json\"");
    (
        [
            (axum::http::header::CONTENT_TYPE, "application/json".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, disposition),
            (axum::http::header::CACHE_CONTROL, "no-store".to_string()),
        ],
        file,
    )
        .into_response()
}

/// `GET /restore` — the form for restoring an identity from a backup.
pub async fn restore_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    let body = if st.site.is_commons() { pages::commons_gate(&st.site) } else { pages::restore_form() };
    render(&st.site, Page::new("Restore", me.as_deref(), body))
}

/// `POST /restore` — bring an identity (and its current version) back from a
/// backup, then sign in.
pub async fn restore_submit(State(st): State<Shared>, body: String) -> Response {
    if st.site.is_commons() {
        return render(&st.site, Page::new("Shared commons", None, pages::commons_gate(&st.site)));
    }
    let f = Form::parse(&body);
    let pass = f.one("passphrase");
    let (mut rec, version) = match backup::open(&f.one("backup"), &pass) {
        Ok(x) => x,
        Err(e) => return error_page(&st.site, &e),
    };
    // Only bring back a version that really is this identity's, signed by its key.
    let version = version.filter(|v| {
        v.handle == rec.handle
            && Some(&v.cid) == rec.current_cid.as_ref()
            && keys::verify_cid(&rec.ed_pub, &v.cid, &v.signature).is_ok()
    });
    (rec.secret_salt, rec.secret_hash) = passphrase::hash(&pass);
    if version.is_none() {
        rec.current_cid = None;
    }
    {
        let _g = st.write.lock().unwrap();
        match st.store.handle(&rec.handle) {
            Ok(None) => {}
            Ok(Some(_)) => return error_page(&st.site, &format!("@{} already exists on this node.", rec.handle)),
            Err(e) => return server_error(&st.site, &e),
        }
        if let Some(v) = &version {
            if let Err(e) = st.store.save_version(v) {
                return server_error(&st.site, &e);
            }
        }
        if let Err(e) = st.store.save_handle(&rec) {
            return server_error(&st.site, &e);
        }
    }
    let token = st.sessions.start(&rec.handle, now());
    redirect_with(&format!("/@{}", rec.handle), Some(sessions::set_cookie(&token, st.config.secure_cookie())))
}

/// `GET /federated/{keyid}` — a profile shared in from another node.
pub async fn federated(State(st): State<Shared>, Path(kid): Path<String>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    let r = match st.store.remote(&kid) {
        Ok(Some(r)) => r,
        Ok(None) => return not_found(&st.site, me.as_deref()),
        Err(e) => return server_error(&st.site, &e),
    };
    let noindex = Reach::parse(&r.visibility) == Some(Reach::Commons);
    let title = format!("@{} (federated)", r.handle);
    render(&st.site, Page::new(&title, me.as_deref(), pages::federated(&r)).noindex(noindex))
}

/// `GET /about`.
pub async fn about(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    render(&st.site, Page::new("How this works", me.as_deref(), pages::about()))
}

/// Add a `Set-Cookie` header to a response.
pub(crate) fn with_cookie(mut resp: Response, cookie: String) -> Response {
    if let Ok(v) = axum::http::HeaderValue::from_str(&cookie) {
        resp.headers_mut().insert(axum::http::header::SET_COOKIE, v);
    }
    resp
}

/// Small extension so a page response can carry a different status.
trait WithStatus {
    fn into_response_with(self, status: StatusCode) -> Response;
}

impl WithStatus for Response {
    fn into_response_with(mut self, status: StatusCode) -> Response {
        *self.status_mut() = status;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_must_be_a_small_image_data_uri() {
        assert_eq!(clean_avatar("data:image/png;base64,AAAA"), "data:image/png;base64,AAAA");
        assert_eq!(clean_avatar("javascript:alert(1)"), "");
        assert_eq!(clean_avatar("data:image/svg+xml;base64,AAAA"), "");
        assert_eq!(clean_avatar(&format!("data:image/png;base64,{}", "A".repeat(AVATAR_MAX))), "");
    }

    #[test]
    fn profile_from_form_clamps_and_skips_empty_place() {
        let f = Form::parse(&format!("name={}&place_name=&bio=hi", "x".repeat(200)));
        let p = profile_from_form(&f);
        assert_eq!(p.name.len(), 120);
        assert!(p.places.is_empty());
        assert_eq!(p.bio, "hi");
    }
}
