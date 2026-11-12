//! The node's side of "Only while I'm here" (decision record 0021): keeping
//! one outgoing connection open to each commons that holds a card of ours,
//! and answering readers' requests over it.
//!
//! **The node still accepts nothing** (decision record 0004). It asks the
//! commons "anything for me?" (`POST /here/poll`, a long poll: the commons
//! holds the request open until a reader asks, or for about 25 seconds),
//! answers whatever comes back in its next poll, and asks again. When the
//! laptop sleeps or the app closes, the polls stop, and readers on the
//! commons see "Not here right now".
//!
//! The rules are pure and tested on their own:
//!
//! - [`connections_needed`]: which (commons, handle) pairs need a connection;
//! - [`answers_for`]: what to answer for each request (only stories shared
//!   to that commons as "Only while I'm here", never members-only ones);
//! - [`Link`]: the connection's state (`Connecting`, `Open`, `Retrying`,
//!   `Unsupported`), with [`Link::next`] for each outcome.
//!
//! [`run`] is the loop that applies them. It reads the data folder and sends
//! requests; it **writes nothing to disk**. The only thing it changes is
//! [`Links`], the in-memory status the story page shows.

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::Duration;

use crate::app::Shared;
use crate::here::{build_relay, Answer, PollReply, Requested, ANSWERS_MAX};
use crate::model::Reach;
use crate::now;
use crate::story::{OwnStory, Travel};

/// How often the loop checks which connections are needed (a story newly
/// shared as "Only while I'm here" connects within this long).
const LOOK_EVERY: Duration = Duration::from_secs(2);

/// How long the node waits for one poll's reply. Longer than any hold a
/// commons may use (`HIC_RELAY_HOLD` is at most 60 seconds).
const POLL_TIMEOUT: Duration = Duration::from_secs(75);

/// The longest wait between retries after failures, in seconds.
const RETRY_MAX: u64 = 30;

/// How long to wait before asking again a commons that doesn't support
/// "Only while I'm here" (it may be upgraded), in seconds.
const UNSUPPORTED_RETRY: u64 = 60;

/// The state of the node's connection to one commons, for one member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// About to ask for the first time.
    Connecting,
    /// The last poll was answered: readers there can read our stories.
    Open,
    /// The last `failures` polls in a row failed (no network, commons down).
    Retrying {
        /// How many in a row.
        failures: u32,
    },
    /// The commons answered 404: it predates "Only while I'm here".
    Unsupported,
}

/// What happened to one poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkEvent {
    /// The commons answered it (with requests, or "nothing yet").
    Polled,
    /// The commons doesn't know `/here/poll`.
    NotSupported,
    /// It failed: unreachable, refused, or a reply we couldn't read.
    Failed,
}

impl Link {
    /// The state after `event`.
    pub fn next(self, event: LinkEvent) -> Link {
        match (self, event) {
            (_, LinkEvent::Polled) => Link::Open,
            (_, LinkEvent::NotSupported) => Link::Unsupported,
            (Link::Retrying { failures }, LinkEvent::Failed) => Link::Retrying { failures: failures.saturating_add(1) },
            (_, LinkEvent::Failed) => Link::Retrying { failures: 1 },
        }
    }

    /// How long to wait before the next poll: none while it's working; after
    /// failures, 1, 2, 4 … up to 30 seconds, so a commons that's down isn't
    /// flooded; a minute for a commons that doesn't support this.
    pub fn wait_before_next(self) -> Option<Duration> {
        match self {
            Link::Connecting | Link::Open => None,
            Link::Retrying { failures } => {
                let secs = 1u64.checked_shl(failures.saturating_sub(1)).unwrap_or(RETRY_MAX).min(RETRY_MAX);
                Some(Duration::from_secs(secs))
            }
            Link::Unsupported => Some(Duration::from_secs(UNSUPPORTED_RETRY)),
        }
    }
}

/// The (commons address, handle) pairs this node must keep a connection
/// open for: every commons where one of a member's stories is shared as
/// "Only while I'm here".
pub fn connections_needed(stories: &[OwnStory]) -> BTreeSet<(String, String)> {
    let mut needed = BTreeSet::new();
    for s in stories {
        for commons in &s.shared_to {
            if s.shared_as(commons) == Some(Travel::Here) {
                needed.insert((commons.clone(), s.story.handle.clone()));
            }
        }
    }
    needed
}

/// The answers to `requests` from `commons`, for `handle`'s stories: the
/// full text of each story that this member shared there as "Only while I'm
/// here". Anything else (a story shared as a copy, members-only, someone
/// else's, or no longer shared there) gets no answer, and the reader is told
/// it isn't here.
pub fn answers_for(requests: &[Requested], stories: &[OwnStory], commons: &str, handle: &str) -> Vec<Answer> {
    requests
        .iter()
        .take(ANSWERS_MAX)
        .filter_map(|r| {
            let s = stories.iter().find(|s| s.story.id == r.story)?;
            let shared_here = s.story.handle == handle
                && s.shared_as(commons) == Some(Travel::Here)
                && s.story.reach() != Reach::Members;
            shared_here.then(|| Answer {
                request: r.request.clone(),
                story: r.story.clone(),
                text: s.story.text.clone(),
            })
        })
        .collect()
}

