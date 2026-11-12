//! What a member sees on their own profile (decision record 0022): one next
//! step at the top, and tabs that appear once they're useful
//! ([`crate::ramp`]):
//!
//! | Tab | Holds |
//! |---|---|
//! | Profile | the profile, and "Update your profile" |
//! | Sharing | share to a commons, waiting to be listed, your commons and withdrawing |
//! | People | invite someone, vouch for someone (once listed somewhere) |
//! | Account | back up your identity, your keys |

use maud::{html, Markup};

use crate::canonical::short;
use crate::ramp::{self, MemberState, NextStep, Tab};
use crate::web::layout::host_label;

/// What the owner sees on their own profile.
pub struct OwnerPanel {
    pub csrf: String,
    /// Whether they have published a profile.
    pub published: bool,
    /// Whether the current version may be shared (not members-only).
    pub shareable: bool,
    /// Commons offered as one-click buttons; the first is suggested.
    pub known: Vec<String>,
    /// Commons already shared to.
    pub shared_to: Vec<String>,
    /// Whether a recovery key has been made for this handle.
    pub has_recovery: bool,
    /// Whether a key change is waiting to reach some commons.
    pub rotation_pending: bool,
    /// Commons where this member's share is waiting for a vouch or approval.
    pub waiting_on: Vec<String>,
    /// Whether they've backed up (or said "Not now").
    pub backup_done: bool,
    /// Their public signing key (people vouching for them check it).
    pub key: String,
    /// The tab asked for (`?tab=`); hidden tabs fall back to Profile.
    pub tab: Tab,
}

impl OwnerPanel {
    /// What the ramp needs to know.
    pub fn state(&self) -> MemberState<'_> {
        MemberState {
            published: self.published,
            shareable: self.shareable,
            suggested_commons: self.known.first().map(String::as_str),
            shared_to: &self.shared_to,
            waiting_on: &self.waiting_on,
            backup_done: self.backup_done,
        }
    }

    /// The tab actually shown.
    pub fn active(&self) -> Tab {
        ramp::active_tab(self.tab, &ramp::tabs(&self.state()))
    }
}

/// The address of one of the member's tabs.
fn tab_href(handle: &str, t: Tab) -> String {
    if t == Tab::Profile {
        format!("/@{handle}")
    } else {
        format!("/@{handle}?tab={}", t.key())
    }
}

/// The one next step, at the top of the Profile tab.
pub(super) fn next_step_block(handle: &str, o: &OwnerPanel) -> Markup {
    let sharing = tab_href(handle, Tab::Sharing);
    match ramp::next_step(&o.state()) {
        NextStep::Done => html! {},
        NextStep::Publish => html! {
            section class="next-step" {
                span class="star step-1" aria-hidden="true" { "1" }
                p class="next-step-label" { "Next step" }
                h2 { "Publish your profile." }
                p class="next-step-lede" { "Say who you are, and choose who can see it and how it can be used. It stays on this computer until you share it." }
                div class="next-step-actions" { a class="btn-solid" href="/new" { "Write your profile" } }
            }
        },
        NextStep::Share(Some(c)) => html! {
            section class="next-step" {
                span class="star step-2" aria-hidden="true" { "2" }
                p class="next-step-label" { "Next step" }
                h2 { "Share your profile to " (host_label(&c)) "." }
                p class="next-step-lede" { "Send a signed copy so people there can find you. The original stays on this computer, and you can withdraw the copy any time." }
                div class="next-step-actions" {
                    form method="post" action="/share" {
                        input type="hidden" name="handle" value=(handle);
                        input type="hidden" name="csrf" value=(o.csrf);
                        input type="hidden" name="commons_url" value=(c);
                        button class="btn-solid" type="submit" { "Share to " (host_label(&c)) }
                    }
                    a class="btn-outline" href=(sharing) { "Share somewhere else" }
                }
            }
        },
        NextStep::Share(None) => html! {
            section class="next-step" {
                span class="star step-2" aria-hidden="true" { "2" }
                p class="next-step-label" { "Next step" }
                h2 { "Share your profile to a commons." }
                p class="next-step-lede" { "Ask the person running your commons for its address (it looks like http://192.168.1.5:8081), then share a signed copy there." }
                div class="next-step-actions" { a class="btn-solid" href=(sharing) { "Choose a commons" } }
            }
        },
        NextStep::Waiting(on) => html! {
            section class="next-step status-block" role="status" {
                span class="star step-3" aria-hidden="true" { "3" }
                p class="status-label" { "Waiting to be listed" }
                h2 { @for (i, c) in on.iter().enumerate() { @if i > 0 { ", " } (host_label(c)) } " has your profile" }
                p { "You'll appear once someone listed there vouches for you, or its steward approves you. Send someone you know there these two things:" }
                p class="cmd" { "@" (handle) " · key " (short(&o.key)) }
                div class="actions" {
                    form method="post" action="/waiting/check" {
                        input type="hidden" name="csrf" value=(o.csrf);
                        button class="btn-ink" type="submit" { "Check now" }
                    }
                    a class="link-btn" href=(sharing) { "I have an invite code" }
                }
            }
        },
        NextStep::Backup => html! {
            section class="next-step" {
                span class="star step-4" aria-hidden="true" { "4" }
                p class="next-step-label" { "Next step" }
                h2 { "Back up your identity." }
                p class="next-step-lede" { "Your keys live only on this computer. A backup file, locked with your passphrase, brings @" (handle) " back on another one." }
                div class="next-step-actions" {
                    form method="post" action="/backup" class="next-step-form" {
                        input type="hidden" name="handle" value=(handle);
                        input type="hidden" name="csrf" value=(o.csrf);
                        label class="field-label" for="next-pass" { "Your passphrase" }
                        input id="next-pass" name="passphrase" type="password" required autocomplete="current-password";
                        button class="btn-solid" type="submit" { "Download backup" }
                    }
                    form method="post" action="/backup/later" {
                        input type="hidden" name="handle" value=(handle);
                        input type="hidden" name="csrf" value=(o.csrf);
                        button class="btn-outline" type="submit" { "Not now" }
                    }
                }
            }
        },
    }
}

