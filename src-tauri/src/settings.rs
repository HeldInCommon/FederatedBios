//! What the app remembers between launches, and the folders it uses.
//!
//! The app data folder (`~/Library/Application Support/org.heldincommon.node`
//! on macOS) holds:
//!
//! ```text
//! app.json        {"role": "node" | "commons", "name": "Studio laptop",
//!                  "other_name": "Room 4", "practice": true,
//!                  "practice_name": "Tuesday practice"}
//! node-data/      the node's data folder
//! commons-data/   the commons' data folder
//! practice-data/  the local commons' data folder, while there is one
//! ```
//!
//! One computer can run up to three servers side by side (see [`Kind`]): the
//! role chosen at first launch (`role`, `name`), the other role if it was
//! added later (`other_name`), and a local commons that anyone on the same Wi-Fi can reach
//! (`practice`). The first role keeps port 8080, so nothing changes for
//! anyone who set up the app before this was possible.
//!
//! The **name** is what this computer's node or commons is called: it shows in
//! the coloured bar on every page and travels with shares as the node's name,
//! so someone with several machines can tell them apart. Earlier versions used
//! the computer's account name instead, which shared it with every commons.
//!
//! Pure rules are separate from the two functions that touch the disk:
//! [`load`] only reads, and [`save`] only writes.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Longest name allowed, in characters.
pub const NAME_MAX: usize = 60;

/// The port of the role chosen at first launch.
pub const FIRST_PORT: u16 = 8080;
/// The port of the other role, once added.
pub const SECOND_PORT: u16 = 8081;
/// The local commons' port.
pub const PRACTICE_PORT: u16 = 8082;

/// What the local commons is called until it is renamed. (In code and in
/// `app.json` it is still the "practice" commons, its first name, so older
/// installs keep theirs.)
pub const PRACTICE_NAME: &str = "Local commons";

/// A server the app can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The person's own node.
    Node,
    /// A commons for their room.
    Commons,
    /// A commons only this computer can reach, for learning.
    Practice,
}

impl Kind {
    /// Every kind, in the order menus list them.
    pub const ALL: [Kind; 3] = [Kind::Node, Kind::Commons, Kind::Practice];

    /// The server role it runs (`HIC_ROLE`).
    pub fn role(self) -> &'static str {
        match self {
            Kind::Node => "node",
            Kind::Commons | Kind::Practice => "commons",
        }
    }

    /// Its name in links, commands and menu ids.
    pub fn key(self) -> &'static str {
        match self {
            Kind::Node => "node",
            Kind::Commons => "commons",
            Kind::Practice => "practice",
        }
    }

    /// Read [`Kind::key`] back.
    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.key() == s)
    }

    /// What kind of server it is, as a label. Never "your commons": a
    /// commons belongs to its community, not to the computer that hosts it.
    /// Where a commons has a name, the app shows that instead
    /// ([`Saved::shown_name`]).
    pub fn label(self) -> &'static str {
        match self {
            Kind::Node => "Your node",
            Kind::Commons => "Commons",
            Kind::Practice => PRACTICE_NAME,
        }
    }

    /// The kind with "a", for sentences: "a node", "a commons", "a local
    /// commons".
    pub fn a_kind(self) -> &'static str {
        match self {
            Kind::Node => "a node",
            Kind::Commons => "a commons",
            Kind::Practice => "a local commons",
        }
    }
}

/// The contents of `app.json`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Saved {
    /// "node" or "commons": the role chosen at first launch. It opens when
    /// the app starts and keeps port 8080.
    pub role: Option<String>,
    /// What that node or commons is called, once named.
    #[serde(default)]
    pub name: Option<String>,
    /// The name of the other role, once it has been added too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_name: Option<String>,
    /// True while there is a local commons.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub practice: bool,
    /// What the local commons is called, once renamed (Go → Rename
    /// local commons…); until then [`PRACTICE_NAME`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub practice_name: Option<String>,
    /// The commons' public address, when it is also reachable from outside
    /// the room (Tailscale Funnel, a reverse proxy), e.g.
    /// `"https://heldincommon.example.ts.net"`. Set by hand in `app.json`;
    /// the commons then accepts invites and vouches made for it too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_address: Option<String>,
}

impl Saved {
    /// True when both the role and the name are known, so the server can start.
    pub fn ready(&self) -> bool {
        self.role.is_some() && self.name.is_some()
    }

    /// The kind chosen at first launch.
    pub fn first(&self) -> Option<Kind> {
        self.role.as_deref().and_then(Kind::parse)
    }

