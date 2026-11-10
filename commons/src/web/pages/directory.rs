//! The directory: a commons' (or node's) front page. Shared-in profiles,
//! local members, people waiting for a vouch and, for the steward, their tools.

use maud::{html, Markup};

use super::origin_host;
use crate::canonical::short;
use crate::keys::keyid;
use crate::model::{display_name_or_handle, HandleRec, PendingShare, Reach, RemoteEntry, Version};
use crate::web::layout::{reach_badge, reach_bar_class, Site};

/// A member's card: the record and the version shown (with its readable
/// profile, if the viewer may read it).
pub struct Card {
    pub rec: HandleRec,
    pub version: Version,
}

/// What the steward of a commons sees on its page (see `web::handlers::steward`).
pub struct StewardView {
    /// The steward session's CSRF token, for the approve/decline/invite forms.
    pub csrf: String,
    /// The founding invite, while it is unused.
    pub founding: Option<String>,
}

/// Who is looking at a commons' page.
pub enum Viewer<'a> {
    /// Anyone.
    Visitor,
    /// Someone at the commons' own computer who hasn't opened the steward view.
    AtThisComputer,
    /// The steward.
    Steward(&'a StewardView),
}

/// The directory: shared-in profiles first, then this server's members.
pub fn directory(
    site: &Site,
    cards: &[Card],
    remotes: &[RemoteEntry],
    pending: &[PendingShare],
    viewer: &Viewer,
) -> Markup {
    let steward = match viewer {
        Viewer::Steward(v) => Some(*v),
        _ => None,
    };
    html! {
        @if let Some(v) = steward { (steward_panel(v)) }
        @if let Viewer::AtThisComputer = viewer {
            p class="hint steward-hint" { "You're at this commons' computer. " a href="/steward" { "Open the steward view" } " to approve people and make invites." }
        }
        header class="intro" {
            h1 { "The commons" }
            p class="lede" { "Creators keep their work on their own node. The commons lets it travel — and lets the community set the terms." }
        }
        @if cards.is_empty() && remotes.is_empty() {
            @if site.is_commons() {
                p class="empty-note" { "Nothing here yet. " a href="/join" { "Run a node" } " and share your profile in — that's how profiles arrive here." }
            } @else {
                p class="empty-note" { "Nothing here yet. " a href="/join" { "Add the first profile" } " — or run your own node and share it in." }
            }
        }
        div class="dir-grid" {
            @for r in remotes {
                a class="dir-card" href={ "/federated/" (keyid(&r.ed_pub)) } {
                    span class="dir-bar vis-federated" {}
                    span class="dir-body" {
                        @if !r.profile.avatar.is_empty() { img class="dir-avatar" src=(r.profile.avatar) alt=""; }
                        span class="dir-name" { (display_name_or_handle(&r.profile.name, &r.handle)) }
                        span class="dir-handle" { "@" (r.handle) }
                        @if !r.profile.role.is_empty() { span class="dir-role" { (r.profile.role) } }
                        // Where it came from, as prominent as the name: the node's
                        // address and a short key fingerprint (the one part the
                        // commons checked). Two cards with the same name still differ.
                        span class="dir-origin" { "from " (origin_host(r)) }
                        span class="dir-key" { "key " (short(&r.ed_pub)) }
                    }
                }
            }
            @for c in cards {
                @let reach = c.version.reach();
                a class="dir-card" href={ "/@" (c.rec.handle) } {
                    span class={ "dir-bar " (reach_bar_class(reach)) } {}
                    span class="dir-body" {
                        @if let Some(p) = &c.version.payload { @if !p.avatar.is_empty() { img class="dir-avatar" src=(p.avatar) alt=""; } }
                        span class="dir-name" {
                            @if let Some(p) = &c.version.payload { (p.name) } @else { "@" (c.rec.handle) }
                        }
                        span class="dir-handle" { "@" (c.rec.handle) }
                        @if let Some(p) = &c.version.payload {
                            @if !p.role.is_empty() { span class="dir-role" { (p.role) } }
                        } @else if reach == Reach::Members {
                            span class="dir-role locked" { "Members-only — encrypted" }
                        }
                        (reach_badge(reach))
                    }
                }
            }
        }
        @if !pending.is_empty() { (pending_list(pending, steward)) }
        div class="actions" {
            @if site.is_commons() {
                a class="primary" href="/join" { "Run a node to join" }
            } @else {
                a class="primary" href="/join" { "Add your profile" }
            }
        }
    }
}

/// The steward's tools at the top of the commons page.
fn steward_panel(v: &StewardView) -> Markup {
    html! {
        section class="recovery steward" {
            h2 class="field-label" { "Steward view" }
            @if let Some(code) = &v.founding {
                p { "Nobody is listed yet, so the first person needs this founding invite. Give it to them (it can be you); they paste it into " strong { "Invite code" } " when they share from their node. It works once." }
                p class="recovery-code mono" { (code) }
            }
            p { "Approve or decline people waiting below, or make an invite code to send to someone." }
            form method="post" action="/steward/invite" class="share-form" {
                input type="hidden" name="csrf" value=(v.csrf);
                button class="secondary" type="submit" { "Make an invite" }
            }
        }
    }
}

/// Shares waiting for a vouch: enough for a member to recognise who's asking.
/// The steward also gets Approve and Decline buttons.
fn pending_list(pending: &[PendingShare], steward: Option<&StewardView>) -> Markup {
    html! {
        section class="pending" {
            h2 class="section-h" { "Waiting for a vouch" }
            p class="hint" { "These people have shared to this commons. A member who knows them can vouch for them from their own node (under " strong { "Vouch for someone" } "), using the handle and key shown here, or the steward can approve them." }
            ul class="pending-list" {
                @for p in pending {
                    li class="pending-item" {
                        span { strong { "@" (p.entry.handle) } " · key " span class="mono" { (short(&p.entry.ed_pub)) } " · from " (origin_host(&p.entry)) }
                        @if let Some(v) = steward {
                            span class="commons-actions" {
                                form method="post" action="/steward/approve" {
                                    input type="hidden" name="csrf" value=(v.csrf);
                                    input type="hidden" name="key" value=(p.entry.ed_pub);
                                    button class="secondary" type="submit" { "Approve" }
                                }
                                form method="post" action="/steward/decline" {
                                    input type="hidden" name="csrf" value=(v.csrf);
                                    input type="hidden" name="key" value=(p.entry.ed_pub);
                                    button class="secondary" type="submit" { "Decline" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
