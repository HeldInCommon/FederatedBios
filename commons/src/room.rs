//! Open to the room (decision record 0027): who may reach what while a
//! server listens on the room's Wi-Fi.
//!
//! In a classroom or a gallery, a person can open their node (or the host
//! their local commons) to everyone on the same router. The server then
//! listens on every network (`0.0.0.0`) instead of only on this computer
//! (`127.0.0.1`), and the **door** in this module stands in front of every
//! request:
//!
//! - The **owner** (a request from this computer, `127.0.0.1` or `::1`) gets
//!   everything, exactly as before.
//! - A **room visitor** on a **node** gets only what's already public:
//!   profiles, public stories, the stylesheet and pictures, and the room
//!   check page. The sign-in page, keys, forms and every `POST` stay on the
//!   owner's computer. The list says what's allowed; anything not on it is
//!   refused, so a page added later is closed to the room until someone adds
//!   it here on purpose.
//! - A room visitor on a **local commons** gets what any commons visitor
//!   gets, except the host's own pages (the steward view, `/room` and the
//!   app's notices).
//!
//! Where a request comes from is only trusted when nothing forwards traffic
//! to the server: a proxy on this computer (Tailscale Funnel, `tailscale
//! serve`, nginx) makes every outside visitor arrive from `127.0.0.1`
//! (decision record 0023). A proxy usually adds `X-Forwarded-For`,
//! `Forwarded`, `X-Real-IP`, `X-Forwarded-Host`, `X-Forwarded-Proto` or
//! `Via`, and a browser never does, so a request carrying one is treated as
//! a room visitor even when it comes from this computer. A forwarder that
//! adds none of them can't be seen from here; the app refuses to open the
//! room on a port Tailscale forwards to, and the guide says not to forward
//! ports to the app.
//!
//! Everything here is pure, so each rule has a unit test below. The
//! middleware that applies it is [`guard`].

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::Response;

use crate::app::Shared;
use crate::role::Role;

/// Who is asking, as far as the door is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visitor {
    /// Someone at this computer: the person who opened the room.
    Owner,
    /// Anyone else on the room's network, or anyone arriving through a proxy.
    Room,
}

/// What the door says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    /// Let the request through to its page.
    In,
    /// Refuse it: this page is only on the owner's own computer.
    OnlyHere,
}

/// Headers a proxy adds when it passes a request on. A browser never sends
/// these itself, so their presence means "not really from this computer".
const FORWARDING_HEADERS: [&str; 6] =
    ["x-forwarded-for", "forwarded", "x-real-ip", "x-forwarded-host", "x-forwarded-proto", "via"];

/// True when the request carries any header a proxy adds (see
/// `FORWARDING_HEADERS`). Pure.
pub fn is_forwarded(headers: &HeaderMap) -> bool {
    FORWARDING_HEADERS.iter().any(|name| headers.contains_key(*name))
}

/// True for this computer's own addresses: `127.0.0.0/8`, `::1`, and the
/// IPv6 spelling of an IPv4 loopback address (`::ffff:127.0.0.1`). Pure.
fn is_this_computer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback()),
    }
}

/// Who a request is from: the owner only when it came from this computer and
/// no proxy passed it on. An unknown sender counts as a room visitor, so a
/// mistake closes the door rather than opening it. Pure.
pub fn visitor(peer: Option<IpAddr>, forwarded: bool) -> Visitor {
    match peer {
        Some(ip) if is_this_computer(ip) && !forwarded => Visitor::Owner,
        _ => Visitor::Room,
    }
}

/// Pages and files on a **node** that a room visitor may fetch with `GET`,
/// matched exactly. Profiles, stories and quilts are matched by shape in
/// [`node_lets_in`].
const NODE_OPEN_PATHS: [&str; 13] = [
    "/",
    "/about",
    "/run-a-commons",
    "/stories",
    "/style.css",
    "/logo.png",
    "/avatar.js",
    "/password.js",
    "/robots.txt",
    "/health",
    "/room/check",
    "/room/help",
    "/favicon.ico",
];

