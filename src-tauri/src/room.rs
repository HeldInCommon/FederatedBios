//! Open to the room (decision record 0027), the app's side: the rules for
//! the switch that opens a node or a local commons to everyone on the same
//! router, and closes it again. Pure: no window, no process, no network.
//!
//! - Whether a server is open lives only in memory (in its
//!   [`crate::sidecar::Sidecar`]), never in `app.json`, so every launch starts
//!   closed (D2).
//! - It can only be opened on a network: the room reaches it at this
//!   computer's address there.
//! - It isn't opened on a port that Tailscale (Funnel or `serve`) forwards
//!   to: a forwarder on this computer makes everyone on the internet look
//!   like the owner (D9, the lesson of decision record 0023).
//! - It closes when the computer's network address changes (D3): the laptop
//!   has probably left the room.

use crate::settings::Kind;

/// What a server open to the room is told besides its address: the room
/// code people type to join a local commons (D4), and a hint about this
/// computer's firewall for its room page (D5). Made fresh each time the room
/// opens; kept in memory only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomExtras {
    /// On a local commons: six letters from `ROOM_CODE_LETTERS`.
    pub code: Option<String>,
    /// What to do about the firewall, if there's anything (`crate::firewall`).
    pub firewall_hint: Option<String>,
}

/// The letters a room code is made of: capitals without I, L and O, which
/// are easy to misread off a projector. The server accepts the same set
/// (`ROOM_CODE_LETTERS` in `commons/src/vouching.rs`).
pub const ROOM_CODE_LETTERS: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ";

/// A fresh room code: six letters from [`ROOM_CODE_LETTERS`], from the
/// operating system's random source (as the launch token is). Each letter
/// comes from one random byte; bytes that would make some letters likelier
/// than others are thrown away and drawn again. Fails only if the system
/// can't give random bytes.
pub fn new_room_code() -> Result<String, rand::Error> {
    use rand::RngCore;
    let n = ROOM_CODE_LETTERS.len();
    // The largest multiple of `n` that fits in a byte: bytes below it map
    // evenly onto the letters.
    let fair = (256 / n * n) as u8;
    let mut code = String::new();
    let mut byte = [0u8; 1];
    while code.len() < 6 {
        rand::rngs::OsRng.try_fill_bytes(&mut byte)?;
        if byte[0] < fair {
            code.push(ROOM_CODE_LETTERS[byte[0] as usize % n] as char);
        }
    }
    Ok(code)
}

/// The settings a server open to the room gets from its `extras`: the room
/// code (a local commons only; a node has nobody to admit) and the firewall
/// hint. Pure.
pub fn room_env(kind: Kind, extras: &RoomExtras) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    if kind != Kind::Node {
        env.extend(extras.code.clone().map(|code| ("HIC_ROOM_CODE", code)));
    }
    env.extend(extras.firewall_hint.clone().map(|hint| ("HIC_FIREWALL_HINT", hint)));
    env
}

/// Can this kind of server be opened to the room? All three can. For a node
/// or a local commons it means listening on the room's network; a full
/// commons is reachable there already, so for it, it means taking a room
/// code (and showing the room page) until it's closed again.
pub fn has_switch(kind: Kind) -> bool {
    matches!(kind, Kind::Node | Kind::Practice | Kind::Commons)
}

/// Why a server can't be opened to the room right now.
#[derive(Debug, PartialEq, Eq)]
pub enum CantOpen {
    /// A full commons has no switch (see [`has_switch`]).
    NoSwitch,
    /// This computer has no network address, so the room couldn't reach it.
    NoNetwork,
    /// Tailscale forwards traffic to this port (D9).
    Forwarded,
    /// This computer's address isn't a room's: a VPN's or Tailscale's
    /// (`100.64.0.0/10`), or a public one. The room couldn't reach it, and
    /// listening there would let others in instead.
    NotARoomNetwork,
}

impl CantOpen {
    /// What to tell the person, in plain words.
    pub fn message(&self) -> &'static str {
        match self {
            CantOpen::NoSwitch => "A commons is already reachable on the network; there's nothing to open.",
            CantOpen::NoNetwork => "This computer isn't on a network. Join the room's Wi-Fi, then try again.",
            CantOpen::NotARoomNetwork => {
                "This computer's network address belongs to a VPN or Tailscale, not the room's Wi-Fi. \
                 Turn the VPN (or Tailscale's exit node) off, then try again."
            }
            CantOpen::Forwarded => {
                "Tailscale passes traffic from the internet to this server, which would let anyone \
                 online in as if they were you. Turn that off (tailscale serve reset) before opening it to the room."
            }
        }
    }
}

