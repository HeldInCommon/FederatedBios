//! Vouching with a click (decision record 0026): who is waiting for a vouch
//! on each commons a member is listed on, as their own node shows it.
//!
//! The flow, end to end:
//!
//! 1. A member opens the **People** tab on their node.
//! 2. For each commons they're listed on, the node signs a fresh
//!    `WaitingListRequest` with the member's key
//!    ([`crate::vouching::make_waiting_request`]) and posts it to that
//!    commons' `/api/waiting` ([`ask`]). Only then, on page load: no
//!    background polling (as in decision record 0018).
//! 3. The commons checks the request is from a key it lists
//!    ([`crate::vouching::check_waiting_request`]) and answers with
//!    [`waiting_from`]: handle, name and key of each person waiting, and
//!    nothing else from their profile.
//! 4. The node turns the answer into a [`Lookup`] ([`lookup_from_answer`]),
//!    and the page shows each person with a **Vouch** button.
//! 5. Pressing it sends the usual signed `Vouch` (`POST /vouch`), and
//!    [`vouch_result`] turns the commons' answer into plain words.
//!
//! Everything here is pure except [`ask`] and [`ask_each`], which make the
//! requests (and write nothing).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::app::{AppState, Shared};
use crate::model::{HandleRec, PendingShare};
use crate::web::layout::host_label;
use crate::{handle, now, outbound, vouching};

/// How long the People tab waits for each commons before saying it couldn't
/// reach it. Short, because the page waits for it.
pub const ASK_TIMEOUT: Duration = Duration::from_secs(5);

/// The longest name shown for someone waiting. A commons could send
/// anything; this keeps one odd entry from swamping the page.
const NAME_MAX: usize = 80;

/// One person waiting for a vouch, as a member is shown them. Only what the
/// person already sent the commons for review: no bio, picture or links.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Waiting {
    /// The handle they asked for on that commons.
    pub handle: String,
    /// The name they gave (their handle, if they gave none).
    pub name: String,
    /// Their full public signing key, hex. A vouch must name it; the page
    /// shows its first characters as a fingerprint.
    pub key: String,
}

/// What a member's node found when it asked a commons who is waiting.
/// One variant per outcome, so the page must say something for each.
#[derive(Debug, PartialEq, Eq)]
pub enum Lookup {
    /// The commons answered; these people are waiting (perhaps nobody).
    Found(Vec<Waiting>),
    /// The commons refused to show its list, with its reason (for example,
    /// it no longer lists this member, or the clocks are far apart).
    Refused(String),
    /// The commons doesn't have one-click vouching (an older version):
    /// the member can still vouch by handle and key.
    NotOffered,
    /// No answer in time, or one that made no sense.
    Unreachable,
}

/// The waiting list a commons sends a member: for each share waiting for a
/// vouch, its handle, name and key, oldest first. Pure.
///
/// Privacy (decision record 0026): this deliberately leaves out the rest of
/// the profile (bio, picture, places, the node's address), which nobody
/// should see before the person is listed.
pub fn waiting_from(pending: &[PendingShare]) -> Vec<Waiting> {
    pending
        .iter()
        .map(|p| {
            let e = &p.entry;
            // The profile's name, else the sign-up name, else the handle.
            let name = if !e.profile.name.trim().is_empty() {
                e.profile.name.clone()
            } else {
                crate::model::display_name_or_handle(&e.display_name, &e.handle)
            };
            Waiting { handle: e.handle.clone(), name, key: e.ed_pub.clone() }
        })
        .collect()
}

/// The JSON a commons answers with: `{"waiting": [...]}`.
#[derive(Deserialize)]
struct Answer {
    /// The people waiting.
    waiting: Vec<Waiting>,
}

/// True when `key` looks like an Ed25519 public key in hex (64 characters).
pub fn is_full_key(key: &str) -> bool {
    key.len() == 64 && key.chars().all(|c| c.is_ascii_hexdigit())
}

/// Keep only entries a vouch could name (a valid handle and a full key), and
/// shorten very long names. A commons is another server, so its answer is
/// checked like any other input. Pure.
fn tidy(list: Vec<Waiting>) -> Vec<Waiting> {
    list.into_iter()
        .filter(|w| handle::is_valid(&w.handle) && is_full_key(&w.key))
        .map(|w| Waiting { name: w.name.chars().take(NAME_MAX).collect(), key: w.key.to_lowercase(), ..w })
        .collect()
}

