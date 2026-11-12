//! "Run your own commons": questions and answers for someone with a node who
//! is wondering about hosting a commons, and a way to try one safely.
//!
//! Inside the desktop app the page offers two buttons. They are `hic-app://`
//! links, which the app catches (`src-tauri/src/links.rs`): a page served by
//! this server can't call the app directly, but it can ask to go somewhere,
//! and the app decides what that means. In a browser or Docker the links
//! would go nowhere, so the page says how to set a commons up instead.

use maud::{html, Markup};

use super::schematics::{figure, FEDERATE_SVG, JOIN_SVG, REACH_SVG, SIDE_BY_SIDE_SVG};

/// Asks the app to open (starting it if needed) a practice commons.
pub const OPEN_PRACTICE: &str = "hic-app://open/practice";
/// Asks the app to show this computer's commons, setting one up if needed.
pub const OPEN_COMMONS: &str = "hic-app://open/commons";
/// Asks the app to show this computer's node, setting one up if needed.
pub const OPEN_NODE: &str = "hic-app://open/node";

/// Where the install guide and the Docker instructions live.
const INSTALL_GUIDE: &str = "https://github.com/HeldInCommon/FederatedBios/blob/main/docs/INSTALL.md";
/// Running a commons with Docker.
const DOCKER_GUIDE: &str = "https://github.com/HeldInCommon/FederatedBios/blob/main/commons/README.md";

/// The page body. `in_app` is true when the desktop app runs this node.
pub fn run_a_commons(in_app: bool) -> Markup {
    html! {
        header class="intro" {
            h1 { "Run your own commons" }
            p class="lede" { "A shared directory for a workshop, a class or a collective, hosted by you." }
        }
        (try_it(in_app))
        section class="prose faq" {
            h2 class="section-h" { "What is a commons?" }
            p { "A directory that people's nodes share to. It lists the profiles they send and the terms they attached. It holds signed copies only: nobody signs up on a commons, and it never holds anyone's keys, so it can't act as them." }
            (figure(FEDERATE_SVG, html! { "Each person's node sends a " strong { "signed copy" } " to the commons. The originals stay on their own computers; the commons gathers the copies into one directory." }))

            h2 class="section-h" { "Who runs one, and what do they do?" }
            p { "Whoever hosts it is its " strong { "steward" } ". The steward decides who gets listed: a newcomer appears once a member vouches for them or the steward approves them. The steward can also make invite codes to send to people. The very first person needs a " strong { "founding invite" } ", which the commons shows its steward until someone uses it. That first person can be you." }
            (figure(JOIN_SVG, html! { "How someone gets listed: they share, wait on the commons' list, and are listed once a " strong { "member vouches" } " for them or the " strong { "steward approves" } " them." }))

            h2 class="section-h" { "Do I give up my node?" }
            p { "No. The desktop app can run your node and a commons side by side on this computer, each with its own folder. Your profile, your keys and your sign-in stay on your node. You can even share your own profile to your own commons, like anyone else would. Move between them from the " strong { "Go" } " menu or the buttons in the corner of the window." }
            (figure(SIDE_BY_SIDE_SVG, html! { "One computer, up to three servers, each with its own folder: " strong { "your node" } ", " strong { "a commons" } " your node can share to like anyone else's, and a " strong { "practice" } " commons only you can reach." }))

            h2 class="section-h" { "Where can people reach it?" }
            p {
                strong { "In this room: " } "the desktop app can run a commons on this computer, and people on the same Wi-Fi share to it. It's there while this computer is awake and the app is open, which suits a workshop or a class. "
                strong { "Online: " } "run it on a server with the Docker image (see " a href=(DOCKER_GUIDE) { "the commons guide" } "). A commons online through a tunnel from this app is coming."
            }

            (figure(REACH_SVG, html! { "How far a commons reaches: a " strong { "practice" } " commons stays on this computer, a " strong { "room" } " commons reaches the same Wi-Fi while the app is open, and an " strong { "online" } " one reaches anyone with its address." }))

            h2 class="section-h" { "What does a commons keep?" }
            p { "The copies people share and nothing else. Members-only work is never sent. When someone withdraws a copy, the commons removes it. A copy someone already downloaded stays with them, as with anything shared." }

            h2 class="section-h" { "What's a practice commons?" }
            p { "A commons only this computer can reach. Nothing on it is seen by anyone else, so it's a safe place to learn: share your profile to it from your node, watch it wait for a vouch, approve it in the steward view, make an invite. When you're done, remove it from the " strong { "Go" } " menu and it's gone." }
        }
    }
}

/// The "try it" panel: buttons in the app, directions elsewhere.
fn try_it(in_app: bool) -> Markup {
    html! {
        section class="recovery try-commons" {
            h2 class="field-label" { "Try it" }
            @if in_app {
                p { "Open a practice commons on this computer to see what running one is like. Only you can reach it. Your node keeps running." }
                div class="commons-actions" {
                    a class="primary" href=(OPEN_PRACTICE) { "Open a practice commons" }
                    a class="secondary" href=(OPEN_COMMONS) { "Set up a real commons" }
                }
                p class="hint" { "A real commons on this computer can be reached by people on the same Wi-Fi." }
            } @else {
                p { "The Held in Common desktop app can open a practice commons that only your computer can reach, and run a real one for your room. To host a commons online, use the Docker image on a server." }
                p { a href=(INSTALL_GUIDE) { "Install the desktop app" } " · " a href=(DOCKER_GUIDE) { "Run a commons in Docker" } }
            }
        }
    }
}

/// On a commons in the app, a note to its steward that they can run their
/// own node alongside it, to share their profile to this commons too.
pub fn steward_node_note() -> Markup {
    html! {
        p class="hint" {
            "Want your own profile here too? "
            a href=(OPEN_NODE) { "Run your own node on this computer" }
            ", then share to this commons like anyone else."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_gets_buttons_a_browser_gets_directions() {
        let app = run_a_commons(true).into_string();
        assert!(app.contains(OPEN_PRACTICE) && app.contains(OPEN_COMMONS));
        let web = run_a_commons(false).into_string();
        assert!(!web.contains("hic-app://"));
        assert!(web.contains(DOCKER_GUIDE));
    }
}
