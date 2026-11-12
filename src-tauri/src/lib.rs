//! Held in Common — the desktop app.
//!
//! A thin native wrapper around the bundled `hic-commons` server. On first
//! launch the person chooses what this computer runs: a **node** (their own
//! profile and work) or a **commons** (a shared directory other nodes share
//! to). Later they can add the other one, and a **practice commons** to
//! learn on; all of them run side by side, and the **Go** menu and a row of
//! buttons on every page move between them. No Docker, no terminal.
//!
//! | Module | Job |
//! |---|---|
//! | [`settings`] | what this computer runs, the names, ports and data folders (`app.json`) |
//! | [`network`] | is *our* server answering; this computer's network address |
//! | [`sidecar`] | starting and stopping the server processes |
//! | [`links`] | `hic-app://` links and the switcher between servers (pure) |
//! | [`commands`] | what the app's screens (`ui/app.js`), the menu and links can ask for |
//!
//! This file only wires them together.

pub mod commands;
pub mod links;
pub mod network;
pub mod settings;
pub mod sidecar;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::Manager;

use commands::{Chosen, Shown};
use settings::{Kind, Saved};
use sidecar::{AppConfig, DefaultCommons, Servers};

/// The Go menu: show each server this computer runs, or set it up; remove
/// the practice commons; rename the node, commons or practice commons; and,
/// always, open the
/// project's site. Rebuilt whenever what the computer runs changes
/// ([`refresh_menu`]). Before first launch's choice it holds only the site.
fn build_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>, saved: &Saved) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
    let menu = Menu::default(app)?;
    let go = Submenu::new(app, "Go", true)?;
    let about =
        MenuItem::with_id(app, "about-project", "About the Held in Common project \u{2197}", true, None::<&str>)?;
    if !saved.ready() {
        go.append(&about)?;
        menu.append(&go)?;
        return Ok(menu);
    }
    for k in Kind::ALL {
        let text = match (k, saved.runs(k)) {
            (_, true) => k.label().to_string(),
            (Kind::Node, false) => "Add your own node\u{2026}".to_string(),
            (Kind::Commons, false) => "Set up a commons\u{2026}".to_string(),
            (Kind::Practice, false) => "Open a practice commons".to_string(),
        };
        go.append(&MenuItem::with_id(app, format!("open:{}", k.key()), text, true, None::<&str>)?)?;
    }
    go.append(&PredefinedMenuItem::separator(app)?)?;
    for k in Kind::ALL.into_iter().filter(|k| saved.runs(*k)) {
        let text = format!("Rename {}\u{2026}", k.label().to_lowercase());
        go.append(&MenuItem::with_id(app, format!("rename:{}", k.key()), text, true, None::<&str>)?)?;
    }
    if saved.practice {
        go.append(&MenuItem::with_id(app, "remove-practice", "Remove practice commons\u{2026}", true, None::<&str>)?)?;
    }
    go.append(&PredefinedMenuItem::separator(app)?)?;
    go.append(&about)?;
    menu.append(&go)?;
    Ok(menu)
}

/// Rebuild the Go menu for what this computer now runs. Queued on the main
/// thread rather than done here: callers may hold the [`Chosen`] lock on
/// another thread, and waiting for the main thread while it waits for that
/// lock (to show a page) would freeze the app.
pub fn refresh_menu(app: &tauri::AppHandle, saved: &Saved) {
    let (app, saved) = (app.clone(), saved.clone());
    let queued = app.clone().run_on_main_thread(move || match build_menu(&app, &saved) {
        Ok(menu) => {
            if let Err(e) = app.set_menu(menu) {
                log::warn!("couldn't update the menu: {e}");
            }
        }
        Err(e) => log::warn!("couldn't build the menu: {e}"),
    });
    if let Err(e) = queued {
        log::warn!("couldn't update the menu: {e}");
    }
}

