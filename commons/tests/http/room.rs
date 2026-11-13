//! Open to the room (decision record 0027): the door in front of a node or a
//! local commons that listens on the room's Wi-Fi.
//!
//! These tests set the sender's address directly, so they need no network:
//! `127.0.0.1` is the owner at their own computer, and `192.168.1.50` is
//! someone else in the room.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use axum::Router;

use crate::common::*;
use hic_commons::role::Role;

/// The owner, at the server's own computer.
const OWNER: &str = "127.0.0.1:50000";
/// Someone else on the room's network.
const ROOM: &str = "192.168.1.50:50000";

/// `req`, marked as sent from `addr` (what the real server learns from the
/// connection).
fn from(addr: &str, mut req: Request<Body>) -> Request<Body> {
    let addr: SocketAddr = addr.parse().unwrap();
    req.extensions_mut().insert(ConnectInfo(addr));
    req
}

/// A server open to the room in `role` (a commons is a local commons).
/// Requests that don't say where they come from are marked as the owner's,
/// so the shared helpers (`join_and_publish`) work as they do on any node.
fn open_to_the_room(role: Role) -> (Router, hic_commons::store::Store) {
    // A commons with the switch is a local commons (`HIC_PRACTICE`).
    let practice = if role == Role::Commons { "1" } else { "0" };
    let settings = [("HIC_ROOM", "1"), ("HIC_ROOM_ADDRESS", "http://192.168.1.23:8080"), ("HIC_PRACTICE", practice)];
    let (app, store) = server_with(role, &settings);
    let app = app.layer(axum::middleware::map_request(|mut req: Request<Body>| async move {
        if req.extensions().get::<ConnectInfo<SocketAddr>>().is_none() {
            req.extensions_mut().insert(ConnectInfo(OWNER.parse::<SocketAddr>().unwrap()));
        }
        req
    }));
    (app, store)
}

/// Write a story with `visibility` as `handle` (signed in as `sid`) and
/// return its id.
async fn write_story(app: &Router, handle: &str, sid: &str, title: &str, visibility: &str) -> String {
    let csrf = csrf_in(&send(app, get("/stories/new", Some(sid))).await.text);
    let body = format!(
        "handle={handle}&csrf={csrf}&title={title}&text=Words&visibility={visibility}&license=cc-by&signals=credit"
    );
    let r = send(app, post_form("/stories/new", &body, Some(sid))).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text);
    let loc = r.headers.get("location").unwrap().to_str().unwrap();
    loc.strip_prefix("/story/").unwrap().split(['/', '?']).next().unwrap().to_string()
}

#[tokio::test]
async fn a_room_visitor_sees_a_public_profile_but_not_the_sign_in_page() {
    let (node, _) = open_to_the_room(Role::Node);
    join_and_publish(&node, "ana", "public").await;

    let profile = send(&node, from(ROOM, get("/@ana", None))).await;
    assert_eq!(profile.status, StatusCode::OK);
    assert!(profile.text.contains("@ana"));

    for path in ["/login", "/join", "/restore", "/new", "/steward", "/api/pending", "/@ana/x"] {
        let r = send(&node, from(ROOM, get(path, None))).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{path}");
        assert!(r.text.contains("This page of Test stays on its owner's computer"), "{path} says why");
    }
    // The owner, at their own computer, still has everything.
    assert_eq!(send(&node, from(OWNER, get("/login", None))).await.status, StatusCode::OK);
}

