//! Shared state and the route table.
//!
//! [`router`] is a plain function from state to an axum `Router`, so tests can
//! build the whole server in-process (see `tests/http.rs`).

use std::sync::{Arc, Mutex};

use axum::http::{header, HeaderName, HeaderValue};
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
use crate::web::layout::Site;
use crate::web::{self, account, handlers, membership, steward};
use crate::{api, web::ROBOTS_TXT};

/// Everything a request handler can reach.
pub struct AppState {
    pub config: Config,
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
    /// Limits `/inbox` requests per sender address (see [`crate::ratelimit`]).
    pub inbox_limit: RateLimiter,
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
            share_default: config.share_default.clone(),
        };
        AppState {
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

    Router::new()
        // Pages
        .route("/", get(handlers::directory))
        .route("/about", get(handlers::about))
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
        .route("/restore", get(handlers::restore_form).post(handlers::restore_submit))
        .route("/@{handle}", get(handlers::profile))
        .route("/federated/{kid}", get(handlers::federated))
        // Federation
        .route("/inbox", post(handlers::inbox))
        // JSON
        .route("/api/directory", get(api::directory))
        .route("/api/@{handle}", get(api::profile))
        .route("/api/handle/{handle}", get(api::handle_available))
        .route("/api/pending", get(api::pending))
        .route("/.well-known/webfinger", get(api::webfinger))
        .route("/ap/@{handle}", get(api::actor))
        .route("/ap/@{handle}/outbox", get(api::outbox))
        // Static files and health
        .route("/style.css", get(|| async { static_file("text/css", web::STYLE_CSS) }))
        .route("/avatar.js", get(|| async { static_file("application/javascript", web::AVATAR_JS) }))
        .route("/password.js", get(|| async { static_file("application/javascript", web::PASSWORD_JS) }))
        .route("/robots.txt", get(|| async { static_file("text/plain", ROBOTS_TXT) }))
        .route("/health", get(api::health))
        .layer(security)
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT))
        .with_state(state)
}
