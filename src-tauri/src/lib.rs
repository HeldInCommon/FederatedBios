//! Held in Common — desktop app.
//!
//! A thin native wrapper around the bundled `hic-commons` binary. On first
//! launch the person chooses what this computer runs: a **node** (their own
//! profile and work) or a **commons** (a shared directory other nodes share to).
//! The choice is saved in the app's data folder; after that the app starts the
//! binary as a sidecar in that role and shows its web UI. No Docker, no terminal.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

const PORT: u16 = 8080;

// The commons address prefilled in the node's "Share to a commons" form is read
// from the host HIC_SHARE_DEFAULT env var. Blank by default: no URL is baked into
// the app — workshop users paste the address their facilitator shares.

// Injected into every loaded page (node or remote commons) so the user always
// knows which they're looking at and can move between them. Local node = jade bar
// with a "visit the commons" button; any remote commons = violet bar with a
// "back to your node" button. The node can't edit a remote commons' HTML, but the
// app can inject this from the host side, so it works everywhere.
const APP_BAR_JS: &str = r#"(function(){
  var NODE='http://localhost:8080';
  var isNode=location.href.indexOf('localhost:8080')>-1||location.href.indexOf('127.0.0.1:8080')>-1;
  var id='hic-back-node';
  var old=document.getElementById(id); if(old){old.remove();}
  if(isNode){return;}
  var b=document.createElement('button'); b.id=id;
  b.textContent='← Back to your node';
  b.style.cssText='position:fixed;top:9px;right:12px;z-index:2147483647;cursor:pointer;border:0;padding:8px 12px;font:700 11px/1 ui-monospace,Menlo,monospace;letter-spacing:.07em;text-transform:uppercase;color:#fff;background:#0a7a54;box-shadow:0 2px 10px rgba(0,0,0,.3);';
  b.onclick=function(){location.href=NODE;};
  document.body.appendChild(b);
})();"#;

/// What the app needs to (re)spawn the sidecar. Computed once at startup and kept
/// in managed state so `choose_role` and `restart_node` can spawn later.
struct AppConfig {
    /// Per-user app data folder. Holds `app.json` and one data folder per role.
    base: PathBuf,
    /// Account name, used to label the instance ("phoenix's node").
    who: String,
    /// Host HIC_INSTANCE override, if set.
    instance_override: Option<String>,
    share_default: String,
}

/// The role chosen for this computer, once known.
struct ChosenRole(Mutex<Option<String>>);

/// Handle to the running sidecar. Kept alive here so the process lives as long as
/// the app; `restart_node` takes it to kill a stuck child before respawning.
struct NodeProc(Mutex<Option<CommandChild>>);

/// `app.json` in the app data folder: which role this computer runs.
#[derive(Serialize, Deserialize)]
struct AppFile {
    role: String,
}

fn valid_role(r: &str) -> bool {
    r == "node" || r == "commons"
}

/// Each role keeps its own data folder. `node-data` is the folder earlier
/// versions of the app used, so existing nodes keep their data.
fn data_dir_for(base: &Path, role: &str) -> PathBuf {
    base.join(if role == "commons" { "commons-data" } else { "node-data" })
}

/// The saved role, if any. Installs from before the chooser have no `app.json`
/// but already have a node with members: treat those as a node and save it, so
/// existing users never see the chooser.
fn load_role(base: &Path) -> Option<String> {
    let saved = std::fs::read_to_string(base.join("app.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<AppFile>(&s).ok())
        .map(|f| f.role)
        .filter(|r| valid_role(r));
    if saved.is_some() {
        return saved;
    }
    let has_members = std::fs::read_dir(base.join("node-data").join("handles"))
        .map(|mut d| d.any(|e| e.map(|e| e.path().extension().map_or(false, |x| x == "json")).unwrap_or(false)))
        .unwrap_or(false);
    if has_members {
        let _ = save_role(base, "node");
        return Some("node".into());
    }
    None
}

fn save_role(base: &Path, role: &str) -> Result<(), String> {
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&AppFile { role: role.into() }).map_err(|e| e.to_string())?;
    std::fs::write(base.join("app.json"), json).map_err(|e| format!("could not save your choice: {e}"))
}

/// This computer's address on the local network, for a commons that people in
/// the same room share to. Connecting a UDP socket sends nothing; it only asks
/// the OS which interface it would route through.
fn lan_url() -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let ip = sock.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    Some(format!("http://{ip}:{PORT}"))
}

