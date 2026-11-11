// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// The program's entry point: set up the Linux web view, then run the app (`lib.rs`).
fn main() {
    // Linux: WebKitGTK (which draws the window) shares GPU buffers with the
    // compositor through DMA-BUF by default. NVIDIA drivers refuse ("Failed
    // to create GBM buffer … Permission denied"), leaving a blank, gray
    // window. The older drawing path works everywhere, so use it unless the
    // person has set this themselves. Set before anything else starts, while
    // the program is still single-threaded, as `set_var` requires.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    hic_onboarding_app_lib::run()
}
