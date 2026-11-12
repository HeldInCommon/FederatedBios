//! "Run your own commons" (the guide on a node), the local commons, and
//! separate sign-in cookies for servers on the same computer.

use axum::http::{header, StatusCode};
use hic_commons::role::Role;

use crate::common::*;

#[tokio::test]
async fn a_node_has_the_guide_and_a_commons_does_not() {
    let (node, _) = server(Role::Node);
    let r = send(&node, get("/run-a-commons", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text.contains("Run your own commons"));
    assert!(r.text.contains("What&#39;s a local commons?") || r.text.contains("What's a local commons?"));
    // Outside the app there is nothing to press: directions instead.
    assert!(!r.text.contains("hic-app://"));

    let (commons, _) = server(Role::Commons);
    assert_eq!(send(&commons, get("/run-a-commons", None)).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn in_the_app_the_guide_offers_a_local_commons() {
    let (node, _) = server_with(Role::Node, &[("HIC_LAUNCH_TOKEN", "t")]);
    let r = send(&node, get("/run-a-commons", None)).await;
    assert!(r.text.contains("hic-app://open/practice"));
    assert!(r.text.contains("hic-app://open/commons"));
}

#[tokio::test]
async fn a_local_commons_says_so_and_shows_where_to_share() {
    let (local, _) = server_with(Role::Commons, &[("HIC_PRACTICE", "1"), ("HIC_BASE_URL", "http://192.168.1.20:8082")]);
    let r = send(&local, get("/", None)).await;
    assert!(r.text.contains("Local commons"));
    assert!(r.text.contains("Anyone on this Wi-Fi can share to <code>http://192.168.1.20:8082</code>"));
}

#[tokio::test]
async fn each_server_reads_only_its_own_cookie() {
    let (node, _) = server_with(Role::Node, &[("HIC_SESSION_COOKIE", "hic_session_8081")]);
    let r = send(
        &node,
        post_form("/join", "handle=ada&display_name=Ada&passphrase=supersecret1&passphrase_again=supersecret1", None),
    )
    .await;
    let set = r.headers.get(header::SET_COOKIE).and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
    assert!(set.starts_with("hic_session_8081="), "set-cookie was {set:?}");
    let token = set.trim_start_matches("hic_session_8081=").split(';').next().unwrap().to_string();

    // Under its own name the session works; under another server's name it doesn't.
    let ours = axum::http::Request::get("/new")
        .header(header::COOKIE, format!("hic_session_8081={token}"))
        .body(axum::body::Body::empty())
        .unwrap();
    assert_eq!(send(&node, ours).await.status, StatusCode::OK);
    let theirs = get("/new", Some(&token)); // sent as hic_session
    assert_ne!(send(&node, theirs).await.status, StatusCode::OK);
}
