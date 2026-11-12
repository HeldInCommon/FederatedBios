//! Stories: short pieces of writing a member makes on their node (in a
//! workshop: "the sound of your street") and may share to a commons.
//!
//! A story is a `hic/v1` object of type `Story` (docs/spec §5.4), signed by
//! the member's key. Its `id` stays the same across versions, so a commons
//! keeps one story per (author, id): the latest. A `StoryWithdraw`, also
//! signed and addressed to one commons, removes it there and leaves a record
//! so the same copy can't be replayed back in.
//!
//! A story with Held in Common Signals (decision record 0025) is signed as
//! type `StoryV2`, whose body adds `held_signals`. A commons that predates
//! it refuses that type; the node then sends the same story as a `Story`,
//! without them ([`older_copy`]). A story without them is a `Story`, exactly
//! as before.
//!
//! A commons accepts a story only from someone it already lists under that
//! handle. Joining stays the gate (decision record 0011); stories ride on it.
//!
//! Everything here is pure; the request handlers do the I/O.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::envelope::{self, Envelope, Signed};
use crate::governance::{license_from_form, signals_from_form, LICENSES, SIGNALS};
use crate::handle;
use crate::held_signals::{self, HeldSignals};
use crate::model::{Reach, RemoteEntry};
use crate::web::layout::host_label;

/// The `hic/v1` type of a story.
pub const STORY: &str = "Story";
/// The `hic/v1` type of a story that carries Held in Common Signals
/// (decision record 0025): a `Story` body plus `held_signals`.
pub const STORY_V2: &str = "StoryV2";
/// The `hic/v1` type of a story's withdrawal from one commons.
pub const STORY_WITHDRAW: &str = "StoryWithdraw";

/// Longest title, in characters.
pub const TITLE_MAX: usize = 120;
/// Longest story, in characters. Ten minutes of writing is usually 2,000–3,000.
pub const TEXT_MAX: usize = 12_000;

/// The workshop prompt shown above the writing form.
pub const PROMPT: &str = "Listen to your street for a minute. Write a small story from what you hear.";

/// What a story says, exactly as signed (the envelope's `body`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Story {
    /// Stable across versions: 16 lowercase hex characters.
    pub id: String,
    /// The handle the author holds (a commons checks they're listed under it).
    pub handle: String,
    /// Its title, 1–120 characters.
    pub title: String,
    /// The story itself, 1–12,000 characters; blank lines separate paragraphs.
    pub text: String,
    /// `public`, `commons` or `members` (members-only stories never leave the node).
    pub visibility: String,
    /// The licence key (see `governance::LICENSES`).
    pub license: String,
    /// CC signal keys (see `governance::SIGNALS`), sorted, no repeats.
    pub signals: Vec<String>,
    /// Held in Common Signals (decision record 0025). Left out of the body
    /// when nothing is chosen, so such a story signs exactly as before.
    #[serde(default, skip_serializing_if = "HeldSignals::is_empty")]
    pub held_signals: HeldSignals,
}

impl Story {
    /// Who may see it.
    pub fn reach(&self) -> Reach {
        Reach::parse(&self.visibility).unwrap_or(Reach::Public)
    }

    /// The first line with any words in it, cut to about 140 characters: a
    /// card's preview. A Markdown-style link shows only its words (see
    /// `crate::text_links`), so a cut never leaves half of `[…](…)` behind.
    pub fn first_line(&self) -> String {
        let line = self.text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
        let line = crate::text_links::without_link_markup(line);
        if line.chars().count() <= 140 {
            line
        } else {
            format!("{}\u{2026}", line.chars().take(139).collect::<String>().trim_end())
        }
    }

    /// How many words it has (for "48 words").
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    /// The same story with a new title and text: its id, author and terms
    /// stay. Checked like a new one.
    pub fn revised(&self, title: &str, text: &str) -> Result<Story, &'static str> {
        let revised = new_story(&self.id, &self.handle, title, text, &self.visibility, &self.license, &self.signals)?;
        Ok(revised.with_held_signals(self.held_signals.clone()))
    }

    /// The same story with new terms: who can see it, licence, signals and
    /// Held in Common Signals.
    pub fn with_terms(&self, visibility: &str, license: &str, signals: &[String], held: HeldSignals) -> Story {
        let mut s = self.clone();
        s.visibility = Reach::parse(visibility).unwrap_or(Reach::Commons).as_str().to_string();
        s.license = license_from_form(license);
        s.signals = signals_from_form(signals);
        s.held_signals = held;
        s
    }

    /// The same story with these Held in Common Signals.
    pub fn with_held_signals(mut self, held: HeldSignals) -> Story {
        self.held_signals = held;
        self
    }

    /// The `hic/v1` type this story is signed as: `StoryV2` when it carries
    /// Held in Common Signals, otherwise `Story`.
    pub fn kind(&self) -> &'static str {
        if self.held_signals.is_empty() {
            STORY
        } else {
            STORY_V2
        }
    }

    /// The story as a plain text file (Export).
    pub fn as_text_file(&self) -> String {
        format!(
            "{}\n\n{}\n\n— @{} · {}\n",
            self.title,
            self.text.trim_end(),
            self.handle,
            crate::governance::license_name(&self.license)
        )
    }
}

