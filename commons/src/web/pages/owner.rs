//! What a member sees under their own profile: share to a commons, waiting
//! to be listed, invite and withdraw, vouch, back up, and their keys.

use maud::{html, Markup};

use crate::web::layout::host_label;

/// What the owner sees under their own profile.
pub struct OwnerPanel {
    pub csrf: String,
    /// Whether the current version may be shared (not members-only).
    pub shareable: bool,
    /// Commons offered as one-click buttons.
    pub known: Vec<String>,
    /// Commons already shared to.
    pub shared_to: Vec<String>,
    /// Whether a recovery key has been made for this handle.
    pub has_recovery: bool,
    /// Whether a key change is waiting to reach some commons.
    pub rotation_pending: bool,
    /// Commons where this member's share is waiting for a vouch or approval.
    pub waiting_on: Vec<String>,
}

/// The owner's "update" button and "share to a commons" controls.
pub(super) fn owner_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        div class="owner-panel" {
            div class="actions owner-actions" { a class="primary" href="/new" { "Update your profile" } }
            @if !o.waiting_on.is_empty() {
                section class="share-panel waiting-panel" {
                    h2 class="field-label" { "Waiting to be listed" }
                    p class="hint" {
                        "Your share is waiting for a member to vouch for you, or the steward to approve you, on: "
                        @for (i, c) in o.waiting_on.iter().enumerate() { @if i > 0 { ", " } strong { (host_label(c)) } }
                        ". Your node checks each time it starts; you can also check now."
                    }
                    form method="post" action="/waiting/check" class="share-form" {
                        input type="hidden" name="csrf" value=(o.csrf);
                        button class="secondary" type="submit" { "Check now" }
                    }
                }
            }
            @if !o.shareable {
                section class="share-panel" {
                    h2 class="field-label" { "Share to a commons" }
                    p class="hint" { "This profile is " strong { "members only" } ", so it stays on your node and is never shared. To share it to a commons, choose " strong { "Update your profile" } " and set who can see it to Public or Listed in the commons." }
                }
            }
            @if o.shareable {
                section class="share-panel" {
                    h2 class="field-label" { "Share to a commons" }
                    p class="hint" { "Push a signed, verifiable copy to a shared commons \u{2014} one click. Members-only versions never leave your node." }
                    @if !o.known.is_empty() {
                        ul class="commons-list" {
                            @for c in &o.known {
                                @let shared = o.shared_to.iter().any(|s| s == c);
                                li class="commons-item" {
                                    form method="post" action="/share" class="commons-share" {
                                        input type="hidden" name="handle" value=(handle);
                                        input type="hidden" name="csrf" value=(o.csrf);
                                        input type="hidden" name="commons_url" value=(c);
                                        span class="commons-meta" {
                                            span class="commons-name" { (host_label(c)) }
                                            @if shared { span class="commons-status" { "shared \u{2713}" } }
                                        }
                                        button class="secondary" type="submit" { (if shared { "Re-share" } else { "Share" }) }
                                    }
                                }
                            }
                        }
                    }
                    details class="add-commons" open[o.known.is_empty()] {
                        summary { (if o.known.is_empty() { "Add a commons" } else { "Share with an invite code, or to another commons" }) }
                        form method="post" action="/share" class="share-form" {
                            input type="hidden" name="handle" value=(handle);
                            input type="hidden" name="csrf" value=(o.csrf);
                            div class="share-row" {
                                input name="commons_url" type="url" placeholder="https://commons.example";
                                button class="secondary" type="submit" { "Share" }
                            }
                            div class="share-row" {
                                input name="invite" type="text" placeholder="Invite code (if someone there gave you one)" autocomplete="off";
                            }
                            p class="hint" { "A commons lists newcomers once someone there vouches for them. With an invite code you're listed straight away; without one, you wait until a member vouches for you." }
                        }
                    }
                }
            }
            // Shown whatever the current reach: someone who narrows a profile
            // to members-only may also want the shared copies gone.
            @if !o.shared_to.is_empty() { (withdraw_panel(handle, o)) (vouch_panel(handle, o)) }
            (backup_panel(handle, &o.csrf))
            (keys_panel(handle, o))
        }
    }
}

