//! One story: on its author's node (with their sharing tools and what just
//! happened), and as shared to a commons. Two columns: the story, and a side
//! panel with where it is and its terms.

use maud::{html, Markup};

use super::parts::{ago, terms_panel, words};
use crate::keys::keyid;
use crate::model::Reach;
use crate::story::{HeldStory, OwnStory, Story};
use crate::text_links::{find_links, Piece};
use crate::web::layout::host_label;

/// What just happened to a story, confirmed at the top of its page.
pub enum StoryOutcome {
    /// Shared to this commons (its host).
    Shared(String),
    /// Withdrawn from this commons (its host).
    Withdrawn(String),
    /// A new version was saved (text or terms).
    Saved,
}

/// What the author sees beside their own story.
pub struct StoryOwner {
    /// The session's CSRF token, for the forms.
    pub csrf: String,
    /// The commons they can share to in one click (where their profile is shared).
    pub known: Vec<String>,
}

/// The confirmation block for an outcome.
fn outcome_block(o: &StoryOutcome, s: &OwnStory) -> Markup {
    match o {
        StoryOutcome::Shared(at) => html! {
            section class="flash flash-done" role="status" {
                p class="flash-label" { "Shared" }
                p { strong { "Your story is on " (at) "." } " People there can read it now, signed by you. You can withdraw it any time." }
            }
        },
        StoryOutcome::Withdrawn(at) => html! {
            section class="flash flash-withdrawn" role="status" {
                p class="flash-label" { "Withdrawn" }
                p { strong { "Your story is gone from " (at) "." } " That commons deleted its copy and won't take the same one back. Anyone who already read it, or copied it, keeps that. It's still here on your node, and you can share it again." }
                p { a class="btn-solid" href="/stories/new" { "Write a new story" } }
            }
        },
        StoryOutcome::Saved => html! {
            section class="flash flash-done" role="status" {
                p class="flash-label" { "Saved" }
                p {
                    strong { "Saved on this computer." }
                    @if !s.shared_to.is_empty() { " The copies already shared are the earlier version until you re-share." }
                }
            }
        },
    }
}

/// The rel of every link in a story: `nofollow ugc` tells search engines the
/// author of the page didn't vouch for it (it's user-generated content), and
/// `noopener` keeps the opened page from reaching back into this one.
const STORY_LINK_REL: &str = "nofollow ugc noopener";

/// One line of a story, with its web addresses and `[words](address)` links
/// made into links (see `crate::text_links`, which only lets `http` and
/// `https` through). Everything is escaped by maud. Links open in the same
/// tab, like every other link on the site.
fn story_line(line: &str) -> Markup {
    html! {
        @for piece in find_links(line) {
            @match piece {
                Piece::Text(words) => (words),
                Piece::Link { words, href } => a href=(href) rel=(STORY_LINK_REL) { (words) },
            }
        }
    }
}

/// A story's text: blank lines separate paragraphs, single line breaks stay,
/// and links are made clickable (see [`story_line`]). The same for every
/// view of a story: its author's node, and a commons.
fn story_text(s: &Story) -> Markup {
    html! {
        div class="story-text" {
            @for para in s.text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
                p {
                    @for (i, line) in para.lines().enumerate() {
                        @if i > 0 { br; }
                        (story_line(line))
                    }
                }
            }
        }
    }
}

/// A story on its author's node. With `owner`, the sharing tools and the
/// confirmation of what they just did (`done`).
pub fn own_story(s: &OwnStory, owner: Option<&StoryOwner>, done: Option<&StoryOutcome>, now: u64) -> Markup {
    let st = &s.story;
    html! {
        @if let Some(d) = done { (outcome_block(d, s)) }
        div class="story-layout" {
            article class="story-main" {
                p class="eyebrow" { "Your story" }
                h1 class="story-title" { (st.title) }
                p class="story-meta" { "@" (st.handle) " \u{b7} " (words(st.word_count())) " \u{b7} written " (ago(s.created_at(), now)) }
                // Your own story: edit it from the top, before reading on.
                @if owner.is_some() {
                    div class="story-tools" {
                        a class="btn-line" href={ "/story/" (st.id) "/edit" } { "Edit story" }
                        a class="btn-line" href={ "/story/" (st.id) "/text" } { "Export as a text file" }
                    }
                }
                (story_text(st))
            }
            aside class="story-side" {
                @if let Some(o) = owner { (owner_side(s, o, now)) }
                (terms_panel(st.reach(), &st.license, &st.signals, false))
                // An outline button, like "Edit story": the story screen has
                // no underlined actions, and plain text wouldn't look pressable.
                @if owner.is_some() {
                    div class="side-actions" {
                        a class="btn-line" href={ "/story/" (st.id) "/rights" } { "Change its terms" }
                    }
                }
            }
        }
    }
}

