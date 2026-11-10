//! Starting and stopping the bundled `hic-commons` server (the "sidecar").

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
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

/// The running server process, if any, and what's needed to restart it
/// after a crash.
#[derive(Default)]
pub struct Sidecar {
    child: Mutex<Option<CommandChild>>,
    /// Bumped each time a server is started or deliberately stopped, so an
    /// old process ending can be told apart from the current one crashing.
    generation: AtomicU64,
    /// Automatic restarts since the last deliberate start.
    restarts: AtomicU32,
}

/// How many times a crashed server is restarted automatically before the
/// app stops trying (the loading screen's Start button can still restart it).
pub const MAX_RESTARTS: u32 = 3;

/// Should a server that stopped be restarted automatically? Only if it was
/// the current server (not stopped on purpose or replaced), it hasn't been
/// restarted too often, and it didn't refuse to start because of its
/// settings (exit code 2, e.g. a data folder belonging to the other role),
/// which would only fail again.
pub fn should_restart(its_generation: u64, current_generation: u64, restarts: u32, exit_code: Option<i32>) -> bool {
    its_generation == current_generation && restarts < MAX_RESTARTS && exit_code != Some(2)
}

impl Sidecar {
    /// Begin a new server generation. `deliberate` (a person started it)
    /// also resets the automatic-restart count. Returns the generation.
    fn begin(&self, deliberate: bool) -> u64 {
        if deliberate {
            self.restarts.store(0, Ordering::SeqCst);
        }
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Remember the running process.
    fn set(&self, child: CommandChild) {
        *self.child.lock().unwrap() = Some(child);
    }

    /// Stop the server if it is running. It won't be restarted automatically.
    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
}

/// Start the server on [`PORT`] in `role`, logging its output to stderr.
fn spawn(
    app: &tauri::AppHandle,
    cfg: &AppConfig,
    role: &str,
    name: &str,
    generation: u64,
) -> Result<CommandChild, String> {
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
    let (app, role, name) = (app.clone(), role.to_string(), name.to_string());
    // Everything the server prints goes to the app's log file (see lib.rs),
    // including how it stopped, so a crash leaves a trace; a crash of the
    // current server is followed by an automatic restart.
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                CommandEvent::Stdout(l) => log::info!("[{tag}] {}", String::from_utf8_lossy(&l).trim_end()),
                CommandEvent::Stderr(l) => log::warn!("[{tag}] {}", String::from_utf8_lossy(&l).trim_end()),
                CommandEvent::Error(e) => log::error!("[{tag}] {e}"),
                CommandEvent::Terminated(t) => {
                    log::warn!("[{tag}] server stopped (exit code {:?}, signal {:?})", t.code, t.signal);
                    restart_after_crash(&app, &role, &name, generation, t.code);
                }
                _ => {}
            }
        }
    });
    Ok(child)
}

/// If the server that just stopped should come back (see [`should_restart`]),
/// start it again after a short pause, on another thread.
fn restart_after_crash(app: &tauri::AppHandle, role: &str, name: &str, generation: u64, code: Option<i32>) {
    let (Some(sc), Some(_)) = (app.try_state::<Sidecar>(), app.try_state::<AppConfig>()) else { return };
    let current = sc.generation.load(Ordering::SeqCst);
    let restarts = sc.restarts.load(Ordering::SeqCst);
    if !should_restart(generation, current, restarts, code) {
        return;
    }
    sc.restarts.fetch_add(1, Ordering::SeqCst);
    log::warn!("restarting the {role} server (attempt {} of {MAX_RESTARTS})", restarts + 1);
    let (app, role, name) = (app.clone(), role.to_string(), name.to_string());
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let (Some(sc), Some(cfg)) = (app.try_state::<Sidecar>(), app.try_state::<AppConfig>()) else { return };
        let generation = sc.begin(false);
        match spawn(&app, &cfg, &role, &name, generation) {
            Ok(child) => sc.set(child),
            Err(e) => log::error!("couldn't restart the server: {e}"),
        }
    });
}

/// After a node starts it asks the commons it's waiting on whether it has
/// been listed (once). Collect any resulting news from the node a few times
/// over the first minute and show each as a system notification. Only talks
/// to this computer; nothing is polled after that.
fn notify_news(app: tauri::AppHandle, token: String) {
    std::thread::spawn(move || {
        for wait in [10, 20, 30] {
            std::thread::sleep(std::time::Duration::from_secs(wait));
            for text in network::claim_notices(&token) {
                use tauri_plugin_notification::NotificationExt;
                if let Err(e) = app.notification().builder().title("Held in Common").body(&text).show() {
                    log::warn!("couldn't show a notification: {e}");
                }
                log::info!("news: {text}");
            }
        }
    });
}

/// Set the window title to say which role is running.
pub fn set_title(app: &tauri::AppHandle, role: &str) {
    if let Some(win) = app.get_webview_window("main") {
        let name = if role == "commons" { "Commons" } else { "Node" };
        let _ = win.set_title(&format!("Held in Common \u{2014} {name}"));
    }
}

/// The page the window opens on: a node's directory, or a commons' steward view.
pub fn start_page(role: &str) -> &'static str {
    if role == "commons" {
        "/steward"
    } else {
        "/"
    }
}

/// In the background, open the server in the window as soon as *our* server
/// answers (a backstop in case the loading screen's own polling stalls).
/// Gives up after about four minutes.
pub fn open_when_up(app: tauri::AppHandle, token: String, path: &'static str) {
    std::thread::spawn(move || {
        for _ in 0..600 {
            if network::probe(&token) == Probe::Ours {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.eval(format!("location.replace('{LOCAL_URL}{path}')"));
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    });
}

/// Start the server in `role` (a deliberate start: the restart count resets),
/// remember the process, and open it when ready.
pub fn start(app: &tauri::AppHandle, cfg: &AppConfig, sidecar: &Sidecar, role: &str, name: &str) -> Result<(), String> {
    let generation = sidecar.begin(true);
    sidecar.set(spawn(app, cfg, role, name, generation)?);
    set_title(app, role);
    // A commons opens in the steward view (approve people, make invites).
    open_when_up(app.clone(), cfg.launch_token.clone(), start_page(role));
    if role == "node" {
        notify_news(app.clone(), cfg.launch_token.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_commons_opens_in_the_steward_view() {
        assert_eq!(start_page("commons"), "/steward");
        assert_eq!(start_page("node"), "/");
    }

    #[test]
    fn restarts_only_the_current_server_a_few_times() {
        assert!(should_restart(5, 5, 0, Some(1)), "a crash of the current server");
        assert!(should_restart(5, 5, 0, None), "killed by a signal");
        assert!(!should_restart(4, 5, 0, Some(1)), "an old server, replaced or stopped on purpose");
        assert!(!should_restart(5, 5, MAX_RESTARTS, Some(1)), "gave up after too many restarts");
        assert!(!should_restart(5, 5, 0, Some(2)), "refused its settings: restarting won't help");
    }
}
