//! Request handlers for the HTML side of the server.
//!
//! Each handler does the same three things: load what it needs (store,
//! session), call pure functions for every decision, then render. Writes that
//! read a record, change it and save it back happen while holding
//! [`AppState::write`], so two requests can't interleave.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::app::{AppState, Shared};
use crate::canonical::{content_id, hash_body};
use crate::federation::{self, Admission, Sender};
use crate::form::Form;
use crate::governance::{license_from_form, signals_from_form, DEFAULT_LICENSE};
use crate::model::{HandleRec, Place, Profile, Reach, Version};
use crate::passphrase::{self, Check};
use crate::sessions::{self, Session};
use crate::web::layout::{error_page, not_found, redirect, redirect_with, render, server_error, Page};
use crate::web::pages::{self, Card, OwnerPanel};
use crate::{handle, keys, now};

/// Largest accepted profile picture, as a data URI, in bytes.
const AVATAR_MAX: usize = 600_000;

// ---------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------

/// The signed-in session for this request, if any.
fn session(st: &AppState, headers: &HeaderMap) -> Option<Session> {
    sessions::token_from(headers).and_then(|t| st.sessions.get(&t, now()))
}

/// True when the request is signed in as `handle` and carries that session's
/// CSRF token in the form.
fn authorised_as(st: &AppState, headers: &HeaderMap, form: &Form, handle: &str) -> bool {
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
pub async fn directory(State(st): State<Shared>, headers: HeaderMap) -> Response {
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
    render(&st.site, Page::new("Directory", me.as_deref(), pages::directory(&st.site, &cards, &remotes)))
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
        }),
        _ => None,
    };
    let noindex = version.as_ref().is_some_and(|v| v.reach() == Reach::Commons);
    let body = pages::profile(&rec, version.as_ref(), readable.as_ref(), owner.as_ref());
    let title = format!("@{}", rec.handle);
    render(&st.site, Page::new(&title, me, body).noindex(noindex))
}

/// The profile in `v` that `viewer` may read: the payload for public and
/// commons versions; for members-only, the copy encrypted to the viewer.
fn readable_profile(st: &AppState, v: &Version, viewer: Option<&str>) -> Option<Profile> {
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
    let k = keys::generate();
    let rec = HandleRec {
        handle: h.clone(),
        display_name: display,
        created_at: now(),
        ed_pub: k.ed_pub,
        ed_priv: k.ed_priv,
        box_pub: k.box_pub,
        box_priv: k.box_priv,
        secret_salt: salt,
        secret_hash: hash,
        current_cid: None,
        shared_to: Vec::new(),
    };
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
    redirect_with("/new", Some(sessions::set_cookie(&token, st.config.secure_cookie())))
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
    let (payload, enc, body_json) = if reach == Reach::Members {
        let recipients: Vec<(String, String)> = st.store.handles().into_iter().map(|m| (m.handle, m.box_pub)).collect();
        let boxes = keys::seal_for_members(&profile, &recipients);
        let bj = serde_json::to_value(&boxes).expect("boxes serialise");
        (None, Some(boxes), bj)
    } else {
        let bj = hash_body(&profile);
        (Some(profile), None, bj)
    };
    let cid = content_id(&h, reach.as_str(), &license, &signals, created_at, &body_json);
    let signature = match keys::sign_cid(&rec.ed_priv, &cid) {
        Ok(s) => s,
        Err(e) => return server_error(&st.site, &format!("@{h}: {e}")),
    };
    let version = Version {
        cid: cid.clone(),
        handle: h.clone(),
        visibility: reach.as_str().into(),
        license,
        signals,
        created_at,
        signature,
        payload,
        enc,
    };
    if let Err(e) = st.store.save_version(&version) {
        return server_error(&st.site, &e);
    }
    rec.current_cid = Some(cid);
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
        Check::Upgrade => upgrade_passphrase_hash(&st, &h, &pass),
        Check::Ok => {}
    }
    st.attempts.succeeded(&h);
    let token = st.sessions.start(&h, t);
    redirect_with(&format!("/@{h}"), Some(sessions::set_cookie(&token, st.config.secure_cookie())))
}

/// Replace an old-format passphrase hash with Argon2id after a correct
/// sign-in. A failure is logged and the sign-in still succeeds.
fn upgrade_passphrase_hash(st: &AppState, h: &str, pass: &str) {
    let _g = st.write.lock().unwrap();
    if let Ok(Some(mut rec)) = st.store.handle(h) {
        (rec.secret_salt, rec.secret_hash) = passphrase::hash(pass);
        if let Err(e) = st.store.save_handle(&rec) {
            eprintln!("warning: couldn't upgrade the passphrase hash for @{h}: {e}");
        }
    }
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
    let activity =
        federation::build_share(&Sender { base_url: &st.config.base_url, instance: &st.site.instance }, &rec, &v);
    let sent = st
        .http
        .post(format!("{commons}/inbox"))
        .json(&activity)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;
    let label = crate::web::layout::host_label(&commons);
    match sent {
        Ok(r) if r.status().is_success() => {
            remember_shared_to(&st, &h, &commons);
            render(&st.site, Page::new("Shared", Some(&h), pages::shared(&h, &commons)))
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
        Err(e) => error_page(&st.site, &format!("Could not reach that commons: {e}")),
    }
}

/// Add `commons` to the member's list of commons shared to.
fn remember_shared_to(st: &AppState, h: &str, commons: &str) {
    let _g = st.write.lock().unwrap();
    if let Ok(Some(mut rec)) = st.store.handle(h) {
        if !rec.shared_to.iter().any(|c| c == commons) {
            rec.shared_to.push(commons.to_string());
            if let Err(e) = st.store.save_handle(&rec) {
                eprintln!("warning: {e}");
            }
        }
    }
}

/// `POST /inbox` — receive a share from another node.
pub async fn inbox(State(st): State<Shared>, body: String) -> Response {
    let Ok(activity) = serde_json::from_str::<serde_json::Value>(&body) else {
        return (StatusCode::BAD_REQUEST, "invalid json").into_response();
    };
    let entry = match federation::verify_share(&activity, now()) {
        Ok(e) => e,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("rejected: {e}")).into_response(),
    };
    let _g = st.write.lock().unwrap();
    let local_holder = match st.store.handle(&entry.handle) {
        Ok(r) => r.map(|r| r.ed_pub),
        Err(e) => {
            eprintln!("error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "could not read").into_response();
        }
    };
    match federation::admit(&entry, local_holder.as_deref(), &st.store.remotes()) {
        Admission::HandleTaken => {
            (StatusCode::CONFLICT, "that handle is already used on this commons by a different key").into_response()
        }
        Admission::Stale => {
            (StatusCode::CONFLICT, "this commons already holds a newer version from this key").into_response()
        }
        Admission::Accept => match st.store.save_remote(&entry) {
            Ok(()) => (StatusCode::ACCEPTED, "accepted").into_response(),
            Err(e) => {
                eprintln!("error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "could not store").into_response()
            }
        },
    }
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
