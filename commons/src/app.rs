//! Shared state and the route table.
//!
//! [`router`] is a plain function from state to an axum `Router`, so tests can
//! build the whole server in-process (see `tests/http/`).

use std::sync::{Arc, Mutex};

use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::config::Config;
use crate::ratelimit::RateLimiter;
use crate::role::Role;
use crate::sessions::{AttemptLimiter, Sessions};
use crate::store::Store;
use crate::web;
use crate::web::handlers::{self, account, membership, steward};
use crate::web::layout::Site;
use crate::{api, web::ROBOTS_TXT};

/// Everything a request handler can reach.
pub struct AppState {
    pub config: Config,
    /// The hosts this server counts as itself ([`Config::own_hosts`]).
    pub own_hosts: Vec<String>,
    pub site: Site,
    pub store: Store,
    pub sessions: Sessions,
    /// Counts wrong passphrases per handle.
    pub attempts: AttemptLimiter,
    /// Held while reading, changing and saving a record, so two requests
    /// can't interleave their changes.
    pub write: Mutex<()>,
    /// Outgoing requests (sharing to a commons, checking a handle).
    pub http: reqwest::Client,
    /// Limits `/inbox` and `/api/waiting` requests per sender address, in
    /// one shared count (see [`crate::ratelimit`]).
    pub inbox_limit: RateLimiter,
    /// On a commons: readers waiting for an author's node, and who is here
    /// now, for "Only while I'm here" (see [`crate::relay`]).
    pub relay: crate::relay::Relay,
    /// On a node: the state of each outgoing connection to a commons holding
    /// one of our cards (see [`crate::relay_node`]).
    pub links: crate::relay_node::Links,
}

/// The state as handlers receive it.
pub type Shared = Arc<AppState>;

impl AppState {
    /// Build the state for a server with this config, role and store.
    pub fn new(config: Config, role: Role, store: Store) -> AppState {
        let site = Site {
            role,
            instance: config.instance.clone(),
            base_url: config.base_url.clone(),
            also_known_as: config.also_known_as.clone(),
            share_default: config.share_default.clone(),
            share_default_name: config.share_default_name.clone(),
            practice: config.practice,
            in_app: config.in_app(),
        };
        AppState {
            own_hosts: config.own_hosts(),
            config,
            site,
            store,
            sessions: Sessions::default(),
            attempts: AttemptLimiter::default(),
            write: Mutex::new(()),
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(3))
                .build()
                .expect("HTTP client builds"),
            inbox_limit: RateLimiter::new(INBOX_PER_MINUTE, 60),
            relay: Default::default(),
            links: Default::default(),
        }
    }
}

/// Content-Security-Policy. Every page is rendered on the server; the only
/// script is `/avatar.js` (the picture picker). Styles and fonts come from
/// this server and Google Fonts; images from this server or `data:` URIs.
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' https://fonts.googleapis.com; \
     font-src https://fonts.gstatic.com; img-src 'self' data:; form-action 'self'; \
     base-uri 'none'; frame-ancestors 'none'";

/// How many `/inbox` requests one address may send per minute.
pub const INBOX_PER_MINUTE: u32 = 120;

/// Largest request body accepted. Must comfortably exceed the 600 KB picture
/// limit, which arrives base64- and form-encoded inside the publish form.
const BODY_LIMIT: usize = 2 * 1024 * 1024;

/// A file compiled into the binary, served with its type.
fn static_file(mime: &'static str, body: &'static str) -> axum::response::Response {
    ([(header::CONTENT_TYPE, format!("{mime}; charset=utf-8"))], body).into_response()
}

