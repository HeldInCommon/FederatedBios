//! `hic-commons`: start a node or a commons.
//!
//! Reads settings from the environment (see `readme.md`), decides the role
//! for the data folder, then serves until Ctrl-C or SIGTERM. All behaviour
//! lives in the library (`src/lib.rs`).

use std::net::SocketAddr;
use std::sync::Arc;

use hic_commons::{app, config::Config, now, role, store::Store, vouching};

/// Start the server. Any start-up failure is printed as one plain line and
/// the process exits with code 2, rather than a panic and a stack trace.
///
/// Learner note: `#[tokio::main]` turns this `async fn` into an ordinary
/// `main` that starts the Tokio runtime and runs it to the end.
#[tokio::main]
async fn main() {
    let config = Config::from_env();
    if let Err(e) = run(config).await {
        eprintln!("hic-commons: {e}");
        std::process::exit(2);
    }
}

/// Open the data folder, settle its role (node or commons), print the
/// one-time links for the operator, and serve until stopped.
///
/// **Writes**, only the first time each is needed: the role file for a new
/// data folder, and on a commons the founding invite and the steward key.
async fn run(config: Config) -> Result<(), String> {
    // Pages call the node's default commons by its name, not its address
    // (see `web::layout::host_label`).
    hic_commons::web::layout::name_default_commons(&config.share_default, &config.share_default_name);
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
        // The steward link: how whoever runs this commons opens the steward
        // view from another computer (decision record 0017). Made once, kept
        // in the data folder.
        if store.steward_key().map_err(|e| e.to_string())?.is_none() {
            store.save_steward_key(&vouching::new_steward_key()).map_err(|e| e.to_string())?;
        }
        if let Some(key) = store.steward_key().map_err(|e| e.to_string())? {
            for base in std::iter::once(&config.base_url).chain(&config.also_known_as) {
                println!("hic-commons steward link (keep private): {base}/steward?key={key}");
            }
        }
    }

    let addr = SocketAddr::new(config.bind_addr(role), config.port);
    let state = Arc::new(app::AppState::new(config, role, store));
    if role == role::Role::Node {
        // Once at start-up: ask each commons we're waiting on whether we've been
        // listed (see waiting.rs; members can also press "Check now").
        let st = state.clone();
        tokio::spawn(async move {
            hic_commons::waiting::check_once(&st).await;
        });
        // For as long as the node runs: keep an outgoing connection open to
        // each commons holding one of our "Only while I'm here" cards, so
        // readers there can fetch the words (decision record 0021). The node
        // opens these itself; it still accepts nothing (decision record 0004).
        tokio::spawn(hic_commons::relay_node::run(state.clone()));
    }
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
