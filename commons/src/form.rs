//! Parsing `application/x-www-form-urlencoded` bodies.
//!
//! Keeps repeated keys (the signal checkboxes arrive as several `signals=`
//! entries), which `serde_urlencoded` can't do.

use std::collections::HashMap;

/// A parsed form.
#[derive(Debug, Default)]
pub struct Form(HashMap<String, Vec<String>>);

impl Form {
    /// Parse a form body.
    pub fn parse(body: &str) -> Form {
        let mut m: HashMap<String, Vec<String>> = HashMap::new();
        for (k, v) in form_urlencoded::parse(body.as_bytes()) {
            m.entry(k.into_owned()).or_default().push(v.into_owned());
        }
        Form(m)
    }

    /// The first value for `key`, trimmed; `""` if absent.
    pub fn one(&self, key: &str) -> String {
        self.0.get(key).and_then(|v| v.first()).map(|s| s.trim().to_string()).unwrap_or_default()
    }

    /// The first value for `key`, trimmed and cut to `max` characters.
    pub fn clamped(&self, key: &str, max: usize) -> String {
        self.one(key).chars().take(max).collect()
    }

    /// Every value for `key`.
    pub fn many(&self, key: &str) -> Vec<String> {
        self.0.get(key).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_keys_and_decoding() {
        let f = Form::parse("signals=open&signals=credit&name=+Tega%20Brain+");
        assert_eq!(f.many("signals"), vec!["open", "credit"]);
        assert_eq!(f.one("name"), "Tega Brain");
        assert_eq!(f.one("missing"), "");
    }

    #[test]
    fn clamped_cuts_by_characters() {
        let f = Form::parse("bio=%C3%A9%C3%A9%C3%A9");
        assert_eq!(f.clamped("bio", 2), "éé");
    }
}
