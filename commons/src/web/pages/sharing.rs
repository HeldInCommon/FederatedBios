//! What a member sees after sharing to a commons, vouching or withdrawing.

use maud::{html, Markup};

use crate::canonical::short;
use crate::web::layout::host_label;

/// Confirmation after a successful share.
pub fn shared(handle: &str, commons: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Shared" }
            p class="lede" { "Your profile is now federated to " a href=(commons) { (commons) } ". They have a signed, verifiable copy." }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}

/// After sharing to a commons that lists newcomers only once vouched for.
pub fn shared_pending(handle: &str, commons: &str, key: &str, csrf: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Waiting for a vouch" }
            p class="lede" { (host_label(commons)) " has your signed profile, and will list it once a member vouches for you or its steward approves you." }
        }
        p { "Ask someone you know there to vouch for you from their node. They'll need your handle and key:" }
        p class="recovery-code mono" { "@" (handle) " · " (short(key)) }
        section class="share-panel" {
            h2 class="field-label" { "Got an invite code?" }
            form method="post" action="/share" class="share-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="commons_url" value=(commons);
                div class="share-row" {
                    input name="invite" type="text" placeholder="Paste the invite code" required autocomplete="off";
                    button class="secondary" type="submit" { "Share with the code" }
                }
            }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}

/// After vouching for someone.
pub fn vouched(handle: &str, who: &str, commons_label: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Vouched" }
            p class="lede" { "@" (who) " is now listed on " (commons_label) ", with a record that you vouched for them." }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}

/// Confirmation after a withdrawal.
pub fn withdrawn(handle: &str, commons_label: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Withdrawn" }
            p class="lede" { (commons_label) " has removed your profile, and any stories you shared there. It keeps only a note that @" (handle) " is yours, so nobody else can take the name there. You can share again whenever you like." }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}
