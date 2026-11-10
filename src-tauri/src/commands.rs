//! The commands the app's own screens call (`ui/app.js`, via `invoke`).

use std::sync::Mutex;

use serde::Serialize;

use crate::network::{self, Probe};
use crate::settings::{self, clean_name, valid_role, Saved};
use crate::sidecar::{self, AppConfig, Sidecar};

/// What this computer runs and what it's called, as far as known.
pub struct Chosen(pub Mutex<Saved>);

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
    /// The name, or null before the person has named this computer's node.
    name: Option<String>,
    /// Where nodes in the same room would reach a commons on this computer.
    lan_url: Option<String>,
}

/// Has a role been chosen and a name given, and what would a room commons'
/// address be?
#[tauri::command]
pub fn setup_state(chosen: tauri::State<'_, Chosen>) -> SetupState {
    let s = chosen.0.lock().unwrap().clone();
    SetupState { role: s.role, name: s.name, lan_url: network::lan_url() }
}

/// First launch: save the role and name, and start the server. The role is
/// fixed for this app's data folder (the server enforces the same rule).
#[tauri::command]
pub fn choose_role(
    app: tauri::AppHandle,
    role: String,
    name: String,
    cfg: tauri::State<'_, AppConfig>,
    chosen: tauri::State<'_, Chosen>,
    proc_: tauri::State<'_, Sidecar>,
) -> Result<(), String> {
    if !valid_role(&role) {
        return Err("Choose a node or a commons.".into());
    }
    let name = clean_name(&name)?;
    let mut current = chosen.0.lock().unwrap();
    if let Some(existing) = current.role.as_deref().filter(|r| *r != role) {
        return Err(format!("This computer is already set up as a {existing}."));
    }
    if network::probe(&cfg.launch_token) == Probe::Other {
        return Err(PORT_TAKEN.into());
    }
    let next = Saved { role: Some(role.clone()), name: Some(name.clone()) };
    settings::save(&cfg.base, &next)?;
    sidecar::start(&app, &cfg, &proc_, &role, &name)?;
    *current = next;
    Ok(())
}

/// Name (or rename) this computer's node or commons. If the server is
/// running, it restarts under the new name.
#[tauri::command]
pub fn set_name(
    app: tauri::AppHandle,
    name: String,
    cfg: tauri::State<'_, AppConfig>,
    chosen: tauri::State<'_, Chosen>,
    proc_: tauri::State<'_, Sidecar>,
) -> Result<(), String> {
    let name = clean_name(&name)?;
    let mut current = chosen.0.lock().unwrap();
    let Some(role) = current.role.clone() else {
        return Err("Choose a node or a commons first.".into());
    };
    let next = Saved { role: Some(role.clone()), name: Some(name.clone()) };
    settings::save(&cfg.base, &next)?;
    proc_.stop();
    // Give the old server up to three seconds to let go of the port.
    for _ in 0..30 {
        if network::probe(&cfg.launch_token) == Probe::Nothing {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    sidecar::start(&app, &cfg, &proc_, &role, &name)?;
    *current = next;
    Ok(())
}

/// Start the server again, for the "Start" button the loading screen shows
/// when start-up stalls (the server crashed, or the port was busy).
#[tauri::command]
pub fn restart_node(
    app: tauri::AppHandle,
    cfg: tauri::State<'_, AppConfig>,
    chosen: tauri::State<'_, Chosen>,
    proc_: tauri::State<'_, Sidecar>,
) -> Result<(), String> {
    let s = chosen.0.lock().unwrap().clone();
    let (Some(role), Some(name)) = (s.role, s.name) else {
        return Err("Choose a node or a commons, and name it, first.".into());
    };
    match network::probe(&cfg.launch_token) {
        Probe::Ours => return Ok(()), // already running: the screen will open it
        Probe::Other => return Err(PORT_TAKEN.into()),
        Probe::Nothing => {}
    }
    proc_.stop();
    let child = sidecar::spawn(&app, &cfg, &role, &name)?;
    *proc_.0.lock().unwrap() = Some(child);
    Ok(())
}
