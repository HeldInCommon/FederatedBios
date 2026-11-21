//! The Feedback page (decision record 0030): tell the Held in Common project
//! about a bug, an idea or anything else, and join the mailing list. Built
//! from the same pieces as writing a story (`story_form.rs`): the page
//! heading, section labels, the choice cards and one action button.

use maud::{html, Markup};

use super::parts::{burst, page_head, section_label, Tone};
use crate::feedback::{Answers, Kind, Problem, EMAIL_MAX, MESSAGE_MAX};

/// What the Feedback page shows. The handler decides which; each state
/// carries only what it needs.
pub enum FeedbackView<'a> {
    /// Not signed in: only people with an account can send feedback.
    /// `is_commons` changes what "an account" means.
    SignedOut { is_commons: bool },
    /// This server has nowhere to send feedback (`HIC_FEEDBACK_URL`).
    NotSetUp,
    /// The form, with what was typed so far and, after a failed send, what
    /// to fix (`problem`) or that the sheet couldn't be reached (`unsent`).
    Form { csrf: &'a str, answers: &'a Answers, problem: Option<Problem>, unsent: bool },
    /// Sent. `mailing_list` says whether they joined it, for the thank-you.
    Sent { mailing_list: bool },
}

/// The page heading, the same on every state.
fn head() -> Markup {
    page_head(
        "Feedback \u{b7} to the Held in Common team",
        "Tell us what you think",
        Some("Found a bug? Wish it did something it doesn't? Anything you tell us helps shape what we build next."),
    )
}

/// The whole Feedback page for `view`.
pub fn feedback(view: FeedbackView) -> Markup {
    html! {
        (head())
        @match view {
            FeedbackView::SignedOut { is_commons } => (signed_out(is_commons)),
            FeedbackView::NotSetUp => {
                section class="flash flash-withdrawn" role="status" {
                    p class="flash-label" { "Not set up here" }
                    p { "This server has nowhere to send feedback. Its operator can set " code { "HIC_FEEDBACK_URL" } " (see the readme)." }
                }
            }
            FeedbackView::Form { csrf, answers, problem, unsent } => (form(csrf, answers, problem, unsent)),
            FeedbackView::Sent { mailing_list } => (sent(mailing_list)),
        }
    }
}

/// Signed out: say why the form isn't here and how to get an account.
fn signed_out(is_commons: bool) -> Markup {
    html! {
        section class="flash" role="status" {
            p class="flash-label" { "Sign in to send feedback" }
            @if is_commons {
                p { "Feedback comes from people with an account. Run your own node with the Held in Common app, then send it from there." }
            } @else {
                p { "Feedback comes from people with an account, so we know it's from someone using Held in Common." }
            }
        }
        @if !is_commons {
            div class="actions" {
                a class="btn-action" href="/login" { "Sign in" }
                a class="link-btn" href="/join" { "Add your profile" }
            }
        }
    }
}

/// The form. Keeps what was typed when it comes back with a problem, so
/// nobody loses a long bug report.
fn form(csrf: &str, a: &Answers, problem: Option<Problem>, unsent: bool) -> Markup {
    html! {
        @if let Some(p) = problem {
            section class="flash flash-withdrawn" role="alert" {
                p class="flash-label" { "Not sent yet" }
                p { (p.message()) }
            }
        }
        @if unsent {
            section class="flash flash-withdrawn" role="alert" {
                p class="flash-label" { "Not sent" }
                p { strong { "We couldn't reach the Held in Common team just now." } " Nothing was sent. Your words are still here: try again in a minute." }
            }
        }
        form method="post" action="/feedback" class="stack-form write-form" {
            input type="hidden" name="csrf" value=(csrf);

            // Step 1: what it's about, as the same cards as "How it travels".
            fieldset class="rights-step" {
                legend { (burst(1, Tone::Ink)) span { "What's it about?" } }
                div class="choice-panel" {
                    @for k in Kind::ALL { (kind_choice(k, a.kind == k.as_str())) }
                }
            }

            // Step 2: the words.
            fieldset class="rights-step" {
                legend { (burst(2, Tone::Ink)) span { "Tell us" } }
                div class="field" {
                    label class="field-label" for="message" { "Your message" }
                    textarea id="message" name="message" class="write-text" rows="10" maxlength=(MESSAGE_MAX)
                        aria-describedby="message_hint" { (a.message) }
                    p class="hint" id="message_hint" {
                        "For a bug: what you did, what you expected, and what happened instead. Up to 5,000 characters."
                    }
                }
            }

            // Step 3: staying in touch. Everything here is optional.
            fieldset class="rights-step" {
                legend { (burst(3, Tone::Ink)) span { "Stay in touch" } }
                p class="hint" { "Optional. Leave your email empty and we won't know who sent this." }
                div class="field" {
                    label class="field-label" for="email" { "Your email" }
                    input id="email" name="email" type="email" maxlength=(EMAIL_MAX) autocomplete="email"
                        aria-describedby="email_hint" value=(a.email);
                    p class="hint" id="email_hint" { "Only if you'd like a reply, or to join the mailing list." }
                }
                div class="choice-panel" {
                    (tick("may_reply", "Email me about this", "So we can ask a question or tell you when it's fixed.", a.may_reply))
                    (tick("mailing_list", "Join the mailing list", "Occasional news from Held in Common: new releases, workshops and research. You can ask to leave at any time.", a.mailing_list))
                }
            }

            (section_label("Where it goes", Some("Nothing is sent until you press Send")))
            p class="hint" {
                "Your message, your email if you gave one, and this app's version go to a Google Sheet that the Held in Common team reads. "
                "Your handle, profile and stories are never sent."
            }
            div class="actions" {
                button class="btn-action" type="submit" { "Send to the team" }
                a class="link-btn" href="/" { "Cancel" }
            }
        }
    }
}

