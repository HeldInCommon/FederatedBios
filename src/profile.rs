//! Step 2: who you are. A form on the left, and on the right a live preview of
//! the directory card others will see.
//!
//! Rules from the design:
//! - Pronouns are free text, not a menu.
//! - Each place carries its own precision, so someone can give a region
//!   without a city.
//! - Nothing leaves the machine. The photo is only previewed locally, and the
//!   draft is meant to save to the node.
//!
//! There is no node yet, so "Save a draft" writes to this browser's
//! localStorage as a stand-in. Swap `save_draft` / `load_draft` for calls to
//! the node once it exists.

use crate::onboarding::Stepper;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

const BIO_LIMIT: usize = 500;
const DRAFT_KEY: &str = "hic.profile.draft";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
enum Precision {
    #[default]
    City,
    Region,
    Country,
}

impl Precision {
    const ALL: [Precision; 3] = [Precision::City, Precision::Region, Precision::Country];

    fn label(self) -> &'static str {
        match self {
            Precision::City => "City",
            Precision::Region => "Region",
            Precision::Country => "Country",
        }
    }

    fn from_label(s: &str) -> Self {
        Self::ALL.into_iter().find(|p| p.label() == s).unwrap_or_default()
    }
}

/// What gets saved. The photo is not included: it is a local preview only.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Draft {
    name: String,
    pronouns: String,
    role: String,
    bio: String,
    places: Vec<SavedPlace>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SavedPlace {
    name: String,
    precision: Precision,
}

/// One editable row. Each field is its own signal so typing in one row
/// doesn't re-render the others.
#[derive(Clone, Copy)]
struct PlaceRow {
    id: u32,
    name: RwSignal<String>,
    precision: RwSignal<Precision>,
}

fn storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

