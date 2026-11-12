//! A fresh invite code, from a member or from the steward, with a button
//! that opens a ready-written email.

use maud::{html, Markup};

use crate::web::layout::{host_label, Site};

/// A new invite code to hand to someone, with an email button.
pub fn invite_code(handle: &str, commons_url: &str, code: &str, days: u64) -> Markup {
    let label = host_label(commons_url);
    let (subject, body) = crate::web::email::invitation(&label, commons_url, code, &format!("— @{handle}"), Some(days));
    html! {
        header class="intro" {
            h1 { "Invite code" }
            p class="lede" { "Give this to the person you're inviting to " (label) ". They paste it into " strong { "Invite code" } " when they first share there, and are listed straight away." }
        }
        section class="recovery" {
            textarea class="recovery-code mono" readonly rows="6" { (code) }
            p class="hint" { "It works once, for " (days) " days, and says that @" (handle) " vouched for them." }
        }
        div class="actions" {
            a class="primary" href=(crate::web::email::mailto(&subject, &body)) { "Email this invite" }
            a class="secondary-link" href={ "/@" (handle) } { "Back to your profile" }
        }
        p class="hint" { "Email this invite opens your own mail app with the message written, including how to install the app; you choose who to send it to." }
    }
}

/// A steward-made invite code, with an email button.
pub fn steward_invite(site: &Site, code: &str) -> Markup {
    // The public address, so an invite emailed outside the room still works.
    let label = host_label(site.public_url());
    let from = format!("— the steward of {}", site.instance);
    let (subject, body) = crate::web::email::invitation(&label, site.public_url(), code, &from, None);
    html! {
        header class="intro" {
            h1 { "Invite code" }
            p class="lede" { "Give this to the person you're inviting. They paste it into " strong { "Invite code" } " when they share to this commons, and are listed straight away." }
        }
        section class="recovery" {
            p class="recovery-code mono" { (code) }
            p class="hint" { "It works once." }
        }
        div class="actions" {
            a class="primary" href=(crate::web::email::mailto(&subject, &body)) { "Email this invite" }
            a class="secondary-link" href="/" { "Back to the commons" }
        }
        p class="hint" { "Email this invite opens your own mail app with the message written; you choose who to send it to." }
    }
}
