//! A story shared to a commons "Only while I'm here" (decision record 0021),
//! as a reader sees it: the text fetched from its author's node just now,
//! or "Not here right now" with the signed card. The handler decides which
//! (`crate::relay::Reading`); this only shows it.

use maud::{html, Markup};

use super::parts::{ago, terms_panel, words};
use super::story::story_text;
use crate::here::HeldCard;
use crate::keys::keyid;
use crate::relay::Reading;

/// What a reader sees instead of the text, when there's no text to show.
fn no_text(card: &HeldCard, reading: &Reading) -> Markup {
    let who = &card.card.handle;
    let (label, message) = match reading {
        Reading::Changed => (
            "Changed since it was shared",
            format!("@{who} has changed this story since sharing it here. It will be back when they share it again."),
        ),
        Reading::Busy => ("Busy", "Lots of people are reading this right now. Try again in a moment.".to_string()),
        // Away (and Gone, which the handler turns into "not found").
        _ => ("Not here right now", format!("Not here right now: @{who} shares this only while they're online.")),
    };
    html! {
        p class="card-first" { (card.card.first_line) }
        div class="side-note" role="status" {
            p class="card-label" { (label) }
            p { (message) }
        }
    }
}

/// A story held here as a card. `reading` is what came back from its
/// author's node: the checked text, or why there's none.
pub fn here_story(held: &HeldCard, reading: &Reading, now: u64) -> Markup {
    let c = &held.card;
    let kid = keyid(held.author());
    let hash_start: String = c.text_sha256.chars().take(12).collect();
    html! {
        div class="story-layout" {
            article class="story-main" {
                p class="eyebrow" { "Story \u{b7} @" (c.handle) " \u{b7} only while they're here" }
                h1 class="story-title" { (c.title) }
                p class="story-meta" { "@" (c.handle) " \u{b7} " (words(c.words as usize)) " \u{b7} shared " (ago(held.received_at, now)) }
                @match reading {
                    Reading::Text(text) => (story_text(text)),
                    other => (no_text(held, other)),
                }
            }
            aside class="story-side" {
                (terms_panel(c.reach(), &c.license, &c.signals, false))
                div class="side-note" {
                    p class="card-label" { "Only while @" (c.handle) " is here" }
                    p { "This commons keeps only a signed card. The words come from @" (c.handle) "'s own computer each time someone reads them, and aren't stored here." }
                    @if let Reading::Text(_) = reading {
                        p { "Checked: the words match the SHA-256 @" (c.handle) " signed, " span class="mono" { (hash_start) "\u{2026}" } "." }
                    }
                }
                div class="side-actions" {
                    a class="btn-line" href={ "/federated/" (kid) } { "@" (c.handle) "'s profile" }
                    a class="btn-line" href="/stories" { "All stories" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::here::{card_for, sign_card};
    use crate::keys;
    use crate::story::new_story;

    /// @ada's card for a two-line story.
    fn held() -> HeldCard {
        let k = keys::generate();
        let s =
            new_story("0123456789abcdef", "ada", "Night bus", "The 38 sighs.\nA dog answers.", "commons", "cc-by", &[])
                .unwrap();
        let card = card_for(&s, "http://room:8081");
        HeldCard { signed: sign_card(&card, &k.ed_pub, &k.ed_priv, 1).unwrap(), card, received_at: 1 }
    }

    #[test]
    fn a_reader_sees_the_text_while_the_author_is_here() {
        let m = here_story(&held(), &Reading::Text("The 38 sighs.\nA dog answers.".into()), 10).into_string();
        assert!(m.contains("A dog answers.") && m.contains("Checked: the words match the SHA-256"));
        assert!(!m.contains("Not here right now"));
    }

    #[test]
    fn a_reader_sees_not_here_right_now_and_only_the_card() {
        let m = here_story(&held(), &Reading::Away, 10).into_string();
        assert!(m.contains("Not here right now: @ada shares this only while they're online."));
        assert!(m.contains("The 38 sighs.") && !m.contains("A dog answers."), "the first line only");
        assert!(!m.contains("Checked:"));
    }

    #[test]
    fn changed_text_is_explained_not_shown() {
        let m = here_story(&held(), &Reading::Changed, 10).into_string();
        assert!(m.contains("has changed this story since sharing it here"));
    }
}
