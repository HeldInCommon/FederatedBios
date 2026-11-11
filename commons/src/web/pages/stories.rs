//! Stories: the writing form, a node's stories, a commons' stories, and one
//! story (with its author's sharing tools on their own node).

use maud::{html, Markup};

use crate::governance::DEFAULT_LICENSE;
use crate::keys::keyid;
use crate::model::Reach;
use crate::story::{HeldStory, OwnStory, Story, PROMPT, TEXT_MAX, TITLE_MAX};
use crate::web::layout::{gov_row, host_label, reach_badge, reach_bar_class};

/// What just happened to a story, confirmed at the top of its page.
pub enum StoryOutcome {
    /// Shared to this commons (its host).
    Shared(String),
    /// Withdrawn from this commons (its host).
    Withdrawn(String),
}

/// The confirmation block for an outcome.
fn outcome_block(o: &StoryOutcome) -> Markup {
    match o {
        StoryOutcome::Shared(at) => html! {
            section class="flash flash-done" role="status" {
                p class="flash-label" { "Shared" }
                p { strong { "Your story is on " (at) "." } " People there can read it now, signed by you. You can withdraw it any time, below." }
            }
        },
        StoryOutcome::Withdrawn(at) => html! {
            section class="flash flash-withdrawn" role="status" {
                p class="flash-label" { "Withdrawn" }
                p { strong { "Your story is gone from " (at) "." } " That commons deleted its copy and won't take the same one back. Anyone who already read it, or copied it, keeps that. It's still here on your node, and you can share it again." }
            }
        },
    }
}

/// What the author sees under their own story.
pub struct StoryOwner {
    /// The session's CSRF token, for the forms.
    pub csrf: String,
    /// The commons they can share to in one click (where their profile is shared).
    pub known: Vec<String>,
}

/// The writing form, with the workshop prompt, the full "who can see it"
/// choices (reach, licence, signals) open by default, and how it travels.
/// (Timing the writing is left to whoever runs the room.)
pub fn story_form(me: &str, csrf: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Write a story" }
            p class="lede" { (PROMPT) }
        }
        form method="post" action="/stories/new" class="stack-form" {
            input type="hidden" name="handle" value=(me);
            input type="hidden" name="csrf" value=(csrf);
            div class="field" {
                label class="field-label" for="title" { "Title" }
                input id="title" name="title" type="text" maxlength=(TITLE_MAX) required placeholder="The night bus";
            }
            div class="field" {
                label class="field-label" for="text" { "Your story" }
                textarea id="text" name="text" class="story-input" rows="16" maxlength=(TEXT_MAX) required {}
                p class="hint" { "It's saved on this computer. Nothing leaves it until you choose to share." }
            }
            (super::publish::reach_and_governance(Reach::Public, DEFAULT_LICENSE, &[]))
            fieldset class="travel-options" {
                legend class="field-label" { "How it travels" }
                label class="reach" {
                    input type="radio" name="travel" value="copy" checked;
                    span class="reach-text" {
                        span class="reach-title" { "Leave a copy in the commons" }
                        span class="reach-body" { "Stays readable after you leave. Withdraw it any time." }
                    }
                }
                label class="reach reach-soon" {
                    input type="radio" name="travel" value="here" disabled;
                    span class="reach-text" {
                        span class="reach-title" { "Only while I am here " span class="soon-tag" { "coming soon" } }
                        span class="reach-body" { "Read from your laptop while you are in the room, never stored on the commons." }
                    }
                }
            }
            div class="actions" { button class="primary" type="submit" { "Save story" } }
        }
    }
}

/// One story as a directory card.
fn card(href: &str, s: &Story, bar_class: &str, footnote: Option<&str>) -> Markup {
    html! {
        a class="dir-card story-card" href=(href) {
            span class={ "dir-bar " (bar_class) } {}
            span class="dir-body" {
                span class="dir-name" { (s.title) }
                span class="dir-handle" { "@" (s.handle) }
                span class="dir-role story-preview" { (s.first_line()) }
                @if let Some(f) = footnote { span class="dir-key" { (f) } }
                (reach_badge(s.reach()))
            }
        }
    }
}

