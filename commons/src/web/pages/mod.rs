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
//! | `owner.rs` | the panels a member sees under their own profile (share, vouch, backup, keys) |
//! | `publish.rs` | the publish form |
//! | `signin.rs` | sign-up, sign-in, and the "this is a shared commons" gate |
//! | `account.rs` | recovery code, key change, restore from backup |
//! | `sharing.rs` | results of sharing: shared, waiting for a vouch, vouched, withdrawn |
//! | `invites.rs` | invite codes made by a member or the steward |
//! | `federated.rs` | a profile shared in from another node |
//! | `about.rs` | how this works |

use crate::model::RemoteEntry;
use crate::web::layout::host_label;

mod about;
mod account;
mod directory;
mod federated;
mod invites;
mod owner;
mod profile;
mod publish;
mod sharing;
mod signin;

pub use about::about;
pub use account::{key_change, recovery_code, restore_form};
pub use directory::{directory, Card, StewardView, Viewer};
pub use federated::federated;
pub use invites::{invite_code, steward_invite};
pub use owner::OwnerPanel;
pub use profile::profile;
pub use publish::publish_form;
pub use sharing::{shared, shared_pending, vouched, withdrawn};
pub use signin::{commons_gate, join_form, login_form};

/// The host a shared profile says it came from, or its node's name if it
/// sent no usable address. Both are the sender's own claims.
pub fn origin_host(r: &RemoteEntry) -> String {
    if r.origin.starts_with("http") {
        host_label(&r.origin)
    } else {
        r.origin_label.clone()
    }
}

/// The commons a member can share to in one click: the default first, then
/// every commons already shared to, without duplicates.
pub fn known_commons(default: &str, shared_to: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for u in std::iter::once(default).chain(shared_to.iter().map(String::as_str)) {
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
        let got = known_commons("http://a/", &["http://b".into(), "http://a".into()]);
        assert_eq!(got, vec!["http://a".to_string(), "http://b".to_string()]);
        assert!(known_commons("", &[]).is_empty());
    }
}
