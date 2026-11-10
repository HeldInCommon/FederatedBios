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
    /// `HIC_INSTANCE`: a human name for this server.
    pub instance: String,
    /// `HIC_SHARE_DEFAULT`: the commons pre-filled in "Share to a commons".
    pub share_default: String,
    /// `HIC_ROLE`, raw. Interpreted by [`crate::role::decide`].
    pub role_env: Option<String>,
    /// `HIC_LAUNCH_TOKEN`: set by the desktop app when it starts this server,
    /// and echoed by `/health`, so the app can tell its own server from any
    /// other program on the same port. Not a secret: it only identifies.
    pub launch_token: Option<String>,
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
            instance: get("HIC_INSTANCE").unwrap_or_else(|| "Held in Common".into()),
            share_default: get("HIC_SHARE_DEFAULT").unwrap_or_default().trim().trim_end_matches('/').to_string(),
            role_env: get("HIC_ROLE"),
            launch_token: get("HIC_LAUNCH_TOKEN").filter(|t| !t.is_empty()),
        }
    }

    /// Build from the process environment.
    pub fn from_env() -> Config {
        Config::from_lookup(|k| std::env::var(k).ok())
    }

    /// The host part of [`Config::base_url`] (used in WebFinger `acct:` ids).
    pub fn domain(&self) -> String {
        self.base_url.split("://").nth(1).unwrap_or("localhost").trim_end_matches('/').to_string()
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
    fn node_listens_locally_commons_listens_everywhere() {
        let c = cfg(&[]);
        assert_eq!(c.bind_addr(Role::Node).to_string(), "127.0.0.1");
        assert_eq!(c.bind_addr(Role::Commons).to_string(), "0.0.0.0");
        assert_eq!(cfg(&[("HIC_BIND", "0.0.0.0")]).bind_addr(Role::Node).to_string(), "0.0.0.0");
    }
}
