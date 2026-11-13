//! Starting and stopping the bundled `hic-commons` servers (the "sidecars").
//!
//! The app can run up to three at once, one per [`Kind`]: a node, a commons
//! and a local commons, each on its own port with its own data folder
//! (see [`crate::settings`]). [`Servers`] holds one [`Sidecar`] per kind.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::Manager;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use crate::network::{self, local_url, Probe};
use crate::settings::{cookie_name, data_dir_for, instance_label, Kind, Saved};
use crate::steward;

/// Fixed for the whole run of the app; computed once at start-up.
pub struct AppConfig {
    /// The app data folder (holds `app.json` and one data folder per kind).
    pub base: PathBuf,
    /// `HIC_INSTANCE` from the host environment, if set.
    pub instance_override: Option<String>,
    /// The commons pre-filled in "Share to a commons", and its name.
    pub share_default: DefaultCommons,
    /// Identifies the servers this launch started (see [`crate::network`]).
    pub launch_token: String,
}

/// The commons a node offers first: its address and, if known, what to call
/// it ("Held in Common Collective Commons"), so pages show a name rather
/// than a long Tailscale address.
#[derive(Clone, Debug, Default)]
pub struct DefaultCommons {
    /// Its address, e.g. `https://heldincommon.example.ts.net` (blank: none).
    pub url: String,
    /// What to call it (blank: pages show the address).
    pub name: String,
}

/// One running server process, if any, and what's needed to restart it
/// after a crash.
#[derive(Default)]
pub struct Sidecar {
    /// The running process, while there is one.
    child: Mutex<Option<CommandChild>>,
    /// Bumped each time the server is started or deliberately stopped, so an
    /// old process ending can be told apart from the current one crashing.
    generation: AtomicU64,
    /// Automatic restarts since the last deliberate start.
    restarts: AtomicU32,
    /// The network address it was started on (`http://10.0.0.5:8082`), for
    /// a commons or local commons: the router can hand this computer a new
    /// one, and [`watch_addresses`] moves the server when it does.
    address: Mutex<Option<String>>,
    /// Open to the room (decision record 0027)? Only ever in memory, never in
    /// `app.json`, so every launch starts closed. Only a node or a local
    /// commons can be open (see [`crate::room::has_switch`]).
    open: AtomicBool,
    /// Held while the server in this slot is stopped and started again
    /// (opening or closing the room, a crash restart, a new address), so two
    /// of those can't interleave and leave a server the app has lost track of.
    changing: Mutex<()>,
    /// The room code and firewall hint made when it was last opened to the
    /// room (decision record 0027); used only while it's open.
    extras: Mutex<crate::room::RoomExtras>,
}

/// One [`Sidecar`] per kind of server.
#[derive(Default)]
pub struct Servers {
    /// The node's server.
    node: Sidecar,
    /// The commons' server.
    commons: Sidecar,
    /// The local commons' server (`practice` in code).
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

    /// The address each running server can be reached at by other people,
    /// for the switcher: a commons always, a node or local commons while
    /// open to the room. Only reads.
    pub fn addresses(&self) -> Vec<(Kind, String)> {
        Kind::ALL.into_iter().filter_map(|k| self.get(k).address().map(|a| (k, a))).collect()
    }

