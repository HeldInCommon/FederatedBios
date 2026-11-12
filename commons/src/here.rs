//! "Only while I'm here" (decision record 0021): the signed messages, and
//! the rules a commons applies to them. Everything here is pure.
//!
//! A story can travel to a commons in two ways (`crate::story::Travel`). As a
//! **copy**, the commons keeps the signed story (decision record 0020). As a
//! **card**, the commons keeps only a signed [`Card`]: the title, author,
//! first line, terms, and the SHA-256 of the full text, but not the text.
//! When someone opens the story, the commons asks the author's node for the
//! text over a connection *the node opened* (a long poll, see
//! `crate::relay` and `crate::relay_node`), checks the text against the
//! signed hash, passes it to the reader and keeps nothing.
//!
//! Two new `hic/v1` types (docs/spec §5.5), neither changing any existing
//! bytes:
//!
//! - `StoryCard`: the card, addressed to one commons. A node posts it to
//!   `/here/card` (not `/inbox`), so a commons that predates it answers 404
//!   and the node can say plainly that this commons doesn't support it yet.
//! - `StoryRelay`: what the node sends each time it asks "anything for me?"
//!   (`/here/poll`), carrying its answers to the last requests. Signed, so
//!   another node can't answer for an author; numbered (`seq`), so an old
//!   one can't be replayed.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::envelope::{self, Envelope, Signed};
use crate::model::{Reach, RemoteEntry};
use crate::story::{check_terms, is_story_id, HeldStory, Story, StoryTombstone, TEXT_MAX};
use crate::web::layout::host_label;

/// The `hic/v1` type of a story's card.
pub const STORY_CARD: &str = "StoryCard";
/// The `hic/v1` type of a node's poll to the commons, with its answers.
pub const STORY_RELAY: &str = "StoryRelay";

/// The longest first line a card may carry (see `Story::first_line`).
pub const FIRST_LINE_MAX: usize = 140;
/// The most answers one poll may carry. A commons hands a node at most this
/// many requests at once, so an honest node never needs more.
pub const ANSWERS_MAX: usize = 32;
/// How far a poll's signed time may be from the commons' clock, in seconds.
/// Polls are made and sent at once; this only allows for clocks that differ.
pub const RELAY_CLOCK_SKEW: u64 = 300;

/// What a card says, exactly as signed (the envelope's `body`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// The story's id (the same as its `Story` id).
    pub id: String,
    /// The handle the author holds on this commons.
    pub handle: String,
    /// The story's title.
    pub title: String,
    /// Its first line, as `Story::first_line` makes it: the card's preview.
    pub first_line: String,
    /// How many words the full story has (for "48 words").
    pub words: u64,
    /// `public` or `commons` (members-only stories never leave the node).
    pub visibility: String,
    /// The licence key.
    pub license: String,
    /// CC signal keys, sorted.
    pub signals: Vec<String>,
    /// Lowercase hex SHA-256 of the story's `text`, as UTF-8: what every
    /// text passed to a reader is checked against.
    pub text_sha256: String,
    /// The commons this card is for (its host), so a card sent to one
    /// commons can't be replayed on another.
    pub commons: String,
}

impl Card {
    /// Who may see it.
    pub fn reach(&self) -> Reach {
        Reach::parse(&self.visibility).unwrap_or(Reach::Public)
    }
}

