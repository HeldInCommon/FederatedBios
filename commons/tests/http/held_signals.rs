//! Held in Common Signals (decision record 0025): chosen on a node's rights
//! panels, signed into a `ProfileV2` or `StoryV2`, listed and shown on a
//! commons; older formats still accepted; own terms kept to 1,000 characters.

use axum::http::StatusCode;
use hic_commons::federation::{self, Sender, ShareFormat};
use hic_commons::held_signals::{HeldSignals, OWN_TERMS_MAX};
use hic_commons::keys::keyid;
use hic_commons::role::Role;
use hic_commons::story;

use crate::common::*;

/// A well-formed story id (16 hex characters) for these tests.
const ID: &str = "0123456789abcdef";

/// The Held in Common Signals these tests choose: both answers given, with
/// some markup in the words to prove it is shown as text.
fn held() -> HeldSignals {
    HeldSignals { withdraw_after_training: true, own_terms: "Open research only.\n<b>Never</b> my voice.".into() }
}

/// Publish `handle`'s profile on `node` again, this time with the
/// withdraw-after-training box ticked and `own_terms` written.
async fn publish_with_held_signals(node: &axum::Router, sid: &str, handle: &str, own_terms: &str) -> Reply {
    let csrf = csrf_in(&send(node, get("/new", Some(sid))).await.text);
    let body = format!(
        "handle={handle}&csrf={csrf}&name=Tega&bio=hello&visibility=public&license=cc-by&signals=credit\
         &withdraw_after_training=yes&own_terms={}",
        urlencode(own_terms)
    );
    send(node, post_form("/publish", &body, Some(sid))).await
}

/// `text` percent-encoded for a form body (everything but letters and digits).
fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

#[tokio::test]
async fn the_rights_panels_offer_held_in_common_signals_as_step_four() {
    let (node, _) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let form = send(&node, get("/new", Some(&sid))).await.text;
    assert!(form.contains("Held in Common Signals: your terms for AI, in your words"));
    assert!(form.contains("name=\"withdraw_after_training\" value=\"yes\""));
    assert!(
        form.contains("<label class=\"field-label\" for=\"own_terms\">Your terms for AI, in your own words</label>")
    );
    assert!(form.contains("data-summary=\"held\""), "it's in What travels with every copy");

    // A story's rights step has the same step 4, and How it travels moves to 5.
    let csrf = csrf_in(&form);
    let r = send(&node, post_form("/stories/new", &format!("handle=ada&csrf={csrf}&title=T&text=Words."), Some(&sid)))
        .await;
    let rights = r.headers.get(axum::http::header::LOCATION).unwrap().to_str().unwrap().to_string();
    let step = send(&node, get(&rights, Some(&sid))).await.text;
    let four = step.find("Held in Common Signals:").unwrap();
    let five = step.find("How it travels").unwrap();
    assert!(four < five && step.contains(">5</text>"));
}

#[tokio::test]
async fn a_profile_with_held_signals_is_signed_shared_listed_and_shown() {
    let (node, node_store) = server(Role::Node);
    let sid = join_and_publish(&node, "tega", "public").await;
    let r = publish_with_held_signals(&node, &sid, "tega", "Open research only.\r\n<b>Never</b> my voice.").await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text);

    // On the node: kept with the version, shown on the profile.
    let rec = node_store.handle("tega").unwrap().unwrap();
    let v = node_store.current_version(&rec).unwrap().unwrap();
    assert_eq!(v.held_signals, held(), "tidied: Windows line endings made \\n");
    let page = send(&node, get("/@tega", None)).await.text;
    assert!(page.contains("Held in Common: Withdraw even after training, Own terms"));
    assert!(page.contains("&lt;b&gt;Never&lt;/b&gt; my voice."), "plain text, escaped");

    // The node offers a ProfileV2 first; a commons takes it.
    let attempts = federation::share_attempts(&Sender { base_url: "http://node", instance: "node" }, &rec, &v).unwrap();
    assert_eq!(attempts[0].0, ShareFormat::WithHeldSignals);
    let (commons, commons_store) = server(Role::Commons);
    let r = send(&commons, post_json("/inbox", &attempts[0].1)).await;
    assert!(r.status.is_success(), "{} {}", r.status, r.text);
    assert_eq!(commons_store.remotes()[0].held_signals, held());

    // Listed in the directory, and shown on the profile page there.
    assert!(send(&commons, get("/", None)).await.text.contains("tega"));
    let shown = send(&commons, get(&format!("/federated/{}", keyid(&rec.ed_pub)), None)).await.text;
    assert!(shown.contains("Held in Common: Withdraw even after training, Own terms"));
    assert!(shown.contains("Terms for AI, in @tega's own words"));
    assert!(shown.contains("Open research only.<br>&lt;b&gt;Never&lt;/b&gt; my voice."));
    let api = send(&node, get("/api/@tega", None)).await.text;
    assert!(api.contains("\"withdraw_after_training\":true"), "the API says so too");
}