/// The commons this member is on, each with "Invite someone" and "Withdraw".
fn withdraw_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        section class="share-panel withdraw-panel" {
            h2 class="field-label" { "Your commons" }
            p class="hint" { strong { "Invite someone" } " makes a one-time code that lets a person you know join that commons. " strong { "Withdraw" } " asks it to remove your shared copy; it keeps only a note that @" (handle) " is yours, so nobody else can take the name there. Copies other people already downloaded stay with them." }
            ul class="commons-list" {
                @for c in &o.shared_to {
                    li class="commons-item commons-row" {
                        span class="commons-meta" { span class="commons-name" { (host_label(c)) } }
                        span class="commons-actions" {
                            form method="post" action="/invite" {
                                input type="hidden" name="handle" value=(handle);
                                input type="hidden" name="csrf" value=(o.csrf);
                                input type="hidden" name="commons_url" value=(c);
                                button class="secondary" type="submit" { "Invite someone" }
                            }
                            form method="post" action="/withdraw" {
                                input type="hidden" name="handle" value=(handle);
                                input type="hidden" name="csrf" value=(o.csrf);
                                input type="hidden" name="commons_url" value=(c);
                                button class="secondary" type="submit" { "Withdraw" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Vouch for someone waiting on a commons this member is listed on.
fn vouch_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        section class="share-panel vouch-panel" {
            h2 class="field-label" { "Vouch for someone" }
            p class="hint" { "Someone you know is waiting to be listed on a commons? Its page shows them under " strong { "Waiting for a vouch" } ". Enter their handle and the key shown there. Only vouch for people you actually know." }
            form method="post" action="/vouch" class="share-form vouch-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(o.csrf);
                div class="field" {
                    label class="field-label" for="vouch-commons" { "Commons" }
                    select id="vouch-commons" name="commons_url" { @for c in &o.shared_to { option value=(c) { (host_label(c)) } } }
                }
                div class="field-row vouch-row" {
                    div class="field" {
                        label class="field-label" for="vouch-who" { "Their handle" }
                        div class="handle-input" { span class="at" { "@" } input id="vouch-who" name="who" type="text" maxlength="30" placeholder="theirhandle" required autocomplete="off"; }
                    }
                    div class="field" {
                        label class="field-label" for="vouch-key" { "Their key" }
                        input id="vouch-key" name="key" type="text" class="mono-input" placeholder="e.g. 3f9a0c21b7de" required autocomplete="off" spellcheck="false";
                    }
                }
                div class="actions" { button class="secondary" type="submit" { "Vouch" } }
            }
        }
    }
}

/// Download an encrypted backup of this identity.
fn backup_panel(handle: &str, csrf: &str) -> Markup {
    html! {
        section class="share-panel backup-panel" {
            h2 class="field-label" { "Back up your identity" }
            p class="hint" { "Your keys live only on this computer, and each commons ties @" (handle) " to them. Download a backup, keep it somewhere safe, and you can restore @" (handle) " on a new computer. It's locked with your passphrase." }
            form method="post" action="/backup" class="share-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(csrf);
                div class="share-row" {
                    input name="passphrase" type="password" placeholder="Your passphrase" required;
                    button class="secondary" type="submit" { "Download backup" }
                }
            }
        }
    }
}

/// Recovery key and key replacement.
fn keys_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        section class="share-panel keys-panel" {
            h2 class="field-label" { "Your keys" }
            @if !o.has_recovery {
                p class="hint" { "@" (handle) " has no recovery key yet. A recovery key, kept on paper, lets you take your handle back if your keys are ever stolen." }
                form method="post" action="/recovery/new" class="share-form" {
                    input type="hidden" name="handle" value=(handle);
                    input type="hidden" name="csrf" value=(o.csrf);
                    button class="secondary" type="submit" { "Make a recovery key" }
                }
            }
            p class="hint" { "Think someone else may have your keys (a lost laptop, a leaked backup)? Replace them. Every commons you've shared to moves @" (handle) " to the new key, and the old one stops working." }
            form method="post" action="/rotate" class="share-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(o.csrf);
                button class="secondary" type="submit" { (if o.rotation_pending { "Finish replacing my key" } else { "Replace my key" }) }
            }
            details class="add-commons" {
                summary { "Someone else has already replaced my key" }
                p class="hint" { "Use the recovery code you printed when you made @" (handle) ". It overrides any key change made with your old key." }
                form method="post" action="/recover" class="share-form" {
                    input type="hidden" name="handle" value=(handle);
                    input type="hidden" name="csrf" value=(o.csrf);
                    div class="share-row" {
                        input name="code" type="text" placeholder="xxxxxxxx-xxxxxxxx-…" required autocomplete="off";
                        button class="secondary" type="submit" { "Recover" }
                    }
                }
            }
        }
    }
}