/// Lowercase hex SHA-256 of `text` as UTF-8.
pub fn text_sha256(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// True when `text` is exactly the text the card's author signed the hash
/// of. This is the check that stops a commons (or anyone between the node
/// and the reader) passing off altered words as the author's.
pub fn text_matches(card: &Card, text: &str) -> bool {
    text_sha256(text) == card.text_sha256
}

/// The card for `story`, addressed to the commons at `commons_url`.
pub fn card_for(story: &Story, commons_url: &str) -> Card {
    Card {
        id: story.id.clone(),
        handle: story.handle.clone(),
        title: story.title.clone(),
        first_line: story.first_line(),
        words: story.word_count() as u64,
        visibility: story.visibility.clone(),
        license: story.license.clone(),
        signals: story.signals.clone(),
        text_sha256: text_sha256(&story.text),
        commons: host_label(commons_url),
    }
}

/// Sign a card with its author's key, dated `created_at`.
pub fn sign_card(card: &Card, author_pub: &str, author_priv: &str, created_at: u64) -> Result<Signed, String> {
    let body = serde_json::to_value(card).map_err(|e| e.to_string())?;
    envelope::sign(Envelope::new(STORY_CARD, author_pub, created_at, None, body), author_priv)
}

/// The message a node posts to a commons' `/here/card`.
pub fn card_message(signed: &Signed) -> Value {
    json!({ "type": "StoryCardShare", "schema": envelope::SCHEMA, "card": signed })
}

/// True for lowercase hex of exactly `len` characters.
fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// Check a received card: a valid `StoryCard`, signed by its author,
/// addressed to this commons (one of `my_hosts`), with terms a commons may
/// list.
pub fn verify_card(msg: &Value, my_hosts: &[String]) -> Result<(Card, Signed), String> {
    let signed: Signed =
        serde_json::from_value(msg.get("card").cloned().ok_or("missing card")?).map_err(|_| "bad card")?;
    envelope::verify(&signed)?;
    if signed.envelope.kind != STORY_CARD {
        return Err(format!("expected a StoryCard, got {:?}", signed.envelope.kind));
    }
    let card: Card = serde_json::from_value(signed.envelope.body.clone()).map_err(|_| "bad card body")?;
    if !my_hosts.contains(&card.commons) {
        return Err("this card was addressed to a different commons".into());
    }
    check_terms(&card.id, &card.handle, &card.title, &card.visibility, &card.license, &card.signals)?;
    if card.first_line.chars().count() > FIRST_LINE_MAX {
        return Err("the first line is too long".into());
    }
    if card.words == 0 || card.words > TEXT_MAX as u64 {
        return Err("the word count is out of range".into());
    }
    if !is_hex(&card.text_sha256, 64) {
        return Err("text_sha256 must be 64 lowercase hex characters".into());
    }
    Ok((card, signed))
}

/// A card a commons holds, with its signature as proof.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeldCard {
    /// What the card says, as signed.
    pub card: Card,
    /// The signed object as received.
    pub signed: Signed,
    /// When this commons received it (seconds since 1970).
    pub received_at: u64,
}

impl HeldCard {
    /// The key that signed it: the only key whose node may answer for it.
    pub fn author(&self) -> &str {
        &self.signed.envelope.author
    }

    /// When its author signed it.
    pub fn created_at(&self) -> u64 {
        self.signed.envelope.created_at
    }
}

/// Whether a commons takes a card.
#[derive(Debug, PartialEq, Eq)]
pub enum CardAdmission {
    /// Keep it; any copy of the same story held here is replaced by it.
    Accept,
    /// The author isn't listed here under the card's handle.
    NotListed,
    /// Something newer is held for this story (a card or a copy), or it was
    /// withdrawn later.
    Stale,
}

/// The rule for taking a verified card: the same gate as a story (the
/// author is listed under its handle), and it must be newer than any card,
/// copy or withdrawal recorded for that (author, id). Re-sending the very
/// same card is harmless.
pub fn admit_card(
    card: &Card,
    signed: &Signed,
    listed: &[RemoteEntry],
    held: &[HeldStory],
    cards: &[HeldCard],
    withdrawn: &[StoryTombstone],
) -> CardAdmission {
    let author = signed.envelope.author.as_str();
    let t = signed.envelope.created_at;
    if !listed.iter().any(|r| r.ed_pub == author && r.handle == card.handle) {
        return CardAdmission::NotListed;
    }
    if withdrawn.iter().any(|w| w.author == author && w.id == card.id && w.withdrawn_at >= t) {
        return CardAdmission::Stale;
    }
    if held.iter().any(|h| h.author() == author && h.story.id == card.id && h.created_at() >= t) {
        return CardAdmission::Stale;
    }
    match cards.iter().find(|c| c.author() == author && c.card.id == card.id) {
        Some(c) if c.signed.cid == signed.cid => CardAdmission::Accept,
        Some(c) if c.created_at() >= t => CardAdmission::Stale,
        _ => CardAdmission::Accept,
    }
}

/// True when a card held here for this (author, id) is as new as, or newer
/// than, a copy dated `created_at`: the copy is then stale. (A newer copy
/// replaces the card, the way a newer card replaces a copy.)
pub fn card_blocks_copy(cards: &[HeldCard], author: &str, id: &str, created_at: u64) -> bool {
    cards.iter().any(|c| c.author() == author && c.card.id == id && c.created_at() >= created_at)
}