    /// The kinds open to the room right now. Only reads.
    pub fn open_kinds(&self) -> Vec<Kind> {
        Kind::ALL.into_iter().filter(|k| self.get(*k).is_open()).collect()
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

    /// Remember the running process and the network address it was started
    /// on.
    /// If one was already recorded (two starts raced), that one is killed
    /// first, so no server keeps running that the app can't stop.
    fn set(&self, child: CommandChild, address: Option<String>) {
        if let Some(old) = self.child.lock().unwrap().replace(child) {
            let _ = old.kill();
        }
        *self.address.lock().unwrap() = address;
    }

    /// Hold this while stopping and starting the server again, so nothing
    /// else does the same at the same time (see `changing`).
    pub fn changing(&self) -> std::sync::MutexGuard<'_, ()> {
        self.changing.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Is a server running in this slot?
    fn is_running(&self) -> bool {
        self.child.lock().unwrap().is_some()
    }

    /// The network address it was started on, if any.
    fn address(&self) -> Option<String> {
        self.address.lock().unwrap().clone()
    }

    /// Is it open to the room? Only reads.
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::SeqCst)
    }

    /// The room code and firewall hint it gets while open. Only reads.
    pub fn extras(&self) -> crate::room::RoomExtras {
        self.extras.lock().unwrap().clone()
    }

    /// Remember the room code and firewall hint for the next time it starts
    /// open to the room. Changes only this, in memory.
    pub fn set_extras(&self, extras: crate::room::RoomExtras) {
        *self.extras.lock().unwrap() = extras;
    }

    /// Record whether it should be open to the room the next time it starts.
    /// Changes only this flag, in memory; the caller restarts the server.
    pub fn set_open(&self, open: bool) {
        self.open.store(open, Ordering::SeqCst);
    }

    /// Stop the server if it is running. It won't be restarted automatically,
    /// and it's closed to the room: whatever starts it next decides again
    /// (decision record 0027, so nothing reopens it without being asked).
    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.open.store(false, Ordering::SeqCst);
        if let Some(child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
}

/// The environment a server of `kind` starts with, apart from the data
/// folder and launch token. Pure, so the per-kind differences are testable.
/// `lan` is this computer's network address for the server, if known;
/// `public` a commons' address outside the room, if set in `app.json`; and
/// `open` whether a node or local commons is open to the room (decision
/// record 0027), which only counts when there is a `lan` address.
#[allow(clippy::too_many_arguments)]
pub fn server_env(
    kind: Kind,
    port: u16,
    name: &str,
    cfg_override: Option<&str>,
    share_default: &DefaultCommons,
    lan: Option<String>,
    public: Option<String>,
    open: bool,
) -> Vec<(&'static str, String)> {
    // The address the room reaches it at, only while it's open, and the IP
    // to listen on: that network only, not every one (a VPN, Tailscale).
    let room = lan.clone().filter(|_| open && crate::room::has_switch(kind));
    let room_ip = room.as_deref().and_then(crate::room::ip_of).unwrap_or_else(|| "0.0.0.0".to_string());
    let mut env = vec![
        ("PORT", port.to_string()),
        ("HIC_ROLE", kind.role().to_string()),
        ("HIC_INSTANCE", instance_label(cfg_override, Some(name), kind.role())),
        ("HIC_SESSION_COOKIE", cookie_name(port)),
        // Stop when this app does, even after a crash or a force-quit (decision
        // record 0027): the server watches its stdin, whose writing end the
        // app holds in `CommandChild` for as long as it runs.
        ("HIC_EXIT_WITH_APP", "1".to_string()),
    ];
    match kind {
        Kind::Node => {
            env.extend([
                ("HIC_SHARE_DEFAULT", share_default.url.clone()),
                ("HIC_SHARE_DEFAULT_NAME", share_default.name.clone()),
            ]);
            // Open to the room: listen on the network with the door in front
            // (`HIC_ROOM`), and know the address the room uses, for the QR
            // code. The node's own address (`HIC_BASE_URL`) stays as it was,
            // so nothing it shares names an address that stops working
            // when the room closes.
            if let Some(url) = room.clone() {
                env.extend([("HIC_ROOM", "1".to_string()), ("HIC_BIND", room_ip.clone()), ("HIC_ROOM_ADDRESS", url)]);
            }
        }
        // A room commons: nodes on the same network reach it at this address,
        // and people outside the room at its public one, if it has one.
        Kind::Commons => {
            env.extend(lan.map(|url| ("HIC_BASE_URL", url)));
            env.extend(public.map(|url| ("HIC_ALSO_KNOWN_AS", url)));
            // Open to the room: it already listens on the network, so this
            // adds the door (the room never reaches the steward view) and
            // the room page with its join link (decision record 0027).
            if let Some(url) = room.clone() {
                env.extend([("HIC_ROOM", "1".to_string()), ("HIC_ROOM_ADDRESS", url)]);
            }
        }
        // A local commons: for a workshop, a class or a session between
        // friends. Closed, only this computer reaches it. Open to the room,
        // anyone on the same Wi-Fi reaches it at this computer's network
        // address, which its pages show, with the door in front of the
        // host's own pages (decision record 0027). Joining still needs a
        // vouch (ADR 0011) or the room code.
        Kind::Practice => {
            match room.clone() {
                Some(url) => env.extend([
                    ("HIC_BASE_URL", url.clone()),
                    ("HIC_ROOM", "1".to_string()),
                    ("HIC_BIND", room_ip.clone()),
                    ("HIC_ROOM_ADDRESS", url),
                ]),
                None => env.extend([("HIC_BIND", "127.0.0.1".to_string()), ("HIC_BASE_URL", local_url(port))]),
            }
            env.push(("HIC_PRACTICE", "1".to_string()));
        }
    }
    env
}