/// `GET /quilt/{file}`: one of the eight quilt backgrounds (`1.svg` … `8.svg`).
async fn quilt(axum::extract::Path(file): axum::extract::Path<String>) -> axum::response::Response {
    match web::quilt_svg(&file) {
        Some(svg) => static_file("image/svg+xml", svg),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The full route table with security headers and the body limit.
pub fn router(state: Shared) -> Router {
    let security = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP)))
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")))
        .layer(SetResponseHeaderLayer::overriding(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer")))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
        ));

    let routes = Router::new()
        // Pages
        .route("/", get(handlers::directory))
        .route("/about", get(handlers::about))
        .route("/run-a-commons", get(handlers::run_a_commons))
        .route("/join", get(handlers::join_form).post(handlers::join_submit))
        .route("/login", get(handlers::login_form).post(handlers::login_submit))
        .route("/logout", post(handlers::logout))
        .route("/new", get(handlers::publish_form))
        .route("/publish", post(handlers::publish))
        .route("/share", post(handlers::share))
        .route("/withdraw", post(handlers::withdraw))
        .route("/recovery/new", post(account::recovery_new))
        .route("/rotate", post(account::rotate))
        .route("/recover", post(account::recover))
        .route("/invite", post(membership::invite))
        .route("/vouch", post(membership::vouch))
        .route("/steward", get(steward::open))
        .route("/steward/approve", post(steward::approve))
        .route("/steward/decline", post(steward::decline))
        .route("/steward/invite", post(steward::invite))
        .route("/backup", post(handlers::backup))
        .route("/backup/later", post(handlers::backup_later))
        .route("/restore", get(handlers::restore_form).post(handlers::restore_submit))
        .route("/stories", get(handlers::stories))
        .route("/stories/new", get(handlers::story_form).post(handlers::story_create))
        .route("/stories/{kid}/{id}", get(handlers::held_story))
        .route("/stories/{kid}/{id}/here.json", get(handlers::here_json))
        .route("/story/{id}", get(handlers::own_story))
        .route("/story/{id}/text", get(handlers::story_text))
        .route("/story/{id}/edit", get(handlers::story_edit_form).post(handlers::story_edit))
        .route("/story/{id}/rights", get(handlers::rights_form).post(handlers::rights_submit))
        .route("/story/share", post(handlers::story_share))
        .route("/story/withdraw", post(handlers::story_withdraw))
        .route("/@{handle}", get(handlers::profile))
        .route("/federated/{kid}", get(handlers::federated))
        // Federation
        .route("/inbox", post(handlers::inbox))
        // "Only while I'm here" (decision record 0021): nodes open these; nothing opens a node.
        .route("/here/card", post(handlers::here_card))
        .route("/here/poll", post(handlers::here_poll))
        // JSON
        .route("/api/directory", get(api::directory))
        .route("/api/@{handle}", get(api::profile))
        .route("/api/handle/{handle}", get(api::handle_available))
        .route("/api/pending", get(api::pending))
        // Members only: the asker signs the request (decision record 0026).
        .route("/api/waiting", post(handlers::waiting_list))
        .route("/api/status", get(api::share_status))
        .route("/api/notices/claim", post(api::claim_notices))
        .route("/notices/dismiss", post(handlers::dismiss_notices))
        .route("/waiting/check", post(handlers::check_waiting))
        .route("/.well-known/webfinger", get(api::webfinger))
        .route("/ap/@{handle}", get(api::actor))
        .route("/ap/@{handle}/outbox", get(api::outbox))
        // Static files and health
        .route("/style.css", get(|| async { static_file("text/css", web::STYLE_CSS) }))
        .route("/avatar.js", get(|| async { static_file("application/javascript", web::AVATAR_JS) }))
        .route("/rights.js", get(|| async { static_file("application/javascript", web::RIGHTS_JS) }))
        .route("/password.js", get(|| async { static_file("application/javascript", web::PASSWORD_JS) }))
        .route("/logo.png", get(|| async { ([(header::CONTENT_TYPE, "image/png")], web::LOGO_PNG).into_response() }))
        .route("/robots.txt", get(|| async { static_file("text/plain", ROBOTS_TXT) }))
        .route("/quilt/{file}", get(quilt))
        .route("/health", get(api::health));
    // Open to the room (decision record 0027): the door stands in front of
    // every route, so a room visitor never reaches a page it doesn't allow.
    // Added only when `HIC_ROOM=1`; otherwise nothing changes.
    let routes = if state.config.room {
        routes.layer(axum::middleware::from_fn_with_state(state.clone(), crate::room::guard))
    } else {
        routes
    };
    routes.layer(security).layer(RequestBodyLimitLayer::new(BODY_LIMIT)).with_state(state)
}
