//! Tiny hash router, one route per setup step:
//! `#/profile` (2), `#/visibility` (3), `#/publish` (4), anything else (1).
//! Hash routes keep this working under `trunk serve` and any static host
//! without server-side rewrites.

use crate::onboarding::{Onboarding, Stepper};
use crate::profile::Profile;
use crate::visibility::Visibility;
use leptos::prelude::*;

fn current_hash() -> String {
    window().location().hash().unwrap_or_default()
}

#[component]
pub fn App() -> impl IntoView {
    let route = RwSignal::new(current_hash());
    let listener = window_event_listener(leptos::ev::hashchange, move |_| route.set(current_hash()));
    on_cleanup(move || listener.remove());

    // Leaving a page unmounts it, which disposes its signals. The node poll
    // notices that and stops, so skipping ahead really does stop the probing.
    move || match route.get().as_str() {
        "#/profile" => view! { <Profile /> }.into_any(),
        "#/visibility" => view! { <Visibility /> }.into_any(),
        "#/publish" => view! { <Publish /> }.into_any(),
        _ => view! { <Onboarding /> }.into_any(),
    }
}

/// Step 4 placeholder so "Continue to publish" lands somewhere real.
#[component]
fn Publish() -> impl IntoView {
    view! {
        <main class="page">
            <Stepper current=4 />
            <header class="intro">
                <h1>"Publish"</h1>
                <p class="lede">"Publishing is not built yet."</p>
            </header>
            <footer class="actions">
                <a class="help-link" href="#/visibility">"Back to who can see this"</a>
            </footer>
        </main>
    }
}
