//! Step 2 placeholder. Reachable either through the gated "Set up your profile"
//! button or the "Skip for now" link, so it must not assume a node is running.

use crate::onboarding::Stepper;
use leptos::prelude::*;

#[component]
pub fn Profile() -> impl IntoView {
    view! {
        <main class="page">
            <Stepper current=2 />

            <header class="intro">
                <h1>"Set up your profile"</h1>
                <p class="lede">
                    "Profile editing is not built yet. Nothing on this page talks to your node, "
                    "so you can work on it before the node is running."
                </p>
            </header>

            <footer class="actions">
                <a class="help-link" href="#/">"Back to running your node"</a>
            </footer>
        </main>
    }
}
