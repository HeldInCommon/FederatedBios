//! `GET /about`.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;

use super::session;
use crate::app::Shared;
use crate::web::layout::{render, Page};
use crate::web::pages;

/// `GET /about`.
pub async fn about(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    render(&st.site, Page::new("How this works", me.as_deref(), pages::about()))
}
