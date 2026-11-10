//! The data folder: the only module that reads or writes files.
//!
//! ```text
//! $HIC_DATA/
//!   instance.json            this folder's role (see crate::role)
//!   handles/<handle>.json    a member's identity and current version (nodes)
//!   versions/<cid>.json      immutable, content-addressed versions (nodes)
//!   remote/<keyid>.json      profiles shared in from other nodes (commons)
//!   tombstones/<keyid>.json  what remains after a withdrawal (commons)
//! ```
//!
//! Every write is atomic: the data goes to a temporary file in the same
//! folder, is flushed to disk, then renamed over the old file. A crash or
//! power cut leaves either the old file or the new one, never half of one.
//!
//! Reading distinguishes "not there" (`Ok(None)`) from "there but unreadable"
//! (`Err`). That matters: if a damaged identity file looked missing, someone
//! could register the same handle again and overwrite it.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};

use crate::canonical::is_cid;
use crate::handle;
use crate::keys::{is_keyid, keyid};
use crate::model::{HandleRec, RemoteEntry, Tombstone, Version};

/// A failure reading or writing the data folder.
#[derive(Debug)]
pub enum StoreError {
    /// The file system refused (permissions, disk full, …).
    Io(PathBuf, std::io::Error),
    /// The file exists but isn't valid JSON for its type.
    Corrupt(PathBuf, serde_json::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(p, e) => write!(f, "{}: {e}", p.display()),
            StoreError::Corrupt(p, e) => write!(f, "{} is damaged: {e}", p.display()),
        }
    }
}

impl std::error::Error for StoreError {}