/// True when `path` is `prefix` followed by exactly one non-empty segment
/// with no further `/`: `one_segment_after("/@", "/@ana")` is true, and
/// `"/@ana/edit"` or `"/@"` are false. Pure.
fn one_segment_after(prefix: &str, path: &str) -> bool {
    path.strip_prefix(prefix).is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
}

/// May a room visitor fetch `path` from a node? Only public pages: the
/// front page, a profile (`/@ana`), the story list and one story
/// (`/story/<id>`, which still hides members-only stories from anyone not
/// signed in), and the files those pages load. Anything else, including
/// `/story/<id>/edit`, `/login`, `/join`, `/restore`, `/api/…` and
/// `/steward`, stays on the owner's computer. Pure.
fn node_lets_in(path: &str) -> bool {
    NODE_OPEN_PATHS.contains(&path)
        || one_segment_after("/@", path)
        || one_segment_after("/story/", path)
        || one_segment_after("/quilt/", path)
}

/// True for the pages on a commons that belong to whoever runs it: the
/// steward view and its actions, the room pages (which carry the QR code and
/// the room code) except `/room/check` and `/room/help`, and the desktop
/// app's notices. Pure.
fn commons_owner_only(path: &str) -> bool {
    let room_page = path == "/room" || (path.starts_with("/room/") && !ROOM_PAGES_FOR_EVERYONE.contains(&path));
    path == "/steward" || path.starts_with("/steward/") || room_page || path == "/api/notices/claim"
}

/// The room pages anyone in the room may open, on a node or a commons: the
/// connection check and "Can't reach someone?".
const ROOM_PAGES_FOR_EVERYONE: [&str; 2] = ["/room/check", "/room/help"];

/// The door's answer for one request. Pure.
///
/// - The owner always gets in.
/// - On a node, a room visitor gets in only to read a public page
///   (`GET` or `HEAD`, see `node_lets_in`). Every other method is refused,
///   so no form can be sent from the room.
/// - On a commons, a room visitor gets in everywhere except the owner's
///   pages (see `commons_owner_only`): taking visitors in is what a
///   commons is for.
pub fn door(role: Role, who: Visitor, method: &Method, path: &str) -> Door {
    let allowed = match (who, role) {
        (Visitor::Owner, _) => true,
        (Visitor::Room, Role::Node) => (method == Method::GET || method == Method::HEAD) && node_lets_in(path),
        (Visitor::Room, Role::Commons) => !commons_owner_only(path),
    };
    if allowed {
        Door::In
    } else {
        Door::OnlyHere
    }
}

