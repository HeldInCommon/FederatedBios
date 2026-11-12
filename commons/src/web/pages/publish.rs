//! The publish form: about you, then Sharing and rights (see `rights.rs`).

use maud::{html, Markup};

use super::parts::{page_head, section_label, terms_panel};
use super::rights::{rights_fields, RightsFor};
use crate::model::{Profile, Reach};

/// The publish form, prefilled from the current version: about you, then
/// Sharing and rights open and explained, then what travels with every copy.
pub fn publish_form(
    me: &str,
    csrf: &str,
    p: &Profile,
    reach: Reach,
    license: &str,
    signals: &[String],
    first_time: bool,
) -> Markup {
    let title = if first_time { "Say who you are" } else { "Update your profile" };
    html! {
        (page_head(&format!("Your profile \u{b7} @{me}"), title, Some("Then decide who can see it, and how it can be used.")))
        form method="post" action="/publish" class="stack-form rights-form" {
            input type="hidden" name="handle" value=(me);
            input type="hidden" name="csrf" value=(csrf);
            (section_label("About you", None))
            (profile_fields(p))
            (section_label("Sharing and rights", None))
            div class="rights-intro" {
                p class="rights-intro-title" { "Who can see it, and how it can be used." }
                p { "These three choices are signed with your key and travel with every copy you share. Change them any time: the next version carries the new terms. Good defaults are already chosen." }
            }
            (rights_fields(RightsFor::Profile, reach, license, signals))
            (section_label("What travels with every copy", None))
            (terms_panel(reach, license, signals, true))
            aside class="caution" {
                p {
                    strong { "Publishing cannot be fully undone." }
                    " Each version gets a permanent content address. You can withdraw copies you share to a commons, but anyone who already downloaded one keeps it."
                }
            }
            div class="actions" {
                button class="btn-action" type="submit" { "Publish" }
                span class="hint" { "It stays on this computer. Sharing comes next." }
            }
        }
        script src="/avatar.js" {}
        script src="/rights.js" {}
    }
}

/// The "About you" fields: photo, name, pronouns, role, bio, where you work.
fn profile_fields(p: &Profile) -> Markup {
    let place = p.places.first();
    html! {
        div class="field" {
            label class="field-label" { "Profile picture" }
            div class="photo-field" {
                @if p.avatar.is_empty() {
                    img id="avatar-preview" class="avatar-preview empty" alt="";
                } @else {
                    img id="avatar-preview" class="avatar-preview" src=(p.avatar) alt="Current profile picture";
                }
                div class="photo-pick" {
                    input id="photo" type="file" accept="image/png,image/jpeg,image/webp";
                    p class="hint" { "A square photo works best — it's resized to about 320px on your device and saved with your profile." }
                    button id="avatar-clear" type="button" class="link-btn" hidden[p.avatar.is_empty()] { "Remove photo" }
                }
            }
            input id="avatar" type="hidden" name="avatar" value=(p.avatar);
        }
        div class="field-row name-row" {
            div class="field" {
                label class="field-label" for="name" { "Name" }
                input id="name" name="name" type="text" maxlength="120" value=(p.name) placeholder="Phoenix Perry";
            }
            div class="field" {
                label class="field-label" for="pronouns" { "Pronouns" }
                input id="pronouns" name="pronouns" type="text" maxlength="40" value=(p.pronouns) placeholder="she/her";
            }
        }
        div class="field" {
            label class="field-label" for="role" { "Role and organisation" }
            input id="role" name="role" type="text" maxlength="160" value=(p.role) placeholder="Artist & researcher, UNSW";
        }
        div class="field" {
            label class="field-label" for="bio" { "Short bio" }
            textarea id="bio" name="bio" maxlength="600" placeholder="A couple of sentences about your work." { (p.bio) }
        }
        div class="field" {
            label class="field-label" for="place" { "Where you work" }
            input id="place" name="place_name" type="text" maxlength="120" value=(place.map(|x| x.name.as_str()).unwrap_or("")) placeholder="Studio or institution";
            input id="place_city" name="place_city" type="text" maxlength="80" value=(place.map(|x| x.city.as_str()).unwrap_or("")) placeholder="City" class="city-input";
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caution_no_longer_promises_withdrawal() {
        let m = publish_form("tega", "x", &Profile::default(), Reach::Public, "cc-by", &[], true).into_string();
        assert!(m.contains("anyone who already downloaded"));
        assert!(!m.contains("stops serving"));
    }
}
