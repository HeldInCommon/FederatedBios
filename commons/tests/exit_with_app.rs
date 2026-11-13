//! The server stops when the desktop app does (decision record 0027).
//!
//! The app starts the server with `HIC_EXIT_WITH_APP=1` and holds the other
//! ends of its stdin, stdout and stderr. When the app ends for any reason (a
//! quit, a crash, a force-quit) the operating system closes all three, the
//! server reads "end of file" on stdin, and stops, so a server open to the
//! room never outlives the app.
//!
//! These tests start the real binary (`CARGO_BIN_EXE_hic-commons`, which
//! Cargo builds for integration tests) and play the app's part, closing all
//! three pipes as a dying app does.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A port nobody is using right now: ask the system for one, then let it go.
/// Another program could take it before the server does, so the tests check
/// for this server's own reply, not just any listener.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// A server started for a test. Dropping it kills the process (if it's still
/// running) and deletes its data folder, so a failed assert leaves nothing
/// behind.
struct Server {
    /// The running process, with its stdin, stdout and stderr piped.
    child: Child,
    /// Its data folder.
    dir: PathBuf,
}

impl Drop for Server {
    /// Kill the process if it's still running and delete its data folder.
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// Start the server as a node on `port` with a fresh data folder and all
/// three standard pipes, with `HIC_EXIT_WITH_APP` set when `exit_with_app`
/// is true.
fn start(port: u16, exit_with_app: bool) -> Server {
    let dir = std::env::temp_dir().join(format!("hic-exit-{}-{port}", std::process::id()));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hic-commons"));
    cmd.env("HIC_DATA", &dir)
        .env("PORT", port.to_string())
        .env("HIC_ROLE", "node")
        .env_remove("HIC_EXIT_WITH_APP")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if exit_with_app {
        cmd.env("HIC_EXIT_WITH_APP", "1");
    }
    Server { child: cmd.spawn().expect("the server binary starts"), dir }
}

/// True when this project's server answers `/health` on `port`.
fn ours_answers(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else { return false };
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    if stream.write_all(b"GET /health HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n").is_err() {
        return false;
    }
    let mut reply = String::new();
    stream.read_to_string(&mut reply).ok();
    reply.contains("hic-commons")
}

/// True once this project's server answers on `port` within `limit`.
fn answers_within(port: u16, limit: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if ours_answers(port) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// The process's exit status if it stops within `limit`, or `None`.
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

/// Close the server's stdin, stdout and stderr, as the operating system does
/// when the app that started it ends.
fn the_app_goes_away(server: &mut Server) {
    drop(server.child.stdin.take());
    drop(server.child.stdout.take());
    drop(server.child.stderr.take());
}

#[test]
fn the_server_stops_when_the_app_is_gone() {
    let port = free_port();
    let mut server = start(port, true);
    assert!(answers_within(port, Duration::from_secs(20)), "the server came up");

    the_app_goes_away(&mut server);

    let status = exits_within(&mut server.child, Duration::from_secs(2));
    assert!(status.is_some(), "the server stopped within 2 seconds");
    assert!(!ours_answers(port), "nothing answers on the port any more");
}

#[test]
fn without_the_setting_closing_stdin_changes_nothing() {
    // Docker and people running the server by hand may give it an empty or
    // closed stdin; without the setting it must keep running. (Here stdin is
    // a pipe that closes, which reads the same as an empty one.)
    let port = free_port();
    let mut server = start(port, false);
    assert!(answers_within(port, Duration::from_secs(20)), "the server came up");
    drop(server.child.stdin.take());

    assert!(exits_within(&mut server.child, Duration::from_millis(1500)).is_none(), "the server kept running");
    assert!(ours_answers(port), "and still answers");
}
