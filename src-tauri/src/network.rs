//! Local networking: is our server answering, and what is this computer's
//! address on the local network?
//!
//! The app must only ever show *its own* servers. Another program might be
//! using port 8080 (a development server, a Docker container, an old copy of
//! the app), and if the window showed that instead, someone could type their
//! passphrase into it. So instead of "is the port open?", the app asks
//! `/health` and checks two things: the service is `hic-commons`, and it echoes
//! the launch token this app gave its server (see [`classify_health`]).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// The address the window opens for the server on `port`. 127.0.0.1 rather
/// than "localhost": a node listens only on IPv4 loopback, and "localhost"
/// may resolve to IPv6 first.
pub fn local_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// The port of a page on one of this computer's servers (`127.0.0.1` or
/// `localhost`), or `None` for anywhere else.
pub fn local_port(url: &str) -> Option<u16> {
    let rest = url.strip_prefix("http://")?;
    let host_port = rest.split(['/', '?', '#']).next()?;
    let (host, port) = host_port.rsplit_once(':')?;
    if host != "127.0.0.1" && host != "localhost" {
        return None;
    }
    port.parse().ok()
}

/// What is answering on a port.
#[derive(Debug, PartialEq, Eq)]
pub enum Probe {
    /// Nothing is listening yet.
    Nothing,
    /// Our server, started by this launch of the app.
    Ours,
    /// Something else: another program, or a server this app didn't start.
    Other,
}

/// Classify a `/health` reply to a probe that sent our launch token. `Ours`
/// only when it is `hic-commons` and says `"ours": true` (the server checked
/// the token; it never sends it back, ADR 0023).
pub fn classify_health(status_ok: bool, body: &str) -> Probe {
    if !status_ok {
        return Probe::Other;
    }
    let Ok(j) = serde_json::from_str::<serde_json::Value>(body) else { return Probe::Other };
    let ours = j.get("service").and_then(|v| v.as_str()) == Some("hic-commons")
        && j.get("ours").and_then(|v| v.as_bool()) == Some(true);
    if ours {
        Probe::Ours
    } else {
        Probe::Other
    }
}

/// Split a raw HTTP/1.x response into (status is 2xx, body).
pub fn split_http(raw: &str) -> (bool, &str) {
    let ok = raw.split_whitespace().nth(1).is_some_and(|code| code.starts_with('2'));
    let body = raw.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
    (ok, body)
}

/// Ask whatever is on `port` for `/health` and classify it.
pub fn probe(port: u16, token: &str) -> Probe {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else { return Probe::Nothing };
    let _ = s.set_read_timeout(Some(Duration::from_millis(800)));
    let req = format!("GET /health HTTP/1.0\r\nHost: 127.0.0.1\r\nX-HIC-Launch: {token}\r\nConnection: close\r\n\r\n");
    if s.write_all(req.as_bytes()).is_err() {
        return Probe::Other;
    }
    let mut raw = String::new();
    // A short or failed read still leaves whatever arrived to classify.
    let _ = s.take(64 * 1024).read_to_string(&mut raw);
    let (ok, body) = split_http(&raw);
    classify_health(ok, body)
}

/// The texts in the node's answer to `POST /api/notices/claim` (a JSON list
/// of notices). Anything unreadable gives none.
pub fn parse_notices(body: &str) -> Vec<String> {
    serde_json::from_str::<Vec<serde_json::Value>>(body)
        .unwrap_or_default()
        .iter()
        .filter_map(|n| n.get("text").and_then(|t| t.as_str()).map(String::from))
        .collect()
}

/// Collect the node's news that hasn't been shown as a notification yet (the
/// node marks it as shown). Proves it's the app with the launch token. Only
/// talks to this computer.
pub fn claim_notices(port: u16, token: &str) -> Vec<String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else { return Vec::new() };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let req = format!(
        "POST /api/notices/claim HTTP/1.0\r\nHost: 127.0.0.1\r\nX-HIC-Launch: {token}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    if s.write_all(req.as_bytes()).is_err() {
        return Vec::new();
    }
    let mut raw = String::new();
    let _ = s.take(256 * 1024).read_to_string(&mut raw);
    match split_http(&raw) {
        (true, body) => parse_notices(body),
        _ => Vec::new(),
    }
}

/// This computer's address on the local network, for a commons on `port`
/// that other people in the room share to. Connecting a UDP socket sends
/// nothing; it only asks the operating system which interface it would use.
pub fn lan_url(port: u16) -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let ip = sock.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    Some(format!("http://{ip}:{port}"))
}

/// Where the Tailscale command-line tool may be: on the `PATH` (Linux,
/// Windows, Homebrew), or inside the Mac app, which doesn't put it there.
const TAILSCALE_PROGRAMS: [&str; 2] = ["tailscale", "/Applications/Tailscale.app/Contents/MacOS/Tailscale"];

/// What `tailscale serve status` printed, if Tailscale is installed.
#[derive(Debug, PartialEq, Eq)]
pub enum TailscaleStatus {
    /// Tailscale isn't installed here (or not where we look).
    NotInstalled,
    /// What it printed: which ports Tailscale (Serve or Funnel) forwards to.
    /// Empty when it isn't running or forwards nothing.
    Answered(String),
    /// It's installed but didn't answer within three seconds.
    NoAnswer,
}

