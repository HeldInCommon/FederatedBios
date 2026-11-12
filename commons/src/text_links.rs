//! Finding web links in a story's plain text, for display only.
//!
//! A story is stored and signed as plain text (decision record 0020); this
//! module never changes it. When a page shows a story, it asks
//! [`find_links`] to split each line into plain words and links, and the
//! page turns the links into `<a>` elements. Two forms are recognised:
//!
//! - a bare web address: `https://example.org/page`;
//! - a Markdown-style link: `[some words](https://example.org/page)`.
//!
//! **Why this is careful.** Story text comes from other people, and a link
//! is one of the easiest ways to run code in a reader's browser
//! (`javascript:…`) or to break out of an HTML attribute (a `"` or `<` in
//! the address). So:
//!
//! - only `http://` and `https://` addresses become links, never
//!   `javascript:`, `data:` or any other scheme;
//! - an address stops at a space, a quote, `<`, `>`, a backtick or a
//!   backslash, so it can never carry markup;
//! - everything else stays text, and the page escapes it (maud escapes every
//!   value it writes, including attribute values).
//!
//! Everything here is pure: text in, pieces out.

/// One piece of a line of story text: plain words, or a link.
///
/// Rust note: `'a` (a *lifetime*) says each piece borrows a slice of the
/// line it came from, rather than copying it. The pieces can't outlive that
/// line.
#[derive(Debug, PartialEq, Eq)]
pub enum Piece<'a> {
    /// Words to show as they are (the page escapes them).
    Text(&'a str),
    /// A link: the words to show, and the checked web address they open.
    Link {
        /// What the reader sees: the address itself, or the words in `[…]`.
        words: &'a str,
        /// The address, always `http://` or `https://` (see [`is_web_address`]).
        href: &'a str,
    },
}

/// The two schemes that may become links, in lower case.
const SCHEMES: [&str; 2] = ["http://", "https://"];

/// True for a character that ends a web address: white space, control
/// characters, and the characters that could break out of an HTML attribute
/// or start markup (`"`, `'`, `<`, `>`, a backtick, a backslash).
fn ends_address(c: char) -> bool {
    c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '`' | '\\')
}

/// The scheme `s` starts with (`http://` or `https://`, in any case), if it
/// starts with one. Returns how many bytes the scheme takes.
fn scheme_len(s: &str) -> Option<usize> {
    SCHEMES.iter().find_map(|scheme| {
        // `get` returns `None` rather than panicking if `s` is shorter, or if
        // the cut would land inside a multi-byte character.
        let start = s.get(..scheme.len())?;
        start.eq_ignore_ascii_case(scheme).then_some(scheme.len())
    })
}

/// True when `s` is a whole web address we're willing to link to: it starts
/// with `http://` or `https://`, names a host (something other than `/`
/// straight after the scheme), and contains nothing that ends an address
/// (white space, quotes, `<`, `>`, a backtick or a backslash). This is the only gate between a story's text and
/// an `href`.
pub fn is_web_address(s: &str) -> bool {
    let Some(n) = scheme_len(s) else { return false };
    let rest = &s[n..];
    !rest.is_empty() && !rest.starts_with('/') && !s.chars().any(ends_address)
}

/// Trailing characters that usually end a sentence rather than an address:
/// in "see https://example.org." the full stop isn't part of it.
fn is_trailing_punctuation(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ']' | '}' | '*' | '_')
}

/// The bare address at the start of `s` (which starts with a scheme), with
/// sentence punctuation trimmed off its end, or `None` if what's left isn't
/// an address. A closing `)` is trimmed only when it has no partner inside
/// the address, so `https://en.wikipedia.org/wiki/Moss_(plant)` keeps its
/// own, while "(see https://example.org)" loses the one that closes the
/// sentence.
fn bare_address_at(s: &str) -> Option<&str> {
    // Up to the first character that can't be in an address.
    let end = s.find(ends_address).unwrap_or(s.len());
    let mut url = &s[..end];
    // Trim from the end, one character at a time, until nothing changes.
    while let Some(last) = url.chars().last() {
        let unpaired_close = last == ')' && url.matches(')').count() > url.matches('(').count();
        if is_trailing_punctuation(last) || unpaired_close {
            url = &url[..url.len() - last.len_utf8()];
        } else {
            break;
        }
    }
    is_web_address(url).then_some(url)
}