/// A Go menu choice.
fn on_menu(app: &tauri::AppHandle, id: &str) {
    let result = if let Some(k) = id.strip_prefix("open:").and_then(Kind::parse) {
        commands::open(app, k)
    } else if let Some(k) = id.strip_prefix("rename:").and_then(Kind::parse) {
        commands::open_setup(app, &format!("do=rename&role={}", k.key()), "Rename");
        Ok(())
    } else if id == "remove-practice" {
        commands::open_setup(app, "do=remove-practice", "Remove practice commons");
        Ok(())
    } else if id == "about-project" {
        open_in_browser(app, links::PROJECT_URL);
        Ok(())
    } else {
        Ok(())
    };
    if let Err(e) = result {
        log::warn!("{id}: {e}");
    }
}

/// Open `url` in the person's own web browser.
fn open_in_browser(app: &tauri::AppHandle, url: &str) {
    use tauri_plugin_opener::OpenerExt;
    if let Err(e) = app.opener().open_url(url, None::<&str>) {
        log::warn!("couldn't open {url} in the browser: {e}");
    }
}

/// True when `url` is a page of one of this computer's servers, as this
/// app runs them.
fn is_ours(app: &tauri::AppHandle, url: &str) -> bool {
    let Some(port) = network::local_port(url) else { return false };
    app.try_state::<Chosen>().is_some_and(|c| links::kind_on(&c.0.lock().unwrap(), port).is_some())
}

/// A `hic-app://` link was followed in the main window. Act on it only if
/// the page it was followed from is one of ours: a commons elsewhere can't
/// ask the app to do anything. Runs on its own thread, since it may start a
/// server or open a window.
fn follow_app_link(app: &tauri::AppHandle, url: &str) {
    let Some(links::AppLink::Open(kind)) = links::parse(url) else { return };
    let from = app.try_state::<Shown>().map(|s| s.0.lock().unwrap().clone()).unwrap_or_default();
    if !is_ours(app, &from) {
        log::warn!("ignored {url}: followed from {from}, which isn't one of this computer's servers");
        return;
    }
    let (app, url) = (app.clone(), url.to_string());
    std::thread::spawn(move || {
        if let Err(e) = commands::open(&app, kind) {
            log::warn!("{url}: {e}");
        }
    });
}

