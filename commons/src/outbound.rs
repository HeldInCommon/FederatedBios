//! Where this server may send requests that a member asked for.
//!
//! When someone presses Share or Withdraw, the *server* contacts the address
//! they typed. A server can reach places its users might not: other programs
//! on the same machine, the local network, and on cloud hosts the address that
//! hands out the machine's own credentials (`169.254.169.254`). Letting a user
//! aim the server at those is "server-side request forgery".
//!
//! The rule (decision record 0012): this only matters when the person pressing
//! the button couldn't reach the address themselves.
//! - A **personal node** listens only on its own computer, so whoever uses it
//!   is at that computer already: it may share anywhere.
//! - A node **reachable from the network** refuses its own machine's
//!   addresses and link-local ones, and refuses local-network addresses unless
//!   `HIC_ALLOW_PRIVATE_SHARE=1`.
//!
//! The address is checked after the DNS lookup, and the request is then pinned
//! to the checked address (so a second lookup can't swap it), with redirects
//! turned off.

use std::net::{IpAddr, SocketAddr};

/// What kind of address an IP is, for this rule.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    /// This machine: `127.0.0.0/8`, `::1`.
    Loopback,
    /// Link-local, including the cloud credentials address: `169.254.0.0/16`, `fe80::/10`.
    LinkLocal,
    /// `0.0.0.0`, `::`.
    Unspecified,
    /// A local network: `10/8`, `172.16/12`, `192.168/16`, `100.64/10`, `fc00::/7`.
    Private,
    /// Anything else.
    Public,
}

/// Classify an IP address.
pub fn kind(ip: IpAddr) -> Kind {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            if v4.is_loopback() {
                Kind::Loopback
            } else if v4.is_link_local() {
                Kind::LinkLocal
            } else if v4.is_unspecified() {
                Kind::Unspecified
            } else if v4.is_private() || (o[0] == 100 && (64..128).contains(&o[1])) {
                Kind::Private
            } else {
                Kind::Public
            }
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return kind(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            if v6.is_loopback() {
                Kind::Loopback
            } else if v6.is_unspecified() {
                Kind::Unspecified
            } else if first & 0xffc0 == 0xfe80 {
                Kind::LinkLocal
            } else if first & 0xfe00 == 0xfc00 {
                Kind::Private
            } else {
                Kind::Public
            }
        }
    }
}

/// The server's situation, which decides what is allowed.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    /// True when this server listens only on its own computer.
    pub personal: bool,
    /// `HIC_ALLOW_PRIVATE_SHARE=1`: a networked node may reach local-network addresses.
    pub allow_private: bool,
}

/// May this server contact an address of this kind?
pub fn permitted(k: Kind, p: Policy) -> bool {
    if p.personal {
        return true;
    }
    match k {
        Kind::Public => true,
        Kind::Private => p.allow_private,
        Kind::Loopback | Kind::LinkLocal | Kind::Unspecified => false,
    }
}

/// Check every address a name resolves to. All must be permitted: a name
/// with one safe and one unsafe address is refused.
pub fn check_all(addrs: &[SocketAddr], p: Policy) -> Result<(), String> {
    if addrs.is_empty() {
        return Err("that address doesn't resolve".into());
    }
    if addrs.iter().all(|a| permitted(kind(a.ip()), p)) {
        Ok(())
    } else {
        Err("This server can't send to addresses inside its own network.".into())
    }
}

/// Look up `url`, check it against the policy, and return an HTTP client
/// pinned to the checked address, with redirects off.
pub async fn client_for(url: &str, p: Policy) -> Result<reqwest::Client, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "That isn't a valid address.".to_string())?;
    let host = parsed.host_str().ok_or("That address has no host.")?.to_string();
    let port = parsed.port_or_known_default().ok_or("That address has no port.")?;
    // `host` may be an IP literal in brackets ("[::1]"); strip them for lookup.
    let lookup = host.trim_start_matches('[').trim_end_matches(']').to_string();
    let addrs: Vec<SocketAddr> =
        tokio::net::lookup_host((lookup.as_str(), port)).await.map_err(|_| format!("Couldn't find {host}."))?.collect();
    check_all(&addrs, p)?;
    // Pin the request to the addresses just checked (all of them, so the
    // client can still fall back from IPv6 to IPv4 as usual).
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(&lookup, &addrs)
        .build()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse an address written in a test.
    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn classifies_addresses() {
        assert_eq!(kind(ip("127.0.0.1")), Kind::Loopback);
        assert_eq!(kind(ip("::1")), Kind::Loopback);
        assert_eq!(kind(ip("169.254.169.254")), Kind::LinkLocal);
        assert_eq!(kind(ip("fe80::1")), Kind::LinkLocal);
        assert_eq!(kind(ip("0.0.0.0")), Kind::Unspecified);
        assert_eq!(kind(ip("192.168.1.20")), Kind::Private);
        assert_eq!(kind(ip("10.0.0.5")), Kind::Private);
        assert_eq!(kind(ip("172.20.0.1")), Kind::Private);
        assert_eq!(kind(ip("100.100.1.1")), Kind::Private);
        assert_eq!(kind(ip("fd00::1")), Kind::Private);
        assert_eq!(kind(ip("::ffff:127.0.0.1")), Kind::Loopback, "IPv4 hidden in IPv6");
        assert_eq!(kind(ip("93.184.216.34")), Kind::Public);
    }

    #[test]
    fn personal_node_may_share_anywhere() {
        let p = Policy { personal: true, allow_private: false };
        for k in [Kind::Loopback, Kind::LinkLocal, Kind::Private, Kind::Public] {
            assert!(permitted(k, p));
        }
    }

    #[test]
    fn networked_node_refuses_inside_addresses() {
        let p = Policy { personal: false, allow_private: false };
        assert!(permitted(Kind::Public, p));
        assert!(!permitted(Kind::Private, p));
        assert!(!permitted(Kind::Loopback, p));
        assert!(!permitted(Kind::LinkLocal, p));
        let p = Policy { allow_private: true, ..p };
        assert!(permitted(Kind::Private, p));
        assert!(!permitted(Kind::LinkLocal, p), "the credentials address is never allowed");
    }

    #[test]
    fn a_name_with_any_unsafe_address_is_refused() {
        let p = Policy { personal: false, allow_private: false };
        let mixed: Vec<SocketAddr> = vec!["93.184.216.34:443".parse().unwrap(), "127.0.0.1:443".parse().unwrap()];
        assert!(check_all(&mixed, p).is_err());
        assert!(check_all(&mixed[..1], p).is_ok());
        assert!(check_all(&[], p).is_err());
    }
}
