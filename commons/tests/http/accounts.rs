//! A member's account on a node: passphrases, sign-in limits, backups.

use crate::common::{csrf_in, get, join_and_publish, post_form, send, server, session_from};
use axum::http::header;
use axum::http::StatusCode;
use hic_commons::role::Role;

#[tokio::test]
async fn new_passphrases_use_argon2() {
    let (app, store) = server(Role::Node);
    join_and_publish(&app, "tega", "public").await;
    assert!(store.handle("tega").unwrap().unwrap().secret_hash.starts_with("$argon2id$"));
}

#[tokio::test]
async fn wrong_passphrases_are_limited() {
    let (app, _) = server(Role::Node);
    join_and_publish(&app, "tega", "public").await;
    for _ in 0..5 {
        let r = send(&app, post_form("/login", "handle=tega&passphrase=wrongwrong", None)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    // Even the right passphrase waits now.
    let r = send(&app, post_form("/login", "handle=tega&passphrase=supersecret1", None)).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn damaged_identity_is_never_overwritten_by_sign_up() {
    let (app, store) = server(Role::Node);
    std::fs::write(store.root().join("handles/tega.json"), b"{\"handle\":\"te").unwrap();
    let r = send(&app, post_form("/join", "handle=tega&display_name=T&passphrase=supersecret1", None)).await;
    assert_eq!(r.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(std::fs::read(store.root().join("handles/tega.json")).unwrap(), b"{\"handle\":\"te");
}

#[tokio::test]
async fn backup_restores_the_same_identity_on_another_node() {
    let (a, astore) = server(Role::Node);
    let sid = join_and_publish(&a, "tega", "public").await;
    let csrf = csrf_in(&send(&a, get("/@tega", Some(&sid))).await.text);

    let wrong =
        send(&a, post_form("/backup", &format!("handle=tega&csrf={csrf}&passphrase=nopenope"), Some(&sid))).await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    let file =
        send(&a, post_form("/backup", &format!("handle=tega&csrf={csrf}&passphrase=supersecret1"), Some(&sid))).await;
    assert_eq!(file.status, StatusCode::OK);
    assert!(file.headers.get(header::CONTENT_DISPOSITION).unwrap().to_str().unwrap().contains("attachment"));
    let original = astore.handle("tega").unwrap().unwrap();
    assert!(!file.text.contains(&original.ed_priv), "keys are encrypted in the file");

    let (b, bstore) = server(Role::Node);
    let encoded: String = form_urlencoded::byte_serialize(file.text.as_bytes()).collect();
    let r = send(&b, post_form("/restore", &format!("backup={encoded}&passphrase=supersecret1"), None)).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert!(session_from(&r).is_some(), "signed in after restoring");
    let restored = bstore.handle("tega").unwrap().unwrap();
    assert_eq!(restored.ed_pub, original.ed_pub);
    assert!(bstore.current_version(&restored).unwrap().is_some(), "profile came back too");
    assert!(send(&b, get("/@tega", None)).await.text.contains("Tega"));

    // Restoring again on the same node is refused rather than overwriting.
    let again = send(&b, post_form("/restore", &format!("backup={encoded}&passphrase=supersecret1"), None)).await;
    assert_eq!(again.status, StatusCode::BAD_REQUEST);
}
