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
//! | `schematics.rs` | the diagrams on How this works and Run your own commons |
//! | `guide.rs` | run your own commons: questions and answers, and a practice commons |
//! | `stories.rs` | story lists: a node's, and a commons' (with recent withdrawals) |
//! | `story.rs` | one story: the author's view with its sharing tools, and a commons' |
//! | `here.rs` | a story shared "Only while I'm here": its text fetched just now, or "Not here right now" |
//! | `story_form.rs` | writing a story (new or editing), then its rights step |
//! | `rights.rs` | Sharing and rights: who can see it, licence, signals, Held in Common Signals, how a story travels |
//! | `parts.rs` | shared pieces: page heading, section label, starburst, terms panel, "2 min ago" |

use crate::model::RemoteEntry;
use crate::web::layout::host_label;

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
pub use sharing::{shared, shared_pending, vouch_outcome, withdrawn};
pub use signin::{commons_gate, join_form, login_form};
pub use stories::{author_stories, commons_stories, node_stories};
pub use story::{held_story, own_story, StoryOutcome, StoryOwner};
pub use story_form::{rights_form, write_form};

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
