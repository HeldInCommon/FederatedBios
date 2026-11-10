//! Held in Common — the desktop app.
//!
//! A thin native wrapper around the bundled `hic-commons` server. On first
//! launch the person chooses what this computer runs: a **node** (their own
//! profile and work) or a **commons** (a shared directory other nodes share
//! to). After that the app starts the server in that role and shows its pages.
//! No Docker, no terminal.
//!
//! | Module | Job |
//! |---|---|
//! | [`settings`] | the saved role and the data folders (`app.json`) |
//! | [`network`] | is *our* server answering; this computer's network address |
//! | [`sidecar`] | starting and stopping the server process |
//! | [`commands`] | what the app's screens (`ui/app.js`) can ask for |
//!
//! This file only wires them together.

pub mod commands;
pub mod network;
pub mod settings;
pub mod sidecar;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::Manager;

use commands::ChosenRole;
use sidecar::{AppConfig, Sidecar};

/// Injected into every server page (the node, or a commons the person
/// visits). On anything but this computer's own server it adds a fixed
/// "Back to your node" button, so a visited commons is never a dead end. The
/// app can add this from the host side, so it works on any commons.
const BACK_BUTTON_JS: &str = r#"(function(){
  var NODE='http://127.0.0.1:8080';
  var here=location.href;
  var isNode=here.indexOf('127.0.0.1:8080')>-1||here.indexOf('localhost:8080')>-1;
  var id='hic-back-node';
  var old=document.getElementById(id); if(old){old.remove();}
  if(isNode){return;}
  var b=document.createElement('button'); b.id=id;
  b.textContent='← Back to your node';
  b.style.cssText='position:fixed;top:9px;right:12px;z-index:2147483647;cursor:pointer;border:0;padding:8px 12px;font:700 11px/1 ui-monospace,Menlo,monospace;letter-spacing:.07em;text-transform:uppercase;color:#fff;background:#0a7a54;box-shadow:0 2px 10px rgba(0,0,0,.3);';
  b.onclick=function(){location.href=NODE;};
  document.body.appendChild(b);
})();"#;

/// True for pages served over http(s) by a server, as opposed to the app's
/// own screens (served from `tauri.localhost`).
fn is_server_page(url: &str) -> bool {
    (url.starts_with("http://") || url.starts_with("https://")) && !url.contains("tauri.localhost")
}

/// The log file: `~/Library/Logs/org.heldincommon.node/held-in-common.log` on
/// macOS (the platform's log folder elsewhere), plus the terminal. Holds the
/// app's messages and everything the server prints, so someone who hits a
/// problem can send it. Capped at 5 MB; the previous file is kept as a backup.
fn log_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    use tauri_plugin_log::{RotationStrategy, Target, TargetKind};
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::LogDir { file_name: Some("held-in-common".into()) }),
            Target::new(TargetKind::Stdout),
        ])
        .max_file_size(5_000_000)
        .rotation_strategy(RotationStrategy::KeepOne)
        .level(log::LevelFilter::Info)
        .build()
}

/// Everything fixed for this run of the app.
fn app_config(base: PathBuf) -> AppConfig {
    let who = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default().trim().to_string();
    let instance_override = std::env::var("HIC_INSTANCE").ok().filter(|s| !s.trim().is_empty());
    // The default commons to pre-fill: a runtime override, else one baked in
    // at build time from the HIC_SHARE_DEFAULT CI variable (never committed).
    let share_default = std::env::var("HIC_SHARE_DEFAULT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| option_env!("HIC_SHARE_DEFAULT").map(str::to_string))
        .unwrap_or_default();
    AppConfig { base, who, instance_override, share_default, launch_token: network::launch_token() }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(log_plugin())
        .invoke_handler(tauri::generate_handler![
            commands::node_up,
            commands::setup_state,
            commands::choose_role,
            commands::restart_node
        ])
        .on_page_load(|window, payload| {
            if is_server_page(payload.url().as_str()) {
                let _ = window.eval(BACK_BUTTON_JS);
            }
        })
        .setup(|app| {
            let base = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&base).ok();
            let cfg = app_config(base.clone());
            let role = settings::load_role(&base);
            let proc_ = Sidecar::default();
            // A returning user starts straight away; a new one sees the chooser.
            if let Some(r) = &role {
                sidecar::start(app.handle(), &cfg, &proc_, r).map_err(std::io::Error::other)?;
            }
            app.manage(cfg);
            app.manage(ChosenRole(Mutex::new(role)));
            app.manage(proc_);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            // The shell plugin doesn't stop sidecars on exit. Without this, a
            // quit leaves the server holding port 8080 and the next launch stalls.
            if let tauri::RunEvent::Exit = event {
                if let Some(proc_) = handle.try_state::<Sidecar>() {
                    proc_.stop();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_server_pages_get_the_back_button() {
        assert!(is_server_page("http://127.0.0.1:8080/@tega"));
        assert!(is_server_page("https://commons.example/"));
        assert!(!is_server_page("http://tauri.localhost/index.html"));
        assert!(!is_server_page("tauri://localhost/"));
    }
}
