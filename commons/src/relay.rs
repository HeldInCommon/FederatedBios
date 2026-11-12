//! The commons' side of "Only while I'm here" (decision record 0021): who is
//! here now, and the readers waiting for a story's text. In memory only.
//!
//! Nothing here touches the disk, and no story text is ever kept beyond the
//! moment it is handed to the reader who asked for it. That is the point of
//! "Only while I'm here": the commons holds the signed card, never the text.
//!
//! The states are explicit enums with pure transitions, so the rules can be
//! tested one line at a time:
//!
//! - [`Presence`]: is the author's node connected? `Away`, `Listening` (a
//!   poll is open now) or `Between` (one just ended; the node normally asks
//!   again within a moment).
//! - [`RequestState`]: a reader's request: `Waiting` for the node to collect
//!   it, `Sent` to the node, then `Answered` with checked text, or `Changed`
//!   when the text didn't match the signed hash.
//!
//! [`Relay`] holds both behind one lock (house style §1 allows shared
//! in-memory state behind a lock) and applies the transitions. The request
//! handlers in `web/handlers/here.rs` do the waiting.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::here::{text_matches, Answer, Card, Requested, ANSWERS_MAX};

/// How long the commons holds a poll open before answering "nothing yet",
/// and how long a reader waits for the node, in seconds. Set from `Config`
/// (`HIC_RELAY_HOLD`, `HIC_RELAY_WAIT`) so tests can make them short.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// A poll is answered after this long even if nobody asked (default 25 s,
    /// under the 30 s many networks allow an idle connection).
    pub hold: u64,
    /// A reader waits this long for the node's answer (default 8 s).
    pub wait: u64,
}

/// The default timing (see [`Timing`]).
pub const DEFAULT_TIMING: Timing = Timing { hold: 25, wait: 8 };

/// After a poll ends, how long the author still counts as here while their
/// node asks again, in seconds.
pub const GRACE: u64 = 10;

/// The most readers waiting for one author at once. More are told to try
/// again shortly, so a crowd (or someone hammering reload) can't make the
/// commons hold unbounded requests or flood the author's laptop.
pub const MAX_WAITING_PER_AUTHOR: usize = ANSWERS_MAX / 2;

/// Is the author's node connected to this commons?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    /// Not connected: never was, went quiet, or failed to answer a reader.
    Away,
    /// A poll is open now (number `poll`, since `since`): a request is handed
    /// over as soon as it arrives.
    Listening {
        /// When this poll opened.
        since: u64,
        /// Which poll it is (each new poll gets a new number).
        poll: u64,
    },
    /// The last poll ended at `at`; the node normally asks again at once.
    Between {
        /// When the last poll ended.
        at: u64,
    },
}

/// Something that happens to an author's connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresenceEvent {
    /// Their node opened poll number `poll` at `at`.
    PollOpened {
        /// When.
        at: u64,
        /// Its number.
        poll: u64,
    },
    /// Poll number `poll` ended at `at` (answered, or held for its full time).
    PollClosed {
        /// When.
        at: u64,
        /// Its number.
        poll: u64,
    },
    /// A reader waited the full time and the node didn't answer: it's gone
    /// (asleep, closed, off the network) even if a poll looks open.
    ReaderGaveUp,
}

impl Presence {
    /// The state after `event`. A newer poll always wins; an older poll
    /// closing after a newer one opened changes nothing.
    pub fn next(self, event: PresenceEvent) -> Presence {
        match (self, event) {
            (_, PresenceEvent::PollOpened { at, poll }) => Presence::Listening { since: at, poll },
            (Presence::Listening { poll: open, .. }, PresenceEvent::PollClosed { at, poll }) if open == poll => {
                Presence::Between { at }
            }
            (state, PresenceEvent::PollClosed { .. }) => state,
            (_, PresenceEvent::ReaderGaveUp) => Presence::Away,
        }
    }

