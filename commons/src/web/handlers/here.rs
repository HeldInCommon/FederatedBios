//! "Only while I'm here" on a commons (decision record 0021):
//! `POST /here/card` (a node shares a story's card), `POST /here/poll` (a
//! node's long poll, carrying its answers), and reading a card's story:
//! the page (from `GET /stories/{keyid}/{id}`, see `stories.rs`) and
//! `GET /stories/{keyid}/{id}/here.json` (the signed card with the text, so
//! a reader's own node or script can check the hash itself).
//!
//! **Nothing is cached.** The text a node sends is checked against the
//! card's signed SHA-256, handed to the one reader who asked, and dropped:
//! it is never written to disk, and every response carries
//! `Cache-Control: no-store` so browsers and proxies don't keep it either.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;

use super::session;
use crate::app::{AppState, Shared};
use crate::here::{self, CardAdmission, HeldCard, PollReply};
use crate::model::Reach;
use crate::relay::{PollRefused, Reading};
use crate::web::layout::{not_found, render, Page, Section};
use crate::web::pages;
use crate::{now, sessions};

/// How often a waiting poll or reader looks for news. Short enough that a
/// reader barely notices; long enough to cost nothing.
const TICK: Duration = Duration::from_millis(100);

/// Plain-text reply for node-to-commons endpoints.
fn reply(status: StatusCode, msg: &str) -> Response {
    (status, msg.to_string()).into_response()
}

/// Log a storage failure and answer 500.
fn store_failed(e: &dyn std::fmt::Display) -> Response {
    eprintln!("error: {e}");
    reply(StatusCode::INTERNAL_SERVER_ERROR, "could not read or write the data folder")
}

/// `response` with `Cache-Control: no-store`: nobody along the way may keep
/// a copy of a story shared "Only while I'm here".
fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    response
}

/// `POST /here/card` — a node shares a story as "Only while I'm here": the
/// commons keeps the signed card (no text) if the author is listed here.
/// A newer card replaces a copy of the same story.
///
/// A commons from before decision record 0021 has no such route and
/// answers 404, which the node reads as "not supported yet".
pub async fn here_card(
    State(st): State<Shared>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    body: String,
) -> Response {
    if !st.site.is_commons() {
        return reply(StatusCode::BAD_REQUEST, "this is a node, not a commons");
    }
    // The same limit as the inbox: checked before any signature work.
    let sender = peer.map(|Extension(ConnectInfo(a))| a.ip().to_string()).unwrap_or_else(|| "unknown".into());
    if !st.inbox_limit.record_if_under_limit(&sender, now()) {
        return reply(StatusCode::TOO_MANY_REQUESTS, "too many requests from this address; try again in a minute");
    }
    let Ok(msg) = serde_json::from_str::<serde_json::Value>(&body) else {
        return reply(StatusCode::BAD_REQUEST, "invalid json");
    };
    let (card, signed) = match here::verify_card(&msg, &st.own_hosts) {
        Ok(v) => v,
        Err(e) => return reply(StatusCode::BAD_REQUEST, &format!("rejected: {e}")),
    };
    let _g = st.write.lock().unwrap();
    let held = st.store.held_stories();
    let cards = st.store.story_cards();
    match here::admit_card(&card, &signed, &st.store.remotes(), &held, &cards, &st.store.story_tombstones()) {
        CardAdmission::NotListed => reply(
            StatusCode::FORBIDDEN,
            "stories can be shared here once you're listed: share your profile to this commons first",
        ),
        CardAdmission::Stale => {
            reply(StatusCode::CONFLICT, "this commons already holds a newer version of that story, or it was withdrawn")
        }
        CardAdmission::Accept => {
            let author = signed.envelope.author.clone();
            let id = card.id.clone();
            // Keep the card, then remove any copy: from now on the text
            // lives only on the author's node.
            let saved = st
                .store
                .save_story_card(&HeldCard { card, signed, received_at: now() })
                .and_then(|_| st.store.delete_held_story(&author, &id));
            match saved {
                Ok(()) => reply(StatusCode::OK, "card shared"),
                Err(e) => store_failed(&e),
            }
        }
    }
}

