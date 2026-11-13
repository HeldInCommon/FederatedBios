//! The server stops when the desktop app does (decision record 0027).
//!
//! The app starts the server with `HIC_EXIT_WITH_APP=1` and holds the writing
//! end of its stdin. When the app ends for any reason (a quit, a crash, a
//! force-quit) the operating system closes that end, the server reads "end of
//! file", and stops, so a server open to the room never outlives the app.
//!
//! These tests start the real binary (`CARGO_BIN_EXE_hic-commons`, which
//! Cargo builds for integration tests) and play the app's part.

use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A port nobody is using right now: ask the system for one, then let it go.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Start the server as a node on `port` with a fresh data folder and a piped
/// stdin, with `HIC_EXIT_WITH_APP` set when `exit_with_app` is true.
fn start(port: u16, exit_with_app: bool) -> Child {
    let dir = std::env::temp_dir().join(format!("hic-exit-{}-{port}", std::process::id()));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hic-commons"));
    cmd.env("HIC_DATA", &dir)
        .env("PORT", port.to_string())
        .env("HIC_ROLE", "node")
        .env_remove("HIC_EXIT_WITH_APP")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if exit_with_app {
        cmd.env("HIC_EXIT_WITH_APP", "1");
    }
    cmd.spawn().expect("the server binary starts")
}

/// True once something answers on `port` within `limit`.
fn answers_within(port: u16, limit: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// The child's exit status if it stops within `limit`, or `None`.
fn exits_within(child: &mut Child, limit: Duration) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

#[test]
fn the_server_stops_when_the_app_is_gone() {
    let port = free_port();
    let mut server = start(port, true);
    assert!(answers_within(port, Duration::from_secs(20)), "the server came up");

    // The app ending closes its end of the pipe; dropping it here does the same.
    drop(server.stdin.take());

    let status = exits_within(&mut server, Duration::from_secs(2));
    assert!(status.is_some(), "the server stopped within 2 seconds");
    assert!(!answers_within(port, Duration::from_millis(300)), "nothing listens on the port any more");
}

#[test]
fn without_the_setting_closing_stdin_changes_nothing() {
    // Docker and people running the server by hand often give it no stdin
    // at all; it must keep running there.
    let port = free_port();
    let mut server = start(port, false);
    assert!(answers_within(port, Duration::from_secs(20)), "the server came up");
    drop(server.stdin.take());

    let status = exits_within(&mut server, Duration::from_millis(1500));
    let still_running = status.is_none();
    server.kill().ok();
    server.wait().ok();
    assert!(still_running, "the server kept running");
}
