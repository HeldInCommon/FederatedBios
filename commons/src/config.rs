//! Settings, read once at start-up from environment variables.
//!
//! [`Config::from_lookup`] is pure (it takes a lookup function), so tests can
//! build any configuration; [`Config::from_env`] wires it to the real
//! environment. Every variable is documented in `readme.md`.

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

/// Everything the server needs to know before it starts.
#[derive(Clone, Debug)]
pub struct Config {
    /// `HIC_DATA`: the data folder. Default `./data`.
    pub data_dir: PathBuf,
    /// `PORT`. Default 8080.
    pub port: u16,
    /// `HIC_BIND`: the address to listen on. Default: chosen by role (see
    /// [`Config::bind_addr`]).
    pub bind: Option<IpAddr>,
    /// `HIC_BASE_URL`: this server's public address. Default `http://localhost:<port>`.
    pub base_url: String,
    /// `HIC_ALSO_KNOWN_AS`: other addresses this server is reached at, comma
    /// separated, such as a commons announced on the room's Wi-Fi that is also
    /// public through Tailscale Funnel or a reverse proxy. Invites, vouches and
    /// withdrawals made for any of them count as addressed here.
    pub also_known_as: Vec<String>,
    /// `HIC_INSTANCE`: a human name for this server.
    pub instance: String,
    /// `HIC_SHARE_DEFAULT`: the commons pre-filled in "Share to a commons".
    pub share_default: String,
    /// `HIC_SHARE_DEFAULT_NAME`: what to call that commons on pages
    /// ("Held in Common Collective Commons"), instead of its address.
    pub share_default_name: String,
    /// `HIC_SHARE_NEARBY`: on a node, the commons the desktop app runs on this
    /// same computer, offered as one-click shares: comma separated
    /// `address=name` pairs (`http://127.0.0.1:8180=spark`). Their
    /// `127.0.0.1` addresses never change, unlike the Wi-Fi's.
    pub share_nearby: Vec<(String, String)>,
    /// `HIC_ROLE`, raw. Interpreted by [`crate::role::decide`].
    pub role_env: Option<String>,
    /// `HIC_LAUNCH_TOKEN`: set by the desktop app when it starts this server,
    /// and echoed by `/health`, so the app can tell its own server from any
    /// other program on the same port. Not a secret: it only identifies.
    pub launch_token: Option<String>,
    /// `HIC_ALLOW_PRIVATE_SHARE=1`: a node reachable from the network may
    /// still share to local-network addresses (see [`crate::outbound`]).
    pub allow_private_share: bool,
    /// `HIC_ROOM=1`: this server is open to the room (decision record 0027).
    /// The door in [`crate::room`] stands in front of every request, and a
    /// node listens on the network unless `HIC_BIND` says otherwise.
    pub room: bool,
    /// `HIC_EXIT_WITH_APP=1`: stop when stdin ends. The desktop app sets it
    /// and holds the other end of the pipe, so the server stops when the app
    /// does, even after a crash (decision record 0027). Off for Docker and
    /// people running the server by hand, where stdin may be empty.
    pub exit_with_app: bool,
    /// `HIC_ROOM_ADDRESS`: the address people in the room reach this server
    /// at (`http://192.168.1.23:8080`), set by the desktop app while it's open
    /// to the room. The room page's QR code holds it. A node keeps its own
    /// `HIC_BASE_URL`, so nothing it shares names an address that stops
    /// working when the room closes.
    pub room_address: Option<String>,
    /// `HIC_ROOM_CODE`: on a commons open to the room, the code people type
    /// (or carry in the join link) to join (decision record 0027, D4), tidied
    /// to six capitals. The desktop app makes a new one each time the room
    /// opens. Ignored unless open to the room.
    pub room_code: Option<String>,
    /// `HIC_FIREWALL_HINT`: what to do about this computer's firewall, read by
    /// the desktop app without admin rights and shown on the room page (D5).
    pub firewall_hint: Option<String>,
    /// `HIC_OPEN_COMMONS=1`: anyone may share in without a vouch (decision record 0011).
    pub open_commons: bool,
    /// `HIC_VOUCH_LIMIT`: how many people one member may bring in per week. The
    /// steward (today, whoever hosts the commons) sets it. Default 100.
    pub vouch_limit: u32,
    /// `HIC_SESSION_COOKIE`: the sign-in cookie's name. Default `hic_session`.
    /// Browsers share cookies between ports of the same host, so the desktop
    /// app gives each server it runs on this computer (a node, a commons, a
    /// practice commons) its own name; otherwise signing in to one would sign
    /// you out of another.
    pub session_cookie: String,
    /// `HIC_PRACTICE=1`: this commons is a practice one, for learning to run a
    /// commons. Only changes what its pages say.
    pub practice: bool,
    /// `HIC_RELAY_HOLD` and `HIC_RELAY_WAIT`, in seconds: how long a commons
    /// holds a node's poll open, and how long a reader waits for the node's
    /// answer, for "Only while I'm here" (decision record 0021). Defaults 25
    /// and 8; tests make them short. Each is kept between 1 and 60.
    pub relay: crate::relay::Timing,
}