/// May `kind` be opened to the room, given this computer's address on the
/// network (`lan`) and whether Tailscale forwards to its port? Returns the
/// address the room will use, or why not. Pure.
pub fn check_open(kind: Kind, lan: Option<String>, forwarded: bool) -> Result<String, CantOpen> {
    if !has_switch(kind) {
        return Err(CantOpen::NoSwitch);
    }
    if forwarded {
        return Err(CantOpen::Forwarded);
    }
    let lan = lan.ok_or(CantOpen::NoNetwork)?;
    if !ip_of(&lan).is_some_and(|ip| is_room_network(&ip)) {
        return Err(CantOpen::NotARoomNetwork);
    }
    Ok(lan)
}

/// Is `ip` an address a room's router hands out: a private IPv4 range
/// (`10/8`, `172.16/12`, `192.168/16`) or a private IPv6 one (`fc00::/7`)?
/// Not `100.64/10` (Tailscale and carrier networks) and not a public address,
/// which is what a VPN or Tailscale exit node makes the computer use. Pure.
pub fn is_room_network(ip: &str) -> bool {
    match ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => v4.is_private(),
        Ok(std::net::IpAddr::V6(v6)) => v6.segments()[0] & 0xfe00 == 0xfc00,
        Err(_) => false,
    }
}

/// Does `tailscale serve status` (or `tailscale funnel status`, which prints
/// the same) show traffic forwarded to `port` on this computer? Looks at
/// each place `:<port>` appears (and the port ends there: `:8081` isn't
/// inside `:80810`), and checks whether the host just before it is this
/// computer, however it's written: any `127.x` address, `localhost`,
/// `[::1]`, `[::ffff:127.0.0.1]`, `0.0.0.0` or `[::]`. That covers both
/// `|-- / proxy http://127.0.0.1:8081` and the `--json` form. Pure.
pub fn forwards_to(status: &str, port: u16) -> bool {
    let needle = format!(":{port}");
    status.match_indices(&needle).any(|(at, _)| {
        let after = status[at + needle.len()..].chars().next();
        if after.is_some_and(|c| c.is_ascii_digit()) {
            return false;
        }
        is_this_computer(host_before(&status[..at]))
    })
}

/// The host written just before a `:port`: everything back to the last
/// `/`, space, quote or `=` (`http://127.0.0.1` gives `127.0.0.1`). A
/// bracketed IPv6 address keeps its brackets. Pure.
fn host_before(text: &str) -> &str {
    let start =
        text.rfind(|c: char| c == '/' || c == '"' || c == '\'' || c == '=' || c.is_whitespace()).map_or(0, |i| i + 1);
    &text[start..]
}

/// True for the ways of writing "this computer" a forwarder might use. Pure.
fn is_this_computer(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host.starts_with("127.")
        || host == "localhost"
        || host == "localhost."
        || host == "[::1]"
        || host.starts_with("[::ffff:127.")
        || host == "0.0.0.0"
        || host == "[::]"
}

/// The IP address in a room address (`http://192.168.1.23:8080` gives
/// `192.168.1.23`), which an open server listens on, so it isn't also
/// reachable over a VPN or Tailscale. Pure.
pub fn ip_of(url: &str) -> Option<String> {
    let host = url.split("://").nth(1)?.split('/').next()?;
    let ip = match host.rsplit_once(':') {
        Some((ip, port)) if port.chars().all(|c| c.is_ascii_digit()) => ip,
        _ => host,
    };
    ip.trim_start_matches('[').trim_end_matches(']').parse::<std::net::IpAddr>().ok().map(|ip| ip.to_string())
}

/// Should a server that's open to the room close, now that this computer's
/// network address is `now` and it opened on `opened_on`? Yes whenever they
/// differ, including when the network has gone (`now` is `None`): the laptop
/// has probably left the room, and the old address could be someone else's
/// on the next network (D3). Pure.
pub fn should_close(opened_on: Option<&str>, now: Option<&str>) -> bool {
    opened_on != now
}

/// What the switcher and the Go menu say about where a server is and who can
/// reach it, for a node or a local commons that may be open to the room.
/// `None` for a full commons, whose wording doesn't depend on a switch
/// (see `Saved::where_is`). Pure.
pub fn where_is(kind: Kind, open: bool) -> Option<&'static str> {
    match (kind, open) {
        (Kind::Node, false) => Some("on this computer, only you"),
        (Kind::Node, true) => Some("open to the room: public pages only"),
        (Kind::Practice, false) => Some("local commons, closed to the room"),
        (Kind::Practice, true) => Some("local commons, open to the room"),
        (Kind::Commons, true) => Some("commons, open to the room"),
        (Kind::Commons, false) => None,
    }
}

