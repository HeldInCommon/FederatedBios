//! `GET /`: the directory.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;

use super::notices::notices_for;
use super::session;
use crate::app::{AppState, Shared};
use crate::model::Reach;
use crate::web::layout::{render, Page};
use crate::web::pages::{self, Card};

/// `GET /` — the directory.
pub async fn directory(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let sess = session(&st, &headers);
    let me = sess.as_ref().map(|s| s.handle.clone());
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
    // The steward sees their tools (and the founding invite).
    let steward = if st.site.is_commons() { super::steward::steward_session(&st, &headers) } else { None };
    let view = steward.map(|s| pages::StewardView {
        csrf: s.csrf,
        founding: st.store.founding_code().ok().flatten(),
        offer_node: st.site.in_app && !st.site.practice,
    });
    let viewer = match &view {
        Some(v) => pages::Viewer::Steward(v),
        None => pages::Viewer::Visitor,
    };
    let signed_in =
        if st.site.is_commons() { None } else { sess.as_ref().and_then(|s| signed_in(&st, &s.handle, &s.csrf)) };
    let body = pages::directory(&st.site, &cards, &remotes, &pending, &viewer, signed_in.as_ref());
    let notices = notices_for(&st, &headers);
    render(&st.site, Page::new("Directory", me.as_deref(), body).notices(notices))
}

/// What the front page offers a signed-in member (see `pages::your_next`).
fn signed_in(st: &AppState, handle: &str, csrf: &str) -> Option<pages::SignedIn> {
    let rec = st.store.handle(handle).ok()??;
    let version = st.store.current_version(&rec).ok().flatten();
    let known = pages::known_commons(&st.site.share_default, &rec.shared_to);
    let next = pages::your_next(
        version.is_some(),
        version.as_ref().is_some_and(|v| v.reach() != Reach::Members),
        &rec.ed_pub,
        &known,
        &rec.shared_to,
        &rec.waiting_on,
    );
    Some(pages::SignedIn { handle: handle.into(), csrf: csrf.into(), next })
}
