//! Vouching with a click (decision record 0026): a commons shows its own
//! members who is waiting for a vouch, to a signed request, and nobody else;
//! a member's node shows that list on the People tab with a Vouch button.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::Router;
use serde_json::json;

use crate::common::{
    closed_commons, csrf_in, get, join_and_publish, person, post_form, post_json, send, server, share_of, COMMONS_URL,
};
use hic_commons::app::{router, AppState};
use hic_commons::config::Config;
use hic_commons::model::HandleRec;
use hic_commons::role::Role;
use hic_commons::store::Store;
use hic_commons::vouching::{make_vouch, make_waiting_request, REQUEST_FRESH_FOR};

/// A closed commons with `member` listed (by the founding invite) and
/// `newcomer` waiting for a vouch.
async fn commons_with_one_waiting(member: &HandleRec, newcomer: &HandleRec) -> (Router, Store) {
    let (commons, store) = closed_commons(5);
    store.save_founding_code("hic-founding-oc1").unwrap();
    assert_eq!(send(&commons, post_json("/inbox", &share_of(member, Some("hic-founding-oc1")))).await.text, "accepted");
    let r = send(&commons, post_json("/inbox", &share_of(newcomer, None))).await;
    assert!(r.text.starts_with("pending"));
    (commons, store)
}

/// `who`'s signed waiting-list request for the test commons, made at `at`.
fn request_by(who: &HandleRec, commons_url: &str, at: u64) -> serde_json::Value {
    serde_json::to_value(make_waiting_request(&who.ed_pub, &who.ed_priv, commons_url, at).unwrap()).unwrap()
}

#[tokio::test]
async fn a_listed_member_sees_who_is_waiting_and_nothing_more() {
    let (member, amy) = (person("member"), person("amy"));
    let (commons, _) = commons_with_one_waiting(&member, &amy).await;
    let r = send(&commons, post_json("/api/waiting", &request_by(&member, COMMONS_URL, hic_commons::now()))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.headers.get("cache-control").unwrap(), "no-store");
    let answer: serde_json::Value = serde_json::from_str(&r.text).unwrap();
    assert_eq!(answer, json!({ "waiting": [{ "handle": "amy", "name": "amy", "key": amy.ed_pub }] }));
}

