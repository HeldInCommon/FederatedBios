//! Docker commands for the onboarding checks. The webview can't run Docker,
//! so the UI calls these through `invoke` and they shell out to the CLI.

use std::process::{Command, Output};

const IMAGE: &str = "heldincommon/node";
const CONTAINER: &str = "hic-node";
const VOLUME: &str = "hic-node-data";
const PORT: &str = "4321:4321";

/// Apps launched from the macOS Finder get a minimal PATH, so `docker` alone
/// often isn't found. Try PATH first, then the usual install locations.
fn docker_bin() -> Option<&'static str> {
    ["docker", "/usr/local/bin/docker", "/opt/homebrew/bin/docker", "/Applications/Docker.app/Contents/Resources/bin/docker"]
        .into_iter()
        .find(|bin| Command::new(bin).arg("--version").output().is_ok_and(|o| o.status.success()))
}

fn docker(args: &[&str]) -> Result<Output, String> {
    let bin = docker_bin().ok_or("Docker Desktop is not installed on this machine.")?;
    Command::new(bin).args(args).output().map_err(|e| format!("Could not run Docker: {e}"))
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).trim().to_string()
}

/// Server version if the daemon is up. The client answers even when the daemon
/// is down, which is how "installed but not running" is told apart.
pub fn status() -> Result<String, String> {
    let out = docker(&["version", "--format", "{{.Server.Version}}"])?;
    let version = stdout(&out);
    if out.status.success() && !version.is_empty() {
        Ok(version)
    } else {
        Err("Docker Desktop is installed but not running.".into())
    }
}

/// Makes sure the image is present and the container is running.
/// Safe to call again: it reuses an existing container rather than failing
/// on the name or the port.
pub fn start_node() -> Result<String, String> {
    status()?;

    let have_image = docker(&["image", "inspect", IMAGE])?.status.success();
    if !have_image {
        let pull = docker(&["pull", IMAGE])?;
        if !pull.status.success() {
            return Err(format!("The node image could not be pulled: {}", stderr(&pull)));
        }
    }

    let running = docker(&["inspect", "--format", "{{.State.Running}}", CONTAINER])?;
    match (running.status.success(), stdout(&running).as_str()) {
        (true, "true") => {}
        (true, _) => {
            let start = docker(&["start", CONTAINER])?;
            if !start.status.success() {
                return Err(format!("The node container would not start: {}", stderr(&start)));
            }
        }
        (false, _) => {
            let volume = format!("{VOLUME}:/data");
            let run = docker(&[
                "run", "-d", "--name", CONTAINER, "-p", PORT, "-v", &volume,
                "--restart", "unless-stopped", IMAGE,
            ])?;
            if !run.status.success() {
                return Err(format!("The node container would not start: {}", stderr(&run)));
            }
        }
    }

    let size = docker(&["image", "inspect", "--format", "{{.Size}}", IMAGE])
        .ok()
        .and_then(|o| stdout(&o).parse::<u64>().ok())
        .map(|bytes| format!(" · {} MB", bytes / 1_000_000))
        .unwrap_or_default();
    Ok(format!("{IMAGE}{size}"))
}

/// The last 200 lines of the container's output (Docker splits it across
/// stdout and stderr, so both are returned).
pub fn logs() -> Result<String, String> {
    let out = docker(&["logs", "--tail", "200", CONTAINER])?;
    if out.status.success() {
        Ok(format!("{}\n{}", stdout(&out), stderr(&out)).trim().to_string())
    } else {
        Err(format!("No log yet: {}", stderr(&out)))
    }
}
