//! Feedback to the Held in Common project (decision record 0030): a bug, an
//! idea for a feature, or anything else someone wants to tell us, and a
//! place to join the project's mailing list.
//!
//! - **Only from people with an account.** The page and the post both need a
//!   signed-in session (a member on a node, or the steward on a commons).
//!   That check is in the handler (`web/handlers/feedback.rs`); this module
//!   only decides what a form's answers mean.
//! - **Where it goes.** The server sends a [`Feedback`] as JSON to a Google
//!   Apps Script web app (`HIC_FEEDBACK_URL`), which adds a row to the
//!   project's Google Sheet. The script lives in `tools/feedback-sheet/`.
//! - **Only what they wrote.** No handle, key, name, profile or story
//!   travels. An email address goes only when the person types one, because
//!   they want a reply or want to join the mailing list.
//!
//! Everything here is pure.

use serde::Serialize;

/// The format name inside every message, so the sheet's script can tell
/// versions apart if the format changes.
pub const FORMAT: &str = "hic-feedback/1";

/// Longest message, in characters. Long enough for a bug report with steps;
/// short enough that a sheet cell stays readable.
pub const MESSAGE_MAX: usize = 5000;

/// Longest email address, in characters (the usual limit, RFC 5321 §4.5.3.1).
pub const EMAIL_MAX: usize = 254;

/// What kind of feedback this is. One radio button each on the form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Something doesn't work as it should.
    Bug,
    /// Something they'd like the app to do.
    Idea,
    /// Anything else: a question, a kind word, a worry.
    Other,
}

impl Kind {
    /// Every kind, in the order the form shows them.
    pub const ALL: [Kind; 3] = [Kind::Bug, Kind::Idea, Kind::Other];

    /// The word sent in the form and to the sheet.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Idea => "idea",
            Kind::Other => "other",
        }
    }

    /// The kind a form value names, if it names one.
    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// What the server sends to the sheet, field for field. Nothing that says
/// who someone is, unless they typed their email.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Feedback {
    /// Always [`FORMAT`].
    pub format: String,
    /// `bug`, `idea` or `other` (see [`Kind`]). Empty when they only joined
    /// the mailing list.
    pub kind: String,
    /// What they wrote. May be empty when they only joined the mailing list.
    pub message: String,
    /// Their email address, or empty.
    pub email: String,
    /// True when they're happy for the team to email them about this.
    pub may_reply: bool,
    /// True when they asked to join the project's mailing list.
    pub mailing_list: bool,
    /// The app's version (`CARGO_PKG_VERSION`), so a bug can be matched to a
    /// release.
    pub version: String,
    /// `node` or `commons`: which kind of server it came from.
    pub role: String,
}

/// The form's answers, as typed. The handler fills this from the post and
/// [`check`] turns it into a [`Feedback`] or says what's wrong.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answers {
    /// The kind radio button's value.
    pub kind: String,
    /// The message box.
    pub message: String,
    /// The email box.
    pub email: String,
    /// The "you may email me about this" box was ticked.
    pub may_reply: bool,
    /// The mailing list box was ticked.
    pub mailing_list: bool,
}

/// Why a form can't be sent yet. Each says what to do, in plain words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// No message, and not joining the mailing list: nothing to send.
    Empty,
    /// A message, but no kind chosen.
    NoKind,
    /// The message is longer than [`MESSAGE_MAX`].
    TooLong,
    /// They asked for a reply or the mailing list but gave no email.
    NeedsEmail,
    /// The email doesn't look like one.
    BadEmail,
}

impl Problem {
    /// What the page says.
    pub fn message(self) -> &'static str {
        match self {
            Problem::Empty => "Write something to send, or tick the box to join the mailing list.",
            Problem::NoKind => "Choose what it's about: a bug, an idea, or something else.",
            Problem::TooLong => "Keep it to 5,000 characters or fewer. For a longer report, send the main points and ask us to get in touch.",
            Problem::NeedsEmail => "Add your email address, so we can reply or add you to the mailing list.",
            Problem::BadEmail => "That email address doesn't look right. Check it, or leave it empty.",
        }
    }
}

/// Tidy typed text: Windows line endings made `\n`, and trimmed.
fn tidy(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

/// A loose check that `email` could be an address: one `@`, something
/// before it, and a dot in the part after it, with no spaces. Real checking
/// would need sending a message; this only catches typing slips. Pure.
fn looks_like_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else { return false };
    !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.chars().any(char::is_whitespace)
        && email.chars().count() <= EMAIL_MAX
}

