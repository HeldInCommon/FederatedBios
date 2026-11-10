//! Held in Common — the server behind every node and every commons.
//!
//! One binary, two roles:
//! - a **node** is someone's own server. It holds their identity (keypairs),
//!   lets them publish signed, content-addressed versions of their profile, and
//!   shares copies to commons;
//! - a **commons** is a shared directory. It has no sign-up; it only receives
//!   signed copies from nodes at `/inbox`, verifies them and lists them.
//!
//! # Layout
//!
//! The code is split so that each module has one job, and so that decisions
//! (pure functions, easy to test) are kept apart from effects (files, network,
//! clock, randomness):
//!
//! | Module | Job | Effects? |
//! |---|---|---|
//! | [`model`] | the data types that are stored and sent | none |
//! | [`governance`] | the licence and signal vocabularies | none |
//! | [`handle`] | rules for what a handle may be | none |
//! | [`canonical`] | the content address (`cid`) of a profile version (format version 0) | none |
//! | [`jcs`] | canonical JSON (RFC 8785) | none |
//! | [`envelope`] | signed objects in format `hic/v1` | none |
//! | [`keys`] | signing, verifying, encrypting to members | randomness only when generating keys |
//! | [`passphrase`] | hashing and checking passphrases | randomness only when hashing |
//! | [`backup`] | encrypted identity backups | randomness only when sealing |
//! | [`identity`] | who holds a handle on a commons; recovery keys and key rotation | none |
//! | [`publishing`] | signing a profile version | randomness only for members-only encryption |
//! | [`federation`] | building and verifying the share sent to a commons | none |
//! | [`role`] | deciding whether a data folder is a node or a commons | reads/writes `instance.json` in one wrapper |
//! | [`config`] | reading settings from the environment | reads env in one wrapper |
//! | [`outbound`] | which addresses this server may contact for a member | DNS lookup in one wrapper |
//! | [`store`] | reading and writing the data folder | filesystem |
//! | [`ratelimit`] | at most N requests per minute per sender | in-memory state |
//! | [`sessions`] | signed-in sessions and their cookies | in-memory state |
//! | [`form`] | parsing form posts | none |
//! | [`web`] | HTML pages (pure rendering) and the request handlers | handlers only |
//! | [`api`] | JSON and ActivityStreams endpoints | handlers only |
//! | [`app`] | shared state and the route table | none |
//!
//! `src/main.rs` only reads the config, opens the store and starts the server.
//! See `docs/CODEMAP.md` for the path a request takes through these modules.

pub mod api;
pub mod app;
pub mod backup;
pub mod canonical;
pub mod config;
pub mod envelope;
pub mod federation;
pub mod form;
pub mod governance;
pub mod handle;
pub mod identity;
pub mod jcs;
pub mod keys;
pub mod model;
pub mod outbound;
pub mod passphrase;
pub mod publishing;
pub mod ratelimit;
pub mod role;
pub mod sessions;
pub mod store;
pub mod web;

/// Seconds since the Unix epoch. The one place the server reads the clock, so
/// pure functions take a timestamp as an argument instead of calling this.
pub fn now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
