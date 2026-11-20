//! What a commons said back when a node sent it something (a share, a
//! withdrawal, a story, a vouch, a key change), in a form that's safe to show
//! the member.
//!
//! A Held in Common commons answers its inbox in **plain text**: a short
//! reason such as "that room code isn't right". The node passes that reason
//! on in its own page. But the address a member types may not be a commons at
//! all. The common case is someone's **node opened to the room** (decision
//! record 0027): its door refuses every `POST` from the room with a whole
//! HTML page. Before this module, the node pasted that page into its error
//! message, and the member saw a screen of HTML code. Any other web server
//! (a router's login page, a typo'd site) does the same.
//!
//! So a reply is read once, here, into a [`CommonsReply`], and only a
//! plain-text reason is ever shown. Everything here is pure.

/// The longest reason we pass on. A commons' reasons are one short sentence;
/// anything longer is not one of ours, and is cut so it can't fill the page.
const MAX_REASON_CHARS: usize = 300;

/// What a commons' reply to a node's request said, once read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonsReply {
    /// A plain-text reason, trimmed and cut to `MAX_REASON_CHARS`: what a
    /// Held in Common commons sends when it refuses something.
    Reason(String),
    /// A web page instead of a reason: the address isn't a commons taking
    /// messages from nodes (most often someone's node opened to the room).
    NotACommons,
    /// An empty reply.
    NoReason,
}

/// Read a reply from its `Content-Type` header (if any) and its body. Pure.
///
/// A reply is a web page when the header says `text/html`, or, when a server
/// sends no header or a wrong one, when the body itself starts like HTML
/// (`<!DOCTYPE …` or `<html …`). Checking both means a page is never shown as
/// a reason, whichever way the other server labels it.
pub fn read(content_type: Option<&str>, body: &str) -> CommonsReply {
    let says_html = content_type.is_some_and(|value| value.trim().to_ascii_lowercase().starts_with("text/html"));
    let trimmed = body.trim();
    let start: String = trimmed.chars().take(20).collect::<String>().to_ascii_lowercase();
    let looks_like_html = start.starts_with("<!doctype") || start.starts_with("<html");
    if says_html || looks_like_html {
        return CommonsReply::NotACommons;
    }
    if trimmed.is_empty() {
        return CommonsReply::NoReason;
    }
    CommonsReply::Reason(trimmed.chars().take(MAX_REASON_CHARS).collect())
}

impl CommonsReply {
    /// The reason to show after "… didn't accept it: ", for a member who sent
    /// something to `place` (a commons' short name, from `host_label`). Pure.
    pub fn reason_for(&self, place: &str) -> String {
        match self {
            CommonsReply::Reason(reason) => reason.clone(),
            CommonsReply::NotACommons => {
                format!("{place} answered with a web page, not as a commons, so it may not be a commons at all.")
            }
            CommonsReply::NoReason => "no reason given".to_string(),
        }
    }

    /// The reason text, if this is a plain-text reason; empty otherwise.
    /// For code that matches on a commons' known reasons (see
    /// `vouch_list::vouch_result`). Pure.
    pub fn text(&self) -> &str {
        match self {
            CommonsReply::Reason(reason) => reason,
            CommonsReply::NotACommons | CommonsReply::NoReason => "",
        }
    }
}

/// What to tell a member whose share went to an address that answered with
/// a web page (see [`CommonsReply::NotACommons`]). The usual cause in a
/// workshop is pasting the address of someone's node opened to the room:
/// a node shows its owner's public profile and stories to the room, but
/// never takes anything in (decision record 0027), so the host needs a local
/// commons for the room to share to. Pure.
pub fn not_a_commons(place: &str) -> String {
    format!(
        "{place} isn't a commons, so it can't take your profile. If it's someone's node opened to the room, \
         that only shows their own profile and stories. To share in the room, ask the host for their local \
         commons' link (This computer \u{2192} Start a local commons, then open it to the room) and paste that \
         into Share to a commons."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The start of the page a node's door sends the room (`layout::only_here`).
    const DOOR_PAGE: &str =
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Only on this computer</title>";

    #[test]
    fn a_web_page_is_never_shown_as_a_reason() {
        assert_eq!(read(Some("text/html; charset=utf-8"), DOOR_PAGE), CommonsReply::NotACommons);
    }

    #[test]
    fn a_web_page_without_the_right_header_is_still_a_web_page() {
        assert_eq!(read(None, DOOR_PAGE), CommonsReply::NotACommons);
        assert_eq!(read(Some("text/plain"), "  <html><body>hi</body></html>"), CommonsReply::NotACommons);
    }

    #[test]
    fn a_commons_plain_reason_is_passed_on() {
        let reply =
            read(Some("text/plain; charset=utf-8"), "that room code isn't right; check it on the host's screen\n");
        assert_eq!(reply, CommonsReply::Reason("that room code isn't right; check it on the host's screen".into()));
        assert_eq!(reply.reason_for("10.0.0.5:8096"), "that room code isn't right; check it on the host's screen");
    }

    #[test]
    fn an_empty_reply_says_no_reason_was_given() {
        assert_eq!(read(Some("text/plain"), "  \n"), CommonsReply::NoReason);
        assert_eq!(read(None, "").reason_for("c.example"), "no reason given");
    }

    #[test]
    fn a_long_reason_is_cut_short() {
        let long = "x".repeat(5000);
        let CommonsReply::Reason(reason) = read(Some("text/plain"), &long) else { panic!("expected a reason") };
        assert_eq!(reason.chars().count(), MAX_REASON_CHARS);
    }

    #[test]
    fn only_a_plain_reason_has_text_to_match_on() {
        assert_eq!(read(None, "isn't a member here").text(), "isn't a member here");
        assert_eq!(read(None, DOOR_PAGE).text(), "");
    }

    #[test]
    fn the_not_a_commons_message_points_to_a_local_commons() {
        let message = not_a_commons("192.168.1.20:8095");
        assert!(message.starts_with("192.168.1.20:8095 isn't a commons"));
        assert!(message.contains("Start a local commons"));
        assert!(!message.contains('<'), "no markup in the message");
    }
}