/// Check the form's answers and build the message to send, or say what to
/// fix. `version` and `role` describe this server. Pure.
///
/// The rules, in order:
/// 1. There must be a message or a mailing list sign-up.
/// 2. A message needs a kind, and fits in [`MESSAGE_MAX`].
/// 3. A reply or the mailing list needs an email that looks like one; an
///    email typed for neither is still checked, and still sent, so a typo
///    doesn't leave the team unable to answer.
pub fn check(answers: &Answers, version: &str, role: &str) -> Result<Feedback, Problem> {
    let message = tidy(&answers.message);
    let email = answers.email.trim().to_string();

    // Step 1: something to send.
    if message.is_empty() && !answers.mailing_list {
        return Err(Problem::Empty);
    }

    // Step 2: a message says what it's about. Joining the mailing list on
    // its own needs no kind.
    let kind = match (message.is_empty(), Kind::parse(&answers.kind)) {
        (true, _) => String::new(),
        (false, Some(k)) => k.as_str().to_string(),
        (false, None) => return Err(Problem::NoKind),
    };
    if message.chars().count() > MESSAGE_MAX {
        return Err(Problem::TooLong);
    }

    // Step 3: an email when one is needed, and a plausible one when given.
    if email.is_empty() && (answers.may_reply || answers.mailing_list) {
        return Err(Problem::NeedsEmail);
    }
    if !email.is_empty() && !looks_like_email(&email) {
        return Err(Problem::BadEmail);
    }

    Ok(Feedback {
        format: FORMAT.into(),
        kind,
        message,
        email,
        may_reply: answers.may_reply,
        mailing_list: answers.mailing_list,
        version: version.into(),
        role: role.into(),
    })
}

/// Where feedback goes: `url` when it's a web address, else `None` and the
/// form says feedback isn't set up on this server. Pure.
pub fn destination(url: &str) -> Option<&str> {
    let url = url.trim();
    (url.starts_with("https://") || url.starts_with("http://")).then_some(url)
}

/// Did the sheet's script keep it? The script answers `{"ok": true}`; any
/// other answer (an error page, a sign-in page because the script isn't
/// shared with "Anyone", `{"ok": false}`) means no. Pure.
pub fn sheet_kept_it(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body).is_ok_and(|v| v["ok"] == serde_json::Value::Bool(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bug report with no email, for these tests.
    fn bug() -> Answers {
        Answers { kind: "bug".into(), message: "  The Share button does nothing.\r\n".into(), ..Answers::default() }
    }

    #[test]
    fn a_bug_report_is_tidied_and_carries_the_version() {
        let f = check(&bug(), "0.9.7", "node").unwrap();
        assert_eq!(f.message, "The Share button does nothing.");
        assert_eq!((f.kind.as_str(), f.version.as_str(), f.role.as_str()), ("bug", "0.9.7", "node"));
        assert_eq!(f.format, FORMAT);
        assert!(f.email.is_empty() && !f.mailing_list);
    }

    #[test]
    fn an_empty_form_has_nothing_to_send() {
        assert_eq!(check(&Answers::default(), "v", "node"), Err(Problem::Empty));
        let spaces = Answers { kind: "idea".into(), message: "   ".into(), ..Answers::default() };
        assert_eq!(check(&spaces, "v", "node"), Err(Problem::Empty));
    }

    #[test]
    fn a_message_needs_a_kind_but_the_mailing_list_alone_does_not() {
        let no_kind = Answers { kind: String::new(), ..bug() };
        assert_eq!(check(&no_kind, "v", "node"), Err(Problem::NoKind));
        let made_up = Answers { kind: "rant".into(), ..bug() };
        assert_eq!(check(&made_up, "v", "node"), Err(Problem::NoKind));
        let list_only = Answers { email: "ada@example.org".into(), mailing_list: true, ..Answers::default() };
        let f = check(&list_only, "v", "node").unwrap();
        assert!(f.mailing_list && f.kind.is_empty() && f.message.is_empty());
    }

    #[test]
    fn a_reply_or_the_mailing_list_needs_an_email() {
        let reply = Answers { may_reply: true, ..bug() };
        assert_eq!(check(&reply, "v", "node"), Err(Problem::NeedsEmail));
        let list = Answers { mailing_list: true, ..bug() };
        assert_eq!(check(&list, "v", "node"), Err(Problem::NeedsEmail));
    }

    #[test]
    fn an_email_that_isnt_one_is_caught() {
        for bad in ["ada", "ada@", "@example.org", "ada@example", "ada@@example.org", "ada @example.org", "ada@.org"] {
            let a = Answers { email: bad.into(), may_reply: true, ..bug() };
            assert_eq!(check(&a, "v", "node"), Err(Problem::BadEmail), "{bad:?}");
        }
        let good = Answers { email: " ada@example.org ".into(), may_reply: true, ..bug() };
        assert_eq!(check(&good, "v", "node").unwrap().email, "ada@example.org");
    }

    #[test]
    fn a_message_past_the_limit_is_refused() {
        let long = Answers { message: "a".repeat(MESSAGE_MAX + 1), ..bug() };
        assert_eq!(check(&long, "v", "node"), Err(Problem::TooLong));
        let exact = Answers { message: "é".repeat(MESSAGE_MAX), ..bug() };
        assert!(check(&exact, "v", "node").is_ok(), "counted in characters, not bytes");
    }

    #[test]
    fn only_web_addresses_are_destinations() {
        assert_eq!(
            destination(" https://script.google.com/macros/s/x/exec "),
            Some("https://script.google.com/macros/s/x/exec")
        );
        assert_eq!(destination(""), None);
        assert_eq!(destination("script.google.com"), None);
    }

    #[test]
    fn only_an_ok_answer_from_the_script_counts_as_kept() {
        assert!(sheet_kept_it(r#"{"ok":true}"#));
        assert!(!sheet_kept_it(r#"{"ok":false,"error":"too long"}"#));
        assert!(!sheet_kept_it("<html>Sign in to continue</html>"));
        assert!(!sheet_kept_it(r#"{"ok":"true"}"#));
    }
}
