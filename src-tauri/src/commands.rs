//! What the app can do: the commands its own screens call (`ui/app.js`, via
//! `invoke`), and the same actions for the **Go** menu and `hic-app://`
//! links (see `lib.rs`).
//!
//! Each action is a plain function taking the [`tauri::AppHandle`] (so the
//! menu and links can call it); the `#[tauri::command]` wrappers below them
//! only adapt arguments.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::network::{self, Probe};
use crate::settings::{self, clean_name, valid_role, Kind, Saved};
use crate::sidecar::{self, open_url, AppConfig, Servers};

/// The label of the small window used to name, add, rename or remove a
/// server (opened from the Go menu or a `hic-app://` link).
pub const SETUP_WINDOW: &str = "setup";

/// What this computer runs and what each is called, as far as known.
pub struct Chosen(pub Mutex<Saved>);

/// The address of the page the main window shows, so a `hic-app://` link
/// can be checked against where it was followed from (see `lib.rs`).
#[derive(Default)]
pub struct Shown(pub Mutex<String>);

/// Shown when another program holds a port.
fn port_taken(port: u16) -> String {
    format!(
        "Another program is using port {port}, so Held in Common can't start. \
         Quit that program (or another copy of this app), then press Start."
    )
}

/// A kind's key from the screens, or the first-launch role when none is given.
fn kind_or_first(which: Option<String>, saved: &Saved) -> Result<Kind, String> {
    match which {
        Some(k) => Kind::parse(&k).ok_or_else(|| "Unknown kind of server.".to_string()),
        None => saved.first().ok_or_else(|| "Choose a node or a commons first.".to_string()),
    }
}

