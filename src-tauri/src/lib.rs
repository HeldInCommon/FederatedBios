//! Held in Common — Node (desktop).
//!
//! A thin native wrapper: on launch it starts the bundled node binary
//! (`hic-commons`) as a sidecar, then the window shows the node's own web UI.
//! No Docker, no terminal — double-click and you are a node.

use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

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

/// Everything needed to (re)spawn the node sidecar. Computed once at startup and
/// kept in managed state so the `restart_node` command can respawn with the same
/// environment after a crash or a failed port bind.
struct NodeConfig {
    data_dir: String,
    instance: String,
    share_default: String,
}

/// Handle to the running sidecar. Kept alive here so the process lives as long as
/// the app; `restart_node` takes it to kill a stuck child before respawning.
struct NodeProc(Mutex<Option<CommandChild>>);

/// Spawn the bundled `hic-commons` binary as our node on port 8080.
fn spawn_node(app: &tauri::AppHandle, cfg: &NodeConfig) -> Result<CommandChild, String> {
    let sidecar = app
        .shell()
        .sidecar("hic-commons")
        .map_err(|e| format!("could not locate the node binary: {e}"))?
        .env("HIC_DATA", &cfg.data_dir)
        .env("PORT", "8080")
        .env("HIC_INSTANCE", &cfg.instance)
        .env("HIC_ROLE", "node")
        .env("HIC_SHARE_DEFAULT", &cfg.share_default);

    let (mut rx, child) = sidecar
        .spawn()
        .map_err(|e| format!("could not start the node: {e}"))?;
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                CommandEvent::Stdout(l) | CommandEvent::Stderr(l) => {
                    eprintln!("[node] {}", String::from_utf8_lossy(&l));
                }
                _ => {}
            }
        }
    });
    Ok(child)
}

/// Is the node's http server accepting connections on 8080? The loading screen
/// polls this via `invoke` instead of `fetch` — a fetch from the app's secure
/// origin to http://localhost is blocked as mixed content, so an in-page probe
/// never fires. A TCP connect from the host side has no such restriction. As a
/// bonus, if the user launched the app twice, the second window sees the first
/// node already listening and simply navigates to it.
#[tauri::command]
fn node_up() -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(400)).is_ok()
}

/// Manually (re)start the node. Called by the "Start the node" button on the
/// loading screen when auto-start stalls — e.g. the sidecar crashed, or port 8080
/// was momentarily busy so the first bind failed. Kills any stuck child, then
/// respawns. Returns a human-readable error the loading screen can show.
#[tauri::command]
fn restart_node(
    app: tauri::AppHandle,
    cfg: tauri::State<'_, NodeConfig>,
    proc: tauri::State<'_, NodeProc>,
) -> Result<(), String> {
    // If a healthy node is already listening, nothing to do — just succeed so the
    // screen navigates to it (covers the "something else is already on 8080" case
    // without us killing it).
    if node_up() {
        return Ok(());
    }
    if let Some(child) = proc.0.lock().unwrap().take() {
        let _ = child.kill();
    }
    let child = spawn_node(&app, &cfg)?;
    *proc.0.lock().unwrap() = Some(child);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![node_up, restart_node])
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
            // Per-user writable store for this node's data.
            let base = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let node_data = base.join("node-data");
            std::fs::create_dir_all(&node_data).ok();

            // Label this node so it is identifiable when federated into a commons
            // ("federated from <label>"). Honour a host HIC_INSTANCE override; else
            // derive one from the account name ("phoenixperry's node") so every
            // install is self-identifying with zero config.
            let instance = std::env::var("HIC_INSTANCE")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    let who = std::env::var("USER")
                        .or_else(|_| std::env::var("USERNAME"))
                        .unwrap_or_default();
                    let who = who.trim();
                    if who.is_empty() {
                        "a Held in Common node".to_string()
                    } else {
                        format!("{who}'s node")
                    }
                });

            // Default commons to pre-fill in "Share to a commons". Prefer a host
            // runtime override; else a build-time default baked in via the
            // HIC_SHARE_DEFAULT env at compile time (set from a CI variable, NOT
            // committed to source — stays blank in a plain `cargo build`).
            let share_default = std::env::var("HIC_SHARE_DEFAULT")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| option_env!("HIC_SHARE_DEFAULT").map(str::to_string))
                .unwrap_or_default();

            let cfg = NodeConfig {
                data_dir: node_data.to_string_lossy().to_string(),
                instance,
                share_default,
            };

            // Spawn the node, then keep both the config (for respawns) and the
            // child handle (to keep the process alive) in managed state.
            let child = spawn_node(&app.handle().clone(), &cfg)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            app.manage(cfg);
            app.manage(NodeProc(Mutex::new(Some(child))));

            // Auto-open the node once its http server is listening. The loading
            // screen also polls (via the node_up command), but this host-side
            // poll is an independent backstop that works even if the webview JS
            // stalls: navigate the window as soon as the port answers.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
                for _ in 0..600 {
                    if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(500)).is_ok() {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        if let Some((_, win)) = handle.webview_windows().iter().next() {
                            let _ = win.eval("location.replace('http://localhost:8080')");
                        }
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(400));
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            // Reap the node sidecar when the app exits. tauri-plugin-shell does not
            // kill spawned sidecars on exit, so without this a quit or force-quit
            // leaves an orphan `hic-commons` holding port 8080 — which then wedges
            // the next launch. Take the child out of state and kill it on exit.
            if let tauri::RunEvent::Exit = event {
                if let Some(proc) = handle.try_state::<NodeProc>() {
                    if let Some(child) = proc.0.lock().unwrap().take() {
                        let _ = child.kill();
                    }
                }
            }
        });
}
