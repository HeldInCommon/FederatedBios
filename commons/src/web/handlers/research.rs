//! Sharing terms for research (decision record 0029), both ends:
//! `POST /research/terms` on a commons keeps a gift as a file in `terms/`,
//! and [`send_terms_gift`] is how a node sends one there.
//!
//! What a gift may hold, and why it holds nothing about the person, is in
//! [`crate::research`].

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Extension;

use super::post_to_commons_at;
use crate::app::{AppState, Shared};
use crate::now;
use crate::research::{self, ReceivedGift, TermsGift};

/// `POST /research/terms` — on a commons, keep one gift of terms as a new
/// file in `terms/` (see `Store::save_terms_gift`). Writes that file. A node
/// answers "not found". The sender's address is used only for the rate
/// limit, shared with `/inbox`, and is never stored.
pub async fn receive_terms(
    State(st): State<Shared>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    body: String,
) -> Response {
    if !st.site.is_commons() {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    let sender = peer.map(|Extension(ConnectInfo(a))| a.ip().to_string()).unwrap_or_else(|| "unknown".into());
    if !st.inbox_limit.record_if_under_limit(&sender, now()) {
        return (StatusCode::TOO_MANY_REQUESTS, "too many requests from this address; try again in a minute")
            .into_response();
    }
    // `deny_unknown_fields` refuses a gift carrying anything not on the list.
    let gift: TermsGift = match serde_json::from_str(&body) {
        Ok(g) => g,
        Err(_) => return (StatusCode::BAD_REQUEST, "not a gift of terms").into_response(),
    };
    if let Err(why) = research::check(&gift) {
        return (StatusCode::BAD_REQUEST, why.message()).into_response();
    }
    match st.store.save_terms_gift(&ReceivedGift { received_at: now(), gift }) {
        Ok(_) => (StatusCode::CREATED, "thank you").into_response(),
        Err(e) => {
            eprintln!("error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not write the data folder").into_response()
        }
    }
}

/// Send `gift` to `destination` (`POST <destination>/research/terms`), with
/// the same address check as every outgoing request (decision record 0012).
/// `Ok` when the commons kept it; an error message otherwise. Sends one
/// request; changes nothing on this node.
pub(crate) async fn send_terms_gift(st: &AppState, destination: &str, gift: &TermsGift) -> Result<(), String> {
    let body = serde_json::to_value(gift).expect("a gift is plain strings, a bool and a list");
    let reply = post_to_commons_at(st, destination, research::PATH, &body, std::time::Duration::from_secs(10)).await?;
    if reply.status().is_success() {
        Ok(())
    } else {
        Err(format!("the research address answered {}", reply.status()))
    }
}
