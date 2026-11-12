//! Small pieces of markup shared by several pages, drawn from the
//! heldincommon.org look: a page heading with its pink eyebrow, a section
//! label followed by a rule, the numbered starburst, the terms summary
//! panel, and "2 min ago".
//!
//! Pure: data in, markup (or text) out.

use maud::{html, Markup};

use crate::governance::{license_name, signal_name};
use crate::held_signals::HeldSignals;
use crate::model::Reach;
use crate::story::Story;

/// A page heading: a small pink label above the title, and an optional
/// serif line under it. (The title's pink full stop comes from the CSS.)
pub fn page_head(eyebrow: &str, title: &str, lead: Option<&str>) -> Markup {
    html! {
        header class="intro" {
            p class="eyebrow" { (eyebrow) }
            h1 { (title) }
            @if let Some(l) = lead { p class="lede" { (l) } }
        }
    }
}

/// A section label followed by a rule across the page, with optional text
/// at its end (a count, a status).
pub fn section_label(text: &str, aside: Option<&str>) -> Markup {
    html! {
        div class="section-label" {
            span { (text) }
            span class="rule-fill" {}
            @if let Some(a) = aside { span class="section-aside" { (a) } }
        }
    }
}

/// The colours a numbered starburst can take.
#[derive(Clone, Copy)]
pub enum Tone {
    /// heldincommon.org's pink: the first step.
    Pink,
    /// Blue: the second.
    Blue,
    /// Jade: the third.
    Jade,
    /// Violet: the fourth.
    Violet,
    /// Ink: the fifth.
    Ink,
}

impl Tone {
    /// The CSS class that sets this colour (`--tone`).
    fn class(self) -> &'static str {
        match self {
            Tone::Pink => "tone-pink",
            Tone::Blue => "tone-blue",
            Tone::Jade => "tone-jade",
            Tone::Violet => "tone-violet",
            Tone::Ink => "tone-ink",
        }
    }
}

/// The numbered starburst heldincommon.org uses for steps. Decoration: the
/// step's words carry the meaning.
pub fn burst(n: u8, tone: Tone) -> Markup {
    html! {
        svg class={ "burst " (tone.class()) } width="40" height="40" viewBox="0 0 100 100" aria-hidden="true" {
            path d="M50 2l9 13 15-6 2 16 16 3-9 13 9 13-16 3-2 16-15-6-9 13-9-13-15 6-2-16-16-3 9-13-9-13 16-3 2-16 15 6z" {}
            text x="50" y="63" text-anchor="middle" { (n) }
        }
    }
}

/// How a reach is named on cards and summaries.
pub fn reach_label(r: Reach) -> &'static str {
    match r {
        Reach::Public => "Public",
        Reach::Commons => "Listed in the commons",
        Reach::Members => "Members only",
    }
}

/// The signals as one line ("Credit, Open"), or "None".
pub fn signals_line(signals: &[String]) -> String {
    if signals.is_empty() {
        "None".into()
    } else {
        signals.iter().map(|s| signal_name(s)).collect::<Vec<_>>().join(", ")
    }
}

/// The terms chosen for a profile version or a story: what the Sharing and
/// rights panels show as chosen, and what travels with every copy.
pub struct Chosen<'a> {
    /// Who can see it.
    pub reach: Reach,
    /// The licence key (see `governance::LICENSES`).
    pub license: &'a str,
    /// The CC signal keys.
    pub signals: &'a [String],
    /// Held in Common Signals (decision record 0025).
    pub held_signals: &'a HeldSignals,
}

