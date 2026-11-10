//! In-process HTTP tests: the whole router, a temporary data folder, no
//! network. Each test builds its own server, so they can run in parallel.
//!
//! Run with `cargo test` in `commons/`. The slower, multi-process federation
//! test is `scripts/test-federation.sh`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

use hic_commons::app::{router, AppState};
use hic_commons::config::Config;
use hic_commons::role::Role;
use hic_commons::store::Store;

/// A server in `role` with a fresh data folder.
fn server(role: Role) -> (Router, Store) {
    let dir =
        std::env::temp_dir().join(format!("hic-http-{}-{}", std::process::id(), hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let config = Config::from_lookup(|k| match k {
        "HIC_DATA" => Some(dir.display().to_string()),
        "HIC_INSTANCE" => Some("Test".into()),
        _ => None,
    });
    let state = Arc::new(AppState::new(config, role, store.clone()));
    (router(state), store)
}

/// The response's status, headers and body text.
struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    text: String,
}

async fn send(app: &Router, req: Request<Body>) -> Reply {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    Reply { status, headers, text: String::from_utf8_lossy(&bytes).into_owned() }
}

fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::get(uri);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("hic_session={c}"));
    }
    b.body(Body::empty()).unwrap()
}

fn post_form(uri: &str, body: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::post(uri).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("hic_session={c}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

fn post_json(uri: &str, body: &serde_json::Value) -> Request<Body> {
    Request::post(uri).header(header::CONTENT_TYPE, "application/json").body(Body::from(body.to_string())).unwrap()
}

/// The session token set by a response, if any.
fn session_from(r: &Reply) -> Option<String> {
    let c = r.headers.get(header::SET_COOKIE)?.to_str().ok()?;
    c.strip_prefix("hic_session=")?.split(';').next().map(String::from).filter(|s| !s.is_empty())
}

/// The CSRF token in a page's forms.
fn csrf_in(html: &str) -> String {
    let at = html.find("name=\"csrf\" value=\"").expect("csrf field") + "name=\"csrf\" value=\"".len();
    html[at..].split('"').next().unwrap().to_string()
}

/// Sign up `handle` and publish a profile with `visibility`; returns the session.
async fn join_and_publish(app: &Router, handle: &str, visibility: &str) -> String {
    let r =
        send(app, post_form("/join", &format!("handle={handle}&display_name=T&passphrase=supersecret1"), None)).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    let sid = session_from(&r).expect("session");
    let csrf = csrf_in(&send(app, get("/new", Some(&sid))).await.text);
    let body =
        format!("handle={handle}&csrf={csrf}&name=Tega&bio=hello&visibility={visibility}&license=cc-by&signals=credit");
    assert_eq!(send(app, post_form("/publish", &body, Some(&sid))).await.status, StatusCode::SEE_OTHER);
    sid
}

#[tokio::test]
async fn health_reports_the_service() {
    let (app, _) = server(Role::Node);
    let r = send(&app, get("/health", None)).await;
    assert!(r.text.contains("hic-commons"));
}

#[tokio::test]
async fn publish_then_listed_in_directory() {
    let (app, _) = server(Role::Node);
    join_and_publish(&app, "tega", "public").await;
    let r = send(&app, get("/", None)).await;
    assert!(r.text.contains("@tega"));
}

#[tokio::test]
async fn commons_reach_is_marked_noindex() {
    let (app, _) = server(Role::Node);
    join_and_publish(&app, "tega", "commons").await;
    let r = send(&app, get("/@tega", None)).await;
    assert_eq!(r.headers.get("x-robots-tag").unwrap(), "noindex");
    assert!(r.text.contains(r#"<meta name="robots" content="noindex">"#));

    join_and_publish(&app, "mo", "public").await;
    assert!(send(&app, get("/@mo", None)).await.headers.get("x-robots-tag").is_none());
}

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
async fn commons_has_no_sign_up() {
    let (app, store) = server(Role::Commons);
    let r = send(&app, post_form("/join", "handle=sneak&display_name=S&passphrase=supersecret1", None)).await;
    assert!(r.text.contains("This is a shared commons"));
    assert!(session_from(&r).is_none());
    assert!(store.handles().is_empty());
}

/// A signed share built by a node for its current version.
async fn share_from_node(handle: &str, bio: &str) -> serde_json::Value {
    let (node, store) = server(Role::Node);
    let sid = join_and_publish(&node, handle, "public").await;
    let csrf = csrf_in(&send(&node, get("/new", Some(&sid))).await.text);
    let body = format!("handle={handle}&csrf={csrf}&name=Tega&bio={bio}&visibility=public&license=cc-by");
    // Publish again so the version differs from the first.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    send(&node, post_form("/publish", &body, Some(&sid))).await;
    let rec = store.handle(handle).unwrap().unwrap();
    let v = store.current_version(&rec).unwrap().unwrap();
    hic_commons::federation::build_share(
        &hic_commons::federation::Sender { base_url: "http://node", instance: "node" },
        &rec,
        &v,
    )
}

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
async fn robots_txt_is_served() {
    let (app, _) = server(Role::Node);
    assert!(send(&app, get("/robots.txt", None)).await.text.contains("Disallow: /login"));
}

#[tokio::test]
async fn health_echoes_the_launch_token_only_when_set() {
    let (app, _) = server(Role::Node);
    assert!(!send(&app, get("/health", None)).await.text.contains("launch"));

    let dir = std::env::temp_dir().join(format!("hic-http-token-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let config = Config::from_lookup(|k| (k == "HIC_LAUNCH_TOKEN").then(|| "abc123".to_string()));
    let app = router(Arc::new(AppState::new(config, Role::Node, store)));
    assert!(send(&app, get("/health", None)).await.text.contains("\"launch\":\"abc123\""));
}
