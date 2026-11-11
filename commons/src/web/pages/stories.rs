//! Lists of stories: a node's own, and the ones shared to a commons (with
//! recent withdrawals, which name who withdrew and when, and nothing else).

use maud::{html, Markup};

use super::parts::{ago, page_head, section_label};
use crate::governance::{license_name, signal_name};
use crate::keys::keyid;
use crate::model::Reach;
use crate::story::{HeldStory, OwnStory, RecentWithdrawal, Story};

/// The licence and signals as small chips.
fn chips(s: &Story) -> Markup {
    html! {
        span class="story-chip" { (license_name(&s.license)) }
        @for sig in &s.signals { span class="story-chip" { (signal_name(sig)) } }
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
fn card(href: &str, s: &Story, tone: &str, foot_end: Markup) -> Markup {
    html! {
        article class={ "story-card " (tone) } {
            div class="card-bar" {}
            div class="card-body" {
                p class="card-label" { "Story \u{b7} @" (s.handle) }
                h2 class="card-title" { a href=(href) { (s.title) } }
                p class="card-first" { (s.first_line()) }
                div class="card-foot" {
                    (chips(s))
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

/// A node's stories: the ones this viewer may read, newest first.
pub fn node_stories(stories: &[OwnStory], signed_in: bool) -> Markup {
    let count = match stories.len() {
        1 => "1 story".to_string(),
        n => format!("{n} stories"),
    };
    html! {
        (page_head("Your node \u{b7} stories", "Stories", Some("Small pieces of writing made on this node. Each stays here until its author shares it.")))
        @if signed_in {
            div class="actions" { a class="btn-ink" href="/stories/new" { "Write a story" } }
        }
        (section_label("Stories", Some(&count)))
        @if stories.is_empty() {
            p class="empty-note" {
                "No stories yet. "
                @if signed_in { a href="/stories/new" { "Write the first one" } "." } @else { a href="/login" { "Sign in" } " to write one." }
            }
        }
        div class="story-grid" {
            @for s in stories {
                @let where_ = match s.shared_to.len() {
                    0 => "On this computer".to_string(),
                    1 => "Shared to 1 commons".to_string(),
                    n => format!("Shared to {n} commons"),
                };
                (card(&format!("/story/{}", s.story.id), &s.story, tone_for(s.story.reach()), html! { (where_) }))
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
                (card(&format!("/stories/{}/{}", keyid(h.author()), h.story.id), &h.story, "tone-violet", verified()))
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
    fn a_withdrawal_names_who_and_when_but_keeps_nothing_of_the_story() {
        let w = [RecentWithdrawal { handle: "sam".into(), at: 100 }];
        let m = commons_stories("Room 4", &[], &w, 220).into_string();
        assert!(m.contains("Withdrawn by @sam \u{b7} 2 min ago"));
        assert!(m.contains("0 shared \u{b7} 1 withdrawn"));
        assert!(!m.contains("card-title"), "no title");
    }
}
