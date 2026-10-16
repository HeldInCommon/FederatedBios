//! The three checks. Each is an independent async fn returning a `CheckState`,
//! so each card resolves (or fails) on its own.
//!
//! `docker()` and `image()` call Rust commands in the Tauri shell, which shell
//! out to Docker. In a plain browser tab (`trunk serve`) there is no shell, so
//! they report as skipped and you start the container yourself with
//! `RUN_COMMAND`. `node()` polls the node's health endpoint over HTTP either way.

use crate::check::CheckState;
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::prelude::*;

pub const NODE_ADDR: &str = "localhost:4321";
pub const RUN_COMMAND: &str =
    "docker run -d --name hic-node -p 4321:4321 -v hic-node-data:/data --restart unless-stopped heldincommon/node";

#[wasm_bindgen]
extern "C" {
    // Needs `"withGlobalTauri": true` under `app` in tauri.conf.json.
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], catch)]
    async fn invoke(cmd: &str) -> Result<JsValue, JsValue>;
}

/// True inside the Tauri webview, false in an ordinary browser tab.
fn in_tauri() -> bool {
    js_sys::Reflect::has(&js_sys::global(), &JsValue::from_str("__TAURI__")).unwrap_or(false)
}

/// Shown on the Docker cards when running in a browser tab.
const BROWSER_SKIP: &str = "Skipped in the browser · start the container yourself";

/// Runs a Tauri command that returns `Result<String, String>`.
async fn call(cmd: &str) -> Result<String, String> {
    match invoke(cmd).await {
        Ok(v) => Ok(v.as_string().unwrap_or_default()),
        Err(e) => Err(e.as_string().unwrap_or_else(|| format!("{e:?}"))),
    }
}

pub async fn docker() -> CheckState {
    if !in_tauri() {
        return CheckState::ok(BROWSER_SKIP);
    }
    match call("docker_status").await {
        Ok(version) => CheckState::ok(format!("Version {version} · running")),
        Err(reason) => CheckState::failed(reason, "Try again"),
    }
}

pub async fn image() -> CheckState {
    if !in_tauri() {
        return CheckState::ok(BROWSER_SKIP);
    }
    match call("start_node").await {
        Ok(detail) => CheckState::ok(detail),
        Err(reason) => CheckState::failed(reason, "Try again"),
    }
}

/// Last lines of the container log, for the "View log" button.
pub async fn node_log() -> String {
    if !in_tauri() {
        return "Logs need the desktop app. In a terminal: docker logs hic-node".into();
    }
    call("node_log").await.unwrap_or_else(|e| e)
}

/// Polls `GET http://localhost:4321/health` until it answers or we give up.
/// `on_attempt` lets the card show progress while waiting. It returns `false`
/// when the caller no longer wants the result (a newer run started, or the page
/// was left), in which case polling stops and this returns `None`.
///
/// The node must send `Access-Control-Allow-Origin` for the page's origin,
/// otherwise the browser will block the response.
pub async fn node(max_attempts: u32, on_attempt: impl Fn(u32) -> bool) -> Option<CheckState> {
    let url = format!("http://{NODE_ADDR}/health");
    let mut last_error = String::new();

    for attempt in 1..=max_attempts {
        if !on_attempt(attempt) {
            return None;
        }
        match Request::get(&url).send().await {
            Ok(resp) if resp.ok() => return Some(CheckState::ok(NODE_ADDR)),
            Ok(resp) => last_error = format!("The node answered with HTTP {}.", resp.status()),
            Err(_) => last_error = format!("Nothing is answering on {NODE_ADDR}."),
        }
        TimeoutFuture::new(1500).await;
    }

    Some(CheckState::failed(last_error, "Try again"))
}