/// The main window. Created here rather than in `tauri.conf.json` so it can
/// catch links that shouldn't load in it: `mailto:` ("Email this invite")
/// goes to the person's own mail app, the project's site
/// ([`links::opens_in_browser`]) to their browser, and `hic-app://` to
/// [`follow_app_link`].
fn main_window(app: &tauri::AppHandle) -> Result<(), String> {
    let handle = app.clone();
    // Size the window to the screen it opens on (see `sidecar::window_size`);
    // if the screen can't be read, a size that fits most laptops.
    let (width, height) = match app.primary_monitor() {
        Ok(Some(m)) => {
            let screen = m.size().to_logical::<f64>(m.scale_factor());
            sidecar::window_size((screen.width, screen.height))
        }
        _ => (1100.0, 760.0),
    };
    tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
        .title("Held in Common")
        .inner_size(width, height)
        .min_inner_size(760.0, 560.0)
        .center()
        .on_navigation(move |url| match url.scheme() {
            "mailto" => {
                use tauri_plugin_opener::OpenerExt;
                if let Err(e) = handle.opener().open_url(url.as_str(), None::<&str>) {
                    log::warn!("couldn't open the mail app: {e}");
                }
                false
            }
            links::SCHEME => {
                follow_app_link(&handle, url.as_str());
                false
            }
            "http" | "https" if links::opens_in_browser(url.host_str()) => {
                open_in_browser(&handle, url.as_str());
                false
            }
            _ => true,
        })
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// True for pages served over http(s) by a server, as opposed to the app's
/// own screens (served from `tauri.localhost`).
fn is_server_page(url: &str) -> bool {
    (url.starts_with("http://") || url.starts_with("https://")) && !url.contains("tauri.localhost")
}

/// A page finished loading in a window. In the main window: remember where
/// it is, set the title to say which server it is, and add the switcher.
fn on_page(window: &tauri::Webview, url: &str) {
    if window.label() != "main" {
        return;
    }
    let app = window.app_handle();
    if let Some(shown) = app.try_state::<Shown>() {
        *shown.0.lock().unwrap() = url.to_string();
    }
    let Some(chosen) = app.try_state::<Chosen>().filter(|_| is_server_page(url)) else { return };
    let saved = chosen.0.lock().unwrap().clone();
    let here = network::local_port(url).and_then(|p| links::kind_on(&saved, p));
    if let (Some(k), Some(w)) = (here, app.get_webview_window("main")) {
        let _ = w.set_title(&sidecar::title_for(k));
    }
    let _ = window.eval(links::switcher_js(&links::places(&saved, here)));
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

/// Everything fixed for this run of the app. Fails only if the operating
/// system can't give us random bytes for the launch token.
fn app_config(base: PathBuf) -> Result<AppConfig, rand::Error> {
    let instance_override = std::env::var("HIC_INSTANCE").ok().filter(|s| !s.trim().is_empty());
    // The default commons to pre-fill, and its name: a runtime override, else
    // one baked in at build time from the HIC_SHARE_DEFAULT and
    // HIC_SHARE_DEFAULT_NAME CI variables (never committed).
    let share_default = DefaultCommons {
        url: setting("HIC_SHARE_DEFAULT", option_env!("HIC_SHARE_DEFAULT")),
        name: setting("HIC_SHARE_DEFAULT_NAME", option_env!("HIC_SHARE_DEFAULT_NAME")),
    };
    Ok(AppConfig { base, instance_override, share_default, launch_token: network::launch_token()? })
}

/// A setting from the environment `var` when set, else the value `baked` in
/// at build time, else blank. Only reads.
fn setting(var: &str, baked: Option<&str>) -> String {
    std::env::var(var).ok().filter(|s| !s.trim().is_empty()).or_else(|| baked.map(str::to_string)).unwrap_or_default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Start the app: plugins, commands, the menu, the main window, and every server this computer runs.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(log_plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::node_up,
            commands::port_problem,
            commands::server_url,
            commands::setup_state,
            commands::choose_role,
            commands::add_role,
            commands::set_name,
            commands::close_setup,
            commands::restart_node,
            commands::remove_practice_now
        ])
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                on_page(webview, payload.url().as_str());
            }
        })
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .setup(|app| {
            let base = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&base).ok();
            // Without a launch token nothing can prove it is this app, so a
            // broken random source stops start-up with that error.
            let cfg = app_config(base.clone()).map_err(std::io::Error::other)?;
            let (saved, changed) = settings::load(&base);
            if changed {
                // An install from before the chooser (or an untidy name): record the result.
                if let Err(e) = settings::save(&base, &saved) {
                    log::warn!("couldn't update app.json: {e}");
                }
            }
            // Before any server starts: the log will hold a commons' steward
            // link and founding invite, so only this user may read it (ADR 0023).
            match app.path().app_log_dir() {
                Ok(dir) => {
                    if let Err(e) = settings::make_private(&dir) {
                        log::warn!("couldn't make the log folder private: {e}");
                    }
                }
                Err(e) => log::warn!("couldn't find the log folder: {e}"),
            }
            app.manage(Shown::default());
            main_window(app.handle()).map_err(std::io::Error::other)?;
            let servers = Servers::default();
            // A returning user starts straight away: every server this
            // computer runs, with the first-launch role in the window (the
            // loading screen opens it). A new one sees the chooser; one from
            // before names existed is asked for a name first.
            for kind in saved.running() {
                if let Err(e) = sidecar::start(app.handle(), &cfg, &servers, &saved, kind, false) {
                    if Some(kind) == saved.first() {
                        return Err(std::io::Error::other(e).into());
                    }
                    log::error!("couldn't start {}: {e}", kind.key());
                }
            }
            refresh_menu(app.handle(), &saved);
            app.manage(cfg);
            app.manage(Chosen(Mutex::new(saved)));
            app.manage(servers);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            // The shell plugin doesn't stop sidecars on exit. Without this, a
            // quit leaves the servers holding their ports and the next launch stalls.
            if let tauri::RunEvent::Exit = event {
                if let Some(servers) = handle.try_state::<Servers>() {
                    servers.stop_all();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_server_pages_get_the_switcher() {
        assert!(is_server_page("http://127.0.0.1:8080/@tega"));
        assert!(is_server_page("https://commons.example/"));
        assert!(!is_server_page("http://tauri.localhost/index.html"));
        assert!(!is_server_page("tauri://localhost/"));
    }
}