    /// What `kind` is called, if this computer runs it.
    pub fn name_of(&self, kind: Kind) -> Option<&str> {
        match kind {
            Kind::Practice => self.practice.then(|| self.practice_name.as_deref().unwrap_or(PRACTICE_NAME)),
            k if self.first() == Some(k) => self.name.as_deref(),
            _ => self.other_name.as_deref().filter(|_| self.ready()),
        }
    }

    /// How the app's menu and buttons show `kind`: "Your node" for the node
    /// (it is the person's own), and a commons by its own name ("Held in
    /// Common Collective Commons"), falling back to the kind.
    pub fn shown_name(&self, kind: Kind) -> String {
        match kind {
            Kind::Node => kind.label().to_string(),
            _ => self.name_of(kind).unwrap_or(kind.label()).to_string(),
        }
    }

    /// Where `kind` is and who can reach it, in a few words, so the app's
    /// buttons and menu never leave anyone guessing: "on this computer, only
    /// you", "local commons, closed to the room", "online commons, hosted
    /// on this computer". This is how each starts; a node or local commons
    /// open to the room says so instead (see `room::place_text`).
    pub fn where_is(&self, kind: Kind) -> &'static str {
        match kind {
            Kind::Node => "on this computer, only you",
            Kind::Practice => "local commons, closed to the room",
            Kind::Commons if self.public_address.is_some() => "online commons, hosted on this computer",
            Kind::Commons => "commons on this computer, anyone on this Wi-Fi",
        }
    }

    /// Does this computer run `kind`?
    pub fn runs(&self, kind: Kind) -> bool {
        self.name_of(kind).is_some()
    }

    /// Every kind this computer runs, first-launch role first.
    pub fn running(&self) -> Vec<Kind> {
        let first = self.first().filter(|_| self.ready());
        first.into_iter().chain(Kind::ALL.into_iter().filter(|k| Some(*k) != first && self.runs(*k))).collect()
    }

    /// Where the window goes when it has nowhere better: the node if there
    /// is one, else the first-launch role.
    pub fn home(&self) -> Option<Kind> {
        if self.runs(Kind::Node) {
            Some(Kind::Node)
        } else {
            self.first().filter(|_| self.ready())
        }
    }

    /// The port `kind` listens on.
    pub fn port_of(&self, kind: Kind) -> u16 {
        port_for(self.role.as_deref(), kind)
    }
}

/// How far this copy of the app moves its ports up from 8080–8082, so a
/// second copy (a test build) can run beside the installed one: with 100,
/// it uses 8180–8182. Baked in at build time from `HIC_PORT_OFFSET` (as the
/// default commons is); 0, and so the usual ports, for every normal build.
/// A test build also needs its own identifier, so it keeps its own data
/// folder (see `docs/MAINTAINER.md`).
pub fn port_offset() -> u16 {
    parse_offset(option_env!("HIC_PORT_OFFSET"))
}

/// A port offset as written, or 0 if it's missing, not a number, or would
/// push the ports past the top of the range. Pure.
pub fn parse_offset(written: Option<&str>) -> u16 {
    written.and_then(|v| v.trim().parse::<u16>().ok()).filter(|o| *o <= 50_000).unwrap_or(0)
}

/// The port for `kind` when `first` is the first-launch role: the
/// first-launch role keeps 8080 (so existing nodes and room commons keep
/// their addresses), the other role gets 8081, and practice 8082, each
/// moved up by [`port_offset`] (0 in a normal build).
pub fn port_for(first: Option<&str>, kind: Kind) -> u16 {
    let base = match kind {
        Kind::Practice => PRACTICE_PORT,
        k if first.is_none_or(|r| r == k.role()) => FIRST_PORT,
        _ => SECOND_PORT,
    };
    base + port_offset()
}

