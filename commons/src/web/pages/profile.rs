//! A member's profile page.

use maud::{html, Markup};

use super::owner::{next_step_block, profile_actions, tab_nav, tab_panel, OwnerPanel};
use crate::canonical::short;
use crate::model::{display_name_or_handle, HandleRec, Profile, Version};
use crate::ramp::Tab;
use crate::web::layout::{gov_row, reach_badge};

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

/// The profile itself: what anyone allowed to read it sees.
fn profile_content(rec: &HandleRec, version: Option<&Version>, readable: Option<&Profile>) -> Markup {
    html! {
        @match version {
            None => {
                header class="intro" {
                    h1 { "@" (rec.handle) }
                    p class="lede" { "This creator has not published a profile yet." }
                }
            }
            Some(v) => {
                header class="intro profile-head" {
                    (reach_badge(v.reach()))
                    @match readable {
                        Some(p) => {
                            @if !p.avatar.is_empty() { img class="profile-avatar" src=(p.avatar) alt=""; }
                            h1 { (display_name_or_handle(&p.name, &rec.handle)) }
                            @if !p.pronouns.is_empty() { p class="pronouns" { (p.pronouns) } }
                            @if !p.role.is_empty() { p class="lede" { (p.role) } }
                        }
                        None => {
                            h1 { "@" (rec.handle) }
                            p class="lede" { "This profile is members-only and encrypted to the member set. Sign in as a member to read it." }
                        }
                    }
                }
                @if let Some(p) = readable { (profile_details(p)) }
                section class="profile-meta" {
                    (gov_row(&v.license, &v.signals))
                    p class="cid" { "Version " span class="mono" { (short(&v.cid)) } " · permanent content address" }
                }
            }
        }
    }
}

/// Bio and places, shared by member and shared-in profile pages.
pub(super) fn profile_details(p: &Profile) -> Markup {
    html! {
        @if !p.bio.is_empty() { section class="profile-bio" { p { (p.bio) } } }
        @if !p.places.is_empty() {
            section class="profile-places" {
                h2 class="section-h" { "Where they work" }
                ul {
                    @for pl in &p.places {
                        li { (pl.name) @if !pl.city.is_empty() { span class="place-city" { " · " (pl.city) } } }
                    }
                }
            }
        }
    }
}