/// For a node: the commons the app runs on this computer, as
/// `HIC_SHARE_NEARBY` (`http://127.0.0.1:8180=spark,…`), so the node offers
/// each as a one-click share. Their `127.0.0.1` addresses never change, so
/// sharing keeps working when the Wi-Fi does; pages show them at this
/// computer's network address. Commas and `=` in names are made spaces.
/// Pure.
pub fn nearby_commons(saved: &Saved) -> String {
    [Kind::Commons, Kind::Practice]
        .into_iter()
        .filter(|k| saved.runs(*k))
        .map(|k| {
            let name = saved.shown_name(k).replace([',', '='], " ");
            format!("{}={name}", local_url(saved.port_of(k)))
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Start the server of `kind` on `port`, logging its output, open to the
/// room if `open` (and there is a network). Returns the process and the
/// network address people reach it at: always for a commons, and for a node
/// or local commons only while it's open to the room (`None` when closed or
/// when this computer has no network).
fn spawn(
    app: &tauri::AppHandle,
    cfg: &AppConfig,
    kind: Kind,
    port: u16,
    name: &str,
    generation: u64,
    room: Option<&crate::room::RoomExtras>,
) -> Result<(CommandChild, Option<String>), String> {
    let open = room.is_some();
    let data_dir = data_dir_for(&cfg.base, kind);
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("could not create the data folder: {e}"))?;
    // A commons is always reached over the Wi-Fi; a node or a local commons
    // only while it's open to the room.
    let lan = network::lan_url(port);
    let reached_at = if kind == Kind::Commons || open { lan.clone() } else { None };
    let public = if kind == Kind::Commons { crate::settings::load(&cfg.base).0.public_address } else { None };
    let mut cmd = app
        .shell()
        .sidecar("hic-commons")
        .map_err(|e| format!("could not locate the server binary: {e}"))?
        .env("HIC_DATA", data_dir.to_string_lossy().to_string())
        .env("HIC_LAUNCH_TOKEN", &cfg.launch_token);
    for (k, v) in server_env(kind, port, name, cfg.instance_override.as_deref(), &cfg.share_default, lan, public, open)
    {
        cmd = cmd.env(k, v);
    }
    // A node offers the commons on this computer as one-click shares.
    if kind == Kind::Node {
        cmd = cmd.env("HIC_SHARE_NEARBY", nearby_commons(&crate::settings::load(&cfg.base).0));
    }
    // Open to the room, with somewhere to open: its room code and hint.
    if let (Some(extras), Some(_)) = (room, reached_at.as_ref()) {
        for (k, v) in crate::room::room_env(kind, extras) {
            cmd = cmd.env(k, v);
        }
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
                // ADR 0024: the steward key is hidden before the line is
                // logged, since people send this log to get help.
                CommandEvent::Stdout(l) => {
                    log::info!("[{tag}] {}", steward::redact_for_log(String::from_utf8_lossy(&l).trim_end()))
                }
                CommandEvent::Stderr(l) => {
                    log::warn!("[{tag}] {}", steward::redact_for_log(String::from_utf8_lossy(&l).trim_end()))
                }
                CommandEvent::Error(e) => log::error!("[{tag}] {e}"),
                CommandEvent::Terminated(t) => {
                    log::warn!("[{tag}] server stopped (exit code {:?}, signal {:?})", t.code, t.signal);
                    restart_after_crash(&app, kind, port, &name, generation, t.code);
                }
                _ => {}
            }
        }
    });
    Ok((child, reached_at))
}

