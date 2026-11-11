//! Starting and stopping the bundled `hic-commons` servers (the "sidecars").
//!
//! The app can run up to three at once, one per [`Kind`]: a node, a commons
//! and a practice commons, each on its own port with its own data folder
//! (see [`crate::settings`]). [`Servers`] holds one [`Sidecar`] per kind.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::Manager;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use crate::network::{self, local_url, Probe};
use crate::settings::{cookie_name, data_dir_for, instance_label, Kind, Saved};

/// Fixed for the whole run of the app; computed once at start-up.
pub struct AppConfig {
    /// The app data folder (holds `app.json` and one data folder per kind).
    pub base: PathBuf,
    /// `HIC_INSTANCE` from the host environment, if set.
    pub instance_override: Option<String>,
    /// The commons pre-filled in "Share to a commons".
    pub share_default: String,
    /// Identifies the servers this launch started (see [`crate::network`]).
    pub launch_token: String,
}

/// One running server process, if any, and what's needed to restart it
/// after a crash.
#[derive(Default)]
pub struct Sidecar {
    child: Mutex<Option<CommandChild>>,
    /// Bumped each time the server is started or deliberately stopped, so an
    /// old process ending can be told apart from the current one crashing.
    generation: AtomicU64,
    /// Automatic restarts since the last deliberate start.
    restarts: AtomicU32,
}

/// One [`Sidecar`] per kind of server.
#[derive(Default)]
pub struct Servers {
    node: Sidecar,
    commons: Sidecar,
    practice: Sidecar,
}

impl Servers {
    /// The slot for `kind`.
    pub fn get(&self, kind: Kind) -> &Sidecar {
        match kind {
            Kind::Node => &self.node,
            Kind::Commons => &self.commons,
            Kind::Practice => &self.practice,
        }
    }

    /// Stop every server (when the app quits).
    pub fn stop_all(&self) {
        for k in Kind::ALL {
            self.get(k).stop();
        }
    }
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

/// The environment a server of `kind` starts with, apart from the data
/// folder and launch token. Pure, so the per-kind differences are testable.
/// `lan` is this computer's network address for the commons, if known.
pub fn server_env(
    kind: Kind,
    port: u16,
    name: &str,
    cfg_override: Option<&str>,
    share_default: &str,
    lan: Option<String>,
) -> Vec<(&'static str, String)> {
    let mut env = vec![
        ("PORT", port.to_string()),
        ("HIC_ROLE", kind.role().to_string()),
        ("HIC_INSTANCE", instance_label(cfg_override, Some(name), kind.role())),
        ("HIC_SESSION_COOKIE", cookie_name(port)),
    ];
    match kind {
        Kind::Node => env.push(("HIC_SHARE_DEFAULT", share_default.to_string())),
        // A room commons: nodes on the same network reach it at this address.
        Kind::Commons => env.extend(lan.map(|url| ("HIC_BASE_URL", url))),
        // Practice: only this computer can reach it, and its pages say so.
        Kind::Practice => env.extend([
            ("HIC_BIND", "127.0.0.1".to_string()),
            ("HIC_BASE_URL", local_url(port)),
            ("HIC_PRACTICE", "1".to_string()),
        ]),
    }
    env
}

/// Start the server of `kind` on `port`, logging its output.
fn spawn(
    app: &tauri::AppHandle,
    cfg: &AppConfig,
    kind: Kind,
    port: u16,
    name: &str,
    generation: u64,
) -> Result<CommandChild, String> {
    let data_dir = data_dir_for(&cfg.base, kind);
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("could not create the data folder: {e}"))?;
    let lan = if kind == Kind::Commons { network::lan_url(port) } else { None };
    let mut cmd = app
        .shell()
        .sidecar("hic-commons")
        .map_err(|e| format!("could not locate the server binary: {e}"))?
        .env("HIC_DATA", data_dir.to_string_lossy().to_string())
        .env("HIC_LAUNCH_TOKEN", &cfg.launch_token);
    for (k, v) in server_env(kind, port, name, cfg.instance_override.as_deref(), &cfg.share_default, lan) {
        cmd = cmd.env(k, v);
    }

    let what = kind.label().to_lowercase();
    let (mut rx, child) = cmd.spawn().map_err(|e| format!("could not start {what}: {e}"))?;
    log::info!("started {what} on port {port} (data in {})", data_dir.display());
    let tag = kind.key();
    let (app, name) = (app.clone(), name.to_string());
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
                    restart_after_crash(&app, kind, port, &name, generation, t.code);
                }
                _ => {}
            }
        }
    });
    Ok(child)
}

/// If the server that just stopped should come back (see [`should_restart`]),
/// start it again after a short pause, on another thread.
fn restart_after_crash(app: &tauri::AppHandle, kind: Kind, port: u16, name: &str, generation: u64, code: Option<i32>) {
    let Some(servers) = app.try_state::<Servers>() else { return };
    let sc = servers.get(kind);
    let current = sc.generation.load(Ordering::SeqCst);
    let restarts = sc.restarts.load(Ordering::SeqCst);
    if !should_restart(generation, current, restarts, code) {
        return;
    }
    sc.restarts.fetch_add(1, Ordering::SeqCst);
    log::warn!("restarting {} (attempt {} of {MAX_RESTARTS})", kind.key(), restarts + 1);
    let (app, name) = (app.clone(), name.to_string());
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let (Some(servers), Some(cfg)) = (app.try_state::<Servers>(), app.try_state::<AppConfig>()) else { return };
        let sc = servers.get(kind);
        let generation = sc.begin(false);
        match spawn(&app, &cfg, kind, port, &name, generation) {
            Ok(child) => sc.set(child),
            Err(e) => log::error!("couldn't restart the server: {e}"),
        }
    });
}

