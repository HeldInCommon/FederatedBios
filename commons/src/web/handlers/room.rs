//! Open to the room (decision record 0027): `GET /room` (the owner's page
//! with the QR code), `GET /room/check` and `GET /room/help`.
//!
//! These pages exist only on a node or a local commons, the servers with a
//! switch; a full commons answers "not found". The door (`crate::room`)
//! keeps `/room` for the owner while the server is open to the room; while
//! it's closed, the desktop app's server listens only on this computer.
//! Someone running a server by hand on the network without `HIC_ROOM` sees
//! only "closed to the room" there, with no address or code.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;

use super::session;
use crate::app::{AppState, Shared};
use crate::web::layout::{not_found, render, Page};
use crate::web::{pages, qr};

/// Where the QR code on a node's room page leads: the member's profile when
/// the node holds exactly one member whose profile the room can see,
/// otherwise the node's front page. `public` lists the handles with a
/// published profile anyone may read. Pure.
pub fn room_path(handles: &[String], public: &[String]) -> String {
    match handles {
        [only] if public.contains(only) => format!("/@{only}"),
        _ => "/".to_string(),
    }
}

/// True for the servers with room pages: a node or a local commons always
/// (they have the switch, decision record 0027, D7), and a full commons
/// while it's open to the room.
fn has_room_pages(st: &AppState) -> bool {
    !st.site.is_commons() || st.site.practice || st.config.room
}

/// The address people in the room open, while the server is open to the
/// room: the room address the app gave it, plus the page the QR code leads
/// to. On a commons with a room code that's its join link (`/join/KTRMWX`),
/// which a node turns into the address and the code in one paste; on a
/// node, the member's profile or the front page. `None` while closed.
fn room_address(st: &AppState) -> Option<String> {
    let base = st.config.room_address.as_deref().filter(|_| st.config.room)?;
    let path = if st.site.is_commons() {
        st.config.room_code.as_deref().map_or("/".to_string(), |code| format!("/join/{code}"))
    } else {
        let records = st.store.handles();
        let handles: Vec<String> = records.iter().map(|rec| rec.handle.clone()).collect();
        // Readable without signing in: a published profile that isn't members-only.
        let public: Vec<String> = records
            .iter()
            .filter(
                |rec| matches!(st.store.current_version(rec), Ok(Some(v)) if v.reach() != crate::model::Reach::Members),
            )
            .map(|rec| rec.handle.clone())
            .collect();
        room_path(&handles, &public)
    };
    Some(qr::address(base, &path))
}

/// `GET /room` — the QR code and address to show the room, the room code on
/// a local commons, and how to close it.
pub async fn room(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    if !has_room_pages(&st) {
        return not_found(&st.site, me.as_deref());
    }
    let address = room_address(&st);
    let svg = address.as_deref().and_then(qr::svg);
    let view = pages::RoomView {
        name: &st.site.instance,
        is_commons: st.site.is_commons(),
        address: address.as_deref(),
        qr_svg: svg.as_deref(),
        room_code: st.config.room_code.as_deref().filter(|_| st.site.is_commons()),
        firewall_hint: st.config.firewall_hint.as_deref(),
    };
    render(&st.site, Page::new("Open to the room", me.as_deref(), pages::room(&view)))
}

/// `GET /join/{code}` on a commons open to the room: what someone sees who
/// opens the room's join link (by scanning its QR code, say): how to join
/// from the Held in Common app on their laptop, with the link to paste.
///
/// It doesn't check the code. If it said whether a code was right, anyone
/// could guess codes here without the limits a share has; the share checks
/// it (five wrong per address, two hundred in all, `handlers/inbox.rs`).
pub async fn join_room(State(st): State<Shared>, Path(code): Path<String>) -> Response {
    if !st.site.is_commons() || !st.config.room || !crate::vouching::looks_like_room_code(&code) {
        return not_found(&st.site, None);
    }
    let base = st.config.room_address.clone().unwrap_or_else(|| st.site.base_url.clone());
    let link = qr::address(&base, &format!("/join/{}", crate::vouching::tidy_room_code(&code)));
    render(&st.site, Page::new("Join this commons", None, pages::join_room(&st.site.instance, &link)))
}

/// `GET /room/check` — "It works: you reached …", for testing the connection
/// from a phone.
pub async fn room_check(State(st): State<Shared>) -> Response {
    if !has_room_pages(&st) {
        return not_found(&st.site, None);
    }
    render(&st.site, Page::new("It works", None, pages::room_check(&st.site.instance)))
}

/// `GET /room/help` — what to try when someone in the room can't connect.
pub async fn room_help(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    if !has_room_pages(&st) {
        return not_found(&st.site, me.as_deref());
    }
    render(&st.site, Page::new("Can't reach someone?", me.as_deref(), pages::room_help()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_qr_code_leads_to_the_only_member_or_the_front_page() {
        let ana = vec!["ana".to_string()];
        assert_eq!(room_path(&ana, &ana), "/@ana");
        assert_eq!(room_path(&ana, &[]), "/", "a members-only profile: the room couldn't open it");
        assert_eq!(room_path(&[], &[]), "/");
        assert_eq!(room_path(&["ana".to_string(), "ben".to_string()], &ana), "/");
    }
}
