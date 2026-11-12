//! Story lists, and a story as shared to a commons (see `crate::story`).
//!
//! `GET /stories`: on a node, its stories; on a commons, the stories shared
//! to it and recent withdrawals. `GET /stories/{keyid}/{id}`: one story on a
//! commons. The author's own story pages are in `story.rs` and `story_edit.rs`.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;

use super::session;
use crate::app::Shared;
use crate::model::Reach;
use crate::now;
use crate::story::{self, OwnStory};
use crate::web::layout::{not_found, render, server_error, Page};
use crate::web::pages;

/// `GET /stories` — a node's stories, or the stories shared to a commons.
pub async fn stories(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    if st.site.is_commons() {
        let held = st.store.held_stories();
        let withdrawn = story::recent_withdrawals(&st.store.story_tombstones(), &held, &st.store.remotes(), now());
        let noindex = held.iter().any(|h| h.story.reach() == Reach::Commons);
        let body = pages::commons_stories(&st.site.instance, &held, &withdrawn, now());
        return render(&st.site, Page::new("Stories", me.as_deref(), body).noindex(noindex).with_cards());
    }
    let visible: Vec<OwnStory> =
        st.store.own_stories().into_iter().filter(|s| me.is_some() || s.story.reach() != Reach::Members).collect();
    render(&st.site, Page::new("Stories", me.as_deref(), pages::node_stories(&visible, me.as_deref())).with_cards())
}

/// `GET /stories/{keyid}/{id}` — a story shared to this commons.
pub async fn held_story(
    State(st): State<Shared>,
    Path((kid, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    if !st.site.is_commons() {
        return not_found(&st.site, me.as_deref());
    }
    match st.store.held_story(&kid, &id) {
        Ok(Some(h)) => {
            let noindex = h.story.reach() == Reach::Commons;
            render(&st.site, Page::new(&h.story.title, me.as_deref(), pages::held_story(&h, now())).noindex(noindex))
        }
        Ok(None) => not_found(&st.site, me.as_deref()),
        Err(e) => server_error(&st.site, &e),
    }
}
