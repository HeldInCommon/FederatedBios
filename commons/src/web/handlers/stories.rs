//! Story lists, and a story as shared to a commons (see `crate::story`).
//!
//! `GET /stories`: on a node, its stories; on a commons, the stories shared
//! to it (copies, and "Only while I'm here" cards) and recent withdrawals.
//! `GET /stories/{keyid}/{id}`: one story on a commons; for a card, the text
//! is fetched from its author's node (see `here.rs`). The author's own story
//! pages are in `story.rs` and `story_edit.rs`.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;

use super::session;
use crate::app::Shared;
use crate::model::Reach;
use crate::now;
use crate::story::{self, OwnStory};
use crate::web::layout::{not_found, render, server_error, Page, Section};
use crate::web::pages;

/// `GET /stories` — a node's stories, or the stories shared to a commons.
pub async fn stories(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    if st.site.is_commons() {
        let held = st.store.held_stories();
        let cards = st.store.story_cards();
        let withdrawn =
            story::recent_withdrawals(&st.store.story_tombstones(), &held, &cards, &st.store.remotes(), now());
        let noindex = held.iter().any(|h| h.story.reach() == Reach::Commons)
            || cards.iter().any(|c| c.card.reach() == Reach::Commons);
        let body = pages::commons_stories(&st.site.instance, st.site.in_app, &held, &cards, &withdrawn, now());
        return render(&st.site, Page::new("Stories", me.as_deref(), body).noindex(noindex).at(Section::Stories));
    }
    let visible: Vec<OwnStory> =
        st.store.own_stories().into_iter().filter(|s| me.is_some() || s.story.reach() != Reach::Members).collect();
    render(
        &st.site,
        Page::new("Stories", me.as_deref(), pages::node_stories(&visible, me.as_deref())).at(Section::Stories),
    )
}

/// `GET /stories/{keyid}/{id}` — a story shared to this commons: a copy, or
/// a card whose text is fetched from its author's node right now.
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
            render(
                &st.site,
                Page::new(&h.story.title, me.as_deref(), pages::held_story(&h, now()))
                    .noindex(noindex)
                    .at(Section::Stories),
            )
        }
        // No copy: perhaps a card ("Only while I'm here").
        Ok(None) => match st.store.story_card(&kid, &id) {
            Ok(Some(card)) => super::here::card_page(&st, card, me.as_deref()).await,
            Ok(None) => not_found(&st.site, me.as_deref()),
            Err(e) => server_error(&st.site, &e),
        },
        Err(e) => server_error(&st.site, &e),
    }
}
