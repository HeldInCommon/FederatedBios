//! Held in Common — Node (desktop).
//!
//! A thin native wrapper: on launch it starts the bundled node binary
//! (`hic-commons`) as a sidecar, then the window shows the node's own web UI.
//! No Docker, no terminal — double-click and you are a node.

use tauri::Manager;
use tauri_plugin_shell::process::CommandEvent;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
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

            let sidecar = app
                .shell()
                .sidecar("hic-commons")?
                .env("HIC_DATA", node_data.to_string_lossy().to_string())
                .env("PORT", "8080")
                .env("HIC_INSTANCE", "My node")
                .env("HIC_ROLE", "node")
                .env("HIC_SHARE_DEFAULT", std::env::var("HIC_SHARE_DEFAULT").unwrap_or_default());

            let (mut rx, child) = sidecar.spawn()?;
            tauri::async_runtime::spawn(async move {
                // Keep the node process tied to the app's lifetime.
                let _child = child;
                while let Some(ev) = rx.recv().await {
                    match ev {
                        CommandEvent::Stdout(l) | CommandEvent::Stderr(l) => {
                            eprintln!("[node] {}", String::from_utf8_lossy(&l));
                        }
                        _ => {}
                    }
                }
            });

            // Auto-open the node once its http server is listening. The loading
            // screen can't detect readiness itself: a fetch() from the app's
            // secure origin to http://localhost is blocked as mixed content, so
            // the in-page probe never fires. Poll the port from here instead and
            // navigate the window when it's up (a navigation isn't mixed-content
            // blocked, which is why clicking the link already worked).
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
