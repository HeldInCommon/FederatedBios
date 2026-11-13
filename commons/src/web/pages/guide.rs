//! "Run a commons": questions and answers for someone with a node who
//! is wondering about hosting a commons, and a way to try one safely.
//!
//! Inside the desktop app the page offers two buttons. They are `hic-app://`
//! links, which the app catches (`src-tauri/src/links.rs`): a page served by
//! this server can't call the app directly, but it can ask to go somewhere,
//! and the app decides what that means. In a browser or Docker the links
//! would go nowhere, so the page says how to set a commons up instead.

use maud::{html, Markup};

use super::schematics::{figure, FEDERATE_SVG, JOIN_SVG, REACH_SVG, SIDE_BY_SIDE_SVG};

/// Asks the app to open (starting it if needed) the local commons (called
/// `practice` in links and code, its first name).
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
            h1 { "Run a commons" }
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
            p { "No. The desktop app can run your node and a commons side by side on this computer, each with its own folder. Your profile, your keys and your sign-in stay on your node. You can even share your own profile to the commons you run, like anyone else would. Move between them from " strong { "This computer" } " in the app's sidebar or the buttons in the corner of the window." }
            (figure(SIDE_BY_SIDE_SVG, html! { "One computer, up to three servers, each with its own folder: " strong { "your node" } ", " strong { "a commons" } " your node can share to like anyone else's, and a " strong { "local commons" } " for a workshop or a session, which anyone on this Wi-Fi can reach." }))

            h2 class="section-h" { "Where can people reach it?" }
            p {
                strong { "On this Wi-Fi: " } "the desktop app can run a " strong { "local commons" } " on this computer, and people on the same Wi-Fi share to it. It's there while this computer is awake and the app is open, which suits a workshop, a class or a session. "
                strong { "Online: " } "an " strong { "online commons" } " runs on a server with the Docker image (see " a href=(DOCKER_GUIDE) { "the commons guide" } "), or from this app at an address you've set up, such as a Tailscale Funnel address or your own domain: choose it when you set up the commons, or later from " strong { "Go → Public address…" } ", which can check that it works."
            }

            (figure(REACH_SVG, html! { "How far each reaches: " strong { "your node" } " stays on this computer; a " strong { "local commons" } " reaches anyone on the same Wi-Fi while the app is open; an " strong { "online commons" } " reaches anyone with its address." }))

            h2 class="section-h" { "What does a commons keep?" }
            p { "The copies people share and nothing else. Members-only work is never sent. When someone withdraws a copy, the commons removes it. A copy someone already downloaded stays with them, as with anything shared." }

            h2 class="section-h" { "What's a local commons?" }
            p { "A commons on this computer that anyone on the same Wi-Fi can reach: for a workshop, a class, or a session between artists hanging out. Start it from " strong { "This computer" } " in the app's sidebar and give it a name. It starts closed: only this computer can reach it. When the room is ready, choose " strong { "Open … to the room" } " there, and show the room its QR code and room code. People share to it from their own nodes and, as on any commons, are listed once a member vouches for them, which is one click, or you approve them. When the session's over, remove it from " strong { "This computer" } " and it's gone." }
            p { "An " strong { "online commons" } " is different: it has an address anyone can reach, on a server or through a tunnel such as Tailscale Funnel, and it stays up for a community over time." }
        }
    }
}

/// The "try it" panel: buttons in the app, directions elsewhere.
fn try_it(in_app: bool) -> Markup {
    html! {
        section class="recovery try-commons" {
            h2 class="field-label" { "Try it" }
            @if in_app {
                p { "Start a local commons for a workshop or a session, then open it to the room from This computer in the app's sidebar: anyone on this Wi-Fi can share to it while it's open. Your node keeps running." }
                div class="commons-actions" {
                    a class="primary" href=(OPEN_PRACTICE) { "Start a local commons" }
                    a class="secondary" href=(OPEN_COMMONS) { "Set up a real commons" }
                }
                p class="hint" { "A real commons on this computer can be reached by people on the same Wi-Fi." }
            } @else {
                p { "The Held in Common desktop app can open a local commons that anyone on the same Wi-Fi can reach, for a workshop or a session. To host a commons online, run it from the app at an address such as a Tailscale Funnel address, or on a server with the Docker image." }
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
