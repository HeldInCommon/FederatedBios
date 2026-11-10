//! A profile shared in from another node, with where it came from.

use maud::{html, Markup};

use super::origin_host;
use super::profile::profile_details;
use crate::canonical::short;
use crate::model::{display_name_or_handle, RemoteEntry};
use crate::web::layout::gov_row;

/// A profile shared in from another node, with where it came from.
pub fn federated(r: &RemoteEntry) -> Markup {
    let p = &r.profile;
    html! {
        header class="intro profile-head" {
            span class="vis-badge vis-federated" { "Federated" }
            @if !p.avatar.is_empty() { img class="profile-avatar" src=(p.avatar) alt=""; }
            h1 { (display_name_or_handle(&p.name, &r.handle)) }
            @if !p.pronouns.is_empty() { p class="pronouns" { (p.pronouns) } }
            @if !p.role.is_empty() { p class="lede" { (p.role) } }
        }
        (profile_details(p))
        section class="profile-meta" {
            (gov_row(&r.license, &r.signals))
            div class="origin-box" {
                p class="origin-line" {
                    "Shared from "
                    @if r.origin.starts_with("http") { a href=(r.origin) { (origin_host(r)) } } @else { strong { (origin_host(r)) } }
                    @if !r.origin_label.is_empty() { span class="origin-label" { " — the node calls itself “" (r.origin_label) "”" } }
                }
                p class="origin-key" { "Signed by key " span class="mono" { (short(&r.ed_pub)) } }
                @if r.signed.is_some() {
                    p class="origin-note" { "On this commons, @" (r.handle) " belongs to that key alone. Everything on this page is signed by it, so nobody could change it on the way here. The address and node name are still what the node says about itself." }
                } @else {
                    p class="origin-note" { "On this commons, @" (r.handle) " belongs to that key alone. Shared in the older format: the profile text is signed, but the picture, name, address and node name are not." }
                }
            }
            p class="cid" { "Version " span class="mono" { (short(&r.cid)) } }
        }
    }
}