/// The sign-in cookie name for a server on `port` (`HIC_SESSION_COOKIE`).
/// Browsers share cookies between ports of one host, so each server needs
/// its own. 8080 keeps the old name, so nobody is signed out by an update.
pub fn cookie_name(port: u16) -> String {
    if port == FIRST_PORT {
        "hic_session".into()
    } else {
        format!("hic_session_{port}")
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

/// Make `dir` readable only by this computer's current user: the folder
/// becomes `700` (owner only) and each file already in it `600`.
///
/// Used for the log folder. A commons prints its steward link and founding
/// invite at start-up (ADR 0017, ADR 0023), and the log is where a steward
/// finds them, so it must not be readable by other accounts on a shared
/// computer. Locking the folder is what protects files written later
/// (including the rotated backup log); the files are tightened as well in
/// case one is ever moved out of it.
///
/// **Writes** the permissions of `dir` and of the files directly in it.
/// Does nothing on Windows, where a user's own folders are already private.
pub fn make_private(dir: &Path) -> std::io::Result<()> {
    // Learner note: `#[cfg(unix)]` compiles this block only on macOS and
    // Linux; `PermissionsExt` (the `mode` numbers) exists only there.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_file() {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            }
        }
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// The data folder for `kind`. `node-data` is the folder earlier versions
/// used, so existing nodes keep their data.
pub fn data_dir_for(base: &Path, kind: Kind) -> PathBuf {
    base.join(match kind {
        Kind::Node => "node-data",
        Kind::Commons => "commons-data",
        Kind::Practice => "practice-data",
    })
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

/// A public address as typed into `app.json`: trimmed, without a trailing
/// slash, and only if it is an `http(s)://` URL.
pub fn clean_address(a: &str) -> Option<String> {
    let a = a.trim().trim_end_matches('/');
    (a.starts_with("https://") || a.starts_with("http://")).then(|| a.to_string())
}

/// Read `app.json`. Returns the settings and whether they differ from the
/// file (an install from before the chooser, or a name that needed tidying),
/// in which case the caller should [`save`] them. Reads only.
pub fn load(base: &Path) -> (Saved, bool) {
    let file: Saved = std::fs::read_to_string(base.join("app.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let has_members = std::fs::read_dir(base.join("node-data").join("handles"))
        .map(|mut d| d.any(|e| e.map(|e| e.path().extension().is_some_and(|x| x == "json")).unwrap_or(false)))
        .unwrap_or(false);
    let saved = Saved {
        role: decide_role(file.role.clone(), has_members),
        name: file.name.as_deref().and_then(|n| clean_name(n).ok()),
        other_name: file.other_name.as_deref().and_then(|n| clean_name(n).ok()),
        practice: file.practice,
        practice_name: file.practice_name.as_deref().and_then(|n| clean_name(n).ok()),
        public_address: file.public_address.as_deref().and_then(clean_address),
    };
    let changed = saved != file;
    (saved, changed)
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
    fn whether_a_server_is_open_to_the_room_is_never_saved() {
        // Decision record 0027, D2: the room lives in memory only, so every
        // launch starts closed. app.json has no field for it.
        let s = Saved { role: Some("node".into()), name: Some("Laptop".into()), practice: true, ..Default::default() };
        let json = serde_json::to_string(&s).unwrap();
        let lower = json.to_lowercase();
        assert!(!lower.contains("open") && !lower.contains("room"), "{json}");
        let back: Saved = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[cfg(unix)]
    #[test]
    fn the_log_folder_is_readable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        // A fresh folder under the system temp folder, readable by everyone
        // to start with, holding one log file that is too.
        let dir = std::env::temp_dir().join(format!("hic-log-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("held-in-common.log");
        std::fs::write(&file, "steward link").unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o775)).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o664)).unwrap();

        make_private(&dir).unwrap();

        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700, "only the owner may enter the folder");
        assert_eq!(mode(&file), 0o600, "only the owner may read the log");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_public_address_in_app_json_is_kept_and_tidied() {
        let dir = std::env::temp_dir().join(format!("hic-settings-public-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("app.json"),
            r#"{"role":"commons","name":"Room","public_address":" https://hic.tail.ts.net/ "}"#,
        )
        .unwrap();
        assert_eq!(load(&dir).0.public_address.as_deref(), Some("https://hic.tail.ts.net"));
        std::fs::write(dir.join("app.json"), r#"{"role":"commons","name":"Room","public_address":"hic.tail"}"#)
            .unwrap();
        assert_eq!(load(&dir).0.public_address, None, "only an http(s) URL counts");
        let _ = std::fs::remove_dir_all(&dir);
    }

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
    fn folders_per_kind() {
        let b = Path::new("/x");
        assert_eq!(data_dir_for(b, Kind::Node), Path::new("/x/node-data"));
        assert_eq!(data_dir_for(b, Kind::Commons), Path::new("/x/commons-data"));
        assert_eq!(data_dir_for(b, Kind::Practice), Path::new("/x/practice-data"));
    }

    #[test]
    fn a_port_offset_is_read_carefully() {
        assert_eq!(parse_offset(None), 0);
        assert_eq!(parse_offset(Some("100")), 100);
        assert_eq!(parse_offset(Some(" 100 ")), 100);
        assert_eq!(parse_offset(Some("lots")), 0);
        assert_eq!(parse_offset(Some("60000")), 0, "would run past the last port");
    }

    #[test]
    fn the_first_role_keeps_8080() {
        assert_eq!(port_for(Some("node"), Kind::Node), 8080);
        assert_eq!(port_for(Some("node"), Kind::Commons), 8081);
        assert_eq!(port_for(Some("commons"), Kind::Commons), 8080);
        assert_eq!(port_for(Some("commons"), Kind::Node), 8081);
        assert_eq!(port_for(None, Kind::Commons), 8080, "first launch: whatever is chosen gets 8080");
        assert_eq!(port_for(Some("node"), Kind::Practice), 8082);
    }

    #[test]
    fn each_port_has_its_own_cookie() {
        assert_eq!(cookie_name(8080), "hic_session");
        assert_eq!(cookie_name(8081), "hic_session_8081");
        assert_ne!(cookie_name(8081), cookie_name(8082));
    }

    #[test]
    fn what_a_computer_runs() {
        let node_only = Saved { role: Some("node".into()), name: Some("Laptop".into()), ..Default::default() };
        assert_eq!(node_only.running(), vec![Kind::Node]);
        assert!(!node_only.runs(Kind::Commons));
        assert_eq!(node_only.home(), Some(Kind::Node));

        let both = Saved { other_name: Some("Room 4".into()), practice: true, ..node_only.clone() };
        assert_eq!(both.running(), vec![Kind::Node, Kind::Commons, Kind::Practice]);
        assert_eq!(both.name_of(Kind::Commons), Some("Room 4"));
        assert_eq!(both.name_of(Kind::Practice), Some(PRACTICE_NAME));
        let renamed = Saved { practice_name: Some("Tuesday practice".into()), ..both.clone() };
        assert_eq!(renamed.name_of(Kind::Practice), Some("Tuesday practice"), "a practice commons can be named");
        let removed = Saved { practice: false, ..renamed };
        assert_eq!(removed.name_of(Kind::Practice), None, "no name without a practice commons");

        let steward = Saved {
            role: Some("commons".into()),
            name: Some("Room 4".into()),
            other_name: Some("Laptop".into()),
            practice: false,
            practice_name: None,
            public_address: None,
        };
        assert_eq!(steward.running(), vec![Kind::Commons, Kind::Node], "the first-launch role comes first");
        assert_eq!(steward.home(), Some(Kind::Node));
        assert_eq!(steward.port_of(Kind::Node), 8081);
    }

    #[test]
    fn kinds_round_trip() {
        for k in Kind::ALL {
            assert_eq!(Kind::parse(k.key()), Some(k));
        }
        assert_eq!(Kind::parse("other"), None);
    }

    #[test]
    fn app_json_without_the_new_fields_still_reads_and_old_apps_can_read_ours() {
        let s: Saved = serde_json::from_str(r#"{"role":"node","name":"Laptop"}"#).unwrap();
        assert_eq!(s.other_name, None);
        assert!(!s.practice);
        // An app from before this change reads only role and name.
        let json = serde_json::to_string(&Saved { other_name: Some("Room".into()), practice: true, ..s }).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["role"], "node");
        assert_eq!(v["name"], "Laptop");
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
    fn the_app_never_calls_a_commons_yours() {
        let s = Saved {
            role: Some("commons".into()),
            name: Some("Held in Common Collective Commons".into()),
            other_name: Some("Laptop".into()),
            practice: true,
            ..Default::default()
        };
        assert_eq!(s.shown_name(Kind::Commons), "Held in Common Collective Commons");
        assert_eq!(s.shown_name(Kind::Node), "Your node", "the node is the person's own");
        assert_eq!(s.shown_name(Kind::Practice), PRACTICE_NAME);
        assert_eq!(s.where_is(Kind::Node), "on this computer, only you");
        assert_eq!(s.where_is(Kind::Practice), "local commons, closed to the room", "every launch starts closed");
        assert_eq!(s.where_is(Kind::Commons), "commons on this computer, anyone on this Wi-Fi");
        let online = Saved { public_address: Some("https://hic.tail.ts.net".into()), ..s.clone() };
        assert_eq!(online.where_is(Kind::Commons), "online commons, hosted on this computer");
        for k in Kind::ALL {
            assert!(!k.label().to_lowercase().contains("your commons"), "{k:?}");
            assert!(!s.shown_name(k).to_lowercase().contains("your commons"), "{k:?}");
        }
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
        assert_eq!(s, Saved { role: Some("node".into()), name: None, ..Default::default() });
        assert!(!s.ready());
    }
}
