//! A member's profile page.

use maud::{html, Markup};

use super::owner::{next_step_block, profile_actions, tab_nav, tab_panel, OwnerPanel};
use super::parts::{own_terms_note, reach_label, signals_line};
use crate::canonical::short;
use crate::governance::license_name;
use crate::held_signals::HeldSignals;
use crate::model::{display_name_or_handle, HandleRec, Profile, Reach, Version};
use crate::ramp::Tab;

/// A member's profile page. `readable` is the profile the viewer may read
/// (for members-only versions, only if it decrypted for them). The owner
/// also gets their next step and tabs (see [`crate::ramp`]).
pub fn profile(
    rec: &HandleRec,
    version: Option<&Version>,
    readable: Option<&Profile>,
    owner: Option<&OwnerPanel>,
) -> Markup {
    let Some(o) = owner else { return profile_content(rec, version, readable) };
    let on_profile = o.active() == Tab::Profile;
    html! {
        @if on_profile { (next_step_block(&rec.handle, o)) }
        (tab_nav(&rec.handle, o))
        @if on_profile {
            (profile_content(rec, version, readable))
            (profile_actions(o))
        } @else {
            (tab_panel(&rec.handle, o))
        }
    }
}

/// The profile itself, as anyone allowed to read it sees it.
fn profile_content(rec: &HandleRec, version: Option<&Version>, readable: Option<&Profile>) -> Markup {
    let Some(v) = version else {
        return html! {
            header class="intro" {
                h1 { "@" (rec.handle) }
                p class="lede" { "This creator has not published a profile yet." }
            }
        };
    };
    let terms = terms_line(v.reach(), &v.license, &v.signals, &v.held_signals);
    html! {
        (profile_header(&rec.handle, readable, &terms))
        @if let Some(p) = readable { (places_list(p)) }
        section class="profile-meta" {
            // Own terms are content: a members-only version shows them only to
            // those who can read it.
            @if readable.is_some() { (own_terms_note(&rec.handle, &v.held_signals)) }
            p class="cid" { "Version " span class="mono" { (short(&v.cid)) } " \u{b7} permanent content address" }
        }
    }
}

/// The terms a profile travels with, as one line ("Public · CC BY 4.0 ·
/// Signals: Credit · Held in Common: Withdraw even after training").
pub(super) fn terms_line(reach: Reach, license: &str, signals: &[String], held: &HeldSignals) -> String {
    let mut line = format!("{} \u{b7} {}", reach_label(reach), license_name(license));
    if !signals.is_empty() {
        line.push_str(&format!(" \u{b7} Signals: {}", signals_line(signals)));
    }
    if !held.is_empty() {
        line.push_str(&format!(" \u{b7} Held in Common: {}", held.summary()));
    }
    line
}

/// A profile's header: a square photo (or the first letter of the name), its
/// terms, the name, handle, pronouns and role, and the bio. `readable` is
/// `None` for a members-only profile the viewer can't decrypt.
pub(super) fn profile_header(handle: &str, readable: Option<&Profile>, terms: &str) -> Markup {
    let name = readable.map(|p| display_name_or_handle(&p.name, handle)).unwrap_or_else(|| format!("@{handle}"));
    let initial = name.trim_start_matches('@').chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    html! {
        section class="profile-head" {
            div class="profile-photo" {
                @match readable.filter(|p| !p.avatar.is_empty()) {
                    Some(p) => { img src=(p.avatar) alt=""; }
                    None => { span aria-hidden="true" { (initial) } }
                }
            }
            div class="profile-info" {
                p class="profile-terms" { (terms) }
                h1 class="profile-name" { (name) }
                @match readable {
                    Some(p) => {
                        p class="profile-handle" {
                            "@" (handle)
                            @if !p.pronouns.is_empty() { " \u{b7} " (p.pronouns) }
                            @if !p.role.is_empty() { " \u{b7} " (p.role) }
                        }
                        @if !p.bio.is_empty() { p class="profile-lead" { (p.bio) } }
                    }
                    None => {
                        p class="profile-lead" { "This profile is members-only and encrypted to the member set. Sign in as a member to read it." }
                    }
                }
            }
        }
    }
}

/// Where someone works, if they said.
pub(super) fn places_list(p: &Profile) -> Markup {
    html! {
        @if !p.places.is_empty() {
            section class="profile-places" {
                h2 class="section-h" { "Where they work" }
                ul {
                    @for pl in &p.places {
                        li { (pl.name) @if !pl.city.is_empty() { span class="place-city" { " \u{b7} " (pl.city) } } }
                    }
                }
            }
        }
    }
}
