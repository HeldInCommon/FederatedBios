//! Sharing terms for research (decision record 0029): a commons keeps each
//! gift as a file in `terms/`, a node sends one only when its author ticks
//! the box, and a gift never carries who they are or what they wrote.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::http::{header, StatusCode};
use axum::Router;
use hic_commons::app::{router, AppState};
use hic_commons::config::Config;
use hic_commons::role::Role;
use hic_commons::store::Store;
use serde_json::{json, Value};

use crate::common::*;

/// A gift as a node sends it.
fn gift() -> Value {
    json!({
        "format": "hic-terms-research/1",
        "about_you": "A member of a local art collective",
        "own_terms": "Open research only, never to imitate my voice.",
        "withdraw_after_training": true,
        "visibility": "commons",
        "license": "cc-by-nc",
        "signals": ["credit"],
        "travel": "copy",
    })
}

/// Every gift a commons has kept, read back from its `terms/` folder.
fn kept(store: &Store) -> Vec<Value> {
    let dir = store.root().join("terms");
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| serde_json::from_slice(&std::fs::read(e.path()).unwrap()).unwrap())
        .collect()
}

#[tokio::test]
async fn a_commons_keeps_a_gift_of_terms_as_a_file() {
    let (commons, store) = server(Role::Commons);
    let r = send(&commons, post_json("/research/terms", &gift())).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    let files = kept(&store);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["own_terms"], "Open research only, never to imitate my voice.");
    assert_eq!(files[0]["about_you"], "A member of a local art collective");
    assert!(files[0]["received_at"].as_u64().unwrap() > 0, "when it arrived");
    assert!(files[0].get("sender").is_none() && files[0].get("ip").is_none(), "not who sent it");
}

#[tokio::test]
async fn a_commons_refuses_a_gift_that_carries_more_than_the_terms() {
    let (commons, store) = server(Role::Commons);
    let mut with_handle = gift();
    with_handle["handle"] = "ada".into();
    assert_eq!(send(&commons, post_json("/research/terms", &with_handle)).await.status, StatusCode::BAD_REQUEST);
    let mut odd_licence = gift();
    odd_licence["license"] = "all-rights-reserved".into();
    assert_eq!(send(&commons, post_json("/research/terms", &odd_licence)).await.status, StatusCode::BAD_REQUEST);
    assert!(kept(&store).is_empty(), "nothing refused is kept");
}

#[tokio::test]
async fn a_node_takes_no_gifts() {
    let (node, _) = server(Role::Node);
    assert_eq!(send(&node, post_json("/research/terms", &gift())).await.status, StatusCode::NOT_FOUND);
}

/// A commons listening on a real port on this computer, so a node can send
/// to it. Returns its address and store.
async fn commons_on_a_port() -> (String, Store) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{port}");
    let dir = std::env::temp_dir().join(format!("hic-http-research-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let (base, port_s) = (url.clone(), port.to_string());
    let config = Config::from_lookup(move |k| match k {
        "HIC_BASE_URL" => Some(base.clone()),
        "PORT" => Some(port_s.clone()),
        _ => None,
    });
    let app: Router = router(Arc::new(AppState::new(config, Role::Commons, store.clone())));
    let serving = app.into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, serving).await.unwrap() });
    (url, store)
}

/// Write a story on `node` as `handle`; returns its id and the CSRF token.
async fn write_story(node: &Router, sid: &str, handle: &str) -> (String, String) {
    let csrf = csrf_in(&send(node, get("/stories/new", Some(sid))).await.text);
    let body = format!("handle={handle}&csrf={csrf}&title=Bins+at+six&text=The+lorry+comes+before+the+light.");
    let r = send(node, post_form("/stories/new", &body, Some(sid))).await;
    let loc = r.headers.get(header::LOCATION).unwrap().to_str().unwrap().to_string();
    let id = loc.strip_prefix("/story/").and_then(|r| r.strip_suffix("/rights")).unwrap().to_string();
    (id, csrf)
}

