//! The steward view of a commons (decision records 0017 and 0023).
//!
//! A commons has no member sign-in, so its steward (whoever runs it) proves
//! who they are one of two ways:
//! - the **desktop app** opens `/steward?launch=…` with the private token it
//!   started this server with, or
//! - the **steward link** the commons prints in its log when it starts
//!   (`/steward?key=…`), for a commons in Docker or on a server.
//!
//! Being "at this computer" is *not* proof (ADR 0023): behind a reverse proxy
//! or tunnel such as Tailscale Funnel, every visitor arrives from this
//! computer.
//!
//! Either starts a steward session. The steward can then approve or decline
//! people waiting for a vouch, and make invite codes. Every form carries the
//! session's CSRF token, so another website can't press these buttons.

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use serde::Deserialize;

use crate::app::{AppState, Shared};
use crate::federation::{self, Admission, Holders};
use crate::form::Form;
use crate::keys;
use crate::now;
use crate::sessions::{self, Session};
use crate::vouching::{self, Intro};
use crate::web::handlers::{list_entry_response, session};
use crate::web::layout::{error_page, not_found, redirect, redirect_with, render, server_error, Page};
use crate::web::pages;

/// The session "handle" a steward signs in as. Handles can't contain `@`, so
/// it can never clash with a member.
pub const STEWARD: &str = "@steward";

/// The steward's session, if this request has one.
pub(crate) fn steward_session(st: &AppState, headers: &HeaderMap) -> Option<Session> {
    session(st, headers).filter(|s| s.handle == STEWARD)
}

/// The steward's session, if the form's CSRF token matches it.
fn authorised_steward(st: &AppState, headers: &HeaderMap, f: &Form) -> Option<Session> {
    steward_session(st, headers).filter(|s| keys::ct_eq(&s.csrf, &f.one("csrf")))
}

/// The query string of `GET /steward`.
#[derive(Deserialize)]
pub struct OpenQ {
    /// The steward link's key.
    pub key: Option<String>,
    /// The desktop app's launch token, when the app opens the steward view.
    pub launch: Option<String>,
}

/// `GET /steward` — start a steward session (with the steward link's key, or
/// the desktop app's launch token), then show the commons page in steward
/// view.
pub async fn open(State(st): State<Shared>, Query(q): Query<OpenQ>) -> Response {
    if !st.site.is_commons() {
        return not_found(&st.site, None);
    }
    let stored = match st.store.steward_key() {
        Ok(k) => k,
        Err(e) => return server_error(&st.site, &e),
    };
    let key_ok = matches!((&q.key, &stored), (Some(given), Some(k)) if keys::ct_eq(given.trim(), k));
    let launch_ok =
        matches!((&q.launch, &st.config.launch_token), (Some(given), Some(t)) if keys::ct_eq(given.trim(), t));
    if !(key_ok || launch_ok) {
        return error_page(
            &st.site,
            "That steward link isn't right. The commons prints the right one in its log when it starts (in the desktop app: Go → Show steward link…).",
        );
    }
    let token = st.sessions.start(STEWARD, now());
    redirect_with("/", Some(sessions::set_cookie(&st.config.session_cookie, &token, st.config.secure_cookie())))
}

/// A steward form that names one person by their key.
#[derive(Deserialize)]
struct KeyForm {
    key: String,
}

/// `POST /steward/approve` — list someone who is waiting for a vouch.
pub async fn approve(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    if authorised_steward(&st, &headers, &f).is_none() {
        return error_page(&st.site, "Open the steward view first.");
    }
    let KeyForm { key } = KeyForm { key: f.one("key") };
    let _g = st.write.lock().unwrap();
    let Some(waiting) = st.store.pending().into_iter().find(|p| p.entry.ed_pub == key) else {
        return error_page(&st.site, "That person is no longer waiting.");
    };
    // Someone else may have taken the handle while this share waited.
    let (held, withdrawn) = (st.store.remotes(), st.store.tombstones());
    let claimed = st.store.identity(&waiting.entry.handle).ok().flatten().map(|i| i.key);
    let holders = Holders { local: None, held: &held, withdrawn: &withdrawn, claimed: claimed.as_deref() };
    if federation::admit(&waiting.entry, &holders) != Admission::Accept {
        let _ = st.store.delete_pending(&key);
        return error_page(&st.site, "That handle was taken while they were waiting.");
    }
    let intro = Intro {
        handle: waiting.entry.handle.clone(),
        key: key.clone(),
        route: "steward".into(),
        voucher: None,
        at: now(),
        proof: None,
    };
    match list_entry_response(&st, &waiting.entry, &waiting.message, Some(intro)) {
        Ok(()) => redirect("/"),
        Err(e) => server_error(&st.site, &e),
    }
}

/// `POST /steward/decline` — take someone off the waiting list.
pub async fn decline(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    if authorised_steward(&st, &headers, &f).is_none() {
        return error_page(&st.site, "Open the steward view first.");
    }
    let _g = st.write.lock().unwrap();
    match st.store.delete_pending(&f.one("key")) {
        Ok(()) => redirect("/"),
        Err(e) => server_error(&st.site, &e),
    }
}

/// `POST /steward/invite` — make a single-use invite code for this commons.
pub async fn invite(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    if authorised_steward(&st, &headers, &f).is_none() {
        return error_page(&st.site, "Open the steward view first.");
    }
    let code = vouching::new_steward_invite_code();
    if let Err(e) = st.store.save_commons_invite(&code, now()) {
        return server_error(&st.site, &e);
    }
    render(&st.site, Page::new("Invite code", Some(STEWARD), pages::steward_invite(&st.site, &code)))
}
