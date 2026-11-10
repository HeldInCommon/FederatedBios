//! `GET /`: the directory.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Extension;

use super::notices::notices_for;
use super::session;
use crate::app::Shared;
use crate::model::Reach;
use crate::web::layout::{render, Page};
use crate::web::pages::{self, Card};

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
    let steward = if st.site.is_commons() { super::steward::steward_session(&st, &headers) } else { None };
    let view = steward.map(|s| pages::StewardView { csrf: s.csrf, founding: st.store.founding_code().ok().flatten() });
    let viewer = match &view {
        Some(v) => pages::Viewer::Steward(v),
        None if st.site.is_commons() && super::steward::from_this_computer(&peer) => pages::Viewer::AtThisComputer,
        None => pages::Viewer::Visitor,
    };
    let body = pages::directory(&st.site, &cards, &remotes, &pending, &viewer);
    let notices = notices_for(&st, &headers);
    render(&st.site, Page::new("Directory", me.as_deref(), body).notices(notices))
}
