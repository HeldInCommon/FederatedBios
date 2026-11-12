//! Lists of stories: a node's own, and the ones shared to a commons (with
//! recent withdrawals, which name who withdrew and when, and nothing else).

use maud::{html, Markup};

use super::parts::{ago, page_head, section_label};
use crate::governance::{license_name, signal_name};
use crate::keys::keyid;
use crate::model::Reach;
use crate::story::{HeldStory, OwnStory, RecentWithdrawal, Story};

/// The licence and signals as small chips. With `edit_terms` (the story is
/// the viewer's own), the chips are one link to its rights step, named for
/// screen readers by what it does.
fn chips(s: &Story, edit_terms: Option<&str>) -> Markup {
    let inner = html! {
        span class="story-chip" { (license_name(&s.license)) }
        @for sig in &s.signals { span class="story-chip" { (signal_name(sig)) } }
    };
    match edit_terms {
        Some(href) => {
            let mut terms = vec![license_name(&s.license).to_string()];
            terms.extend(s.signals.iter().map(|sig| signal_name(sig).to_string()));
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
fn card(href: &str, s: &Story, tone: &str, foot_end: Markup, edit_terms: Option<&str>) -> Markup {
    html! {
        article class={ "story-card " (tone) } {
            div class="card-bar" {}
            div class="card-body" {
                p class="card-label" { "Story \u{b7} @" (s.handle) }
                h2 class="card-title" { a href=(href) { (s.title) } }
                p class="card-first" { (s.first_line()) }
                div class="card-foot" {
                    (chips(s, edit_terms))
                    span class="card-foot-end" { (foot_end) }
                }
            }
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
        @if stories.is_empty() {
            p class="empty-note" {
                "No stories yet. "
                @if signed_in { a href="/stories/new" { "Write the first one" } "." } @else { a href="/login" { "Sign in" } " to write one." }
            }
        }
        div class="story-grid" {
            // Writing another is always the first card, so it's there after
            // a withdrawal or whenever there's more to say.
            @if signed_in && !stories.is_empty() {
                a class="new-story-card" href="/stories/new" {
                    span class="star step-1" aria-hidden="true" { "+" }
                    span class="new-story-title" { "Write a new story" }
                    span class="new-story-sub" { "It stays on this computer until you share it." }
                }
            }
            @for s in stories {
                @let where_ = match s.shared_to.len() {
                    0 => "On this computer".to_string(),
                    1 => "Shared to 1 commons".to_string(),
                    n => format!("Shared to {n} commons"),
                };
                // `bool::then`: `Some(link)` when it's the viewer's own story, else `None`.
                @let edit = (me == Some(s.story.handle.as_str())).then(|| format!("/story/{}/rights", s.story.id));
                (card(&format!("/story/{}", s.story.id), &s.story, tone_for(s.story.reach()), html! { (where_) }, edit.as_deref()))
            }
        }
    }
}

/// The stories one member has shared to this commons, under their profile:
/// everything of theirs this commons holds, in one place.
pub fn author_stories(handle: &str, held: &[&HeldStory]) -> Markup {
    let count = format!("{} shared here", held.len());
    html! {
        (section_label(&format!("Stories by @{handle}"), Some(&count)))
        @if held.is_empty() {
            p class="empty-note" { "@" (handle) " hasn't shared any stories here yet." }
        } @else {
            div class="story-grid" {
                @for h in held {
                    (card(&format!("/stories/{}/{}", keyid(h.author()), h.story.id), &h.story, "tone-violet", verified(), None))
                }
            }
        }
    }
}

/// The stories shared to this commons, most recently arrived first, then
/// any withdrawn in the last day.
pub fn commons_stories(instance: &str, held: &[HeldStory], withdrawn: &[RecentWithdrawal], now: u64) -> Markup {
    let count = if withdrawn.is_empty() {
        format!("{} shared", held.len())
    } else {
        format!("{} shared \u{b7} {} withdrawn", held.len(), withdrawn.len())
    };
    html! {
        (page_head(&format!("{instance} \u{b7} stories"), "Stories", Some("Small stories, each still held by the person who wrote it.")))
        (section_label("Stories", Some(&count)))
        @if held.is_empty() && withdrawn.is_empty() {
            p class="empty-note" { "No stories here yet. Members write them on their own nodes and share them in." }
        }
        div class="story-grid" {
            @for h in held {
                (card(&format!("/stories/{}/{}", keyid(h.author()), h.story.id), &h.story, "tone-violet", verified(), None))
            }
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
    fn room_cards_show_terms_and_verification() {
        let m = commons_stories("Room 4", &[held("Night bus")], &[], 10).into_string();
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
        }];
        let mine = node_stories(&own, Some("ada")).into_string();
        assert!(mine.contains("href=\"/story/0123456789abcdef/rights\""));
        assert!(mine.contains("aria-label=\"Change the terms of Night bus: CC BY 4.0, Credit\""));
        let theirs = node_stories(&own, Some("sam")).into_string();
        assert!(!theirs.contains("/rights"), "only the author can change the terms");
        assert!(!node_stories(&own, None).into_string().contains("/rights"));
        assert!(!commons_stories("Room 4", &[held("Night bus")], &[], 10).into_string().contains("/rights"));
    }

    #[test]
    fn a_withdrawal_names_who_and_when_but_keeps_nothing_of_the_story() {
        let w = [RecentWithdrawal { handle: "sam".into(), at: 100 }];
        let m = commons_stories("Room 4", &[], &w, 220).into_string();
        assert!(m.contains("Withdrawn by @sam \u{b7} 2 min ago"));
        assert!(m.contains("0 shared \u{b7} 1 withdrawn"));
        assert!(!m.contains("card-title"), "no title");
    }
}