    /// True when a reader should be told the author is here, at `now`: a
    /// poll is open (and hasn't outlived its hold, which would mean the
    /// connection died without closing), or one ended a moment ago.
    pub fn is_here(&self, now: u64, timing: Timing) -> bool {
        match *self {
            Presence::Away => false,
            Presence::Listening { since, .. } => now <= since + timing.hold + GRACE,
            Presence::Between { at } => now <= at + GRACE,
        }
    }
}

/// Where one reader's request is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestState {
    /// Made; waiting for the author's node to collect it.
    Waiting,
    /// Handed to the node in a poll's reply; waiting for its answer.
    Sent,
    /// The node answered and the text matched the card's signed hash. The
    /// text stays only until the reader's page takes it.
    Answered(String),
    /// The node answered with text that doesn't match the card: the story
    /// was changed after the card was shared (or something altered it).
    Changed,
}

/// Something that happens to a reader's request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestEvent {
    /// The node collected it in a poll.
    HandedOver,
    /// The node answered with `text`; `matches` says whether it matched the
    /// card's signed hash (see [`crate::here::text_matches`]).
    Answered {
        /// The text the node sent.
        text: String,
        /// Whether it matched the signed hash.
        matches: bool,
    },
}

/// A move the request rules don't allow (for example, an answer to a
/// request that was never handed over: nobody could have known its id).
#[derive(Debug, PartialEq, Eq)]
pub struct NotExpected;

impl RequestState {
    /// The state after `event`, or [`NotExpected`] for a move the rules
    /// don't allow. Only a request the node was handed can be answered, and
    /// only once.
    pub fn next(self, event: RequestEvent) -> Result<RequestState, NotExpected> {
        match (self, event) {
            (RequestState::Waiting, RequestEvent::HandedOver) => Ok(RequestState::Sent),
            (RequestState::Sent, RequestEvent::Answered { text, matches: true }) => Ok(RequestState::Answered(text)),
            (RequestState::Sent, RequestEvent::Answered { matches: false, .. }) => Ok(RequestState::Changed),
            _ => Err(NotExpected),
        }
    }
}

/// What a reader gets, in the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading {
    /// The text, checked against the card's signed hash.
    Text(String),
    /// The author's node isn't connected, or didn't answer in time.
    Away,
    /// The node's text didn't match the card (see [`RequestState::Changed`]).
    Changed,
    /// Too many readers are waiting for this author; try again shortly.
    Busy,
    /// The card was withdrawn while the reader waited.
    Gone,
}

/// One reader's request, while it lasts.
#[derive(Clone, Debug)]
struct Pending {
    /// The author's key: only a poll signed by this key may answer.
    author: String,
    /// The story wanted.
    story: String,
    /// Where it is.
    state: RequestState,
}

/// Everything behind the relay's lock.
#[derive(Default)]
struct Inner {
    /// Each author's connection, by key. Missing means [`Presence::Away`].
    presence: HashMap<String, Presence>,
    /// Readers' requests, by request id.
    requests: HashMap<String, Pending>,
    /// The highest poll number taken from each author, so an old poll can't
    /// be replayed. In memory: after a commons restarts, the signed time
    /// check (`crate::here::RELAY_CLOCK_SKEW`) still limits replays.
    last_seq: HashMap<String, u64>,
    /// The number the next opened poll gets.
    next_poll: u64,
}

/// Why a poll was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum PollRefused {
    /// Its `seq` wasn't higher than the last one taken from this author.
    Replayed,
}

/// The commons' relay: presence and waiting readers, in memory.
#[derive(Default)]
pub struct Relay {
    /// The state, behind one lock (see the module notes).
    inner: Mutex<Inner>,
}

