//! Following up on shares that are waiting for a vouch or approval.
//!
//! A commons can't contact nodes: a node listens only on its own computer
//! (decision record 0004) and is usually behind a home router, so a callback
//! would mean exposing it. So the node asks, but only when there's reason to:
//! **once each time it starts**, and **when the member presses "Check now"**.
//! No background polling. It asks each commons it is waiting on about each
//! waiting member (`GET /api/status`). When the answer changes it stops
//! waiting and records a [`Notice`]: shown as a banner on the member's pages,
//! and by the desktop app as a system notification.
//!
//! [`outcome`] is the pure rule; [`check_once`] does the asking and saving.

use crate::app::AppState;
use crate::model::Notice;
use crate::web::layout::host_label;
use crate::{now, outbound};

/// What a commons' answer means for a member waiting there.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Still waiting; check again later.
    StillWaiting,
    /// Stop waiting, and tell the member this.
    Done(String),
}

/// Turn a commons' status answer (`listed`, `waiting`, or anything else) for
/// `handle` into what to do.
pub fn outcome(status: &str, commons_url: &str, handle: &str) -> Outcome {
    let place = host_label(commons_url);
    match status {
        "waiting" => Outcome::StillWaiting,
        "listed" => Outcome::Done(format!("@{handle} is now listed on {place}.")),
        _ => Outcome::Done(format!(
            "{place} didn't list @{handle}: its steward may have declined. Share again with an invite code, or ask a member there to vouch for you."
        )),
    }
}

/// Ask one commons about one member. `None` if the commons can't be reached
/// (try again next time).
async fn ask(st: &AppState, commons: &str, handle: &str, key: &str) -> Option<String> {
    let policy = st.config.share_policy(st.site.role);
    let client = outbound::client_for(commons, policy).await.ok()?;
    let j: serde_json::Value = client
        .get(format!("{commons}/api/status"))
        .query(&[("handle", handle), ("key", key)])
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    j.get("status").and_then(|s| s.as_str()).map(String::from)
}

/// Check every waiting share once. **Writes** members' records (no longer
/// waiting) and `notices.json` (a notice per change). Returns how many
/// notices were added.
pub async fn check_once(st: &AppState) -> usize {
    let mut added = 0;
    for rec in st.store.handles().into_iter().filter(|r| !r.waiting_on.is_empty()) {
        for commons in rec.waiting_on.clone() {
            let Some(status) = ask(st, &commons, &rec.handle, &rec.ed_pub).await else { continue };
            let Outcome::Done(text) = outcome(&status, &commons, &rec.handle) else { continue };
            let saved = crate::web::handlers::update_member(st, &rec.handle, |r| r.without_waiting_on(&commons))
                .and_then(|_| {
                    let n = Notice { handle: rec.handle.clone(), text, at: now(), seen: false, pushed: false };
                    st.store.add_notice(n).map_err(|e| e.to_string())
                });
            match saved {
                Ok(()) => added += 1,
                Err(e) => eprintln!("warning: couldn't record news for @{}: {e}", rec.handle),
            }
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_and_declined_end_the_wait_waiting_doesnt() {
        assert_eq!(outcome("waiting", "http://c:8081", "ada"), Outcome::StillWaiting);
        assert_eq!(outcome("listed", "http://c:8081", "ada"), Outcome::Done("@ada is now listed on c:8081.".into()));
        assert!(
            matches!(outcome("not_listed", "http://c:8081", "ada"), Outcome::Done(t) if t.contains("didn't list @ada"))
        );
    }
}
