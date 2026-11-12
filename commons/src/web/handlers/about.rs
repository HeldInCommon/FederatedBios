//! `GET /about` and `GET /run-a-commons`.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;

use super::session;
use crate::app::Shared;
use crate::web::layout::{not_found, render, Page, Section};
use crate::web::pages;

/// `GET /about`.
pub async fn about(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    render(&st.site, Page::new("How this works", me.as_deref(), pages::about(!st.site.is_commons())).at(Section::About))
}

/// `GET /run-a-commons`: on a node, how to host a commons and a way to try
/// one. A commons has no use for it.
pub async fn run_a_commons(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if st.site.is_commons() {
        return not_found(&st.site, None);
    }
    let me = session(&st, &headers).map(|s| s.handle);
    render(&st.site, Page::new("Run a commons", me.as_deref(), pages::run_a_commons(st.site.in_app)))
}
