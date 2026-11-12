//! `POST /api/waiting`: a commons tells its own members who is waiting for a
//! vouch (decision record 0026).
//!
//! The asker proves they are listed here by sending a fresh
//! `WaitingListRequest` signed with their key (the same kind of proof as a
//! vouch; see [`crate::vouching::check_waiting_request`]). Anyone else gets
//! a refusal. The answer holds only each person's handle, name and key
//! ([`crate::vouch_list::waiting_from`]).
//!
//! This only reads: it changes nothing on the commons except the rate
//! limiter's count.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;

use crate::app::Shared;
use crate::envelope::Signed;
use crate::{now, vouch_list, vouching};

/// `POST /api/waiting` — the people waiting for a vouch, for a listed member
/// who signed the request. Answers 404 on a node, 429 when one address asks
/// too often, 400 for something that isn't a signed object, and 403 (with
/// the reason) when the request isn't from a member of this commons, is for
/// another commons, or is out of date.
pub async fn waiting_list(
    State(st): State<Shared>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    body: String,
) -> Response {
    // Only a commons has a waiting list.
    if !st.site.is_commons() {
        return (StatusCode::NOT_FOUND, "this is a node, not a commons").into_response();
    }
    // Counted with `/inbox`, before any parsing or signature work, so a
    // flood of requests costs the commons little (see `ratelimit.rs`).
    let sender = peer.map(|Extension(ConnectInfo(a))| a.ip().to_string()).unwrap_or_else(|| "unknown".into());
    if !st.inbox_limit.record_if_under_limit(&sender, now()) {
        return (StatusCode::TOO_MANY_REQUESTS, "too many requests from this address; try again in a minute")
            .into_response();
    }
    let Ok(request) = serde_json::from_str::<Signed>(&body) else {
        return (StatusCode::BAD_REQUEST, "rejected: not a signed object").into_response();
    };
    // Members are the keys of everyone listed here, the same people who may
    // vouch (ADR 0011). Someone still waiting is not a member, so they
    // can't read the list they are on.
    let members: Vec<String> = st.store.remotes().into_iter().map(|r| r.ed_pub).collect();
    if let Err(why) = vouching::check_waiting_request(&request, &st.own_hosts, &members, now()) {
        return (StatusCode::FORBIDDEN, why).into_response();
    }
    let waiting = vouch_list::waiting_from(&st.store.pending());
    // `no-store`: this is for one member's eyes, so no cache along the way
    // should keep a copy.
    ([(header::CACHE_CONTROL, "no-store")], Json(json!({ "waiting": waiting }))).into_response()
}
