mod docker;

// Each command runs on a blocking thread: the Docker CLI can take seconds
// (a pull, much longer) and must not stall the async runtime.
async fn blocking(f: fn() -> Result<String, String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("Internal error: {e}"))?
}

#[tauri::command]
async fn docker_status() -> Result<String, String> {
    blocking(docker::status).await
}

#[tauri::command]
async fn start_node() -> Result<String, String> {
    blocking(docker::start_node).await
}

#[tauri::command]
async fn node_log() -> Result<String, String> {
    blocking(docker::logs).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![docker_status, start_node, node_log])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