/// How a story travels to a commons (the "How it travels" choice).
///
/// This is the author's choice on their own node, kept beside the story and
/// not inside its signed body, so choosing it doesn't change the signed
/// `Story` format. What a commons receives differs instead: a signed copy,
/// or a signed card (decision record 0021, `crate::here`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Travel {
    /// The commons keeps a signed copy until it's withdrawn (decision record 0020).
    #[default]
    Copy,
    /// "Only while I'm here": the commons keeps a signed card, and asks this
    /// node for the text each time someone reads it (decision record 0021).
    Here,
}

impl Travel {
    /// The choice from a form value: `here`, or anything else for a copy.
    pub fn from_form(value: &str) -> Travel {
        match value {
            "here" => Travel::Here,
            _ => Travel::Copy,
        }
    }

    /// The form value for this choice.
    pub fn as_str(self) -> &'static str {
        match self {
            Travel::Copy => "copy",
            Travel::Here => "here",
        }
    }
}

/// A story on the node that wrote it: what it says, its signed form, and the
/// commons it has been shared to.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OwnStory {
    /// What the story says, as signed.
    pub story: Story,
    /// The signed object: what gets sent to a commons, and proof of authorship.
    pub signed: Signed,
    /// The commons it is shared to now (their addresses).
    #[serde(default)]
    pub shared_to: Vec<String>,
    /// When it was last shared to each commons in `shared_to`. For a card
    /// ("Only while I'm here") this is the card's signed time.
    #[serde(default)]
    pub shared_at: BTreeMap<String, u64>,
    /// How the author wants it to travel next time it's shared. Stories
    /// written before this choice existed travel as copies.
    #[serde(default)]
    pub travel: Travel,
    /// How it was shared to each commons in `shared_to`. Missing means a
    /// copy (everything shared before "Only while I'm here" existed).
    #[serde(default)]
    pub shared_as: BTreeMap<String, Travel>,
}

impl OwnStory {
    /// A story just written: signed, shared nowhere, and set to travel as a
    /// copy until its author chooses otherwise.
    pub fn new(story: Story, signed: Signed) -> OwnStory {
        OwnStory {
            story,
            signed,
            shared_to: vec![],
            shared_at: BTreeMap::new(),
            travel: Travel::Copy,
            shared_as: BTreeMap::new(),
        }
    }

    /// When it was written (the signed time).
    pub fn created_at(&self) -> u64 {
        self.signed.envelope.created_at
    }

    /// The time to date this story's next version: `now`, but always later
    /// than the current version. A commons keeps only a newer version, and
    /// refuses one no later than a withdrawal, so an edit made in the same
    /// second as the last version or a withdrawal must still come after it.
    pub fn next_version_at(&self, now: u64) -> u64 {
        now.max(self.created_at() + 1)
    }

    /// The same story, recorded as shared to `commons` at time `at`, as a copy.
    pub fn with_shared_to(self, commons: &str, at: u64) -> OwnStory {
        self.with_shared_as(commons, at, Travel::Copy)
    }

    /// The same story, recorded as shared to `commons` at time `at`, the way
    /// `how` says (a copy, or a card for "Only while I'm here").
    pub fn with_shared_as(mut self, commons: &str, at: u64, how: Travel) -> OwnStory {
        if !self.shared_to.iter().any(|c| c == commons) {
            self.shared_to.push(commons.to_string());
        }
        self.shared_at.insert(commons.to_string(), at);
        self.shared_as.insert(commons.to_string(), how);
        self
    }

    /// The same story, no longer recorded as shared to `commons`.
    pub fn without_shared_to(mut self, commons: &str) -> OwnStory {
        self.shared_to.retain(|c| c != commons);
        self.shared_at.remove(commons);
        self.shared_as.remove(commons);
        self
    }

    /// The same story, to travel as `travel` the next time it's shared.
    /// Where it's shared already doesn't change until it's re-shared.
    pub fn with_travel(mut self, travel: Travel) -> OwnStory {
        self.travel = travel;
        self
    }

    /// How it's shared to `commons` now, if it is: a copy or a card.
    pub fn shared_as(&self, commons: &str) -> Option<Travel> {
        if !self.shared_to.iter().any(|c| c == commons) {
            return None;
        }
        Some(self.shared_as.get(commons).copied().unwrap_or_default())
    }

    /// The time to date whatever is sent to `commons` next (a new copy or a
    /// card): after this story's current version (see
    /// [`OwnStory::next_version_at`]) and, if a card is there now, after that
    /// card. A commons keeps only what is newer than what it holds for this
    /// story, so switching between a copy and a card must always move forward.
    pub fn next_share_at(&self, commons: &str, now: u64) -> u64 {
        let after_card = match self.shared_as(commons) {
            Some(Travel::Here) => self.shared_at.get(commons).map_or(0, |at| at + 1),
            _ => 0,
        };
        self.next_version_at(now).max(after_card)
    }

