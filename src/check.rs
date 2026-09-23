//! One self-contained status card. It knows nothing about Docker or nodes;
//! it only renders whatever `CheckState` it is handed.

use leptos::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum CheckState {
    /// Still working. `note` says what we're waiting on.
    Pending { note: String },
    /// Done. `detail` is the one-line proof ("Version 4.38 · running").
    Ok { detail: String },
    /// Failed. Names the thing that failed and offers exactly one action.
    Failed { reason: String, action: &'static str },
}

impl CheckState {
    pub fn pending(note: impl Into<String>) -> Self {
        Self::Pending { note: note.into() }
    }
    pub fn ok(detail: impl Into<String>) -> Self {
        Self::Ok { detail: detail.into() }
    }
    pub fn failed(reason: impl Into<String>, action: &'static str) -> Self {
        Self::Failed { reason: reason.into(), action }
    }
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok { .. })
    }
}

/// Title for each state, so the card reads naturally in all three.
#[derive(Clone, Copy)]
pub struct Titles {
    pub pending: &'static str,
    pub ok: &'static str,
    pub failed: &'static str,
}

#[component]
pub fn Check(
    titles: Titles,
    state: RwSignal<CheckState>,
    /// Called when the person presses the failure action (usually "Try again").
    on_action: Callback<()>,
    /// Show a filled "live" dot instead of a tick once ok.
    #[prop(optional)]
    live: bool,
    /// Render the ok detail in monospace (addresses, image names).
    #[prop(optional)]
    mono_detail: bool,
    /// Command shown under the card once it succeeds.
    #[prop(optional)]
    command: Option<&'static str>,
    /// Optional secondary button in the top-right corner.
    #[prop(optional)]
    on_view_log: Option<Callback<()>>,
) -> impl IntoView {
    let kind = move || match state.get() {
        CheckState::Pending { .. } => "pending",
        CheckState::Ok { .. } => "ok",
        CheckState::Failed { .. } => "failed",
    };

    let title = move || match state.get() {
        CheckState::Pending { .. } => titles.pending,
        CheckState::Ok { .. } => titles.ok,
        CheckState::Failed { .. } => titles.failed,
    };

    let body = move || match state.get() {
        CheckState::Pending { note } => view! { <p class="check-detail">{note}</p> }.into_any(),
        CheckState::Ok { detail } => view! {
            <p class="check-detail" class:mono=mono_detail>{detail}</p>
        }
        .into_any(),
        CheckState::Failed { reason, action } => view! {
            <p class="check-detail">{reason}</p>
            <button class="check-action" on:click=move |_| on_action.run(())>{action}</button>
        }
        .into_any(),
    };

    view! {
        <section class="check" data-state=kind aria-live="polite">
            <div class="check-row">
                <StatusIcon state=state live=live />
                <div class="check-text">
                    <h2 class="check-title">{title}</h2>
                    {body}
                </div>
                {on_view_log.map(|cb| view! {
                    <button class="secondary" on:click=move |_| cb.run(())>"View log"</button>
                })}
            </div>
            {move || command.filter(|_| state.with(CheckState::is_ok)).map(|cmd| view! {
                <pre class="command"><code>{cmd}</code></pre>
            })}
        </section>
    }
}

#[component]
fn StatusIcon(state: RwSignal<CheckState>, live: bool) -> impl IntoView {
    move || match state.get() {
        CheckState::Pending { .. } => view! {
            <span class="icon spinner" role="img" aria-label="In progress"></span>
        }
        .into_any(),
        CheckState::Ok { .. } if live => view! {
            <span class="icon live-dot" role="img" aria-label="Running"></span>
        }
        .into_any(),
        CheckState::Ok { .. } => view! {
            <svg class="icon" viewBox="0 0 24 24" role="img" aria-label="Done">
                <circle cx="12" cy="12" r="10.5" fill="none" stroke="currentColor" stroke-width="1.5" />
                <path d="M7.5 12.3l3 3 6-6.3" fill="none" stroke="currentColor" stroke-width="1.6"
                    stroke-linecap="round" stroke-linejoin="round" />
            </svg>
        }
        .into_any(),
        CheckState::Failed { .. } => view! {
            <svg class="icon" viewBox="0 0 24 24" role="img" aria-label="Failed">
                <circle cx="12" cy="12" r="10.5" fill="none" stroke="currentColor" stroke-width="1.5" />
                <path d="M8.5 8.5l7 7M15.5 8.5l-7 7" fill="none" stroke="currentColor" stroke-width="1.6"
                    stroke-linecap="round" />
            </svg>
        }
        .into_any(),
    }
}