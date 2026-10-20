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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
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
