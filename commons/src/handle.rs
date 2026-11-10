//! What a handle (the `@name` people choose) may contain.
//!
//! Handles become file names in the data folder, so this rule is also a
//! security check: nothing that isn't a valid handle may reach the filesystem.

/// Longest handle allowed, in characters.
pub const MAX_LEN: usize = 30;

/// True when `h` is 2–30 characters of lowercase ASCII letters, digits or `_`.
pub fn is_valid(h: &str) -> bool {
    let n = h.len();
    (2..=MAX_LEN).contains(&n) && h.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Normalise what someone typed: trimmed, lowercased and cut to [`MAX_LEN`].
/// The result still has to pass [`is_valid`].
pub fn normalise(typed: &str) -> String {
    typed.trim().to_lowercase().chars().take(MAX_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_handles() {
        assert!(is_valid("tega"));
        assert!(is_valid("phoenix_perry2"));
    }

    #[test]
    fn rejects_paths_and_odd_characters() {
        for bad in ["", "a", "../etc", "a/b", "Tega", "tega!", "tēga", &"x".repeat(31)] {
            assert!(!is_valid(bad), "{bad:?} should be invalid");
        }
    }

    #[test]
    fn normalise_lowercases_trims_and_cuts() {
        assert_eq!(normalise("  Tega "), "tega");
        assert_eq!(normalise(&"a".repeat(40)).len(), MAX_LEN);
    }
}
