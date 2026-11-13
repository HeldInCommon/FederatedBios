//! Request handlers for the HTML side of the server: one file per page or
//! flow, plus the helpers they share (below).
//!
//! Each handler does the same three things: load what it needs (store,
//! session), call pure functions for every decision, then render. Writes that
//! read a record, change it and save it back happen while holding
//! [`AppState::write`], so two requests can't interleave. Helpers that write
//! say so in their first line.
//!
//! | File | Routes |
//! |---|---|
//! | `directory.rs` | `GET /` |
//! | `profile.rs` | `GET /@{handle}` |
//! | `notices.rs` | `POST /waiting/check`, `POST /notices/dismiss` |
//! | `signin.rs` | `/join`, `/login`, `POST /logout` |
//! | `publish.rs` | `GET /new`, `POST /publish` |
//! | `share.rs` | `POST /share`, `POST /withdraw`, and sending to a commons |
//! | `inbox.rs` | `POST /inbox`: what a commons receives from nodes |
//! | `here.rs` | "Only while I'm here" on a commons: `POST /here/card`, `POST /here/poll`, reading a card's story |
//! | `backup.rs` | `POST /backup`, `/restore` |
//! | `federated.rs` | `GET /federated/{keyid}` |
//! | `about.rs` | `GET /about`, `GET /run-a-commons` |
//! | `account.rs` | recovery key, replacing and recovering a key |
//! | `membership.rs` | invite codes and vouches, from a member's node (`POST /invite`, `POST /vouch`) |
//! | `waiting_list.rs` | `POST /api/waiting`: a commons shows its members who is waiting for a vouch |
//! | `room.rs` | open to the room: `GET /room` (the owner's QR code), `GET /room/check`, `GET /room/help` |
//! | `steward.rs` | the steward view of a commons |
//! | `stories.rs` | `GET /stories` (a node's, or a commons' with recent withdrawals), `GET /stories/{keyid}/{id}` |
//! | `story.rs` | one story on its author's node: `GET /story/{id}`, export, `POST /story/share`, `POST /story/withdraw` |
//! | `story_edit.rs` | writing and changing a story: `/stories/new`, `/story/{id}/edit`, `/story/{id}/rights` |

use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use crate::app::AppState;
use crate::form::Form;
use crate::model::{HandleRec, Profile, Reach, Version};
use crate::sessions::{self, Session};
use crate::{keys, now};

mod about;
mod backup;
mod directory;
mod federated;
mod here;
mod inbox;
mod notices;
mod profile;
mod publish;
mod room;
mod share;
mod signin;
mod stories;
mod story;
mod story_edit;
mod waiting_list;

pub mod account;
pub mod membership;
pub mod steward;

pub use about::{about, run_a_commons};
pub use backup::{backup, backup_later, restore_form, restore_submit};
pub use directory::directory;
pub use federated::federated;
pub use here::{here_card, here_json, here_poll};
pub use inbox::inbox;
pub(crate) use inbox::list_entry_response;
pub use notices::{check_waiting, dismiss_notices};
pub use profile::profile;
pub use publish::{clean_avatar, profile_from_form, publish, publish_form};
pub use room::{room, room_check, room_help};
pub(crate) use share::{post_to_commons, post_to_commons_at, send_share};
pub use share::{share, withdraw};
pub use signin::{join_form, join_submit, login_form, login_submit, logout};
pub use stories::{held_story, stories};
pub use story::{own_story, story_share, story_text, story_withdraw};
pub use story_edit::{rights_form, rights_submit, story_create, story_edit, story_edit_form, story_form};
pub use waiting_list::waiting_list;

/// The signed-in session for this request, if any.
pub(crate) fn session(st: &AppState, headers: &HeaderMap) -> Option<Session> {
    sessions::token_from(headers, &st.config.session_cookie).and_then(|t| st.sessions.get(&t, now()))
}

/// True when the request is signed in as `handle` and carries that session's
/// CSRF token in the form.
pub(crate) fn authorised_as(st: &AppState, headers: &HeaderMap, form: &Form, handle: &str) -> bool {
    session(st, headers)
        .is_some_and(|s| s.handle == handle && !s.csrf.is_empty() && keys::ct_eq(&s.csrf, &form.one("csrf")))
}

/// The profile in `v` that `viewer` may read: the payload for public and
/// commons versions; for members-only, the copy encrypted to the viewer.
pub(crate) fn readable_profile(st: &AppState, v: &Version, viewer: Option<&str>) -> Option<Profile> {
    match v.reach() {
        Reach::Members => {
            let viewer = viewer?;
            let vrec = st.store.handle(viewer).ok()??;
            keys::open_for(v.enc.as_deref()?, viewer, &vrec.box_priv)
        }
        _ => v.payload.clone(),
    }
}

/// Load member `h`, apply the pure `change` to their record, and save the
/// result, all under the write lock. **Writes the member's file.** Returns
/// why it couldn't, for the caller to handle.
pub(crate) fn update_member(st: &AppState, h: &str, change: impl FnOnce(HandleRec) -> HandleRec) -> Result<(), String> {
    let _g = st.write.lock().unwrap();
    let rec = st.store.handle(h).map_err(|e| e.to_string())?.ok_or("no such member")?;
    st.store.save_handle(&change(rec)).map_err(|e| e.to_string())
}

/// Add a `Set-Cookie` header to a response.
pub(crate) fn with_cookie(mut resp: Response, cookie: String) -> Response {
    if let Ok(v) = axum::http::HeaderValue::from_str(&cookie) {
        resp.headers_mut().insert(axum::http::header::SET_COOKIE, v);
    }
    resp
}

/// Small extension so a page response can carry a different status.
pub(crate) trait WithStatus {
    /// This response with `status` instead of its own.
    fn into_response_with(self, status: StatusCode) -> Response;
}

impl WithStatus for Response {
    /// Replaces the status and returns the same response.
    fn into_response_with(mut self, status: StatusCode) -> Response {
        *self.status_mut() = status;
        self
    }
}
