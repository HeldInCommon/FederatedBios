//! Writing a story in two steps, editing it, changing its terms, and how a
//! commons shows a withdrawal (decision records 0020 and 0022).

use axum::http::{header, StatusCode};
use hic_commons::role::Role;
use hic_commons::story;

use crate::common::*;

/// Write a story through the first step; returns its id and the CSRF token.
async fn write(node: &axum::Router, sid: &str, handle: &str) -> (String, String) {
    let csrf = csrf_in(&send(node, get("/stories/new", Some(sid))).await.text);
    let body = format!("handle={handle}&csrf={csrf}&title=Bins+at+six&text=The+lorry+comes+before+the+light.");
    let r = send(node, post_form("/stories/new", &body, Some(sid))).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text);
    let loc = r.headers.get(header::LOCATION).unwrap().to_str().unwrap().to_string();
    let id = loc.strip_prefix("/story/").and_then(|r| r.strip_suffix("/rights")).expect("goes to the rights step");
    (id.to_string(), csrf)
}

#[tokio::test]
async fn a_new_story_starts_listed_in_the_commons_and_goes_to_its_rights() {
    let (node, store) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let (id, _) = write(&node, &sid, "ada").await;
    let s = store.own_story(&id).unwrap().unwrap().story;
    assert_eq!((s.visibility.as_str(), s.license.as_str()), ("commons", "cc-by"));
    assert_eq!(s.signals, vec!["credit".to_string()]);

    let rights = send(&node, get(&format!("/story/{id}/rights"), Some(&sid))).await.text;
    assert!(rights.contains("Who can read it, and how"));
    assert!(rights.contains("value=\"commons\" data-label=\"Listed in the commons\" checked"));
    // No commons to share to yet: only Save.
    assert!(!rights.contains("Share to the room") && rights.contains("first share your profile"));
    // Someone else can't open it.
    assert_ne!(send(&node, get(&format!("/story/{id}/rights"), None)).await.status, StatusCode::OK);
}

#[tokio::test]
async fn changing_its_terms_signs_a_new_version_and_keep_shares_nothing() {
    let (node, store) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let (id, csrf) = write(&node, &sid, "ada").await;
    let before = store.own_story(&id).unwrap().unwrap().signed.cid;

    let body = format!("csrf={csrf}&visibility=public&license=cc-by-nc&signals=open&action=keep");
    let r = send(&node, post_form(&format!("/story/{id}/rights"), &body, Some(&sid))).await;
    assert_eq!(r.headers.get(header::LOCATION).unwrap(), &format!("/story/{id}?done=saved"));
    let after = store.own_story(&id).unwrap().unwrap();
    assert_eq!((after.story.visibility.as_str(), after.story.license.as_str()), ("public", "cc-by-nc"));
    assert_ne!(after.signed.cid, before, "a new signed version");
    hic_commons::envelope::verify(&after.signed).unwrap();
    assert!(after.shared_to.is_empty());

    let page = send(&node, get(&format!("/story/{id}?done=saved"), Some(&sid))).await.text;
    assert!(page.contains("Saved on this computer."));
}

#[tokio::test]
async fn sharing_from_the_rights_step_only_goes_to_a_commons_on_offer() {
    let (node, _) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let (id, csrf) = write(&node, &sid, "ada").await;
    let body =
        format!("csrf={csrf}&visibility=commons&license=cc-by&action=share&commons_url=http://elsewhere.example");
    let r = send(&node, post_form(&format!("/story/{id}/rights"), &body, Some(&sid))).await;
    assert!(r.text.contains("Share your profile to that commons first"));
}

#[tokio::test]
async fn editing_makes_a_new_version_with_the_same_id() {
    let (node, store) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let (id, csrf) = write(&node, &sid, "ada").await;
    let form = send(&node, get(&format!("/story/{id}/edit"), Some(&sid))).await.text;
    assert!(form.contains("value=\"Bins at six\"") && form.contains("Save changes"));

    let body = format!("csrf={csrf}&title=Bins+at+seven&text=Later+now.");
    let r = send(&node, post_form(&format!("/story/{id}/edit"), &body, Some(&sid))).await;
    assert_eq!(r.headers.get(header::LOCATION).unwrap(), &format!("/story/{id}?done=saved"));
    let s = store.own_story(&id).unwrap().unwrap();
    assert_eq!((s.story.title.as_str(), s.story.text.as_str()), ("Bins at seven", "Later now."));
    assert_eq!(s.story.license, "cc-by", "terms stay");

    // Without the session's token, nothing changes.
    let forged = send(&node, post_form(&format!("/story/{id}/edit"), "title=x&text=y", Some(&sid))).await;
    assert_ne!(forged.status, StatusCode::SEE_OTHER);
    assert_eq!(store.own_story(&id).unwrap().unwrap().story.title, "Bins at seven");
}

#[tokio::test]
async fn the_room_shows_who_withdrew_but_nothing_of_the_story() {
    let (commons, _) = server(Role::Commons);
    let ada = person("ada");
    assert!(send(&commons, post_json("/inbox", &share_of(&ada, None))).await.status.is_success());
    let s = story::new_story("0123456789abcdef", "ada", "Pigeon committee", "Coo.", "commons", "cc-by", &[]).unwrap();
    let signed = story::sign(&s, &ada.ed_pub, &ada.ed_priv, hic_commons::now()).unwrap();
    assert_eq!(send(&commons, post_json("/inbox", &story::share_message(&signed))).await.status, StatusCode::OK);

    let w = story::build_withdraw(&ada.ed_pub, &ada.ed_priv, "0123456789abcdef", COMMONS_URL, hic_commons::now() + 1)
        .unwrap();
    assert_eq!(send(&commons, post_json("/inbox", &w)).await.status, StatusCode::OK);
    let page = send(&commons, get("/stories", None)).await.text;
    assert!(page.contains("Withdrawn by @ada"));
    assert!(page.contains("0 shared \u{b7} 1 withdrawn"));
    assert!(!page.contains("Pigeon committee") && !page.contains("Coo."), "the story itself is gone");
}

#[tokio::test]
async fn the_rights_summary_script_is_served() {
    let (node, _) = server(Role::Node);
    let r = send(&node, get("/rights.js", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text.contains("data-summary"));
}
