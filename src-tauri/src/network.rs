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

/// Classify a `/health` reply. `Ours` only when it is `hic-commons` and
/// echoes `token`.
pub fn classify_health(status_ok: bool, body: &str, token: &str) -> Probe {
    if !status_ok {
        return Probe::Other;
    }
    let Ok(j) = serde_json::from_str::<serde_json::Value>(body) else { return Probe::Other };
    let ours = j.get("service").and_then(|v| v.as_str()) == Some("hic-commons")
        && j.get("launch").and_then(|v| v.as_str()) == Some(token);
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
    let req = "GET /health HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    if s.write_all(req.as_bytes()).is_err() {
        return Probe::Other;
    }
    let mut raw = String::new();
    // A short or failed read still leaves whatever arrived to classify.
    let _ = s.take(64 * 1024).read_to_string(&mut raw);
    let (ok, body) = split_http(&raw);
    classify_health(ok, body, token)
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

/// A token identifying this launch of the app. It only needs to differ
/// between launches (it tells our server apart from others); it is not a
/// secret, since anyone who can reach `/health` can read it.
pub fn launch_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:x}-{:x}", nanos, std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_server_is_recognised() {
        let body = r#"{"status":"ok","service":"hic-commons","launch":"abc"}"#;
        assert_eq!(classify_health(true, body, "abc"), Probe::Ours);
    }

    #[test]
    fn another_hic_server_is_not_ours() {
        let body = r#"{"status":"ok","service":"hic-commons","launch":"old"}"#;
        assert_eq!(classify_health(true, body, "abc"), Probe::Other);
        let no_token = r#"{"status":"ok","service":"hic-commons"}"#;
        assert_eq!(classify_health(true, no_token, "abc"), Probe::Other);
    }

    #[test]
    fn other_programs_are_not_ours() {
        assert_eq!(classify_health(true, "<html>dev server</html>", "abc"), Probe::Other);
        assert_eq!(classify_health(false, r#"{"service":"hic-commons","launch":"abc"}"#, "abc"), Probe::Other);
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
    fn launch_tokens_differ() {
        let a = launch_token();
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert_ne!(a, launch_token());
    }
}