/// If the server that just stopped should come back (see [`should_restart`]),
/// start it again after a short pause, on another thread.
fn restart_after_crash(app: &tauri::AppHandle, kind: Kind, port: u16, name: &str, generation: u64, code: Option<i32>) {
    let Some(servers) = app.try_state::<Servers>() else { return };
    let sc = servers.get(kind);
    let current = sc.generation.load(Ordering::SeqCst);
    // The current server ended without being asked to: a crash closes the
    // room (decision record 0027, D3), whether or not it's restarted, so
    // nothing brings it back open without the person choosing again.
    if generation == current && sc.is_open() {
        sc.set_open(false);
        log::warn!("{} was open to the room and stopped unexpectedly; it is now closed", kind.key());
        if let Some(chosen) = app.try_state::<crate::commands::Chosen>() {
            let saved = chosen.0.lock().unwrap().clone();
            crate::refresh_menu(app, &saved);
        }
    }
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
        let _changing = sc.changing();
        // Someone started or stopped it while we waited (opened it to the
        // room, say): theirs is the server to keep.
        if sc.generation.load(Ordering::SeqCst) != current {
            return;
        }
        let generation = sc.begin(false);
        match spawn(&app, &cfg, kind, port, &name, generation, None) {
            Ok((child, address)) => sc.set(child, address),
            Err(e) => log::error!("couldn't restart the server: {e}"),
        }
    });
}

/// Has the network address a commons was started on changed? Returns the new
/// address to restart on, or `None` to leave it. Pure.
///
/// - The router gave this computer a new address: move to it.
/// - It started with no network and now has one: move to it, so people can
///   reach it.
/// - The network went away: leave it running; it moves when one comes back.
pub fn moved_to(started_on: Option<&str>, now: Option<&str>) -> Option<String> {
    match (started_on, now) {
        (Some(was), Some(is)) if was != is => Some(is.to_string()),
        (None, Some(is)) => Some(is.to_string()),
        _ => None,
    }
}

/// How often [`watch_addresses`] looks.
const ADDRESS_CHECK: std::time::Duration = std::time::Duration::from_secs(60);