/// What a commons records when a card is withdrawn, or why not: the same
/// rule as a copy (an older withdrawal can't remove a later card).
pub fn apply_card_withdraw(w: &crate::story::StoryWithdrawal, card: &HeldCard) -> Result<StoryTombstone, &'static str> {
    if card.created_at() > w.created_at {
        return Err("a newer card was shared after this withdrawal");
    }
    Ok(StoryTombstone { author: w.author.clone(), id: w.id.clone(), withdrawn_at: w.created_at })
}

/// One answer from a node: the text of `story` for the reader's `request`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    /// The request id the commons gave (32 lowercase hex characters).
    pub request: String,
    /// The story asked for.
    pub story: String,
    /// The story's full text, as the node holds it now.
    pub text: String,
}

/// A request a commons hands a node: someone wants to read `story`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Requested {
    /// The id to answer with (32 lowercase hex characters).
    pub request: String,
    /// The story they want.
    pub story: String,
}

/// What a commons answers a poll with: the requests waiting for this node.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct PollReply {
    /// Requests to answer in the next poll; empty when the poll timed out.
    pub requests: Vec<Requested>,
}

/// Build the signed poll a node sends to `commons_url`'s `/here/poll`:
/// number `seq` (higher than any before it), carrying `answers`.
pub fn build_relay(
    author_pub: &str,
    author_priv: &str,
    commons_url: &str,
    seq: u64,
    answers: &[Answer],
    created_at: u64,
) -> Result<Value, String> {
    let body = json!({ "commons": host_label(commons_url), "seq": seq, "answers": answers });
    let signed = envelope::sign(Envelope::new(STORY_RELAY, author_pub, created_at, None, body), author_priv)?;
    Ok(json!({ "type": "StoryRelay", "schema": envelope::SCHEMA, "relay": signed }))
}

/// A verified poll from a node.
#[derive(Debug, PartialEq, Eq)]
pub struct Relay {
    /// The key that signed it: whose stories it may answer for.
    pub author: String,
    /// Its number; a commons refuses one not higher than the last it took.
    pub seq: u64,
    /// The node's answers to earlier requests.
    pub answers: Vec<Answer>,
}