/// Where a server is and who can reach it, for the switcher and the Go
/// menu: the room's wording for a node or local commons, otherwise what
/// `app.json` says about the commons. Pure.
pub fn place_text(saved: &crate::settings::Saved, kind: Kind, open: bool) -> &'static str {
    where_is(kind, open).unwrap_or_else(|| saved.where_is(kind))
}

/// Who can reach a server, in a few words, for the chip on its card in the
/// sidebar ("Only you", "Open to the room"). The card's name already says
/// what it is, so the chip doesn't repeat it. Matches `Place::chip` in
/// `commons/src/web/layout.rs`, which draws the card you're on. Pure.
pub fn chip_text(saved: &crate::settings::Saved, kind: Kind, open: bool) -> &'static str {
    match (kind, open) {
        (_, true) => "Open to the room",
        (Kind::Node, false) => "Only you",
        (Kind::Practice, false) => "Closed to the room",
        (Kind::Commons, false) if saved.public_address.is_some() => "Anyone with its address",
        (Kind::Commons, false) => "Anyone on this Wi-Fi",
    }
}

/// What kind of server it is, for a card's second line ("Local commons").
/// Matches `Place::kind_name` in `commons/src/web/layout.rs`. Pure.
pub fn kind_text(saved: &crate::settings::Saved, kind: Kind) -> &'static str {
    match kind {
        Kind::Node => "Your node",
        Kind::Practice => "Local commons",
        Kind::Commons if saved.public_address.is_some() => "Online commons",
        Kind::Commons => "Commons on this Wi-Fi",
    }
}

/// The Go menu item that opens or closes `kind`, named `name`, for the room.
/// Pure.
pub fn menu_label(kind: Kind, name: &str, open: bool) -> String {
    let what = if kind == Kind::Node { "my node".to_string() } else { name.to_string() };
    if open {
        format!("Close {what} to the room")
    } else {
        format!("Open {what} to the room\u{2026}")
    }
}

