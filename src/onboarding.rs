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
    let node = RwSignal::new(CheckState::pending("Starts once the node image is running"));

    // Bumped on every node run. A poll whose number is stale (or whose
    // counter has been disposed because the page was left) stops itself.
    let node_run = StoredValue::new(0u32);

    let run_docker = move || {
        docker.set(CheckState::pending("Looking for Docker Desktop on this machine"));
        spawn_local(async move { docker.set(probes::docker().await) });
    };
    let run_image = move || {
        image.set(CheckState::pending("Pulling heldincommon/node"));
        spawn_local(async move { image.set(probes::image().await) });
    };
    let run_node = move || {
        node_run.update_value(|n| *n += 1);
        let this_run = node_run.get_value();
        let still_current = move || node_run.try_get_value() == Some(this_run);

        spawn_local(async move {
            let result = probes::node(NODE_ATTEMPTS, move |n| {
                if !still_current() {
                    return false;
                }
                node.set(CheckState::pending(format!(
                    "Waiting for {NODE_ADDR} (attempt {n} of {NODE_ATTEMPTS})"
                )));
                true
            })
            .await;
            if let Some(result) = result.filter(|_| still_current()) {
                node.set(result);
            }
        });
    };

    // Docker and the image are checked straight away. The node is only polled
    // once the image reports it has started; before that there is nothing on
    // localhost:4321 to answer. If the image check is retried, any poll in
    // flight is cancelled and the node card goes back to waiting.
    run_docker();
    run_image();
    Effect::new(move |_| {
        if image.with(CheckState::is_ok) {
            run_node();
        } else {
            node_run.update_value(|n| *n += 1);
            node.set(CheckState::pending("Starts once the node image is running"));
        }
    });

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
                <a class="help-link" href="#/profile" class:hidden=node_ready>
                    "Skip for now"
                </a>
                <a class="help-link" href="#/help/docker">"Docker will not start"</a>
            </footer>
        </main>
    }
}

#[component]
pub fn Stepper(current: u8) -> impl IntoView {
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