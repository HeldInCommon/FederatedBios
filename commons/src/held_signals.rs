//! Held in Common Signals (decision record 0025): two answers a person can
//! give about AI, beyond the CC signals in [`crate::governance`].
//!
//! - **Withdraw even after training**: they keep the right to ask that their
//!   work be taken out of an AI model already trained on it, or that the
//!   model stop using it.
//! - **Own terms**: their own permissions and relationship to AI systems for
//!   this piece of work, in their own words (plain text, up to
//!   [`OWN_TERMS_MAX`] characters).
//!
//! The answers are signed. They travel in the body of two `hic/v1` types,
//! `ProfileV2` and `StoryV2` (`docs/spec/README.md` §5.5), as the object
//! `held_signals`. A commons that predates them refuses those types, and the
//! node then sends the older type without the answers (see
//! [`crate::federation::share_attempts`]). When nothing is chosen, nothing
//! changes: the older types are sent, byte for byte as before.
//!
//! Everything here is pure.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Longest "own terms", in characters (not bytes, so every script gets the
/// same room).
pub const OWN_TERMS_MAX: usize = 1_000;

/// How "withdraw even after training" is named on summaries, cards and chips.
pub const WITHDRAW_LABEL: &str = "Withdraw even after training";

/// How the presence of own terms is named on summaries and chips. The words
/// themselves are shown in full on the page of the profile or story.
pub const OWN_TERMS_LABEL: &str = "Own terms";

/// The name of the object in a signed body (and its two keys, below).
pub const BODY_KEY: &str = "held_signals";

/// The plain message shown when the own terms are too long.
pub const TOO_LONG: &str = "Keep your own terms for AI to 1,000 characters or fewer.";

/// A person's Held in Common Signals for one profile version or story.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldSignals {
    /// True when they keep the right to withdraw this even after a model has
    /// been trained on it. False means "not chosen", not "given up".
    #[serde(default)]
    pub withdraw_after_training: bool,
    /// Their own terms for AI, as plain text (never HTML): trimmed, with
    /// Windows line endings made `\n`. Empty when they wrote nothing.
    #[serde(default)]
    pub own_terms: String,
}

impl HeldSignals {
    /// True when nothing is chosen and nothing written. Such answers are left
    /// out of what is signed, so the older formats are used unchanged.
    pub fn is_empty(&self) -> bool {
        !self.withdraw_after_training && self.own_terms.is_empty()
    }

    /// The answers from the rights form: the checkbox `withdraw_after_training`
    /// (ticked sends `yes`) and the text box `own_terms`. The text is tidied
    /// first, then measured, so line endings don't count twice. Too long is
    /// refused with a plain message rather than cut, so nobody's words are
    /// changed without them seeing it.
    pub fn from_form(withdraw: &str, own_terms: &str) -> Result<HeldSignals, &'static str> {
        let own_terms = tidy(own_terms);
        if own_terms.chars().count() > OWN_TERMS_MAX {
            return Err(TOO_LONG);
        }
        Ok(HeldSignals { withdraw_after_training: withdraw == "yes", own_terms })
    }

    /// The answers as one line for a summary ("Withdraw even after training,
    /// Own terms"), or "None".
    pub fn summary(&self) -> String {
        let labels = self.labels();
        if labels.is_empty() {
            "None".into()
        } else {
            labels.join(", ")
        }
    }

    /// The names of what is chosen, for chips and summaries.
    pub fn labels(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.withdraw_after_training {
            out.push(WITHDRAW_LABEL);
        }
        if !self.own_terms.is_empty() {
            out.push(OWN_TERMS_LABEL);
        }
        out
    }
}