/// The next poll's number: higher than `last`, and based on the clock
/// (milliseconds) so it keeps rising after the node restarts.
pub fn next_seq(last: u64, now_secs: u64) -> u64 {
    (last + 1).max(now_secs.saturating_mul(1000))
}

/// The state of each connection, for the story page. In memory only (house
/// style §1 allows shared in-memory state behind a lock).
#[derive(Default)]
pub struct Links {
    /// By (commons address, handle).
    inner: Mutex<HashMap<(String, String), Link>>,
}

impl Links {
    /// The connection to `commons` for `handle`, if there is one.
    pub fn status(&self, commons: &str, handle: &str) -> Option<Link> {
        self.inner.lock().unwrap().get(&(commons.to_string(), handle.to_string())).copied()
    }

    /// **Records** the connection's state.
    fn record(&self, commons: &str, handle: &str, link: Link) {
        self.inner.lock().unwrap().insert((commons.to_string(), handle.to_string()), link);
    }

    /// **Forgets** a connection that's no longer needed.
    fn forget(&self, commons: &str, handle: &str) {
        self.inner.lock().unwrap().remove(&(commons.to_string(), handle.to_string()));
    }
}

/// Why one poll didn't get requests back.
enum PollError {
    /// 404: the commons predates "Only while I'm here".
    NotSupported,
    /// Anything else (the reason is for the operator's log).
    Failed(String),
}

/// Keep every needed connection open, for as long as the node runs: every
/// `LOOK_EVERY` (2 seconds), start a `keep_open` task for each pair
/// [`connections_needed`] lists that doesn't have one. Each task stops itself
/// when its pair is no longer needed (the story was withdrawn, or switched
/// to a copy).
///
/// Rust note: `tokio::spawn` runs a task alongside this one; its
/// `JoinHandle` says whether it has finished.
pub async fn run(st: Shared) {
    let mut running: HashMap<(String, String), tokio::task::JoinHandle<()>> = HashMap::new();
    loop {
        running.retain(|_, task| !task.is_finished());
        for pair in connections_needed(&st.store.own_stories()) {
            // `entry(...).or_insert_with`: start a task only if this pair has none.
            running.entry(pair.clone()).or_insert_with(|| {
                let (commons, handle) = pair;
                // `move`: the task takes its own copies of these to keep.
                let st = st.clone();
                tokio::spawn(async move { keep_open(st, commons, handle).await })
            });
        }
        tokio::time::sleep(LOOK_EVERY).await;
    }
}

/// Poll `commons` for `handle` again and again, answering what it asks,
/// until no story of theirs is shared there as "Only while I'm here". Each
/// poll carries the answers to the last one's requests.
async fn keep_open(st: Shared, commons: String, handle: String) {
    let mut link = Link::Connecting;
    let mut seq = 0;
    let mut answers: Vec<Answer> = Vec::new();
    loop {
        // Still needed? (Re-read each time: the member may have withdrawn.)
        if !connections_needed(&st.store.own_stories()).contains(&(commons.clone(), handle.clone())) {
            st.links.forget(&commons, &handle);
            return;
        }
        st.links.record(&commons, &handle, link);
        seq = next_seq(seq, now());
        let event = match poll_once(&st, &commons, &handle, seq, &answers).await {
            Ok(reply) => {
                // Answer from the stories as they are now.
                answers = answers_for(&reply.requests, &st.store.own_stories(), &commons, &handle);
                LinkEvent::Polled
            }
            Err(PollError::NotSupported) => {
                answers.clear();
                LinkEvent::NotSupported
            }
            Err(PollError::Failed(why)) => {
                // Answers that didn't arrive are dropped: those readers have
                // been told the author isn't here, so there's no one to send
                // them to. Only the commons and the cause are logged, never text.
                answers.clear();
                // Log the first failure in a row, not every retry.
                if !matches!(link, Link::Retrying { .. }) {
                    eprintln!("note: Only while I'm here: can't reach {commons} for @{handle}: {why}");
                }
                LinkEvent::Failed
            }
        };
        link = link.next(event);
        st.links.record(&commons, &handle, link);
        if let Some(wait) = link.wait_before_next() {
            tokio::time::sleep(wait).await;
        }
    }
}