/// A node's stories: the ones this viewer may read, newest first.
pub fn node_stories(stories: &[OwnStory], signed_in: bool) -> Markup {
    html! {
        header class="intro" {
            h1 { "Stories" }
            p class="lede" { "Small pieces of writing made on this node. Each stays here until its author shares it." }
        }
        @if signed_in {
            div class="actions story-actions" { a class="primary" href="/stories/new" { "Write a story" } }
        }
        @if stories.is_empty() {
            p class="empty-note" {
                "No stories yet. "
                @if signed_in { a href="/stories/new" { "Write the first one" } "." } @else { a href="/login" { "Sign in" } " to write one." }
            }
        }
        div class="dir-grid" {
            @for s in stories {
                (card(&format!("/story/{}", s.story.id), &s.story, reach_bar_class(s.story.reach()), None))
            }
        }
    }
}

/// The stories shared to this commons, most recently arrived first.
pub fn commons_stories(stories: &[HeldStory]) -> Markup {
    html! {
        header class="intro" {
            h1 { "Stories" }
            p class="lede" { "Shared here by people listed on this commons. Each one is signed by its author, and they can withdraw it at any time." }
        }
        @if stories.is_empty() {
            p class="empty-note" { "No stories here yet. Members write them on their own nodes and share them in." }
        }
        div class="dir-grid" {
            @for h in stories {
                (card(
                    &format!("/stories/{}/{}", keyid(h.author()), h.story.id),
                    &h.story,
                    "vis-federated",
                    Some("signature checked \u{2713}"),
                ))
            }
        }
    }
}

/// A story's text: blank lines separate paragraphs, single line breaks stay.
fn story_text(s: &Story) -> Markup {
    html! {
        div class="prose story-text" {
            @for para in s.text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
                p {
                    @for (i, line) in para.lines().enumerate() {
                        @if i > 0 { br; }
                        (line)
                    }
                }
            }
        }
    }
}

/// The heading shared by both story pages.
fn story_head(s: &Story, by: Markup) -> Markup {
    html! {
        header class="intro" {
            h1 { (s.title) }
            p class="lede" { (by) }
        }
        div class="story-terms" {
            (reach_badge(s.reach()))
            (gov_row(&s.license, &s.signals))
        }
    }
}

/// A story on its author's node. With `owner`, the sharing tools, and the
/// confirmation of what they just did (`done`).
pub fn own_story(s: &OwnStory, owner: Option<&StoryOwner>, done: Option<&StoryOutcome>) -> Markup {
    let st = &s.story;
    html! {
        @if let Some(d) = done { (outcome_block(d)) }
        (story_head(st, html! { "by @" (st.handle) }))
        (story_text(st))
        @if let Some(o) = owner { (owner_tools(s, o)) }
    }
}