impl<'a> Chosen<'a> {
    /// The terms a story carries.
    ///
    /// Rust note: `'a` says the borrowed fields live as long as the story
    /// they point into, so `Chosen` can't outlive it.
    pub fn of_story(s: &'a Story) -> Chosen<'a> {
        Chosen { reach: s.reach(), license: &s.license, signals: &s.signals, held_signals: &s.held_signals }
    }
}

/// The terms that travel with every copy, as one panel of cells: who can see
/// it, licence, signals and Held in Common Signals. `live` marks the cells
/// the rights form's script keeps up to date.
pub fn terms_panel(chosen: &Chosen, live: bool) -> Markup {
    let cell = |label: &str, value: String, key: &str| {
        html! {
            div class="terms-cell" {
                p class="terms-label" { (label) }
                @if live { p class="terms-value" data-summary=(key) { (value) } }
                @else { p class="terms-value" { (value) } }
            }
        }
    };
    html! {
        div class="terms-panel" {
            (cell("Who can see it", reach_label(chosen.reach).to_string(), "visibility"))
            (cell("Licence", license_name(chosen.license).to_string(), "license"))
            (cell("Signals", signals_line(chosen.signals), "signals"))
            (cell("Held in Common Signals", chosen.held_signals.summary(), "held"))
        }
    }
}

/// Someone's own terms for AI, in their words, under a label naming whose
/// they are; nothing when they wrote none. Plain text: maud escapes it, and
/// line breaks are kept as `<br>`.
pub fn own_terms_note(handle: &str, held: &HeldSignals) -> Markup {
    html! {
        @if !held.own_terms.is_empty() {
            div class="side-note" {
                p class="card-label" { "Terms for AI, in @" (handle) "'s own words" }
                p {
                    @for (i, line) in held.own_terms.lines().enumerate() {
                        @if i > 0 { br; }
                        (line)
                    }
                }
            }
        }
    }
}

/// How long ago `then` was, at `now`, in words: "just now", "2 min ago",
/// "3 hours ago", "yesterday", "4 days ago".
pub fn ago(then: u64, now: u64) -> String {
    let secs = now.saturating_sub(then);
    match secs {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", secs / 60),
        3_600..=7_199 => "an hour ago".into(),
        7_200..=86_399 => format!("{} hours ago", secs / 3_600),
        86_400..=172_799 => "yesterday".into(),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// "1 word", "48 words".
pub fn words(n: usize) -> String {
    if n == 1 {
        "1 word".into()
    } else {
        format!("{n} words")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_as_words() {
        assert_eq!(ago(100, 130), "just now");
        assert_eq!(ago(0, 125), "2 min ago");
        assert_eq!(ago(0, 3_700), "an hour ago");
        assert_eq!(ago(0, 3 * 3_600), "3 hours ago");
        assert_eq!(ago(0, 90_000), "yesterday");
        assert_eq!(ago(0, 4 * 86_400), "4 days ago");
        assert_eq!(ago(500, 100), "just now", "a clock that runs behind isn't the future");
    }

    #[test]
    fn terms_read_plainly() {
        assert_eq!(signals_line(&[]), "None");
        assert_eq!(signals_line(&["credit".into(), "open".into()]), "Credit, Open");
        let none = HeldSignals::default();
        let credit = ["credit".to_string()];
        let chosen = Chosen { reach: Reach::Commons, license: "cc-by-nc", signals: &credit, held_signals: &none };
        let m = terms_panel(&chosen, true).into_string();
        assert!(m.contains("Listed in the commons") && m.contains("CC BY-NC 4.0") && m.contains("data-summary"));
        assert!(m.contains("Held in Common Signals") && m.contains("data-summary=\"held\">None<"));
        let plain = Chosen { reach: Reach::Public, license: "cc0", signals: &[], held_signals: &none };
        assert!(!terms_panel(&plain, false).into_string().contains("data-summary"));
        assert_eq!(words(1), "1 word");
    }

    #[test]
    fn held_signals_travel_in_the_summary_and_own_terms_are_plain_text() {
        let held = HeldSignals { withdraw_after_training: true, own_terms: "<b>Ask</b>\nfirst".into() };
        let chosen = Chosen { reach: Reach::Public, license: "cc-by", signals: &[], held_signals: &held };
        assert!(terms_panel(&chosen, false).into_string().contains("Withdraw even after training, Own terms"));
        let note = own_terms_note("ada", &held).into_string();
        assert!(note.contains("in @ada's own words") && note.contains("&lt;b&gt;Ask&lt;/b&gt;<br>first"));
        assert!(own_terms_note("ada", &HeldSignals::default()).into_string().is_empty(), "nothing written");
    }

    #[test]
    fn headings_and_marks() {
        let h = page_head("Your story", "Bins at six", Some("A lead")).into_string();
        assert!(h.contains("class=\"eyebrow\"") && h.contains("<h1>Bins at six</h1>"));
        let b = burst(2, Tone::Blue).into_string();
        assert!(b.contains("aria-hidden=\"true\"") && b.contains(">2</text>"));
    }
}