/// Wait up to three seconds for whatever is on `port` to let go of it.
fn wait_until_free(port: u16, token: &str) {
    for _ in 0..30 {
        if network::probe(port, token) == Probe::Nothing {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Write `next` to `app.json`, remember it, and rebuild the Go menu.
fn remember(app: &AppHandle, current: &mut Saved, next: Saved) -> Result<(), String> {
    settings::save(&app.state::<AppConfig>().base, &next)?;
    *current = next;
    crate::refresh_menu(app, current);
    Ok(())
}

// ---- actions (menu, links and screens) -------------------------------------

/// Show `kind` in the main window: switch to it if this computer runs it;
/// otherwise set it up. A node or commons is named first (in the setup
/// window); a practice commons just starts.
pub fn open(app: &AppHandle, kind: Kind) -> Result<(), String> {
    let chosen = app.state::<Chosen>();
    let mut current = chosen.0.lock().unwrap();
    if !current.ready() {
        return Err("Choose a node or a commons first.".into());
    }
    let cfg = app.state::<AppConfig>();
    if current.runs(kind) {
        sidecar::open_when_up(app.clone(), cfg.launch_token.clone(), kind, current.port_of(kind));
        return Ok(());
    }
    if kind != Kind::Practice {
        drop(current);
        open_setup(app, &format!("do=add&role={}", kind.key()), "Set up");
        return Ok(());
    }
    let port = current.port_of(Kind::Practice);
    if network::probe(port, &cfg.launch_token) == Probe::Other {
        return Err(port_taken(port));
    }
    let next = Saved { practice: true, ..current.clone() };
    sidecar::start(app, &cfg, &app.state::<Servers>(), &next, Kind::Practice, true)?;
    remember(app, &mut current, next)
}

/// Stop the practice commons and delete everything in it. The node and any
/// real commons are untouched. The main window goes home if it was showing
/// the practice commons.
pub fn remove_practice(app: &AppHandle) -> Result<(), String> {
    let chosen = app.state::<Chosen>();
    let mut current = chosen.0.lock().unwrap();
    let cfg = app.state::<AppConfig>();
    let port = current.port_of(Kind::Practice);
    app.state::<Servers>().get(Kind::Practice).stop();
    wait_until_free(port, &cfg.launch_token);
    let dir = settings::data_dir_for(&cfg.base, Kind::Practice);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => log::info!("removed the practice commons ({})", dir.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("couldn't delete the practice commons' folder: {e}")),
    }
    // Its name goes with it: a new practice commons starts as "Practice commons".
    let next = Saved { practice: false, practice_name: None, ..current.clone() };
    remember(app, &mut current, next)?;
    let showing_practice = network::local_port(&app.state::<Shown>().0.lock().unwrap()) == Some(port);
    if let (true, Some(home)) = (showing_practice, current.home()) {
        sidecar::open_when_up(app.clone(), cfg.launch_token.clone(), home, current.port_of(home));
    }
    Ok(())
}

/// Open the setup window with `query` (what it should do), or bring it
/// forward if it's open.
pub fn open_setup(app: &AppHandle, query: &str, title: &str) {
    if let Some(win) = app.get_webview_window(SETUP_WINDOW) {
        let _ = win.close();
    }
    let url = tauri::WebviewUrl::App(format!("index.html?{query}").into());
    let built = tauri::WebviewWindowBuilder::new(app, SETUP_WINDOW, url)
        .title(title)
        .inner_size(640.0, 620.0)
        .resizable(false)
        .build();
    if let Err(e) = built {
        log::warn!("couldn't open the setup window: {e}");
    }
}

// ---- commands for the app's own screens ------------------------------------

/// Is *our* server of `which` kind (default: the first-launch role)
/// answering? (Not just "is the port open": see [`crate::network`].) The
/// loading screen polls this; a `fetch` from the app's own page to
/// http://127.0.0.1 would be blocked as mixed content.
#[tauri::command]
pub fn node_up(which: Option<String>, cfg: tauri::State<'_, AppConfig>, chosen: tauri::State<'_, Chosen>) -> bool {
    let saved = chosen.0.lock().unwrap().clone();
    let Ok(kind) = kind_or_first(which, &saved) else { return false };
    network::probe(saved.port_of(kind), &cfg.launch_token) == Probe::Ours
}

/// What to tell someone whose server hasn't started, given what is on its
/// `port`: the port-taken message when another program holds it, otherwise
/// nothing (still starting, or already up). Pure, so it can be tested.
pub fn stall_message(on_port: Probe, port: u16) -> Option<String> {
    match on_port {
        // ADR 0008: never open a server we didn't start; say why instead.
        Probe::Other => Some(port_taken(port)),
        Probe::Nothing | Probe::Ours => None,
    }
}

/// Why `which` kind's server (default: the first-launch role) hasn't come
/// up, if the app can tell: today, only "another program has the port". The
/// loading screen asks once start-up has taken longer than usual. Only reads.
#[tauri::command]
pub fn port_problem(
    which: Option<String>,
    cfg: tauri::State<'_, AppConfig>,
    chosen: tauri::State<'_, Chosen>,
) -> Option<String> {
    let saved = chosen.0.lock().unwrap().clone();
    let kind = kind_or_first(which, &saved).ok()?;
    let port = saved.port_of(kind);
    stall_message(network::probe(port, &cfg.launch_token), port)
}

/// Where the window goes to show `which` kind (default: the first-launch role).
#[tauri::command]
pub fn server_url(
    which: Option<String>,
    cfg: tauri::State<'_, AppConfig>,
    chosen: tauri::State<'_, Chosen>,
) -> Result<String, String> {
    let saved = chosen.0.lock().unwrap().clone();
    let kind = kind_or_first(which, &saved)?;
    Ok(open_url(kind, saved.port_of(kind), &cfg.launch_token))
}

/// What the app's screens need.
#[derive(Serialize)]
pub struct SetupState {
    /// "node" or "commons" (the first-launch role), or null before the person has chosen.
    role: Option<String>,
    /// The first-launch role's name, or null before it is named.
    name: Option<String>,
    /// The node's and commons' names, if this computer runs them.
    node_name: Option<String>,
    commons_name: Option<String>,
    /// What the practice commons is called, while there is one.
    practice_name: Option<String>,
    /// True while there is a practice commons.
    practice: bool,
    /// Where nodes in the same room would reach this computer's commons.
    lan_url: Option<String>,
}

/// What this computer runs, and what a room commons' address would be.
#[tauri::command]
pub fn setup_state(chosen: tauri::State<'_, Chosen>) -> SetupState {
    let s = chosen.0.lock().unwrap().clone();
    SetupState {
        node_name: s.name_of(Kind::Node).map(String::from),
        commons_name: s.name_of(Kind::Commons).map(String::from),
        practice_name: s.name_of(Kind::Practice).map(String::from),
        practice: s.practice,
        lan_url: network::lan_url(s.port_of(Kind::Commons)),
        role: s.role,
        name: s.name,
    }
}

/// Close the setup window (its Back and Cancel buttons, or once it's done).
#[tauri::command]
pub fn close_setup(app: AppHandle) {
    if let Some(w) = app.get_webview_window(SETUP_WINDOW) {
        let _ = w.close();
    }
}

/// First launch: save the role and name, and start the server. The
/// first-launch role keeps port 8080; the other can be added later.
#[tauri::command]
pub fn choose_role(app: AppHandle, role: String, name: String) -> Result<(), String> {
    if !valid_role(&role) {
        return Err("Choose a node or a commons.".into());
    }
    let name = clean_name(&name)?;
    let chosen = app.state::<Chosen>();
    let mut current = chosen.0.lock().unwrap();
    if let Some(existing) = current.role.as_deref().filter(|r| *r != role) {
        return Err(format!("This computer is already set up as a {existing}."));
    }
    let cfg = app.state::<AppConfig>();
    let next = Saved { role: Some(role), name: Some(name), ..current.clone() };
    let kind = next.first().expect("a valid role");
    if network::probe(next.port_of(kind), &cfg.launch_token) == Probe::Other {
        return Err(port_taken(next.port_of(kind)));
    }
    remember(&app, &mut current, next)?;
    sidecar::start(&app, &cfg, &app.state::<Servers>(), &current, kind, false)
}

/// Add the other role (a node beside a commons, or a commons beside a node),
/// named `name`, and start it. The main window switches to it once it answers.
#[tauri::command]
pub fn add_role(app: AppHandle, role: String, name: String) -> Result<(), String> {
    let kind = Kind::parse(&role).filter(|k| *k != Kind::Practice).ok_or("Choose a node or a commons.")?;
    let name = clean_name(&name)?;
    let chosen = app.state::<Chosen>();
    let mut current = chosen.0.lock().unwrap();
    if !current.ready() {
        return Err("Choose a node or a commons first.".into());
    }
    if current.runs(kind) {
        return Err(format!("This computer already runs {}.", kind.label().to_lowercase()));
    }
    let cfg = app.state::<AppConfig>();
    let next = Saved { other_name: Some(name), ..current.clone() };
    let port = next.port_of(kind);
    if network::probe(port, &cfg.launch_token) == Probe::Other {
        return Err(port_taken(port));
    }
    remember(&app, &mut current, next)?;
    sidecar::start(&app, &cfg, &app.state::<Servers>(), &current, kind, true)
}

/// Name (or rename) this computer's node or commons (`role`; default the
/// first-launch role). If it is running, it restarts under the new name.
#[tauri::command]
pub fn set_name(app: AppHandle, role: Option<String>, name: String) -> Result<(), String> {
    let name = clean_name(&name)?;
    let chosen = app.state::<Chosen>();
    let mut current = chosen.0.lock().unwrap();
    let kind = kind_or_first(role, &current)?;
    let next = match kind {
        k if current.first() == Some(k) => Saved { name: Some(name), ..current.clone() },
        Kind::Practice if current.practice => Saved { practice_name: Some(name), ..current.clone() },
        k if k != Kind::Practice && current.runs(k) => Saved { other_name: Some(name), ..current.clone() },
        _ => return Err(format!("This computer doesn't run {}.", kind.label().to_lowercase())),
    };
    let cfg = app.state::<AppConfig>();
    let port = next.port_of(kind);
    remember(&app, &mut current, next)?;
    let servers = app.state::<Servers>();
    servers.get(kind).stop();
    wait_until_free(port, &cfg.launch_token);
    // Reopen it in the main window if that's where it was (or if the main
    // window is still on the app's own screens, naming it for the first time).
    let shown = app.state::<Shown>().0.lock().unwrap().clone();
    let show = network::local_port(&shown).is_none_or(|p| p == port);
    sidecar::start(&app, &cfg, &servers, &current, kind, show)?;
    if let Some(w) = app.get_webview_window(SETUP_WINDOW) {
        let _ = w.close();
    }
    Ok(())
}

/// Start a server again (default: the first-launch role), for the "Start"
/// button the loading screen shows when start-up stalls (the server
/// crashed, or the port was busy).
#[tauri::command]
pub fn restart_node(app: AppHandle, which: Option<String>) -> Result<(), String> {
    let saved = app.state::<Chosen>().0.lock().unwrap().clone();
    let kind = kind_or_first(which, &saved)?;
    if !saved.runs(kind) {
        return Err("Choose a node or a commons, and name it, first.".into());
    }
    let cfg = app.state::<AppConfig>();
    let port = saved.port_of(kind);
    match network::probe(port, &cfg.launch_token) {
        Probe::Ours => return Ok(()), // already running: the screen will open it
        Probe::Other => return Err(port_taken(port)),
        Probe::Nothing => {}
    }
    let servers = app.state::<Servers>();
    servers.get(kind).stop();
    sidecar::start(&app, &cfg, &servers, &saved, kind, false)
}

/// Remove the practice commons (the setup window's confirm button).
#[tauri::command]
pub fn remove_practice_now(app: AppHandle) -> Result<(), String> {
    remove_practice(&app)?;
    close_setup(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn another_program_on_the_port_is_named_not_opened() {
        let msg = stall_message(Probe::Other, 8080).expect("a message when another program has the port");
        assert!(msg.contains("port 8080"), "{msg}");
    }

    #[test]
    fn a_server_still_starting_or_already_ours_needs_no_message() {
        assert_eq!(stall_message(Probe::Nothing, 8080), None);
        assert_eq!(stall_message(Probe::Ours, 8080), None);
    }
}
