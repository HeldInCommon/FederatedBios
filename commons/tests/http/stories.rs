//! Stories: written and kept on a node, shared to a commons where the author
//! is listed, withdrawn from it (see `hic_commons::story`).

use axum::http::{header, StatusCode};
use hic_commons::role::Role;
use hic_commons::story;

use crate::common::*;

const ID: &str = "0123456789abcdef";

/// A signed story share by `rec`, dated `t`.
fn story_share(rec: &hic_commons::model::HandleRec, id: &str, t: u64, title: &str) -> serde_json::Value {
    let s = story::new_story(
        id,
        &rec.handle,
        title,
        "The 38 sighs at the stop.\nA dog answers.",
        "public",
        "cc-by",
        &["credit".into()],
    )
    .unwrap();
    story::share_message(&story::sign(&s, &rec.ed_pub, &rec.ed_priv, t).unwrap())
}

/// A commons with `ada` listed on it, and ada's identity.
async fn commons_with_ada() -> (axum::Router, hic_commons::store::Store, hic_commons::model::HandleRec) {
    let (commons, store) = server(Role::Commons);
    let ada = person("ada");
    let r = send(&commons, post_json("/inbox", &share_of(&ada, None))).await;
    assert!(r.status.is_success(), "ada is listed: {} {}", r.status, r.text);
    (commons, store, ada)
}

#[tokio::test]
async fn a_listed_member_shares_a_story_and_anyone_can_read_it() {
    let (commons, _, ada) = commons_with_ada().await;
    let r = send(&commons, post_json("/inbox", &story_share(&ada, ID, 100, "Night bus"))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);

    let list = send(&commons, get("/stories", None)).await;
    assert!(list.text.contains("Night bus"));
    assert!(list.text.contains("The 38 sighs at the stop."), "the first line is the preview");
    let kid = hic_commons::keys::keyid(&ada.ed_pub);
    let page = send(&commons, get(&format!("/stories/{kid}/{ID}"), None)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text.contains("A dog answers."));
    assert!(page.text.contains("checked when it arrived"));
}

