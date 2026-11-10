//! Sign-up and sign-in (nodes), and the page a commons shows instead.

use maud::{html, Markup};

use crate::web::layout::Site;

/// Where the "Get the app" button points.
const APP_DOWNLOAD: &str = "https://github.com/HeldInCommon/FederatedBios/releases/latest";

/// The sign-up form (nodes only).
pub fn join_form() -> Markup {
    html! {
        header class="intro" {
            h1 { "Join the commons" }
            p class="lede" { "Pick a handle. We mint your keys on the node — you never handle a private key yourself." }
        }
        form method="post" action="/join" class="stack-form" {
            div class="field" {
                label class="field-label" for="handle" { "Handle" }
                div class="handle-input" { span class="at" { "@" } input id="handle" name="handle" type="text" maxlength="30" placeholder="phoenixperry" required; }
                p class="hint" { "Lowercase letters, numbers and underscores. 2–30 characters." }
            }
            div class="field" {
                label class="field-label" for="display_name" { "Display name" }
                input id="display_name" name="display_name" type="text" maxlength="80" placeholder="Phoenix Perry" required;
            }
            div class="field" {
                label class="field-label" for="passphrase" { "Passphrase" }
                input id="passphrase" name="passphrase" type="password" minlength="8" maxlength="200" placeholder="At least 8 characters" required;
                p class="hint" { "You'll use this to sign in and to publish. Choose something you'll remember — we store only a salted hash and can't recover it." }
            }
            div class="actions" { button class="primary" type="submit" { "Create handle" } }
        }
        p class="hint" { "Moving to a new computer? " a href="/restore" { "Restore from a backup" } "." }
    }
}

/// The sign-in form (nodes only).
pub fn login_form() -> Markup {
    html! {
        header class="intro" { h1 { "Sign in" } p class="lede" { "Sign in as a member to read members-only profiles." } }
        form method="post" action="/login" class="stack-form" {
            div class="field" {
                label class="field-label" for="handle" { "Handle" }
                div class="handle-input" { span class="at" { "@" } input id="handle" name="handle" type="text" maxlength="30" placeholder="phoenixperry" required; }
            }
            div class="field" {
                label class="field-label" for="passphrase" { "Passphrase" }
                input id="passphrase" name="passphrase" type="password" maxlength="200" required;
            }
            div class="actions" { button class="primary" type="submit" { "Sign in" } }
        }
    }
}

/// Shown instead of sign-up and sign-in on a commons: explains that profiles
/// arrive from people's own nodes.
pub fn commons_gate(site: &Site) -> Markup {
    html! {
        header class="intro" {
            h1 { "This is a shared commons" }
            p class="lede" {
                "Profiles here are signed copies that people publish from their own nodes. "
                "There's no sign-up on a commons — your identity and keys live on your node "
                "and never leave your machine."
            }
        }
        div class="gate" {
            h2 { "To appear in this directory" }
            ol class="gate-steps" {
                li { "Run your own node — download the app (no account, no Docker)." }
                li { "Create your handle and profile on your node." }
                li {
                    "Click " strong { "“Share to a commons”" } " and paste this commons's address"
                    @if !site.instance.is_empty() { " (" (site.instance) ")" } "."
                }
            }
            div class="actions" { a class="primary" href=(APP_DOWNLOAD) { "Get the app \u{2192}" } }
            p class="hint" {
                "A signed copy of your profile then appears here. Members-only profiles stay "
                "encrypted on your node and are never shared — not even to a commons."
            }
        }
    }
}
