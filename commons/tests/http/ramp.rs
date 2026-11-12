//! The ramp (decision record 0022): the member's next step and tabs, the
//! front page for a signed-in member, and confirmations after acting.

use axum::http::{header, StatusCode};
use hic_commons::role::Role;

use crate::common::*;

/// Sign up without publishing; returns the session.
async fn join_only(app: &axum::Router, handle: &str) -> String {
    let r = send(
        app,
        post_form(
            "/join",
            &format!("handle={handle}&display_name=T&passphrase=supersecret1&passphrase_again=supersecret1"),
            None,
        ),
    )
    .await;
    session_from(&r).expect("session")
}

#[tokio::test]
async fn the_next_step_and_tabs_follow_what_has_happened() {
    let (node, _) = server(Role::Node);
    let sid = join_only(&node, "ada").await;
    let page = send(&node, get("/@ada", Some(&sid))).await.text;
    assert!(page.contains("Publish your profile."), "first: publish");
    assert!(!page.contains("?tab=sharing"), "no Sharing tab before there's anything to share");
    let people = send(&node, get("/@ada?tab=people", Some(&sid))).await.text;
    assert!(people.contains("Publish your profile first"), "People points an unpublished member at publishing");

    let sid = join_and_publish(&node, "tega", "public").await;
    let page = send(&node, get("/@tega", Some(&sid))).await.text;
    assert!(page.contains("Share your profile to a commons."), "then: share (no default commons here)");
    assert!(page.contains("/@tega?tab=sharing") && page.contains("/@tega?tab=account"));
    assert!(page.contains("/@tega?tab=people"), "People is always there, so members can find vouching");

    // Before they're listed, People says how to get listed instead of showing forms.
    let people = send(&node, get("/@tega?tab=people", Some(&sid))).await.text;
    assert!(!people.contains("Vouch for someone"));
    assert!(people.contains("Share your profile to get listed"));
    let account = send(&node, get("/@tega?tab=account", Some(&sid))).await.text;
    assert!(account.contains("If this computer is lost") && account.contains("Your key"));
    assert!(account.contains("for=\"backup-pass\""), "the passphrase box has a visible label");
    let sharing = send(&node, get("/@tega?tab=sharing", Some(&sid))).await.text;
    assert!(sharing.contains("Share to a commons"));

    // Visitors see the profile only.
    let visitor = send(&node, get("/@tega", None)).await.text;
    assert!(!visitor.contains("Next step") && !visitor.contains("?tab=account"));
}

#[tokio::test]
async fn not_now_stops_suggesting_a_backup() {
    let (node, store) = server(Role::Node);
    let sid = join_and_publish(&node, "mo", "members").await;
    let page = send(&node, get("/@mo", Some(&sid))).await.text;
    assert!(page.contains("Back up your identity."), "members-only profiles go straight to backing up");
    let csrf = csrf_in(&page);
    let r = send(&node, post_form("/backup/later", &format!("handle=mo&csrf={csrf}"), Some(&sid))).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert!(store.handle("mo").unwrap().unwrap().backup_done);
    assert!(!send(&node, get("/@mo", Some(&sid))).await.text.contains("Back up your identity."));
    // Still on the Account tab.
    assert!(send(&node, get("/@mo?tab=account", Some(&sid))).await.text.contains("Download backup"));
}

#[tokio::test]
async fn downloading_a_backup_counts_as_done() {
    let (node, store) = server(Role::Node);
    let sid = join_and_publish(&node, "mo", "members").await;
    let csrf = csrf_in(&send(&node, get("/@mo?tab=account", Some(&sid))).await.text);
    let r =
        send(&node, post_form("/backup", &format!("handle=mo&csrf={csrf}&passphrase=supersecret1"), Some(&sid))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(store.handle("mo").unwrap().unwrap().backup_done);
}

#[tokio::test]
async fn signed_in_members_get_their_own_next_thing_on_the_front_page() {
    let (node, _) = server(Role::Node);
    let out = send(&node, get("/", None)).await.text;
    assert!(out.contains("Add your profile"));

    let sid = join_and_publish(&node, "tega", "public").await;
    let page = send(&node, get("/", Some(&sid))).await.text;
    assert!(page.contains("Share your profile to a commons"));
    assert!(page.contains("Create another profile for a new commons"));
    assert!(!page.contains(">Add your profile<"), "no button that quietly makes a second identity");

    let join = send(&node, get("/join", Some(&sid))).await.text;
    assert!(join.contains("Create another profile"));
    assert!(join.contains("You're signed in as @tega") || join.contains("You&#39;re signed in as @tega"));
}

#[tokio::test]
async fn licences_are_cards_with_plain_terms() {
    let (node, _) = server(Role::Node);
    let sid = join_and_publish(&node, "tega", "public").await;
    let form = send(&node, get("/new", Some(&sid))).await.text;
    assert!(form.contains("class=\"license-card\""));
    assert!(form.contains("Commercial use"));
    assert!(form.contains("value=\"cc-by\" data-label=\"CC BY 4.0\" checked"), "the current licence is selected");
    assert!(form.contains("What travels with every copy") && form.contains("/rights.js"));
}

/// The room keeps time, not the app: the writing form has no timer.
#[tokio::test]
async fn the_story_form_offers_how_it_travels_and_no_timer() {
    let (node, _) = server(Role::Node);
    assert_eq!(send(&node, get("/timer.js", None)).await.status, StatusCode::NOT_FOUND);
    let sid = join_and_publish(&node, "tega", "public").await;
    let form = send(&node, get("/stories/new", Some(&sid))).await.text;
    assert!(!form.contains("Start 10 minutes") && !form.contains("/timer.js"));
    // "How it travels" is on the second step, the rights page.
    let csrf = csrf_in(&form);
    let r = send(&node, post_form("/stories/new", &format!("handle=tega&csrf={csrf}&title=T&text=Words"), Some(&sid)))
        .await;
    let rights = r.headers.get(header::LOCATION).unwrap().to_str().unwrap().to_string();
    assert!(rights.ends_with("/rights"));
    let page = send(&node, get(&rights, Some(&sid))).await.text;
    assert!(page.contains("How it travels") && page.contains("Only while I'm here"));
    assert!(!page.contains("Coming soon"), "Only while I'm here is built (decision record 0021)");
}

#[tokio::test]
async fn a_story_page_confirms_what_just_happened() {
    let (node, _) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let csrf = csrf_in(&send(&node, get("/stories/new", Some(&sid))).await.text);
    let body = format!("handle=ada&csrf={csrf}&title=Night+bus&text=Hello&visibility=public&license=cc-by");
    let r = send(&node, post_form("/stories/new", &body, Some(&sid))).await;
    let loc = r.headers.get(header::LOCATION).unwrap().to_str().unwrap().trim_end_matches("/rights").to_string();

    let gone = send(&node, get(&format!("{loc}?done=withdrawn&at=room.local:8081"), Some(&sid))).await.text;
    assert!(gone.contains("Your story is gone from room.local:8081"));
    let shared = send(&node, get(&format!("{loc}?done=shared&at=room.local:8081"), Some(&sid))).await.text;
    assert!(shared.contains("Your story is on room.local:8081"));
    // Made-up addresses or outcomes show nothing.
    let odd = send(&node, get(&format!("{loc}?done=withdrawn&at=%3Cb%3Ehi"), Some(&sid))).await.text;
    assert!(!odd.contains("flash-withdrawn"));
    // And only the author sees it.
    assert!(!send(&node, get(&format!("{loc}?done=shared&at=room.local:8081"), None))
        .await
        .text
        .contains("Your story is on"));
}