#[tokio::test]
async fn someone_not_listed_cannot_share_a_story() {
    let (commons, _, _) = commons_with_ada().await;
    let stranger = person("sam");
    let r = send(&commons, post_json("/inbox", &story_share(&stranger, ID, 100, "Hello"))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(r.text.contains("share your profile"));
    // Nor can someone else's key use a listed handle.
    let mut impostor = person("ada");
    impostor.handle = "ada".into();
    let r = send(&commons, post_json("/inbox", &story_share(&impostor, ID, 100, "Hello"))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_tampered_story_is_refused() {
    let (commons, _, ada) = commons_with_ada().await;
    let mut msg = story_share(&ada, ID, 100, "Night bus");
    msg["story"]["envelope"]["body"]["text"] = serde_json::json!("words ada never wrote");
    let r = send(&commons, post_json("/inbox", &msg)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_newer_version_replaces_an_older_one_and_not_the_reverse() {
    let (commons, store, ada) = commons_with_ada().await;
    assert_eq!(send(&commons, post_json("/inbox", &story_share(&ada, ID, 200, "Second"))).await.status, StatusCode::OK);
    let old = send(&commons, post_json("/inbox", &story_share(&ada, ID, 100, "First"))).await;
    assert_eq!(old.status, StatusCode::CONFLICT);
    let held = store.held_stories();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].story.title, "Second");
}

#[tokio::test]
async fn withdrawing_removes_the_story_and_it_cannot_be_replayed() {
    let (commons, store, ada) = commons_with_ada().await;
    let share = story_share(&ada, ID, 100, "Night bus");
    assert_eq!(send(&commons, post_json("/inbox", &share)).await.status, StatusCode::OK);

    let w = story::build_withdraw(&ada.ed_pub, &ada.ed_priv, ID, COMMONS_URL, 150).unwrap();
    let r = send(&commons, post_json("/inbox", &w)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert!(store.held_stories().is_empty());
    assert!(!send(&commons, get("/stories", None)).await.text.contains("Night bus"));

    let replay = send(&commons, post_json("/inbox", &share)).await;
    assert_eq!(replay.status, StatusCode::CONFLICT, "the withdrawn copy can't come back");

    // Addressed to another commons: refused.
    let elsewhere = story::build_withdraw(&ada.ed_pub, &ada.ed_priv, ID, "http://other.example", 160).unwrap();
    assert_eq!(send(&commons, post_json("/inbox", &elsewhere)).await.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn withdrawing_your_profile_takes_your_stories_with_it() {
    let (commons, store, ada) = commons_with_ada().await;
    assert_eq!(
        send(&commons, post_json("/inbox", &story_share(&ada, ID, 100, "Night bus"))).await.status,
        StatusCode::OK
    );
    let w = hic_commons::federation::build_withdraw(&ada, COMMONS_URL, hic_commons::now() + 5).unwrap();
    let r = send(&commons, post_json("/inbox", &w)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert!(store.held_stories().is_empty());
}

#[tokio::test]
async fn a_node_does_not_take_stories_in() {
    let (node, _) = server(Role::Node);
    let ada = person("ada");
    let r = send(&node, post_json("/inbox", &story_share(&ada, ID, 100, "Night bus"))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

/// The story id from a `Location: /story/<id>…` header.
fn story_id(r: &Reply) -> String {
    let loc = r.headers.get(header::LOCATION).unwrap().to_str().unwrap().to_string();
    let rest = loc.strip_prefix("/story/").expect("redirects to the story");
    rest.split(['/', '?']).next().unwrap().to_string()
}

#[tokio::test]
async fn writing_a_story_on_a_node() {
    let (node, store) = server(Role::Node);
    // Signed out: no form.
    let r = send(&node, get("/stories/new", None)).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);

    let sid = join_and_publish(&node, "ada", "public").await;
    let form = send(&node, get("/stories/new", Some(&sid))).await;
    assert!(form.text.contains("Listen to your street"));
    assert!(!form.text.contains("name=\"visibility\""), "rights come in step two");
    let csrf = csrf_in(&form.text);

    let body = format!(
        "handle=ada&csrf={csrf}&title=Night+bus&text=The+38+sighs.%0D%0A%0D%0AA+dog+answers.&visibility=public&license=cc-by-nc&signals=credit"
    );
    let r = send(&node, post_form("/stories/new", &body, Some(&sid))).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text);
    let id = story_id(&r);

    let own = store.own_story(&id).unwrap().unwrap();
    assert_eq!(own.story.license, "cc-by-nc");
    hic_commons::envelope::verify(&own.signed).unwrap();
    assert_eq!(own.signed.envelope.author, store.handle("ada").unwrap().unwrap().ed_pub);

    let page = send(&node, get(&format!("/story/{id}"), Some(&sid))).await;
    assert!(page.text.contains("Night bus"));
    assert!(page.text.contains("Share your profile to a commons first"), "not shared anywhere yet");

    let text = send(&node, get(&format!("/story/{id}/text"), None)).await;
    assert_eq!(text.status, StatusCode::OK);
    assert!(text.headers.get(header::CONTENT_DISPOSITION).unwrap().to_str().unwrap().contains("attachment"));
    assert!(text.text.starts_with("Night bus\n\nThe 38 sighs."));

    // Without the CSRF token nothing is written.
    let forged = send(&node, post_form("/stories/new", "handle=ada&title=x&text=y", Some(&sid))).await;
    assert_ne!(forged.status, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn members_only_stories_stay_with_members() {
    let (node, _) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let csrf = csrf_in(&send(&node, get("/stories/new", Some(&sid))).await.text);
    let body = format!("handle=ada&csrf={csrf}&title=Private&text=Just+for+us&visibility=members&license=cc-by");
    let id = story_id(&send(&node, post_form("/stories/new", &body, Some(&sid))).await);

    assert!(send(&node, get("/stories", Some(&sid))).await.text.contains("Private"));
    assert!(!send(&node, get("/stories", None)).await.text.contains("Private"));
    assert_eq!(send(&node, get(&format!("/story/{id}"), None)).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&node, get(&format!("/story/{id}/text"), None)).await.status, StatusCode::NOT_FOUND);

    // And it can't be shared.
    let share = format!("id={id}&csrf={csrf}&commons_url=http://localhost:9");
    let r = send(&node, post_form("/story/share", &share, Some(&sid))).await;
    assert!(r.text.contains("stay on your node"));
}

#[tokio::test]
async fn bad_ids_are_simply_not_found() {
    let (node, _) = server(Role::Node);
    assert_eq!(send(&node, get("/story/..%2F..%2Fetc", None)).await.status, StatusCode::NOT_FOUND);
    let (commons, _) = server(Role::Commons);
    assert_eq!(send(&commons, get("/stories/zz/zz", None)).await.status, StatusCode::NOT_FOUND);
}

/// The desktop app's room commons announces its Wi-Fi address, while a node
/// on the same computer reaches it at 127.0.0.1. Withdrawals addressed that
/// way must still count (they were refused as "a different commons").
#[tokio::test]
async fn withdrawals_from_a_node_on_the_commons_own_computer_are_accepted() {
    let (commons, store) = server_with(Role::Commons, &[("PORT", "8081"), ("HIC_BASE_URL", "http://192.168.1.5:8081")]);
    let ada = person("ada");
    assert!(send(&commons, post_json("/inbox", &share_of(&ada, None))).await.status.is_success());
    assert_eq!(
        send(&commons, post_json("/inbox", &story_share(&ada, ID, 100, "Night bus"))).await.status,
        StatusCode::OK
    );

    let w = story::build_withdraw(&ada.ed_pub, &ada.ed_priv, ID, "http://127.0.0.1:8081", 150).unwrap();
    let r = send(&commons, post_json("/inbox", &w)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert!(store.held_stories().is_empty());

    let w = hic_commons::federation::build_withdraw(&ada, "http://127.0.0.1:8081", hic_commons::now() + 5).unwrap();
    let r = send(&commons, post_json("/inbox", &w)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);

    // Another port is another commons.
    let other = story::build_withdraw(&ada.ed_pub, &ada.ed_priv, ID, "http://127.0.0.1:9999", 160).unwrap();
    assert_eq!(send(&commons, post_json("/inbox", &other)).await.status, StatusCode::BAD_REQUEST);
}