    /// The time to date a withdrawal from `commons`: `now`, but never before
    /// what was sent there (a copy, at most this version; or a card). A
    /// commons refuses a withdrawal older than what it holds, and a version
    /// or card can be dated a second ahead of the clock (see
    /// [`OwnStory::next_share_at`]).
    pub fn withdraw_at(&self, commons: &str, now: u64) -> u64 {
        let sent = self.shared_at.get(commons).copied().unwrap_or(0);
        now.max(sent).max(self.created_at())
    }

    /// The same story, a new version: `story` signed as `signed`. Where it's
    /// shared doesn't change (the copies there are now older; see
    /// [`OwnStory::changed_since_shared`]).
    pub fn with_version(mut self, story: Story, signed: Signed) -> OwnStory {
        self.story = story;
        self.signed = signed;
        self
    }

    /// True when this version was made after the copy on `commons` was
    /// shared, so that copy is out of date until it's shared again.
    pub fn changed_since_shared(&self, commons: &str) -> bool {
        self.shared_at.get(commons).is_some_and(|at| self.created_at() > *at)
    }
}

/// A story shared to a commons, kept with its signature as proof.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeldStory {
    /// What the story says, as signed.
    pub story: Story,
    /// The signed object as received, kept as proof.
    pub signed: Signed,
    /// When this commons received it (seconds since 1970).
    pub received_at: u64,
}

impl HeldStory {
    /// The key that signed it.
    pub fn author(&self) -> &str {
        &self.signed.envelope.author
    }

    /// When its author wrote this version.
    pub fn created_at(&self) -> u64 {
        self.signed.envelope.created_at
    }
}

/// The stories in `held` signed by `author` (a public key), newest version
/// first: what a member has shared to this commons, for their profile page.
pub fn by_author<'a>(held: &'a [HeldStory], author: &str) -> Vec<&'a HeldStory> {
    let mut theirs: Vec<&HeldStory> = held.iter().filter(|h| h.author() == author).collect();
    theirs.sort_by_key(|h| std::cmp::Reverse(h.created_at()));
    theirs
}

/// What remains on a commons after a story is withdrawn.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StoryTombstone {
    /// The key that signed the withdrawn story.
    pub author: String,
    /// The story's id.
    pub id: String,
    /// When it was withdrawn: copies dated then or earlier are refused.
    pub withdrawn_at: u64,
}

/// How long a commons shows that a story was withdrawn: a day.
pub const WITHDRAWN_SHOWN_FOR: u64 = 86_400;

/// A recent withdrawal, as a commons shows it: who, and when. Nothing of the
/// story itself is kept (decision record 0020).
#[derive(Debug, PartialEq, Eq)]
pub struct RecentWithdrawal {
    /// Who withdrew it (their handle on this commons).
    pub handle: String,
    /// When (seconds since 1970).
    pub at: u64,
}

/// The withdrawals to show on a commons' stories page, newest first: within
/// [`WITHDRAWN_SHOWN_FOR`] of `now`, by someone still listed here (for their
/// handle), and not since shared again (as a copy in `held`, or as a card in
/// `cards`).
pub fn recent_withdrawals(
    withdrawn: &[StoryTombstone],
    held: &[HeldStory],
    cards: &[crate::here::HeldCard],
    listed: &[RemoteEntry],
    now: u64,
) -> Vec<RecentWithdrawal> {
    let mut out: Vec<RecentWithdrawal> = withdrawn
        .iter()
        .filter(|t| now.saturating_sub(t.withdrawn_at) <= WITHDRAWN_SHOWN_FOR)
        .filter(|t| !held.iter().any(|h| h.author() == t.author && h.story.id == t.id))
        .filter(|t| !cards.iter().any(|c| c.author() == t.author && c.card.id == t.id))
        .filter_map(|t| {
            let who = listed.iter().find(|r| r.ed_pub == t.author)?;
            Some(RecentWithdrawal { handle: who.handle.clone(), at: t.withdrawn_at })
        })
        .collect();
    out.sort_by_key(|w| std::cmp::Reverse(w.at));
    out
}

/// True for a story id: 16 lowercase hex characters (also safe in file names).
pub fn is_story_id(s: &str) -> bool {
    s.len() == 16 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// A story from the writing form, tidied and checked. `id` and `handle` come
/// from the server, the rest from the form.
pub fn new_story(
    id: &str,
    handle_: &str,
    title: &str,
    text: &str,
    visibility: &str,
    license: &str,
    signals: &[String],
) -> Result<Story, &'static str> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = text.replace("\r\n", "\n").trim().to_string();
    if title.is_empty() {
        return Err("Give your story a title.");
    }
    if title.chars().count() > TITLE_MAX {
        return Err("Keep the title to 120 characters or fewer.");
    }
    if text.is_empty() {
        return Err("Write something first: the story is empty.");
    }
    if text.chars().count() > TEXT_MAX {
        return Err("That's longer than a story can be here (12,000 characters).");
    }
    let visibility = Reach::parse(visibility).unwrap_or(Reach::Public).as_str().to_string();
    Ok(Story {
        id: id.into(),
        handle: handle_.into(),
        title,
        text,
        visibility,
        license: license_from_form(license),
        signals: signals_from_form(signals),
        held_signals: HeldSignals::default(),
    })
}