/// `POST /here/poll` — a node asks "anything for me?". The commons checks
/// the signed poll, takes its answers (each checked against the card's
/// hash), then holds the request open until a reader asks for one of this
/// author's stories, a newer poll arrives, or the hold time is up, and
/// answers with the requests waiting.
pub async fn here_poll(State(st): State<Shared>, body: String) -> Response {
    if !st.site.is_commons() {
        return reply(StatusCode::BAD_REQUEST, "this is a node, not a commons");
    }
    let Ok(msg) = serde_json::from_str::<serde_json::Value>(&body) else {
        return reply(StatusCode::BAD_REQUEST, "invalid json");
    };
    let relay = match here::verify_relay(&msg, &st.own_hosts, now()) {
        Ok(r) => r,
        Err(e) => return reply(StatusCode::BAD_REQUEST, &format!("rejected: {e}")),
    };
    // Only an author with a card here may hold a poll open: the signature
    // says who they are, the card says they have something to answer for.
    let theirs: Vec<HeldCard> = st.store.story_cards().into_iter().filter(|c| c.author() == relay.author).collect();
    if theirs.is_empty() {
        return reply(StatusCode::FORBIDDEN, "nothing of yours is shared here as Only while I'm here");
    }
    let poll = match st.relay.open_poll(&relay.author, relay.seq, now()) {
        Ok(p) => p,
        Err(PollRefused::Replayed) => return reply(StatusCode::CONFLICT, "rejected: an old poll (its seq was used)"),
    };
    st.relay.apply_answers(&relay.author, &relay.answers, |story| {
        theirs.iter().find(|c| c.card.id == story).map(|c| &c.card)
    });
    // Hold the poll open, looking for news every tick.
    let deadline = Instant::now() + Duration::from_secs(st.config.relay.hold);
    let requests = loop {
        if !st.relay.is_current_poll(&relay.author, poll) {
            // A newer poll took over (or a reader gave up on this one).
            break Vec::new();
        }
        let handed = st.relay.hand_over(&relay.author);
        if !handed.is_empty() || Instant::now() >= deadline {
            break handed;
        }
        tokio::time::sleep(TICK).await;
    };
    st.relay.record_poll_closed(&relay.author, poll, now());
    no_store(Json(PollReply { requests }).into_response())
}

/// Ask the author's node for a card's text and wait for it (at most the
/// reader wait, `HIC_RELAY_WAIT`). If they aren't here, say so at once.
/// The text comes back already checked against the card's signed hash
/// (see [`crate::relay::Relay::apply_answers`]).
async fn fetch_text(st: &AppState, held: &HeldCard) -> Reading {
    let author = held.author();
    if !st.relay.is_here(author, now(), st.config.relay) {
        return Reading::Away;
    }
    let id = match st.relay.record_request(author, &held.card.id, sessions::random_hex(16)) {
        Ok(id) => id,
        Err(busy) => return busy,
    };
    let deadline = Instant::now() + Duration::from_secs(st.config.relay.wait);
    loop {
        if let Some(reading) = st.relay.take_reading(&id) {
            return reading;
        }
        if Instant::now() >= deadline {
            st.relay.record_gave_up(&id);
            return Reading::Away;
        }
        tokio::time::sleep(TICK).await;
    }
}

/// The page for a story held here as a card: its text if the author is
/// here (fetched now, never stored), or "Not here right now". Called from
/// `GET /stories/{keyid}/{id}` when there's a card and no copy.
pub(crate) async fn card_page(st: &AppState, held: HeldCard, me: Option<&str>) -> Response {
    let reading = fetch_text(st, &held).await;
    if reading == Reading::Gone {
        // Withdrawn while the reader waited.
        return not_found(&st.site, me);
    }
    let body = pages::here_story(&held, &reading, now());
    let noindex = held.card.reach() == Reach::Commons;
    no_store(render(&st.site, Page::new(&held.card.title, me, body).noindex(noindex).at(Section::Stories)))
}

/// `GET /stories/{keyid}/{id}/here.json` — the signed card and, if its
/// author is here, the text: so a reader's own node or a script can check
/// the text against the signed SHA-256 itself, without trusting this
/// commons. 503 when the author isn't here; 409 when the text didn't match.
pub async fn here_json(
    State(st): State<Shared>,
    Path((kid, id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    let held = match st.store.story_card(&kid, &id) {
        Ok(Some(c)) if st.site.is_commons() => c,
        Ok(_) => return not_found(&st.site, me.as_deref()),
        Err(e) => return store_failed(&e),
    };
    let (status, body) = match fetch_text(&st, &held).await {
        Reading::Text(text) => (StatusCode::OK, json!({ "card": held.signed, "here": true, "text": text })),
        Reading::Away => (StatusCode::SERVICE_UNAVAILABLE, json!({ "card": held.signed, "here": false })),
        Reading::Changed => (StatusCode::CONFLICT, json!({ "card": held.signed, "here": true, "changed": true })),
        Reading::Busy => (StatusCode::TOO_MANY_REQUESTS, json!({ "card": held.signed, "busy": true })),
        Reading::Gone => return not_found(&st.site, me.as_deref()),
    };
    no_store((status, Json(body)).into_response())
}
