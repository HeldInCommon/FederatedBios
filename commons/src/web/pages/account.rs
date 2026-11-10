//! Pages about a member's identity: the one-time recovery code, the result
//! of replacing a key, and restoring from a backup.

use maud::{html, Markup};

use crate::web::layout::host_label;

/// The one-time display of a recovery code.
pub fn recovery_code(handle: &str, code: &str, next: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Your recovery code" }
            p class="lede" { "Write this down or print this page, and keep it somewhere safe and offline. It's shown only once." }
        }
        section class="recovery" {
            p class="recovery-code mono" { (code) }
            p class="hint" {
                "If your keys are ever stolen, this code lets you take @" (handle) " back on every commons, even if the thief changed your key first. "
                "Don't keep it on this computer or in your backup file: anyone with your laptop or backup could then use it too."
            }
        }
        div class="actions" { a class="primary" href=(next) { "I've saved it" } }
    }
}

/// The result of replacing or recovering a key.
pub fn key_change(handle: &str, done: &[String], failed: &[(String, String)], new_code: Option<&str>) -> Markup {
    html! {
        header class="intro" {
            @if failed.is_empty() {
                h1 { "Key replaced" }
                p class="lede" { "@" (handle) " now uses a new key on this node and on every commons you've shared to. The old key no longer works." }
            } @else {
                h1 { "Not finished yet" }
                p class="lede" { "Some commons couldn't be reached. This node keeps using your current key until every commons has the change; press the button again later to finish." }
            }
        }
        @if !done.is_empty() {
            p { "Accepted by: " @for (i, c) in done.iter().enumerate() { @if i > 0 { ", " } (host_label(c)) } }
        }
        @if !failed.is_empty() {
            ul { @for (c, why) in failed { li { strong { (host_label(c)) } ": " (why) } } }
        }
        @if let Some(code) = new_code {
            section class="recovery" {
                h2 class="field-label" { "Your new recovery code" }
                p class="recovery-code mono" { (code) }
                p class="hint" { "Your old recovery code no longer works. Write this one down; it's shown only once." }
            }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}

/// Restore an identity from a backup file (nodes only).
pub fn restore_form() -> Markup {
    html! {
        header class="intro" {
            h1 { "Restore from a backup" }
            p class="lede" { "Open your backup file in a text editor, copy everything in it, and paste it below with the passphrase you used." }
        }
        form method="post" action="/restore" class="stack-form" {
            div class="field" {
                label class="field-label" for="backup" { "Backup file contents" }
                textarea id="backup" name="backup" rows="10" required placeholder="{ \"format\": \"hic-identity-backup/1\", … }" {}
            }
            div class="field" {
                label class="field-label" for="passphrase" { "Passphrase" }
                input id="passphrase" name="passphrase" type="password" maxlength="200" required;
            }
            div class="actions" { button class="primary" type="submit" { "Restore" } }
        }
    }
}