/// Sign a story with its author's key, dated `created_at`, as the type
/// [`Story::kind`] names.
pub fn sign(story: &Story, author_pub: &str, author_priv: &str, created_at: u64) -> Result<Signed, String> {
    let body = serde_json::to_value(story).map_err(|e| e.to_string())?;
    envelope::sign(Envelope::new(story.kind(), author_pub, created_at, None, body), author_priv)
}

/// For a commons that predates `StoryV2` (it answered 400): the same story,
/// dated the same, signed as a `Story` without its Held in Common Signals.
/// `None` when `signed` is already a `Story`, so there is nothing older to
/// try. Signing is deterministic, so this is the same copy every time.
pub fn older_copy(story: &Story, signed: &Signed, author_priv: &str) -> Result<Option<Signed>, String> {
    if signed.envelope.kind != STORY_V2 {
        return Ok(None);
    }
    let without = story.clone().with_held_signals(HeldSignals::default());
    sign(&without, &signed.envelope.author, author_priv, signed.envelope.created_at).map(Some)
}

/// The message a node posts to a commons' `/inbox` to share a story.
pub fn share_message(signed: &Signed) -> Value {
    json!({ "type": "StoryShare", "schema": envelope::SCHEMA, "story": signed })
}

/// Why a story can't be federated, if it can't: everything a commons checks
/// about the content itself.
fn check(story: &Story) -> Result<(), String> {
    check_terms(&story.id, &story.handle, &story.title, &story.visibility, &story.license, &story.signals)?;
    if story.text.trim().is_empty() || story.text.chars().count() > TEXT_MAX {
        return Err("the story is empty or too long".into());
    }
    Ok(())
}

/// The checks a commons makes on everything a story carries except its
/// text: the id, handle, title, reach, licence and signals. Shared by a
/// story and its "Only while I'm here" card (`crate::here`), which carries
/// the same terms without the text.
pub(crate) fn check_terms(
    id: &str,
    handle_: &str,
    title: &str,
    visibility: &str,
    license: &str,
    signals: &[String],
) -> Result<(), String> {
    if !handle::is_valid(handle_) {
        return Err("invalid handle".into());
    }
    if !is_story_id(id) {
        return Err("invalid story id".into());
    }
    if title.trim().is_empty() || title.chars().count() > TITLE_MAX {
        return Err("the title is empty or too long".into());
    }
    match Reach::parse(visibility) {
        Some(Reach::Members) => return Err("members-only stories are not federated".into()),
        None => return Err("unknown visibility".into()),
        Some(_) => {}
    }
    if !LICENSES.iter().any(|l| l.0 == license) {
        return Err("unknown licence".into());
    }
    if !signals.iter().all(|s| SIGNALS.iter().any(|k| k.0 == s)) {
        return Err("unknown signal".into());
    }
    Ok(())
}

/// Check a received story share: a valid `Story` or `StoryV2` signed by its
/// `author`, with content a commons may list. A `StoryV2` must carry
/// well-formed Held in Common Signals, and a `Story` none (see
/// [`held_signals::from_body`]).
pub fn verify_share(msg: &Value) -> Result<(Story, Signed), String> {
    let signed: Signed =
        serde_json::from_value(msg.get("story").cloned().ok_or("missing story")?).map_err(|_| "bad story")?;
    envelope::verify(&signed)?;
    let kind = signed.envelope.kind.as_str();
    if kind != STORY && kind != STORY_V2 {
        return Err(format!("expected a Story, got {kind:?}"));
    }
    let held = held_signals::from_body(&signed.envelope.body, kind == STORY_V2)?;
    let story: Story = serde_json::from_value(signed.envelope.body.clone()).map_err(|_| "bad story body")?;
    // Keep the strictly read answers, not serde's lenient reading of them.
    let story = story.with_held_signals(held);
    check(&story)?;
    Ok((story, signed))
}

/// Build the signed withdrawal of story `id` from the commons at `commons_url`.
pub fn build_withdraw(
    author_pub: &str,
    author_priv: &str,
    id: &str,
    commons_url: &str,
    created_at: u64,
) -> Result<Value, String> {
    let body = json!({ "story": id, "commons": host_label(commons_url) });
    let signed = envelope::sign(Envelope::new(STORY_WITHDRAW, author_pub, created_at, None, body), author_priv)?;
    Ok(json!({ "type": "StoryWithdraw", "schema": envelope::SCHEMA, "withdraw": signed }))
}

/// A verified request to withdraw a story.
#[derive(Debug, PartialEq, Eq)]
pub struct StoryWithdrawal {
    /// The key that signed the withdrawal.
    pub author: String,
    /// The story being withdrawn.
    pub id: String,
    /// When the withdrawal was signed.
    pub created_at: u64,
}

