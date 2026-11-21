//! Feedback to the project (decision record 0030): only someone signed in
//! can send it, it reaches the sheet's script as JSON without their handle,
//! and a failed send keeps their words.

use std::sync::{Arc, Mutex};

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::routing;
use axum::{Json, Router};
use hic_commons::role::Role;
use serde_json::Value;

use crate::common::*;

/// What the fake script received.
type Received = Arc<Mutex<Vec<Value>>>;

/// A stand-in for the Google Apps Script web app, on a real port. Like
/// Google, it takes the `POST`, then answers `302` to another address where
/// the reply waits, so the test covers following that redirect. When
/// `answer_ok` is false it replies `{"ok": false}`. Returns its address and
/// what it received.
async fn fake_sheet(answer_ok: bool) -> (String, Received) {
    let received: Received = Arc::new(Mutex::new(Vec::new()));
    let kept = received.clone();
    let reply = if answer_ok { r#"{"ok":true}"# } else { r#"{"ok":false,"error":"refused"}"# };
    let app = Router::new()
        .route(
            "/exec",
            routing::post(move |Json(v): Json<Value>| async move {
                kept.lock().unwrap().push(v);
                Redirect::to("/echo").into_response()
            }),
        )
        .route("/echo", routing::get(move || async move { ([(header::CONTENT_TYPE, "application/json")], reply) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://127.0.0.1:{}/exec", listener.local_addr().unwrap().port());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, received)
}

/// Where a response redirects to.
fn location(r: &Reply) -> &str {
    r.headers.get(header::LOCATION).unwrap().to_str().unwrap()
}

#[tokio::test]
async fn the_nav_offers_feedback_on_every_page() {
    let (node, _) = server(Role::Node);
    assert!(send(&node, get("/about", None)).await.text.contains(r#"href="/feedback""#));
    let (commons, _) = server(Role::Commons);
    assert!(send(&commons, get("/", None)).await.text.contains(r#"href="/feedback""#));
}

#[tokio::test]
async fn without_an_account_there_is_no_form_and_nothing_is_sent() {
    let (url, received) = fake_sheet(true).await;
    let (node, _) = server_with(Role::Node, &[("HIC_FEEDBACK_URL", &url)]);
    let page = send(&node, get("/feedback", None)).await.text;
    assert!(page.contains("Sign in to send feedback") && !page.contains(r#"name="message""#));
    let r = send(&node, post_form("/feedback", "kind=bug&message=Broken", None)).await;
    assert!(r.text.contains("Sign in to send feedback"));
    assert!(received.lock().unwrap().is_empty(), "a post without a session sends nothing");
}

#[tokio::test]
async fn a_post_without_the_csrf_token_sends_nothing() {
    let (url, received) = fake_sheet(true).await;
    let (node, _) = server_with(Role::Node, &[("HIC_FEEDBACK_URL", &url)]);
    let sid = join_and_publish(&node, "ada", "public").await;
    let r = send(&node, post_form("/feedback", "csrf=wrong&kind=bug&message=Broken", Some(&sid))).await;
    assert_ne!(r.status, StatusCode::SEE_OTHER);
    assert!(received.lock().unwrap().is_empty(), "another site can't post in their name");
}

#[tokio::test]
async fn a_member_sends_a_bug_report_to_the_sheet_without_their_handle() {
    let (url, received) = fake_sheet(true).await;
    let (node, _) = server_with(Role::Node, &[("HIC_FEEDBACK_URL", &url)]);
    let sid = join_and_publish(&node, "ada", "public").await;
    let page = send(&node, get("/feedback", Some(&sid))).await.text;
    assert!(page.contains(r#"href="/feedback" aria-current="page""#), "the nav marks Feedback");
    let csrf = csrf_in(&page);

    let body = format!("csrf={csrf}&kind=bug&message=The+Share+button+does+nothing.&email=&action=send");
    let r = send(&node, post_form("/feedback", &body, Some(&sid))).await;
    assert_eq!(location(&r), "/feedback?sent=1", "{}", r.text);

    let got = received.lock().unwrap().clone();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0]["kind"], "bug");
    assert_eq!(got[0]["message"], "The Share button does nothing.");
    assert_eq!(got[0]["role"], "node");
    assert_eq!(got[0]["format"], "hic-feedback/1");
    assert!(!got[0].to_string().contains("ada"), "the handle never travels");

    let thanks = send(&node, get("/feedback?sent=1", Some(&sid))).await.text;
    assert!(thanks.contains("Thank you.") && !thanks.contains("mailing list."));
}

#[tokio::test]
async fn joining_the_mailing_list_needs_only_an_email() {
    let (url, received) = fake_sheet(true).await;
    let (node, _) = server_with(Role::Node, &[("HIC_FEEDBACK_URL", &url)]);
    let sid = join_and_publish(&node, "ada", "public").await;
    let csrf = csrf_in(&send(&node, get("/feedback", Some(&sid))).await.text);

    // Ticked without an email: the form comes back and says so.
    let r = send(&node, post_form("/feedback", &format!("csrf={csrf}&mailing_list=yes"), Some(&sid))).await;
    assert!(r.text.contains("Add your email address"));
    assert!(r.text.contains(r#"name="mailing_list" value="yes" aria-describedby="mailing_list_hint" checked"#));
    assert!(received.lock().unwrap().is_empty());

    let body = format!("csrf={csrf}&email=ada%40example.org&mailing_list=yes");
    let r = send(&node, post_form("/feedback", &body, Some(&sid))).await;
    assert_eq!(location(&r), "/feedback?sent=list");
    let got = received.lock().unwrap().clone();
    assert_eq!((got[0]["email"].as_str(), got[0]["mailing_list"].as_bool()), (Some("ada@example.org"), Some(true)));
    assert!(send(&node, get("/feedback?sent=list", Some(&sid))).await.text.contains("add you to the mailing list"));
}

#[tokio::test]
async fn when_the_sheet_says_no_the_words_are_kept_to_try_again() {
    let (url, received) = fake_sheet(false).await;
    let (node, _) = server_with(Role::Node, &[("HIC_FEEDBACK_URL", &url)]);
    let sid = join_and_publish(&node, "ada", "public").await;
    let csrf = csrf_in(&send(&node, get("/feedback", Some(&sid))).await.text);
    let body = format!("csrf={csrf}&kind=idea&message=A+dark+mode%2C+please.");
    let r = send(&node, post_form("/feedback", &body, Some(&sid))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text.contains("Nothing was sent") && r.text.contains("A dark mode, please."));
    assert!(r.text.contains(r#"value="idea" checked"#));
    assert_eq!(received.lock().unwrap().len(), 1, "it was tried once");
}

#[tokio::test]
async fn an_unreachable_sheet_keeps_the_words_too() {
    // Port 9 (discard) on this computer: nothing answers.
    let (node, _) = server_with(Role::Node, &[("HIC_FEEDBACK_URL", "http://127.0.0.1:9/exec")]);
    let sid = join_and_publish(&node, "ada", "public").await;
    let csrf = csrf_in(&send(&node, get("/feedback", Some(&sid))).await.text);
    let r = send(&node, post_form("/feedback", &format!("csrf={csrf}&kind=other&message=Hello"), Some(&sid))).await;
    assert!(r.text.contains("Nothing was sent") && r.text.contains(">Hello</textarea>"));
}

#[tokio::test]
async fn with_nowhere_to_send_the_page_says_so() {
    // Only meaningful when this build has no feedback address baked in.
    if hic_commons::config::BUILT_IN_FEEDBACK_URL.is_some() {
        return;
    }
    let (node, _) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let page = send(&node, get("/feedback", Some(&sid))).await.text;
    assert!(page.contains("Not set up here") && !page.contains(r#"name="message""#));
}

#[tokio::test]
async fn the_steward_can_send_feedback_from_a_commons() {
    let (url, received) = fake_sheet(true).await;
    let (commons, store) = server_with(Role::Commons, &[("HIC_FEEDBACK_URL", &url)]);
    let (sid, _) = steward_session(&commons, &store).await;
    let csrf = csrf_in(&send(&commons, get("/feedback", Some(&sid))).await.text);
    let r =
        send(&commons, post_form("/feedback", &format!("csrf={csrf}&kind=idea&message=More+tools"), Some(&sid))).await;
    assert_eq!(location(&r), "/feedback?sent=1");
    assert_eq!(received.lock().unwrap()[0]["role"], "commons");
}
