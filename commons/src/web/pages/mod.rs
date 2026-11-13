//! The body of each HTML page, as pure functions from data to markup: one
//! file per page (or per small family of pages).
//!
//! Handlers (in [`crate::web::handlers`]) load the data and pass it in;
//! nothing here reads files, the clock or the session. Every page is
//! re-exported here, so callers write `pages::profile(…)`.
//!
//! | File | Pages |
//! |---|---|
//! | `directory.rs` | the commons' front page, with the steward's tools and the waiting list |
//! | `profile.rs` | a member's profile page |
//! | `owner_account.rs` | the Account tab: backup, recovery key, your key, Advanced |
//! | `owner.rs` | a member's own profile: next step, tabs, Sharing |
//! | `owner_people.rs` | the People tab: who is waiting for a vouch (with Vouch buttons), invite someone |
//! | `publish.rs` | the publish form |
//! | `signin.rs` | sign-up, sign-in, and the "this is a shared commons" gate |
//! | `account.rs` | recovery code, key change, restore from backup |
//! | `sharing.rs` | results of sharing: shared, waiting for a vouch, vouched (or not), withdrawn |
//! | `invites.rs` | invite codes made by a member or the steward |
//! | `federated.rs` | a profile shared in from another node |
//! | `about.rs` | how this works |
//! | `schematics.rs` | the diagrams on How this works and Run a commons |
//! | `guide.rs` | run a commons: questions and answers, and a local commons |
//! | `stories.rs` | story lists: a node's, and a commons' (with recent withdrawals) |
//! | `story.rs` | one story: the author's view with its sharing tools, and a commons' |
//! | `here.rs` | a story shared "Only while I'm here": its text fetched just now, or "Not here right now" |
//! | `story_form.rs` | writing a story (new or editing), then its rights step |
//! | `rights.rs` | Sharing and rights: who can see it, licence, signals, Held in Common Signals, how a story travels |
//! | `room.rs` | open to the room: the owner's room page with its QR code, the check page and "Can't reach someone?" |
//! | `parts.rs` | shared pieces: page heading, section label, starburst, terms panel, "2 min ago" |

use crate::model::RemoteEntry;

mod about;
mod account;
mod directory;
mod federated;
mod guide;
mod here;
mod invites;
mod owner;
mod owner_account;
mod owner_people;
mod parts;
mod profile;
mod publish;
mod rights;
mod room;
mod schematics;
mod sharing;
mod signin;
mod stories;
mod story;
mod story_form;

pub use about::about;
pub use account::{key_change, recovery_code, restore_form};
pub use directory::{directory, your_next, Card, SignedIn, StewardView, Viewer, YourNext};
pub use federated::federated;
pub use guide::{run_a_commons, steward_node_note};
pub use here::here_story;
pub use invites::{invite_code, steward_invite};
pub use owner::OwnerPanel;
pub use parts::Chosen;
pub use profile::profile;
pub use publish::publish_form;
pub use room::{join_room, room, room_check, room_help, RoomView};
pub use sharing::{shared, shared_pending, vouch_outcome, withdrawn};
pub use signin::{commons_gate, join_form, login_form};
pub use stories::{author_stories, commons_stories, node_stories};
pub use story::{held_story, own_story, StoryOutcome, StoryOwner};
pub use story_form::{rights_form, write_form};

/// Where a shared profile says it came from, for people to read: its
/// node's address if that names somewhere else on the network, otherwise
/// the node's name for itself ("Tega's node"), or "a node" if it gave none.
/// Both are the sender's own claims.
pub fn origin_host(r: &RemoteEntry) -> String {
    match origin_address(&r.origin) {
        Some(address) => address,
        None if r.origin_label.is_empty() => "a node".to_string(),
        None => r.origin_label.clone(),
    }
}

