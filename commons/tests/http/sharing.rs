//! Sharing to a commons, and what a commons refuses: older versions,
//! someone else's handle, floods, and (from a networked node) private addresses.
//! Withdrawing a shared copy.

use crate::common::{csrf_in, get, join_and_publish, post_form, post_json, send, server, share_from_node};
use axum::http::StatusCode;
use hic_commons::app::{router, AppState};
use hic_commons::config::Config;
use hic_commons::role::Role;
use hic_commons::store::Store;
use std::sync::Arc;

#[tokio::test]
async fn commons_refuses_a_genuine_older_signed_version() {
    // Build two real, signed versions from one node, send the newer first.
    let (node, store) = server(Role::Node);
    let sid = join_and_publish(&node, "tega", "public").await;
    let rec = store.handle("tega").unwrap().unwrap();
    let first = store.current_version(&rec).unwrap().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let csrf = csrf_in(&send(&node, get("/new", Some(&sid))).await.text);
    send(
        &node,
        post_form(
            "/publish",
            &format!("handle=tega&csrf={csrf}&name=Tega&bio=second&visibility=public&license=cc-by"),
            Some(&sid),
        ),
    )
    .await;
    let rec = store.handle("tega").unwrap().unwrap();
    let second = store.current_version(&rec).unwrap().unwrap();
    let sender = hic_commons::federation::Sender { base_url: "http://node", instance: "node" };
    let old_share = hic_commons::federation::build_share(&sender, &rec, &first);
    let new_share = hic_commons::federation::build_share(&sender, &rec, &second);

    let (commons, _) = server(Role::Commons);
    assert_eq!(send(&commons, post_json("/inbox", &new_share)).await.status, StatusCode::ACCEPTED);
    let r = send(&commons, post_json("/inbox", &old_share)).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert!(r.text.contains("newer"));
    let dir = send(&commons, get("/", None)).await.text;
    assert!(dir.contains("from node"), "still listed");
}

#[tokio::test]
async fn commons_refuses_same_handle_from_another_key() {
    let (commons, _) = server(Role::Commons);
    let a = share_from_node("tega", "real").await;
    let b = share_from_node("tega", "impostor").await;
    assert_eq!(send(&commons, post_json("/inbox", &a)).await.status, StatusCode::ACCEPTED);
    assert_eq!(send(&commons, post_json("/inbox", &b)).await.status, StatusCode::CONFLICT);
    let r = send(&commons, get("/api/handle/tega", None)).await;
    assert!(r.text.contains("\"available\":false"));
}

#[tokio::test]
async fn withdrawal_removes_the_copy_and_keeps_the_handle() {
    // A node's member and their signed share.
    let (node, nstore) = server(Role::Node);
    join_and_publish(&node, "tega", "public").await;
    let rec = nstore.handle("tega").unwrap().unwrap();
    let v = nstore.current_version(&rec).unwrap().unwrap();
    let sender = hic_commons::federation::Sender { base_url: "http://node", instance: "node" };
    let share = hic_commons::federation::build_share(&sender, &rec, &v);

    // The test commons' own address is http://localhost:8080 (the default).
    let (commons, cstore) = server(Role::Commons);
    assert_eq!(send(&commons, post_json("/inbox", &share)).await.status, StatusCode::ACCEPTED);

    let to_other =
        hic_commons::federation::build_withdraw(&rec, "https://elsewhere.example", v.created_at + 1).unwrap();
    assert_eq!(send(&commons, post_json("/inbox", &to_other)).await.status, StatusCode::BAD_REQUEST);

    let w = hic_commons::federation::build_withdraw(&rec, "http://localhost:8080", v.created_at + 1).unwrap();
    assert_eq!(send(&commons, post_json("/inbox", &w)).await.status, StatusCode::OK);
    assert!(cstore.remotes().is_empty());
    assert!(!send(&commons, get("/", None)).await.text.contains("@tega"));

    // The withdrawn version can't be replayed, and the handle stays theirs.
    assert_eq!(send(&commons, post_json("/inbox", &share)).await.status, StatusCode::CONFLICT);
    assert!(send(&commons, get("/api/handle/tega", None)).await.text.contains("\"available\":false"));
}

#[tokio::test]
async fn a_networked_node_refuses_to_share_to_its_own_machine() {
    let dir = std::env::temp_dir().join(format!("hic-http-ssrf-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let config = Config::from_lookup(|k| (k == "HIC_BIND").then(|| "0.0.0.0".to_string()));
    let app = router(Arc::new(AppState::new(config, Role::Node, store)));
    let sid = join_and_publish(&app, "tega", "public").await;
    let csrf = csrf_in(&send(&app, get("/@tega", Some(&sid))).await.text);
    for target in ["http://127.0.0.1:9", "http://169.254.169.254", "http://192.168.1.1", "http://[::1]:9"] {
        let body = format!("handle=tega&csrf={csrf}&commons_url={target}");
        let r = send(&app, post_form("/share", &body, Some(&sid))).await;
        assert!(r.text.contains("inside its own network"), "{target} should be refused");
    }
}

#[tokio::test]
async fn inbox_is_rate_limited() {
    let dir = std::env::temp_dir().join(format!("hic-http-rate-{}", hic_commons::sessions::random_hex(6)));
    let mut state = AppState::new(Config::from_lookup(|_| None), Role::Commons, Store::open(&dir).unwrap());
    state.inbox_limit = hic_commons::ratelimit::RateLimiter::new(2, 60);
    let app = router(Arc::new(state));
    let junk = serde_json::json!({"type": "nothing"});
    assert_eq!(send(&app, post_json("/inbox", &junk)).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(send(&app, post_json("/inbox", &junk)).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(send(&app, post_json("/inbox", &junk)).await.status, StatusCode::TOO_MANY_REQUESTS);
}