fn instance_label(cfg: &AppConfig, role: &str) -> String {
    if let Some(i) = &cfg.instance_override {
        return i.clone();
    }
    match (cfg.who.is_empty(), role) {
        (true, r) => format!("a Held in Common {r}"),
        (false, r) => format!("{}'s {r}", cfg.who),
    }
}

/// Spawn the bundled `hic-commons` binary on port 8080 in the given role.
fn spawn_server(app: &tauri::AppHandle, cfg: &AppConfig, role: &str) -> Result<CommandChild, String> {
    let data_dir = data_dir_for(&cfg.base, role);
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("could not create the data folder: {e}"))?;
    let mut cmd = app
        .shell()
        .sidecar("hic-commons")
        .map_err(|e| format!("could not locate the server binary: {e}"))?
        .env("HIC_DATA", data_dir.to_string_lossy().to_string())
        .env("PORT", PORT.to_string())
        .env("HIC_INSTANCE", instance_label(cfg, role))
        .env("HIC_ROLE", role);
    if role == "node" {
        cmd = cmd.env("HIC_SHARE_DEFAULT", &cfg.share_default);
    } else if let Some(url) = lan_url() {
        // A room commons: nodes on the same network reach it at this address.
        cmd = cmd.env("HIC_BASE_URL", url);
    }

    let (mut rx, child) = cmd.spawn().map_err(|e| format!("could not start the {role}: {e}"))?;
    let tag = role.to_string();
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            if let CommandEvent::Stdout(l) | CommandEvent::Stderr(l) = ev {
                eprintln!("[{tag}] {}", String::from_utf8_lossy(&l));
            }
        }
    });
    Ok(child)
}

fn set_title(app: &tauri::AppHandle, role: &str) {
    if let Some(win) = app.get_webview_window("main") {
        let name = if role == "commons" { "Commons" } else { "Node" };
        let _ = win.set_title(&format!("Held in Common \u{2014} {name}"));
    }
}

/// Host-side backstop: navigate the window to the server as soon as its port
/// answers, even if the loading screen's own polling stalls.
fn open_when_up(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        for _ in 0..600 {
            if node_up() {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if let Some((_, win)) = app.webview_windows().iter().next() {
                    let _ = win.eval("location.replace('http://localhost:8080')");
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    });
}

/// Start the server in `role` and remember the child process.
fn start(app: &tauri::AppHandle, cfg: &AppConfig, proc: &NodeProc, role: &str) -> Result<(), String> {
    let child = spawn_server(app, cfg, role)?;
    *proc.0.lock().unwrap() = Some(child);
    set_title(app, role);
    open_when_up(app.clone());
    Ok(())
}

/// Is the server accepting connections on 8080? The loading screen polls this via
/// `invoke` instead of `fetch` — a fetch from the app's secure origin to
/// http://localhost is blocked as mixed content. A TCP connect from the host side
/// has no such restriction. If the app was launched twice, the second window sees
/// the first server already listening and simply navigates to it.
#[tauri::command]
fn node_up() -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], PORT));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(400)).is_ok()
}

#[derive(Serialize)]
struct SetupState {
    /// "node", "commons", or null when the person hasn't chosen yet.
    role: Option<String>,
    /// Where nodes in the same room would reach a commons on this computer.
    lan_url: Option<String>,
}

/// What the first screen needs: has a role been chosen, and if not, the address a
/// room commons would have.
#[tauri::command]
fn setup_state(chosen: tauri::State<'_, ChosenRole>) -> SetupState {
    SetupState { role: chosen.0.lock().unwrap().clone(), lan_url: lan_url() }
}

