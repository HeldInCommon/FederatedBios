//! The Held in Common node: a small HTTP service that keeps one member's
//! profile on their own machine. The onboarding UI calls it from the browser,
//! so every response carries CORS headers for the UI's origin.
//!
//! Configuration comes from the environment:
//! - `HIC_ADDR`: where to listen (default `127.0.0.1:4321`; the image uses `0.0.0.0:4321`
//!   so Docker's port mapping can reach it).
//! - `HIC_DATA_DIR`: the local store (default `./data`; the image uses the `/data` volume).
//! - `HIC_ALLOWED_ORIGINS`: comma-separated origins allowed to call the node
//!   (default: the `trunk serve` addresses).

mod store;

use axum::extract::State;
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use store::Store;
use tower_http::cors::CorsLayer;

const DEFAULT_ORIGINS: &str = "http://127.0.0.1:8080,http://localhost:8080";

/// Matches `BIO_LIMIT` in the UI's `profile.rs`.
const BIO_LIMIT: usize = 500;

// The shapes below mirror `Draft` in `src/profile.rs` and `Reach` in
// `src/visibility.rs`, so the UI can send what it already builds.

#[derive(Serialize, Deserialize)]
struct Profile {
    name: String,
    pronouns: String,
    role: String,
    bio: String,
    places: Vec<Place>,
}

#[derive(Serialize, Deserialize)]
struct Place {
    name: String,
    precision: Precision,
}

#[derive(Serialize, Deserialize)]
enum Precision {
    City,
    Region,
    Country,
}

#[derive(Serialize, Deserialize)]
struct Visibility {
    reach: Reach,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Reach {
    Public,
    Commons,
    Members,
}

type AppState = Arc<Store>;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = env_or("HIC_ADDR", "127.0.0.1:4321");
    let data_dir = env_or("HIC_DATA_DIR", "./data");
    let origins = env_or("HIC_ALLOWED_ORIGINS", DEFAULT_ORIGINS);

    let store = Store::open(data_dir.clone().into()).await?;
    let app = router(Arc::new(store), &origins);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!("hic-node {} listening on {addr}, data in {data_dir}", env!("CARGO_PKG_VERSION"));
    eprintln!("allowing origins: {origins}");
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await
}

fn router(store: AppState, origins: &str) -> Router {
    let origins: Vec<HeaderValue> = origins
        .split(',')
        .map(str::trim)
        .filter(|o| !o.is_empty())
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::PUT])
        .allow_headers([header::CONTENT_TYPE]);

    Router::new()
        .route("/health", get(health))
        .route("/profile", get(get_profile).put(put_profile))
        .route("/visibility", get(get_visibility).put(put_visibility))
        .layer(cors)
        .with_state(store)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

async fn get_profile(State(store): State<AppState>) -> Result<Json<Profile>, ApiError> {
    store.read("profile").await?.map(Json).ok_or(ApiError::NotFound)
}

async fn put_profile(State(store): State<AppState>, Json(profile): Json<Profile>) -> Result<StatusCode, ApiError> {
    if profile.bio.chars().count() > BIO_LIMIT {
        return Err(ApiError::Invalid(format!("The bio is longer than {BIO_LIMIT} characters.")));
    }
    store.write("profile", &profile).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_visibility(State(store): State<AppState>) -> Result<Json<Visibility>, ApiError> {
    store.read("visibility").await?.map(Json).ok_or(ApiError::NotFound)
}

async fn put_visibility(State(store): State<AppState>, Json(choice): Json<Visibility>) -> Result<StatusCode, ApiError> {
    store.write("visibility", &choice).await?;
    Ok(StatusCode::NO_CONTENT)
}

enum ApiError {
    /// Nothing saved yet. The UI treats this as an empty form.
    NotFound,
    Invalid(String),
    Store(std::io::Error),
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        Self::Store(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "Nothing saved yet.").into_response(),
            Self::Invalid(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg).into_response(),
            Self::Store(e) => {
                eprintln!("store error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "The node could not read or write its data.").into_response()
            }
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// As PID 1 in a container the process gets no default signal handling, so
/// without this `docker stop` would wait out its timeout and then kill us.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
