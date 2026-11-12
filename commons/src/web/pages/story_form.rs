//! Writing a story, in two steps: the words (new, or editing), then who can
//! read it and how it travels, ending in "Share to the room".

use maud::{html, Markup};

use super::parts::{page_head, section_label, terms_panel};
use super::rights::{how_it_travels, rights_fields, RightsFor};
use crate::story::{Story, PROMPT, TEXT_MAX, TITLE_MAX};
use crate::web::layout::host_label;

/// Step one: the title and the story. `editing` fills it from a story already
/// written, which keeps its terms.
pub fn write_form(me: &str, csrf: &str, editing: Option<&Story>) -> Markup {
    let (eyebrow, action, button, status) = match editing {
        None => (
            "New story \u{b7} workshop prompt".to_string(),
            "/stories/new".to_string(),
            "Next: choose who can see it",
            "Nothing leaves this computer until you share it",
        ),
        Some(s) => (
            format!("Edit story \u{b7} {}", s.title),
            format!("/story/{}/edit", s.id),
            "Save",
            "Saved on this computer",
        ),
    };
    html! {
        // A new story starts from the workshop prompt; an edit is just an edit.
        @if editing.is_some() {
            (page_head(&eyebrow, "Edit story", None))
        } @else {
            (page_head(&eyebrow, "The sound of your street", Some(PROMPT)))
        }
        form method="post" action=(action) class="stack-form write-form" {
            input type="hidden" name="handle" value=(me);
            input type="hidden" name="csrf" value=(csrf);
            (section_label("Your story", Some(status)))
            div class="field" {
                label class="field-label" for="title" { "Title" }
                input id="title" name="title" type="text" class="write-title" maxlength=(TITLE_MAX) required
                    value=(editing.map(|s| s.title.as_str()).unwrap_or(""));
            }
            div class="field" {
                label class="field-label" for="text" { "Story" }
                textarea id="text" name="text" class="write-text" rows="14" maxlength=(TEXT_MAX) required {
                    (editing.map(|s| s.text.as_str()).unwrap_or(""))
                }
                p class="hint" { "As long as you like, up to 12,000 characters." }
                // Links are found when the story is shown (crate::text_links);
                // what's saved and signed stays plain text.
                p class="hint" {
                    "To add a link, paste a web address (https://\u{2026}), or write "
                    code { "[some words](https://\u{2026})" }
                    " to make some words into a link."
                }
            }
            div class="actions" {
                button class="btn-action" type="submit" { (button) }
                @if let Some(s) = editing {
                    a class="link-btn" href={ "/story/" (s.id) } { "Cancel" }
                }
            }
        }
    }
}

/// Step two: who can read it, its licence and signals, how it travels; then
/// share it to `share_to` (the commons suggested for this member) or keep it.
pub fn rights_form(s: &Story, csrf: &str, share_to: Option<&str>) -> Markup {
    html! {
        (page_head(
            &format!("Story \u{b7} {}", s.title),
            "Who can read it, and how",
            Some("These choices are signed with your key and travel with the story wherever it goes."),
        ))
        form method="post" action={ "/story/" (s.id) "/rights" } class="stack-form rights-form" {
            input type="hidden" name="csrf" value=(csrf);
            (rights_fields(RightsFor::Story, s.reach(), &s.license, &s.signals))
            (how_it_travels())
            (section_label("What travels with every copy", None))
            (terms_panel(s.reach(), &s.license, &s.signals, true))
            div class="actions" {
                @if let Some(c) = share_to {
                    input type="hidden" name="commons_url" value=(c);
                    button class="btn-action" type="submit" name="action" value="share" { "Share to the room" }
                    button class="link-btn" type="submit" name="action" value="keep" { "Keep it on this computer for now" }
                    span class="hint" { "Sharing to " (host_label(c)) }
                } @else {
                    button class="btn-action" type="submit" name="action" value="keep" { "Save" }
                    span class="hint" { "To share it, first share your profile to a commons where you'll be listed." }
                }
            }
        }
        script src="/rights.js" {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::new_story;

    /// A story listed in the commons, for these tests.
    fn story() -> Story {
        new_story("0123456789abcdef", "ada", "Night bus", "Words.", "commons", "cc-by", &["credit".into()]).unwrap()
    }

    #[test]
    fn writing_comes_first_and_editing_is_prefilled() {
        let new = write_form("ada", "c", None).into_string();
        assert!(new.contains("Listen to your street") && new.contains("Next: choose who can see it"));
        assert!(!new.contains("name=\"visibility\""), "rights come in step two");
        assert!(new.contains("To add a link") && new.contains("[some words](https://"), "how to add a link");
        let edit = write_form("ada", "c", Some(&story())).into_string();
        assert!(edit.contains("value=\"Night bus\"") && edit.contains("action=\"/story/0123456789abcdef/edit\""));
    }

    #[test]
    fn the_rights_step_ends_in_sharing_to_the_room() {
        let m = rights_form(&story(), "c", Some("http://room:8081")).into_string();
        assert!(m.contains("Share to the room") && m.contains("Keep it on this computer for now"));
        assert!(m.contains("Sharing to room:8081") && m.contains("How it travels"));
        assert!(m.contains("value=\"commons\" data-label=\"Listed in the commons\" checked"));
        let alone = rights_form(&story(), "c", None).into_string();
        assert!(!alone.contains("Share to the room") && alone.contains("first share your profile"));
    }
}