#[tokio::test]
async fn a_commons_cannot_be_handed_changed_held_signals() {
    let (node, node_store) = server(Role::Node);
    let sid = join_and_publish(&node, "tega", "public").await;
    publish_with_held_signals(&node, &sid, "tega", "Ask me first.").await;
    let rec = node_store.handle("tega").unwrap().unwrap();
    let v = node_store.current_version(&rec).unwrap().unwrap();
    let mut msg = federation::build_share_v2(&Sender { base_url: "http://node", instance: "node" }, &rec, &v).unwrap();
    msg["profile"]["envelope"]["body"]["held_signals"]["withdraw_after_training"] = serde_json::json!(false);
    let (commons, store) = server(Role::Commons);
    let r = send(&commons, post_json("/inbox", &msg)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "the signature covers them");
    assert!(store.remotes().is_empty());
}

#[tokio::test]
async fn older_format_shares_still_work_and_read_as_not_chosen() {
    let (commons, store) = server(Role::Commons);
    // hic/v1 `Profile`, as an older node sends it.
    let ada = person("ada");
    assert!(send(&commons, post_json("/inbox", &share_of(&ada, None))).await.status.is_success());
    // Version 0, older still.
    let v0 = share_from_node("tega", "hello").await;
    assert!(send(&commons, post_json("/inbox", &v0)).await.status.is_success());
    assert_eq!(store.remotes().len(), 2);
    assert!(store.remotes().iter().all(|r| r.held_signals.is_empty()), "no fields means not chosen");
    let page = send(&commons, get(&format!("/federated/{}", keyid(&ada.ed_pub)), None)).await.text;
    assert!(!page.contains("Held in Common:") && !page.contains("own words"));
}