/// The notice shown when a server closed because the network changed.
pub fn closed_notice(name: &str) -> String {
    format!(
        "{name} is closed to the room: this computer's network address changed. Open it again from This computer in the sidebar if you're still in the room."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `tailscale serve status` on the workshop machine, 13 November 2026.
    const SERVE_STATUS: &str = "\n# Funnel on:\n#     - https://heldincommon.cerberus-broadnose.ts.net\n\n\
        https://heldincommon.cerberus-broadnose.ts.net (Funnel on)\n|-- / proxy http://127.0.0.1:8081\n";

    /// The same, from `tailscale serve status --json`.
    const SERVE_JSON: &str = r#"{"TCP":{"443":{"HTTPS":true}},"Web":{"heldincommon.cerberus-broadnose.ts.net:443":
        {"Handlers":{"/":{"Proxy":"http://127.0.0.1:8081"}}}},"AllowFunnel":{"heldincommon.cerberus-broadnose.ts.net:443":true}}"#;

    #[test]
    fn a_port_tailscale_forwards_to_is_recognised() {
        assert!(forwards_to(SERVE_STATUS, 8081));
        assert!(forwards_to(SERVE_JSON, 8081));
        assert!(forwards_to("|-- tcp://localhost:8080", 8080));
        for other_spelling in ["127.0.0.2", "[::ffff:127.0.0.1]", "localhost.", "[::1]", "LOCALHOST", "0.0.0.0"] {
            assert!(forwards_to(&format!("|-- / proxy http://{other_spelling}:8080"), 8080), "{other_spelling}");
        }
    }

    #[test]
    fn forwarding_to_another_computer_is_not_this_one() {
        assert!(!forwards_to("|-- / proxy http://192.168.1.9:8080", 8080));
        assert!(!forwards_to("https://heldincommon.example.ts.net:8080 (Funnel on)", 8080));
    }

    #[test]
    fn other_ports_are_not_forwarded() {
        assert!(!forwards_to(SERVE_STATUS, 8080));
        assert!(!forwards_to(SERVE_STATUS, 808), "a prefix of the port isn't the port");
        assert!(!forwards_to("|-- / proxy http://127.0.0.1:80810", 8081));
        assert!(!forwards_to("No serve config", 8080));
        assert!(!forwards_to("", 8080));
    }

    #[test]
    fn every_kind_can_be_opened_on_a_room_network() {
        let lan = Some("http://192.168.1.5:8080".to_string());
        assert_eq!(check_open(Kind::Node, lan.clone(), false), Ok("http://192.168.1.5:8080".to_string()));
        assert!(check_open(Kind::Practice, lan.clone(), false).is_ok());
        assert!(check_open(Kind::Commons, lan, false).is_ok(), "a full commons too: it takes a room code");
    }

    #[test]
    fn it_wont_open_on_a_vpn_or_tailscale_address() {
        assert_eq!(
            check_open(Kind::Node, Some("http://100.101.102.103:8080".into()), false),
            Err(CantOpen::NotARoomNetwork)
        );
        assert_eq!(check_open(Kind::Node, Some("http://8.8.8.8:8080".into()), false), Err(CantOpen::NotARoomNetwork));
        for room in
            ["http://10.0.0.5:8080", "http://172.20.10.3:8080", "http://192.168.1.5:8080", "http://[fd00::5]:8080"]
        {
            assert!(check_open(Kind::Node, Some(room.into()), false).is_ok(), "{room}");
        }
    }

    #[test]
    fn it_cant_open_without_a_network() {
        assert_eq!(check_open(Kind::Node, None, false), Err(CantOpen::NoNetwork));
    }

    #[test]
    fn it_wont_open_a_port_tailscale_forwards_to() {
        let lan = Some("http://192.168.1.5:8080".to_string());
        assert_eq!(check_open(Kind::Node, lan, true), Err(CantOpen::Forwarded));
        assert!(CantOpen::Forwarded.message().contains("tailscale serve reset"));
    }

    #[test]
    fn an_open_server_listens_on_the_room_address_only() {
        assert_eq!(ip_of("http://192.168.1.23:8080").as_deref(), Some("192.168.1.23"));
        assert_eq!(ip_of("http://[fd00::5]:8080").as_deref(), Some("fd00::5"));
        assert_eq!(ip_of("not an address"), None);
    }

    #[test]
    fn a_room_code_is_six_fair_letters_without_look_alikes() {
        for _ in 0..200 {
            let code = new_room_code().unwrap();
            assert_eq!(code.len(), 6);
            assert!(code.bytes().all(|b| ROOM_CODE_LETTERS.contains(&b)), "{code}");
        }
        assert!(!ROOM_CODE_LETTERS.iter().any(|b| b"ILO".contains(b)));
    }

    #[test]
    fn a_commons_is_given_a_room_code_and_a_node_is_not() {
        let extras = RoomExtras { code: Some("KTRMWX".into()), firewall_hint: Some("Allow it".into()) };
        let local: Vec<_> = room_env(Kind::Practice, &extras);
        assert!(local.contains(&("HIC_ROOM_CODE", "KTRMWX".to_string())));
        assert!(local.contains(&("HIC_FIREWALL_HINT", "Allow it".to_string())));
        assert!(
            room_env(Kind::Commons, &extras).contains(&("HIC_ROOM_CODE", "KTRMWX".to_string())),
            "a full commons too"
        );
        let node = room_env(Kind::Node, &extras);
        assert!(!node.iter().any(|(k, _)| *k == "HIC_ROOM_CODE"), "a node admits nobody");
        assert!(room_env(Kind::Practice, &RoomExtras::default()).is_empty());
    }

    #[test]
    fn a_change_of_network_closes_the_room() {
        let here = Some("http://192.168.1.5:8080");
        assert!(!should_close(here, here), "same network: stays open");
        assert!(should_close(here, Some("http://10.0.0.7:8080")), "a new address");
        assert!(should_close(here, None), "the network went away");
    }

    #[test]
    fn the_switch_says_where_a_server_is_in_words() {
        assert_eq!(where_is(Kind::Node, false), Some("on this computer, only you"));
        assert_eq!(where_is(Kind::Node, true), Some("open to the room: public pages only"));
        assert_eq!(where_is(Kind::Practice, true), Some("local commons, open to the room"));
        assert_eq!(where_is(Kind::Commons, true), Some("commons, open to the room"));
        assert_eq!(where_is(Kind::Commons, false), None, "closed: as app.json says");
        assert_eq!(menu_label(Kind::Node, "Laptop", false), "Open my node to the room\u{2026}");
        assert_eq!(menu_label(Kind::Practice, "Thursday", true), "Close Thursday to the room");
    }
}