impl Relay {
    /// True when `author` is here now (see [`Presence::is_here`]).
    pub fn is_here(&self, author: &str, now: u64, timing: Timing) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.presence.get(author).copied().unwrap_or(Presence::Away).is_here(now, timing)
    }

    /// **Records** a reader's request for `story` by `author` and returns its
    /// id (`new_id`, made by the caller so this stays free of randomness),
    /// or [`Reading::Busy`] when too many are already waiting for them.
    pub fn record_request(&self, author: &str, story: &str, new_id: String) -> Result<String, Reading> {
        let mut inner = self.inner.lock().unwrap();
        let waiting = inner.requests.values().filter(|p| p.author == author).count();
        if waiting >= MAX_WAITING_PER_AUTHOR {
            return Err(Reading::Busy);
        }
        let pending = Pending { author: author.into(), story: story.into(), state: RequestState::Waiting };
        inner.requests.insert(new_id.clone(), pending);
        Ok(new_id)
    }

    /// The outcome of request `id` if it has one, **removing** the request
    /// (and so the text) when it does. `None` while the node hasn't answered.
    pub fn take_reading(&self, id: &str) -> Option<Reading> {
        let mut inner = self.inner.lock().unwrap();
        let outcome = match inner.requests.get(id).map(|p| &p.state) {
            None => return Some(Reading::Gone),
            Some(RequestState::Answered(text)) => Reading::Text(text.clone()),
            Some(RequestState::Changed) => Reading::Changed,
            Some(RequestState::Waiting | RequestState::Sent) => return None,
        };
        inner.requests.remove(id);
        Some(outcome)
    }

    /// **Removes** request `id` after the reader waited the full time, and
    /// **marks** its author away: their node didn't answer, so the next
    /// reader is told at once instead of waiting too.
    pub fn record_gave_up(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(p) = inner.requests.remove(id) {
            let state = inner.presence.get(&p.author).copied().unwrap_or(Presence::Away);
            inner.presence.insert(p.author, state.next(PresenceEvent::ReaderGaveUp));
        }
    }

    /// **Opens** a poll from `author` numbered `seq`, at `now`: refuses a
    /// replayed number, then marks them listening. Returns the poll's own
    /// number, for [`Relay::is_current_poll`] and [`Relay::record_poll_closed`].
    pub fn open_poll(&self, author: &str, seq: u64, now: u64) -> Result<u64, PollRefused> {
        let mut inner = self.inner.lock().unwrap();
        if inner.last_seq.get(author).is_some_and(|last| seq <= *last) {
            return Err(PollRefused::Replayed);
        }
        inner.last_seq.insert(author.into(), seq);
        inner.next_poll += 1;
        let poll = inner.next_poll;
        let state = inner.presence.get(author).copied().unwrap_or(Presence::Away);
        inner.presence.insert(author.into(), state.next(PresenceEvent::PollOpened { at: now, poll }));
        Ok(poll)
    }

    /// **Applies** a poll's answers from `author`: each must answer a request
    /// that was handed to this author, for the same story; its text is
    /// checked against that story's card (`card_of`, the cards held here).
    /// Answers to anything else are ignored: a node can't answer for
    /// another author, or for a request it was never given.
    pub fn apply_answers<'a>(&self, author: &str, answers: &[Answer], card_of: impl Fn(&str) -> Option<&'a Card>) {
        let mut inner = self.inner.lock().unwrap();
        for a in answers {
            let Some(p) = inner.requests.get_mut(&a.request) else { continue };
            if p.author != author || p.story != a.story {
                continue;
            }
            // No card any more (withdrawn): leave it; the reader sees Gone.
            let Some(card) = card_of(&a.story) else { continue };
            let event = RequestEvent::Answered { text: a.text.clone(), matches: text_matches(card, &a.text) };
            if let Ok(next) = p.state.clone().next(event) {
                p.state = next;
            }
        }
    }

    /// **Hands over** the requests waiting for `author`: marks them sent and
    /// returns them, for the poll's reply.
    pub fn hand_over(&self, author: &str) -> Vec<Requested> {
        let mut inner = self.inner.lock().unwrap();
        let mut out = Vec::new();
        for (id, p) in inner.requests.iter_mut().filter(|(_, p)| p.author == author) {
            if let Ok(next) = p.state.clone().next(RequestEvent::HandedOver) {
                p.state = next;
                out.push(Requested { request: id.clone(), story: p.story.clone() });
            }
        }
        out
    }

    /// True while poll `poll` is `author`'s newest: an older one stops
    /// waiting as soon as a newer one opens.
    pub fn is_current_poll(&self, author: &str, poll: u64) -> bool {
        let inner = self.inner.lock().unwrap();
        matches!(inner.presence.get(author), Some(Presence::Listening { poll: open, .. }) if *open == poll)
    }

    /// **Records** that poll `poll` from `author` ended at `now`.
    pub fn record_poll_closed(&self, author: &str, poll: u64, now: u64) {
        let mut inner = self.inner.lock().unwrap();
        let state = inner.presence.get(author).copied().unwrap_or(Presence::Away);
        inner.presence.insert(author.into(), state.next(PresenceEvent::PollClosed { at: now, poll }));
    }

    /// **Removes** every request for `story` by `author` (it was withdrawn,
    /// or replaced by a copy): waiting readers then see [`Reading::Gone`],
    /// and a late answer finds nothing to fill.
    pub fn remove_story(&self, author: &str, story: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.requests.retain(|_, p| !(p.author == author && p.story == story));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::here::{card_for, text_sha256};
    use crate::story::new_story;

    /// Short timings for these tests.
    const T: Timing = Timing { hold: 25, wait: 8 };

    /// A card for a story with this text.
    fn card(text: &str) -> Card {
        let s = new_story("0123456789abcdef", "ada", "Night bus", text, "commons", "cc-by", &[]).unwrap();
        card_for(&s, "http://room:8081")
    }

    #[test]
    fn presence_follows_the_polls() {
        let away = Presence::Away;
        assert!(!away.is_here(100, T));
        let open = away.next(PresenceEvent::PollOpened { at: 100, poll: 1 });
        assert_eq!(open, Presence::Listening { since: 100, poll: 1 });
        assert!(open.is_here(100 + T.hold, T));
        assert!(!open.is_here(100 + T.hold + GRACE + 1, T), "a poll that never closed has died");
        let between = open.next(PresenceEvent::PollClosed { at: 120, poll: 1 });
        assert_eq!(between, Presence::Between { at: 120 });
        assert!(between.is_here(120 + GRACE, T));
        assert!(!between.is_here(120 + GRACE + 1, T), "didn't come back");
    }

    #[test]
    fn an_older_poll_closing_does_not_undo_a_newer_one() {
        let newer = Presence::Listening { since: 110, poll: 2 };
        assert_eq!(newer.next(PresenceEvent::PollClosed { at: 111, poll: 1 }), newer);
    }

    #[test]
    fn a_reader_who_gives_up_marks_the_author_away() {
        let open = Presence::Listening { since: 100, poll: 3 };
        assert_eq!(open.next(PresenceEvent::ReaderGaveUp), Presence::Away);
    }

    #[test]
    fn a_request_moves_waiting_sent_answered() {
        let sent = RequestState::Waiting.next(RequestEvent::HandedOver).unwrap();
        assert_eq!(sent, RequestState::Sent);
        let ok = sent.clone().next(RequestEvent::Answered { text: "words".into(), matches: true }).unwrap();
        assert_eq!(ok, RequestState::Answered("words".into()));
        let changed = sent.next(RequestEvent::Answered { text: "other".into(), matches: false }).unwrap();
        assert_eq!(changed, RequestState::Changed);
    }

    #[test]
    fn a_request_never_handed_over_cannot_be_answered() {
        let e = RequestEvent::Answered { text: "x".into(), matches: true };
        assert_eq!(RequestState::Waiting.next(e.clone()), Err(NotExpected));
        assert_eq!(RequestState::Answered("x".into()).next(e), Err(NotExpected), "only once");
        assert_eq!(RequestState::Sent.next(RequestEvent::HandedOver), Err(NotExpected));
    }

    #[test]
    fn a_reader_gets_the_checked_text_and_it_is_not_kept() {
        let relay = Relay::default();
        let c = card("The 38 sighs.");
        let poll = relay.open_poll("ada-key", 1, 100).unwrap();
        assert!(relay.is_here("ada-key", 100, T) && relay.is_current_poll("ada-key", poll));
        let id = relay.record_request("ada-key", &c.id, "r1".into()).unwrap();
        assert_eq!(relay.take_reading(&id), None, "nothing yet");
        let handed = relay.hand_over("ada-key");
        assert_eq!(handed, vec![Requested { request: "r1".into(), story: c.id.clone() }]);
        assert!(relay.hand_over("ada-key").is_empty(), "handed over once");
        let answer = Answer { request: id.clone(), story: c.id.clone(), text: "The 38 sighs.".into() };
        relay.apply_answers("ada-key", &[answer], |_| Some(&c));
        assert_eq!(relay.take_reading(&id), Some(Reading::Text("The 38 sighs.".into())));
        assert_eq!(relay.take_reading(&id), Some(Reading::Gone), "the text was handed over and dropped");
    }

    #[test]
    fn altered_text_is_caught_and_never_shown() {
        let relay = Relay::default();
        let c = card("The 38 sighs.");
        assert_eq!(c.text_sha256, text_sha256("The 38 sighs."));
        relay.open_poll("ada-key", 1, 100).unwrap();
        let id = relay.record_request("ada-key", &c.id, "r1".into()).unwrap();
        relay.hand_over("ada-key");
        let altered = Answer { request: id.clone(), story: c.id.clone(), text: "The 38 screams.".into() };
        relay.apply_answers("ada-key", &[altered], |_| Some(&c));
        assert_eq!(relay.take_reading(&id), Some(Reading::Changed));
    }

    #[test]
    fn another_authors_poll_cannot_answer_a_request() {
        let relay = Relay::default();
        let c = card("The 38 sighs.");
        relay.open_poll("ada-key", 1, 100).unwrap();
        let id = relay.record_request("ada-key", &c.id, "r1".into()).unwrap();
        relay.hand_over("ada-key");
        // Bea's node has read the story before and knows the exact words.
        let forged = Answer { request: id.clone(), story: c.id.clone(), text: "The 38 sighs.".into() };
        relay.apply_answers("bea-key", &[forged], |_| Some(&c));
        assert_eq!(relay.take_reading(&id), None, "still waiting for Ada");
    }

    #[test]
    fn a_replayed_poll_is_refused() {
        let relay = Relay::default();
        relay.open_poll("ada-key", 5, 100).unwrap();
        assert_eq!(relay.open_poll("ada-key", 5, 101), Err(PollRefused::Replayed));
        assert_eq!(relay.open_poll("ada-key", 4, 101), Err(PollRefused::Replayed));
        assert!(relay.open_poll("ada-key", 6, 101).is_ok());
        assert!(relay.open_poll("bea-key", 1, 101).is_ok(), "each author counts on their own");
    }

    #[test]
    fn too_many_waiting_readers_are_asked_to_try_again() {
        let relay = Relay::default();
        for i in 0..MAX_WAITING_PER_AUTHOR {
            relay.record_request("ada-key", "0123456789abcdef", format!("r{i}")).unwrap();
        }
        assert_eq!(relay.record_request("ada-key", "0123456789abcdef", "one-more".into()), Err(Reading::Busy));
        assert!(relay.record_request("bea-key", "0123456789abcdef", "bea".into()).is_ok());
    }

    #[test]
    fn a_withdrawn_story_leaves_its_readers_with_nothing() {
        let relay = Relay::default();
        let c = card("The 38 sighs.");
        relay.open_poll("ada-key", 1, 100).unwrap();
        let id = relay.record_request("ada-key", &c.id, "r1".into()).unwrap();
        relay.hand_over("ada-key");
        relay.remove_story("ada-key", &c.id);
        let late = Answer { request: id.clone(), story: c.id.clone(), text: "The 38 sighs.".into() };
        relay.apply_answers("ada-key", &[late], |_| Some(&c));
        assert_eq!(relay.take_reading(&id), Some(Reading::Gone));
    }

    #[test]
    fn giving_up_drops_the_request_and_marks_the_author_away() {
        let relay = Relay::default();
        relay.open_poll("ada-key", 1, 100).unwrap();
        let id = relay.record_request("ada-key", "0123456789abcdef", "r1".into()).unwrap();
        relay.record_gave_up(&id);
        assert!(!relay.is_here("ada-key", 101, T));
        assert_eq!(relay.take_reading(&id), Some(Reading::Gone));
    }
}
