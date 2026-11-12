//! The Account tab on a member's own profile: keeping their identity safe.
//! A backup, a recovery key and their key as cards; replacing or recovering
//! a key behind "Advanced"; signing out.

use maud::{html, Markup};

use super::owner::OwnerPanel;
use super::parts::section_label;
use crate::canonical::short;

/// The Account tab.
pub(super) fn account_tab(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        (section_label("If this computer is lost", None))
        div class="account-grid" {
            section class="account-card" {
                p class="card-label" { "Backup" }
                h2 class="card-title" { "A file you keep somewhere safe" }
                p class="muted" { "Locked with your passphrase. Restore it on another computer to carry on as @" (handle) "." }
                form method="post" action="/backup" class="account-form" {
                    input type="hidden" name="handle" value=(handle);
                    input type="hidden" name="csrf" value=(o.csrf);
                    label class="field-label" for="backup-pass" { "Your passphrase" }
                    input id="backup-pass" name="passphrase" type="password" required autocomplete="current-password";
                    button class="btn-line" type="submit" { "Download backup" }
                }
            }
            section class="account-card" {
                p class="card-label" { "Recovery key" }
                h2 class="card-title" { "A spare key, kept offline" }
                p class="muted" { "Moves you to fresh keys if this computer is lost or someone takes your key." }
                @if o.has_recovery {
                    p { "You've made one. Keep its code on paper, somewhere safe." }
                } @else {
                    form method="post" action="/recovery/new" class="account-form" {
                        input type="hidden" name="handle" value=(handle);
                        input type="hidden" name="csrf" value=(o.csrf);
                        button class="btn-line" type="submit" { "Make a recovery key" }
                    }
                }
            }
            section class="account-card is-ink" {
                p class="card-label" { "Your key" }
                p class="key-value" { (short(&o.key)) }
                p class="muted" { "People who vouch for you check that this matches." }
            }
        }
        details class="advanced" open[o.rotation_pending] {
            summary { "Advanced \u{b7} replace my key, recover my handle" }
            p class="hint" { "Think someone else may have your keys (a lost laptop, a leaked backup)? Replace them. Every commons you've shared to moves @" (handle) " to the new key, and the old one stops working." }
            form method="post" action="/rotate" class="share-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(o.csrf);
                button class="btn-line" type="submit" { (if o.rotation_pending { "Finish replacing my key" } else { "Replace my key" }) }
            }
            p class="hint" { "Someone else has already replaced your key? Use the recovery code you printed when you made @" (handle) ". It overrides any key change made with your old key." }
            form method="post" action="/recover" class="share-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(o.csrf);
                label class="field-label" for="recover-code" { "Recovery code" }
                div class="share-row" {
                    input id="recover-code" name="code" type="text" placeholder="xxxxxxxx-xxxxxxxx-…" required autocomplete="off";
                    button class="btn-line" type="submit" { "Recover" }
                }
            }
        }
        form method="post" action="/logout" class="account-signout" {
            button class="btn-action" type="submit" { "Sign out" }
        }
    }
}
