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
    /// `HIC_ROLE`, raw. Interpreted by [`crate::role::decide`].
    pub role_env: Option<String>,
    /// `HIC_LAUNCH_TOKEN`: set by the desktop app when it starts this server,
    /// and echoed by `/health`, so the app can tell its own server from any
    /// other program on the same port. Not a secret: it only identifies.
    pub launch_token: Option<String>,
    /// `HIC_ALLOW_PRIVATE_SHARE=1`: a node reachable from the network may
    /// still share to local-network addresses (see [`crate::outbound`]).
    pub allow_private_share: bool,
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
}

/// The host (and port, if any) of a URL: `https://c.example/x` → `c.example`.
fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or("localhost");
    rest.split('/').next().unwrap_or(rest).to_string()
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
            role_env: get("HIC_ROLE"),
            launch_token: get("HIC_LAUNCH_TOKEN").filter(|t| !t.is_empty()),
            allow_private_share: get("HIC_ALLOW_PRIVATE_SHARE").is_some_and(|v| v.trim() == "1"),
            open_commons: get("HIC_OPEN_COMMONS").is_some_and(|v| v.trim() == "1"),
            vouch_limit: get("HIC_VOUCH_LIMIT")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(crate::vouching::DEFAULT_LIMIT),
            session_cookie: get("HIC_SESSION_COOKIE")
                .map(|c| c.trim().to_string())
                .filter(|c| valid_cookie_name(c))
                .unwrap_or_else(|| DEFAULT_SESSION_COOKIE.into()),
            practice: get("HIC_PRACTICE").is_some_and(|v| v.trim() == "1"),
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

    /// Where to listen. A **node** listens only on this computer
    /// (`127.0.0.1`) unless `HIC_BIND` says otherwise: it holds its members'
    /// keys and sign-in page, and shouldn't be reachable from the rest of the
    /// Wi-Fi. A **commons** exists to be reached, so it listens on every
    /// interface (`0.0.0.0`). Docker images set `HIC_BIND=0.0.0.0`.
    pub fn bind_addr(&self, role: crate::role::Role) -> IpAddr {
        self.bind.unwrap_or(match role {
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
    fn practice_and_app_flags() {
        assert!(!cfg(&[]).practice);
        assert!(cfg(&[("HIC_PRACTICE", "1")]).practice);
        assert!(!cfg(&[]).in_app());
        assert!(cfg(&[("HIC_LAUNCH_TOKEN", "t")]).in_app());
    }

    #[test]
    fn node_listens_locally_commons_listens_everywhere() {
        let c = cfg(&[]);
        assert_eq!(c.bind_addr(Role::Node).to_string(), "127.0.0.1");
        assert_eq!(c.bind_addr(Role::Commons).to_string(), "0.0.0.0");
        assert_eq!(cfg(&[("HIC_BIND", "0.0.0.0")]).bind_addr(Role::Node).to_string(), "0.0.0.0");
    }
}
