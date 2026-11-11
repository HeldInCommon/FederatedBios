//! Canonical JSON, following RFC 8785 (the JSON Canonicalization Scheme).
//!
//! Two programs that mean the same JSON must produce the same bytes, or their
//! hashes and signatures won't match. RFC 8785 fixes the three things that
//! usually differ between JSON libraries:
//!
//! 1. **Key order:** object keys are sorted by their UTF-16 code units.
//! 2. **Strings:** only `"`, `\` and control characters are escaped; `\b \f \n
//!    \r \t` use their short forms, other control characters use `\u00xx`
//!    (lowercase hex); everything else is written as raw UTF-8.
//! 3. **Numbers:** written as ECMAScript does.
//!
//! Held in Common's format (`docs/spec/README.md`) allows **only integers**
//! between −(2⁵³−1) and 2⁵³−1, written as plain decimals, which is what RFC
//! 8785 produces for them. Anything else is an error rather than a guess, so
//! we never depend on floating-point formatting.
//!
//! No whitespace is written anywhere.

use serde_json::Value;

/// The largest integer every JSON implementation represents exactly (2⁵³−1).
pub const MAX_SAFE_INT: i64 = 9_007_199_254_740_991;

/// Write `v` as canonical JSON bytes.
pub fn to_vec(v: &Value) -> Result<Vec<u8>, String> {
    let mut out = String::new();
    write(v, &mut out)?;
    Ok(out.into_bytes())
}

/// Append the canonical form of `v` to `out`: object keys sorted, no
/// whitespace, integers only (within the range JavaScript can hold exactly).
fn write(v: &Value, out: &mut String) -> Result<(), String> {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let i = n.as_i64().ok_or("only integers are allowed in signed objects")?;
            if !(-MAX_SAFE_INT..=MAX_SAFE_INT).contains(&i) {
                return Err("integer outside the safe range (±2^53−1)".into());
            }
            out.push_str(&i.to_string());
        }
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            // Sort by UTF-16 code units, as RFC 8785 requires (this differs
            // from byte order only for characters outside the Basic
            // Multilingual Plane).
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write(&map[*k], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Append `s` as a JSON string, escaped as RFC 8785 asks: the short escapes
/// (`\n`, `\t` …) where JSON has them, `\u00XX` for other control characters,
/// and everything else as it is.
fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The canonical form of `v`, as text, for comparing in tests.
    fn canon(v: Value) -> String {
        String::from_utf8(to_vec(&v).unwrap()).unwrap()
    }

    #[test]
    fn keys_are_sorted_and_no_whitespace() {
        assert_eq!(canon(json!({"b": 1, "a": [true, null, "x"]})), r#"{"a":[true,null,"x"],"b":1}"#);
    }

    #[test]
    fn nested_objects_are_sorted_too() {
        assert_eq!(canon(json!({"z": {"y": 1, "x": 2}})), r#"{"z":{"x":2,"y":1}}"#);
    }

    #[test]
    fn string_escaping_follows_rfc_8785() {
        assert_eq!(canon(json!("a\"b\\c\n\t\u{1}é€😀/")), "\"a\\\"b\\\\c\\n\\t\\u0001é€😀/\"");
    }

    /// RFC 8785 §3.2.3 sorts by UTF-16 code units: U+1F600 (surrogates
    /// D83D DE00) sorts *before* U+FB33, though its UTF-8 bytes sort after.
    #[test]
    fn sorting_uses_utf16_code_units() {
        assert_eq!(canon(json!({"\u{FB33}": 1, "\u{1F600}": 2})), "{\"\u{1F600}\":2,\"\u{FB33}\":1}");
    }

    #[test]
    fn only_safe_integers() {
        assert_eq!(canon(json!(-42)), "-42");
        assert!(to_vec(&json!(1.5)).is_err());
        assert!(to_vec(&json!(9_007_199_254_740_992_i64)).is_err());
        assert!(to_vec(&json!(u64::MAX)).is_err());
    }
}
