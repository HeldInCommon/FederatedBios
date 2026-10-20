//! Held in Common — Node (desktop).
//!
//! A thin native wrapper: on launch it starts the bundled node binary
//! (`hic-commons`) as a sidecar, then the window shows the node's own web UI.
//! No Docker, no terminal — double-click and you are a node.

use tauri::Manager;
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

/// The shared commons prefilled in the node's "Share to a commons" form.
const SHARE_DEFAULT: &str = "https://drh1in8ipba68q-8080.proxy.runpod.net";

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
                .env("HIC_SHARE_DEFAULT", SHARE_DEFAULT);

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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
