//! The directory: a commons' (or node's) front page. Shared-in profiles,
//! local members, people waiting for a vouch and, for the steward, their tools.

use maud::{html, Markup};

use super::origin_host;
use crate::canonical::short;
use crate::keys::keyid;
use crate::model::{display_name_or_handle, HandleRec, PendingShare, Reach, RemoteEntry, Version};
use crate::web::layout::{host_label, reach_badge, reach_bar_class, tone_for_key, Site};

/// A member's card: the record and the version shown (with its readable
/// profile, if the viewer may read it).
pub struct Card {
    /// The member's record.
    pub rec: HandleRec,
    /// The version shown on the card.
    pub version: Version,
}

/// What the steward of a commons sees on its page (see `web::handlers::steward`).
pub struct StewardView {
    /// The steward session's CSRF token, for the approve/decline/invite forms.
    pub csrf: String,
    /// The founding invite, while it is unused.
    pub founding: Option<String>,
    /// Suggest the steward runs their own node too (in the desktop app, on
    /// a real commons: see `pages::steward_node_note`).
    pub offer_node: bool,
}

/// The main button a signed-in member sees on their node's front page. For
/// them "Add your profile" would make a second identity, so it becomes the
/// next thing for the profile they have; a new profile is the secondary link.
#[derive(Debug, PartialEq, Eq)]
pub enum YourNext {
    /// Nothing published yet.
    Publish,
    /// Share it to this commons, in one click.
    Share(String),
    /// Shareable, but there's no commons to suggest.
    ChooseCommons,
    /// Shared, and waiting to be listed on this commons.
    Waiting(String),
    /// Listed on `commons`; `profile_url` is their page there.
    Listed { commons: String, profile_url: String },
    /// Members-only: it stays on the node.
    Private,
}

/// A signed-in member, for the front page.
pub struct SignedIn {
    /// Their handle.
    pub handle: String,
    /// The session's CSRF token, for a one-click share.
    pub csrf: String,
    /// What their button offers.
    pub next: YourNext,
}

/// Decide [`YourNext`]: listed beats waiting beats sharing.
pub fn your_next(
    published: bool,
    shareable: bool,
    ed_pub: &str,
    known: &[String],
    shared_to: &[String],
    waiting_on: &[String],
) -> YourNext {
    if !published {
        return YourNext::Publish;
    }
    if let Some(c) = shared_to.iter().find(|c| !waiting_on.contains(c)) {
        return YourNext::Listed { commons: c.clone(), profile_url: format!("{c}/federated/{}", keyid(ed_pub)) };
    }
    if let Some(c) = waiting_on.first() {
        return YourNext::Waiting(c.clone());
    }
    if !shareable {
        return YourNext::Private;
    }
    known.first().map(|c| YourNext::Share(c.clone())).unwrap_or(YourNext::ChooseCommons)
}

/// The front page's buttons for a signed-in member.
fn signed_in_actions(m: &SignedIn) -> Markup {
    let h = &m.handle;
    html! {
        div class="actions" {
            @match &m.next {
                YourNext::Publish => { a class="primary" href="/new" { "Publish your profile" } }
                YourNext::Share(c) => {
                    form method="post" action="/share" class="inline-form" {
                        input type="hidden" name="handle" value=(h);
                        input type="hidden" name="csrf" value=(m.csrf);
                        input type="hidden" name="commons_url" value=(c);
                        button class="primary" type="submit" { "Share your profile to " (host_label(c)) }
                    }
                }
                YourNext::ChooseCommons => { a class="primary" href={ "/@" (h) "?tab=sharing" } { "Share your profile to a commons" } }
                YourNext::Waiting(c) => { a class="primary" href={ "/@" (h) } { "Waiting to be listed on " (host_label(c)) } }
                YourNext::Listed { commons, profile_url } => {
                    a class="primary" href=(profile_url) { "See your profile on " (host_label(commons)) " \u{2197}" }
                }
                YourNext::Private => { a class="primary" href={ "/@" (h) } { "Your profile" } }
            }
            a class="secondary-link" href="/join" { "Create another profile for a new commons" }
        }
    }
}

/// Who is looking at a commons' page.
pub enum Viewer<'a> {
    /// Anyone.
    Visitor,
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
    me: Option<&SignedIn>,
) -> Markup {
    let steward = match viewer {
        Viewer::Steward(v) => Some(*v),
        _ => None,
    };
    html! {
        @if let Some(v) = steward { (steward_panel(v)) }
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
                    span class={ "dir-bar " (tone_for_key(&r.ed_pub)) } {}
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
        @match (site.is_commons(), me) {
            (true, _) => { div class="actions" { a class="primary" href="/join" { "Run a node to join" } } }
            (false, Some(m)) => { (signed_in_actions(m)) }
            (false, None) => { div class="actions" { a class="primary" href="/join" { "Add your profile" } } }
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
            @if v.offer_node { (super::steward_node_note()) }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signed_in_member_is_offered_their_own_next_thing() {
        let room = vec!["http://room:8081".to_string()];
        assert_eq!(your_next(false, true, "k", &room, &[], &[]), YourNext::Publish);
        assert_eq!(your_next(true, true, "k", &room, &[], &[]), YourNext::Share("http://room:8081".into()));
        assert_eq!(your_next(true, true, "k", &[], &[], &[]), YourNext::ChooseCommons);
        assert_eq!(your_next(true, true, "k", &room, &room, &room), YourNext::Waiting("http://room:8081".into()));
        assert_eq!(your_next(true, false, "k", &room, &[], &[]), YourNext::Private);
        match your_next(true, true, "k", &room, &room, &[]) {
            YourNext::Listed { commons, profile_url } => {
                assert_eq!(commons, "http://room:8081");
                assert!(profile_url.starts_with("http://room:8081/federated/"));
            }
            other => panic!("expected Listed, got {other:?}"),
        }
    }

    #[test]
    fn the_new_profile_link_is_secondary_for_members() {
        let m = SignedIn { handle: "ada".into(), csrf: "c".into(), next: YourNext::Share("http://room:8081".into()) };
        let html = signed_in_actions(&m).into_string();
        assert!(html.contains("Share your profile to room:8081"));
        assert!(html.contains("Create another profile for a new commons"));
        assert!(!html.contains("Add your profile"));
    }
}