/// Turn a commons' answer to a waiting-list request (its HTTP status and
/// body) into a [`Lookup`]. Pure.
pub fn lookup_from_answer(status: u16, body: &str) -> Lookup {
    match status {
        200 => match serde_json::from_str::<Answer>(body) {
            Ok(a) => Lookup::Found(tidy(a.waiting)),
            Err(_) => Lookup::Unreachable,
        },
        // No such route: a commons from before this feature (or not a commons).
        404 | 405 => Lookup::NotOffered,
        400..=499 => Lookup::Refused(sentence(body)),
        _ => Lookup::Unreachable,
    }
}

/// A server's short reason as a sentence: trimmed, first letter capital, a
/// full stop at the end, and cut short if it is long.
fn sentence(reason: &str) -> String {
    let trimmed: String = reason.trim().chars().take(200).collect();
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else { return "No reason given.".into() };
    let mut out: String = first.to_uppercase().chain(chars).collect();
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

/// Which addresses this node may contact for a member (decision record 0012).
fn policy(st: &AppState) -> outbound::Policy {
    outbound::Policy {
        personal: st.config.bind_addr(st.site.role).is_loopback(),
        allow_private: st.config.allow_private_share,
    }
}

/// Ask `commons` who is waiting there, signing the request with `rec`'s key.
/// Makes one network request and writes nothing. Gives up after
/// [`ASK_TIMEOUT`].
pub async fn ask(st: &AppState, rec: &HandleRec, commons: &str) -> Lookup {
    let Ok(request) = vouching::make_waiting_request(&rec.ed_pub, &rec.ed_priv, commons, now()) else {
        // Only fails if the stored key is damaged; nothing to ask with.
        return Lookup::Unreachable;
    };
    // The whole exchange (address check, connecting, answer) shares one
    // timeout, so a commons that is down can't hold the page for long.
    let exchange = async {
        let client = outbound::client_for(commons, policy(st)).await.ok()?;
        let resp = client.post(format!("{commons}/api/waiting")).json(&request).send().await.ok()?;
        let status = resp.status().as_u16();
        let body = resp.text().await.ok()?;
        Some(lookup_from_answer(status, &body))
    };
    match tokio::time::timeout(ASK_TIMEOUT, exchange).await {
        Ok(Some(lookup)) => lookup,
        // Refused by the address rules, no connection, or out of time.
        _ => Lookup::Unreachable,
    }
}

/// Ask each of `commons` who is waiting, all at once, and return the answers
/// in the same order. Writes nothing.
///
/// Learner note: `JoinSet` runs several tasks side by side. Each task must
/// own what it uses (`'static`), so it gets its own clone of the shared state
/// (an `Arc`, so cloning it is cheap) and of the member's record; `move`
/// hands those clones to the task.
pub async fn ask_each(st: &Shared, rec: &HandleRec, commons: &[String]) -> Vec<(String, Lookup)> {
    let mut tasks = tokio::task::JoinSet::new();
    for (i, c) in commons.iter().enumerate() {
        let (st, rec, c) = (st.clone(), rec.clone(), c.clone());
        tasks.spawn(async move { (i, ask(&st, &rec, &c).await) });
    }
    let mut answers: Vec<Option<Lookup>> = commons.iter().map(|_| None).collect();
    while let Some(done) = tasks.join_next().await {
        if let Ok((i, lookup)) = done {
            answers[i] = Some(lookup);
        }
    }
    // A task that panicked counts as unreachable rather than vanishing.
    commons.iter().cloned().zip(answers.into_iter().map(|a| a.unwrap_or(Lookup::Unreachable))).collect()
}

/// What happened when a member pressed **Vouch**.
#[derive(Debug, PartialEq, Eq)]
pub enum VouchResult {
    /// The commons listed them.
    Listed,
    /// The commons didn't, for this reason (in plain words).
    Refused(String),
}

/// Turn a commons' answer to a vouch for `@who` (its HTTP status and text)
/// into what to tell the member. Pure.
pub fn vouch_result(status: u16, text: &str, who: &str, commons: &str) -> VouchResult {
    let place = host_label(commons);
    if (200..300).contains(&status) {
        return VouchResult::Listed;
    }
    let why = match status {
        404 => format!(
            "@{who} isn't waiting on {place} any more. Someone may already have vouched for them, or the steward may have declined them."
        ),
        409 => format!("Someone else took the handle @{who} on {place} while they were waiting."),
        429 => format!("{place} is busy. Try again in a minute."),
        403 if text.contains("as many people") => format!(
            "You've vouched for as many people as {place} allows in a week. Try again next week, or ask its steward to approve @{who}."
        ),
        403 if text.contains("isn't a member") => {
            format!("{place} doesn't list you any more, so it can't take your vouch.")
        }
        400..=499 => format!("{place} didn't accept the vouch: {}", sentence(text)),
        _ => format!("{place} had a problem (HTTP {status}). Try again later."),
    };
    VouchResult::Refused(why)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Profile, RemoteEntry};

    /// A full-looking key made of one repeated hex digit.
    fn key(c: char) -> String {
        std::iter::repeat_n(c, 64).collect()
    }

    /// A share waiting on a commons, with a bio and a picture that must not
    /// be passed on.
    fn pending(handle: &str, name: &str) -> PendingShare {
        let entry = RemoteEntry {
            ed_pub: key('a'),
            box_pub: String::new(),
            handle: handle.into(),
            display_name: String::new(),
            origin: "http://node.example".into(),
            origin_label: "node".into(),
            cid: String::new(),
            visibility: "public".into(),
            license: "cc-by".into(),
            signals: vec![],
            held_signals: Default::default(),
            created_at: 0,
            profile: Profile {
                name: name.into(),
                bio: "private until listed".into(),
                avatar: "data:image/png;base64,AAAA".into(),
                ..Default::default()
            },
            signature: String::new(),
            received_at: 0,
            signed: None,
        };
        PendingShare { entry, message: serde_json::json!({}) }
    }

    #[test]
    fn the_waiting_list_holds_only_handle_name_and_key() {
        let list = waiting_from(&[pending("amy", "Amy Ng")]);
        assert_eq!(list, vec![Waiting { handle: "amy".into(), name: "Amy Ng".into(), key: key('a') }]);
        let json = serde_json::to_string(&list).unwrap();
        assert!(
            !json.contains("private until listed") && !json.contains("data:image") && !json.contains("node.example")
        );
    }

    #[test]
    fn someone_without_a_name_is_shown_by_handle() {
        assert_eq!(waiting_from(&[pending("amy", " ")])[0].name, "amy");
    }

    #[test]
    fn a_commons_answer_becomes_a_lookup() {
        let body = format!(r#"{{"waiting":[{{"handle":"amy","name":"Amy","key":"{}"}}]}}"#, key('b'));
        assert_eq!(
            lookup_from_answer(200, &body),
            Lookup::Found(vec![Waiting { handle: "amy".into(), name: "Amy".into(), key: key('b') }])
        );
        assert_eq!(lookup_from_answer(200, r#"{"waiting":[]}"#), Lookup::Found(vec![]));
        assert_eq!(lookup_from_answer(200, "not json"), Lookup::Unreachable);
        assert_eq!(lookup_from_answer(404, ""), Lookup::NotOffered);
        assert_eq!(lookup_from_answer(500, "boom"), Lookup::Unreachable);
        assert_eq!(
            lookup_from_answer(403, "whoever made this isn't a member of this commons"),
            Lookup::Refused("Whoever made this isn't a member of this commons.".into())
        );
    }

    #[test]
    fn entries_a_vouch_could_not_name_are_dropped() {
        let long_name: String = std::iter::repeat_n('x', 500).collect();
        let body = serde_json::json!({ "waiting": [
            { "handle": "Not A Handle!", "name": "x", "key": key('c') },
            { "handle": "amy", "name": "x", "key": "abc" },
            { "handle": "bo", "name": long_name, "key": key('D') },
        ]})
        .to_string();
        let Lookup::Found(list) = lookup_from_answer(200, &body) else { panic!("expected a list") };
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].handle, "bo");
        assert_eq!(list[0].name.chars().count(), NAME_MAX);
        assert_eq!(list[0].key, key('d'), "keys are kept in lower case");
    }

    #[test]
    fn a_vouch_answer_is_said_in_plain_words() {
        let c = "http://room:8081";
        assert_eq!(vouch_result(202, "accepted", "amy", c), VouchResult::Listed);
        let refused = |status, text| match vouch_result(status, text, "amy", c) {
            VouchResult::Refused(why) => why,
            VouchResult::Listed => panic!("expected a refusal"),
        };
        assert!(refused(404, "nobody by that handle and key is waiting here").contains("isn't waiting on room:8081"));
        assert!(refused(403, "that member has vouched for as many people as this commons allows this week")
            .contains("as many people as room:8081 allows in a week"));
        assert!(refused(403, "whoever made this isn't a member of this commons").contains("doesn't list you"));
        assert!(refused(409, "that handle was taken").contains("took the handle @amy"));
        assert!(refused(429, "").contains("busy"));
        assert!(refused(500, "").contains("HTTP 500"));
    }
}