/// Check a received story withdrawal: signed, and addressed to this commons
/// (one of `my_hosts`, see `Config::own_hosts`), so one sent elsewhere can't
/// be replayed here.
pub fn verify_withdraw(msg: &Value, my_hosts: &[String]) -> Result<StoryWithdrawal, String> {
    let signed: Signed =
        serde_json::from_value(msg.get("withdraw").cloned().ok_or("missing withdraw")?).map_err(|_| "bad withdraw")?;
    envelope::verify(&signed)?;
    let env = &signed.envelope;
    if env.kind != STORY_WITHDRAW {
        return Err(format!("expected a StoryWithdraw, got {:?}", env.kind));
    }
    let field = |k: &str| env.body.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    if !my_hosts.iter().any(|h| *h == field("commons")) {
        return Err("this withdrawal was addressed to a different commons".into());
    }
    let id = field("story");
    if !is_story_id(&id) {
        return Err("invalid story id".into());
    }
    Ok(StoryWithdrawal { author: env.author.clone(), id, created_at: env.created_at })
}

/// Whether a commons takes a story.
#[derive(Debug, PartialEq, Eq)]
pub enum StoryAdmission {
    /// Keep it (or keep the same copy again).
    Accept,
    /// The author isn't listed here under the story's handle.
    NotListed,
    /// A newer version, or a later withdrawal, is already recorded.
    Stale,
}

/// The rule for taking a verified story: its author must be listed here
/// under its handle, and it must be newer than what is held or withdrawn for
/// that (author, id). Re-sending the very same story is harmless.
pub fn admit(
    story: &Story,
    signed: &Signed,
    listed: &[RemoteEntry],
    held: &[HeldStory],
    withdrawn: &[StoryTombstone],
) -> StoryAdmission {
    let author = signed.envelope.author.as_str();
    let t = signed.envelope.created_at;
    if !listed.iter().any(|r| r.ed_pub == author && r.handle == story.handle) {
        return StoryAdmission::NotListed;
    }
    if withdrawn.iter().any(|w| w.author == author && w.id == story.id && w.withdrawn_at >= t) {
        return StoryAdmission::Stale;
    }
    match held.iter().find(|h| h.author() == author && h.story.id == story.id) {
        Some(h) if h.signed.cid == signed.cid => StoryAdmission::Accept,
        Some(h) if h.created_at() >= t => StoryAdmission::Stale,
        _ => StoryAdmission::Accept,
    }
}