/// Where the story is shared, with Re-share and Withdraw; or where it can go.
fn owner_side(s: &OwnStory, o: &StoryOwner, now: u64) -> Markup {
    let id = &s.story.id;
    let unshared: Vec<&String> = o.known.iter().filter(|c| !s.shared_to.contains(c)).collect();
    html! {
        @for c in &s.shared_to {
            div class="share-card" {
                div class="card-bar" {}
                div class="card-body" {
                    p class="card-label" { "Shared to" }
                    p class="share-host" { (host_label(c)) }
                    @if s.changed_since_shared(c) {
                        p class="share-when is-stale" { "You've changed it since. Re-share to update the copy there." }
                    } @else if let Some(at) = s.shared_at.get(c) {
                        p class="share-when" { "Shared " (ago(*at, now)) " \u{b7} a signed copy" }
                    } @else {
                        p class="share-when" { "A signed copy" }
                    }
                    div class="share-actions" {
                        form method="post" action="/story/share" {
                            input type="hidden" name="id" value=(id);
                            input type="hidden" name="csrf" value=(o.csrf);
                            input type="hidden" name="commons_url" value=(c);
                            button class="btn-solid" type="submit" { "Re-share" }
                        }
                        form method="post" action="/story/withdraw" {
                            input type="hidden" name="id" value=(id);
                            input type="hidden" name="csrf" value=(o.csrf);
                            input type="hidden" name="commons_url" value=(c);
                            button class="btn-line" type="submit" { "Withdraw" }
                        }
                    }
                }
            }
        }
        @if s.story.reach() == Reach::Members {
            div class="side-note" {
                p { "This story is " strong { "members only" } ", so it stays on your node. Change its terms to share it." }
            }
        } @else if o.known.is_empty() {
            div class="side-note" {
                p { "Stories go to a commons where you're listed. Share your profile to a commons first (from your profile's Sharing tab), then come back here." }
            }
        } @else {
            @for c in unshared {
                div class="share-card is-new" {
                    div class="card-bar" {}
                    div class="card-body" {
                        p class="card-label" { "Not shared yet" }
                        p class="share-host" { (host_label(c)) }
                        form method="post" action="/story/share" {
                            input type="hidden" name="id" value=(id);
                            input type="hidden" name="csrf" value=(o.csrf);
                            input type="hidden" name="commons_url" value=(c);
                            button class="btn-solid" type="submit" { "Share" }
                        }
                    }
                }
            }
        }
        @if !s.shared_to.is_empty() {
            div class="side-note" {
                p class="card-label" { "What withdrawing does" }
                p { strong { "Removes the copy" } " from that commons, straight away. The story stays on this computer." }
                p class="muted" { strong { "Can't undo" } " anyone who already read or copied it: they keep what they have. Your licence and signals still say what they may do with it." }
            }
        }
    }
}

