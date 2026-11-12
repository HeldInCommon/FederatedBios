//! The data folder: the only module that reads or writes files.
//!
//! ```text
//! $HIC_DATA/
//!   instance.json            this folder's role (see crate::role)
//!   handles/<handle>.json    a member's identity and current version (nodes)
//!   versions/<cid>.json      immutable, content-addressed versions (nodes)
//!   remote/<keyid>.json      profiles shared in from other nodes (commons)
//!   tombstones/<keyid>.json  what remains after a withdrawal (commons)
//!   identities/<handle>.json who holds each handle, recovery key, rotation history (commons)
//!   intros/<keyid>.json      how each person joined, and who vouched (commons)
//!   pending/<keyid>.json     shares waiting for a vouch (commons)
//!   used-invites/<id>.json   invite codes already used (commons)
//!   founding-invite.json     the one-time invite for the first member (commons)
//!   steward.json             the steward key in the steward link (commons)
//!   commons-invites/<id>.json invite codes made by the steward (commons)
//!   notices.json             things members should hear about (nodes)
//!   my-stories/<id>.json     stories members wrote here (nodes)
//!   stories/<keyid>-<id>.json stories shared in from other nodes (commons)
//!   story-tombstones/<keyid>-<id>.json what remains after a story is withdrawn (commons)
//!   story-cards/<keyid>-<id>.json "Only while I'm here" cards: no text (commons)
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
use std::sync::{Arc, Mutex};

use serde::{de::DeserializeOwned, Serialize};

use crate::canonical::is_cid;
use crate::handle;
use crate::here::HeldCard;
use crate::identity::Identity;
use crate::keys::{is_keyid, keyid};
use crate::model::{HandleRec, Notice, PendingShare, RemoteEntry, Tombstone, Version};
use crate::story::{is_story_id, HeldStory, OwnStory, StoryTombstone};
use crate::vouching::Intro;

/// A failure reading or writing the data folder.
#[derive(Debug)]
pub enum StoreError {
    /// The file system refused (permissions, disk full, …).
    Io(PathBuf, std::io::Error),
    /// The file exists but isn't valid JSON for its type.
    Corrupt(PathBuf, serde_json::Error),
}

impl fmt::Display for StoreError {
    /// Say what went wrong and which file, for logs and the operator.
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
///
/// The two lists read on almost every request, all members ([`Store::handles`])
/// and all shared profiles ([`Store::remotes`]), are kept in memory after the
/// first read, so a page doesn't read every file (review item M8). **Every
/// write through the store that changes one of those lists clears its cache**
/// (`save_handle`, `save_remote`, `delete_remote`, and so everything built on
/// them). Files changed by hand while the server runs aren't seen until a
/// restart. Clones of a `Store` share the cache.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    handles_cache: Arc<Mutex<Option<Vec<HandleRec>>>>,
    remotes_cache: Arc<Mutex<Option<Vec<RemoteEntry>>>>,
}

impl Store {
    /// Use `root` as the data folder, creating its sub-folders.
    pub fn open(root: impl Into<PathBuf>) -> std::io::Result<Store> {
        let root = root.into();
        for sub in [
            "handles",
            "versions",
            "remote",
            "tombstones",
            "identities",
            "intros",
            "pending",
            "used-invites",
            "commons-invites",
            "my-stories",
            "stories",
            "story-tombstones",
            "story-cards",
        ] {
            std::fs::create_dir_all(root.join(sub))?;
        }
        Ok(Store { root, handles_cache: Arc::default(), remotes_cache: Arc::default() })
    }

    /// The data folder's path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // ---- generic helpers -------------------------------------------------

