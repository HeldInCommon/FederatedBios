//! Feedback to the Held in Common project (decision record 0030):
//! `GET /feedback` shows the form to a signed-in member (or the steward),
//! and `POST /feedback` checks it and sends it to the project's Google Sheet
//! through a Google Apps Script web app (`HIC_FEEDBACK_URL`).
//!
//! Nothing is written on this server: the message goes straight to the
//! sheet, or stays in the form for another try.

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use std::collections::HashMap;
use std::time::Duration;

use super::session;
use crate::app::{AppState, Shared};
use crate::feedback::{self, Answers, Feedback};
use crate::form::Form;
use crate::keys;
use crate::web::layout::{redirect, render, Page, Section};
use crate::web::pages::{self, FeedbackView};

/// How long to wait for the sheet. Apps Script can take a few seconds to
/// start when it hasn't run for a while.
const SEND_TIMEOUT: Duration = Duration::from_secs(20);

/// Render the Feedback page in `view` for `me` (the signed-in handle, if any).
fn page(st: &AppState, me: Option<&str>, view: FeedbackView) -> Response {
    let page = Page::new("Feedback", me, pages::feedback(view)).at(Section::Feedback).noindex(true);
    render(&st.site, page)
}

/// `GET /feedback` — the form for someone signed in; otherwise a note that
/// feedback needs an account. `?sent=1` (or `?sent=list`, after joining the
/// mailing list) shows the thank-you instead.
pub async fn feedback_form(
    State(st): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let Some(s) = session(&st, &headers) else {
        return page(&st, None, FeedbackView::SignedOut { is_commons: st.site.is_commons() });
    };
    let view = match q.get("sent").map(String::as_str) {
        Some("list") => FeedbackView::Sent { mailing_list: true },
        Some(_) => FeedbackView::Sent { mailing_list: false },
        None if st.config.feedback_destination().is_none() => FeedbackView::NotSetUp,
        None => FeedbackView::Form { csrf: &s.csrf, answers: &Answers::default(), problem: None, unsent: false },
    };
    page(&st, Some(&s.handle), view)
}

/// `POST /feedback` — check the session and CSRF token, check the answers
/// (`feedback::check`, pure), send them to the sheet, and show the
/// thank-you. When something is wrong the form comes back with the words
/// kept. Sends one request; writes nothing on this server.
pub async fn feedback_submit(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);

    // Only people with an account: a session, and that session's CSRF token
    // in the form so another site can't post here in their name.
    let Some(s) = session(&st, &headers) else {
        return page(&st, None, FeedbackView::SignedOut { is_commons: st.site.is_commons() });
    };
    if s.csrf.is_empty() || !keys::ct_eq(&s.csrf, &f.one("csrf")) {
        return page(&st, None, FeedbackView::SignedOut { is_commons: st.site.is_commons() });
    }
    let Some(destination) = st.config.feedback_destination() else {
        return page(&st, Some(&s.handle), FeedbackView::NotSetUp);
    };

    // Read the answers, clamped a little past the limit so `check` can say
    // the message was too long rather than cutting it off silently.
    let answers = Answers {
        kind: f.one("kind"),
        message: f.clamped("message", feedback::MESSAGE_MAX + 1),
        email: f.clamped("email", feedback::EMAIL_MAX + 1),
        may_reply: f.one("may_reply") == "yes",
        mailing_list: f.one("mailing_list") == "yes",
    };
    let role = if st.site.is_commons() { "commons" } else { "node" };
    let message = match feedback::check(&answers, env!("CARGO_PKG_VERSION"), role) {
        Ok(m) => m,
        Err(problem) => {
            let view = FeedbackView::Form { csrf: &s.csrf, answers: &answers, problem: Some(problem), unsent: false };
            return page(&st, Some(&s.handle), view);
        }
    };

    match send_to_sheet(destination, &message).await {
        // Redirect after a post, so reloading the thank-you doesn't send twice.
        Ok(()) => redirect(if message.mailing_list { "/feedback?sent=list" } else { "/feedback?sent=1" }),
        Err(why) => {
            // For the operator: what failed, never what was written.
            eprintln!("feedback: couldn't reach the sheet: {why}");
            let view = FeedbackView::Form { csrf: &s.csrf, answers: &answers, problem: None, unsent: true };
            page(&st, Some(&s.handle), view)
        }
    }
}

/// Send `message` to the Apps Script web app at `destination` as JSON.
/// `Ok` when the script answered `{"ok": true}`. Sends one request (and
/// follows Google's redirect to read the answer); changes nothing here.
///
/// Why this doesn't use `outbound::client_for` (decision record 0012): that
/// check guards addresses a *member* types, so nobody can aim this server at
/// the local network. This address is set by whoever runs the server or
/// built the app, like `HIC_RESEARCH_URL`.
///
/// Why redirects are followed: Apps Script runs `doPost` on the first
/// request, then answers `302` to a `script.googleusercontent.com` address
/// that holds the reply. `reqwest` follows it with a `GET` (as browsers do
/// for a 302), which is what Google expects. Limited to a few hops.
async fn send_to_sheet(destination: &str, message: &Feedback) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(3))
        .timeout(SEND_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let reply = client.post(destination).json(message).send().await.map_err(|e| e.to_string())?;
    let status = reply.status();
    let text = reply.text().await.map_err(|e| e.to_string())?;
    if status.is_success() && feedback::sheet_kept_it(&text) {
        Ok(())
    } else {
        // Only the status: the body may be a long Google error page.
        Err(format!("the sheet answered {status} without \"ok\""))
    }
}