/// The tab bar (only the tabs this member can see).
pub(super) fn tab_nav(handle: &str, o: &OwnerPanel) -> Markup {
    let active = o.active();
    html! {
        nav class="tabs" aria-label="Your profile" {
            @for t in ramp::tabs(&o.state()) {
                @if t == active {
                    a class="tab" href=(tab_href(handle, t)) aria-current="page" { (t.label()) }
                } @else {
                    a class="tab" href=(tab_href(handle, t)) { (t.label()) }
                }
            }
        }
    }
}

/// Under the profile, on the Profile tab.
pub(super) fn profile_actions(o: &OwnerPanel) -> Markup {
    html! {
        @if o.published {
            div class="actions owner-actions" { a class="btn-line" href="/new" { "Update your profile" } }
        }
    }
}

/// The content of a tab other than Profile.
pub(super) fn tab_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        div class="owner-panel" {
            @match o.active() {
                Tab::Profile => {}
                Tab::Sharing => { (sharing_tab(handle, o)) }
                Tab::People => { (people_tab(handle, o)) }
                Tab::Account => { (super::owner_account::account_tab(handle, o)) }
            }
        }
    }
}

/// Sharing: waiting, share to a commons, and the commons they're on.
fn sharing_tab(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
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
                            input name="commons_url" type="url" placeholder="https://commons.example" aria-label="Commons address";
                            button class="secondary" type="submit" { "Share" }
                        }
                        div class="share-row" {
                            input name="invite" type="text" placeholder="Invite code (if someone there gave you one)" autocomplete="off" aria-label="Invite code";
                        }
                        p class="hint" { "A commons lists newcomers once someone there vouches for them. With an invite code you're listed straight away; without one, you wait until a member vouches for you." }
                    }
                }
            }
        }
        // Shown whatever the current reach: someone who narrows a profile to
        // members-only may also want the shared copies gone.
        @if !o.shared_to.is_empty() { (your_commons(handle, o)) }
        p class="hint" { a href="/stories" { "Your stories" } " have their own Share and Withdraw buttons." }
    }
}

/// The commons this member has shared to, each with its status and Withdraw.
fn your_commons(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        section class="share-panel withdraw-panel" {
            h2 class="field-label" { "Your commons" }
            p class="hint" { strong { "Withdraw" } " asks that commons to remove your profile, and your stories there, straight away. It keeps only a note that @" (handle) " is yours, so nobody else can take the name there. You can share again whenever you like. Copies other people already downloaded stay with them." }
            ul class="commons-list" {
                @for c in &o.shared_to {
                    li class="commons-item commons-row" {
                        span class="commons-meta" {
                            span class="commons-name" { (host_label(c)) }
                            span class="commons-status" { (if o.waiting_on.contains(c) { "waiting for a vouch" } else { "listed \u{2713}" }) }
                        }
                        form method="post" action="/withdraw" {
                            input type="hidden" name="handle" value=(handle);
                            input type="hidden" name="csrf" value=(o.csrf);
                            input type="hidden" name="commons_url" value=(c);
                            button class="secondary" type="submit" { "Withdraw my profile" }
                        }
                    }
                }
            }
        }
    }
}

/// People: invite and vouch once listed on a commons; before that, what to do
/// to get listed. Vouching needs a listing because a commons only accepts a
/// vouch from someone it already lists (ADR 0011).
fn people_tab(handle: &str, o: &OwnerPanel) -> Markup {
    if !o.state().listed_on().is_empty() {
        return html! { (invite_panel(handle, o)) (vouch_panel(handle, o)) };
    }
    html! {
        section class="share-panel" {
            h2 class="field-label" { "Invite and vouch for people" }
            p class="hint" { "Once you're listed on a commons, this is where you invite people you know and vouch for people waiting to join it." }
            @if !o.waiting_on.is_empty() {
                p class="hint" {
                    "You're waiting to be listed on "
                    @for (i, c) in o.waiting_on.iter().enumerate() { @if i > 0 { ", " } strong { (host_label(c)) } }
                    ". When a member vouches for you or the steward approves you, you can bring others in."
                }
            } @else if o.published {
                div class="actions" { a class="btn-line" href=(tab_href(handle, Tab::Sharing)) { "Share your profile to get listed" } }
            } @else {
                div class="actions" { a class="btn-line" href="/new" { "Publish your profile first" } }
            }
        }
    }
}

/// Invite someone to a commons this member is listed on.
fn invite_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        section class="share-panel" {
            h2 class="field-label" { "Invite someone" }
            p class="hint" { "Makes a one-time code that lets a person you know join that commons straight away, with an " strong { "Email this invite" } " button." }
            ul class="commons-list" {
                @for c in o.state().listed_on() {
                    li class="commons-item commons-row" {
                        span class="commons-meta" { span class="commons-name" { (host_label(&c)) } }
                        form method="post" action="/invite" {
                            input type="hidden" name="handle" value=(handle);
                            input type="hidden" name="csrf" value=(o.csrf);
                            input type="hidden" name="commons_url" value=(c);
                            button class="secondary" type="submit" { "Invite someone" }
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
                    select id="vouch-commons" name="commons_url" { @for c in o.state().listed_on() { option value=(c) { (host_label(&c)) } } }
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