/// A number of seconds from a setting, kept between 1 and 60, or `default`
/// when it's missing or not a number.
fn seconds(value: Option<String>, default: u64) -> u64 {
    value.and_then(|v| v.trim().parse::<u64>().ok()).map_or(default, |s| s.clamp(1, 60))
}

/// The host (and port, if any) of a URL: `https://c.example/x` → `c.example`.
fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or("localhost");
    rest.split('/').next().unwrap_or(rest).to_string()
}

/// The host a signed message names a commons by: its address without the
/// scheme or a trailing slash (`hic.example.ts.net`, `10.0.0.5:8081`).
///
/// Used for everything that is **signed** (invites, vouches, withdrawals,
/// waiting-list requests, "Only while I'm here" cards and polls), and must
/// match [`Config::own_hosts`] on the commons. Never a display name or a
/// substituted address: for what people read, use
/// `web::layout::host_label`. Pure.
pub fn commons_host(url: &str) -> String {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url)
        .trim_end_matches('/')
        .to_string()
}

/// `HIC_SHARE_NEARBY` read into (address, name) pairs: comma separated
/// `address=name`, addresses with a scheme and no trailing slash. Anything
/// malformed is skipped. Pure.
fn parse_nearby(written: &str) -> Vec<(String, String)> {
    written
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(url, name)| (url.trim().trim_end_matches('/').to_string(), name.trim().to_string()))
        .filter(|(url, name)| url.contains("://") && !name.is_empty())
        .collect()
}

/// The sign-in cookie's name unless `HIC_SESSION_COOKIE` says otherwise.
pub const DEFAULT_SESSION_COOKIE: &str = "hic_session";

