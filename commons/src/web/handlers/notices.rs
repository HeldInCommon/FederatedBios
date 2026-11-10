//! News for a member ("you're now listed on …"): the banner, checking now,
//! and dismissing it.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;

use super::session;
use crate::app::{AppState, Shared};
use crate::form::Form;
use crate::keys;
use crate::web::layout::{error_page, redirect, server_error};

/// The signed-in member's unseen notices, for the banner. Reads only.
pub(super) fn notices_for(st: &AppState, headers: &HeaderMap) -> Option<crate::web::layout::NoticeBar> {
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
