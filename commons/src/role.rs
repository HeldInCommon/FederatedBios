//! Whether a data folder belongs to a node or a commons.
//!
//! The role is saved in `$HIC_DATA/instance.json` the first time a folder is
//! used and is fixed after that: a node's folder holds its members' private
//! keys and a commons' folder holds other people's shared copies, so one must
//! never quietly become the other.
//!
//! [`inspect`] reads the folder, [`decide`] is the pure rule, and [`save`]
//! writes `instance.json`. `main.rs` calls them in that order, so each effect
//! is visible where it happens.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// What this server is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Someone's own server: sign-up, publishing, sharing out.
    Node,
    /// A shared directory: receives shares, no sign-up.
    Commons,
}

impl Role {
    /// The role as written in the role file and on pages: `node` or `commons`.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Node => "node",
            Role::Commons => "commons",
        }
    }

    /// Parse `node` or `commons` (any case, surrounding space ignored).
    pub fn parse(s: &str) -> Option<Role> {
        match s.trim().to_lowercase().as_str() {
            "node" => Some(Role::Node),
            "commons" => Some(Role::Commons),
            _ => None,
        }
    }
}

/// The contents of `instance.json`.
#[derive(Serialize, Deserialize)]
pub struct InstanceFile {
    pub role: Role,
    /// When the role was decided, in Unix seconds.
    pub created_at: u64,
}

/// What [`decide`] knows about a folder.
pub struct Facts<'a> {
    /// The role in `instance.json`: `None` if there is no file, `Some(None)`
    /// if the file exists but is unreadable.
    pub saved: Option<Option<Role>>,
    /// `HIC_ROLE`, if set (raw text).
    pub env: Option<&'a str>,
    /// `HIC_INSTANCE`, used only for the deprecated guess.
    pub instance_label: &'a str,
    /// True when the folder already holds member identities (`handles/*.json`).
    pub has_members: bool,
}

/// The outcome of [`decide`].
#[derive(Debug, PartialEq, Eq)]
pub struct Decision {
    pub role: Role,
    /// True when `instance.json` should be written (a new folder).
    pub save: bool,
    /// True when the role was guessed from the instance label (deprecated).
    pub guessed: bool,
}

/// Decide the role, in order:
/// 1. the saved role (and `HIC_ROLE`, if set, must match it);
/// 2. `HIC_ROLE`, for a new folder;
/// 3. deprecated: "commons" if the instance label mentions "commons" or
///    "federated", so hosted commons from before `HIC_ROLE` stay closed to
///    sign-up after an upgrade;
/// 4. otherwise node.
///
/// A folder that already holds member identities can never become a commons.
pub fn decide(f: &Facts, folder: &str) -> Result<Decision, String> {
    let env = match f.env.map(str::trim).filter(|s| !s.is_empty()) {
        None => None,
        Some(raw) => {
            Some(Role::parse(raw).ok_or_else(|| format!("HIC_ROLE must be \"node\" or \"commons\", not \"{raw}\"."))?)
        }
    };

    if let Some(saved) = f.saved {
        let saved =
            saved.ok_or_else(|| format!("{folder}/instance.json is unreadable. Fix or remove it to continue."))?;
        if let Some(e) = env.filter(|e| *e != saved) {
            return Err(format!(
                "This data folder ({folder}) belongs to a {}, but HIC_ROLE={}. A folder's role can't change; \
                 point HIC_DATA at a new folder to run a {}.",
                saved.as_str(),
                e.as_str(),
                e.as_str()
            ));
        }
        return Ok(Decision { role: saved, save: false, guessed: false });
    }

    let (role, guessed) = match env {
        Some(e) => (e, false),
        None => {
            let label = f.instance_label.to_lowercase();
            if label.contains("commons") || label.contains("federated") {
                (Role::Commons, true)
            } else {
                (Role::Node, false)
            }
        }
    };
    if role == Role::Commons && f.has_members {
        return Err(format!(
            "This data folder ({folder}) holds a node's member identities, so it can't become a commons. \
             Point HIC_DATA at a new folder to run a commons."
        ));
    }
    Ok(Decision { role, save: true, guessed })
}

/// What is on disk about a data folder's role. Reads only.
pub struct OnDisk {
    /// See [`Facts::saved`].
    pub saved: Option<Option<Role>>,
    /// See [`Facts::has_members`].
    pub has_members: bool,
}

/// Read `instance.json` and check for member identities. Reads only.
pub fn inspect(data_dir: &Path) -> OnDisk {
    let path = data_dir.join("instance.json");
    let saved = path.exists().then(|| {
        std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<InstanceFile>(&s).ok()).map(|f| f.role)
    });
    let has_members = std::fs::read_dir(data_dir.join("handles"))
        .map(|mut d| d.any(|e| e.map(|e| e.path().extension().is_some_and(|x| x == "json")).unwrap_or(false)))
        .unwrap_or(false);
    OnDisk { saved, has_members }
}

/// Write `instance.json` for a folder's newly decided role. Writes only.
pub fn save(data_dir: &Path, role: Role, now: u64) -> Result<(), String> {
    let path = data_dir.join("instance.json");
    let json = serde_json::to_string_pretty(&InstanceFile { role, created_at: now }).map_err(|e| e.to_string())?;
    crate::store::write_atomic(&path, json.as_bytes()).map_err(|e| format!("could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The facts `decide` looks at, written out in one line per test.
    fn facts(
        saved: Option<Option<Role>>,
        env: Option<&'static str>,
        label: &'static str,
        has_members: bool,
    ) -> Facts<'static> {
        Facts { saved, env, instance_label: label, has_members }
    }

    #[test]
    fn saved_role_wins_and_is_not_rewritten() {
        let d = decide(&facts(Some(Some(Role::Commons)), None, "x", false), "f").unwrap();
        assert_eq!(d, Decision { role: Role::Commons, save: false, guessed: false });
    }

    #[test]
    fn env_that_contradicts_saved_role_refuses() {
        let e = decide(&facts(Some(Some(Role::Commons)), Some("node"), "x", false), "f").unwrap_err();
        assert!(e.contains("belongs to a commons"));
    }

    #[test]
    fn unreadable_file_refuses() {
        assert!(decide(&facts(Some(None), None, "x", false), "f").unwrap_err().contains("unreadable"));
    }

    #[test]
    fn bad_env_value_refuses() {
        assert!(decide(&facts(None, Some("server"), "x", false), "f").is_err());
    }

    #[test]
    fn new_folder_takes_env_and_saves() {
        let d = decide(&facts(None, Some(" Commons "), "x", false), "f").unwrap();
        assert_eq!(d, Decision { role: Role::Commons, save: true, guessed: false });
    }

    #[test]
    fn legacy_label_guess_still_works() {
        let d = decide(&facts(None, None, "Held in Common (federated)", false), "f").unwrap();
        assert!(d.guessed && d.role == Role::Commons);
        assert_eq!(decide(&facts(None, None, "Tega node", false), "f").unwrap().role, Role::Node);
    }

    #[test]
    fn node_folder_never_becomes_a_commons() {
        assert!(decide(&facts(None, Some("commons"), "x", true), "f").unwrap_err().contains("member identities"));
    }
}
