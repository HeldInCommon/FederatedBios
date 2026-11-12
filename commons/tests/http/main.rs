//! In-process HTTP tests: the whole router, a temporary data folder, no
//! network. Each test builds its own server, so they run in parallel.
//!
//! One file per behaviour; shared helpers live in `common.rs`. To add tests
//! for a new area, add a file here and a `mod` line below.
//!
//! Run with `cargo test` in `commons/` (or `cargo test --test http vouching`
//! for one area). The slower, multi-process federation test is
//! `scripts/test-federation.sh`.

mod common;

mod accounts;
mod basics;
mod guide;
mod held_signals;
mod publishing;
mod ramp;
mod sharing;
mod steward;
mod stories;
mod story_edit;
mod vouching;
mod waiting;
