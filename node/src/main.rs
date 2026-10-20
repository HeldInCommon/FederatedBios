//! The local node: a small HTTP service the onboarding UI talks to.
//! For now it only answers `GET /health`, which is all `probes::node()` needs.

use axum::{http::HeaderValue, routing::get, Json, Router};
use serde::Serialize;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Origins allowed to call the node. Covers `trunk serve` and a Tauri webview.
/// Override with ALLOWED_ORIGINS="http://a,http://b".
const DEFAULT_ORIGINS: &str =
    "http://localhost:8080,http://127.0.0.1:8080,tauri://localhost,http://tauri.localhost";

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health { status: "ok", version: env!("CARGO_PKG_VERSION") })
}

#[tokio::main]
async fn main() {
    let origins: Vec<HeaderValue> = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| DEFAULT_ORIGINS.to_string())
        .split(',')
        .filter_map(|o| o.trim().parse().ok())
        .collect();

    let app = Router::new()
        .route("/health", get(health))
        .layer(CorsLayer::new().allow_origin(AllowOrigin::list(origins)));

    // 0.0.0.0, not 127.0.0.1: inside a container, loopback is unreachable
    // from the host even with -p 4321:4321.
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4321").await.expect("bind 4321");
    println!("hic-node listening on 4321");
    axum::serve(listener, app)
        .with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; })
        .await
        .expect("server error");
}