/// A command for `program` that runs without a window: on Windows, a
/// command-line program started from an app flashes a console window
/// unless told not to (`CREATE_NO_WINDOW`). Elsewhere, an ordinary command.
pub fn quiet_command(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW, from the Windows process creation flags.
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// What a finished `tailscale serve status` means: its answer when it
/// succeeded; "forwards nothing" when it failed because Tailscale isn't
/// running (then nothing can be forwarded); otherwise no answer, since a
/// failure we don't recognise could hide a Funnel that's on. Pure.
pub fn tailscale_outcome(succeeded: bool, out: String, err: &str) -> TailscaleStatus {
    let err = err.to_lowercase();
    let not_running = err.contains("tailscaled") && (err.contains("not running") || err.contains("failed to connect"));
    match (succeeded, not_running) {
        (true, _) => TailscaleStatus::Answered(out),
        (false, true) => TailscaleStatus::Answered(String::new()),
        (false, false) => TailscaleStatus::NoAnswer,
    }
}

/// Ask `tailscale serve status` which ports Tailscale (Serve or Funnel)
/// forwards to on this computer, waiting at most three seconds; a Tailscale
/// that doesn't answer is stopped. Only reads Tailscale's settings; changes
/// nothing (decision record 0027, D9; see [`crate::room::forwards_to`]).
pub fn tailscale_serve_status() -> TailscaleStatus {
    for program in TAILSCALE_PROGRAMS {
        let Ok(mut child) = quiet_command(program)
            .args(["serve", "status"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        else {
            continue;
        };
        // Wait in small steps rather than forever, so a stuck Tailscale
        // can't freeze the app. Its answer is a few lines, which fit in the
        // pipe while it finishes.
        for _ in 0..30 {
            if let Ok(Some(status)) = child.try_wait() {
                let (mut out, mut err) = (String::new(), String::new());
                if let Some(mut stdout) = child.stdout.take() {
                    let _ = stdout.read_to_string(&mut out);
                }
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut err);
                }
                return tailscale_outcome(status.success(), out, &err);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
        return TailscaleStatus::NoAnswer;
    }
    TailscaleStatus::NotInstalled
}

/// A random token for this launch of the app: 32 bytes from the operating
/// system, as hex. It is a **secret**: it tells our servers apart from others,
/// collects the node's notices, and opens a commons' steward view (ADR 0023).
/// Only this app and the servers it starts know it.
///
/// The bytes come from `OsRng`, the operating system's secure random source
/// on every platform (macOS, Linux and Windows), the same source the server
/// uses for session tokens. Returns an error if that source fails, which
/// means the system is broken: the app can't make a secret, so it doesn't
/// start (see `app_config`).
pub fn launch_token() -> Result<String, rand::Error> {
    // `RngCore` is the trait (an interface, §2 of the house style) that gives
    // `OsRng` its `try_fill_bytes` method; it has to be in scope to call it.
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.try_fill_bytes(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_tailscale_check_counts_as_no_answer_unless_tailscale_is_off() {
        let off = "failed to connect to local tailscaled; it doesn't appear to be running";
        assert_eq!(tailscale_outcome(false, String::new(), off), TailscaleStatus::Answered(String::new()));
        assert_eq!(tailscale_outcome(false, String::new(), "permission denied"), TailscaleStatus::NoAnswer);
        assert_eq!(tailscale_outcome(true, "x".into(), ""), TailscaleStatus::Answered("x".into()));
    }

    #[test]
    fn our_server_is_recognised() {
        let body = r#"{"status":"ok","service":"hic-commons","ours":true}"#;
        assert_eq!(classify_health(true, body), Probe::Ours);
    }

    #[test]
    fn another_hic_server_is_not_ours() {
        let not_ours = r#"{"status":"ok","service":"hic-commons"}"#;
        assert_eq!(classify_health(true, not_ours), Probe::Other);
    }

    #[test]
    fn other_programs_are_not_ours() {
        assert_eq!(classify_health(true, "<html>dev server</html>"), Probe::Other);
        assert_eq!(classify_health(true, r#"{"service":"other","ours":true}"#), Probe::Other);
        assert_eq!(classify_health(false, r#"{"service":"hic-commons","ours":true}"#), Probe::Other);
    }

    #[test]
    fn http_split() {
        let (ok, body) = split_http("HTTP/1.1 200 OK\r\ncontent-type: x\r\n\r\n{\"a\":1}");
        assert!(ok);
        assert_eq!(body, "{\"a\":1}");
        assert!(!split_http("HTTP/1.1 404 Not Found\r\n\r\n").0);
        assert!(!split_http("").0);
    }

    #[test]
    fn notice_texts_are_read_from_the_list() {
        let body = r#"[{"handle":"ada","text":"@ada is now listed on c:8081.","at":1}]"#;
        assert_eq!(parse_notices(body), vec!["@ada is now listed on c:8081.".to_string()]);
        assert!(parse_notices("not json").is_empty());
    }

    #[test]
    fn local_pages_are_recognised_by_port() {
        assert_eq!(local_port("http://127.0.0.1:8080/@tega"), Some(8080));
        assert_eq!(local_port("http://localhost:8082"), Some(8082));
        assert_eq!(local_port("http://127.0.0.1:8081?x=1"), Some(8081));
        assert_eq!(local_port("http://192.168.1.5:8081/"), None, "a room commons seen from outside");
        assert_eq!(local_port("https://commons.example/"), None);
        assert_eq!(local_port("http://127.0.0.1.evil.example:8080/"), None);
        assert_eq!(local_port("tauri://localhost/index.html"), None);
    }

    #[test]
    fn launch_tokens_are_random_and_url_safe() {
        let a = launch_token().expect("the test machine has a random source");
        let b = launch_token().expect("the test machine has a random source");
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "a token is never reused");
    }
}