/// Every minute, check this computer's network address for each server
/// that's reachable on the network. Runs for as long as the app does.
///
/// - A **commons** moves: when the router has given the computer a new
///   address, restart it there (its data stays), and tell the person with a
///   notification and in the log, so they can give the room the new address.
/// - A **node or local commons open to the room** closes instead (decision
///   record 0027, D3): a new address means the laptop has probably left the
///   room, so it restarts closed, with a notification saying so.
pub fn watch_addresses(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(ADDRESS_CHECK);
        let (Some(servers), Some(cfg), Some(chosen), Some(shown)) = (
            app.try_state::<Servers>(),
            app.try_state::<AppConfig>(),
            app.try_state::<crate::commands::Chosen>(),
            app.try_state::<crate::commands::Shown>(),
        ) else {
            continue;
        };
        let saved = chosen.0.lock().unwrap().clone();
        for kind in Kind::ALL {
            let sc = servers.get(kind);
            if !saved.runs(kind) || !sc.is_running() {
                continue;
            }
            let port = saved.port_of(kind);
            let now = network::lan_url(port);
            let _changing = sc.changing();
            let text = if kind == Kind::Commons {
                let Some(new) = moved_to(sc.address().as_deref(), now.as_deref()) else { continue };
                log::info!("{} moved to {new}: this computer has a new network address", kind.key());
                format!(
                    "{} has a new address: {new}. Your Wi-Fi gave this computer a new one. Tell people who share to it.",
                    saved.shown_name(kind)
                )
            } else if sc.is_open() && crate::room::should_close(sc.address().as_deref(), now.as_deref()) {
                // `stop` below closes it to the room.
                log::info!("{} closed to the room: this computer changed network", kind.key());
                crate::room::closed_notice(&saved.shown_name(kind))
            } else {
                continue;
            };
            // Restart it (moved, or closed), and keep showing it if the window was.
            let show = network::local_port(&shown.0.lock().unwrap()).is_some_and(|p| p == port);
            sc.stop();
            crate::commands::wait_until_free(port, &cfg.launch_token);
            let started = start(&app, &cfg, &servers, &saved, kind, show);
            crate::refresh_menu(&app, &saved);
            if let Err(e) = started {
                log::error!("couldn't restart {}: {e}", kind.key());
                continue;
            }
            use tauri_plugin_notification::NotificationExt;
            if let Err(e) = app.notification().builder().title("Held in Common").body(&text).show() {
                log::warn!("couldn't show a notification: {e}");
            }
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

/// The main window's size, in logical pixels, for a screen of `screen`
/// logical pixels `(width, height)`: about 80% of the width and 85% of the
/// height, so everything fits with room around it, but no bigger than
/// 1440×1000 (a 4K screen would otherwise give a window too wide to read) and
/// no smaller than 760×560. On a screen smaller than that minimum, the whole
/// screen. Pages scroll inside the window when they're longer.
pub fn window_size(screen: (f64, f64)) -> (f64, f64) {
    let (sw, sh) = screen;
    let width = (sw * 0.8).clamp(760.0, 1440.0).min(sw);
    let height = (sh * 0.85).clamp(560.0, 1000.0).min(sh);
    (width.round(), height.round())
}

/// The window title while `kind` is shown.
pub fn title_for(kind: Kind) -> String {
    let name = match kind {
        Kind::Node => "Node",
        Kind::Commons => "Commons",
        Kind::Practice => "Local commons",
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

/// Where the window goes to show `kind` on `port`. A commons' steward view
/// carries the launch `token`, which is what proves the steward (ADR 0023).
pub fn open_url(kind: Kind, port: u16, token: &str) -> String {
    let page = start_page(kind);
    if page == "/steward" {
        format!("{}{page}?launch={token}", local_url(port))
    } else {
        format!("{}{page}", local_url(port))
    }
}

/// In the background, show the server on `port` in the main window as soon
/// as it answers (as *our* server). Gives up after about four minutes.
pub fn open_when_up(app: tauri::AppHandle, token: String, kind: Kind, port: u16) {
    let url = open_url(kind, port, &token);
    show_when_up(app, token, port, url);
}

/// In the background, show `url` (a page of the server on `port`) in the
/// main window as soon as that server answers as *our* server. Gives up
/// after about four minutes.
pub fn show_when_up(app: tauri::AppHandle, token: String, port: u16, url: String) {
    std::thread::spawn(move || {
        for _ in 0..600 {
            if network::probe(port, &token) == Probe::Ours {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if let Some(win) = app.get_webview_window("main") {
                    // The address is ours (127.0.0.1, a port and a fixed path),
                    // so it can go into the script as it is.
                    let _ = win.eval(format!("location.replace('{url}')"));
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    });
}

/// The room page of the server on `port`: its QR code and the address the
/// room uses (decision record 0027).
pub fn room_page_url(port: u16) -> String {
    format!("{}/room", local_url(port))
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
    let open = sc.is_open();
    let extras = sc.extras();
    let (child, address) = spawn(app, cfg, kind, port, name, generation, open.then_some(&extras))?;
    // Asked to open, but there was no network to open it on: it's closed.
    if open && address.is_none() {
        sc.set_open(false);
    }
    sc.set(child, address);
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
    fn a_commons_moves_when_the_router_gives_a_new_address() {
        let (a, b) = ("http://10.0.0.5:8082", "http://10.0.0.9:8082");
        assert_eq!(moved_to(Some(a), Some(b)), Some(b.to_string()), "a new address: move");
        assert_eq!(moved_to(Some(a), Some(a)), None, "the same: leave it");
        assert_eq!(moved_to(None, Some(b)), Some(b.to_string()), "the network came back: move onto it");
        assert_eq!(moved_to(Some(a), None), None, "the network went away: leave it running");
        assert_eq!(moved_to(None, None), None);
    }

    #[test]
    fn a_commons_opens_in_the_steward_view() {
        assert_eq!(start_page(Kind::Commons), "/steward");
        assert_eq!(start_page(Kind::Practice), "/steward");
        assert_eq!(start_page(Kind::Node), "/");
        assert_eq!(open_url(Kind::Practice, 8082, "t0k"), "http://127.0.0.1:8082/steward?launch=t0k");
        assert_eq!(open_url(Kind::Node, 8080, "t0k"), "http://127.0.0.1:8080/", "a node needs no token");
    }

    #[test]
    fn the_window_fits_the_screen_with_room_around_it() {
        assert_eq!(window_size((1440.0, 900.0)), (1152.0, 765.0), "a laptop");
        assert_eq!(window_size((2865.0, 1611.0)), (1440.0, 1000.0), "a 4K screen at 134%: capped");
        assert_eq!(window_size((800.0, 600.0)), (760.0, 560.0), "small: the minimum");
        assert_eq!(window_size((700.0, 500.0)), (700.0, 500.0), "tiny: never bigger than the screen");
    }

    #[test]
    fn restarts_only_the_current_server_a_few_times() {
        assert!(should_restart(5, 5, 0, Some(1)), "a crash of the current server");
        assert!(should_restart(5, 5, 0, None), "killed by a signal");
        assert!(!should_restart(4, 5, 0, Some(1)), "an old server, replaced or stopped on purpose");
        assert!(!should_restart(5, 5, MAX_RESTARTS, Some(1)), "gave up after too many restarts");
        assert!(!should_restart(5, 5, 0, Some(2)), "refused its settings: restarting won't help");
    }

    /// The settings a server of `kind` on `port` starts with, closed to the
    /// room, as a lookup table.
    fn env_of(kind: Kind, port: u16) -> std::collections::HashMap<&'static str, String> {
        env_with(kind, port, false)
    }

    /// The settings a server of `kind` on `port` starts with, open to the
    /// room if `open`, as a lookup table.
    fn env_with(kind: Kind, port: u16, open: bool) -> std::collections::HashMap<&'static str, String> {
        server_env(
            kind,
            port,
            "Name",
            None,
            &DefaultCommons { url: "https://c.example".into(), name: "The Collective".into() },
            Some("http://192.168.1.5:8081".into()),
            Some("https://hic.tail.ts.net".into()),
            open,
        )
        .into_iter()
        .collect()
    }

    #[test]
    fn each_kind_starts_with_its_own_settings() {
        let node = env_of(Kind::Node, 8080);
        assert_eq!(node["HIC_ROLE"], "node");
        assert_eq!(node["HIC_SESSION_COOKIE"], "hic_session");
        assert_eq!(node["HIC_SHARE_DEFAULT"], "https://c.example");
        assert_eq!(node["HIC_SHARE_DEFAULT_NAME"], "The Collective");
        assert!(!node.contains_key("HIC_BIND"), "a node listens locally by default");

        let commons = env_of(Kind::Commons, 8081);
        assert_eq!(commons["HIC_ROLE"], "commons");
        assert_eq!(commons["HIC_BASE_URL"], "http://192.168.1.5:8081");
        assert_eq!(commons["HIC_SESSION_COOKIE"], "hic_session_8081");
        assert_eq!(commons["HIC_ALSO_KNOWN_AS"], "https://hic.tail.ts.net");
        assert!(!env_of(Kind::Node, 8080).contains_key("HIC_ALSO_KNOWN_AS"), "only a commons has one");
        assert!(!commons.contains_key("HIC_PRACTICE"));

        let local = env_of(Kind::Practice, 8082);
        assert_eq!(local["HIC_ROLE"], "commons");
        assert_eq!(local["HIC_BIND"], "127.0.0.1", "a local commons starts closed to the room");
        assert_eq!(local["HIC_BASE_URL"], "http://127.0.0.1:8082");
        assert!(!local.contains_key("HIC_ROOM"));
        assert_eq!(local["HIC_PRACTICE"], "1");
        assert_eq!(local["HIC_INSTANCE"], "Name");
        assert!(!local.contains_key("HIC_ALSO_KNOWN_AS"), "a local commons is not online");
    }

    #[test]
    fn a_node_open_to_the_room_listens_on_the_network_behind_the_door() {
        let node = env_with(Kind::Node, 8080, true);
        assert_eq!(node["HIC_ROOM"], "1");
        assert_eq!(node["HIC_BIND"], "192.168.1.5", "the room's network only, not a VPN or Tailscale");
        assert_eq!(node["HIC_ROOM_ADDRESS"], "http://192.168.1.5:8081");
        assert!(!node.contains_key("HIC_BASE_URL"), "what it shares keeps its own address");
        let closed = env_of(Kind::Node, 8080);
        assert!(!closed.contains_key("HIC_ROOM") && !closed.contains_key("HIC_BIND"), "closed: as before");
    }

    #[test]
    fn a_local_commons_open_to_the_room_is_reached_at_its_network_address() {
        let local = env_with(Kind::Practice, 8082, true);
        assert_eq!(local["HIC_ROOM"], "1");
        assert_eq!(local["HIC_BIND"], "192.168.1.5");
        assert_eq!(local["HIC_BASE_URL"], "http://192.168.1.5:8081");
        assert_eq!(local["HIC_ROOM_ADDRESS"], "http://192.168.1.5:8081");
    }

    #[test]
    fn a_full_commons_open_to_the_room_keeps_its_address_and_adds_the_door() {
        let open = env_with(Kind::Commons, 8081, true);
        let closed = env_of(Kind::Commons, 8081);
        assert_eq!(open["HIC_ROOM"], "1");
        assert_eq!(open["HIC_ROOM_ADDRESS"], "http://192.168.1.5:8081");
        assert_eq!(open["HIC_BASE_URL"], closed["HIC_BASE_URL"], "same address");
        assert!(!open.contains_key("HIC_BIND"), "it listens as it always does");
        assert!(!closed.contains_key("HIC_ROOM"));
    }

    #[test]
    fn a_node_is_told_the_commons_on_this_computer() {
        let saved = Saved {
            role: Some("commons".into()),
            name: Some("spark".into()),
            other_name: Some("potato".into()),
            practice: true,
            practice_name: Some("Thurs, day=1".into()),
            ..Default::default()
        };
        let base = crate::settings::port_offset();
        assert_eq!(
            nearby_commons(&saved),
            format!("http://127.0.0.1:{}=spark,http://127.0.0.1:{}=Thurs  day 1", 8080 + base, 8082 + base)
        );
        assert_eq!(nearby_commons(&Saved { role: Some("node".into()), ..Default::default() }), "");
    }

    #[test]
    fn every_server_stops_with_the_app() {
        for (kind, port) in [(Kind::Node, 8080), (Kind::Commons, 8081), (Kind::Practice, 8082)] {
            assert_eq!(env_of(kind, port)["HIC_EXIT_WITH_APP"], "1", "{kind:?}");
        }
    }

    #[test]
    fn a_local_commons_without_a_network_listens_only_here() {
        let env: std::collections::HashMap<&str, String> = server_env(
            Kind::Practice,
            8082,
            "Name",
            None,
            &DefaultCommons::default(),
            None,
            Some("https://hic.tail.ts.net".into()),
            true,
        )
        .into_iter()
        .collect();
        assert_eq!(env["HIC_BIND"], "127.0.0.1", "asked to open, but there is no network to open on");
        assert!(!env.contains_key("HIC_ROOM"));
        assert_eq!(env["HIC_BASE_URL"], "http://127.0.0.1:8082");
    }
}
