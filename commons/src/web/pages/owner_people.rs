//! The People tab on a member's own profile (decision records 0011, 0022
//! and 0026): who is waiting for a vouch on each commons the member is
//! listed on, each with a **Vouch** button; inviting someone; and vouching
//! by handle and key, for a commons without one-click vouching.
//!
//! The waiting lists arrive already fetched, as [`Lookup`]s in
//! [`OwnerPanel::waiting_lists`]; this file only shows them.

use maud::{html, Markup};

use super::owner::{tab_href, OwnerPanel};
use crate::canonical::short;
use crate::ramp::Tab;
use crate::vouch_list::{Lookup, Waiting};
use crate::web::layout::host_label;

/// People: once listed on a commons, who is waiting there, inviting and
/// vouching; before that, what to do to get listed. Vouching needs a listing
/// because a commons only accepts a vouch from someone it already lists
/// (ADR 0011).
pub(super) fn people_tab(handle: &str, o: &OwnerPanel) -> Markup {
    if !o.state().listed_on().is_empty() {
        return html! { (waiting_panel(handle, o)) (invite_panel(handle, o)) (vouch_by_key(handle, o)) };
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

/// Who is waiting for a vouch on each commons this member is listed on,
/// with what the page should say for each kind of answer.
fn waiting_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        section class="share-panel waiting-for-vouch" {
            h2 class="field-label" { "Waiting for a vouch" }
            // Privacy and care, said where the decision is made (ADR 0026).
            p class="hint" {
                "Only people listed on a commons can see who is waiting there. You see just what each person sent for review: their handle, their name and their key's fingerprint, not their profile."
            }
            p class="hint" {
                strong { "Vouch only for people you know." }
                " Before you do, check the fingerprint with them: their own node shows it while they wait."
            }
            @for (commons, lookup) in &o.waiting_lists {
                @let place = host_label(commons);
                h3 class="section-h" { (place) }
                @match lookup {
                    Lookup::Found(list) if list.is_empty() => {
                        p class="hint" { "Nobody is waiting on " (place) " right now." }
                    }
                    Lookup::Found(list) => {
                        ul class="pending-list" {
                            @for w in list { (waiting_person(handle, &o.csrf, commons, w)) }
                        }
                    }
                    Lookup::Refused(why) => {
                        p class="hint" role="status" { (place) " didn't show its waiting list: " (why) }
                    }
                    Lookup::NotOffered => {
                        p class="hint" { (place) " doesn't offer one-click vouching yet. You can still vouch by handle and key, below." }
                    }
                    Lookup::Unreachable => {
                        p class="hint" role="status" { "Couldn't reach " (place) " to see who's waiting. It may be offline, or this computer may not be on its network. Reload this page to try again." }
                    }
                }
            }
        }
    }
}

/// One person waiting: their name, handle and key fingerprint, and a Vouch
/// button. The form carries the member's CSRF token, like every form on the
/// node, so another website can't press it for them.
fn waiting_person(handle: &str, csrf: &str, commons: &str, w: &Waiting) -> Markup {
    html! {
        li class="pending-item" {
            span { strong { (w.name) } " · @" (w.handle) }
            span { "key " span class="mono" { (short(&w.key)) } }
            form method="post" action="/vouch" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="commons_url" value=(commons);
                input type="hidden" name="who" value=(w.handle);
                input type="hidden" name="key" value=(w.key);
                // Pink with ink text on the oxblood card (not ink on ink).
                button class="btn-solid" type="submit" { "Vouch for @" (w.handle) }
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

/// Vouch by typing a handle and key: for a commons without one-click
/// vouching, or someone who joined the list after the page loaded. Folded
/// away, since the list above covers most cases.
fn vouch_by_key(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        details class="advanced" {
            summary { "Vouch by handle and key" }
            form method="post" action="/vouch" class="share-form vouch-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(o.csrf);
                p class="hint" { "Enter the handle and key the person gives you, or that the commons' page shows under " strong { "Waiting for a vouch" } ". Only vouch for people you actually know." }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A member listed on one commons, with `lookup` as its waiting list.
    fn panel(lookup: Lookup) -> OwnerPanel {
        let room = "http://room:8081".to_string();
        OwnerPanel {
            csrf: "tok3n".into(),
            published: true,
            shareable: true,
            known: vec![room.clone()],
            shared_to: vec![room.clone()],
            has_recovery: true,
            rotation_pending: false,
            waiting_on: vec![],
            backup_done: true,
            key: "k".into(),
            tab: Tab::People,
            waiting_lists: vec![(room, lookup)],
        }
    }

    #[test]
    fn each_person_waiting_gets_a_vouch_button_with_the_csrf_token() {
        let amy = Waiting { handle: "amy".into(), name: "Amy Ng".into(), key: "ab".repeat(32) };
        let html = people_tab("tega", &panel(Lookup::Found(vec![amy]))).into_string();
        assert!(html.contains("Amy Ng") && html.contains("@amy"));
        assert!(html.contains("abababababab"), "the key's fingerprint is shown");
        assert!(html.contains("Vouch for @amy"));
        assert!(html.contains(r#"name="csrf" value="tok3n""#));
        assert!(html.contains(&format!(r#"name="key" value="{}""#, "ab".repeat(32))), "the button sends the full key");
        assert!(html.contains("not their profile"), "the page says what it shows and what it doesn't");
    }

    #[test]
    fn every_kind_of_answer_says_something() {
        let says = |l| people_tab("tega", &panel(l)).into_string();
        assert!(says(Lookup::Found(vec![])).contains("Nobody is waiting on room:8081"));
        assert!(says(Lookup::Unreachable).contains("Couldn't reach room:8081"));
        assert!(says(Lookup::NotOffered).contains("vouch by handle and key"));
        assert!(says(Lookup::Refused("Too old.".into())).contains("didn't show its waiting list: Too old."));
    }
}
