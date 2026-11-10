//! What the app remembers between launches, and the folders it uses.
//!
//! The app data folder (`~/Library/Application Support/org.heldincommon.node`
//! on macOS) holds:
//!
//! ```text
//! app.json        {"role": "node" | "commons"}: what this computer runs
//! node-data/      the server's data folder when running a node
//! commons-data/   the server's data folder when running a commons
//! ```
//!
//! Pure rules are separate from the two functions that touch the disk
//! ([`load_role`], [`save_role`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The contents of `app.json`.
#[derive(Serialize, Deserialize)]
struct AppFile {
    role: String,
}

/// True for the two roles the app can run.
pub fn valid_role(r: &str) -> bool {
    r == "node" || r == "commons"
}

/// The server's data folder for `role`. `node-data` is the folder earlier
/// versions used, so existing nodes keep their data.
pub fn data_dir_for(base: &Path, role: &str) -> PathBuf {
    base.join(if role == "commons" { "commons-data" } else { "node-data" })
}

/// The role to use, from what is saved and what is on disk:
/// a valid saved role wins; otherwise an install from before the chooser that
/// already has node members is a node; otherwise nothing has been chosen yet.
pub fn decide_role(saved: Option<String>, has_node_members: bool) -> Option<String> {
    saved.filter(|r| valid_role(r)).or_else(|| has_node_members.then(|| "node".to_string()))
}

/// A human name for this server, sent along with shares as the node's name.
pub fn instance_label(override_: Option<&str>, who: &str, role: &str) -> String {
    match (override_, who.is_empty()) {
        (Some(i), _) => i.to_string(),
        (None, true) => format!("a Held in Common {role}"),
        (None, false) => format!("{who}'s {role}"),
    }
}

/// Read the saved role (migrating installs from before the chooser).
pub fn load_role(base: &Path) -> Option<String> {
    let saved = std::fs::read_to_string(base.join("app.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<AppFile>(&s).ok())
        .map(|f| f.role);
    let had_saved = saved.as_deref().is_some_and(valid_role);
    let has_members = std::fs::read_dir(base.join("node-data").join("handles"))
        .map(|mut d| d.any(|e| e.map(|e| e.path().extension().is_some_and(|x| x == "json")).unwrap_or(false)))
        .unwrap_or(false);
    let role = decide_role(saved, has_members);
    if let (Some(r), false) = (&role, had_saved) {
        // Save the migrated choice so it's never asked again.
        let _ = save_role(base, r);
    }
    role
}

/// Save the chosen role to `app.json`.
pub fn save_role(base: &Path, role: &str) -> Result<(), String> {
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&AppFile { role: role.into() }).map_err(|e| e.to_string())?;
    std::fs::write(base.join("app.json"), json).map_err(|e| format!("could not save your choice: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_role_wins() {
        assert_eq!(decide_role(Some("commons".into()), true).as_deref(), Some("commons"));
    }

    #[test]
    fn old_install_with_members_is_a_node() {
        assert_eq!(decide_role(None, true).as_deref(), Some("node"));
        assert_eq!(decide_role(Some("garbage".into()), true).as_deref(), Some("node"));
    }

    #[test]
    fn fresh_install_has_no_role() {
        assert_eq!(decide_role(None, false), None);
    }

    #[test]
    fn folders_per_role() {
        let b = Path::new("/x");
        assert_eq!(data_dir_for(b, "node"), Path::new("/x/node-data"));
        assert_eq!(data_dir_for(b, "commons"), Path::new("/x/commons-data"));
    }

    #[test]
    fn labels() {
        assert_eq!(instance_label(Some("Room 4"), "pp", "node"), "Room 4");
        assert_eq!(instance_label(None, "", "commons"), "a Held in Common commons");
        assert_eq!(instance_label(None, "pp", "node"), "pp's node");
    }
}