#[tokio::test]
async fn a_stranger_cannot_read_the_waiting_list() {
    let (member, amy) = (person("member"), person("amy"));
    let (commons, _) = commons_with_one_waiting(&member, &amy).await;
    let stranger = person("stranger");
    let r = send(&commons, post_json("/api/waiting", &request_by(&stranger, COMMONS_URL, hic_commons::now()))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(!r.text.contains(&amy.ed_pub) && !r.text.contains("amy"));
}

#[tokio::test]
async fn someone_still_waiting_cannot_read_the_waiting_list() {
    let (member, amy) = (person("member"), person("amy"));
    let (commons, _) = commons_with_one_waiting(&member, &amy).await;
    let r = send(&commons, post_json("/api/waiting", &request_by(&amy, COMMONS_URL, hic_commons::now()))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "waiting is not being listed");
}

#[tokio::test]
async fn a_member_of_a_different_commons_cannot_read_this_waiting_list() {
    let (member, amy) = (person("member"), person("amy"));
    let (here, _) = commons_with_one_waiting(&member, &amy).await;
    // Someone listed only on another commons, asking there and here.
    let (elsewhere, other_store) = closed_commons(5);
    other_store.save_founding_code("hic-founding-oc2").unwrap();
    let outsider = person("outsider");
    send(&elsewhere, post_json("/inbox", &share_of(&outsider, Some("hic-founding-oc2")))).await;
    let other_url = "http://other.example";
    // Their request addressed to the other commons, replayed here.
    let replayed = send(&here, post_json("/api/waiting", &request_by(&outsider, other_url, hic_commons::now()))).await;
    assert_eq!(replayed.status, StatusCode::FORBIDDEN);
    // And one addressed here: they aren't listed here.
    let direct = send(&here, post_json("/api/waiting", &request_by(&outsider, COMMONS_URL, hic_commons::now()))).await;
    assert_eq!(direct.status, StatusCode::FORBIDDEN);
    assert!(direct.text.contains("isn't a member"));
    // A member here can't use a request made for another commons either.
    let misaddressed =
        send(&here, post_json("/api/waiting", &request_by(&member, other_url, hic_commons::now()))).await;
    assert_eq!(misaddressed.status, StatusCode::FORBIDDEN);
    assert!(misaddressed.text.contains("different commons"));
}

#[tokio::test]
async fn an_old_or_forged_waiting_list_request_is_refused() {
    let (member, amy) = (person("member"), person("amy"));
    let (commons, _) = commons_with_one_waiting(&member, &amy).await;
    let old = request_by(&member, COMMONS_URL, hic_commons::now() - REQUEST_FRESH_FOR - 60);
    let r = send(&commons, post_json("/api/waiting", &old)).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(r.text.contains("too old"));
    // A stranger's request with the member's key written in as its author.
    let stranger = person("stranger");
    let mut forged = request_by(&stranger, COMMONS_URL, hic_commons::now());
    forged["envelope"]["author"] = json!(member.ed_pub);
    assert_eq!(send(&commons, post_json("/api/waiting", &forged)).await.status, StatusCode::FORBIDDEN);
    // A member's vouch is not a waiting-list request.
    let vouch =
        make_vouch(&member.ed_pub, &member.ed_priv, COMMONS_URL, "amy", &amy.ed_pub, hic_commons::now()).unwrap();
    let r = send(&commons, post_json("/api/waiting", &serde_json::to_value(&vouch).unwrap())).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // Not a signed object at all, and no request body.
    assert_eq!(send(&commons, post_json("/api/waiting", &json!({"hello": 1}))).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(send(&commons, get("/api/waiting", None)).await.status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn the_waiting_list_is_rate_limited_like_the_inbox() {
    let (member, amy) = (person("member"), person("amy"));
    let (commons, _) = commons_with_one_waiting(&member, &amy).await;
    // The two shares above already used two of this address's 120 a minute.
    let req = request_by(&member, COMMONS_URL, hic_commons::now());
    for _ in 0..(hic_commons::app::INBOX_PER_MINUTE - 2) {
        assert_eq!(send(&commons, post_json("/api/waiting", &req)).await.status, StatusCode::OK);
    }
    assert_eq!(send(&commons, post_json("/api/waiting", &req)).await.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_node_has_no_waiting_list() {
    let (node, _) = server(Role::Node);
    let m = person("member");
    let r = send(&node, post_json("/api/waiting", &request_by(&m, COMMONS_URL, hic_commons::now()))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_vouch_for_someone_not_waiting_is_refused() {
    let (member, amy) = (person("member"), person("amy"));
    let (commons, store) = commons_with_one_waiting(&member, &amy).await;
    // Right handle, wrong key: not the person who is waiting.
    let imposter = person("amy");
    let v =
        make_vouch(&member.ed_pub, &member.ed_priv, COMMONS_URL, "amy", &imposter.ed_pub, hic_commons::now()).unwrap();
    let r = send(&commons, post_json("/inbox", &serde_json::to_value(&v).unwrap())).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    // Someone who never shared at all.
    let v = make_vouch(&member.ed_pub, &member.ed_priv, COMMONS_URL, "ghost", &imposter.ed_pub, hic_commons::now())
        .unwrap();
    assert_eq!(
        send(&commons, post_json("/inbox", &serde_json::to_value(&v).unwrap())).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(store.pending().len(), 1, "amy is still waiting");
    assert!(store.remotes().iter().all(|r| r.handle != "amy"));
}

/// A closed commons listening on a real port on this computer, so a node can
/// reach it over the network. Returns its address, its router (for sending
/// to it in process too) and its store.
async fn commons_on_a_port() -> (String, Router, Store) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{port}");
    let dir = std::env::temp_dir().join(format!("hic-http-oneclick-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    // Its own address is the one the node uses, so requests name it.
    let (base, port_s) = (url.clone(), port.to_string());
    let config = Config::from_lookup(move |k| match k {
        "HIC_BASE_URL" => Some(base.clone()),
        "PORT" => Some(port_s.clone()),
        _ => None,
    });
    let app = router(Arc::new(AppState::new(config, Role::Commons, store.clone())));
    let serving = app.clone().into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, serving).await.unwrap() });
    (url, app, store)
}

/// The value of the hidden field `name` in the first form in `html` that
/// has it.
fn field(html: &str, name: &str) -> String {
    let marker = format!("name=\"{name}\" value=\"");
    let at = html.find(&marker).unwrap_or_else(|| panic!("no {name} field")) + marker.len();
    html[at..].split('"').next().unwrap().to_string()
}

/// The whole flow over a real connection: a member's node shows who is
/// waiting on its People tab, a click vouches, and the commons lists them.
/// The Vouch form needs the session's CSRF token.
#[tokio::test]
async fn a_member_vouches_with_one_click_from_their_node() {
    let (commons_url, commons, commons_store) = commons_on_a_port().await;
    commons_store.save_founding_code("hic-founding-oc3").unwrap();

    // Tega joins on their node and is listed on the commons.
    let (node, node_store) = server(Role::Node);
    let sid = join_and_publish(&node, "tega", "public").await;
    let csrf = csrf_in(&send(&node, get("/new", Some(&sid))).await.text);
    let share = format!("handle=tega&csrf={csrf}&commons_url={commons_url}&invite=hic-founding-oc3");
    assert!(send(&node, post_form("/share", &share, Some(&sid))).await.text.contains("federated to"));

    // Amy (from some other node) waits for a vouch.
    let amy = person("amy");
    send(&commons, post_json("/inbox", &share_of(&amy, None))).await;

    // Tega's People tab shows her, with a Vouch button.
    let page = send(&node, get("/@tega?tab=people", Some(&sid))).await.text;
    assert!(page.contains("Waiting for a vouch") && page.contains("Vouch for @amy"), "{page}");
    assert!(page.contains(&amy.ed_pub[..12]), "her key's fingerprint is shown");
    let key = field(&page, "key");
    assert_eq!(key, amy.ed_pub);

    // The same press without the CSRF token does nothing.
    let no_token = format!("handle=tega&commons_url={commons_url}&who=amy&key={key}");
    let r = send(&node, post_form("/vouch", &no_token, Some(&sid))).await;
    assert!(r.text.contains("sign in"));
    let wrong_token = format!("handle=tega&csrf=nope&commons_url={commons_url}&who=amy&key={key}");
    assert!(send(&node, post_form("/vouch", &wrong_token, Some(&sid))).await.text.contains("sign in"));
    assert_eq!(commons_store.pending().len(), 1, "still waiting");

    // With it, one click lists her.
    let press = format!("handle=tega&csrf={}&commons_url={commons_url}&who=amy&key={key}", field(&page, "csrf"));
    let r = send(&node, post_form("/vouch", &press, Some(&sid))).await;
    assert!(r.text.contains("@amy is now listed on 127.0.0.1"), "{}", r.text);
    assert!(commons_store.pending().is_empty());
    let intro = commons_store.intros().into_iter().find(|i| i.handle == "amy").unwrap();
    let tega_key = node_store.handle("tega").unwrap().unwrap().ed_pub;
    assert_eq!(intro.voucher.as_deref(), Some(tega_key.as_str()), "recorded as tega's vouch");

    // Pressing again: she isn't waiting any more, said in plain words.
    let r = send(&node, post_form("/vouch", &press, Some(&sid))).await;
    assert!(r.text.contains("Not vouched") && r.text.contains("isn't waiting on 127.0.0.1"), "{}", r.text);

    // And the list is now empty.
    let page = send(&node, get("/@tega?tab=people", Some(&sid))).await.text;
    assert!(page.contains("Nobody is waiting on 127.0.0.1"));
}

/// A commons that can't be reached gets a clear message, not an error page.
#[tokio::test]
async fn an_unreachable_commons_is_named_on_the_people_tab() {
    let (node, node_store) = server(Role::Node);
    let sid = join_and_publish(&node, "tega", "public").await;
    // Listed (as far as the node knows) on a commons that isn't running:
    // port 1 on this computer refuses connections straight away.
    let rec = node_store.handle("tega").unwrap().unwrap().with_shared_to("http://127.0.0.1:1");
    node_store.save_handle(&rec).unwrap();
    let page = send(&node, get("/@tega?tab=people", Some(&sid))).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text.contains("Couldn't reach 127.0.0.1:1"), "{}", page.text);
}
