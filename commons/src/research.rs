//! Sharing your terms for research (decision record 0029): a person may
//! send a copy of the terms they set for a story to the Held in Common
//! project, so we can learn how people set terms for their work.
//!
//! - **Opt in, every time.** A checkbox under "Your terms for AI, in your own
//!   words" on a story's rights step. It's never ticked when the page loads
//!   and never remembered, so each copy sent is one the person chose to send.
//! - **Only the terms, never the person's data.** What travels is a
//!   [`TermsGift`]: the own terms, "withdraw even after training", who can
//!   see it, the licence, the CC signals, how it travels, and how the person
//!   describes themselves. No handle, key, name, title, story text or story
//!   id. The commons refuses any other field (`deny_unknown_fields`), so a
//!   future change can't quietly send more.
//! - **Where it goes.** The node sends it to `HIC_RESEARCH_URL`, or, when
//!   that isn't set, to its default online commons (`HIC_SHARE_DEFAULT`):
//!   `POST <address>/research/terms`. The commons keeps each one as a file
//!   in `terms/` in its data folder (see `Store::save_terms_gift`).
//!
//! Everything here is pure.

use serde::{Deserialize, Serialize};

use crate::governance::{LICENSES, SIGNALS};
use crate::held_signals::{HeldSignals, OWN_TERMS_MAX};
use crate::model::Reach;
use crate::story::Travel;

/// The path on a commons that takes a [`TermsGift`].
pub const PATH: &str = "/research/terms";

/// The format name inside every gift, so the files can be read later even
/// if the format changes.
pub const FORMAT: &str = "hic-terms-research/1";

/// Longest self-description, in characters.
pub const ABOUT_YOU_MAX: usize = 300;

/// The plain message shown when the self-description is too long.
pub const ABOUT_YOU_TOO_LONG: &str = "Keep how you describe yourself to 300 characters or fewer.";

/// What a person agreed to send: their terms for one story and how they
/// describe themselves. Nothing that says who they are or what they wrote.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TermsGift {
    /// Always [`FORMAT`].
    pub format: String,
    /// How they describe themselves, in terms of how they might use Held in
    /// Common ("an artist sharing work with a collaborator"). May be empty.
    pub about_you: String,
    /// Their own terms for AI, in their own words. May be empty.
    pub own_terms: String,
    /// True when they keep the right to withdraw even after training.
    pub withdraw_after_training: bool,
    /// Who can see the story: `public`, `commons` or `members`.
    pub visibility: String,
    /// The licence key (see `governance::LICENSES`).
    pub license: String,
    /// The CC signal keys (see `governance::SIGNALS`).
    pub signals: Vec<String>,
    /// How the story travels: `copy` or `here`.
    pub travel: String,
}

/// A gift as the commons keeps it: the gift, and when it arrived (Unix
/// seconds). Nothing about the sender, not even their address.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ReceivedGift {
    /// When the commons received it.
    pub received_at: u64,
    /// What was sent.
    #[serde(flatten)]
    pub gift: TermsGift,
}

/// Tidy a self-description the way own terms are tidied: trimmed, Windows
/// line endings made `\n`.
fn tidy(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

/// The gift for a story's terms, from the rights form's answers: `agreed`
/// is the checkbox (ticked sends `yes`), `about_you` the self-description.
/// `Ok(None)` when they didn't agree, so nothing is sent; an error message
/// when the self-description is too long. Pure.
pub fn gift_from_form(
    agreed: &str,
    about_you: &str,
    held: &HeldSignals,
    reach: Reach,
    license: &str,
    signals: &[String],
    travel: Travel,
) -> Result<Option<TermsGift>, &'static str> {
    if agreed != "yes" {
        return Ok(None);
    }
    let about_you = tidy(about_you);
    if about_you.chars().count() > ABOUT_YOU_MAX {
        return Err(ABOUT_YOU_TOO_LONG);
    }
    Ok(Some(TermsGift {
        format: FORMAT.into(),
        about_you,
        own_terms: held.own_terms.clone(),
        withdraw_after_training: held.withdraw_after_training,
        visibility: reach.as_str().into(),
        license: license.into(),
        signals: signals.to_vec(),
        travel: travel.as_str().into(),
    }))
}

/// Why a commons refuses a gift.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The format isn't [`FORMAT`].
    UnknownFormat,
    /// The self-description or own terms are longer than allowed.
    TooLong,
    /// A choice isn't one Held in Common offers (`visibility`, `license`,
    /// a signal or `travel`).
    UnknownChoice,
}

impl Refusal {
    /// What the commons says back, in plain words.
    pub fn message(&self) -> &'static str {
        match self {
            Refusal::UnknownFormat => "unknown format",
            Refusal::TooLong => "too long",
            Refusal::UnknownChoice => "a choice Held in Common doesn't offer",
        }
    }
}

