//! The local store: one JSON file per record in the data directory.
//!
//! Writes go to a temporary file first and are then renamed into place, so a
//! crash mid-write leaves the previous version intact rather than half a file.

use serde::{de::DeserializeOwned, Serialize};
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use tokio::sync::Mutex;

pub struct Store {
    dir: PathBuf,
    /// Serialises writes so two saves can't race on the same temp file.
    write_lock: Mutex<()>,
}

impl Store {
    pub async fn open(dir: PathBuf) -> io::Result<Self> {
        tokio::fs::create_dir_all(&dir).await?;
        Ok(Self { dir, write_lock: Mutex::new(()) })
    }

    /// `Ok(None)` when the record has never been saved.
    pub async fn read<T: DeserializeOwned>(&self, name: &str) -> io::Result<Option<T>> {
        match tokio::fs::read(self.path(name)).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| io::Error::new(ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn write<T: Serialize>(&self, name: &str, value: &T) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(value)?;
        let _guard = self.write_lock.lock().await;
        let tmp = self.path(&format!("{name}.tmp"));
        tokio::fs::write(&tmp, json).await?;
        tokio::fs::rename(&tmp, self.path(name)).await
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }
}
