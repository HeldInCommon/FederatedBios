//! The server as a whole: health, robots.txt, and what each role offers.

use crate::common::{get, post_form, send, server, session_from};
use hic_commons::app::{router, AppState};
use hic_commons::config::Config;
use hic_commons::role::Role;
use hic_commons::store::Store;
use std::sync::Arc;

#[tokio::test]
async fn health_reports_the_service() {
    let (app, _) = server(Role::Node);
    let r = send(&app, get("/health", None)).await;
    assert!(r.text.contains("hic-commons"));
}

#[tokio::test]
async fn robots_txt_is_served() {
    let (app, _) = server(Role::Node);
    assert!(send(&app, get("/robots.txt", None)).await.text.contains("Disallow: /login"));
}

#[tokio::test]
async fn health_never_reveals_the_launch_token() {
    let dir = std::env::temp_dir().join(format!("hic-http-token-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let config = Config::from_lookup(|k| (k == "HIC_LAUNCH_TOKEN").then(|| "abc123".to_string()));
    let app = router(Arc::new(AppState::new(config, Role::Node, store)));
    let anyone = send(&app, get("/health", None)).await.text;
    assert!(!anyone.contains("abc123") && !anyone.contains("ours"), "a visitor learns nothing");

    let mut asked = get("/health", None);
    asked.headers_mut().insert("x-hic-launch", "abc123".parse().unwrap());
    let app_reply = send(&app, asked).await.text;
    assert!(
        app_reply.contains("\"ours\":true") && !app_reply.contains("abc123"),
        "the app is told it's ours, not the token"
    );

    let mut wrong = get("/health", None);
    wrong.headers_mut().insert("x-hic-launch", "nope".parse().unwrap());
    assert!(!send(&app, wrong).await.text.contains("ours"));
}

#[tokio::test]
async fn commons_has_no_sign_up() {
    let (app, store) = server(Role::Commons);
    let r = send(
        &app,
        post_form("/join", "handle=sneak&display_name=S&passphrase=supersecret1&passphrase_again=supersecret1", None),
    )
    .await;
    assert!(r.text.contains("This is a shared commons"));
    assert!(session_from(&r).is_none());
    assert!(store.handles().is_empty());
}
