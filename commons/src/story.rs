//! Stories: short pieces of writing a member makes on their node (in a
//! workshop: "the sound of your street") and may share to a commons.
//!
//! A story is a `hic/v1` object of type `Story` (docs/spec §5.4), signed by
//! the member's key. Its `id` stays the same across versions, so a commons
//! keeps one story per (author, id): the latest. A `StoryWithdraw`, also
//! signed and addressed to one commons, removes it there and leaves a record
//! so the same copy can't be replayed back in.
//!
//! A commons accepts a story only from someone it already lists under that
//! handle. Joining stays the gate (decision record 0011); stories ride on it.
//!
//! Everything here is pure; the request handlers do the I/O.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::envelope::{self, Envelope, Signed};
use crate::governance::{license_from_form, signals_from_form, LICENSES, SIGNALS};
use crate::handle;
use crate::model::{Reach, RemoteEntry};
use crate::web::layout::host_label;

/// The `hic/v1` type of a story.
pub const STORY: &str = "Story";
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
    pub handle: String,
    pub title: String,
    pub text: String,
    /// `public`, `commons` or `members` (members-only stories never leave the node).
    pub visibility: String,
    pub license: String,
    pub signals: Vec<String>,
}

impl Story {
    /// Who may see it.
    pub fn reach(&self) -> Reach {
        Reach::parse(&self.visibility).unwrap_or(Reach::Public)
    }

    /// The first line with any words in it, cut to about 140 characters: a
    /// card's preview.
    pub fn first_line(&self) -> String {
        let line = self.text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
        if line.chars().count() <= 140 {
            line.to_string()
        } else {
            format!("{}\u{2026}", line.chars().take(139).collect::<String>().trim_end())
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

/// A story on the node that wrote it: what it says, its signed form, and the
/// commons it has been shared to.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OwnStory {
    pub story: Story,
    pub signed: Signed,
    #[serde(default)]
    pub shared_to: Vec<String>,
}

impl OwnStory {
    /// When it was written (the signed time).
    pub fn created_at(&self) -> u64 {
        self.signed.envelope.created_at
    }

    /// The same story, recorded as shared to `commons`.
    pub fn with_shared_to(mut self, commons: &str) -> OwnStory {
        if !self.shared_to.iter().any(|c| c == commons) {
            self.shared_to.push(commons.to_string());
        }
        self
    }

    /// The same story, no longer recorded as shared to `commons`.
    pub fn without_shared_to(mut self, commons: &str) -> OwnStory {
        self.shared_to.retain(|c| c != commons);
        self
    }
}

/// A story shared to a commons, kept with its signature as proof.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeldStory {
    pub story: Story,
    pub signed: Signed,
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

/// What remains on a commons after a story is withdrawn.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StoryTombstone {
    pub author: String,
    pub id: String,
    pub withdrawn_at: u64,
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
    })
}

/// Sign a story with its author's key, dated `created_at`.
pub fn sign(story: &Story, author_pub: &str, author_priv: &str, created_at: u64) -> Result<Signed, String> {
    let body = serde_json::to_value(story).map_err(|e| e.to_string())?;
    envelope::sign(Envelope::new(STORY, author_pub, created_at, None, body), author_priv)
}

/// The message a node posts to a commons' `/inbox` to share a story.
pub fn share_message(signed: &Signed) -> Value {
    json!({ "type": "StoryShare", "schema": envelope::SCHEMA, "story": signed })
}

/// Why a story can't be federated, if it can't: everything a commons checks
/// about the content itself.
fn check(story: &Story) -> Result<(), String> {
    if !handle::is_valid(&story.handle) {
        return Err("invalid handle".into());
    }
    if !is_story_id(&story.id) {
        return Err("invalid story id".into());
    }
    if story.title.trim().is_empty() || story.title.chars().count() > TITLE_MAX {
        return Err("the title is empty or too long".into());
    }
    if story.text.trim().is_empty() || story.text.chars().count() > TEXT_MAX {
        return Err("the story is empty or too long".into());
    }
    match Reach::parse(&story.visibility) {
        Some(Reach::Members) => return Err("members-only stories are not federated".into()),
        None => return Err("unknown visibility".into()),
        Some(_) => {}
    }
    if !LICENSES.iter().any(|l| l.0 == story.license) {
        return Err("unknown licence".into());
    }
    if !story.signals.iter().all(|s| SIGNALS.iter().any(|k| k.0 == s)) {
        return Err("unknown signal".into());
    }
    Ok(())
}

/// Check a received story share: a valid `Story` signed by its `author`,
/// with content a commons may list.
pub fn verify_share(msg: &Value) -> Result<(Story, Signed), String> {
    let signed: Signed =
        serde_json::from_value(msg.get("story").cloned().ok_or("missing story")?).map_err(|_| "bad story")?;
    envelope::verify(&signed)?;
    if signed.envelope.kind != STORY {
        return Err(format!("expected a Story, got {:?}", signed.envelope.kind));
    }
    let story: Story = serde_json::from_value(signed.envelope.body.clone()).map_err(|_| "bad story body")?;
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
    pub author: String,
    pub id: String,
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

    const ID: &str = "0123456789abcdef";

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
    fn ids_are_checked() {
        assert!(is_story_id(ID));
        assert!(!is_story_id("0123456789ABCDEF"));
        assert!(!is_story_id("../../etc/passwd"));
        assert!(!is_story_id("0123"));
    }
}
