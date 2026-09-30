//! Step 3: who can see this. Three levels of reach, each drawn as the chain of
//! places the profile travels through. A stop the profile reaches is a solid
//! chip; a stop it never reaches is a dashed, faded one.
//!
//! Nothing is chosen up front: this is a consent decision, so the member picks
//! it explicitly before they can continue.
//!
//! Open question from the design: members-only needs the gateway copy
//! encrypted to the member set. Until that exists, this page only records the
//! choice. There is no node yet, so the choice is kept in this browser's
//! localStorage as a stand-in (see `load_choice` / `save_choice`).

use crate::onboarding::Stepper;
use leptos::prelude::*;

const CHOICE_KEY: &str = "hic.profile.visibility";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reach {
    Public,
    Commons,
    Members,
}

struct ReachOption {
    reach: Reach,
    id: &'static str,
    title: &'static str,
    body: &'static str,
    /// (stop, reached?) in the order the profile travels.
    path: &'static [(&'static str, bool)],
}

const OPTIONS: [ReachOption; 3] = [
    ReachOption {
        reach: Reach::Public,
        id: "public",
        title: "Public",
        body: "Anyone can open your profile at your node\u{2019}s own address. \
               Listed on heldincommon.org and open to search engines.",
        path: &[("your node", true), ("commons storage", true), ("heldincommon.org", true), ("search engines", true)],
    },
    ReachOption {
        reach: Reach::Commons,
        id: "commons",
        title: "Listed in the commons",
        body: "Shown in the directory on heldincommon.org. Anyone with the link can read it, \
               but it is kept out of search results.",
        path: &[("your node", true), ("commons storage", true), ("heldincommon.org", true), ("search engines", false)],
    },
    ReachOption {
        reach: Reach::Members,
        id: "members",
        title: "Members only",
        body: "Only other members running a node can fetch your profile. \
               It does not appear on the public website at all.",
        path: &[("your node", true), ("member nodes", true), ("heldincommon.org", false), ("search engines", false)],
    },
];

impl Reach {
    fn from_id(id: &str) -> Option<Self> {
        OPTIONS.iter().find(|o| o.id == id).map(|o| o.reach)
    }
    fn id(self) -> &'static str {
        OPTIONS.iter().find(|o| o.reach == self).map(|o| o.id).unwrap_or_default()
    }
}

// TODO(node): read and write the choice on the node instead.
fn load_choice() -> Option<Reach> {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(CHOICE_KEY).ok().flatten())
        .and_then(|id| Reach::from_id(&id))
}

fn save_choice(reach: Reach) {
    if let Some(s) = window().local_storage().ok().flatten() {
        let _ = s.set_item(CHOICE_KEY, reach.id());
    }
}

#[component]
pub fn Visibility() -> impl IntoView {
    let choice = RwSignal::new(load_choice());
    let chosen = move || choice.get().is_some();

    let choose = move |reach: Reach| {
        choice.set(Some(reach));
        save_choice(reach);
    };

    view! {
        <main class="page">
            <Stepper current=3 />

            <header class="intro visibility-intro">
                <h1>"Who can see this"</h1>
                <p class="lede">"Each choice sets how far your profile travels. You can change it later."</p>
            </header>

            <fieldset class="reach-options">
                <legend class="visually-hidden">"Who can see your profile"</legend>
                {OPTIONS
                    .iter()
                    .map(|opt| {
                        let reach = opt.reach;
                        let selected = move || choice.get() == Some(reach);
                        view! {
                            <label class="reach" class:selected=selected>
                                <input
                                    type="radio"
                                    name="reach"
                                    value=opt.id
                                    prop:checked=selected
                                    on:change=move |_| choose(reach)
                                />
                                <span class="reach-text">
                                    <span class="reach-title">{opt.title}</span>
                                    <span class="reach-body">{opt.body}</span>
                                    <ReachPath path=opt.path />
                                </span>
                            </label>
                        }
                    })
                    .collect_view()}
            </fieldset>

            <aside class="caution">
                <p>
                    <strong>"Publishing cannot be fully undone."</strong>
                    " Each version gets a permanent content address. If you later narrow your visibility "
                    "or withdraw, the commons stops serving that version, but a copy someone has already "
                    "fetched stays with them."
                </p>
            </aside>

            <footer class="actions">
                <a
                    class="primary"
                    href="#/publish"
                    class:disabled=move || !chosen()
                    aria-disabled=move || (!chosen()).to_string()
                    on:click=move |ev| if !chosen() { ev.prevent_default() }
                >
                    "Continue to publish"
                </a>
                <a class="help-link" href="#/profile">"Back to your profile"</a>
            </footer>
        </main>
    }
}

/// The chain of stops, e.g. your node → commons storage → heldincommon.org.
#[component]
fn ReachPath(path: &'static [(&'static str, bool)]) -> impl IntoView {
    let reached = path.iter().filter(|(_, r)| *r).count();
    let summary = format!(
        "Reaches {}.{}",
        path.iter().filter(|(_, r)| *r).map(|(s, _)| *s).collect::<Vec<_>>().join(", "),
        if reached < path.len() {
            format!(
                " Does not reach {}.",
                path.iter().filter(|(_, r)| !*r).map(|(s, _)| *s).collect::<Vec<_>>().join(" or ")
            )
        } else {
            String::new()
        }
    );

    view! {
        <span class="reach-path">
            <span class="visually-hidden">{summary}</span>
            {path
                .iter()
                .enumerate()
                .map(|(i, (stop, reached))| {
                    // Arrow and stop share one unbreakable unit, so a wrapped
                    // line starts with an arrow instead of ending on one.
                    view! {
                        <span class="leg" aria-hidden="true">
                            {(i > 0).then(|| view! { <span class="hop">"\u{2192}"</span> })}
                            <span class="stop" class:unreached=!*reached>{*stop}</span>
                        </span>
                    }
                })
                .collect_view()}
        </span>
    }
}
