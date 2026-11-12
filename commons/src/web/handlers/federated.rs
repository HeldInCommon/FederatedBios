//! `GET /federated/{keyid}`: a profile shared in from another node.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use maud::html;

use super::session;
use crate::app::Shared;
use crate::model::Reach;
use crate::story;
use crate::web::layout::{not_found, render, server_error, Page};
use crate::web::pages;

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
    // Everything this member has shared here, under their profile.
    let held = st.store.held_stories();
    let theirs = story::by_author(&held, &r.ed_pub);
    let body = html! { (pages::federated(&r)) (pages::author_stories(&r.handle, &theirs)) };
    render(&st.site, Page::new(&title, me.as_deref(), body).noindex(noindex))
}
