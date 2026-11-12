//! Lists of stories: a node's own, and the ones shared to a commons (with
//! recent withdrawals, which name who withdrew and when, and nothing else).

use maud::{html, Markup};

use super::parts::{ago, page_head, section_label};
use crate::governance::{license_name, signal_name};
use crate::here::{Card, HeldCard};
use crate::keys::keyid;
use crate::model::Reach;
use crate::story::{HeldStory, OwnStory, RecentWithdrawal, Story};

/// What a story card shows, from a story (its text is there) or from an
/// "Only while I'm here" card (only its signed first line is).
struct Preview<'a> {
    /// The author's handle.
    handle: &'a str,
    /// The title.
    title: &'a str,
    /// The first line.
    first_line: String,
    /// The licence key.
    license: &'a str,
    /// The signal keys.
    signals: &'a [String],
    /// The Held in Common Signals chosen, as chip labels (decision record
    /// 0025). A card has none: they travel only with a copy.
    held: Vec<&'static str>,
}

impl<'a> Preview<'a> {
    /// The preview of a story.
    fn of_story(s: &'a Story) -> Preview<'a> {
        Preview {
            handle: &s.handle,
            title: &s.title,
            first_line: s.first_line(),
            license: &s.license,
            signals: &s.signals,
            held: s.held_signals.labels(),
        }
    }

    /// The preview of a card.
    fn of_card(c: &'a Card) -> Preview<'a> {
        Preview {
            handle: &c.handle,
            title: &c.title,
            first_line: c.first_line.clone(),
            license: &c.license,
            signals: &c.signals,
            held: Vec::new(),
        }
    }
}

/// The licence, signals and Held in Common Signals as small chips. With `edit_terms` (the story is
/// the viewer's own), the chips are one link to its rights step, named for
/// screen readers by what it does.
fn chips(s: &Preview, edit_terms: Option<&str>) -> Markup {
    let inner = html! {
        span class="story-chip" { (license_name(s.license)) }
        @for sig in s.signals { span class="story-chip" { (signal_name(sig)) } }
        @for label in &s.held { span class="story-chip" { (label) } }
    };
    match edit_terms {
        Some(href) => {
            let mut terms = vec![license_name(s.license).to_string()];
            terms.extend(s.signals.iter().map(|sig| signal_name(sig).to_string()));
            terms.extend(held.iter().map(|label| label.to_string()));
            let label = format!("Change the terms of {}: {}", s.title, terms.join(", "));
            html! { a class="story-chips is-editable" href=(href) aria-label=(label) title="Change its terms" { (inner) } }
        }
        None => html! { span class="story-chips" { (inner) } },
    }
}

/// The colour of a story card's bar, by who can see it.
fn tone_for(reach: Reach) -> &'static str {
    match reach {
        Reach::Public => "tone-pink",
        Reach::Commons => "tone-violet",
        Reach::Members => "tone-ink",
    }
}

/// One story as a card: its author, title (the link), first line, terms, and
/// something at the end of the foot (where it is, or that it's verified).
fn card(href: &str, s: &Preview, tone: &str, foot_end: Markup, edit_terms: Option<&str>) -> Markup {
    html! {
        article class={ "story-card " (tone) } {
            div class="card-bar" {}
            div class="card-body" {
                p class="card-label" { "Story \u{b7} @" (s.handle) }
                h2 class="card-title" { a href=(href) { (s.title) } }
                p class="card-first" { (s.first_line) }
                div class="card-foot" {
                    (chips(s, edit_terms))
                    // Your own story: edit it from the card.
                    @if edit_terms.is_some() {
                        a class="card-edit" href={ (href) "/edit" } { "Edit" }
                    }
                    span class="card-foot-end" { (foot_end) }
                }
            }
        }
    }
}

/// The "Write a new story" card that leads every story grid: `href` is where
/// writing happens (this node's form, or the way to a node), `sub` says so.
fn new_story_card(href: &str, sub: &str) -> Markup {
    html! {
        a class="new-story-card" href=(href) {
            span class="star step-1" aria-hidden="true" { "+" }
            span class="new-story-title" { "Write a new story" }
            span class="new-story-sub" { (sub) }
        }
    }
}

/// "Verified", with a tick.
fn verified() -> Markup {
    html! {
        span class="verified" {
            svg width="12" height="12" viewBox="0 0 24 24" aria-hidden="true" { path d="M5 12l5 5L20 7" {} }
            "Verified"
        }
    }
}

/// A node's stories: the ones this viewer may read, newest first. `me` is
/// the signed-in member: on their own stories the chips open the rights step.
pub fn node_stories(stories: &[OwnStory], me: Option<&str>) -> Markup {
    let signed_in = me.is_some();
    let count = match stories.len() {
        1 => "1 story".to_string(),
        n => format!("{n} stories"),
    };
    html! {
        (page_head("Your node \u{b7} stories", "Stories", Some("Small pieces of writing made on this node. Each stays here until its author shares it.")))
        (section_label("Stories", Some(&count)))
        div class="story-grid" {
            // Writing another is always the first card, so it's there after
            // a withdrawal, on an empty page, or whenever there's more to say.
            // Signed out, the form asks you to sign in first.
            (new_story_card("/stories/new", if signed_in {
                "It stays on this computer until you share it."
            } else {
                "Sign in first; it stays on this computer until you share it."
            }))
            @for s in stories {
                @let where_ = match s.shared_to.len() {
                    0 => "On this computer".to_string(),
                    1 => "Shared to 1 commons".to_string(),
                    n => format!("Shared to {n} commons"),
                };
                // `bool::then`: `Some(link)` when it's the viewer's own story, else `None`.
                @let edit = (me == Some(s.story.handle.as_str())).then(|| format!("/story/{}/rights", s.story.id));
                (card(&format!("/story/{}", s.story.id), &Preview::of_story(&s.story), tone_for(s.story.reach()), html! { (where_) }, edit.as_deref()))
            }
        }
    }
}

/// A shared copy as a card in a commons' grid.
fn copy_card(h: &HeldStory) -> Markup {
    let href = format!("/stories/{}/{}", keyid(h.author()), h.story.id);
    card(&href, &Preview::of_story(&h.story), "tone-violet", verified(), None)
}

/// An "Only while I'm here" card in a commons' grid: jade, and saying so,
/// since it can only be read while its author is online.
fn here_card(h: &HeldCard) -> Markup {
    let href = format!("/stories/{}/{}", keyid(h.author()), h.card.id);
    let foot = html! { "Only while @" (h.card.handle) " is here" };
    card(&href, &Preview::of_card(&h.card), "tone-jade", foot, None)
}

/// The stories one member has shared to this commons, under their profile:
/// everything of theirs this commons holds, in one place (copies, then
/// "Only while I'm here" cards).
pub fn author_stories(handle: &str, held: &[&HeldStory], cards: &[HeldCard]) -> Markup {
    let count = format!("{} shared here", held.len() + cards.len());
    html! {
        (section_label(&format!("Stories by @{handle}"), Some(&count)))
        @if held.is_empty() && cards.is_empty() {
            p class="empty-note" { "@" (handle) " hasn't shared any stories here yet." }
        } @else {
            div class="story-grid" {
                @for h in held { (copy_card(h)) }
                @for c in cards { (here_card(c)) }
            }
        }
    }
}

/// The stories shared to this commons: copies, then "Only while I'm here"
/// cards, each most recently arrived first; then any withdrawn in the last day.
pub fn commons_stories(
    instance: &str,
    in_app: bool,
    held: &[HeldStory],
    cards: &[HeldCard],
    withdrawn: &[RecentWithdrawal],
    now: u64,
) -> Markup {
    let shared = held.len() + cards.len();
    let count = if withdrawn.is_empty() {
        format!("{shared} shared")
    } else {
        format!("{shared} shared \u{b7} {} withdrawn", withdrawn.len())
    };
    html! {
        (page_head(&format!("{instance} \u{b7} stories"), "Stories", Some("Small stories, each still held by the person who wrote it.")))
        (section_label("Stories", Some(&count)))
        div class="story-grid" {
            // Stories are written on a node: in the app, yours is one click
            // away; anyone else is shown how to get one.
            @if in_app {
                (new_story_card(super::guide::OPEN_NODE, "Stories are written on your node, then shared here."))
            } @else {
                (new_story_card("/join", "Stories are written on your own node. Here's how to get one."))
            }
            @for h in held { (copy_card(h)) }
            @for c in cards { (here_card(c)) }
            @for w in withdrawn {
                article class="story-card is-withdrawn" {
                    div class="card-bar" {}
                    div class="card-body" {
                        p class="card-label" { "Story \u{b7} @" (w.handle) }
                        p class="card-first" { "A story was withdrawn. The copy has been deleted from this commons." }
                        div class="card-foot" { "Withdrawn by @" (w.handle) " \u{b7} " (ago(w.at, now)) }
                    }
                }
            }
        }
        @if !held.is_empty() {
            p class="hint" { "Verified: the commons checked each story's signature against its author's key." }
        }
        @if !cards.is_empty() {
            p class="hint" { "Only while they're here: this commons keeps a signed card, and fetches the words from the author's own computer while they're online." }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;
    use crate::story::{new_story, sign};

    /// A story shared to a commons, for these tests.
    fn held(title: &str) -> HeldStory {
        let k = keys::generate();
        let s =
            new_story("0123456789abcdef", "ada", title, "First line.\nMore.", "commons", "cc-by", &["credit".into()])
                .unwrap();
        HeldStory { signed: sign(&s, &k.ed_pub, &k.ed_priv, 1).unwrap(), story: s, received_at: 1 }
    }

    #[test]
    fn a_commons_leads_to_a_node_for_writing() {
        let in_app = commons_stories("Room 4", true, &[], &[], &[], 10).into_string();
        assert!(in_app.contains(r#"class="new-story-card" href="hic-app://open/node""#), "in the app: your node");
        let visitor = commons_stories("Room 4", false, &[], &[], &[], 10).into_string();
        assert!(visitor.contains(r#"class="new-story-card" href="/join""#), "elsewhere: how to get one");
    }

    #[test]
    fn room_cards_show_terms_and_verification() {
        let m = commons_stories("Room 4", false, &[held("Night bus")], &[], &[], 10).into_string();
        assert!(m.contains("Room 4 \u{b7} stories") && m.contains("Night bus") && m.contains("First line."));
        assert!(m.contains("CC BY 4.0") && m.contains("Credit") && m.contains("Verified"));
        assert!(m.contains("1 shared"));
    }

    #[test]
    fn your_own_story_chips_open_its_terms_and_nobody_elses_do() {
        let k = keys::generate();
        let s = new_story("0123456789abcdef", "ada", "Night bus", "Words.", "commons", "cc-by", &["credit".into()])
            .unwrap();
        let own = [OwnStory {
            signed: sign(&s, &k.ed_pub, &k.ed_priv, 1).unwrap(),
            story: s,
            shared_to: vec![],
            shared_at: Default::default(),
            travel: Default::default(),
            shared_as: Default::default(),
        }];
        let mine = node_stories(&own, Some("ada")).into_string();
        assert!(mine.contains("href=\"/story/0123456789abcdef/rights\""));
        assert!(mine.contains("aria-label=\"Change the terms of Night bus: CC BY 4.0, Credit\""));
        let theirs = node_stories(&own, Some("sam")).into_string();
        assert!(!theirs.contains("/rights"), "only the author can change the terms");
        assert!(!node_stories(&own, None).into_string().contains("/rights"));
        assert!(!commons_stories("Room 4", false, &[held("Night bus")], &[], &[], 10)
            .into_string()
            .contains("/rights"));
    }

    #[test]
    fn cards_show_held_in_common_signals_beside_the_cc_signals() {
        let mut h = held("Night bus");
        h.story.held_signals =
            crate::held_signals::HeldSignals { withdraw_after_training: true, own_terms: "Words.".into() };
        let m = commons_stories("Room 4", false, &[h], &[], 10).into_string();
        assert!(m.contains(">Credit<") && m.contains(">Withdraw even after training<") && m.contains(">Own terms<"));
    }

    #[test]
    fn a_withdrawal_names_who_and_when_but_keeps_nothing_of_the_story() {
        let w = [RecentWithdrawal { handle: "sam".into(), at: 100 }];
        let m = commons_stories("Room 4", false, &[], &[], &w, 220).into_string();
        assert!(m.contains("Withdrawn by @sam \u{b7} 2 min ago"));
        assert!(m.contains("0 shared \u{b7} 1 withdrawn"));
        assert!(!m.contains("card-title"), "no title");
    }
}