    /// Read one JSON file. `Ok(None)` when it doesn't exist; an error when it
    /// can't be read or doesn't parse.
    fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, StoreError> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| StoreError::Corrupt(path.into(), e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StoreError::Io(path.into(), e)),
        }
    }

    /// Write one value as pretty JSON, atomically (see [`write_atomic`]).
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

    /// Where a member's record lives. Callers check the handle is valid first.
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
        Self::write(&self.handle_path(&rec.handle), rec)?;
        *self.handles_cache.lock().unwrap() = None;
        Ok(())
    }

    /// Every member, newest first (from memory after the first read).
    pub fn handles(&self) -> Vec<HandleRec> {
        let mut cache = self.handles_cache.lock().unwrap();
        cache
            .get_or_insert_with(|| {
                let mut out: Vec<HandleRec> = self.read_all("handles");
                out.sort_by_key(|r| std::cmp::Reverse(r.created_at));
                out
            })
            .clone()
    }

    // ---- versions --------------------------------------------------------

    /// Where a version lives. Callers check the `cid` is well formed first.
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

    /// Where a shared profile lives. Callers check the key id first.
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
        Self::write(&self.remote_path(&keyid(&r.ed_pub)), r)?;
        *self.remotes_cache.lock().unwrap() = None;
        Ok(())
    }

    /// Every shared profile, most recently received first (from memory after
    /// the first read).
    pub fn remotes(&self) -> Vec<RemoteEntry> {
        let mut cache = self.remotes_cache.lock().unwrap();
        cache
            .get_or_insert_with(|| {
                let mut out: Vec<RemoteEntry> = self.read_all("remote");
                out.sort_by_key(|r| std::cmp::Reverse(r.received_at));
                out
            })
            .clone()
    }

    /// Remove a shared profile (after a withdrawal). Removing one that isn't
    /// there is not an error.
    pub fn delete_remote(&self, kid: &str) -> Result<(), StoreError> {
        if !is_keyid(kid) {
            return Ok(());
        }
        *self.remotes_cache.lock().unwrap() = None;
        Self::remove_if_present(self.remote_path(kid))
    }

    // ---- stories ------------------------------------------------------------

    /// Where a story written on this node lives. Callers check the id first.
    fn own_story_path(&self, id: &str) -> PathBuf {
        self.root.join("my-stories").join(format!("{id}.json"))
    }

    /// A story written on this node. `Ok(None)` if absent or `id` is malformed.
    pub fn own_story(&self, id: &str) -> Result<Option<OwnStory>, StoreError> {
        if !is_story_id(id) {
            return Ok(None);
        }
        Self::read(&self.own_story_path(id))
    }

    /// Save a story written on this node.
    pub fn save_own_story(&self, s: &OwnStory) -> Result<(), StoreError> {
        if !is_story_id(&s.story.id) {
            return Err(StoreError::Io(self.root.join("my-stories"), std::io::ErrorKind::InvalidInput.into()));
        }
        Self::write(&self.own_story_path(&s.story.id), s)
    }

    /// Every story written on this node, newest first.
    pub fn own_stories(&self) -> Vec<OwnStory> {
        let mut out: Vec<OwnStory> = self.read_all("my-stories");
        out.sort_by_key(|s| std::cmp::Reverse(s.created_at()));
        out
    }

    /// The file name for a held story: the author's key id, then the story id.
    fn held_story_name(author: &str, id: &str) -> String {
        format!("{}-{id}.json", keyid(author))
    }

    /// A story shared to this commons, by its author's [`keyid`] and its id.
    pub fn held_story(&self, kid: &str, id: &str) -> Result<Option<HeldStory>, StoreError> {
        if !is_keyid(kid) || !is_story_id(id) {
            return Ok(None);
        }
        Self::read(&self.root.join("stories").join(format!("{kid}-{id}.json")))
    }

    /// Save a story shared to this commons, replacing that author's earlier version.
    pub fn save_held_story(&self, h: &HeldStory) -> Result<(), StoreError> {
        if !is_story_id(&h.story.id) {
            return Err(StoreError::Io(self.root.join("stories"), std::io::ErrorKind::InvalidInput.into()));
        }
        Self::write(&self.root.join("stories").join(Self::held_story_name(h.author(), &h.story.id)), h)
    }

    /// Every story shared to this commons, most recently received first.
    pub fn held_stories(&self) -> Vec<HeldStory> {
        let mut out: Vec<HeldStory> = self.read_all("stories");
        out.sort_by_key(|h| std::cmp::Reverse(h.received_at));
        out
    }

    /// Remove a shared story. Removing one that isn't there is not an error.
    pub fn delete_held_story(&self, author: &str, id: &str) -> Result<(), StoreError> {
        if !is_story_id(id) {
            return Ok(());
        }
        Self::remove_if_present(self.root.join("stories").join(Self::held_story_name(author, id)))
    }

    /// Remove every story by `author`, copies and "Only while I'm here" cards
    /// (when they withdraw their profile here, or their key is replaced).
    pub fn delete_stories_by(&self, author: &str) -> Result<(), StoreError> {
        for h in self.held_stories().iter().filter(|h| h.author() == author) {
            self.delete_held_story(author, &h.story.id)?;
        }
        for c in self.story_cards().iter().filter(|c| c.author() == author) {
            self.delete_story_card(author, &c.card.id)?;
        }
        Ok(())
    }

    // ---- "Only while I'm here" cards (decision record 0021) ----------------

    /// A story's card held on this commons, by its author's [`keyid`] and the
    /// story's id. The card has no text: that stays on the author's node.
    pub fn story_card(&self, kid: &str, id: &str) -> Result<Option<HeldCard>, StoreError> {
        if !is_keyid(kid) || !is_story_id(id) {
            return Ok(None);
        }
        Self::read(&self.root.join("story-cards").join(format!("{kid}-{id}.json")))
    }

    /// Save a story's card, replacing that author's earlier card for it.
    pub fn save_story_card(&self, c: &HeldCard) -> Result<(), StoreError> {
        if !is_story_id(&c.card.id) {
            return Err(StoreError::Io(self.root.join("story-cards"), std::io::ErrorKind::InvalidInput.into()));
        }
        Self::write(&self.root.join("story-cards").join(Self::held_story_name(c.author(), &c.card.id)), c)
    }

    /// Every card held on this commons, most recently received first.
    pub fn story_cards(&self) -> Vec<HeldCard> {
        let mut out: Vec<HeldCard> = self.read_all("story-cards");
        out.sort_by_key(|c| std::cmp::Reverse(c.received_at));
        out
    }

    /// Remove a card. Removing one that isn't there is not an error.
    pub fn delete_story_card(&self, author: &str, id: &str) -> Result<(), StoreError> {
        if !is_story_id(id) {
            return Ok(());
        }
        Self::remove_if_present(self.root.join("story-cards").join(Self::held_story_name(author, id)))
    }

    /// Save the record of a story's withdrawal.
    pub fn save_story_tombstone(&self, t: &StoryTombstone) -> Result<(), StoreError> {
        if !is_story_id(&t.id) {
            return Err(StoreError::Io(self.root.join("story-tombstones"), std::io::ErrorKind::InvalidInput.into()));
        }
        Self::write(&self.root.join("story-tombstones").join(Self::held_story_name(&t.author, &t.id)), t)
    }

    /// Every story withdrawal recorded here.
    pub fn story_tombstones(&self) -> Vec<StoryTombstone> {
        self.read_all("story-tombstones")
    }

    // ---- tombstones (withdrawals on a commons) ------------------------------

    /// Save the record of a withdrawal.
    pub fn save_tombstone(&self, t: &Tombstone) -> Result<(), StoreError> {
        Self::write(&self.tombstone_path(&t.ed_pub), t)
    }

    /// Every withdrawal recorded here.
    pub fn tombstones(&self) -> Vec<Tombstone> {
        self.read_all("tombstones")
    }

    /// Where the withdrawal record for a key lives, named by its key id.
    fn tombstone_path(&self, ed_pub: &str) -> PathBuf {
        self.root.join("tombstones").join(format!("{}.json", keyid(ed_pub)))
    }

    /// Remove the withdrawal record for a key (used when a handle moves to a
    /// new key). Removing one that isn't there is not an error.
    pub fn delete_tombstone(&self, ed_pub: &str) -> Result<(), StoreError> {
        let p = self.tombstone_path(ed_pub);
        match std::fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StoreError::Io(p, e)),
        }
    }

    // ---- identities (who holds each handle, on a commons) -------------------

    /// Where a handle's identity record lives. Callers check the handle first.
    fn identity_path(&self, h: &str) -> PathBuf {
        self.root.join("identities").join(format!("{h}.json"))
    }

    /// The identity record for a handle, if any.
    pub fn identity(&self, h: &str) -> Result<Option<Identity>, StoreError> {
        if !handle::is_valid(h) {
            return Ok(None);
        }
        Self::read(&self.identity_path(h))
    }

    /// Save an identity record.
    pub fn save_identity(&self, id: &Identity) -> Result<(), StoreError> {
        if !handle::is_valid(&id.handle) {
            return Err(StoreError::Io(self.identity_path("invalid"), std::io::Error::other("invalid handle")));
        }
        Self::write(&self.identity_path(&id.handle), id)
    }

    // ---- vouching (commons) ------------------------------------------------

    /// Delete a file. One that is already gone is not an error, so a retry is safe.
    fn remove_if_present(p: PathBuf) -> Result<(), StoreError> {
        match std::fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StoreError::Io(p, e)),
        }
    }

    /// Record how someone joined.
    pub fn save_intro(&self, i: &Intro) -> Result<(), StoreError> {
        Self::write(&self.root.join("intros").join(format!("{}.json", keyid(&i.key))), i)
    }

    /// Every recorded admission.
    pub fn intros(&self) -> Vec<Intro> {
        self.read_all("intros")
    }

    /// Put a share on the pending list (replacing that key's earlier one).
    pub fn save_pending(&self, p: &PendingShare) -> Result<(), StoreError> {
        Self::write(&self.root.join("pending").join(format!("{}.json", keyid(&p.entry.ed_pub))), p)
    }

    /// Every share on the pending list, oldest first.
    pub fn pending(&self) -> Vec<PendingShare> {
        let mut out: Vec<PendingShare> = self.read_all("pending");
        out.sort_by_key(|p| p.entry.received_at);
        out
    }

    /// Take a share off the pending list.
    pub fn delete_pending(&self, ed_pub: &str) -> Result<(), StoreError> {
        Self::remove_if_present(self.root.join("pending").join(format!("{}.json", keyid(ed_pub))))
    }

    /// Invite codes already used.
    pub fn used_codes(&self) -> Vec<String> {
        self.read_all::<serde_json::Value>("used-invites")
            .into_iter()
            .filter_map(|v| v.get("code_id").and_then(|c| c.as_str()).map(String::from))
            .collect()
    }

    /// Mark an invite code as used.
    pub fn mark_code_used(&self, code_id: &str, at: u64) -> Result<(), StoreError> {
        if code_id.is_empty() || !code_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(StoreError::Io(self.root.join("used-invites"), std::io::Error::other("invalid code id")));
        }
        let rec = serde_json::json!({ "code_id": code_id, "used_at": at });
        Self::write(&self.root.join("used-invites").join(format!("{code_id}.json")), &rec)
    }

    /// The founding invite, if one exists and hasn't been used.
    pub fn founding_code(&self) -> Result<Option<String>, StoreError> {
        let v: Option<serde_json::Value> = Self::read(&self.root.join("founding-invite.json"))?;
        Ok(v.filter(|v| v.get("used") != Some(&serde_json::Value::Bool(true)))
            .and_then(|v| v.get("code").and_then(|c| c.as_str()).map(String::from)))
    }

    /// True when a founding invite has ever been made for this commons.
    pub fn has_founding_record(&self) -> bool {
        self.root.join("founding-invite.json").exists()
    }

    /// Store a new founding invite.
    pub fn save_founding_code(&self, code: &str) -> Result<(), StoreError> {
        Self::write(&self.root.join("founding-invite.json"), &serde_json::json!({ "code": code, "used": false }))
    }

    // ---- notices (nodes) ---------------------------------------------------

    /// The node's notices file (one file holding the whole list).
    fn notices_path(&self) -> PathBuf {
        self.root.join("notices.json")
    }

    /// Every notice, oldest first.
    pub fn notices(&self) -> Result<Vec<Notice>, StoreError> {
        Ok(Self::read(&self.notices_path())?.unwrap_or_default())
    }

    /// Add a notice. **Writes `notices.json`.**
    pub fn add_notice(&self, n: Notice) -> Result<(), StoreError> {
        let mut all = self.notices()?;
        all.push(n);
        Self::write(&self.notices_path(), &all)
    }

    /// Mark all of `handle`'s notices as seen. **Writes `notices.json`.**
    pub fn mark_notices_seen(&self, handle: &str) -> Result<(), StoreError> {
        let all: Vec<Notice> = self
            .notices()?
            .into_iter()
            .map(|n| if n.handle == handle { Notice { seen: true, ..n } } else { n })
            .collect();
        Self::write(&self.notices_path(), &all)
    }

    /// Return the notices not yet shown as system notifications, and mark
    /// them as shown. **Writes `notices.json`.**
    pub fn claim_unpushed_notices(&self) -> Result<Vec<Notice>, StoreError> {
        let all = self.notices()?;
        let fresh: Vec<Notice> = all.iter().filter(|n| !n.pushed).cloned().collect();
        if !fresh.is_empty() {
            let marked: Vec<Notice> = all.into_iter().map(|n| Notice { pushed: true, ..n }).collect();
            Self::write(&self.notices_path(), &marked)?;
        }
        Ok(fresh)
    }

    /// The steward key, if one has been made.
    pub fn steward_key(&self) -> Result<Option<String>, StoreError> {
        let v: Option<serde_json::Value> = Self::read(&self.root.join("steward.json"))?;
        Ok(v.and_then(|v| v.get("key").and_then(|k| k.as_str()).map(String::from)))
    }

    /// Save the steward key.
    pub fn save_steward_key(&self, key: &str) -> Result<(), StoreError> {
        Self::write(&self.root.join("steward.json"), &serde_json::json!({ "key": key }))
    }

    /// Where an invite code's record lives. The file is named by a hash of the
    /// code, so the code itself never appears in a file name.
    fn commons_invite_path(&self, code: &str) -> PathBuf {
        self.root.join("commons-invites").join(format!("{}.json", keyid(code)))
    }

    /// Save a new steward-made invite code.
    pub fn save_commons_invite(&self, code: &str, now: u64) -> Result<(), StoreError> {
        let rec = serde_json::json!({ "code": code, "created_at": now, "used_at": null });
        Self::write(&self.commons_invite_path(code), &rec)
    }

    /// True when `code` is a steward-made invite that hasn't been used.
    pub fn commons_invite_unused(&self, code: &str) -> Result<bool, StoreError> {
        let v: Option<serde_json::Value> = Self::read(&self.commons_invite_path(code))?;
        Ok(v.is_some_and(|v| {
            v.get("code").and_then(|c| c.as_str()).is_some_and(|c| crate::keys::ct_eq(c, code))
                && v["used_at"].is_null()
        }))
    }

    /// Mark a steward-made invite code as used.
    pub fn use_commons_invite(&self, code: &str, now: u64) -> Result<(), StoreError> {
        let rec = serde_json::json!({ "code": code, "used_at": now });
        Self::write(&self.commons_invite_path(code), &rec)
    }

    /// Mark the founding invite as used.
    pub fn use_founding_code(&self) -> Result<(), StoreError> {
        Self::write(&self.root.join("founding-invite.json"), &serde_json::json!({ "used": true }))
    }

    /// Apply a [`crate::federation::Listing`]. **Writes four things:** the
    /// shared profile, the handle's identity record, the admission record (if
    /// any), and removes the key's pending share (if any).
    pub fn apply_listing(&self, l: &crate::federation::Listing) -> Result<(), StoreError> {
        self.save_remote(&l.remote)?;
        self.save_identity(&l.identity)?;
        if let Some(i) = &l.intro {
            self.save_intro(i)?;
        }
        self.delete_pending(&l.remote.ed_pub)
    }

    /// Move everything this server holds for `old_key` to `new_key` (after a
    /// key rotation): the shared profile and any withdrawal record. Stories
    /// are signed by the key itself, so they can't move: they're taken down,
    /// and the member's node re-signs them if they're shared again.
    pub fn rekey(&self, old_key: &str, new_key: &str, new_box_key: &str) -> Result<(), StoreError> {
        if let Some(mut r) = self.remote(&keyid(old_key))? {
            r.ed_pub = new_key.to_string();
            r.box_pub = new_box_key.to_string();
            self.save_remote(&r)?;
            self.delete_remote(&keyid(old_key))?;
        }
        if let Some(mut t) = self.tombstones().into_iter().find(|t| t.ed_pub == old_key) {
            t.ed_pub = new_key.to_string();
            self.save_tombstone(&t)?;
            self.delete_tombstone(old_key)?;
        }
        self.delete_stories_by(old_key)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Profile;

    /// A store in a new, empty folder of its own under the system temp folder,
    /// so tests running at the same time don't share files.
    fn temp_store() -> Store {
        let dir = std::env::temp_dir().join(format!(
            "hic-store-test-{}-{}",
            std::process::id(),
            crate::keys::keyid(&format!("{:?}", std::time::Instant::now()))
        ));
        Store::open(dir).unwrap()
    }

    /// A member record with placeholder keys (the store never checks them).
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
            recovery_pub: None,
            recovery_decl: None,
            pending_rotation: None,
            backup_done: false,
            waiting_on: vec![],
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
    fn cached_lists_follow_writes() {
        let s = temp_store();
        assert!(s.handles().is_empty());
        s.save_handle(&rec("tega")).unwrap();
        assert_eq!(s.handles().len(), 1, "a save clears the cache");
        let copy = s.clone();
        copy.save_handle(&rec("mo")).unwrap();
        assert_eq!(s.handles().len(), 2, "clones share the cache");
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
            signed: None,
            held_signals: Default::default(),
        };
        s.save_remote(&r).unwrap();
        assert_eq!(s.remote(&keyid("abc")).unwrap().unwrap().handle, "tega");
        assert_eq!(s.remotes().len(), 1);
        s.delete_remote(&keyid("abc")).unwrap();
        assert!(s.remotes().is_empty());
    }
}