// TODO(node): read the draft from the node instead.
fn load_draft() -> Draft {
    storage()
        .and_then(|s| s.get_item(DRAFT_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

// TODO(node): write the draft to the node instead.
fn save_draft(draft: &Draft) -> bool {
    let Ok(json) = serde_json::to_string(draft) else { return false };
    storage().is_some_and(|s| s.set_item(DRAFT_KEY, &json).is_ok())
}

#[component]
pub fn Profile() -> impl IntoView {
    let draft = load_draft();

    let photo = RwSignal::new(None::<String>);
    let name = RwSignal::new(draft.name);
    let pronouns = RwSignal::new(draft.pronouns);
    let role = RwSignal::new(draft.role);
    let bio = RwSignal::new(draft.bio);

    let next_id = StoredValue::new(0u32);
    let new_row = move |place: SavedPlace| {
        next_id.update_value(|n| *n += 1);
        PlaceRow {
            id: next_id.get_value(),
            name: RwSignal::new(place.name),
            precision: RwSignal::new(place.precision),
        }
    };
    let mut initial: Vec<PlaceRow> = draft.places.into_iter().map(new_row).collect();
    if initial.is_empty() {
        initial.push(new_row(SavedPlace::default()));
    }
    let places = RwSignal::new(initial);

    let add_place = move |_| places.update(|v| v.push(new_row(SavedPlace::default())));
    let remove_place = move |id: u32| places.update(|v| v.retain(|r| r.id != id));

    let place_line = move || {
        places
            .get()
            .iter()
            .map(|r| r.name.get().trim().to_string())
            .filter(|n| !n.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    };

    let saved_note = RwSignal::new(None::<&'static str>);
    let on_save = move |_| {
        let draft = Draft {
            name: name.get_untracked(),
            pronouns: pronouns.get_untracked(),
            role: role.get_untracked(),
            bio: bio.get_untracked(),
            places: places
                .get_untracked()
                .iter()
                .map(|r| SavedPlace { name: r.name.get_untracked(), precision: r.precision.get_untracked() })
                .collect(),
        };
        saved_note.set(Some(if save_draft(&draft) {
            "Draft saved in this browser. It will save to your node once there is one."
        } else {
            "Could not save the draft in this browser."
        }));
    };

    let on_photo = move |ev: leptos::ev::Event| {
        let input: web_sys::HtmlInputElement = event_target(&ev);
        let Some(file) = input.files().and_then(|f| f.get(0)) else { return };
        if let Ok(url) = web_sys::Url::create_object_url_with_blob(&file) {
            if let Some(old) = photo.get_untracked() {
                let _ = web_sys::Url::revoke_object_url(&old);
            }
            photo.set(Some(url));
        }
    };
    on_cleanup(move || {
        if let Some(url) = photo.try_get_untracked().flatten() {
            let _ = web_sys::Url::revoke_object_url(&url);
        }
    });

    view! {
        <div class="profile-layout">
            <main class="page profile-form">
                <Stepper current=2 />

                <h1 class="form-title">"Who you are"</h1>

                <div class="photo-field">
                    <Photo photo=photo class="photo photo-lg" />
                    <div>
                        <span class="field-label">"Photo"</span>
                        <label class="secondary file-button">
                            "Choose a file"
                            <input type="file" accept="image/*" class="visually-hidden" on:change=on_photo />
                        </label>
                        <p class="hint">"Square, at least 400 px. Stored on your node, not uploaded to a server."</p>
                    </div>
                </div>

                <div class="field-row name-row">
                    <label class="field">
                        <span class="field-label">"Chosen name"</span>
                        <input type="text" autocomplete="name" bind:value=name />
                    </label>
                    <label class="field">
                        <span class="field-label">"Pronouns"</span>
                        <input type="text" placeholder="optional" bind:value=pronouns />
                    </label>
                </div>

                <label class="field">
                    <span class="field-label">"Role and organisation"</span>
                    <input type="text" placeholder="e.g. Research Fellow, NYU" bind:value=role />
                </label>

                <label class="field">
                    <span class="field-label field-label-split">
                        "Short bio"
                        <span class="counter">{move || bio.with(|b| b.chars().count())} " / " {BIO_LIMIT}</span>
                    </span>
                    <textarea rows="3" maxlength=BIO_LIMIT bind:value=bio></textarea>
                </label>

                <fieldset class="places">
                    <legend>
                        <span class="field-label">"Where you work"</span>
                        <span class="hint-inline">"Add as many places as apply"</span>
                    </legend>
                    <For each=move || places.get() key=|r| r.id let:row>
                        <div class="place-row">
                            <input type="text" aria-label="Place name" placeholder="Place name" bind:value=row.name />
                            <select
                                aria-label="How precise this place is"
                                prop:value=move || row.precision.get().label()
                                on:change=move |ev| row.precision.set(Precision::from_label(&event_target_value(&ev)))
                            >
                                {Precision::ALL
                                    .into_iter()
                                    .map(|p| view! { <option value=p.label()>{p.label()}</option> })
                                    .collect_view()}
                            </select>
                            <button class="remove-place" aria-label="Remove this place" on:click=move |_| remove_place(row.id)>
                                <svg viewBox="0 0 24 24" aria-hidden="true">
                                    <path d="M6 6l12 12M18 6L6 18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
                                </svg>
                            </button>
                        </div>
                    </For>
                    <button class="text-button" on:click=add_place>"Add another place"</button>
                </fieldset>
            </main>

            <aside class="preview-pane">
                <h2 class="pane-label">"How others will see you"</h2>

                <article class="preview-card" aria-live="polite">
                    <Photo photo=photo class="photo photo-sm" />
                    <h3 class="preview-name" class:empty=move || name.with(|n| n.trim().is_empty())>
                        {move || {
                            let n = name.get();
                            if n.trim().is_empty() { "Your name".to_string() } else { n }
                        }}
                        {move || {
                            let p = pronouns.get();
                            (!p.trim().is_empty()).then(|| view! { <span class="preview-pronouns">{p}</span> })
                        }}
                    </h3>
                    {move || {
                        let r = role.get();
                        (!r.trim().is_empty()).then(|| view! { <p class="preview-meta">{r}</p> })
                    }}
                    {move || {
                        let line = place_line();
                        (!line.is_empty()).then(|| view! { <p class="preview-meta">{line}</p> })
                    }}
                    {move || {
                        let b = bio.get();
                        (!b.trim().is_empty()).then(|| view! { <p class="preview-bio">{b}</p> })
                    }}
                </article>

                <p class="pane-note">
                    "This card is what appears in the directory. The full record on your node can hold more later."
                </p>

                <div class="pane-actions">
                    <a class="primary" href="#/visibility">"Choose who can see this"</a>
                    <button class="text-button quiet" on:click=on_save>"Save a draft on this node"</button>
                    {move || saved_note.get().map(|note| view! { <p class="save-note" role="status">{note}</p> })}
                </div>
            </aside>
        </div>
    }
}

/// The chosen photo, or the crossed-out placeholder box.
#[component]
fn Photo(photo: RwSignal<Option<String>>, class: &'static str) -> impl IntoView {
    move || match photo.get() {
        Some(src) => view! { <img class=class src=src alt="Your photo" /> }.into_any(),
        None => view! {
            <svg class=class viewBox="0 0 100 100" preserveAspectRatio="none" role="img" aria-label="No photo yet">
                <line x1="0" y1="0" x2="100" y2="100" vector-effect="non-scaling-stroke" />
                <line x1="100" y1="0" x2="0" y2="100" vector-effect="non-scaling-stroke" />
            </svg>
        }
        .into_any(),
    }
}
