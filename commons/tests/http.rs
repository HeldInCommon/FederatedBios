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
        // Most tests exercise other features; vouching has its own tests below.
        "HIC_OPEN_COMMONS" => Some("1".into()),
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
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text.contains("Your recovery code"), "sign-up shows the recovery code once");
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

// ---------------------------------------------------------------------------
// Vouching (decision record 0011)
// ---------------------------------------------------------------------------

/// A commons that requires vouching, with `limit` people per member per week.
fn closed_commons(limit: u32) -> (Router, Store) {
    let dir = std::env::temp_dir().join(format!("hic-http-vouch-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let lim = limit.to_string();
    let config = Config::from_lookup(move |k| (k == "HIC_VOUCH_LIMIT").then(|| lim.clone()));
    (router(Arc::new(AppState::new(config, Role::Commons, store.clone()))), store)
}

/// A member identity with a published profile, and its hic/v1 share
/// (optionally carrying an invite code).
fn person(h: &str) -> hic_commons::model::HandleRec {
    hic_commons::model::HandleRec::new(h, h, 0, hic_commons::keys::generate(), String::new(), String::new())
}

fn share_of(rec: &hic_commons::model::HandleRec, invite: Option<&str>) -> serde_json::Value {
    use hic_commons::{federation, model, publishing};
    let draft = publishing::Draft {
        profile: model::Profile { name: rec.handle.clone(), ..Default::default() },
        reach: model::Reach::Public,
        license: "cc-by".into(),
        signals: vec![],
        created_at: hic_commons::now(),
    };
    let v = publishing::sign_version(rec, draft, &[]).unwrap();
    let mut m =
        federation::build_share_v1(&federation::Sender { base_url: "http://node", instance: "node" }, rec, &v).unwrap();
    if let Some(code) = invite {
        m["invite"] = serde_json::json!(code);
    }
    m
}

/// The test commons' own address (the default base URL).
const COMMONS_URL: &str = "http://localhost:8080";

#[tokio::test]
async fn without_a_vouch_a_newcomer_waits() {
    let (commons, _) = closed_commons(5);
    let r = send(&commons, post_json("/inbox", &share_of(&person("newbie"), None))).await;
    assert_eq!(r.status, StatusCode::ACCEPTED);
    assert!(r.text.starts_with("pending"));
    let dir = send(&commons, get("/", None)).await.text;
    assert!(dir.contains("Waiting for a vouch") && !dir.contains("dir-card"), "listed only as waiting");
    assert!(send(&commons, get("/api/pending", None)).await.text.contains("newbie"));
}

#[tokio::test]
async fn founding_invite_admits_the_first_person_once() {
    let (commons, store) = closed_commons(5);
    store.save_founding_code("hic-founding-abc123").unwrap();
    let first = send(&commons, post_json("/inbox", &share_of(&person("steward"), Some("hic-founding-abc123")))).await;
    assert_eq!(first.text, "accepted");
    let again = send(&commons, post_json("/inbox", &share_of(&person("other"), Some("hic-founding-abc123")))).await;
    assert_eq!(again.status, StatusCode::FORBIDDEN);
    assert_eq!(store.intros()[0].route, "founding");
}

#[tokio::test]
async fn a_members_invite_admits_once_and_is_recorded() {
    let (commons, store) = closed_commons(5);
    store.save_founding_code("hic-founding-x1").unwrap();
    let m = person("member");
    send(&commons, post_json("/inbox", &share_of(&m, Some("hic-founding-x1")))).await;

    let inv = hic_commons::vouching::make_invite(&m.ed_pub, &m.ed_priv, COMMONS_URL, hic_commons::now()).unwrap();
    let code = hic_commons::vouching::invite_to_code(&inv);
    let r = send(&commons, post_json("/inbox", &share_of(&person("friend"), Some(&code)))).await;
    assert_eq!(r.text, "accepted");
    let reused = send(&commons, post_json("/inbox", &share_of(&person("third"), Some(&code)))).await;
    assert_eq!(reused.status, StatusCode::FORBIDDEN);
    assert!(reused.text.contains("already been used"));
    let friend = store.intros().into_iter().find(|i| i.handle == "friend").unwrap();
    assert_eq!(friend.voucher.as_deref(), Some(m.ed_pub.as_str()));
}

#[tokio::test]
async fn a_vouch_lists_someone_waiting() {
    let (commons, store) = closed_commons(5);
    store.save_founding_code("hic-founding-x2").unwrap();
    let m = person("member");
    send(&commons, post_json("/inbox", &share_of(&m, Some("hic-founding-x2")))).await;
    let newbie = person("newbie");
    send(&commons, post_json("/inbox", &share_of(&newbie, None))).await;

    let stranger = person("stranger");
    let bad = hic_commons::vouching::make_vouch(
        &stranger.ed_pub,
        &stranger.ed_priv,
        COMMONS_URL,
        "newbie",
        &newbie.ed_pub,
        hic_commons::now(),
    )
    .unwrap();
    assert_eq!(
        send(&commons, post_json("/inbox", &serde_json::to_value(&bad).unwrap())).await.status,
        StatusCode::FORBIDDEN
    );

    let v = hic_commons::vouching::make_vouch(
        &m.ed_pub,
        &m.ed_priv,
        COMMONS_URL,
        "newbie",
        &newbie.ed_pub,
        hic_commons::now(),
    )
    .unwrap();
    let r = send(&commons, post_json("/inbox", &serde_json::to_value(&v).unwrap())).await;
    assert_eq!(r.text, "accepted");
    assert!(store.pending().is_empty());
    assert!(send(&commons, get("/", None)).await.text.contains("@newbie"));
}

#[tokio::test]
async fn the_steward_sets_the_weekly_limit() {
    let (commons, store) = closed_commons(1);
    store.save_founding_code("hic-founding-x3").unwrap();
    let m = person("member");
    send(&commons, post_json("/inbox", &share_of(&m, Some("hic-founding-x3")))).await;
    let code = |_: u8| {
        hic_commons::vouching::invite_to_code(
            &hic_commons::vouching::make_invite(&m.ed_pub, &m.ed_priv, COMMONS_URL, hic_commons::now()).unwrap(),
        )
    };
    assert_eq!(send(&commons, post_json("/inbox", &share_of(&person("one"), Some(&code(1))))).await.text, "accepted");
    let r = send(&commons, post_json("/inbox", &share_of(&person("two"), Some(&code(2))))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(r.text.contains("as many people"));
}

#[tokio::test]
async fn people_already_listed_stay_and_reshare_freely() {
    let (commons, store) = closed_commons(5);
    store.save_founding_code("hic-founding-x4").unwrap();
    let m = person("member");
    send(&commons, post_json("/inbox", &share_of(&m, Some("hic-founding-x4")))).await;
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let r = send(&commons, post_json("/inbox", &share_of(&m, None))).await;
    assert_eq!(r.text, "accepted", "a returning member needs no new vouch");
}

// ---------------------------------------------------------------------------
// The steward view (decision record 0017)
// ---------------------------------------------------------------------------

/// Open the steward view with the steward link; returns the session and the
/// steward's CSRF token (read from the page).
async fn steward_session(commons: &Router, store: &Store) -> (String, String) {
    store.save_steward_key("k3y").unwrap();
    let wrong = send(commons, get("/steward?key=nope", None)).await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST, "a wrong steward link is refused");
    let r = send(commons, get("/steward?key=k3y", None)).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    let sid = session_from(&r).expect("steward session");
    let page = send(commons, get("/", Some(&sid))).await.text;
    assert!(page.contains("Steward view"));
    (sid.clone(), csrf_in(&page))
}

#[tokio::test]
async fn steward_approves_and_declines_people_waiting() {
    let (commons, store) = closed_commons(5);
    let (sid, csrf) = steward_session(&commons, &store).await;
    let (amy, bo) = (person("amy"), person("bo"));
    send(&commons, post_json("/inbox", &share_of(&amy, None))).await;
    send(&commons, post_json("/inbox", &share_of(&bo, None))).await;
    let page = send(&commons, get("/", Some(&sid))).await.text;
    assert!(page.contains("/steward/approve") && page.contains("/steward/decline"), "buttons for the steward");
    assert!(!send(&commons, get("/", None)).await.text.contains("/steward/approve"), "but not for visitors");

    // Without the steward's token, nothing happens.
    let forged = send(&commons, post_form("/steward/approve", &format!("key={}", amy.ed_pub), Some(&sid))).await;
    assert_eq!(forged.status, StatusCode::BAD_REQUEST);
    assert_eq!(store.pending().len(), 2);

    let ok =
        send(&commons, post_form("/steward/approve", &format!("csrf={csrf}&key={}", amy.ed_pub), Some(&sid))).await;
    assert_eq!(ok.status, StatusCode::SEE_OTHER);
    let no = send(&commons, post_form("/steward/decline", &format!("csrf={csrf}&key={}", bo.ed_pub), Some(&sid))).await;
    assert_eq!(no.status, StatusCode::SEE_OTHER);
    assert!(store.pending().is_empty());
    assert_eq!(store.remotes().len(), 1);
    assert_eq!(store.intros()[0].route, "steward");
}

#[tokio::test]
async fn steward_invite_codes_work_once_and_can_be_emailed() {
    let (commons, store) = closed_commons(5);
    let (sid, csrf) = steward_session(&commons, &store).await;
    let page = send(&commons, post_form("/steward/invite", &format!("csrf={csrf}"), Some(&sid))).await.text;
    assert!(page.contains("mailto:?subject="), "an Email this invite button");
    let code =
        page.split("hic-invite-").nth(1).unwrap().chars().take_while(|c| c.is_ascii_hexdigit()).collect::<String>();
    let code = format!("hic-invite-{code}");
    assert_eq!(send(&commons, post_json("/inbox", &share_of(&person("guest"), Some(&code)))).await.text, "accepted");
    let again = send(&commons, post_json("/inbox", &share_of(&person("other"), Some(&code)))).await;
    assert_eq!(again.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn member_invite_page_can_be_emailed_and_passphrase_boxes_get_an_eye() {
    let (app, _) = server(Role::Node);
    assert!(send(&app, get("/join", None)).await.text.contains(r#"<script src="/password.js">"#));
    let js = send(&app, get("/password.js", None)).await;
    assert!(js.text.contains("Show passphrase"));
    // A members-only profile explains why it can't be shared.
    let sid = join_and_publish(&app, "mo", "members").await;
    assert!(send(&app, get("/@mo", Some(&sid))).await.text.contains("so it stays on your node and is never shared"));
}

// ---------------------------------------------------------------------------
// Following up on a waiting share
// ---------------------------------------------------------------------------

#[tokio::test]
async fn commons_answers_share_status() {
    let (commons, store) = closed_commons(5);
    let newbie = person("newbie");
    let status = |h: &str, k: &str| format!("/api/status?handle={h}&key={k}");
    assert!(send(&commons, get(&status("newbie", &newbie.ed_pub), None)).await.text.contains("not_listed"));
    send(&commons, post_json("/inbox", &share_of(&newbie, None))).await;
    assert!(send(&commons, get(&status("newbie", &newbie.ed_pub), None)).await.text.contains("\"waiting\""));
    store.save_founding_code("hic-founding-x9").unwrap();
    send(&commons, post_json("/inbox", &share_of(&person("other"), Some("hic-founding-x9")))).await;
    let o = store.remotes()[0].clone();
    assert!(send(&commons, get(&status("other", &o.ed_pub), None)).await.text.contains("\"listed\""));
}

#[tokio::test]
async fn only_the_desktop_app_can_collect_notices() {
    let dir = std::env::temp_dir().join(format!("hic-http-notices-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let config = Config::from_lookup(|k| (k == "HIC_LAUNCH_TOKEN").then(|| "tok".to_string()));
    let app = router(Arc::new(AppState::new(config, Role::Node, store.clone())));
    let sid = join_and_publish(&app, "ada", "public").await;
    store
        .add_notice(hic_commons::model::Notice {
            handle: "ada".into(),
            text: "@ada is now listed on c.".into(),
            at: 1,
            seen: false,
            pushed: false,
        })
        .unwrap();
    // The banner shows on the member's pages.
    let page = send(&app, get("/@ada", Some(&sid))).await.text;
    assert!(page.contains("@ada is now listed on c.") && page.contains("/notices/dismiss"));
    // Without the launch token, nothing is collected.
    let no = Request::post("/api/notices/claim").body(Body::empty()).unwrap();
    assert_eq!(send(&app, no).await.status, StatusCode::FORBIDDEN);
    let yes = Request::post("/api/notices/claim").header("x-hic-launch", "tok").body(Body::empty()).unwrap();
    assert!(send(&app, yes).await.text.contains("now listed"));
    let again = Request::post("/api/notices/claim").header("x-hic-launch", "tok").body(Body::empty()).unwrap();
    assert_eq!(send(&app, again).await.text, "[]", "each notice is collected once");
    // Dismissing hides the banner.
    let csrf = csrf_in(&page);
    send(&app, post_form("/notices/dismiss", &format!("csrf={csrf}"), Some(&sid))).await;
    assert!(!send(&app, get("/@ada", Some(&sid))).await.text.contains("now listed"));
}