#[tokio::test]
async fn ticking_the_box_sends_only_the_terms_to_the_research_address() {
    let (research_url, research_store) = commons_on_a_port().await;
    let (node, _) = server_with(Role::Node, &[("HIC_RESEARCH_URL", &research_url)]);
    let sid = join_and_publish(&node, "ada", "public").await;
    let (id, csrf) = write_story(&node, &sid, "ada").await;

    let page = send(&node, get(&format!("/story/{id}/rights"), Some(&sid))).await.text;
    assert!(page.contains("Share these terms with the Held in Common project for research"));
    assert!(page.contains("only interested in the terms, not your data"), "says what isn't sent");
    assert!(!page.contains("127.0.0.1:"), "names where it goes, never its address");
    assert!(page.contains("describe yourself"), "asks how they'd describe themselves");
    assert!(!page.contains(r#"name="research" value="yes" checked"#), "never ticked to begin with");

    let body = format!(
        "csrf={csrf}&visibility=public&license=cc-by-sa&signals=credit&signals=open&own_terms=Ask+me+first.\
         &travel=here&research=yes&about_you=An+artist+sharing+work+with+a+collaborator&action=keep"
    );
    let r = send(&node, post_form(&format!("/story/{id}/rights"), &body, Some(&sid))).await;
    assert_eq!(r.headers.get(header::LOCATION).unwrap(), &format!("/story/{id}?done=saved&research=sent"));

    let files = kept(&research_store);
    assert_eq!(files.len(), 1, "one copy, kept on the research commons");
    let g = &files[0];
    assert_eq!(g["own_terms"], "Ask me first.");
    assert_eq!(g["about_you"], "An artist sharing work with a collaborator");
    assert_eq!(
        (g["visibility"].as_str(), g["license"].as_str(), g["travel"].as_str()),
        (Some("public"), Some("cc-by-sa"), Some("here"))
    );
    assert_eq!(g["signals"], json!(["credit", "open"]));
    let text = g.to_string();
    for private in ["ada", "Bins at six", "lorry", id.as_str()] {
        assert!(!text.contains(private), "the gift doesn't carry {private:?}");
    }

    let story = send(&node, get(&format!("/story/{id}?done=saved&research=sent"), Some(&sid))).await.text;
    assert!(story.contains("Shared for research"));
}

#[tokio::test]
async fn without_the_box_ticked_nothing_is_sent() {
    let (research_url, research_store) = commons_on_a_port().await;
    let (node, _) = server_with(Role::Node, &[("HIC_RESEARCH_URL", &research_url)]);
    let sid = join_and_publish(&node, "ada", "public").await;
    let (id, csrf) = write_story(&node, &sid, "ada").await;
    let body = format!("csrf={csrf}&visibility=public&license=cc-by&own_terms=Mine.&about_you=An+artist&action=keep");
    let r = send(&node, post_form(&format!("/story/{id}/rights"), &body, Some(&sid))).await;
    assert_eq!(r.headers.get(header::LOCATION).unwrap(), &format!("/story/{id}?done=saved"));
    assert!(kept(&research_store).is_empty());
}

#[tokio::test]
async fn an_unreachable_research_address_still_saves_and_says_so() {
    // Port 9 (discard) on this computer: nothing answers.
    let (node, store) = server_with(Role::Node, &[("HIC_RESEARCH_URL", "http://127.0.0.1:9")]);
    let sid = join_and_publish(&node, "ada", "public").await;
    let (id, csrf) = write_story(&node, &sid, "ada").await;
    let body = format!("csrf={csrf}&visibility=public&license=cc-by-nc&research=yes&action=keep");
    let r = send(&node, post_form(&format!("/story/{id}/rights"), &body, Some(&sid))).await;
    assert_eq!(r.headers.get(header::LOCATION).unwrap(), &format!("/story/{id}?done=saved&research=not-sent"));
    assert_eq!(store.own_story(&id).unwrap().unwrap().story.license, "cc-by-nc", "the terms are saved anyway");
    let page = send(&node, get(&format!("/story/{id}?done=saved&research=not-sent"), Some(&sid))).await.text;
    assert!(page.contains("Not shared for research") && page.contains("Nothing was sent"));
}

#[tokio::test]
async fn with_nowhere_to_send_the_option_is_not_offered() {
    let (node, _) = server(Role::Node);
    let sid = join_and_publish(&node, "ada", "public").await;
    let (id, _) = write_story(&node, &sid, "ada").await;
    let page = send(&node, get(&format!("/story/{id}/rights"), Some(&sid))).await.text;
    assert!(!page.contains("for research"));
}
