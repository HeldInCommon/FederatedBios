//! "Sharing and rights": who can see something, its licence and its signals,
//! as numbered panels with each choice explained (decision record 0022).
//! Used by the profile form and by a story's rights step.
//!
//! Every choice carries a `data-label`, which `/rights.js` copies into the
//! "What travels with every copy" summary as the reader changes their mind.
//! Without JavaScript the form works the same; the summary shows what was
//! chosen when the page loaded.

use maud::{html, Markup};

use super::parts::{burst, Tone};
use crate::governance::{license_terms, LICENSES, SIGNALS};
use crate::model::Reach;
use crate::story::Travel;

/// Whose rights these are: the wording differs a little.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RightsFor {
    /// A member's profile: licences explained in full.
    Profile,
    /// One story: licences in brief (the profile form explains them in full).
    Story,
}

/// What each reach means, for this kind of thing.
fn reach_choices(kind: RightsFor) -> [(Reach, &'static str, &'static str, &'static str); 3] {
    match kind {
        RightsFor::Profile => [
            (
                Reach::Public,
                "Public",
                "Listed in the commons directory and open to search engines.",
                "Directory \u{b7} yes\nSearch engines \u{b7} yes",
            ),
            (
                Reach::Commons,
                "Listed in the commons",
                "In the directory; anyone with the link can read it, kept out of search.",
                "Directory \u{b7} yes\nSearch engines \u{b7} asked not to",
            ),
            (
                Reach::Members,
                "Members only",
                "Encrypted to the member set. Only signed-in members of this node can read it.",
                "Leaves this computer \u{b7} never",
            ),
        ],
        RightsFor::Story => [
            (Reach::Public, "Public", "Listed in the commons and open to search engines.", "Search engines \u{b7} yes"),
            (
                Reach::Commons,
                "Listed in the commons",
                "The room can read it; kept out of search. A good default for a workshop.",
                "Search engines \u{b7} asked not to",
            ),
            (
                Reach::Members,
                "Members only",
                "For members of this node only. Never leaves this computer.",
                "Leaves this computer \u{b7} never",
            ),
        ],
    }
}

/// The three numbered panels: who can see it, licence, signals.
pub fn rights_fields(kind: RightsFor, reach: Reach, license: &str, signals: &[String]) -> Markup {
    html! {
        fieldset class="rights-step" {
            legend { (burst(1, Tone::Pink)) span { "Who can see it" } }
            div class="choice-panel" {
                @for (value, title, body, facts) in reach_choices(kind) {
                    label class="choice" {
                        span class="choice-head" {
                            input type="radio" name="visibility" value=(value.as_str()) data-label=(title) checked[value == reach];
                            span class="choice-title" { (title) }
                        }
                        span class="choice-body" { (body) }
                        span class="choice-facts" { (facts) }
                    }
                }
            }
        }
        fieldset class="rights-step" {
            legend { (burst(2, Tone::Blue)) span { "Licence: how people may reuse it" } }
            p class="hint" { "A Creative Commons licence tells people what they may do with your work without asking you first." }
            div class={ "license-grid" @if kind == RightsFor::Story { " compact" } } {
                @for (key, name, _) in LICENSES {
                    @let t = license_terms(key);
                    label class="license-card" {
                        span class="license-head" {
                            input type="radio" name="license" value=(key) data-label=(name) checked[*key == license];
                            span class="license-name" { (name) }
                        }
                        @if kind == RightsFor::Story {
                            span class="license-short" { (t.short) }
                        } @else {
                            span class="license-summary" { (t.summary) }
                            span class="license-facts" {
                                span { "Credit \u{b7} " (t.credit) }
                                span { "Adapt \u{b7} " (t.adapt) }
                                span class={ "license-commercial " (if t.commercial { "is-yes" } else { "is-no" }) } {
                                    "Commercial use \u{b7} " (if t.commercial { "yes" } else { "no" })
                                }
                            }
                        }
                    }
                }
            }
            @if kind == RightsFor::Story {
                p class="hint" { "Each licence is explained in full on your profile's Sharing and rights." }
            }
        }
        fieldset class="rights-step" {
            legend { (burst(3, Tone::Jade)) span { "Signals: how AI models may learn from it" } }
            p class="hint" { "CC signals are machine-readable preferences for datasets and model training. Choose any that apply." }
            div class="choice-panel" {
                @for (key, name, desc) in SIGNALS {
                    label class="choice" {
                        span class="choice-head" {
                            input type="checkbox" name="signals" value=(key) data-label=(name) checked[signals.iter().any(|s| s == key)];
                            span class="choice-title" { (name) }
                        }
                        span class="choice-body" { (desc) }
                    }
                }
            }
        }
    }
}

/// The fourth step for a story: does the commons keep a copy, or only a card
/// and the words while you're online (decision record 0021)? `travel` is the
/// story's current choice.
pub fn how_it_travels(travel: Travel) -> Markup {
    html! {
        fieldset class="rights-step" {
            legend { (burst(4, Tone::Violet)) span { "How it travels" } }
            p class="hint" { "Does the commons keep a copy of your story, or only show it while you're here?" }
            div class="choice-panel" {
                label class="choice" {
                    span class="choice-head" {
                        input type="radio" name="travel" value=(Travel::Copy.as_str()) data-label="Keep a copy on the commons" checked[travel == Travel::Copy];
                        span class="choice-title" { "Keep a copy on the commons" }
                    }
                    span class="choice-body" { "A signed copy stays on the commons, so it's still readable when you leave. Withdraw it any time and the copy is deleted." }
                    span class="choice-facts" { "Commons keeps \u{b7} a signed copy\nReadable when you leave \u{b7} yes" }
                }
                label class="choice" {
                    span class="choice-head" {
                        input type="radio" name="travel" value=(Travel::Here.as_str()) data-label="Only while I'm here" checked[travel == Travel::Here];
                        span class="choice-title" { "Only while I'm here" }
                    }
                    span class="choice-body" { "The commons lists a signed card, and fetches the words from this computer each time someone reads them. It never stores them. Close your laptop and the room sees \u{201c}Not here right now\u{201d}." }
                    span class="choice-facts" { "Commons keeps \u{b7} a signed card only\nReadable when you leave \u{b7} no" }
                }
            }
            p class="hint" { "Either way, anyone who reads it could copy the words. The licence and signals are how you say what they may do with them." }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_choices_are_selected_and_labelled() {
        let m = rights_fields(RightsFor::Profile, Reach::Commons, "cc-by-nc", &["open".into()]).into_string();
        assert!(m.contains("value=\"commons\" data-label=\"Listed in the commons\" checked"));
        assert!(m.contains("value=\"cc-by-nc\" data-label=\"CC BY-NC 4.0\" checked"));
        assert!(m.contains("value=\"open\" data-label=\"Open\" checked"));
        assert!(m.contains("Commercial use"), "profiles explain licences in full");
    }

    #[test]
    fn stories_never_claim_encryption() {
        let m = rights_fields(RightsFor::Story, Reach::Commons, "cc-by", &[]).into_string();
        assert!(!m.contains("Encrypted"), "members-only stories aren't encrypted (decision record 0020)");
        assert!(m.contains("license-grid compact") && m.contains("Any use, with credit"));
    }

    #[test]
    fn both_ways_to_travel_are_offered_and_the_current_one_is_chosen() {
        let copy = how_it_travels(Travel::Copy).into_string();
        assert!(copy.contains(r#"value="copy" data-label="Keep a copy on the commons" checked"#));
        assert!(!copy.contains("disabled") && !copy.contains("Coming soon"));
        let here = how_it_travels(Travel::Here).into_string();
        assert!(here.contains(r#"value="here" data-label="Only while I'm here" checked"#));
    }
}