/// What a commons records for a verified withdrawal, or why not. The caller
/// then deletes the held story. Withdrawing twice is harmless; an older
/// withdrawal can't remove a later version.
pub fn apply_withdraw(
    w: &StoryWithdrawal,
    held: &[HeldStory],
    withdrawn: &[StoryTombstone],
) -> Result<StoryTombstone, &'static str> {
    let tomb = StoryTombstone { author: w.author.clone(), id: w.id.clone(), withdrawn_at: w.created_at };
    match held.iter().find(|h| h.author() == w.author && h.story.id == w.id) {
        Some(h) if h.created_at() > w.created_at => Err("a newer version was shared after this withdrawal"),
        Some(_) => Ok(tomb),
        None if withdrawn.iter().any(|t| t.author == w.author && t.id == w.id) => Ok(tomb),
        None => Err("that story isn't shared here"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;

    /// A well-formed story id (16 hex characters) for these tests.
    const ID: &str = "0123456789abcdef";

    /// A small public story by `handle_`, for these tests.
    fn story(handle_: &str) -> Story {
        new_story(
            ID,
            handle_,
            "Night bus",
            "The 38 sighs at the stop.\nA dog answers.",
            "public",
            "cc-by",
            &["credit".into()],
        )
        .unwrap()
    }

    /// A commons' record of `handle_` listed under `key`, with nothing else filled in.
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
            created_at: 0,
            profile: crate::model::Profile::default(),
            signature: String::new(),
            received_at: 0,
            signed: None,
            held_signals: HeldSignals::default(),
        }
    }

    #[test]
    fn the_form_is_tidied_and_checked() {
        let s = new_story(
            ID,
            "ada",
            "  Night   bus ",
            "\r\nline one\r\nline two\r\n",
            "commons",
            "bogus",
            &["open".into(), "x".into()],
        )
        .unwrap();
        assert_eq!(s.title, "Night bus");
        assert_eq!(s.text, "line one\nline two");
        assert_eq!(s.visibility, "commons");
        assert_eq!(s.license, crate::governance::DEFAULT_LICENSE);
        assert_eq!(s.signals, vec!["open".to_string()]);
        assert!(new_story(ID, "ada", " ", "text", "public", "cc-by", &[]).is_err());
        assert!(new_story(ID, "ada", "t", "   ", "public", "cc-by", &[]).is_err());
        assert!(new_story(ID, "ada", "t", &"x".repeat(TEXT_MAX + 1), "public", "cc-by", &[]).is_err());
        assert!(new_story(ID, "ada", &"t".repeat(TITLE_MAX + 1), "x", "public", "cc-by", &[]).is_err());
    }

    #[test]
    fn previews_and_exports() {
        let s = story("ada");
        assert_eq!(s.first_line(), "The 38 sighs at the stop.");
        let long = Story { text: "w".repeat(300), ..s.clone() };
        assert_eq!(long.first_line().chars().count(), 140);
        let linked = Story { text: "Hear [the bus](https://example.org/38) sigh.".into(), ..s.clone() };
        assert_eq!(linked.first_line(), "Hear the bus sigh.", "previews show a link's words");
        assert!(s.as_text_file().starts_with("Night bus\n\nThe 38 sighs"));
        assert!(s.as_text_file().contains("@ada · CC BY 4.0"));
    }

    #[test]
    fn a_signed_story_verifies_and_tampering_shows() {
        let k = keys::generate();
        let signed = sign(&story("ada"), &k.ed_pub, &k.ed_priv, 100).unwrap();
        let msg = share_message(&signed);
        let (s, got) = verify_share(&msg).unwrap();
        assert_eq!(s, story("ada"));
        assert_eq!(got.envelope.author, k.ed_pub);

        let mut tampered = msg.clone();
        tampered["story"]["envelope"]["body"]["text"] = serde_json::json!("something else");
        assert!(verify_share(&tampered).is_err());
    }

    #[test]
    fn members_only_and_unknown_terms_are_refused() {
        let k = keys::generate();
        let members = Story { visibility: "members".into(), ..story("ada") };
        let msg = share_message(&sign(&members, &k.ed_pub, &k.ed_priv, 1).unwrap());
        assert!(verify_share(&msg).unwrap_err().contains("members-only"));
        let odd = Story { license: "mine-all-mine".into(), ..story("ada") };
        assert!(verify_share(&share_message(&sign(&odd, &k.ed_pub, &k.ed_priv, 1).unwrap())).is_err());
    }

    #[test]
    fn a_story_needs_its_author_listed_under_its_handle() {
        let k = keys::generate();
        let signed = sign(&story("ada"), &k.ed_pub, &k.ed_priv, 100).unwrap();
        let s = story("ada");
        assert_eq!(admit(&s, &signed, &[], &[], &[]), StoryAdmission::NotListed);
        assert_eq!(admit(&s, &signed, &[listed("tega", &k.ed_pub)], &[], &[]), StoryAdmission::NotListed);
        assert_eq!(admit(&s, &signed, &[listed("ada", "someone-else")], &[], &[]), StoryAdmission::NotListed);
        assert_eq!(admit(&s, &signed, &[listed("ada", &k.ed_pub)], &[], &[]), StoryAdmission::Accept);
    }

    #[test]
    fn older_versions_and_withdrawn_copies_are_stale_but_resending_is_fine() {
        let k = keys::generate();
        let me = [listed("ada", &k.ed_pub)];
        let s = story("ada");
        let v2 = sign(&s, &k.ed_pub, &k.ed_priv, 200).unwrap();
        let v1 = sign(&s, &k.ed_pub, &k.ed_priv, 100).unwrap();
        let held = [HeldStory { story: s.clone(), signed: v2.clone(), received_at: 0 }];
        assert_eq!(admit(&s, &v2, &me, &held, &[]), StoryAdmission::Accept, "the same copy again");
        assert_eq!(admit(&s, &v1, &me, &held, &[]), StoryAdmission::Stale);
        let tomb = [StoryTombstone { author: k.ed_pub.clone(), id: ID.into(), withdrawn_at: 150 }];
        assert_eq!(admit(&s, &v1, &me, &[], &tomb), StoryAdmission::Stale, "a replay after withdrawal");
        assert_eq!(admit(&s, &v2, &me, &[], &tomb), StoryAdmission::Accept, "a newer version after withdrawal");
    }

    #[test]
    fn a_profile_lists_only_its_authors_stories_newest_first() {
        let (ada, bea) = (keys::generate(), keys::generate());
        let held_by = |k: &keys::NewKeys, handle_: &str, at: u64| {
            let s = story(handle_);
            HeldStory { story: s.clone(), signed: sign(&s, &k.ed_pub, &k.ed_priv, at).unwrap(), received_at: 0 }
        };
        let held = [held_by(&ada, "ada", 100), held_by(&bea, "bea", 200), held_by(&ada, "ada", 300)];
        let theirs = by_author(&held, &ada.ed_pub);
        assert_eq!(theirs.iter().map(|h| h.created_at()).collect::<Vec<_>>(), vec![300, 100]);
        assert!(by_author(&held, "someone-else").is_empty());
    }

    #[test]
    fn withdrawals_are_addressed_and_ordered() {
        let k = keys::generate();
        let msg = build_withdraw(&k.ed_pub, &k.ed_priv, ID, "http://room.local:8081", 300).unwrap();
        assert!(verify_withdraw(&msg, &["elsewhere:8081".to_string()]).is_err());
        let w = verify_withdraw(&msg, &["room.local:8081".to_string()]).unwrap();
        assert_eq!(w, StoryWithdrawal { author: k.ed_pub.clone(), id: ID.into(), created_at: 300 });

        let s = story("ada");
        let held_old =
            [HeldStory { story: s.clone(), signed: sign(&s, &k.ed_pub, &k.ed_priv, 100).unwrap(), received_at: 0 }];
        assert!(apply_withdraw(&w, &held_old, &[]).is_ok());
        let held_new =
            [HeldStory { story: s.clone(), signed: sign(&s, &k.ed_pub, &k.ed_priv, 400).unwrap(), received_at: 0 }];
        assert!(apply_withdraw(&w, &held_new, &[]).is_err(), "a later re-share stays");
        assert!(apply_withdraw(&w, &[], &[]).is_err(), "nothing to withdraw");
        let tomb = [StoryTombstone { author: k.ed_pub.clone(), id: ID.into(), withdrawn_at: 300 }];
        assert!(apply_withdraw(&w, &[], &tomb).is_ok(), "withdrawing twice is harmless");
    }

    #[test]
    fn editing_keeps_the_id_and_terms_and_marks_shared_copies_out_of_date() {
        let k = keys::generate();
        let s = story("ada");
        let own = OwnStory {
            signed: sign(&s, &k.ed_pub, &k.ed_priv, 100).unwrap(),
            story: s.clone(),
            shared_to: vec![],
            shared_at: BTreeMap::new(),
            travel: Travel::Copy,
            shared_as: BTreeMap::new(),
        }
        .with_shared_to("http://room", 150);
        assert!(!own.changed_since_shared("http://room"));
        let v2 = s.revised("Night bus, again", "New words.").unwrap();
        assert_eq!((v2.id.as_str(), v2.license.as_str()), (ID, "cc-by"));
        let own = own.with_version(v2.clone(), sign(&v2, &k.ed_pub, &k.ed_priv, 200).unwrap());
        assert!(own.changed_since_shared("http://room"), "the copy there is the old version");
        assert!(!own.with_shared_to("http://room", 210).changed_since_shared("http://room"));
        assert!(s.revised(" ", "x").is_err(), "checked like a new story");
    }

    #[test]
    fn each_new_version_is_dated_after_the_last() {
        let k = keys::generate();
        let s = story("ada");
        let own = OwnStory {
            signed: sign(&s, &k.ed_pub, &k.ed_priv, 500).unwrap(),
            story: s,
            shared_to: vec![],
            shared_at: BTreeMap::new(),
            travel: Travel::Copy,
            shared_as: BTreeMap::new(),
        };
        assert_eq!(own.next_version_at(900), 900, "normally, now");
        assert_eq!(own.next_version_at(500), 501, "same second: one later");
        assert_eq!(own.next_version_at(10), 501, "a clock that went back: still later");
    }

    #[test]
    fn switching_between_a_copy_and_a_card_always_moves_forward_in_time() {
        let k = keys::generate();
        let s = story("ada");
        let own = OwnStory::new(s.clone(), sign(&s, &k.ed_pub, &k.ed_priv, 500).unwrap());
        assert_eq!(own.shared_as("http://room"), None, "not shared");
        assert_eq!(own.next_share_at("http://room", 900), 900);
        // A card dated 1,000 is there now: whatever comes next is later.
        let carded = own.with_shared_as("http://room", 1_000, Travel::Here);
        assert_eq!(carded.shared_as("http://room"), Some(Travel::Here));
        assert_eq!(carded.next_share_at("http://room", 900), 1_001, "after the card, even if the clock is behind");
        assert_eq!(carded.withdraw_at("http://room", 900), 1_000, "a withdrawal is never older than the card");
        // A copy shared before this choice existed reads as a copy.
        let old = carded.without_shared_to("http://room").with_shared_to("http://room", 600);
        assert_eq!(old.shared_as("http://room"), Some(Travel::Copy));
        assert_eq!(old.next_share_at("http://room", 700), 700);
    }

    #[test]
    fn the_travel_choice_reads_from_a_form_and_defaults_to_a_copy() {
        assert_eq!(Travel::from_form("here"), Travel::Here);
        assert_eq!(Travel::from_form("copy"), Travel::Copy);
        assert_eq!(Travel::from_form("anything"), Travel::Copy);
        // A story file saved before the choice existed has neither field.
        let k = keys::generate();
        let own = OwnStory::new(story("ada"), sign(&story("ada"), &k.ed_pub, &k.ed_priv, 1).unwrap())
            .with_travel(Travel::Here)
            .with_shared_as("http://room", 5, Travel::Here);
        let mut saved = serde_json::to_value(&own).unwrap();
        saved.as_object_mut().unwrap().remove("travel");
        saved.as_object_mut().unwrap().remove("shared_as");
        let older: OwnStory = serde_json::from_value(saved).unwrap();
        assert_eq!(older.travel, Travel::Copy);
        assert_eq!(older.shared_as("http://room"), Some(Travel::Copy));
    }

    #[test]
    fn new_terms_are_tidied() {
        let s = story("ada").with_terms("members", "nonsense", &["open".into(), "x".into()], HeldSignals::default());
        assert_eq!(s.visibility, "members");
        assert_eq!(s.license, crate::governance::DEFAULT_LICENSE);
        assert_eq!(s.signals, vec!["open".to_string()]);
        assert_eq!(s.word_count(), 9, "The 38 sighs at the stop. A dog answers.");
    }

    #[test]
    fn a_commons_shows_recent_withdrawals_without_the_story() {
        let k = keys::generate();
        let tomb = StoryTombstone { author: k.ed_pub.clone(), id: ID.into(), withdrawn_at: 1_000 };
        let ada = [listed("ada", &k.ed_pub)];
        let recent = recent_withdrawals(std::slice::from_ref(&tomb), &[], &[], &ada, 1_100);
        assert_eq!(recent, vec![RecentWithdrawal { handle: "ada".into(), at: 1_000 }]);
        let a_day_later = 1_000 + WITHDRAWN_SHOWN_FOR + 1;
        assert!(recent_withdrawals(std::slice::from_ref(&tomb), &[], &[], &ada, a_day_later).is_empty(), "old news");
        assert!(recent_withdrawals(std::slice::from_ref(&tomb), &[], &[], &[], 1_100).is_empty(), "they left");
        let again = [HeldStory {
            story: story("ada"),
            signed: sign(&story("ada"), &k.ed_pub, &k.ed_priv, 1_050).unwrap(),
            received_at: 1_050,
        }];
        assert!(recent_withdrawals(&[tomb], &again, &[], &ada, 1_100).is_empty(), "shared again");
    }

    /// Held in Common Signals with the right to withdraw after training and
    /// some own terms, for these tests.
    fn held() -> HeldSignals {
        HeldSignals { withdraw_after_training: true, own_terms: "Not for voice models.".into() }
    }

    #[test]
    fn a_story_without_held_signals_signs_exactly_as_before() {
        let k = keys::generate();
        let signed = sign(&story("ada"), &k.ed_pub, &k.ed_priv, 100).unwrap();
        assert_eq!(signed.envelope.kind, STORY);
        assert!(signed.envelope.body.get("held_signals").is_none(), "nothing chosen, nothing added");
    }

    #[test]
    fn held_signals_are_signed_as_a_story_v2_and_tampering_shows() {
        let k = keys::generate();
        let s = story("ada").with_held_signals(held());
        let signed = sign(&s, &k.ed_pub, &k.ed_priv, 100).unwrap();
        assert_eq!(signed.envelope.kind, STORY_V2);
        let (got, _) = verify_share(&share_message(&signed)).unwrap();
        assert_eq!(got.held_signals, held());

        let mut msg = share_message(&signed);
        msg["story"]["envelope"]["body"]["held_signals"]["withdraw_after_training"] = json!(false);
        assert!(verify_share(&msg).is_err(), "a commons can't untick it");
        let mut msg = share_message(&signed);
        msg["story"]["envelope"]["body"]["held_signals"]["own_terms"] = json!("Anything goes.");
        assert!(verify_share(&msg).is_err(), "nor change the words");
    }

    #[test]
    fn a_story_type_may_not_carry_held_signals() {
        let k = keys::generate();
        let body = serde_json::to_value(story("ada").with_held_signals(held())).unwrap();
        let wrong = envelope::sign(Envelope::new(STORY, &k.ed_pub, 1, None, body), &k.ed_priv).unwrap();
        assert!(verify_share(&share_message(&wrong)).unwrap_err().contains("V2"));
    }

    #[test]
    fn an_older_commons_gets_the_same_story_without_held_signals() {
        let k = keys::generate();
        let s = story("ada").with_held_signals(held());
        let v2 = sign(&s, &k.ed_pub, &k.ed_priv, 100).unwrap();
        let older = older_copy(&s, &v2, &k.ed_priv).unwrap().expect("a StoryV2 has an older copy");
        assert_eq!(older.envelope.kind, STORY);
        assert_eq!(older.envelope.created_at, 100, "dated the same, so it's the same version");
        let (got, _) = verify_share(&share_message(&older)).unwrap();
        assert!(got.held_signals.is_empty());
        assert_eq!((got.id, got.title), (s.id.clone(), s.title.clone()));
        let plain = sign(&story("ada"), &k.ed_pub, &k.ed_priv, 100).unwrap();
        assert!(older_copy(&story("ada"), &plain, &k.ed_priv).unwrap().is_none(), "nothing older to try");
    }

    #[test]
    fn editing_keeps_held_signals_and_new_terms_can_clear_them() {
        let s = story("ada").with_held_signals(held());
        assert_eq!(s.revised("New title", "New words.").unwrap().held_signals, held());
        let cleared = s.with_terms("public", "cc-by", &[], HeldSignals::default());
        assert!(cleared.held_signals.is_empty() && cleared.kind() == STORY);
    }

    #[test]
    fn ids_are_checked() {
        assert!(is_story_id(ID));
        assert!(!is_story_id("0123456789ABCDEF"));
        assert!(!is_story_id("../../etc/passwd"));
        assert!(!is_story_id("0123"));
    }
}