/// After a node starts it asks the commons it's waiting on whether it has
/// been listed (once). Collect any resulting news from the node a few times
/// over the first minute and show each as a system notification. Only talks
/// to this computer; nothing is polled after that.
fn notify_news(app: tauri::AppHandle, port: u16, token: String) {
    std::thread::spawn(move || {
        for wait in [10, 20, 30] {
            std::thread::sleep(std::time::Duration::from_secs(wait));
            for text in network::claim_notices(port, &token) {
                use tauri_plugin_notification::NotificationExt;
                if let Err(e) = app.notification().builder().title("Held in Common").body(&text).show() {
                    log::warn!("couldn't show a notification: {e}");
                }
                log::info!("news: {text}");
            }
        }
    });
}

/// The window title while `kind` is shown.
pub fn title_for(kind: Kind) -> String {
    let name = match kind {
        Kind::Node => "Node",
        Kind::Commons => "Commons",
        Kind::Practice => "Practice commons",
    };
    format!("Held in Common \u{2014} {name}")
}

/// The page the window opens on: a node's directory, or a commons' steward
/// view (approve people, make invites).
pub fn start_page(kind: Kind) -> &'static str {
    if kind == Kind::Node {
        "/"
    } else {
        "/steward"
    }
}

/// Where the window goes to show `kind` on `port`.
pub fn open_url(kind: Kind, port: u16) -> String {
    format!("{}{}", local_url(port), start_page(kind))
}

/// In the background, show the server on `port` in the main window as soon
/// as it answers (as *our* server). Gives up after about four minutes.
pub fn open_when_up(app: tauri::AppHandle, token: String, kind: Kind, port: u16) {
    std::thread::spawn(move || {
        for _ in 0..600 {
            if network::probe(port, &token) == Probe::Ours {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.eval(format!("location.replace('{}')", open_url(kind, port)));
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    });
}

/// Start the server of `kind` as `saved` describes it (a deliberate start:
/// the restart count resets). With `show`, the main window switches to it
/// once it answers.
pub fn start(
    app: &tauri::AppHandle,
    cfg: &AppConfig,
    servers: &Servers,
    saved: &Saved,
    kind: Kind,
    show: bool,
) -> Result<(), String> {
    let name = saved.name_of(kind).ok_or_else(|| format!("{} isn't set up on this computer.", kind.label()))?;
    let port = saved.port_of(kind);
    let sc = servers.get(kind);
    let generation = sc.begin(true);
    sc.set(spawn(app, cfg, kind, port, name, generation)?);
    if show {
        open_when_up(app.clone(), cfg.launch_token.clone(), kind, port);
    }
    if kind == Kind::Node {
        notify_news(app.clone(), port, cfg.launch_token.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_commons_opens_in_the_steward_view() {
        assert_eq!(start_page(Kind::Commons), "/steward");
        assert_eq!(start_page(Kind::Practice), "/steward");
        assert_eq!(start_page(Kind::Node), "/");
        assert_eq!(open_url(Kind::Practice, 8082), "http://127.0.0.1:8082/steward");
    }

    #[test]
    fn restarts_only_the_current_server_a_few_times() {
        assert!(should_restart(5, 5, 0, Some(1)), "a crash of the current server");
        assert!(should_restart(5, 5, 0, None), "killed by a signal");
        assert!(!should_restart(4, 5, 0, Some(1)), "an old server, replaced or stopped on purpose");
        assert!(!should_restart(5, 5, MAX_RESTARTS, Some(1)), "gave up after too many restarts");
        assert!(!should_restart(5, 5, 0, Some(2)), "refused its settings: restarting won't help");
    }

    fn env_of(kind: Kind, port: u16) -> std::collections::HashMap<&'static str, String> {
        server_env(kind, port, "Name", None, "https://c.example", Some("http://192.168.1.5:8081".into()))
            .into_iter()
            .collect()
    }

    #[test]
    fn each_kind_starts_with_its_own_settings() {
        let node = env_of(Kind::Node, 8080);
        assert_eq!(node["HIC_ROLE"], "node");
        assert_eq!(node["HIC_SESSION_COOKIE"], "hic_session");
        assert_eq!(node["HIC_SHARE_DEFAULT"], "https://c.example");
        assert!(!node.contains_key("HIC_BIND"), "a node listens locally by default");

        let commons = env_of(Kind::Commons, 8081);
        assert_eq!(commons["HIC_ROLE"], "commons");
        assert_eq!(commons["HIC_BASE_URL"], "http://192.168.1.5:8081");
        assert_eq!(commons["HIC_SESSION_COOKIE"], "hic_session_8081");
        assert!(!commons.contains_key("HIC_PRACTICE"));

        let practice = env_of(Kind::Practice, 8082);
        assert_eq!(practice["HIC_ROLE"], "commons");
        assert_eq!(practice["HIC_BIND"], "127.0.0.1", "only this computer can reach a practice commons");
        assert_eq!(practice["HIC_BASE_URL"], "http://127.0.0.1:8082");
        assert_eq!(practice["HIC_PRACTICE"], "1");
        assert_eq!(practice["HIC_INSTANCE"], "Name");
    }
}