/// Share, re-share, withdraw and export, for the author.
fn owner_tools(s: &OwnStory, o: &StoryOwner) -> Markup {
    let id = &s.story.id;
    html! {
        section class="owner-panel" {
            section class="share-panel" {
                h2 class="field-label" { "Share this story" }
                @if s.story.reach() == Reach::Members {
                    p class="hint" { "This story is " strong { "members only" } ", so it stays on your node. Write a new story with another choice to share one." }
                } @else if o.known.is_empty() {
                    p class="hint" { "Stories go to a commons where you're listed. Share your profile to a commons first (from your profile page), then come back here." }
                } @else {
                    p class="hint" { "Sends a signed copy to the commons. People there can read it; it stays yours, and you can withdraw it." }
                    ul class="commons-list" {
                        @for c in &o.known {
                            @let shared = s.shared_to.iter().any(|x| x == c);
                            li class="commons-item" {
                                form method="post" action="/story/share" class="commons-share" {
                                    input type="hidden" name="id" value=(id);
                                    input type="hidden" name="csrf" value=(o.csrf);
                                    input type="hidden" name="commons_url" value=(c);
                                    span class="commons-meta" {
                                        span class="commons-name" { (host_label(c)) }
                                        @if shared { span class="commons-status" { "shared \u{2713}" } }
                                    }
                                    button class="secondary" type="submit" { (if shared { "Re-share" } else { "Share" }) }
                                }
                            }
                        }
                    }
                }
            }
            @if !s.shared_to.is_empty() {
                section class="share-panel withdraw-panel" {
                    h2 class="field-label" { "Withdraw" }
                    p class="hint" { "Asks the commons to delete its copy, straight away. Anyone who already read it, or copied it, keeps that." }
                    ul class="commons-list" {
                        @for c in &s.shared_to {
                            li class="commons-item commons-row" {
                                span class="commons-meta" { span class="commons-name" { (host_label(c)) } }
                                form method="post" action="/story/withdraw" {
                                    input type="hidden" name="id" value=(id);
                                    input type="hidden" name="csrf" value=(o.csrf);
                                    input type="hidden" name="commons_url" value=(c);
                                    button class="secondary" type="submit" { "Withdraw" }
                                }
                            }
                        }
                    }
                }
            }
            p { a class="secondary-link" href={ "/story/" (id) "/text" } { "Export as a text file" } }
        }
    }
}

/// A story shared to this commons.
pub fn held_story(h: &HeldStory) -> Markup {
    let st = &h.story;
    let kid = keyid(h.author());
    html! {
        (story_head(st, html! { "by " a href={ "/federated/" (kid) } { "@" (st.handle) } }))
        (story_text(st))
        p class="hint story-proof" {
            "Signed by @" (st.handle) "'s key " span class="mono" { (&h.author()[..12.min(h.author().len())]) "\u{2026}" }
            " and checked when it arrived. " a href="/stories" { "All stories" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;
    use crate::story::{new_story, sign};

    fn own(text: &str, visibility: &str) -> OwnStory {
        let k = keys::generate();
        let s = new_story("0123456789abcdef", "ada", "Night bus", text, visibility, "cc-by", &[]).unwrap();
        OwnStory { signed: sign(&s, &k.ed_pub, &k.ed_priv, 1).unwrap(), story: s, shared_to: vec![] }
    }

    #[test]
    fn paragraphs_and_line_breaks_are_kept_and_text_is_escaped() {
        let m = story_text(&own("one\ntwo\n\n<b>three</b>", "public").story).into_string();
        assert!(m.contains("<p>one<br>two</p>"));
        assert!(m.contains("&lt;b&gt;three&lt;/b&gt;"));
    }

    #[test]
    fn the_owner_sees_how_to_share_and_export() {
        let s = own("hi", "public");
        let none = own_story(&s, Some(&StoryOwner { csrf: "c".into(), known: vec![] }), None).into_string();
        assert!(none.contains("Share your profile to a commons first"));
        let some = own_story(&s, Some(&StoryOwner { csrf: "c".into(), known: vec!["http://room:8081".into()] }), None)
            .into_string();
        assert!(some.contains("action=\"/story/share\""));
        assert!(some.contains("/story/0123456789abcdef/text"));
        let members = own_story(&own("hi", "members"), Some(&StoryOwner { csrf: "c".into(), known: vec![] }), None);
        assert!(members.into_string().contains("members only"));
        assert!(!own_story(&s, None, None).into_string().contains("/story/share"), "visitors get no tools");
        let gone = own_story(&s, None, Some(&StoryOutcome::Withdrawn("room:8081".into()))).into_string();
        assert!(gone.contains("Your story is gone from room:8081"));
    }
}
