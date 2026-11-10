//! What the app remembers between launches, and the folders it uses.
//!
//! The app data folder (`~/Library/Application Support/org.heldincommon.node`
//! on macOS) holds:
//!
//! ```text
//! app.json        {"role": "node" | "commons", "name": "Studio laptop"}
//! node-data/      the server's data folder when running a node
//! commons-data/   the server's data folder when running a commons
//! ```
//!
//! The **name** is what this computer's node or commons is called: it shows in
//! the coloured bar on every page and travels with shares as the node's name,
//! so someone with several machines can tell them apart. Earlier versions used
//! the computer's account name instead, which shared it with every commons.
//!
//! Pure rules are separate from the two functions that touch the disk
//! ([`load`], [`save`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Longest name allowed, in characters.
pub const NAME_MAX: usize = 60;

/// The contents of `app.json`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Saved {
    /// "node" or "commons", once chosen.
    pub role: Option<String>,
    /// What this node or commons is called, once named.
    #[serde(default)]
    pub name: Option<String>,
}

impl Saved {
    /// True when both the role and the name are known, so the server can start.
    pub fn ready(&self) -> bool {
        self.role.is_some() && self.name.is_some()
    }
}

/// True for the two roles the app can run.
pub fn valid_role(r: &str) -> bool {
    r == "node" || r == "commons"
}

/// A name as typed, tidied (spaces trimmed and collapsed), or why it can't be used.
pub fn clean_name(typed: &str) -> Result<String, String> {
    let name = typed.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err("Give this computer's node a name, like \"Studio laptop\".".into());
    }
    if name.chars().count() > NAME_MAX {
        return Err(format!("Keep the name to {NAME_MAX} characters or fewer."));
    }
    if name.chars().any(char::is_control) {
        return Err("The name can't contain control characters.".into());
    }
    Ok(name)
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

/// The name the server uses for itself: a host override (`HIC_INSTANCE`)
/// wins, then the chosen name, then a neutral fallback. Never the account name.
pub fn instance_label(override_: Option<&str>, name: Option<&str>, role: &str) -> String {
    override_.or(name).map(str::to_string).unwrap_or_else(|| format!("A Held in Common {role}"))
}

/// Read `app.json` (migrating installs from before the chooser, which have
/// node members but no file).
pub fn load(base: &Path) -> Saved {
    let file: Saved = std::fs::read_to_string(base.join("app.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let has_members = std::fs::read_dir(base.join("node-data").join("handles"))
        .map(|mut d| d.any(|e| e.map(|e| e.path().extension().is_some_and(|x| x == "json")).unwrap_or(false)))
        .unwrap_or(false);
    let role = decide_role(file.role.clone(), has_members);
    let name = file.name.as_deref().and_then(|n| clean_name(n).ok());
    let saved = Saved { role, name };
    if saved != file {
        // Save the migrated or tidied settings so they're never asked again.
        let _ = save(base, &saved);
    }
    saved
}

/// Write `app.json`.
pub fn save(base: &Path, s: &Saved) -> Result<(), String> {
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
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
    fn names_are_tidied_and_checked() {
        assert_eq!(clean_name("  Studio   laptop ").unwrap(), "Studio laptop");
        assert!(clean_name("   ").is_err());
        assert!(clean_name(&"x".repeat(NAME_MAX + 1)).is_err());
        assert!(clean_name("bad\u{7}name").is_err());
        assert_eq!(clean_name("Atelier é 🎛").unwrap(), "Atelier é 🎛");
    }

    #[test]
    fn labels_never_use_the_account_name() {
        assert_eq!(instance_label(Some("Room 4"), Some("Studio"), "node"), "Room 4");
        assert_eq!(instance_label(None, Some("Studio laptop"), "node"), "Studio laptop");
        assert_eq!(instance_label(None, None, "commons"), "A Held in Common commons");
    }

    #[test]
    fn old_app_json_without_a_name_still_reads() {
        let s: Saved = serde_json::from_str(r#"{"role":"node"}"#).unwrap();
        assert_eq!(s, Saved { role: Some("node".into()), name: None });
        assert!(!s.ready());
    }
}
