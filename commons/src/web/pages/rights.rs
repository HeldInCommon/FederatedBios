//! "Sharing and rights": who can see something, its licence, its signals and
//! its Held in Common Signals, as numbered panels with each choice explained
//! (decision records 0022 and 0025). Used by the profile form and by a
//! story's rights step, which adds a fifth: how it travels.
//!
//! Every choice carries a `data-label`, which `/rights.js` copies into the
//! "What travels with every copy" summary as the reader changes their mind.
//! Without JavaScript the form works the same; the summary shows what was
//! chosen when the page loaded.

use maud::{html, Markup};

use super::parts::{burst, Chosen, Tone};
use crate::governance::{license_terms, LICENSES, SIGNALS};
use crate::held_signals::{HeldSignals, OWN_TERMS_LABEL, OWN_TERMS_MAX, WITHDRAW_LABEL};
use crate::model::Reach;

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

/// The four numbered panels, filled in with what is `chosen`: who can see
/// it, licence, signals, and Held in Common Signals.
pub fn rights_fields(kind: RightsFor, chosen: &Chosen) -> Markup {
    let (reach, license, signals) = (chosen.reach, chosen.license, chosen.signals);
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
        (held_signals_step(chosen.held_signals))
    }
}

/// Step four, Held in Common Signals (decision record 0025): the right to
/// withdraw even after a model is trained, and the person's own terms for
/// AI in their own words. Both are signed and travel with every copy.
fn held_signals_step(held: &HeldSignals) -> Markup {
    html! {
        fieldset class="rights-step" {
            legend { (burst(4, Tone::Violet)) span { "Held in Common Signals: your terms for AI, in your words" } }
            p class="hint" { "Two more things you can say about AI, beyond the CC signals. Both are signed with your key and travel with every copy." }
            div class="choice-panel" {
                label class="choice" {
                    span class="choice-head" {
                        input type="checkbox" name="withdraw_after_training" value="yes" data-label=(WITHDRAW_LABEL) checked[held.withdraw_after_training];
                        span class="choice-title" { (WITHDRAW_LABEL) }
                    }
                    span class="choice-body" { "If an AI model has already been trained on this, you keep the right to ask for it to be taken out of the model, or for the model to stop using it." }
                }
            }
            div class="field" {
                label class="field-label" for="own_terms" { "Your terms for AI, in your own words" }
                textarea id="own_terms" name="own_terms" maxlength=(OWN_TERMS_MAX) rows="4" data-label=(OWN_TERMS_LABEL) aria-describedby="own_terms_hint"
                    placeholder="For example: you may learn from this for open research, but not to imitate my voice." { (held.own_terms) }
                p class="hint" id="own_terms_hint" { "Say what AI systems may and may not do with this, and how you relate to them. Plain text, up to 1,000 characters. Leave it empty if the choices above say enough." }
            }
        }
    }
}

/// The fifth step for a story: does the commons keep a copy? Only the first
/// way exists today; the second is proposed (decision record 0021).
pub fn how_it_travels() -> Markup {
    html! {
        fieldset class="rights-step" {
            legend { (burst(5, Tone::Ink)) span { "How it travels" } }
            p class="hint" { "Does the commons keep a copy of your story, or only show it while you're here?" }
            div class="choice-panel" {
                label class="choice" {
                    span class="choice-head" {
                        input type="radio" name="travel" value="copy" checked;
                        span class="choice-title" { "Leave a copy in the commons" }
                    }
                    span class="choice-body" { "A signed copy stays on the commons, so it's still readable when you leave. Withdraw it any time and the copy is deleted." }
                    span class="choice-facts" { "Commons keeps \u{b7} a signed copy\nReadable when you leave \u{b7} yes" }
                }
                label class="choice is-soon" {
                    span class="choice-head" {
                        input type="radio" name="travel" value="here" disabled;
                        span class="choice-title" { "Only while I'm here" }
                        span class="soon-tag" { "Coming soon" }
                    }
                    span class="choice-body" { "The commons lists it, but reads it from your laptop each time and never stores it. Close your laptop and it's gone from the room." }
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

    /// Terms with these choices, for these tests.
    fn chosen<'a>(reach: Reach, license: &'a str, signals: &'a [String], held: &'a HeldSignals) -> Chosen<'a> {
        Chosen { reach, license, signals, held_signals: held }
    }

    #[test]
    fn the_current_choices_are_selected_and_labelled() {
        let (open, none) = (["open".to_string()], HeldSignals::default());
        let m = rights_fields(RightsFor::Profile, &chosen(Reach::Commons, "cc-by-nc", &open, &none)).into_string();
        assert!(m.contains("value=\"commons\" data-label=\"Listed in the commons\" checked"));
        assert!(m.contains("value=\"cc-by-nc\" data-label=\"CC BY-NC 4.0\" checked"));
        assert!(m.contains("value=\"open\" data-label=\"Open\" checked"));
        assert!(m.contains("Commercial use"), "profiles explain licences in full");
        assert!(!m.contains("value=\"yes\" data-label=\"Withdraw even after training\" checked"));
    }

    #[test]
    fn held_in_common_signals_are_step_four_and_prefilled() {
        let held = HeldSignals { withdraw_after_training: true, own_terms: "<i>Mine</i>".into() };
        let m = rights_fields(RightsFor::Story, &chosen(Reach::Public, "cc-by", &[], &held)).into_string();
        let step3 = m.find("Signals: how AI models may learn from it").unwrap();
        let step4 = m.find("Held in Common Signals").unwrap();
        assert!(step3 < step4, "after the CC signals");
        assert!(m.contains(">4</text>"));
        assert!(m.contains("value=\"yes\" data-label=\"Withdraw even after training\" checked"));
        assert!(m.contains("label class=\"field-label\" for=\"own_terms\""), "the text box has a visible label");
        assert!(m.contains("maxlength=\"1000\""));
        assert!(m.contains("&lt;i&gt;Mine&lt;/i&gt;</textarea>"), "own terms are escaped, never HTML");
        assert!(how_it_travels().into_string().contains(">5</text>"), "How it travels is now step five");
    }

    #[test]
    fn stories_never_claim_encryption() {
        let none = HeldSignals::default();
        let m = rights_fields(RightsFor::Story, &chosen(Reach::Commons, "cc-by", &[], &none)).into_string();
        assert!(!m.contains("Encrypted"), "members-only stories aren't encrypted (decision record 0020)");
        assert!(m.contains("license-grid compact") && m.contains("Any use, with credit"));
    }

    #[test]
    fn only_while_im_here_is_not_offered_yet() {
        let m = how_it_travels().into_string();
        assert!(m.contains("value=\"here\" disabled") && m.contains("Coming soon"));
    }
}
