//! `hic-commons`: start a node or a commons.
//!
//! Reads settings from the environment (see `readme.md`), decides the role
//! for the data folder, then serves until Ctrl-C or SIGTERM. All behaviour
//! lives in the library (`src/lib.rs`).

use std::net::SocketAddr;
use std::sync::Arc;

use hic_commons::{app, config::Config, now, role, store::Store};

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    if let Err(e) = run(config).await {
        eprintln!("hic-commons: {e}");
        std::process::exit(2);
    }
}

async fn run(config: Config) -> Result<(), String> {
    let store = Store::open(&config.data_dir)
        .map_err(|e| format!("can't use the data folder {}: {e}", config.data_dir.display()))?;
    let role = role::resolve(store.root(), config.role_env.as_deref(), &config.instance, now())?;
    println!("hic-commons role: {}", role.as_str());

    let addr = SocketAddr::new(config.bind_addr(role), config.port);
    let state = Arc::new(app::AppState::new(config, role, store));
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| format!("can't listen on {addr}: {e}"))?;
    println!("hic-commons listening on {addr}");
    axum::serve(listener, app::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("server stopped: {e}"))
}

/// Resolves on Ctrl-C, or on SIGTERM (what `docker stop` and the desktop app
/// send), so in-flight requests finish before the process exits.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}