/// A Markdown-style link at the start of `s` (which starts with `[`):
/// `[words](address)`. Returns the words, the address and how many bytes the
/// whole link takes, or `None` if it isn't one we accept: the words must be
/// non-empty and contain no `[` or `]`; the address must pass
/// [`is_web_address`]; one level of brackets inside the address is allowed
/// (`(…(…)…)`), so Wikipedia-style addresses work.
fn markdown_link_at(s: &str) -> Option<(&str, &str, usize)> {
    // The words: everything up to the first `]`, with no `[` inside.
    let close = s.find(']')?;
    let words = &s[1..close];
    if words.trim().is_empty() || words.contains('[') {
        return None;
    }
    // Straight after `]` must come `(`, with no space between.
    let after = &s[close + 1..];
    if !after.starts_with('(') {
        return None;
    }
    // The address: up to the `)` that closes the one we just opened.
    let mut depth = 0usize;
    let mut href_end = None;
    for (i, c) in after.char_indices().skip(1) {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => {
                href_end = Some(i);
                break;
            }
            ')' => depth -= 1,
            _ => {}
        }
    }
    let href_end = href_end?;
    let href = &after[1..href_end];
    if !is_web_address(href) {
        return None;
    }
    // 1 for `[`, the words, 1 for `]`, then `(address)`.
    let length = 1 + words.len() + 1 + href_end + 1;
    Some((words, href, length))
}

/// True when the character before byte `at` in `text` lets an address start
/// there: the start of the text, or anything but a letter or digit. This
/// keeps "xhttp://…" from becoming a link to "http://…".
fn starts_a_word(text: &str, at: usize) -> bool {
    text[..at].chars().last().is_none_or(|c| !c.is_alphanumeric())
}

/// Split `text` (one line, or more) into plain words and links, in order.
/// Joining every piece's words back together gives `text` again, except that
/// a Markdown link shows only its words. Links never nest: a web address
/// written inside the words of a Markdown link stays text.
pub fn find_links(text: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    // `plain_from` is where the text not yet handed out as a piece begins;
    // `i` is where we are looking now. Both are byte positions.
    let mut plain_from = 0;
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        // What starts here: a Markdown link, a bare address, or neither.
        let found = if rest.starts_with('[') {
            markdown_link_at(rest).map(|(words, href, length)| (Piece::Link { words, href }, length))
        } else if scheme_len(rest).is_some() && starts_a_word(text, i) {
            bare_address_at(rest).map(|url| (Piece::Link { words: url, href: url }, url.len()))
        } else {
            None
        };
        match found {
            Some((link, length)) => {
                // Hand out the plain text before the link, then the link.
                if plain_from < i {
                    pieces.push(Piece::Text(&text[plain_from..i]));
                }
                pieces.push(link);
                i += length;
                plain_from = i;
            }
            None => {
                // Move on by one whole character (UTF-8 characters can be
                // several bytes long, and a slice must not cut one in half).
                i += rest.chars().next().map_or(1, char::len_utf8);
            }
        }
    }
    if plain_from < text.len() {
        pieces.push(Piece::Text(&text[plain_from..]));
    }
    pieces
}

