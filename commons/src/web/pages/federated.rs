//! A profile shared in from another node, with where it came from.

use maud::{html, Markup};

use super::origin_host;
use super::profile::{places_list, profile_header, terms_line};
use crate::canonical::short;
use crate::model::{Reach, RemoteEntry};

/// A profile shared in from another node, with where it came from.
pub fn federated(r: &RemoteEntry) -> Markup {
    let p = &r.profile;
    let reach = Reach::parse(&r.visibility).unwrap_or(Reach::Public);
    html! {
        (profile_header(&r.handle, Some(p), &format!("Shared in \u{b7} {}", terms_line(reach, &r.license, &r.signals))))
        (places_list(p))
        section class="profile-meta" {
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
