//! "Only while I'm here" (decision record 0021): a commons lists a signed
//! card, fetches the words from the author's node over the node's own long
//! poll, checks them against the signed hash, and keeps nothing. The node
//! side (sharing a card, the "not supported yet" message) is at the end.
//!
//! A node's polls are played here by the test itself, signing with the
//! author's key, so each step can be checked in turn.

use std::time::Duration;

use axum::http::{header, StatusCode};
use axum::Router;
use hic_commons::here::{self, Answer, PollReply};
use hic_commons::model::HandleRec;
use hic_commons::role::Role;
use hic_commons::story;
use serde_json::Value;

use crate::common::*;

/// A well-formed story id.
const ID: &str = "0123456789abcdef";
/// The story's words: only the first line goes in the card.
const TEXT: &str = "The 38 sighs at the stop.\nA dog answers from a balcony.";

/// A commons with short relay timings (a 2 s hold, a 3 s reader wait) and
/// `ada` listed on it, holding ada's card for the story above.
async fn commons_with_adas_card() -> (Router, hic_commons::store::Store, HandleRec) {
    let (commons, store) = server_with(Role::Commons, &[("HIC_RELAY_HOLD", "2"), ("HIC_RELAY_WAIT", "3")]);
    let ada = person("ada");
    assert!(send(&commons, post_json("/inbox", &share_of(&ada, None))).await.status.is_success());
    let r = send(&commons, post_json("/here/card", &card_of(&ada, 100))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    (commons, store, ada)
}

/// `rec`'s signed card for the story above, dated `at`, for this commons.
fn card_of(rec: &HandleRec, at: u64) -> Value {
    let s = story::new_story(ID, &rec.handle, "Night bus", TEXT, "commons", "cc-by", &[]).unwrap();
    let card = here::card_for(&s, COMMONS_URL);
    here::card_message(&here::sign_card(&card, &rec.ed_pub, &rec.ed_priv, at).unwrap())
}

/// One poll by `rec`, numbered `seq`, carrying `answers`; returns the
/// status and, on success, the requests handed back.
async fn poll(commons: &Router, rec: &HandleRec, seq: u64, answers: &[Answer]) -> (StatusCode, PollReply) {
    let msg = here::build_relay(&rec.ed_pub, &rec.ed_priv, COMMONS_URL, seq, answers, hic_commons::now()).unwrap();
    let r = send(commons, post_json("/here/poll", &msg)).await;
    let reply = serde_json::from_str(&r.text).unwrap_or_default();
    (r.status, reply)
}

/// Play `rec`'s node for one reader: poll, wait for a request, answer it
/// with `text`, and poll once more to deliver the answer.
async fn answer_once(commons: Router, rec: HandleRec, text: &'static str) {
    let (status, first) = poll(&commons, &rec, 1, &[]).await;
    assert_eq!(status, StatusCode::OK);
    let answers: Vec<Answer> = first
        .requests
        .iter()
        .map(|r| Answer { request: r.request.clone(), story: r.story.clone(), text: text.into() })
        .collect();
    poll(&commons, &rec, 2, &answers).await;
}

/// The page for ada's story on the commons.
fn story_path(rec: &HandleRec) -> String {
    format!("/stories/{}/{ID}", hic_commons::keys::keyid(&rec.ed_pub))
}

/// True when any file under the commons' data folder contains `needle`:
/// how the tests check the commons never wrote the words down.
fn data_folder_contains(store: &hic_commons::store::Store, needle: &str) -> bool {
    /// True when any file under `dir`, or its folders, contains `needle`.
    fn walk(dir: &std::path::Path, needle: &str) -> bool {
        std::fs::read_dir(dir).into_iter().flatten().flatten().any(|e| {
            let p = e.path();
            if p.is_dir() {
                walk(&p, needle)
            } else {
                std::fs::read_to_string(&p).is_ok_and(|t| t.contains(needle))
            }
        })
    }
    walk(store.root(), needle)
}

#[tokio::test]
async fn a_card_is_listed_and_readers_see_not_here_right_now_while_the_author_is_away() {
    let (commons, store, ada) = commons_with_adas_card().await;
    let list = send(&commons, get("/stories", None)).await.text;
    assert!(list.contains("Night bus") && list.contains("Only while @ada is here"));
    let page = send(&commons, get(&story_path(&ada), None)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text.contains("Not here right now: @ada shares this only while they're online."));
    assert!(page.text.contains("The 38 sighs at the stop.") && !page.text.contains("A dog answers"));
    assert!(!data_folder_contains(&store, "A dog answers"), "the commons has only the card");
}

#[tokio::test]
async fn a_reader_gets_the_words_from_the_authors_node_and_nothing_is_kept() {
    let (commons, store, ada) = commons_with_adas_card().await;
    let node = tokio::spawn(answer_once(commons.clone(), ada.clone(), TEXT));
    // Let the node's first poll open before the reader arrives.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let page = send(&commons, get(&story_path(&ada), None)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text.contains("A dog answers from a balcony."), "the words, fetched just now");
    assert!(page.text.contains("Checked: the words match the SHA-256"));
    assert_eq!(page.headers.get(header::CACHE_CONTROL).unwrap(), "no-store", "no caching anywhere");
    node.await.unwrap();
    assert!(!data_folder_contains(&store, "A dog answers"), "passed through, never written down");
}

#[tokio::test]
async fn text_that_does_not_match_the_signed_hash_is_never_shown() {
    let (commons, _, ada) = commons_with_adas_card().await;
    let node = tokio::spawn(answer_once(commons.clone(), ada.clone(), "The 38 sighs at the stop.\nA cat answers."));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let page = send(&commons, get(&story_path(&ada), None)).await.text;
    assert!(!page.contains("A cat answers"), "altered words are caught");
    assert!(page.contains("has changed this story since sharing it here"));
    node.await.unwrap();
}

/// A commons could alter the words it passes on; a reader's own node (or a
/// script) catches it with the signed card, which it can't alter.
#[tokio::test]
async fn a_commons_altering_the_words_is_caught_by_the_reader() {
    let (commons, _, ada) = commons_with_adas_card().await;
    let node = tokio::spawn(answer_once(commons.clone(), ada.clone(), TEXT));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let r = send(&commons, get(&format!("{}/here.json", story_path(&ada)), None)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    node.await.unwrap();
    let got: Value = serde_json::from_str(&r.text).unwrap();
    // The reader's side: check the card is ada's, then the words against it.
    let signed: hic_commons::envelope::Signed = serde_json::from_value(got["card"].clone()).unwrap();
    hic_commons::envelope::verify(&signed).unwrap();
    assert_eq!(signed.envelope.author, ada.ed_pub);
    let card: here::Card = serde_json::from_value(signed.envelope.body.clone()).unwrap();
    assert!(here::text_matches(&card, got["text"].as_str().unwrap()), "honest: matches");
    // A dishonest commons changes the words: caught.
    assert!(!here::text_matches(&card, "The 38 sighs at the stop.\nA cat answers."));
    // Or changes the hash in the card to match its words: the signature breaks.
    let mut forged = signed.clone();
    forged.envelope.body["text_sha256"] = serde_json::json!(here::text_sha256("forged"));
    assert!(hic_commons::envelope::verify(&forged).is_err());
}

#[tokio::test]
async fn another_node_cannot_answer_for_an_author() {
    let (commons, _, ada) = commons_with_adas_card().await;
    // Bea is listed too and shares her own card, so she may poll.
    let bea = person("bea");
    assert!(send(&commons, post_json("/inbox", &share_of(&bea, None))).await.status.is_success());
    assert_eq!(send(&commons, post_json("/here/card", &card_of(&bea, 100))).await.status, StatusCode::OK);

    // Ada's node collects the request but goes quiet; Bea, who knows the
    // words, answers it with her own key.
    let (c2, a2) = (commons.clone(), ada.clone());
    let ada_node = tokio::spawn(async move { poll(&c2, &a2, 1, &[]).await.1 });
    tokio::time::sleep(Duration::from_millis(300)).await;
    let reader = tokio::spawn({
        let c = commons.clone();
        let path = story_path(&ada);
        async move { send(&c, get(&path, None)).await.text }
    });
    let handed = ada_node.await.unwrap();
    assert_eq!(handed.requests.len(), 1, "ada's node was asked");
    let forged = Answer { request: handed.requests[0].request.clone(), story: ID.into(), text: TEXT.into() };
    poll(&commons, &bea, 1, &[forged]).await;
    let page = reader.await.unwrap();
    assert!(!page.contains("A dog answers"), "Bea's answer is ignored");
    assert!(page.contains("Not here right now"));

    // Someone with no card here can't hold a poll open at all.
    let sam = person("sam");
    assert_eq!(poll(&commons, &sam, 1, &[]).await.0, StatusCode::FORBIDDEN);
    // Nor can a poll claim Ada's key with someone else's signature.
    let mut claimed = here::build_relay(&bea.ed_pub, &bea.ed_priv, COMMONS_URL, 9, &[], hic_commons::now()).unwrap();
    claimed["relay"]["envelope"]["author"] = serde_json::json!(ada.ed_pub);
    assert_eq!(send(&commons, post_json("/here/poll", &claimed)).await.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_old_poll_cannot_be_replayed() {
    let (commons, _, ada) = commons_with_adas_card().await;
    assert_eq!(poll(&commons, &ada, 5, &[]).await.0, StatusCode::OK);
    assert_eq!(poll(&commons, &ada, 5, &[]).await.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_reader_cannot_fetch_a_withdrawn_story() {
    let (commons, store, ada) = commons_with_adas_card().await;
    let w = story::build_withdraw(&ada.ed_pub, &ada.ed_priv, ID, COMMONS_URL, hic_commons::now()).unwrap();
    assert_eq!(send(&commons, post_json("/inbox", &w)).await.status, StatusCode::OK);
    assert!(store.story_cards().is_empty(), "the card is deleted at once");
    assert_eq!(send(&commons, get(&story_path(&ada), None)).await.status, StatusCode::NOT_FOUND);
    let json = send(&commons, get(&format!("{}/here.json", story_path(&ada)), None)).await;
    assert_eq!(json.status, StatusCode::NOT_FOUND);
    // The author's node, still polling, is told it has nothing here.
    assert_eq!(poll(&commons, &ada, 1, &[]).await.0, StatusCode::FORBIDDEN);
    let list = send(&commons, get("/stories", None)).await.text;
    assert!(list.contains("Withdrawn by @ada") && !list.contains("Night bus"));
    // And the withdrawn card can't be sent back in.
    assert_eq!(send(&commons, post_json("/here/card", &card_of(&ada, 100))).await.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn cards_follow_the_same_gate_as_stories() {
    let (commons, _) = server(Role::Commons);
    let stranger = person("sam");
    assert_eq!(send(&commons, post_json("/here/card", &card_of(&stranger, 100))).await.status, StatusCode::FORBIDDEN);
    // A card addressed to another commons is refused.
    let ada = person("ada");
    assert!(send(&commons, post_json("/inbox", &share_of(&ada, None))).await.status.is_success());
    let s = story::new_story(ID, "ada", "Night bus", TEXT, "commons", "cc-by", &[]).unwrap();
    let elsewhere = here::card_for(&s, "http://elsewhere:8081");
    let msg = here::card_message(&here::sign_card(&elsewhere, &ada.ed_pub, &ada.ed_priv, 100).unwrap());
    assert_eq!(send(&commons, post_json("/here/card", &msg)).await.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_newer_copy_replaces_a_card_and_a_newer_card_replaces_a_copy() {
    let (commons, store, ada) = commons_with_adas_card().await;
    let s = story::new_story(ID, "ada", "Night bus", TEXT, "commons", "cc-by", &[]).unwrap();
    let copy = |at| story::share_message(&story::sign(&s, &ada.ed_pub, &ada.ed_priv, at).unwrap());
    assert_eq!(send(&commons, post_json("/inbox", &copy(100))).await.status, StatusCode::CONFLICT, "not newer");
    assert_eq!(send(&commons, post_json("/inbox", &copy(150))).await.status, StatusCode::OK);
    assert!(store.story_cards().is_empty() && store.held_stories().len() == 1);
    assert_eq!(send(&commons, post_json("/here/card", &card_of(&ada, 200))).await.status, StatusCode::OK);
    assert!(store.held_stories().is_empty() && store.story_cards().len() == 1, "the copy is gone");
}

// ---- the node's side --------------------------------------------------------

/// On a node: sign up `handle`, write a story, choose "Only while I'm here"
/// and keep it; returns the session, CSRF token and story id.
async fn a_story_to_share_only_while_here(node: &Router, handle: &str) -> (String, String, String) {
    let sid = join_and_publish(node, handle, "public").await;
    let csrf = csrf_in(&send(node, get("/stories/new", Some(&sid))).await.text);
    let body = format!("handle={handle}&csrf={csrf}&title=Night+bus&text=The+38+sighs.");
    let r = send(node, post_form("/stories/new", &body, Some(&sid))).await;
    let loc = r.headers.get(header::LOCATION).unwrap().to_str().unwrap().to_string();
    let id = loc.strip_prefix("/story/").and_then(|r| r.strip_suffix("/rights")).unwrap().to_string();
    let body = format!("csrf={csrf}&visibility=commons&license=cc-by&travel=here&action=keep");
    send(node, post_form(&format!("/story/{id}/rights"), &body, Some(&sid))).await;
    (sid, csrf, id)
}

#[tokio::test]
async fn the_travel_choice_is_saved_and_offered_again() {
    let (node, store) = server(Role::Node);
    let (sid, _, id) = a_story_to_share_only_while_here(&node, "ada").await;
    assert_eq!(store.own_story(&id).unwrap().unwrap().travel, story::Travel::Here);
    let rights = send(&node, get(&format!("/story/{id}/rights"), Some(&sid))).await.text;
    assert!(rights.contains(r#"value="here" data-label="Only while I'm here" checked"#));
}

/// A commons from before "Only while I'm here" has no `/here/card` and
/// answers 404. Played by a bare server on this computer (loopback only),
/// since the node sends real requests.
#[tokio::test]
async fn a_commons_that_predates_it_is_named_plainly() {
    let old = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let old_url = format!("http://{}", old.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(old, Router::new()).await.unwrap() });

    let (node, store) = server(Role::Node);
    let (sid, csrf, id) = a_story_to_share_only_while_here(&node, "ada").await;
    let body = format!("id={id}&csrf={csrf}&commons_url={old_url}");
    let r = send(&node, post_form("/story/share", &body, Some(&sid))).await;
    assert!(r.text.contains("doesn't support Only while I'm here yet"), "{}", r.text);
    assert!(store.own_story(&id).unwrap().unwrap().shared_to.is_empty(), "nothing recorded as shared");
}