/// Save the person's choice and start the server in that role. The choice is
/// fixed for this app's data folder; the server enforces the same rule.
#[tauri::command]
fn choose_role(
    app: tauri::AppHandle,
    role: String,
    cfg: tauri::State<'_, AppConfig>,
    chosen: tauri::State<'_, ChosenRole>,
    proc: tauri::State<'_, NodeProc>,
) -> Result<(), String> {
    if !valid_role(&role) {
        return Err("Choose a node or a commons.".into());
    }
    let mut current = chosen.0.lock().unwrap();
    if let Some(existing) = current.as_ref() {
        if *existing == role {
            return Ok(());
        }
        return Err(format!("This computer is already set up as a {existing}."));
    }
    save_role(&cfg.base, &role)?;
    start(&app, &cfg, &proc, &role)?;
    *current = Some(role);
    Ok(())
}

/// Manually (re)start the server. Called by the "Start" button on the loading
/// screen when auto-start stalls — e.g. the sidecar crashed, or port 8080 was
/// momentarily busy so the first bind failed. Kills any stuck child, then
/// respawns. Returns a human-readable error the loading screen can show.
#[tauri::command]
fn restart_node(
    app: tauri::AppHandle,
    cfg: tauri::State<'_, AppConfig>,
    chosen: tauri::State<'_, ChosenRole>,
    proc: tauri::State<'_, NodeProc>,
) -> Result<(), String> {
    let Some(role) = chosen.0.lock().unwrap().clone() else {
        return Err("Choose a node or a commons first.".into());
    };
    // If a healthy server is already listening, nothing to do — just succeed so
    // the screen navigates to it (covers the "something else is already on 8080"
    // case without us killing it).
    if node_up() {
        return Ok(());
    }
    if let Some(child) = proc.0.lock().unwrap().take() {
        let _ = child.kill();
    }
    let child = spawn_server(&app, &cfg, &role)?;
    *proc.0.lock().unwrap() = Some(child);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![node_up, setup_state, choose_role, restart_node])
        .on_page_load(|window, payload| {
            let url = payload.url().to_string();
            let is_http = url.starts_with("http://") || url.starts_with("https://");
            // skip the app's own loading screen (tauri.localhost); only decorate
            // real node / commons pages.
            if !is_http || url.contains("tauri.localhost") {
                return;
            }
            let _ = window.eval(APP_BAR_JS);
        })
        .setup(|app| {
            let base = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&base).ok();

            let who = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let instance_override = std::env::var("HIC_INSTANCE").ok().filter(|s| !s.trim().is_empty());

            // Default commons to pre-fill in "Share to a commons". Prefer a host
            // runtime override; else a build-time default baked in via the
            // HIC_SHARE_DEFAULT env at compile time (set from a CI variable, NOT
            // committed to source — stays blank in a plain `cargo build`).
            let share_default = std::env::var("HIC_SHARE_DEFAULT")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| option_env!("HIC_SHARE_DEFAULT").map(str::to_string))
                .unwrap_or_default();

            let cfg = AppConfig { base: base.clone(), who, instance_override, share_default };
            let role = load_role(&base);
            let proc = NodeProc(Mutex::new(None));

            // A returning user starts straight away. A new one sees the chooser
            // (ui/index.html), which calls `choose_role`.
            if let Some(r) = &role {
                start(&app.handle().clone(), &cfg, &proc, r)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            }
            app.manage(cfg);
            app.manage(ChosenRole(Mutex::new(role)));
            app.manage(proc);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            // Reap the sidecar when the app exits. tauri-plugin-shell does not kill
            // spawned sidecars on exit, so without this a quit or force-quit leaves
            // an orphan `hic-commons` holding port 8080 — which then wedges the next
            // launch. Take the child out of state and kill it on exit.
            if let tauri::RunEvent::Exit = event {
                if let Some(proc) = handle.try_state::<NodeProc>() {
                    if let Some(child) = proc.0.lock().unwrap().take() {
                        let _ = child.kill();
                    }
                }
            }
        });
}
