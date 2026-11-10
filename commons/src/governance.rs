//! The licences and signals a creator can attach to a version.
//!
//! Licences say what others may do with the work. Signals are machine-readable
//! preferences aimed at datasets and model training ("govern the models, not
//! only the data"). Both are fixed vocabularies: the keys are stored and
//! signed, so never rename an existing key.

/// `(key, name, url)` for each licence. `arr` has no URL.
pub const LICENSES: &[(&str, &str, &str)] = &[
    ("cc0", "CC0 1.0", "https://creativecommons.org/publicdomain/zero/1.0/"),
    ("cc-by", "CC BY 4.0", "https://creativecommons.org/licenses/by/4.0/"),
    ("cc-by-sa", "CC BY-SA 4.0", "https://creativecommons.org/licenses/by-sa/4.0/"),
    ("cc-by-nc", "CC BY-NC 4.0", "https://creativecommons.org/licenses/by-nc/4.0/"),
    ("cc-by-nc-sa", "CC BY-NC-SA 4.0", "https://creativecommons.org/licenses/by-nc-sa/4.0/"),
    ("cc-by-nd", "CC BY-ND 4.0", "https://creativecommons.org/licenses/by-nd/4.0/"),
    ("cc-by-nc-nd", "CC BY-NC-ND 4.0", "https://creativecommons.org/licenses/by-nc-nd/4.0/"),
    ("arr", "All rights reserved", ""),
];

/// The licence used when none (or an unknown one) is chosen on the form.
pub const DEFAULT_LICENSE: &str = "cc-by";

/// `(key, name, description)` for each CC signal.
pub const SIGNALS: &[(&str, &str, &str)] = &[
    ("credit", "Credit", "Attribution expected when this informs or trains a model."),
    ("direct-contribution", "Direct Contribution", "Those who build on it should give back to the creators."),
    ("ecosystem", "Ecosystem", "Reuse should sustain the open commons it came from."),
    ("open", "Open", "Models and outputs derived from it should stay open."),
];

/// Display name of a licence key. Unknown keys read as "All rights reserved",
/// the most restrictive reading.
pub fn license_name(key: &str) -> &'static str {
    LICENSES.iter().find(|l| l.0 == key).map(|l| l.1).unwrap_or("All rights reserved")
}

/// URL of a licence key, or `""` if it has none or is unknown.
pub fn license_url(key: &str) -> &'static str {
    LICENSES.iter().find(|l| l.0 == key).map(|l| l.2).unwrap_or("")
}

/// Display name of a signal key. Unknown keys show as a neutral "signal"
/// rather than echoing whatever a remote node sent.
pub fn signal_name(key: &str) -> &'static str {
    SIGNALS.iter().find(|s| s.0 == key).map(|s| s.1).unwrap_or("signal")
}

/// The licence key to store for a form value: known keys pass, anything else
/// becomes [`DEFAULT_LICENSE`].
pub fn license_from_form(value: &str) -> String {
    if LICENSES.iter().any(|l| l.0 == value) {
        value.to_string()
    } else {
        DEFAULT_LICENSE.to_string()
    }
}

/// The signals to store for the form's checkboxes: known keys only, sorted
/// and without duplicates, so the same choice always hashes the same way.
pub fn signals_from_form(values: &[String]) -> Vec<String> {
    let mut out: Vec<String> = values.iter().filter(|s| SIGNALS.iter().any(|x| x.0 == s.as_str())).cloned().collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_licence_reads_as_most_restrictive() {
        assert_eq!(license_name("cc-by"), "CC BY 4.0");
        assert_eq!(license_name("made-up"), "All rights reserved");
        assert_eq!(license_url("arr"), "");
    }

    #[test]
    fn form_licence_falls_back_to_default() {
        assert_eq!(license_from_form("cc0"), "cc0");
        assert_eq!(license_from_form("<script>"), DEFAULT_LICENSE);
    }

    #[test]
    fn form_signals_are_known_sorted_and_unique() {
        let picked = vec!["open".into(), "credit".into(), "bogus".into(), "open".into()];
        assert_eq!(signals_from_form(&picked), vec!["credit".to_string(), "open".to_string()]);
    }

    #[test]
    fn unknown_signal_is_not_echoed() {
        assert_eq!(signal_name("<b>hi</b>"), "signal");
    }
}