/// A cookie name we accept: 1 to 40 ASCII letters, digits or `_`. Anything
/// else would need quoting in a `Set-Cookie` header, so it falls back to the
/// default.
fn valid_cookie_name(c: &str) -> bool {
    (1..=40).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl Config {
    /// Build from any `name -> value` lookup.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Config {
        let port = get("PORT").and_then(|p| p.parse().ok()).unwrap_or(8080);
        let room = get("HIC_ROOM").is_some_and(|v| v.trim() == "1");
        let practice = get("HIC_PRACTICE").is_some_and(|v| v.trim() == "1");
        Config {
            data_dir: PathBuf::from(get("HIC_DATA").unwrap_or_else(|| "./data".into())),
            port,
            bind: get("HIC_BIND").and_then(|b| b.trim().parse().ok()),
            base_url: get("HIC_BASE_URL")
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| format!("http://localhost:{port}")),
            also_known_as: get("HIC_ALSO_KNOWN_AS")
                .unwrap_or_default()
                .split(',')
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| u.contains("://"))
                .collect(),
            instance: get("HIC_INSTANCE").unwrap_or_else(|| "Held in Common".into()),
            share_default: get("HIC_SHARE_DEFAULT").unwrap_or_default().trim().trim_end_matches('/').to_string(),
            share_default_name: get("HIC_SHARE_DEFAULT_NAME").unwrap_or_default().trim().to_string(),
            share_nearby: parse_nearby(&get("HIC_SHARE_NEARBY").unwrap_or_default()),
            role_env: get("HIC_ROLE"),
            launch_token: get("HIC_LAUNCH_TOKEN").filter(|t| !t.is_empty()),
            allow_private_share: get("HIC_ALLOW_PRIVATE_SHARE").is_some_and(|v| v.trim() == "1"),
            room,
            exit_with_app: get("HIC_EXIT_WITH_APP").is_some_and(|v| v.trim() == "1"),
            room_code: get("HIC_ROOM_CODE")
                .filter(|c| room && crate::vouching::looks_like_room_code(c))
                .map(|c| crate::vouching::tidy_room_code(&c)),
            // Plain text, shown escaped; capped so a stray value can't fill the page.
            firewall_hint: get("HIC_FIREWALL_HINT")
                .map(|h| h.trim().chars().take(600).collect::<String>())
                .filter(|h| !h.is_empty()),
            room_address: get("HIC_ROOM_ADDRESS")
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| u.starts_with("http://") || u.starts_with("https://")),
            open_commons: get("HIC_OPEN_COMMONS").is_some_and(|v| v.trim() == "1"),
            vouch_limit: get("HIC_VOUCH_LIMIT")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(crate::vouching::DEFAULT_LIMIT),
            session_cookie: get("HIC_SESSION_COOKIE")
                .map(|c| c.trim().to_string())
                .filter(|c| valid_cookie_name(c))
                .unwrap_or_else(|| DEFAULT_SESSION_COOKIE.into()),
            practice,
            relay: crate::relay::Timing {
                hold: seconds(get("HIC_RELAY_HOLD"), crate::relay::DEFAULT_TIMING.hold),
                wait: seconds(get("HIC_RELAY_WAIT"), crate::relay::DEFAULT_TIMING.wait),
            },
        }
    }

    /// Build from the process environment.
    pub fn from_env() -> Config {
        Config::from_lookup(|k| std::env::var(k).ok())
    }

    /// The host part of [`Config::base_url`] (used in WebFinger `acct:` ids).
    pub fn domain(&self) -> String {
        host_of(&self.base_url)
    }

    /// True when the desktop app started this server (it set a launch token),
    /// so pages may offer the app's own actions (see `web::pages::guide`).
    pub fn in_app(&self) -> bool {
        self.launch_token.is_some()
    }

    /// The hosts this server counts as itself when something signed is
    /// addressed to it (a withdrawal, an invite, a vouch): its public address,
    /// and `127.0.0.1` and `localhost` on its port. The last two matter on the
    /// commons' own computer: a node there (the desktop app runs both) reaches
    /// the commons at `127.0.0.1`, while the commons announces its Wi-Fi
    /// address. Accepting them lets a message meant for "the commons on this
    /// port of the sender's own computer" also count here; that is the same
    /// commons whenever the sender is on this computer, which is the case
    /// these names exist for.
    pub fn own_hosts(&self) -> Vec<String> {
        let mut hosts = vec![self.domain()];
        for url in &self.also_known_as {
            let host = host_of(url);
            if !hosts.contains(&host) {
                hosts.push(host);
            }
        }
        for h in ["127.0.0.1", "localhost"] {
            let local = format!("{h}:{}", self.port);
            if !hosts.contains(&local) {
                hosts.push(local);
            }
        }
        hosts
    }

    /// Mark session cookies `Secure` only when served over https: browsers
    /// (WKWebView included) drop Secure cookies on plain http, which would
    /// leave a local node stuck in a sign-in loop.
    pub fn secure_cookie(&self) -> bool {
        self.base_url.starts_with("https://")
    }

    /// Which addresses this server may contact when a member asks it to
    /// (sharing, vouching, checking a waiting list): decision record 0012,
    /// see [`crate::outbound`]. A server listening only on this computer is
    /// *personal*; a node open to the room (decision record 0027) may also
    /// reach the room's network and this machine. Pure.
    pub fn share_policy(&self, role: crate::role::Role) -> crate::outbound::Policy {
        crate::outbound::Policy {
            personal: self.bind_addr(role).is_loopback(),
            allow_private: self.allow_private_share,
            in_room: self.room && role == crate::role::Role::Node,
        }
    }

    /// Every address to listen on: [`Config::bind_addr`], and also this
    /// computer (`127.0.0.1`) when a server open to the room listens on one
    /// network address only (the desktop app gives it the room's address, so
    /// it isn't also reachable over a VPN or Tailscale; the owner's window
    /// still reaches it on `127.0.0.1`). Decision record 0027. Pure.
    pub fn listen_addrs(&self, role: crate::role::Role) -> Vec<IpAddr> {
        let main = self.bind_addr(role);
        if self.room && !main.is_loopback() && !main.is_unspecified() {
            vec![main, IpAddr::V4(Ipv4Addr::LOCALHOST)]
        } else {
            vec![main]
        }
    }

    /// Where to listen. A **node** listens only on this computer
    /// (`127.0.0.1`) unless `HIC_BIND` says otherwise: it holds its members'
    /// keys and sign-in page, and shouldn't be reachable from the rest of the
    /// Wi-Fi. A **commons** exists to be reached, so it listens on every
    /// interface (`0.0.0.0`). Docker images set `HIC_BIND=0.0.0.0`. A node
    /// **open to the room** (`HIC_ROOM=1`) listens everywhere too, with the
    /// door in front of it (decision record 0027).
    pub fn bind_addr(&self, role: crate::role::Role) -> IpAddr {
        self.bind.unwrap_or(match role {
            crate::role::Role::Node if self.room => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            crate::role::Role::Node => IpAddr::V4(Ipv4Addr::LOCALHOST),
            crate::role::Role::Commons => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::role::Role;
    use std::collections::HashMap;

    /// A config built only from `vars`, as if they were the environment.
    fn cfg(vars: &[(&str, &str)]) -> Config {
        let m: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Config::from_lookup(|k| m.get(k).cloned())
    }

    #[test]
    fn defaults() {
        let c = cfg(&[]);
        assert_eq!(c.port, 8080);
        assert_eq!(c.base_url, "http://localhost:8080");
        assert_eq!(c.domain(), "localhost:8080");
        assert!(!c.secure_cookie());
    }

    #[test]
    fn trailing_slashes_are_trimmed() {
        let c = cfg(&[("HIC_BASE_URL", "https://c.example/"), ("HIC_SHARE_DEFAULT", "http://x/ ")]);
        assert_eq!(c.base_url, "https://c.example");
        assert_eq!(c.share_default, "http://x");
        assert!(c.secure_cookie());
    }

    #[test]
    fn a_commons_answers_to_its_public_and_local_addresses() {
        let c = cfg(&[("PORT", "8081"), ("HIC_BASE_URL", "http://192.168.1.5:8081")]);
        assert_eq!(c.own_hosts(), vec!["192.168.1.5:8081", "127.0.0.1:8081", "localhost:8081"]);
        assert_eq!(cfg(&[]).own_hosts(), vec!["localhost:8080", "127.0.0.1:8080"], "no duplicates");
    }

    #[test]
    fn a_commons_also_answers_to_its_other_public_addresses() {
        let c = cfg(&[
            ("PORT", "8081"),
            ("HIC_BASE_URL", "http://10.0.0.5:8081"),
            ("HIC_ALSO_KNOWN_AS", " https://hic.tail.ts.net/ , not-a-url, https://c.example:8443"),
        ]);
        assert_eq!(c.also_known_as, vec!["https://hic.tail.ts.net", "https://c.example:8443"], "URLs only, tidied");
        assert_eq!(
            c.own_hosts(),
            vec!["10.0.0.5:8081", "hic.tail.ts.net", "c.example:8443", "127.0.0.1:8081", "localhost:8081"]
        );
        assert!(cfg(&[]).also_known_as.is_empty());
    }

    #[test]
    fn session_cookie_name_is_checked() {
        assert_eq!(cfg(&[]).session_cookie, "hic_session");
        assert_eq!(cfg(&[("HIC_SESSION_COOKIE", " hic_session_8081 ")]).session_cookie, "hic_session_8081");
        assert_eq!(cfg(&[("HIC_SESSION_COOKIE", "bad; name")]).session_cookie, "hic_session");
        assert_eq!(cfg(&[("HIC_SESSION_COOKIE", "")]).session_cookie, "hic_session");
    }

    #[test]
    fn signed_messages_name_a_commons_by_its_exact_host() {
        assert_eq!(commons_host("https://hic.example.ts.net/"), "hic.example.ts.net");
        assert_eq!(commons_host("http://127.0.0.1:8082"), "127.0.0.1:8082", "never swapped for a network address");
        assert_eq!(commons_host("http://10.0.0.5:8081"), "10.0.0.5:8081");
    }

    #[test]
    fn a_server_open_on_one_address_also_listens_for_its_owner() {
        let open = cfg(&[("HIC_ROOM", "1"), ("HIC_BIND", "192.168.1.23")]);
        let addrs: Vec<String> = open.listen_addrs(Role::Node).iter().map(|a| a.to_string()).collect();
        assert_eq!(addrs, vec!["192.168.1.23", "127.0.0.1"]);
        let everywhere = cfg(&[("HIC_ROOM", "1")]);
        assert_eq!(everywhere.listen_addrs(Role::Node).len(), 1, "0.0.0.0 already includes this computer");
        assert_eq!(cfg(&[("HIC_BIND", "192.168.1.23")]).listen_addrs(Role::Node).len(), 1, "only with the room");
    }

    #[test]
    fn the_room_address_is_tidied_and_must_be_a_web_address() {
        assert_eq!(
            cfg(&[("HIC_ROOM_ADDRESS", " http://192.168.1.23:8080/ ")]).room_address.as_deref(),
            Some("http://192.168.1.23:8080")
        );
        assert_eq!(cfg(&[("HIC_ROOM_ADDRESS", "192.168.1.23")]).room_address, None);
        assert_eq!(cfg(&[]).room_address, None);
    }

    #[test]
    fn only_a_commons_open_to_the_room_takes_a_room_code() {
        let local = [("HIC_ROOM", "1"), ("HIC_PRACTICE", "1"), ("HIC_ROOM_CODE", "ktr mwx")];
        assert_eq!(cfg(&local).room_code.as_deref(), Some("KTRMWX"));
        assert_eq!(cfg(&local[1..]).room_code, None, "closed to the room");
        assert_eq!(
            cfg(&[local[0], local[2]]).room_code.as_deref(),
            Some("KTRMWX"),
            "a full commons open to the room too"
        );
        assert_eq!(cfg(&[local[0], local[1], ("HIC_ROOM_CODE", "hic-invite-x")]).room_code, None, "not a room code");
    }

    #[test]
    fn nearby_commons_are_read_as_address_and_name() {
        let c = cfg(&[("HIC_SHARE_NEARBY", "http://127.0.0.1:8180/=spark, nonsense, http://127.0.0.1:8182=Thursday")]);
        assert_eq!(
            c.share_nearby,
            vec![
                ("http://127.0.0.1:8180".to_string(), "spark".to_string()),
                ("http://127.0.0.1:8182".to_string(), "Thursday".to_string())
            ]
        );
        assert!(cfg(&[]).share_nearby.is_empty());
    }

    #[test]
    fn practice_and_app_flags() {
        assert!(!cfg(&[]).exit_with_app);
        assert!(cfg(&[("HIC_EXIT_WITH_APP", "1")]).exit_with_app);
        assert!(!cfg(&[]).practice);
        assert!(cfg(&[("HIC_PRACTICE", "1")]).practice);
        assert!(!cfg(&[]).in_app());
        assert!(cfg(&[("HIC_LAUNCH_TOKEN", "t")]).in_app());
    }

    #[test]
    fn a_node_open_to_the_room_listens_on_the_network_and_shares_to_the_room() {
        let open = cfg(&[("HIC_ROOM", "1")]);
        assert!(open.room);
        assert_eq!(open.bind_addr(Role::Node).to_string(), "0.0.0.0", "the room implies the network");
        assert!(open.share_policy(Role::Node).in_room);
        assert!(!open.share_policy(Role::Node).personal);
        assert!(!open.share_policy(Role::Commons).in_room, "a commons takes requests from anyone");
        assert!(cfg(&[]).share_policy(Role::Node).personal, "a node on 127.0.0.1");
        assert!(!cfg(&[("HIC_BIND", "0.0.0.0")]).share_policy(Role::Node).in_room, "networked without the door");
    }

    #[test]
    fn node_listens_locally_commons_listens_everywhere() {
        let c = cfg(&[]);
        assert_eq!(c.bind_addr(Role::Node).to_string(), "127.0.0.1");
        assert_eq!(c.bind_addr(Role::Commons).to_string(), "0.0.0.0");
        assert_eq!(cfg(&[("HIC_BIND", "0.0.0.0")]).bind_addr(Role::Node).to_string(), "0.0.0.0");
    }
}