/// One kind of feedback as a radio card, `checked` when it was chosen.
fn kind_choice(k: Kind, checked: bool) -> Markup {
    let (title, body) = match k {
        Kind::Bug => ("Something's broken", "A bug: something didn't work, or worked in a way you didn't expect."),
        Kind::Idea => ("An idea", "A feature you'd like, or a way to make something easier."),
        Kind::Other => ("Something else", "A question, a worry, or what you like about it."),
    };
    html! {
        label class="choice" {
            span class="choice-head" {
                input type="radio" name="kind" value=(k.as_str()) checked[checked];
                span class="choice-title" { (title) }
            }
            span class="choice-body" { (body) }
        }
    }
}

/// One checkbox as a card, like the research offer on a story's rights step.
fn tick(name: &str, title: &str, body: &str, checked: bool) -> Markup {
    let hint = format!("{name}_hint");
    html! {
        label class="choice" {
            span class="choice-head" {
                input type="checkbox" name=(name) value="yes" aria-describedby=(hint) checked[checked];
                span class="choice-title" { (title) }
            }
            span class="choice-body" id=(hint) { (body) }
        }
    }
}

/// The thank-you after a send.
fn sent(mailing_list: bool) -> Markup {
    html! {
        section class="flash flash-done" role="status" {
            p class="flash-label" { "Sent" }
            p {
                strong { "Thank you." }
                " The Held in Common team has your message."
                @if mailing_list { " We'll add you to the mailing list." }
            }
        }
        div class="actions" {
            a class="btn-action" href="/feedback" { "Send something else" }
            a class="link-btn" href="/" { "Back to the directory" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_keeps_what_was_typed_after_a_problem() {
        let a = Answers {
            kind: "idea".into(),
            message: "Dark <mode>".into(),
            email: "ada@example.org".into(),
            may_reply: false,
            mailing_list: true,
        };
        let m =
            feedback(FeedbackView::Form { csrf: "c", answers: &a, problem: Some(Problem::NeedsEmail), unsent: false })
                .into_string();
        assert!(m.contains(r#"value="idea" checked"#) && !m.contains(r#"value="bug" checked"#));
        assert!(m.contains("Dark &lt;mode&gt;"), "kept, and escaped");
        assert!(m.contains(r#"value="ada@example.org""#));
        assert!(m.contains(r#"name="mailing_list" value="yes" aria-describedby="mailing_list_hint" checked"#));
        assert!(m.contains(Problem::NeedsEmail.message()));
    }

    #[test]
    fn the_form_says_what_is_and_isnt_sent() {
        let m = feedback(FeedbackView::Form { csrf: "c", answers: &Answers::default(), problem: None, unsent: false })
            .into_string();
        assert!(m.contains("Google Sheet") && m.contains("never sent"));
        assert!(!m.contains("checked"), "nothing chosen for them");
    }

    #[test]
    fn signed_out_people_are_asked_to_sign_in() {
        let node = feedback(FeedbackView::SignedOut { is_commons: false }).into_string();
        assert!(node.contains("Sign in to send feedback") && node.contains(r#"href="/login""#));
        assert!(!node.contains("<form"));
        let commons = feedback(FeedbackView::SignedOut { is_commons: true }).into_string();
        assert!(commons.contains("Run your own node") && !commons.contains(r#"href="/login""#));
    }

    #[test]
    fn the_thank_you_mentions_the_mailing_list_only_when_joined() {
        assert!(feedback(FeedbackView::Sent { mailing_list: true }).into_string().contains("mailing list"));
        assert!(!feedback(FeedbackView::Sent { mailing_list: false }).into_string().contains("mailing list"));
    }
}
