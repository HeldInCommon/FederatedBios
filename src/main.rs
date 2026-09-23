mod check;
mod onboarding;
mod probes;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(onboarding::Onboarding);
}