/// A story shared to this commons.
pub fn held_story(h: &HeldStory, now: u64) -> Markup {
    let st = &h.story;
    let kid = keyid(h.author());
    let key_start: String = h.author().chars().take(12).collect();
    html! {
        div class="story-layout" {
            article class="story-main" {
                p class="eyebrow" { "Story \u{b7} @" (st.handle) }
                h1 class="story-title" { (st.title) }
                p class="story-meta" { "@" (st.handle) " \u{b7} " (words(st.word_count())) " \u{b7} shared " (ago(h.received_at, now)) }
                (story_text(st))
            }
            aside class="story-side" {
                (terms_panel(st.reach(), &st.license, &st.signals, false))
                div class="side-note" {
                    p class="card-label" { "Verified" }
                    p { "Signed by @" (st.handle) "'s key " span class="mono" { (key_start) "\u{2026}" } " and checked when it arrived." }
                }
                div class="side-actions" {
                    a class="btn-line" href={ "/federated/" (kid) } { "@" (st.handle) "'s profile" }
                    a class="btn-line" href="/stories" { "All stories" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;
    use crate::story::{new_story, sign};

    /// One of @ada's stories with this text and reach, not shared anywhere.
    fn own(text: &str, visibility: &str) -> OwnStory {
        let k = keys::generate();
        let s = new_story("0123456789abcdef", "ada", "Night bus", text, visibility, "cc-by", &[]).unwrap();
        OwnStory {
            signed: sign(&s, &k.ed_pub, &k.ed_priv, 1).unwrap(),
            story: s,
            shared_to: vec![],
            shared_at: Default::default(),
        }
    }

    /// The author's view, offering these commons.
    fn owner(known: &[&str]) -> StoryOwner {
        StoryOwner { csrf: "c".into(), known: known.iter().map(|k| k.to_string()).collect() }
    }

    #[test]
    fn paragraphs_and_line_breaks_are_kept_and_text_is_escaped() {
        let m = story_text(&own("one\ntwo\n\n<b>three</b>", "public").story).into_string();
        assert!(m.contains("<p>one<br>two</p>"));
        assert!(m.contains("&lt;b&gt;three&lt;/b&gt;"));
    }

    #[test]
    fn links_in_a_story_become_safe_links_in_the_same_tab() {
        let m = story_line("At https://example.org/a?x=1&y=2, or [the map](https://example.org/map).").into_string();
        assert!(m.contains(
            r#"<a href="https://example.org/a?x=1&amp;y=2" rel="nofollow ugc noopener">https://example.org/a?x=1&amp;y=2</a>,"#
        ));
        assert!(m.contains(r#"<a href="https://example.org/map" rel="nofollow ugc noopener">the map</a>."#));
        assert!(!m.contains("target="), "same tab");
    }

    #[test]
    fn a_hostile_link_stays_escaped_text() {
        let m = story_line(r#"[x](javascript:alert(1)) https://e.org/"><script>alert(1)</script>"#).into_string();
        assert!(!m.contains("javascript:alert(1)\""), "no javascript: href");
        assert!(!m.contains("<script>"), "markup is escaped");
        assert!(m.contains(r#"<a href="https://e.org/" rel="nofollow ugc noopener">"#));
        assert!(m.contains("&quot;&gt;&lt;script&gt;"));
    }

    #[test]
    fn a_commons_shows_links_too_and_no_underlined_actions() {
        let s = own("Hear [it](https://example.org).", "commons");
        let held = HeldStory { story: s.story.clone(), signed: s.signed.clone(), received_at: 1 };
        let m = held_story(&held, 10).into_string();
        assert!(m.contains(r#"<a href="https://example.org" rel="nofollow ugc noopener">it</a>"#));
        assert!(!m.contains("link-btn"), "actions are buttons, not underlined links");
        let mine = own_story(&s, Some(&owner(&["http://room:8081"])), None, 10).into_string();
        assert!(!mine.contains("link-btn"));
        assert!(mine.contains(r#"class="btn-line" href="/story/0123456789abcdef/rights""#));
    }

    #[test]
    fn the_owner_sees_where_it_can_go_and_how_to_change_it() {
        let s = own("hi", "public");
        let none = own_story(&s, Some(&owner(&[])), None, 10).into_string();
        assert!(none.contains("Share your profile to a commons first"));
        let some = own_story(&s, Some(&owner(&["http://room:8081"])), None, 10).into_string();
        assert!(some.contains("action=\"/story/share\"") && some.contains("Not shared yet"));
        assert!(some.contains("/story/0123456789abcdef/text") && some.contains("/story/0123456789abcdef/edit"));
        assert!(some.contains("Change its terms"));
        let members = own_story(&own("hi", "members"), Some(&owner(&[])), None, 10).into_string();
        assert!(members.contains("members only"));
        assert!(!own_story(&s, None, None, 10).into_string().contains("/story/share"), "visitors get no tools");
    }

    #[test]
    fn a_shared_story_says_when_and_offers_withdraw() {
        let s = own("hi", "commons").with_shared_to("http://room:8081", 1);
        let m = own_story(&s, Some(&owner(&["http://room:8081"])), None, 121).into_string();
        assert!(m.contains("Shared 2 min ago") && m.contains("Withdraw") && m.contains("What withdrawing does"));
        let gone = own_story(&s, None, Some(&StoryOutcome::Withdrawn("room:8081".into())), 10).into_string();
        assert!(gone.contains("Your story is gone from room:8081"));
    }
}