/// One poll: sign it (with the member's current key), send it with
/// `answers`, and read the requests that come back.
async fn poll_once(
    st: &Shared,
    commons: &str,
    handle: &str,
    seq: u64,
    answers: &[Answer],
) -> Result<PollReply, PollError> {
    let rec = match st.store.handle(handle) {
        Ok(Some(r)) => r,
        Ok(None) => return Err(PollError::Failed("the member is gone".into())),
        Err(e) => return Err(PollError::Failed(e.to_string())),
    };
    let msg = build_relay(&rec.ed_pub, &rec.ed_priv, commons, seq, answers, now()).map_err(PollError::Failed)?;
    let resp = crate::web::handlers::post_to_commons_at(st, commons, "/here/poll", &msg, POLL_TIMEOUT)
        .await
        .map_err(PollError::Failed)?;
    match resp.status().as_u16() {
        200 => resp.json::<PollReply>().await.map_err(|e| PollError::Failed(e.to_string())),
        404 => Err(PollError::NotSupported),
        code => {
            let why = resp.text().await.unwrap_or_default();
            Err(PollError::Failed(format!("{code} {why}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;
    use crate::story::{new_story, sign};

    /// The commons these tests share to.
    const ROOM: &str = "http://room:8081";

    /// @`handle_`'s story `id`, shared to ROOM the way `how` says.
    fn shared(handle_: &str, id: &str, visibility: &str, how: Travel) -> OwnStory {
        let k = keys::generate();
        let s = new_story(id, handle_, "Night bus", "The 38 sighs.", visibility, "cc-by", &[]).unwrap();
        OwnStory::new(s.clone(), sign(&s, &k.ed_pub, &k.ed_priv, 1).unwrap()).with_shared_as(ROOM, 2, how)
    }

    /// A request for story `id`.
    fn asks_for(id: &str) -> Requested {
        Requested { request: "a".repeat(32), story: id.into() }
    }

    #[test]
    fn only_cards_need_a_connection() {
        let here = shared("ada", "000000000000000a", "commons", Travel::Here);
        let copy = shared("ada", "000000000000000b", "commons", Travel::Copy);
        let bea = shared("bea", "000000000000000c", "commons", Travel::Here);
        let needed = connections_needed(&[here, copy, bea]);
        let want: BTreeSet<(String, String)> =
            [(ROOM.to_string(), "ada".to_string()), (ROOM.to_string(), "bea".to_string())].into();
        assert_eq!(needed, want);
        assert!(connections_needed(&[shared("ada", "000000000000000b", "commons", Travel::Copy)]).is_empty());
    }

    #[test]
    fn a_withdrawn_card_closes_the_connection() {
        let s = shared("ada", "000000000000000a", "commons", Travel::Here).without_shared_to(ROOM);
        assert!(connections_needed(&[s]).is_empty());
    }

    #[test]
    fn the_node_answers_only_for_its_own_cards_on_that_commons() {
        let here = shared("ada", "000000000000000a", "commons", Travel::Here);
        let copy = shared("ada", "000000000000000b", "commons", Travel::Copy);
        let stories = [here, copy];
        let got = answers_for(&[asks_for("000000000000000a")], &stories, ROOM, "ada");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "The 38 sighs.");
        assert!(answers_for(&[asks_for("000000000000000b")], &stories, ROOM, "ada").is_empty(), "a copy");
        assert!(answers_for(&[asks_for("000000000000000a")], &stories, "http://elsewhere", "ada").is_empty());
        assert!(answers_for(&[asks_for("000000000000000a")], &stories, ROOM, "bea").is_empty(), "someone else's");
        assert!(answers_for(&[asks_for("ffffffffffffffff")], &stories, ROOM, "ada").is_empty(), "unknown");
    }

    #[test]
    fn a_withdrawn_or_members_only_story_is_never_sent() {
        let gone = shared("ada", "000000000000000a", "commons", Travel::Here).without_shared_to(ROOM);
        assert!(answers_for(&[asks_for("000000000000000a")], &[gone], ROOM, "ada").is_empty());
        let members = shared("ada", "000000000000000a", "members", Travel::Here);
        assert!(answers_for(&[asks_for("000000000000000a")], &[members], ROOM, "ada").is_empty());
    }

    #[test]
    fn the_link_backs_off_and_recovers() {
        let failing = Link::Connecting.next(LinkEvent::Failed);
        assert_eq!(failing, Link::Retrying { failures: 1 });
        assert_eq!(failing.wait_before_next(), Some(Duration::from_secs(1)));
        let worse = failing.next(LinkEvent::Failed).next(LinkEvent::Failed);
        assert_eq!(worse.wait_before_next(), Some(Duration::from_secs(4)));
        let very = Link::Retrying { failures: 40 };
        assert_eq!(very.wait_before_next(), Some(Duration::from_secs(RETRY_MAX)), "capped");
        assert_eq!(worse.next(LinkEvent::Polled), Link::Open);
        assert_eq!(Link::Open.wait_before_next(), None, "ask again at once");
        let old = Link::Open.next(LinkEvent::NotSupported);
        assert_eq!(old, Link::Unsupported);
        assert_eq!(old.wait_before_next(), Some(Duration::from_secs(UNSUPPORTED_RETRY)));
    }

    #[test]
    fn poll_numbers_keep_rising() {
        assert_eq!(next_seq(0, 100), 100_000);
        assert_eq!(next_seq(100_000, 100), 100_001, "two polls in one second");
        assert_eq!(next_seq(100_001, 50), 100_002, "a clock that went back");
    }
}
