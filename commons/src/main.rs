//! `hic-commons`: start a node or a commons.
//!
//! Reads settings from the environment (see `readme.md`), decides the role
//! for the data folder, then serves until Ctrl-C or SIGTERM. All behaviour
//! lives in the library (`src/lib.rs`).

use std::net::SocketAddr;
use std::sync::Arc;

use hic_commons::{app, config::Config, now, role, store::Store, vouching};

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
    // Decide the folder's role: read what's there, apply the rule, then save
    // the decision for a new folder.
    let on_disk = role::inspect(store.root());
    let facts = role::Facts {
        saved: on_disk.saved,
        env: config.role_env.as_deref(),
        instance_label: &config.instance,
        has_members: on_disk.has_members,
    };
    let decision = role::decide(&facts, &store.root().display().to_string())?;
    if decision.guessed {
        eprintln!(
            "warning: guessing role \"commons\" from HIC_INSTANCE. This guess is deprecated; set HIC_ROLE=commons."
        );
    }
    if decision.save {
        role::save(store.root(), decision.role, now())?;
    }
    let role = decision.role;
    println!("hic-commons role: {}", role.as_str());
    if role == role::Role::Commons && !config.open_commons {
        // A brand-new commons makes a one-time founding invite for its first
        // member (decision record 0011), saved in the data folder.
        if vouching::needs_founding_invite(store.has_founding_record(), !store.remotes().is_empty()) {
            store.save_founding_code(&vouching::new_founding_code()).map_err(|e| e.to_string())?;
        }
        if let Some(code) = store.founding_code().map_err(|e| e.to_string())? {
            println!("hic-commons founding invite (one use, for the first member): {code}");
        }
    }

    let addr = SocketAddr::new(config.bind_addr(role), config.port);
    let state = Arc::new(app::AppState::new(config, role, store));
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| format!("can't listen on {addr}: {e}"))?;
    println!("hic-commons listening on {addr}");
    // With connection info, so handlers can see the sender's address (rate limits).
    axum::serve(listener, app::router(state).into_make_service_with_connect_info::<SocketAddr>())
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
