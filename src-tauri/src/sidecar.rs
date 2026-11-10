//! Starting and stopping the bundled `hic-commons` server (the "sidecar").

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::Manager;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use crate::network::{self, Probe, LOCAL_URL, PORT};
use crate::settings::{data_dir_for, instance_label};

/// Fixed for the whole run of the app; computed once at start-up.
pub struct AppConfig {
    /// The app data folder (holds `app.json` and one data folder per role).
    pub base: PathBuf,
    /// `HIC_INSTANCE` from the host environment, if set.
    pub instance_override: Option<String>,
    /// The commons pre-filled in "Share to a commons".
    pub share_default: String,
    /// Identifies the server this launch started (see [`crate::network`]).
    pub launch_token: String,
}

/// The running server process, if any.
#[derive(Default)]
pub struct Sidecar(pub Mutex<Option<CommandChild>>);

impl Sidecar {
    /// Stop the server if it is running.
    pub fn stop(&self) {
        if let Some(child) = self.0.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
}

/// Start the server on [`PORT`] in `role`, logging its output to stderr.
pub fn spawn(app: &tauri::AppHandle, cfg: &AppConfig, role: &str, name: &str) -> Result<CommandChild, String> {
    let data_dir = data_dir_for(&cfg.base, role);
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("could not create the data folder: {e}"))?;
    let mut cmd = app
        .shell()
        .sidecar("hic-commons")
        .map_err(|e| format!("could not locate the server binary: {e}"))?
        .env("HIC_DATA", data_dir.to_string_lossy().to_string())
        .env("PORT", PORT.to_string())
        .env("HIC_INSTANCE", instance_label(cfg.instance_override.as_deref(), Some(name), role))
        .env("HIC_ROLE", role)
        .env("HIC_LAUNCH_TOKEN", &cfg.launch_token);
    if role == "node" {
        cmd = cmd.env("HIC_SHARE_DEFAULT", &cfg.share_default);
    } else if let Some(url) = network::lan_url() {
        // A room commons: nodes on the same network reach it at this address.
        cmd = cmd.env("HIC_BASE_URL", url);
    }

    let (mut rx, child) = cmd.spawn().map_err(|e| format!("could not start the {role}: {e}"))?;
    log::info!("started the {role} server (data in {})", data_dir.display());
    let tag = role.to_string();
    // Everything the server prints goes to the app's log file (see lib.rs),
    // including how it stopped, so a crash leaves a trace.
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                CommandEvent::Stdout(l) => log::info!("[{tag}] {}", String::from_utf8_lossy(&l).trim_end()),
                CommandEvent::Stderr(l) => log::warn!("[{tag}] {}", String::from_utf8_lossy(&l).trim_end()),
                CommandEvent::Error(e) => log::error!("[{tag}] {e}"),
                CommandEvent::Terminated(t) => {
                    log::warn!("[{tag}] server stopped (exit code {:?}, signal {:?})", t.code, t.signal)
                }
                _ => {}
            }
        }
    });
    Ok(child)
}

/// Set the window title to say which role is running.
pub fn set_title(app: &tauri::AppHandle, role: &str) {
    if let Some(win) = app.get_webview_window("main") {
        let name = if role == "commons" { "Commons" } else { "Node" };
        let _ = win.set_title(&format!("Held in Common \u{2014} {name}"));
    }
}

/// In the background, open the server in the window as soon as *our* server
/// answers (a backstop in case the loading screen's own polling stalls).
/// Gives up after about four minutes.
pub fn open_when_up(app: tauri::AppHandle, token: String) {
    std::thread::spawn(move || {
        for _ in 0..600 {
            if network::probe(&token) == Probe::Ours {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if let Some((_, win)) = app.webview_windows().iter().next() {
                    let _ = win.eval(format!("location.replace('{LOCAL_URL}')"));
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    });
}

/// Start the server in `role`, remember the process, and open it when ready.
pub fn start(app: &tauri::AppHandle, cfg: &AppConfig, sidecar: &Sidecar, role: &str, name: &str) -> Result<(), String> {
    let child = spawn(app, cfg, role, name)?;
    *sidecar.0.lock().unwrap() = Some(child);
    set_title(app, role);
    open_when_up(app.clone(), cfg.launch_token.clone());
    Ok(())
}