/// Own terms as they are kept: Windows line endings made `\n`, and space at
/// either end trimmed.
fn tidy(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

/// Read the Held in Common Signals of a received, already verified signed
/// body. `is_v2` says whether the object's type is one that carries them
/// (`ProfileV2`, `StoryV2`).
///
/// Strict on purpose: these are signed statements about someone's rights, so
/// a commons that can't read them all must refuse the object rather than
/// list it without them (decision record 0025). So:
/// - a V2 type must carry `held_signals`, and an older type must not;
/// - `held_signals` has exactly the keys `withdraw_after_training` (a
///   boolean) and `own_terms` (a string, at most [`OWN_TERMS_MAX`] characters).
pub fn from_body(body: &Value, is_v2: bool) -> Result<HeldSignals, String> {
    let found = body.get(BODY_KEY);
    let value = match (is_v2, found) {
        (true, Some(v)) => v,
        (true, None) => return Err("missing held_signals".into()),
        (false, Some(_)) => return Err("held_signals need a V2 type".into()),
        (false, None) => return Ok(HeldSignals::default()),
    };
    let object = value.as_object().ok_or("held_signals must be an object")?;
    if let Some(unknown) = object.keys().find(|k| *k != "withdraw_after_training" && *k != "own_terms") {
        return Err(format!("unknown held signal {unknown:?}"));
    }
    let withdraw_after_training = object
        .get("withdraw_after_training")
        .and_then(Value::as_bool)
        .ok_or("withdraw_after_training must be true or false")?;
    let own_terms = object.get("own_terms").and_then(Value::as_str).ok_or("own_terms must be text")?;
    if own_terms.chars().count() > OWN_TERMS_MAX {
        return Err("own_terms are too long".into());
    }
    Ok(HeldSignals { withdraw_after_training, own_terms: own_terms.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_form_is_tidied_and_a_tick_means_yes() {
        let h = HeldSignals::from_form("yes", "  Train on it for research.\r\nNot to imitate me.  ").unwrap();
        assert!(h.withdraw_after_training);
        assert_eq!(h.own_terms, "Train on it for research.\nNot to imitate me.");
        let none = HeldSignals::from_form("", "   ").unwrap();
        assert!(none.is_empty(), "an empty box and no tick is nothing chosen");
        assert!(!HeldSignals::from_form("on", "").unwrap().withdraw_after_training, "only `yes` ticks it");
    }

    #[test]
    fn own_terms_longer_than_the_limit_are_refused_plainly() {
        assert!(HeldSignals::from_form("", &"é".repeat(OWN_TERMS_MAX)).is_ok(), "the limit counts characters");
        assert_eq!(HeldSignals::from_form("", &"x".repeat(OWN_TERMS_MAX + 1)).unwrap_err(), TOO_LONG);
        let lines = "line\r\n".repeat(OWN_TERMS_MAX / 5);
        assert!(HeldSignals::from_form("", &lines).is_ok(), "Windows line endings count once");
    }

    #[test]
    fn summaries_name_what_is_chosen() {
        assert_eq!(HeldSignals::default().summary(), "None");
        let both = HeldSignals { withdraw_after_training: true, own_terms: "Mine.".into() };
        assert_eq!(both.summary(), "Withdraw even after training, Own terms");
        let words = HeldSignals { own_terms: "Mine.".into(), ..Default::default() };
        assert_eq!(words.labels(), vec![OWN_TERMS_LABEL]);
    }

    #[test]
    fn a_v2_body_must_carry_well_formed_held_signals() {
        let good = json!({ "held_signals": { "withdraw_after_training": true, "own_terms": "Ask me." } });
        assert_eq!(
            from_body(&good, true).unwrap(),
            HeldSignals { withdraw_after_training: true, own_terms: "Ask me.".into() }
        );
        assert!(from_body(&json!({}), true).is_err(), "missing");
        assert!(from_body(&json!({ "held_signals": "yes" }), true).is_err(), "not an object");
        let missing_key = json!({ "held_signals": { "withdraw_after_training": true } });
        assert!(from_body(&missing_key, true).is_err());
        let wrong_type = json!({ "held_signals": { "withdraw_after_training": "true", "own_terms": "" } });
        assert!(from_body(&wrong_type, true).is_err(), "a string isn't a boolean");
        let extra = json!({ "held_signals": { "withdraw_after_training": false, "own_terms": "", "sell": true } });
        assert!(from_body(&extra, true).unwrap_err().contains("unknown held signal"), "nothing is guessed");
        let long =
            json!({ "held_signals": { "withdraw_after_training": false, "own_terms": "x".repeat(OWN_TERMS_MAX + 1) } });
        assert!(from_body(&long, true).is_err());
    }

    #[test]
    fn an_older_type_carries_none_and_may_not_smuggle_them_in() {
        assert_eq!(from_body(&json!({ "handle": "ada" }), false).unwrap(), HeldSignals::default());
        let smuggled = json!({ "held_signals": { "withdraw_after_training": true, "own_terms": "" } });
        assert!(from_body(&smuggled, false).is_err());
    }

    #[test]
    fn a_version_file_without_answers_is_unchanged_and_old_files_still_load() {
        // Version skips empty answers, so stored files stay as they were;
        // a file written before this field existed loads as "not chosen".
        let old = r#"{"cid":"c","handle":"ada","visibility":"public","license":"cc-by","signals":[],
            "created_at":1,"signature":"s","payload":null,"enc":null}"#;
        let v: crate::model::Version = serde_json::from_str(old).unwrap();
        assert!(v.held_signals.is_empty());
        assert!(!serde_json::to_string(&v).unwrap().contains("held_signals"));
        let chosen = crate::model::Version {
            held_signals: HeldSignals { withdraw_after_training: true, ..Default::default() },
            ..v
        };
        assert!(serde_json::to_string(&chosen).unwrap().contains("\"withdraw_after_training\":true"));
    }
}
