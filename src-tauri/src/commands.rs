//! The commands the app's own screens call (`ui/app.js`, via `invoke`).

use std::sync::Mutex;

use serde::Serialize;

use crate::network::{self, Probe};
use crate::settings::{save_role, valid_role};
use crate::sidecar::{self, AppConfig, Sidecar};

/// The role chosen for this computer, once known.
pub struct ChosenRole(pub Mutex<Option<String>>);

/// Shown when another program holds the port.
const PORT_TAKEN: &str = "Another program is using port 8080, so Held in Common can't start. \
     Quit that program (or another copy of this app), then press Start.";

/// Is *our* server answering? (Not just "is the port open": see
/// [`crate::network`].) The loading screen polls this; a `fetch` from the
/// app's own page to http://127.0.0.1 would be blocked as mixed content.
#[tauri::command]
pub fn node_up(cfg: tauri::State<'_, AppConfig>) -> bool {
    network::probe(&cfg.launch_token) == Probe::Ours
}

/// What the first screen needs.
#[derive(Serialize)]
pub struct SetupState {
    /// "node", "commons", or null before the person has chosen.
    role: Option<String>,
    /// Where nodes in the same room would reach a commons on this computer.
    lan_url: Option<String>,
}

/// Has a role been chosen, and what would a room commons' address be?
#[tauri::command]
pub fn setup_state(chosen: tauri::State<'_, ChosenRole>) -> SetupState {
    SetupState { role: chosen.0.lock().unwrap().clone(), lan_url: network::lan_url() }
}

/// Save the person's choice and start the server in that role. The choice is
/// fixed for this app's data folder (the server enforces the same rule).
#[tauri::command]
pub fn choose_role(
    app: tauri::AppHandle,
    role: String,
    cfg: tauri::State<'_, AppConfig>,
    chosen: tauri::State<'_, ChosenRole>,
    proc_: tauri::State<'_, Sidecar>,
) -> Result<(), String> {
    if !valid_role(&role) {
        return Err("Choose a node or a commons.".into());
    }
    let mut current = chosen.0.lock().unwrap();
    match current.as_deref() {
        Some(existing) if existing == role => return Ok(()),
        Some(existing) => return Err(format!("This computer is already set up as a {existing}.")),
        None => {}
    }
    if network::probe(&cfg.launch_token) == Probe::Other {
        return Err(PORT_TAKEN.into());
    }
    save_role(&cfg.base, &role)?;
    sidecar::start(&app, &cfg, &proc_, &role)?;
    *current = Some(role);
    Ok(())
}

/// Start the server again, for the "Start" button the loading screen shows
/// when start-up stalls (the server crashed, or the port was busy).
#[tauri::command]
pub fn restart_node(
    app: tauri::AppHandle,
    cfg: tauri::State<'_, AppConfig>,
    chosen: tauri::State<'_, ChosenRole>,
    proc_: tauri::State<'_, Sidecar>,
) -> Result<(), String> {
    let Some(role) = chosen.0.lock().unwrap().clone() else {
        return Err("Choose a node or a commons first.".into());
    };
    match network::probe(&cfg.launch_token) {
        Probe::Ours => return Ok(()), // already running: the screen will open it
        Probe::Other => return Err(PORT_TAKEN.into()),
        Probe::Nothing => {}
    }
    proc_.stop();
    let child = sidecar::spawn(&app, &cfg, &role)?;
    *proc_.0.lock().unwrap() = Some(child);
    Ok(())
}