/// Write `bytes` to `path` atomically (temporary file, flush, rename).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| std::io::Error::other("path has no folder"))?;
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    // Flush the folder entry too, so the rename itself survives a power cut.
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// The data folder.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Use `root` as the data folder, creating its sub-folders.
    pub fn open(root: impl Into<PathBuf>) -> std::io::Result<Store> {
        let root = root.into();
        for sub in ["handles", "versions", "remote", "tombstones"] {
            std::fs::create_dir_all(root.join(sub))?;
        }
        Ok(Store { root })
    }

    /// The data folder's path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // ---- generic helpers -------------------------------------------------

    fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, StoreError> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| StoreError::Corrupt(path.into(), e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StoreError::Io(path.into(), e)),
        }
    }

    fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec_pretty(value).expect("stored types serialise");
        write_atomic(path, &bytes).map_err(|e| StoreError::Io(path.into(), e))
    }

    /// Every readable `*.json` in a sub-folder. Damaged files are skipped and
    /// reported on stderr, so one bad file can't take the directory down.
    fn read_all<T: DeserializeOwned>(&self, sub: &str) -> Vec<T> {
        let Ok(dir) = std::fs::read_dir(self.root.join(sub)) else { return Vec::new() };
        dir.flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| match Self::read::<T>(&p) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("warning: skipping {e}");
                    None
                }
            })
            .collect()
    }

    // ---- handles (a node's members) ----------------------------------------

    fn handle_path(&self, h: &str) -> PathBuf {
        self.root.join("handles").join(format!("{h}.json"))
    }

    /// A member's record. `Ok(None)` if there is none, or if `h` isn't a valid
    /// handle (so no crafted name ever reaches the filesystem).
    pub fn handle(&self, h: &str) -> Result<Option<HandleRec>, StoreError> {
        if !handle::is_valid(h) {
            return Ok(None);
        }
        Self::read(&self.handle_path(h))
    }

    /// Save a member's record.
    pub fn save_handle(&self, rec: &HandleRec) -> Result<(), StoreError> {
        if !handle::is_valid(&rec.handle) {
            return Err(StoreError::Io(self.handle_path("invalid"), std::io::Error::other("invalid handle")));
        }
        Self::write(&self.handle_path(&rec.handle), rec)
    }

    /// Every member, newest first.
    pub fn handles(&self) -> Vec<HandleRec> {
        let mut out: Vec<HandleRec> = self.read_all("handles");
        out.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        out
    }

    // ---- versions --------------------------------------------------------

    fn version_path(&self, cid: &str) -> PathBuf {
        self.root.join("versions").join(format!("{cid}.json"))
    }

    /// A version by `cid`. `Ok(None)` if absent or `cid` is malformed.
    pub fn version(&self, cid: &str) -> Result<Option<Version>, StoreError> {
        if !is_cid(cid) {
            return Ok(None);
        }
        Self::read(&self.version_path(cid))
    }

    /// Save a version (named by its own `cid`).
    pub fn save_version(&self, v: &Version) -> Result<(), StoreError> {
        if !is_cid(&v.cid) {
            return Err(StoreError::Io(self.version_path("invalid"), std::io::Error::other("invalid cid")));
        }
        Self::write(&self.version_path(&v.cid), v)
    }

    /// A member's current version, if they have published one.
    pub fn current_version(&self, rec: &HandleRec) -> Result<Option<Version>, StoreError> {
        match &rec.current_cid {
            Some(cid) => self.version(cid),
            None => Ok(None),
        }
    }

    // ---- remote (profiles shared to a commons) -----------------------------

    fn remote_path(&self, kid: &str) -> PathBuf {
        self.root.join("remote").join(format!("{kid}.json"))
    }

    /// A shared profile by [`keyid`]. `Ok(None)` if absent or malformed.
    pub fn remote(&self, kid: &str) -> Result<Option<RemoteEntry>, StoreError> {
        if !is_keyid(kid) {
            return Ok(None);
        }
        Self::read(&self.remote_path(kid))
    }

    /// Save a shared profile, replacing that key's previous one.
    pub fn save_remote(&self, r: &RemoteEntry) -> Result<(), StoreError> {
        Self::write(&self.remote_path(&keyid(&r.ed_pub)), r)
    }

    /// Every shared profile, most recently received first.
    pub fn remotes(&self) -> Vec<RemoteEntry> {
        let mut out: Vec<RemoteEntry> = self.read_all("remote");
        out.sort_by_key(|r| std::cmp::Reverse(r.received_at));
        out
    }

    /// Remove a shared profile (after a withdrawal). Removing one that isn't
    /// there is not an error.
    pub fn delete_remote(&self, kid: &str) -> Result<(), StoreError> {
        if !is_keyid(kid) {
            return Ok(());
        }
        let p = self.remote_path(kid);
        match std::fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StoreError::Io(p, e)),
        }
    }

    // ---- tombstones (withdrawals on a commons) ------------------------------

    /// Save the record of a withdrawal.
    pub fn save_tombstone(&self, t: &Tombstone) -> Result<(), StoreError> {
        Self::write(&self.root.join("tombstones").join(format!("{}.json", keyid(&t.ed_pub))), t)
    }

    /// Every withdrawal recorded here.
    pub fn tombstones(&self) -> Vec<Tombstone> {
        self.read_all("tombstones")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Profile;

    fn temp_store() -> Store {
        let dir = std::env::temp_dir().join(format!(
            "hic-store-test-{}-{}",
            std::process::id(),
            crate::keys::keyid(&format!("{:?}", std::time::Instant::now()))
        ));
        Store::open(dir).unwrap()
    }

    fn rec(h: &str) -> HandleRec {
        HandleRec {
            handle: h.into(),
            display_name: h.into(),
            created_at: 1,
            ed_pub: "aa".into(),
            ed_priv: "bb".into(),
            box_pub: "cc".into(),
            box_priv: "dd".into(),
            secret_salt: String::new(),
            secret_hash: String::new(),
            current_cid: None,
            shared_to: vec![],
        }
    }

    #[test]
    fn handle_round_trip_and_missing() {
        let s = temp_store();
        assert!(s.handle("tega").unwrap().is_none());
        s.save_handle(&rec("tega")).unwrap();
        assert_eq!(s.handle("tega").unwrap().unwrap().handle, "tega");
        assert_eq!(s.handles().len(), 1);
    }

    #[test]
    fn damaged_file_is_an_error_not_missing() {
        let s = temp_store();
        std::fs::write(s.root().join("handles/tega.json"), b"{\"handle\": \"te").unwrap();
        assert!(matches!(s.handle("tega"), Err(StoreError::Corrupt(..))));
        assert!(s.handles().is_empty(), "listing skips the damaged file");
    }

    #[test]
    fn invalid_names_never_touch_the_disk() {
        let s = temp_store();
        assert!(s.handle("../instance").unwrap().is_none());
        assert!(s.version("../../etc/passwd").unwrap().is_none());
        assert!(s.remote("../x").unwrap().is_none());
    }

    #[test]
    fn atomic_write_leaves_no_temp_files() {
        let s = temp_store();
        s.save_handle(&rec("tega")).unwrap();
        s.save_handle(&rec("tega")).unwrap();
        let names: Vec<_> =
            std::fs::read_dir(s.root().join("handles")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(names.len(), 1, "{names:?}");
    }

    #[test]
    fn delete_remote_and_tombstones() {
        let s = temp_store();
        s.delete_remote(&keyid("nothing-here")).unwrap();
        let t = Tombstone { ed_pub: "abc".into(), handle: "tega".into(), withdrawn_at: 5 };
        s.save_tombstone(&t).unwrap();
        assert_eq!(s.tombstones(), vec![t]);
    }

    #[test]
    fn remote_round_trip() {
        let s = temp_store();
        let r = RemoteEntry {
            ed_pub: "abc".into(),
            box_pub: String::new(),
            handle: "tega".into(),
            display_name: "Tega".into(),
            origin: String::new(),
            origin_label: String::new(),
            cid: "c".into(),
            visibility: "public".into(),
            license: "cc-by".into(),
            signals: vec![],
            created_at: 1,
            profile: Profile::default(),
            signature: String::new(),
            received_at: 2,
        };
        s.save_remote(&r).unwrap();
        assert_eq!(s.remote(&keyid("abc")).unwrap().unwrap().handle, "tega");
        assert_eq!(s.remotes().len(), 1);
        s.delete_remote(&keyid("abc")).unwrap();
        assert!(s.remotes().is_empty());
    }
}