#[tokio::test]
async fn a_story_with_held_signals_is_shared_listed_and_shown() {
    let (commons, _) = server(Role::Commons);
    let ada = person("ada");
    assert!(send(&commons, post_json("/inbox", &share_of(&ada, None))).await.status.is_success());
    let s = story::new_story(ID, "ada", "Night bus", "The 38 sighs.", "public", "cc-by", &["credit".into()])
        .unwrap()
        .with_held_signals(held());
    let signed = story::sign(&s, &ada.ed_pub, &ada.ed_priv, 100).unwrap();
    assert_eq!(signed.envelope.kind, story::STORY_V2);
    let r = send(&commons, post_json("/inbox", &story::share_message(&signed))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);

    let cards = send(&commons, get("/stories", None)).await.text;
    assert!(cards.contains(">Withdraw even after training<") && cards.contains(">Own terms<"));
    let page = send(&commons, get(&format!("/stories/{}/{ID}", keyid(&ada.ed_pub)), None)).await.text;
    assert!(page.contains("Withdraw even after training, Own terms"));
    assert!(page.contains("Terms for AI, in @ada's own words") && page.contains("&lt;b&gt;Never&lt;/b&gt;"));

    // The older type of the same story is still taken (as a later version would be).
    let older = story::older_copy(&s, &story::sign(&s, &ada.ed_pub, &ada.ed_priv, 200).unwrap(), &ada.ed_priv)
        .unwrap()
        .unwrap();
    let r = send(&commons, post_json("/inbox", &story::share_message(&older))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let page = send(&commons, get(&format!("/stories/{}/{ID}", keyid(&ada.ed_pub)), None)).await.text;
    assert!(!page.contains("own words"), "the plain Story carries none");
}

#[tokio::test]
async fn own_terms_longer_than_1000_characters_are_refused_with_a_plain_message() {
    let (node, store) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let before = store.handle("ada").unwrap().unwrap().current_cid;
    let r = publish_with_held_signals(&node, &sid, "ada", &"x".repeat(OWN_TERMS_MAX + 1)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(r.text.contains("Keep your own terms for AI to 1,000 characters or fewer."));
    assert_eq!(store.handle("ada").unwrap().unwrap().current_cid, before, "nothing was published");
    let ok = publish_with_held_signals(&node, &sid, "ada", &"x".repeat(OWN_TERMS_MAX)).await;
    assert_eq!(ok.status, StatusCode::SEE_OTHER, "exactly 1,000 is fine");

    // The same on a story's rights step.
    let csrf = csrf_in(&send(&node, get("/new", Some(&sid))).await.text);
    let r = send(&node, post_form("/stories/new", &format!("handle=ada&csrf={csrf}&title=T&text=Words."), Some(&sid)))
        .await;
    let rights = r.headers.get(axum::http::header::LOCATION).unwrap().to_str().unwrap().to_string();
    let body =
        format!("csrf={csrf}&visibility=commons&license=cc-by&own_terms={}&action=keep", "x".repeat(OWN_TERMS_MAX + 1));
    let r = send(&node, post_form(&rights, &body, Some(&sid))).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(r.text.contains("1,000 characters or fewer"));

    // And a commons refuses a signed story whose own terms are too long.
    let (commons, _) = server(Role::Commons);
    let ada = person("ada");
    send(&commons, post_json("/inbox", &share_of(&ada, None))).await;
    let long = HeldSignals { withdraw_after_training: false, own_terms: "x".repeat(OWN_TERMS_MAX + 1) };
    let s = story::new_story(ID, "ada", "T", "Words.", "public", "cc-by", &[]).unwrap().with_held_signals(long);
    let signed = story::sign(&s, &ada.ed_pub, &ada.ed_priv, 100).unwrap();
    assert_eq!(
        send(&commons, post_json("/inbox", &story::share_message(&signed))).await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn changing_a_storys_terms_saves_held_signals_in_a_new_signed_version() {
    let (node, store) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let csrf = csrf_in(&send(&node, get("/new", Some(&sid))).await.text);
    let r = send(&node, post_form("/stories/new", &format!("handle=ada&csrf={csrf}&title=T&text=Words."), Some(&sid)))
        .await;
    let rights = r.headers.get(axum::http::header::LOCATION).unwrap().to_str().unwrap().to_string();
    let id = rights.trim_start_matches("/story/").trim_end_matches("/rights").to_string();
    let body = format!(
        "csrf={csrf}&visibility=commons&license=cc-by&signals=credit&withdraw_after_training=yes&own_terms={}&action=keep",
        urlencode("Ask me first.")
    );
    assert_eq!(send(&node, post_form(&rights, &body, Some(&sid))).await.status, StatusCode::SEE_OTHER);
    let own = store.own_story(&id).unwrap().unwrap();
    assert_eq!(own.signed.envelope.kind, story::STORY_V2);
    assert!(own.story.held_signals.withdraw_after_training && own.story.held_signals.own_terms == "Ask me first.");
    let step = send(&node, get(&rights, Some(&sid))).await.text;
    assert!(step.contains("data-label=\"Withdraw even after training\" checked"), "the step shows what was chosen");
    assert!(step.contains(">Ask me first.</textarea>"));
}
