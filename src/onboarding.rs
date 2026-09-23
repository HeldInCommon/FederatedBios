//! Step 1 of setup: get a local node running before asking anything about a profile.

use crate::check::{Check, CheckState, Titles};
use crate::probes::{self, NODE_ADDR, RUN_COMMAND};
use leptos::prelude::*;
use leptos::task::spawn_local;

const NODE_ATTEMPTS: u32 = 20;

#[component]
pub fn Onboarding() -> impl IntoView {
    let docker = RwSignal::new(CheckState::pending("Looking for Docker Desktop on this machine"));
    let image = RwSignal::new(CheckState::pending("Waiting for Docker"));
    let node = RwSignal::new(CheckState::pending(format!("Waiting for {NODE_ADDR}")));

    // Each runner resets its own card and re-probes. None depends on another.
    let run_docker = move || {
        docker.set(CheckState::pending("Looking for Docker Desktop on this machine"));
        spawn_local(async move { docker.set(probes::docker().await) });
    };
    let run_image = move || {
        image.set(CheckState::pending("Pulling heldincommon/node"));
        spawn_local(async move { image.set(probes::image().await) });
    };
    let run_node = move || {
        spawn_local(async move {
            let result = probes::node(NODE_ATTEMPTS, move |n| {
                node.set(CheckState::pending(format!(
                    "Waiting for {NODE_ADDR} (attempt {n} of {NODE_ATTEMPTS})"
                )));
            })
            .await;
            node.set(result);
        });
    };

    // Kick everything off once, on mount.
    run_docker();
    run_image();
    run_node();

    // The only gate: nothing about a profile until the node answers.
    let node_ready = move || node.with(CheckState::is_ok);

    view! {
        <main class="page">
            <Stepper current=1 />

            <header class="intro">
                <h1>"Run your node"</h1>
                <p class="lede">
                    "Your profile lives on a small service running on this machine. "
                    "Held in Common reads it. It never holds it."
                </p>
            </header>

            <div class="checks">
                <Check
                    titles=Titles {
                        pending: "Looking for Docker Desktop",
                        ok: "Docker Desktop found",
                        failed: "Docker Desktop is not running",
                    }
                    state=docker
                    on_action=Callback::new(move |_| run_docker())
                />
                <Check
                    titles=Titles {
                        pending: "Pulling the node image",
                        ok: "Node image pulled and started",
                        failed: "Node image did not start",
                    }
                    state=image
                    on_action=Callback::new(move |_| run_image())
                    command=RUN_COMMAND
                    on_view_log=Callback::new(|_| leptos::logging::log!("TODO: open container log"))
                />
                <Check
                    titles=Titles {
                        pending: "Starting your node",
                        ok: "Your node is running",
                        failed: "Your node is not answering",
                    }
                    state=node
                    on_action=Callback::new(move |_| run_node())
                    live=true
                    mono_detail=true
                />
            </div>

            <footer class="actions">
                <a
                    class="primary"
                    href="#/profile"
                    class:disabled=move || !node_ready()
                    aria-disabled=move || (!node_ready()).to_string()
                    on:click=move |ev| if !node_ready() { ev.prevent_default() }
                >
                    "Set up your profile"
                </a>
                <a class="help-link" href="#/help/docker">"Docker will not start"</a>
            </footer>
        </main>
    }
}

#[component]
fn Stepper(current: u8) -> impl IntoView {
    const STEPS: [&str; 4] = ["Install", "Profile", "Visibility", "Publish"];

    view! {
        <nav class="stepper" aria-label="Setup progress">
            <ol>
                {STEPS
                    .iter()
                    .enumerate()
                    .map(|(i, name)| {
                        let n = i as u8 + 1;
                        let is_current = n == current;
                        view! {
                            <li
                                class:current=is_current
                                aria-current=if is_current { Some("step") } else { None }
                            >
                                <span class="step-num">{n}</span>
                                " · "
                                {*name}
                            </li>
                        }
                    })
                    .collect_view()}
            </ol>
        </nav>
    }
}