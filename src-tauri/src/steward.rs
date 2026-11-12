//! The commons' steward link: kept out of the app's log, and shown only when
//! the person asks for it (Go → Show steward link…), ADR 0024.
//!
//! The steward link (`/steward?key=…`) opens a commons' steward view from any
//! computer, and its key doesn't change. The commons prints it when it starts,
//! for people who run it from the command line; the app copies everything the
//! server prints into its log, and logs get shared for support. So the app
//! hides the key as it writes the log ([`redact_for_log`]), and reads the key
//! from the commons' own file when the menu asks ([`read_key`], [`links`]).

use std::path::Path;

/// The file in the commons' data folder that holds its steward key. The
/// commons writes it the first time it starts (`Store::save_steward_key` in
/// `commons/src/store.rs`); the app only reads it.
const KEY_FILE: &str = "steward.json";

/// What the log shows instead of a hidden secret, and where to find it.
const HIDDEN: &str = "(hidden: Go \u{2192} Show steward link\u{2026})";

/// Read the steward key from the commons' data folder `data_dir`. Only
/// reads. The error is a sentence for the person, since the setup window
/// shows it as it is.
pub fn read_key(data_dir: &Path) -> Result<String, String> {
    let path = data_dir.join(KEY_FILE);
    // A missing file means the commons hasn't started yet on this computer.
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err("Your commons hasn't made its steward link yet. Start the commons once, then try again.".into())
        }
        Err(e) => return Err(format!("Couldn't read the steward link: {e}")),
    };
    key_from_json(&text).ok_or_else(|| "The commons' steward file is damaged. Restart the commons to fix it.".into())
}

/// The key inside the text of `steward.json` (`{"key": "…"}`), if there is
/// a non-empty one. Pure, so the file format is tested without a file.
fn key_from_json(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let key = value.get("key")?.as_str()?.trim();
    (!key.is_empty()).then(|| key.to_string())
}

/// The steward link for each address the commons answers to (its public
/// address first, then its room address), in the same form the commons
/// prints. Pure.
pub fn links(key: &str, addresses: &[String]) -> Vec<String> {
    addresses.iter().map(|base| format!("{}/steward?key={key}", base.trim_end_matches('/'))).collect()
}

/// A line of server output with any steward key or launch token replaced by
/// a note saying where to find it, ready for the log. Pure. Lines without
/// one come back unchanged.
pub fn redact_for_log(line: &str) -> String {
    let mut out = line.to_string();
    // The steward key never changes, so it must never reach a shared log.
    // The launch token changes every launch, but is hidden too, in case a
    // future message prints a link that carries it (ADR 0023).
    for marker in ["steward?key=", "launch="] {
        out = hide_after(&out, marker);
    }
    out
}

/// `text` with whatever follows each `marker`, up to the next space, `&` or
/// end of line, replaced by [`HIDDEN`]. Pure.
fn hide_after(text: &str, marker: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    // Copy up to and including each marker, then skip the secret after it.
    while let Some(at) = rest.find(marker) {
        let (before, after) = rest.split_at(at + marker.len());
        out.push_str(before);
        let secret_len = after.find(|c: char| c.is_whitespace() || c == '&').unwrap_or(after.len());
        if secret_len > 0 {
            out.push_str(HIDDEN);
        }
        rest = &after[secret_len..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steward_key_never_reaches_the_log() {
        let line = "hic-commons steward link (keep private): https://hic.example/steward?key=abc123SECRET";
        let logged = redact_for_log(line);
        assert!(!logged.contains("abc123SECRET"), "{logged}");
        assert!(logged.starts_with("hic-commons steward link (keep private): https://hic.example/steward?key="));
        assert!(logged.contains("Show steward link"), "says where to find it");
    }

    #[test]
    fn every_steward_key_and_launch_token_in_a_line_is_hidden() {
        let line = "a/steward?key=one b/steward?key=two&x=1 /steward?launch=three";
        let logged = redact_for_log(line);
        for secret in ["one", "two", "three"] {
            assert!(!logged.contains(secret), "{secret} in {logged}");
        }
        assert!(logged.contains("&x=1"), "only the secret is hidden");
    }

    #[test]
    fn ordinary_lines_are_logged_as_they_are() {
        for line in ["started commons on port 8081", "GET /steward 200", "steward?key=", ""] {
            assert_eq!(redact_for_log(line), line);
        }
    }

    #[test]
    fn a_link_for_each_address_public_first() {
        let addresses = vec!["https://hic.example.ts.net/".to_string(), "http://10.0.0.5:8081".to_string()];
        assert_eq!(
            links("k1", &addresses),
            vec!["https://hic.example.ts.net/steward?key=k1", "http://10.0.0.5:8081/steward?key=k1"]
        );
    }

    #[test]
    fn the_key_is_read_from_the_commons_own_file_format() {
        assert_eq!(key_from_json(r#"{"key":"abc"}"#), Some("abc".to_string()));
        assert_eq!(key_from_json(r#"{"key":"  "}"#), None, "an empty key is no key");
        assert_eq!(key_from_json(r#"{"other":"abc"}"#), None);
        assert_eq!(key_from_json("not json"), None);
    }

    #[test]
    fn a_commons_that_never_started_says_so() {
        let empty = std::env::temp_dir().join("hic-no-such-commons-data");
        let err = read_key(&empty).unwrap_err();
        assert!(err.contains("Start the commons once"), "{err}");
    }
}
