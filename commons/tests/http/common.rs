//! Helpers shared by the HTTP tests: build a server with a fresh data folder,
//! send it requests, and set up members, shares and stewards.

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
pub fn server(role: Role) -> (Router, Store) {
    server_with(role, &[])
}

/// A server in `role` with a fresh data folder and these extra settings.
pub fn server_with(role: Role, extra: &[(&str, &str)]) -> (Router, Store) {
    let extra: Vec<(String, String)> = extra.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let dir =
        std::env::temp_dir().join(format!("hic-http-{}-{}", std::process::id(), hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let config = Config::from_lookup(|k| match k {
        "HIC_DATA" => Some(dir.display().to_string()),
        "HIC_INSTANCE" => Some("Test".into()),
        // Most tests exercise other features; vouching has its own tests below.
        "HIC_OPEN_COMMONS" => Some("1".into()),
        _ => extra.iter().find(|(name, _)| name == k).map(|(_, v)| v.clone()),
    });
    let state = Arc::new(AppState::new(config, role, store.clone()));
    (router(state), store)
}

/// The response's status, headers and body text.
pub struct Reply {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub text: String,
}

/// Send `req` through the router in process (no network) and collect the reply.
pub async fn send(app: &Router, req: Request<Body>) -> Reply {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    Reply { status, headers, text: String::from_utf8_lossy(&bytes).into_owned() }
}

/// A `GET` request, signed in when `cookie` holds a session token.
pub fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::get(uri);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("hic_session={c}"));
    }
    b.body(Body::empty()).unwrap()
}

/// A form `POST`, signed in when `cookie` holds a session token.
pub fn post_form(uri: &str, body: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::post(uri).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("hic_session={c}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

/// A JSON `POST`, as one server sends to another.
pub fn post_json(uri: &str, body: &serde_json::Value) -> Request<Body> {
    Request::post(uri).header(header::CONTENT_TYPE, "application/json").body(Body::from(body.to_string())).unwrap()
}

/// The session token set by a response, if any.
pub fn session_from(r: &Reply) -> Option<String> {
    let c = r.headers.get(header::SET_COOKIE)?.to_str().ok()?;
    c.strip_prefix("hic_session=")?.split(';').next().map(String::from).filter(|s| !s.is_empty())
}

/// The CSRF token in a page's forms.
pub fn csrf_in(html: &str) -> String {
    let at = html.find("name=\"csrf\" value=\"").expect("csrf field") + "name=\"csrf\" value=\"".len();
    html[at..].split('"').next().unwrap().to_string()
}

/// Sign up `handle` and publish a profile with `visibility`; returns the session.
pub async fn join_and_publish(app: &Router, handle: &str, visibility: &str) -> String {
    let r = send(
        app,
        post_form(
            "/join",
            &format!("handle={handle}&display_name=T&passphrase=supersecret1&passphrase_again=supersecret1"),
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text.contains("Your recovery code"), "sign-up shows the recovery code once");
    let sid = session_from(&r).expect("session");
    let csrf = csrf_in(&send(app, get("/new", Some(&sid))).await.text);
    let body =
        format!("handle={handle}&csrf={csrf}&name=Tega&bio=hello&visibility={visibility}&license=cc-by&signals=credit");
    assert_eq!(send(app, post_form("/publish", &body, Some(&sid))).await.status, StatusCode::SEE_OTHER);
    sid
}

/// A signed share built by a node for its current version.
pub async fn share_from_node(handle: &str, bio: &str) -> serde_json::Value {
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

/// A commons that requires vouching, with `limit` people per member per week.
pub fn closed_commons(limit: u32) -> (Router, Store) {
    let dir = std::env::temp_dir().join(format!("hic-http-vouch-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let lim = limit.to_string();
    let config = Config::from_lookup(move |k| (k == "HIC_VOUCH_LIMIT").then(|| lim.clone()));
    (router(Arc::new(AppState::new(config, Role::Commons, store.clone()))), store)
}

/// A member identity with a published profile, and its hic/v1 share
/// (optionally carrying an invite code).
pub fn person(h: &str) -> hic_commons::model::HandleRec {
    hic_commons::model::HandleRec::new(h, h, 0, hic_commons::keys::generate(), String::new(), String::new())
}

/// The JSON a node sends to share `rec`'s public profile to a commons, with
/// an invite code when one is given.
pub fn share_of(rec: &hic_commons::model::HandleRec, invite: Option<&str>) -> serde_json::Value {
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
pub const COMMONS_URL: &str = "http://localhost:8080";

/// Open the steward view with the steward link; returns the session and the
/// steward's CSRF token (read from the page).
pub async fn steward_session(commons: &Router, store: &Store) -> (String, String) {
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