/// `text` with each Markdown link replaced by its words, and everything else
/// as it is: for previews (a story card's first line), where the card itself
/// is the link and the `[…](…)` markup would only be noise.
pub fn without_link_markup(text: &str) -> String {
    find_links(text)
        .into_iter()
        .map(|piece| match piece {
            Piece::Text(words) | Piece::Link { words, .. } => words,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shorthand for a link piece whose words are its address.
    fn bare(url: &str) -> Piece<'_> {
        Piece::Link { words: url, href: url }
    }

    #[test]
    fn plain_text_is_one_piece() {
        assert_eq!(find_links("The 38 sighs at the stop."), vec![Piece::Text("The 38 sighs at the stop.")]);
        assert_eq!(find_links(""), vec![]);
    }

    #[test]
    fn a_bare_address_becomes_a_link() {
        assert_eq!(
            find_links("Hear it at https://example.org/street now"),
            vec![Piece::Text("Hear it at "), bare("https://example.org/street"), Piece::Text(" now")]
        );
        assert_eq!(
            find_links("http://room.local:8081/x?a=1&b=2#here"),
            vec![bare("http://room.local:8081/x?a=1&b=2#here")]
        );
    }

    #[test]
    fn the_scheme_may_be_in_capitals_and_the_address_is_kept_as_written() {
        assert_eq!(find_links("HTTPS://Example.org"), vec![bare("HTTPS://Example.org")]);
    }

    #[test]
    fn a_markdown_link_shows_its_words() {
        assert_eq!(
            find_links("Read [the map](https://example.org/map) first."),
            vec![
                Piece::Text("Read "),
                Piece::Link { words: "the map", href: "https://example.org/map" },
                Piece::Text(" first."),
            ]
        );
    }

    #[test]
    fn trailing_sentence_punctuation_is_not_part_of_the_address() {
        for (text, url) in [
            ("See https://example.org.", "https://example.org"),
            ("See https://example.org, then", "https://example.org"),
            ("Really? https://example.org/a?!", "https://example.org/a"),
            ("(see https://example.org)", "https://example.org"),
            ("(see https://example.org).", "https://example.org"),
            ("https://example.org/x;", "https://example.org/x"),
        ] {
            assert!(find_links(text).contains(&bare(url)), "{text}");
        }
    }

    #[test]
    fn brackets_that_belong_to_the_address_are_kept() {
        let wiki = "https://en.wikipedia.org/wiki/Moss_(plant)";
        assert!(find_links(&format!("Moss: {wiki}.")).contains(&bare(wiki)));
        assert_eq!(
            find_links(&format!("[moss]({wiki})")),
            vec![Piece::Link { words: "moss", href: wiki }],
            "one level of brackets inside a Markdown address"
        );
    }

    #[test]
    fn javascript_and_other_schemes_never_become_links() {
        for text in [
            "[click](javascript:alert(1))",
            "[click](JaVaScRiPt:alert(1))",
            "[click](data:text/html,<script>alert(1)</script>)",
            "[click](vbscript:msgbox)",
            "[click](//evil.example/x)",
            "[click](ftp://example.org)",
            "javascript:alert(1)",
        ] {
            assert!(find_links(text).iter().all(|p| matches!(p, Piece::Text(_))), "{text} must stay text");
        }
    }

    #[test]
    fn a_disguised_scheme_inside_words_is_not_an_address() {
        // "xhttp://" isn't an address; nor is "javascript:http://…" one
        // that starts with javascript: (the link found starts at http).
        assert!(find_links("xhttp://example.org").iter().all(|p| matches!(p, Piece::Text(_))));
        let pieces = find_links("javascript:http://example.org");
        assert!(pieces.iter().all(|p| match p {
            Piece::Link { href, .. } => href.starts_with("http"),
            Piece::Text(_) => true,
        }));
    }

    #[test]
    fn quotes_and_angle_brackets_end_an_address_so_it_cannot_carry_markup() {
        assert_eq!(
            find_links("https://example.org/\"onmouseover=\"alert(1)"),
            vec![bare("https://example.org/"), Piece::Text("\"onmouseover=\"alert(1)")]
        );
        assert_eq!(
            find_links("https://example.org/<script>"),
            vec![bare("https://example.org/"), Piece::Text("<script>")]
        );
        assert!(find_links("https://example.org/'x").contains(&bare("https://example.org/")));
        // In the Markdown form, a quote, `<` or space spoils the Markdown
        // link. The clean start of the address may still be found as a bare
        // address, which is safe: it stops before the bad character.
        for text in [
            "[x](https://example.org/\"onclick=\"alert(1))",
            "[x](https://example.org/<b>)",
            "[x](https://a b)",
            "[click]( https://example.org)",
        ] {
            for piece in find_links(text) {
                if let Piece::Link { words, href } = piece {
                    assert_eq!(words, href, "{text}: no Markdown link");
                    assert!(is_web_address(href), "{text}: {href}");
                }
            }
        }
    }

    #[test]
    fn an_address_needs_a_host() {
        for text in ["http://", "https:///etc/passwd", "see http:// here"] {
            assert!(find_links(text).iter().all(|p| matches!(p, Piece::Text(_))), "{text}");
        }
    }

    #[test]
    fn brackets_inside_text_are_left_alone() {
        assert_eq!(find_links("[a note] and (an aside)"), vec![Piece::Text("[a note] and (an aside)")]);
        // An unmatched `[` before a real link: the link is still found.
        assert_eq!(
            find_links("[a [b](https://example.org)"),
            vec![Piece::Text("[a "), Piece::Link { words: "b", href: "https://example.org" }]
        );
        // `]` then something other than `(`: not a Markdown link, but the
        // address later in it still is one.
        assert_eq!(
            find_links("[a]b](https://example.org)"),
            vec![Piece::Text("[a]b]("), bare("https://example.org"), Piece::Text(")")]
        );
        assert!(find_links("[](https://example.org)").iter().any(|p| *p == bare("https://example.org")), "no words");
    }

    #[test]
    fn an_address_inside_markdown_words_does_not_nest() {
        assert_eq!(
            find_links("[see https://a.example](https://b.example)"),
            vec![Piece::Link { words: "see https://a.example", href: "https://b.example" }]
        );
    }

    #[test]
    fn non_ascii_text_is_split_safely() {
        assert_eq!(
            find_links("café — https://example.org/é ok"),
            vec![Piece::Text("café — "), bare("https://example.org/é"), Piece::Text(" ok")]
        );
        assert_eq!(
            find_links("[ünï](https://example.org)"),
            vec![Piece::Link { words: "ünï", href: "https://example.org" }]
        );
    }

    #[test]
    fn the_pieces_put_back_together_are_the_text() {
        let text = "One https://a.example, two [b](https://b.example) and (c https://c.example/x_(y)).";
        let rebuilt: String = find_links(text)
            .iter()
            .map(|p| match p {
                Piece::Text(t) => t.to_string(),
                Piece::Link { words, href } if words == href => href.to_string(),
                Piece::Link { words, href } => format!("[{words}]({href})"),
            })
            .collect();
        assert_eq!(rebuilt, text);
    }

    #[test]
    fn previews_drop_the_markup_but_keep_the_words() {
        assert_eq!(without_link_markup("Read [the map](https://example.org) here"), "Read the map here");
        assert_eq!(without_link_markup("At https://example.org."), "At https://example.org.");
        assert_eq!(without_link_markup("[x](javascript:alert(1))"), "[x](javascript:alert(1))");
    }
}
