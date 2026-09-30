mod app;
mod check;
mod onboarding;
mod probes;
mod profile;
mod visibility;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