/// A node's address without its scheme or trailing slash
/// (`10.0.0.7:8080`, `tega.example`), or `None` if it has no usable address.
///
/// `127.0.0.1` and `localhost` always mean "this same computer", so a node
/// in the desktop app that says it lives at `http://127.0.0.1:8080` means
/// *its own* computer. Shown on the commons, that address would point at
/// whoever is reading the page, so it counts as no address at all. This is
/// also why it doesn't go through `layout::host_label`, which shows a
/// commons on this computer at this computer's network address: that is
/// right for a commons, and wrong for somebody else's node. Pure.
pub fn origin_address(origin: &str) -> Option<String> {
    let rest = origin.strip_prefix("https://").or_else(|| origin.strip_prefix("http://"))?;
    let host = rest.split('/').next().unwrap_or(rest);
    if host.is_empty() || is_loopback_host(host) {
        return None;
    }
    Some(host.to_string())
}

/// Whether a host (with or without a port) names this same computer:
/// `localhost`, `127.x.x.x` or `[::1]`. Pure.
fn is_loopback_host(host: &str) -> bool {
    // Drop the port. An IPv6 address is in brackets, and has colons of its
    // own, so for one of those keep what is inside the brackets.
    let name = match host.strip_prefix('[') {
        Some(inside) => inside.split(']').next().unwrap_or(inside),
        None => host.rsplit_once(':').map_or(host, |(name, _port)| name),
    };
    name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The commons a member can share to in one click: the default first, then
/// the commons the desktop app runs on this computer (`nearby`, address and
/// name), then every commons already shared to, without duplicates. Pure.
pub fn known_commons(default: &str, nearby: &[(String, String)], shared_to: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let nearby = nearby.iter().map(|(url, _)| url.as_str());
    for u in std::iter::once(default).chain(nearby).chain(shared_to.iter().map(String::as_str)) {
        let u = u.trim().trim_end_matches('/');
        if !u.is_empty() && !out.iter().any(|x| x == u) {
            out.push(u.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_commons_dedups_and_keeps_default_first() {
        let got = known_commons("http://a/", &[], &["http://b".into(), "http://a".into()]);
        assert_eq!(got, vec!["http://a".to_string(), "http://b".to_string()]);
        assert!(known_commons("", &[], &[]).is_empty());
        let near = vec![("http://127.0.0.1:8180".to_string(), "spark".to_string())];
        let got = known_commons("http://a", &near, &["http://127.0.0.1:8180".into()]);
        assert_eq!(
            got,
            vec!["http://a".to_string(), "http://127.0.0.1:8180".to_string()],
            "this computer's commons next"
        );
    }

    #[test]
    fn a_node_on_its_own_computer_has_no_address_to_show() {
        for own in ["http://127.0.0.1:8080", "http://localhost:8080/", "http://[::1]:8080", "http://127.0.0.1"] {
            assert_eq!(origin_address(own), None, "{own}");
        }
    }

    #[test]
    fn a_node_elsewhere_is_shown_at_its_own_address_unchanged() {
        assert_eq!(origin_address("http://10.0.0.7:8080/").as_deref(), Some("10.0.0.7:8080"));
        assert_eq!(origin_address("https://tega.example").as_deref(), Some("tega.example"));
        assert_eq!(origin_address("https://localhost.example").as_deref(), Some("localhost.example"));
        assert_eq!(origin_address("Tega's node"), None, "not an address");
    }

    #[test]
    fn a_node_without_an_address_is_shown_by_its_name() {
        let mut r = RemoteEntry {
            ed_pub: String::new(),
            box_pub: String::new(),
            handle: "tega".into(),
            display_name: "Tega".into(),
            origin: "http://127.0.0.1:8080".into(),
            origin_label: "Tega's node".into(),
            cid: String::new(),
            visibility: "public".into(),
            license: "cc-by".into(),
            signals: vec![],
            held_signals: Default::default(),
            created_at: 0,
            profile: Default::default(),
            signature: String::new(),
            received_at: 0,
            signed: None,
        };
        assert_eq!(origin_host(&r), "Tega's node");
        r.origin_label.clear();
        assert_eq!(origin_host(&r), "a node");
    }
}