/// Middleware: ask the [`door`] about every request, and answer a refused
/// one with the "only on the owner's computer" page (status 403) instead of
/// passing it on. The router adds this only when `HIC_ROOM=1`.
///
/// The sender's address is in the request's extensions when the server was
/// started with connection info (`main.rs`); without it (as in some tests)
/// the sender counts as a room visitor.
pub async fn guard(State(st): State<Shared>, request: Request, next: Next) -> Response {
    let peer = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|ConnectInfo(addr)| addr.ip());
    let who = visitor(peer, is_forwarded(request.headers()));
    match door(st.site.role, who, request.method(), request.uri().path()) {
        Door::In => next.run(request).await,
        Door::OnlyHere => crate::web::layout::only_here(&st.site),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A room visitor's address on a typical classroom router.
    fn room() -> Visitor {
        visitor(Some("192.168.1.50".parse().unwrap()), false)
    }

    /// The owner, at this computer.
    fn owner() -> Visitor {
        visitor(Some("127.0.0.1".parse().unwrap()), false)
    }

    #[test]
    fn the_owner_reaches_everything() {
        for path in ["/login", "/join", "/restore", "/steward", "/api/pending", "/story/x/edit", "/room"] {
            assert_eq!(door(Role::Node, owner(), &Method::GET, path), Door::In, "{path}");
            assert_eq!(door(Role::Commons, owner(), &Method::GET, path), Door::In, "{path}");
        }
        assert_eq!(door(Role::Node, owner(), &Method::POST, "/publish"), Door::In);
    }

    #[test]
    fn ipv6_loopback_and_its_ipv4_spelling_are_the_owner() {
        assert_eq!(visitor(Some("::1".parse().unwrap()), false), Visitor::Owner);
        assert_eq!(visitor(Some("::ffff:127.0.0.1".parse().unwrap()), false), Visitor::Owner);
    }

    #[test]
    fn a_room_visitor_cannot_reach_the_sign_in_page() {
        assert_eq!(door(Role::Node, room(), &Method::GET, "/login"), Door::OnlyHere);
        assert_eq!(door(Role::Node, room(), &Method::GET, "/join"), Door::OnlyHere);
        assert_eq!(door(Role::Node, room(), &Method::GET, "/restore"), Door::OnlyHere);
    }

    #[test]
    fn a_room_visitor_reads_public_pages_on_a_node() {
        for path in [
            "/",
            "/@ana",
            "/stories",
            "/story/abc123",
            "/style.css",
            "/password.js",
            "/run-a-commons",
            "/quilt/3.svg",
            "/health",
            "/room/check",
            "/room/help",
        ] {
            assert_eq!(door(Role::Node, room(), &Method::GET, path), Door::In, "{path}");
        }
    }

    #[test]
    fn an_unknown_path_is_refused_by_default() {
        for path in
            ["/new-page", "/api/directory", "/story/x/edit", "/story/x/text", "/@ana/x", "/@", "/room", "//login"]
        {
            assert_eq!(door(Role::Node, room(), &Method::GET, path), Door::OnlyHere, "{path}");
        }
    }

    #[test]
    fn a_post_from_the_room_is_refused() {
        for path in ["/login", "/publish", "/share", "/", "/@ana"] {
            assert_eq!(door(Role::Node, room(), &Method::POST, path), Door::OnlyHere, "{path}");
        }
    }

    #[test]
    fn a_forwarded_request_is_not_the_owner() {
        let through_a_proxy = visitor(Some("127.0.0.1".parse().unwrap()), true);
        assert_eq!(through_a_proxy, Visitor::Room);
        assert_eq!(door(Role::Node, through_a_proxy, &Method::GET, "/login"), Door::OnlyHere);
    }

    #[test]
    fn proxy_headers_are_recognised() {
        for name in FORWARDING_HEADERS {
            let mut headers = HeaderMap::new();
            headers.insert(name, "203.0.113.9".parse().unwrap());
            assert!(is_forwarded(&headers), "{name}");
        }
        assert!(!is_forwarded(&HeaderMap::new()));
    }

    #[test]
    fn an_unknown_sender_is_a_room_visitor() {
        assert_eq!(visitor(None, false), Visitor::Room);
    }

    #[test]
    fn a_room_visitor_cannot_open_the_steward_view_of_a_local_commons() {
        for path in ["/steward", "/steward/approve", "/room", "/room/qr.svg", "/api/notices/claim"] {
            assert_eq!(door(Role::Commons, room(), &Method::GET, path), Door::OnlyHere, "{path}");
        }
    }

    #[test]
    fn a_room_visitor_uses_a_local_commons_as_usual() {
        assert_eq!(door(Role::Commons, room(), &Method::GET, "/"), Door::In);
        assert_eq!(door(Role::Commons, room(), &Method::POST, "/inbox"), Door::In);
        assert_eq!(door(Role::Commons, room(), &Method::GET, "/room/check"), Door::In);
        assert_eq!(door(Role::Commons, room(), &Method::GET, "/stewardship"), Door::In, "only /steward itself");
    }
}