/// Check a received poll: a `StoryRelay`, signed, addressed to this commons,
/// dated near `now`, with well-formed answers. Whether `seq` is new, and
/// whether each answer matches a request and a card, is the relay's to
/// decide (`crate::relay`), since that needs what's in memory.
pub fn verify_relay(msg: &Value, my_hosts: &[String], now: u64) -> Result<Relay, String> {
    let signed: Signed =
        serde_json::from_value(msg.get("relay").cloned().ok_or("missing relay")?).map_err(|_| "bad relay")?;
    envelope::verify(&signed)?;
    let env = &signed.envelope;
    if env.kind != STORY_RELAY {
        return Err(format!("expected a StoryRelay, got {:?}", env.kind));
    }
    if env.created_at.abs_diff(now) > RELAY_CLOCK_SKEW {
        return Err("this poll is too old, or its clock is far from this commons' clock".into());
    }
    let commons = env.body.get("commons").and_then(|v| v.as_str()).unwrap_or("");
    if !my_hosts.iter().any(|h| h == commons) {
        return Err("this poll was addressed to a different commons".into());
    }
    let seq = env.body.get("seq").and_then(|v| v.as_u64()).ok_or("missing seq")?;
    let answers: Vec<Answer> =
        serde_json::from_value(env.body.get("answers").cloned().unwrap_or(json!([]))).map_err(|_| "bad answers")?;
    if answers.len() > ANSWERS_MAX {
        return Err("too many answers in one poll".into());
    }
    for a in &answers {
        if !is_hex(&a.request, 32) || !is_story_id(&a.story) || a.text.chars().count() > TEXT_MAX {
            return Err("a malformed answer".into());
        }
    }
    Ok(Relay { author: env.author.clone(), seq, answers })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;
    use crate::story::{new_story, sign, StoryWithdrawal};

    /// A well-formed story id for these tests.
    const ID: &str = "0123456789abcdef";
    /// The commons these tests address.
    const ROOM: &str = "http://room.local:8081";

    /// This commons' hosts, as `Config::own_hosts` would give them.
    fn hosts() -> Vec<String> {
        vec!["room.local:8081".to_string()]
    }

    /// A small story by @ada.
    fn story() -> Story {
        new_story(ID, "ada", "Night bus", "The 38 sighs at the stop.\nA dog answers.", "commons", "cc-by", &[]).unwrap()
    }

    /// A commons' record of `handle_` listed under `key`.
    fn listed(handle_: &str, key: &str) -> RemoteEntry {
        RemoteEntry {
            ed_pub: key.into(),
            box_pub: String::new(),
            handle: handle_.into(),
            display_name: handle_.into(),
            origin: String::new(),
            origin_label: String::new(),
            cid: String::new(),
            visibility: "public".into(),
            license: "cc-by".into(),
            signals: vec![],
            held_signals: Default::default(),
            created_at: 0,
            profile: crate::model::Profile::default(),
            signature: String::new(),
            received_at: 0,
            signed: None,
        }
    }

    /// @ada's card for `story()`, signed at `at`, as a commons would hold it.
    fn held_card(k: &keys::NewKeys, at: u64) -> HeldCard {
        let card = card_for(&story(), ROOM);
        HeldCard { signed: sign_card(&card, &k.ed_pub, &k.ed_priv, at).unwrap(), card, received_at: at }
    }

    #[test]
    fn a_card_carries_the_terms_and_the_hash_but_not_the_text() {
        let card = card_for(&story(), ROOM);
        assert_eq!(card.first_line, "The 38 sighs at the stop.");
        assert_eq!(card.words, 9);
        assert_eq!(card.commons, "room.local:8081");
        assert_eq!(card.text_sha256, text_sha256(&story().text));
        let body = serde_json::to_string(&card).unwrap();
        assert!(!body.contains("A dog answers"), "the rest of the text never goes in the card");
    }

    #[test]
    fn the_hash_is_sha256_of_the_utf8_text() {
        // SHA-256("abc"), FIPS 180-2 appendix B.1.
        assert_eq!(text_sha256("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn a_signed_card_verifies_only_where_it_was_addressed() {
        let k = keys::generate();
        let signed = sign_card(&card_for(&story(), ROOM), &k.ed_pub, &k.ed_priv, 100).unwrap();
        let msg = card_message(&signed);
        let (card, got) = verify_card(&msg, &hosts()).unwrap();
        assert_eq!(card.id, ID);
        assert_eq!(got.envelope.author, k.ed_pub);
        assert!(verify_card(&msg, &["elsewhere:8081".to_string()]).unwrap_err().contains("different commons"));
    }

    #[test]
    fn a_tampered_card_is_refused() {
        let k = keys::generate();
        let signed = sign_card(&card_for(&story(), ROOM), &k.ed_pub, &k.ed_priv, 100).unwrap();
        let mut msg = card_message(&signed);
        msg["card"]["envelope"]["body"]["text_sha256"] = json!("0".repeat(64));
        assert!(verify_card(&msg, &hosts()).is_err(), "the hash is signed");
        // A story's signature can't be passed off as a card's.
        let as_story = json!({ "card": sign(&story(), &k.ed_pub, &k.ed_priv, 100).unwrap() });
        assert!(verify_card(&as_story, &hosts()).is_err());
    }

    #[test]
    fn a_members_only_card_is_refused() {
        let k = keys::generate();
        let mut card = card_for(&story(), ROOM);
        card.visibility = "members".into();
        let msg = card_message(&sign_card(&card, &k.ed_pub, &k.ed_priv, 100).unwrap());
        assert!(verify_card(&msg, &hosts()).unwrap_err().contains("members-only"));
    }

    #[test]
    fn altered_text_does_not_match_the_signed_hash() {
        let card = card_for(&story(), ROOM);
        assert!(text_matches(&card, &story().text));
        assert!(!text_matches(&card, "The 38 sighs at the stop.\nA cat answers."), "one word changed");
        assert!(!text_matches(&card, &format!("{} ", story().text)), "one space added");
    }

    #[test]
    fn a_card_needs_its_author_listed_and_must_be_newer() {
        let k = keys::generate();
        let me = [listed("ada", &k.ed_pub)];
        let c = held_card(&k, 200);
        assert_eq!(admit_card(&c.card, &c.signed, &[], &[], &[], &[]), CardAdmission::NotListed);
        assert_eq!(
            admit_card(&c.card, &c.signed, &[listed("ada", "someone-else")], &[], &[], &[]),
            CardAdmission::NotListed,
            "another key can't use a listed handle"
        );
        assert_eq!(admit_card(&c.card, &c.signed, &me, &[], &[], &[]), CardAdmission::Accept);
        assert_eq!(
            admit_card(&c.card, &c.signed, &me, &[], std::slice::from_ref(&c), &[]),
            CardAdmission::Accept,
            "resent"
        );
        let older = held_card(&k, 100);
        assert_eq!(
            admit_card(&older.card, &older.signed, &me, &[], std::slice::from_ref(&c), &[]),
            CardAdmission::Stale
        );
        let tomb = StoryTombstone { author: k.ed_pub.clone(), id: ID.into(), withdrawn_at: 200 };
        assert_eq!(admit_card(&c.card, &c.signed, &me, &[], &[], &[tomb]), CardAdmission::Stale, "replay");
    }

    #[test]
    fn a_card_and_a_copy_replace_each_other_only_when_newer() {
        let k = keys::generate();
        let me = [listed("ada", &k.ed_pub)];
        let copy =
            HeldStory { story: story(), signed: sign(&story(), &k.ed_pub, &k.ed_priv, 150).unwrap(), received_at: 0 };
        let newer = held_card(&k, 200);
        assert_eq!(
            admit_card(&newer.card, &newer.signed, &me, std::slice::from_ref(&copy), &[], &[]),
            CardAdmission::Accept
        );
        let older = held_card(&k, 150);
        assert_eq!(admit_card(&older.card, &older.signed, &me, &[copy], &[], &[]), CardAdmission::Stale);
        assert!(card_blocks_copy(std::slice::from_ref(&newer), &k.ed_pub, ID, 200));
        assert!(!card_blocks_copy(&[newer], &k.ed_pub, ID, 201));
    }

    #[test]
    fn withdrawing_a_card_needs_a_withdrawal_at_least_as_new() {
        let k = keys::generate();
        let c = held_card(&k, 200);
        let at = |t| StoryWithdrawal { author: k.ed_pub.clone(), id: ID.into(), created_at: t };
        assert!(apply_card_withdraw(&at(199), &c).is_err());
        assert_eq!(apply_card_withdraw(&at(200), &c).unwrap().withdrawn_at, 200);
    }

    #[test]
    fn a_poll_is_signed_addressed_and_recent() {
        let k = keys::generate();
        let answers = [Answer { request: "a".repeat(32), story: ID.into(), text: "words".into() }];
        let msg = build_relay(&k.ed_pub, &k.ed_priv, ROOM, 7, &answers, 1_000).unwrap();
        let r = verify_relay(&msg, &hosts(), 1_010).unwrap();
        assert_eq!((r.author.as_str(), r.seq, r.answers.len()), (k.ed_pub.as_str(), 7, 1));
        assert!(verify_relay(&msg, &["elsewhere:1".to_string()], 1_010).is_err(), "addressed elsewhere");
        assert!(verify_relay(&msg, &hosts(), 1_000 + RELAY_CLOCK_SKEW + 1).is_err(), "too old");
    }

    #[test]
    fn another_node_cannot_answer_for_an_author() {
        // Bea signs a poll and puts Ada's key as the author: the signature
        // doesn't verify, so the commons never sees it as Ada's.
        let (ada, bea) = (keys::generate(), keys::generate());
        let answers = [Answer { request: "a".repeat(32), story: ID.into(), text: "forged".into() }];
        let mut msg = build_relay(&bea.ed_pub, &bea.ed_priv, ROOM, 1, &answers, 1_000).unwrap();
        msg["relay"]["envelope"]["author"] = json!(ada.ed_pub);
        assert!(verify_relay(&msg, &hosts(), 1_000).is_err());
        // Nor can someone change the text in a signed answer on the way.
        let mut honest = build_relay(&ada.ed_pub, &ada.ed_priv, ROOM, 1, &answers, 1_000).unwrap();
        honest["relay"]["envelope"]["body"]["answers"][0]["text"] = json!("altered");
        assert!(verify_relay(&honest, &hosts(), 1_000).is_err());
    }

    #[test]
    fn malformed_answers_are_refused() {
        let k = keys::generate();
        let bad_id = [Answer { request: "not-hex".into(), story: ID.into(), text: "x".into() }];
        let msg = build_relay(&k.ed_pub, &k.ed_priv, ROOM, 1, &bad_id, 1_000).unwrap();
        assert!(verify_relay(&msg, &hosts(), 1_000).is_err());
        let many: Vec<Answer> =
            (0..=ANSWERS_MAX).map(|_| Answer { request: "b".repeat(32), story: ID.into(), text: "x".into() }).collect();
        let msg = build_relay(&k.ed_pub, &k.ed_priv, ROOM, 1, &many, 1_000).unwrap();
        assert!(verify_relay(&msg, &hosts(), 1_000).is_err());
    }
}