/// Check a gift a commons received: the format it knows, lengths within the
/// form's limits, and only choices the form offers. Pure.
pub fn check(gift: &TermsGift) -> Result<(), Refusal> {
    if gift.format != FORMAT {
        return Err(Refusal::UnknownFormat);
    }
    if gift.about_you.chars().count() > ABOUT_YOU_MAX || gift.own_terms.chars().count() > OWN_TERMS_MAX {
        return Err(Refusal::TooLong);
    }
    let visibility_ok = Reach::parse(&gift.visibility).is_some();
    let license_ok = LICENSES.iter().any(|(key, _, _)| *key == gift.license);
    let signals_ok =
        gift.signals.len() <= SIGNALS.len() && gift.signals.iter().all(|s| SIGNALS.iter().any(|(key, _, _)| key == s));
    let travel_ok = [Travel::Copy.as_str(), Travel::Here.as_str()].contains(&gift.travel.as_str());
    if visibility_ok && license_ok && signals_ok && travel_ok {
        Ok(())
    } else {
        Err(Refusal::UnknownChoice)
    }
}

/// Where a node sends gifts: `HIC_RESEARCH_URL` when set, otherwise its
/// default online commons. `None` when neither is set: the option isn't
/// offered. Pure.
pub fn destination<'a>(research_url: &'a str, share_default: &'a str) -> Option<&'a str> {
    [research_url, share_default].into_iter().find(|u| u.starts_with("http://") || u.starts_with("https://"))
}

/// What the rights page calls where gifts go: the default online commons'
/// name and "node" ("Held in Common Collective Commons node"), or "Held in
/// Common project" when it has no name. Never an address. Pure.
pub fn destination_name(share_default_name: &str) -> String {
    let name = share_default_name.trim();
    if name.is_empty() {
        "Held in Common project".into()
    } else {
        format!("{name} node")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A gift with ordinary choices, for these tests.
    fn gift() -> TermsGift {
        let held = HeldSignals { withdraw_after_training: true, own_terms: "Open research only.".into() };
        gift_from_form(
            "yes",
            "  An artist sharing work with a collaborator \r\n",
            &held,
            Reach::Commons,
            "cc-by",
            &["credit".into()],
            Travel::Here,
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn nothing_is_sent_unless_they_agree() {
        let held = HeldSignals::default();
        for agreed in ["", "no", "on"] {
            let none = gift_from_form(agreed, "an artist", &held, Reach::Public, "cc-by", &[], Travel::Copy);
            assert_eq!(none, Ok(None), "{agreed:?} isn't agreeing");
        }
    }

    #[test]
    fn a_gift_carries_the_terms_and_choices_tidied() {
        let g = gift();
        assert_eq!(g.format, FORMAT);
        assert_eq!(g.about_you, "An artist sharing work with a collaborator");
        assert_eq!(g.own_terms, "Open research only.");
        assert!(g.withdraw_after_training);
        assert_eq!((g.visibility.as_str(), g.license.as_str(), g.travel.as_str()), ("commons", "cc-by", "here"));
        assert_eq!(g.signals, vec!["credit".to_string()]);
        assert_eq!(check(&g), Ok(()));
    }

    #[test]
    fn a_gift_never_carries_who_they_are_or_what_they_wrote() {
        let json = serde_json::to_value(gift()).unwrap();
        let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        for private in ["handle", "key", "author", "title", "text", "id", "name", "email"] {
            assert!(!keys.iter().any(|k| k.as_str() == private), "no {private}");
        }
        assert_eq!(keys.len(), 8, "exactly the fields listed in the decision record");
    }

    #[test]
    fn a_commons_refuses_a_gift_with_extra_fields() {
        let mut json = serde_json::to_value(gift()).unwrap();
        json["handle"] = "ada".into();
        assert!(serde_json::from_value::<TermsGift>(json).is_err(), "a handle can't sneak in");
    }

    #[test]
    fn a_too_long_self_description_is_refused_not_cut() {
        let long = "a".repeat(ABOUT_YOU_MAX + 1);
        let held = HeldSignals::default();
        let refused = gift_from_form("yes", &long, &held, Reach::Public, "cc-by", &[], Travel::Copy);
        assert_eq!(refused, Err(ABOUT_YOU_TOO_LONG));
        assert_eq!(check(&TermsGift { about_you: long, ..gift() }), Err(Refusal::TooLong));
    }

    #[test]
    fn a_commons_refuses_choices_the_form_does_not_offer() {
        assert_eq!(check(&TermsGift { format: "other/1".into(), ..gift() }), Err(Refusal::UnknownFormat));
        assert_eq!(check(&TermsGift { license: "all-rights".into(), ..gift() }), Err(Refusal::UnknownChoice));
        assert_eq!(check(&TermsGift { visibility: "secret".into(), ..gift() }), Err(Refusal::UnknownChoice));
        assert_eq!(check(&TermsGift { signals: vec!["<script>".into()], ..gift() }), Err(Refusal::UnknownChoice));
        assert_eq!(check(&TermsGift { travel: "post".into(), ..gift() }), Err(Refusal::UnknownChoice));
    }

    #[test]
    fn the_page_names_where_gifts_go_never_its_address() {
        assert_eq!(destination_name("Held in Common Collective Commons"), "Held in Common Collective Commons node");
        assert_eq!(destination_name(" "), "Held in Common project");
    }

    #[test]
    fn gifts_go_to_the_research_address_or_else_the_online_commons() {
        assert_eq!(destination("https://r.example", "https://c.example"), Some("https://r.example"));
        assert_eq!(destination("", "https://c.example"), Some("https://c.example"));
        assert_eq!(destination("", ""), None, "nowhere to send: the option isn't offered");
        assert_eq!(destination("not a url", ""), None);
    }
}