#[tokio::test]
async fn nothing_can_be_sent_from_the_room() {
    let (node, _) = open_to_the_room(Role::Node);
    let body = "handle=mal&display_name=M&passphrase=supersecret1&passphrase_again=supersecret1";
    let r = send(&node, from(ROOM, post_form("/join", body, None))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(session_from(&r).is_none(), "no account was made");
    let r = send(&node, from(ROOM, post_form("/login", "handle=ana&passphrase=guess", None))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "no passphrase guessing from the room");
}

#[tokio::test]
async fn a_members_only_story_stays_hidden_from_the_room() {
    let (node, _) = open_to_the_room(Role::Node);
    let sid = join_and_publish(&node, "ana", "public").await;
    let public = write_story(&node, "ana", &sid, "Open+book", "public").await;
    let private = write_story(&node, "ana", &sid, "Diary", "members").await;

    let r = send(&node, from(ROOM, get(&format!("/story/{public}"), None))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text.contains("Open book"));

    let r = send(&node, from(ROOM, get(&format!("/story/{private}"), None))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let list = send(&node, from(ROOM, get("/stories", None))).await;
    assert!(!list.text.contains("Diary"), "not listed either");
    // The story's tools stay on the owner's computer.
    let edit = send(&node, from(ROOM, get(&format!("/story/{public}/edit"), None))).await;
    assert_eq!(edit.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_visitor_through_a_proxy_on_this_computer_is_a_room_visitor() {
    let (node, _) = open_to_the_room(Role::Node);
    let mut req = from(OWNER, get("/login", None));
    req.headers_mut().insert("x-forwarded-for", "203.0.113.9".parse().unwrap());
    assert_eq!(send(&node, req).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn without_hic_room_nothing_changes() {
    let (node, _) = server(Role::Node);
    let r = send(&node, from(ROOM, get("/login", None))).await;
    assert_eq!(r.status, StatusCode::OK, "the door only exists when the room is open");
}

#[tokio::test]
async fn the_room_cannot_open_the_steward_view_of_a_local_commons() {
    let (commons, _) = open_to_the_room(Role::Commons);
    for path in ["/steward", "/steward?key=guess", "/room"] {
        assert_eq!(send(&commons, from(ROOM, get(path, None))).await.status, StatusCode::FORBIDDEN, "{path}");
    }
    // Everything a commons visitor normally reaches still works.
    assert_eq!(send(&commons, from(ROOM, get("/", None))).await.status, StatusCode::OK);
    assert_eq!(send(&commons, from(ROOM, get("/health", None))).await.status, StatusCode::OK);
}

#[tokio::test]
async fn a_node_open_to_the_room_may_still_share_to_the_room() {
    // Decision record 0012 refuses local-network addresses on a networked
    // node; a node open to the room is personal (only its owner can press
    // Share), so a commons at a room address isn't refused as "inside".
    let (node, _) = open_to_the_room(Role::Node);
    let sid = join_and_publish(&node, "ana", "public").await;
    let csrf = csrf_in(&send(&node, get("/@ana?tab=account", Some(&sid))).await.text);
    // A commons on the same laptop, and one elsewhere on the room's network.
    // (Port 9 answers nothing, so the share fails, but not as "inside".)
    for target in ["http://127.0.0.1:9", "http://192.168.0.0:9"] {
        let body = format!("handle=ana&csrf={csrf}&commons_url={target}");
        let r = send(&node, post_form("/share", &body, Some(&sid))).await;
        assert!(!r.text.contains("inside its own network"), "{target} is not refused as an internal address");
    }
    // Never the cloud credentials address, even from the owner.
    let body = format!("handle=ana&csrf={csrf}&commons_url=http://169.254.169.254");
    let r = send(&node, post_form("/share", &body, Some(&sid))).await;
    assert!(r.text.contains("inside its own network"), "link-local stays refused");
}

#[tokio::test]
async fn the_owner_sees_the_qr_code_and_the_address_for_the_room() {
    let (node, _) = open_to_the_room(Role::Node);
    join_and_publish(&node, "ana", "public").await;
    let page = send(&node, from(OWNER, get("/room", None))).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text.contains("<code>http://192.168.1.23:8080/@ana</code>"), "the address as real text");
    assert!(page.text.contains("<svg"), "the QR code");
    assert!(page.text.contains(r#"aria-label="QR code for http://192.168.1.23:8080/@ana""#));
    assert!(page.text.contains(r#"href="/room/help""#));
}

#[tokio::test]
async fn the_room_cannot_see_the_room_page_but_can_check_and_get_help() {
    for role in [Role::Node, Role::Commons] {
        let (app, _) = open_to_the_room(role);
        assert_eq!(send(&app, from(ROOM, get("/room", None))).await.status, StatusCode::FORBIDDEN, "{role:?}");
        let check = send(&app, from(ROOM, get("/room/check", None))).await;
        assert_eq!(check.status, StatusCode::OK, "{role:?}");
        assert!(check.text.contains("You reached Test."));
        assert_eq!(send(&app, from(OWNER, get("/room/check", None))).await.status, StatusCode::OK);
        let help = send(&app, from(ROOM, get("/room/help", None))).await;
        assert!(help.text.contains("client isolation"), "{role:?}");
    }
}

#[tokio::test]
async fn a_closed_server_says_so_on_its_room_page() {
    let (node, _) = server(Role::Node);
    let page = send(&node, get("/room", None)).await;
    assert!(page.text.contains("closed to the room"));
    assert!(!page.text.contains("<svg"), "no code to scan while closed");
}

#[tokio::test]
async fn a_local_commons_qr_code_leads_to_its_front_page() {
    let (commons, _) = open_to_the_room(Role::Commons);
    let page = send(&commons, from(OWNER, get("/room", None))).await;
    assert!(page.text.contains("<code>http://192.168.1.23:8080/</code>"));
}

#[tokio::test]
async fn a_full_commons_has_no_room_pages() {
    let (commons, _) = server(Role::Commons);
    for path in ["/room", "/room/check", "/room/help"] {
        assert_eq!(send(&commons, get(path, None)).await.status, StatusCode::NOT_FOUND, "{path}");
    }
}

/// A local commons open to the room that needs vouching, with room code
/// `code`, and its store. Without `practice` it's a full commons given the
/// same settings, which must ignore the code.
fn room_commons(code: &str, practice: bool) -> (Router, hic_commons::store::Store) {
    let dir = std::env::temp_dir().join(format!("hic-http-roomcode-{}", hic_commons::sessions::random_hex(6)));
    let store = hic_commons::store::Store::open(&dir).unwrap();
    let code = code.to_string();
    let config = hic_commons::config::Config::from_lookup(move |k| match k {
        "HIC_ROOM" => Some("1".into()),
        "HIC_PRACTICE" if practice => Some("1".into()),
        "HIC_ROOM_CODE" => Some(code.clone()),
        "HIC_ROOM_ADDRESS" => Some("http://192.168.1.23:8082".into()),
        _ => None,
    });
    let state = std::sync::Arc::new(hic_commons::app::AppState::new(config, Role::Commons, store.clone()));
    (hic_commons::app::router(state), store)
}

/// Share `handle`'s profile to `commons` from `addr` with `code` in the
/// invite box, as a node does; returns the status.
async fn share_with_code(commons: &Router, handle: &str, code: &str, addr: &str) -> StatusCode {
    let message = share_of(&person(handle), Some(code));
    send(commons, from(addr, post_json("/inbox", &message))).await.status
}

/// Is `handle` listed on the commons?
fn is_listed(store: &hic_commons::store::Store, handle: &str) -> bool {
    store.remotes().iter().any(|r| r.handle == handle)
}

#[tokio::test]
async fn the_room_code_lists_you_straight_away() {
    let (commons, store) = room_commons("KTRMWX", true);
    assert_eq!(
        share_with_code(&commons, "ana", "ktr mwx", ROOM).await,
        StatusCode::ACCEPTED,
        "typed as people type it"
    );
    assert!(is_listed(&store, "ana"));
    assert_eq!(store.intros()[0].route, "room-code", "recorded as coming in with the room code");
    let page = send(&commons, from(OWNER, get("/room", None))).await.text;
    assert!(page.contains("KTR MWX"), "the host's room page shows the code");
}

#[tokio::test]
async fn a_wrong_room_code_lists_nobody_and_guessing_is_limited() {
    let (commons, store) = room_commons("KTRMWX", true);
    for _ in 0..5 {
        assert_eq!(share_with_code(&commons, "mal", "ABCDEF", ROOM).await, StatusCode::FORBIDDEN);
    }
    assert_eq!(share_with_code(&commons, "mal", "ABCDEF", ROOM).await, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        share_with_code(&commons, "mal", "KTRMWX", ROOM).await,
        StatusCode::TOO_MANY_REQUESTS,
        "even the right code waits once the limit is reached"
    );
    assert!(!is_listed(&store, "mal"));
    assert_eq!(
        share_with_code(&commons, "ben", "KTRMWX", "192.168.1.51:50000").await,
        StatusCode::ACCEPTED,
        "others aren't held up"
    );
}

#[tokio::test]
async fn an_old_room_code_fails_once_the_room_reopens() {
    // Closing and reopening restarts the commons with a new code.
    let (reopened, store) = room_commons("PQRSTU", true);
    assert_eq!(share_with_code(&reopened, "ana", "KTRMWX", ROOM).await, StatusCode::FORBIDDEN);
    assert!(!is_listed(&store, "ana"));
}

#[tokio::test]
async fn a_full_commons_ignores_a_room_code() {
    let (commons, store) = room_commons("KTRMWX", false);
    let status = share_with_code(&commons, "ana", "KTRMWX", ROOM).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!is_listed(&store, "ana"), "only a local commons open to the room takes one");
}

#[tokio::test]
async fn guessing_from_many_addresses_stops_the_code_until_the_room_reopens() {
    let (commons, store) = room_commons("KTRMWX", true);
    let max = hic_commons::app::ROOM_CODE_MAX_WRONG;
    // Spread across addresses, so no one address reaches its own limit.
    for n in 0..max {
        let addr = format!("10.0.{}.{}:50000", n / 200, n % 200 + 1);
        assert_eq!(share_with_code(&commons, "mal", "ABCDEF", &addr).await, StatusCode::FORBIDDEN);
    }
    let r = send(&commons, from(ROOM, post_json("/inbox", &share_of(&person("ana"), Some("KTRMWX"))))).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "even the right code, until the host reopens the room");
    assert!(r.text.contains("ask the host"));
    assert!(!is_listed(&store, "ana"));
}

#[tokio::test]
async fn the_room_code_needs_a_signed_hic_v1_share() {
    let (commons, store) = room_commons("KTRMWX", true);
    let mut older = share_from_node("ana", "hello").await;
    older["invite"] = serde_json::json!("KTRMWX");
    let r = send(&commons, from(ROOM, post_json("/inbox", &older))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(!is_listed(&store, "ana"), "the older format leaves some of what's shown unsigned");
}
