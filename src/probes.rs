//! The three checks. Each is an independent async fn returning a `CheckState`,
//! so each card resolves (or fails) on its own.
//!
//! A browser tab cannot see Docker, so `docker()` and `image()` are stubs here.
//! In a Tauri build, replace their bodies with `invoke("docker_status")` etc.
//! `node()` is real: it polls the node's health endpoint over HTTP.

use crate::check::CheckState;
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;

pub const NODE_ADDR: &str = "localhost:4321";
pub const RUN_COMMAND: &str = "docker run -d -p 4321:4321 heldincommon/node";

/// Flip to see the failure states without breaking your real setup.
const SIMULATE_DOCKER_FAILURE: bool = false;

pub async fn docker() -> CheckState {
    // TODO(tauri): run `docker version --format '{{.Server.Version}}'` host-side.
    TimeoutFuture::new(700).await;
    if SIMULATE_DOCKER_FAILURE {
        CheckState::failed("Docker Desktop is installed but not running.", "Try again")
    } else {
        CheckState::ok("Version 4.38 · running")
    }
}

pub async fn image() -> CheckState {
    // TODO(tauri): `docker pull heldincommon/node` then `docker run …`, streaming the log.
    TimeoutFuture::new(1600).await;
    if SIMULATE_DOCKER_FAILURE {
        CheckState::failed("The node image could not be pulled because Docker is not running.", "Try again")
    } else {
        CheckState::ok("heldincommon/node · 41 MB")
    }
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