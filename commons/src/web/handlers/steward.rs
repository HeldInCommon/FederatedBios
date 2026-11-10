//! The steward view of a commons (decision record 0017).
//!
//! A commons has no member sign-in, so its steward (whoever runs it) proves
//! who they are one of two ways:
//! - by opening `/steward` **at the commons' own computer** (the desktop
//!   app's commons window counts), or
//! - with the **steward link** the commons prints in its log when it starts
//!   (`/steward?key=…`), for a commons in Docker or on a server.
//!
//! Either starts a steward session. The steward can then approve or decline
//! people waiting for a vouch, and make invite codes. Every form carries the
//! session's CSRF token, so another website can't press these buttons.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Extension;
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

/// True when the request comes from this computer.
pub(crate) fn from_this_computer(peer: &Option<Extension<ConnectInfo<SocketAddr>>>) -> bool {
    peer.as_ref().is_some_and(|Extension(ConnectInfo(a))| a.ip().is_loopback())
}

#[derive(Deserialize)]
pub struct OpenQ {
    pub key: Option<String>,
}

/// `GET /steward` — start a steward session (at this computer, or with the
/// steward link's key), then show the commons page in steward view.
pub async fn open(
    State(st): State<Shared>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    Query(q): Query<OpenQ>,
) -> Response {
    if !st.site.is_commons() {
        return not_found(&st.site, None);
    }
    let stored = match st.store.steward_key() {
        Ok(k) => k,
        Err(e) => return server_error(&st.site, &e),
    };
    let key_ok = matches!((&q.key, &stored), (Some(given), Some(k)) if keys::ct_eq(given.trim(), k));
    if !(from_this_computer(&peer) || key_ok) {
        return error_page(
            &st.site,
            "That steward link isn't right. The commons prints the right one in its log when it starts.",
        );
    }
    let token = st.sessions.start(STEWARD, now());
    redirect_with("/", Some(sessions::set_cookie(&token, st.config.secure_cookie())))
}

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
