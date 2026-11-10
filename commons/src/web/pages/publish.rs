//! The publish form: profile fields, who can see it, licence and signals.

use maud::{html, Markup};

use crate::governance::{LICENSES, SIGNALS};
use crate::model::{Profile, Reach};

/// The publish form, prefilled from the current version.
pub fn publish_form(
    me: &str,
    csrf: &str,
    p: &Profile,
    reach: Reach,
    license: &str,
    signals: &[String],
    first_time: bool,
) -> Markup {
    html! {
        header class="intro" {
            h1 { @if first_time { "Your profile" } @else { "Update your profile" } }
            p class="lede" {
                @if first_time { "You're signed in as @" (me) ". Fill this in and publish it to the commons." }
                @else { "Signed in as @" (me) ". Change anything and republish — each version gets its own content address." }
            }
        }
        form method="post" action="/publish" class="stack-form" {
            input type="hidden" name="handle" value=(me);
            input type="hidden" name="csrf" value=(csrf);
            (profile_fields(p))
            (reach_and_governance(reach, license, signals))
            div class="actions" { button class="primary" type="submit" { "Publish to the commons" } }
        }
        script src="/avatar.js" {}
    }
}

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

fn reach_and_governance(sel: Reach, sel_license: &str, sel_sig: &[String]) -> Markup {
    html! {
        fieldset class="reach-options" {
            legend class="field-label" { "Who can see this" }
            @for (val, title, desc) in [
                (Reach::Public, "Public", "Listed in the commons directory and open to search engines."),
                (Reach::Commons, "Listed in the commons", "In the directory; anyone with the link can read it, kept out of search."),
                (Reach::Members, "Members only", "Encrypted to the member set. Only signed-in members can read it."),
            ] {
                label class="reach" {
                    input type="radio" name="visibility" value=(val.as_str()) checked[val == sel];
                    span class="reach-text" {
                        span class="reach-title" { (title) }
                        span class="reach-body" { (desc) }
                    }
                }
            }
        }
        fieldset class="gov-options" {
            legend class="field-label" { "License" }
            select name="license" class="license-select" {
                @for (key, name, _) in LICENSES {
                    option value=(key) selected[*key == sel_license] { (name) }
                }
            }
        }
        fieldset class="gov-options" {
            legend class="field-label" { "Signals — how models may learn from this" }
            div class="signal-grid" {
                @for (key, name, desc) in SIGNALS {
                    label class="signal" {
                        input type="checkbox" name="signals" value=(key) checked[sel_sig.iter().any(|s| s == key)];
                        span { span class="signal-name" { (name) } span class="signal-desc" { (desc) } }
                    }
                }
            }
        }
        aside class="caution" {
            p {
                strong { "Publishing cannot be fully undone." }
                " Each version gets a permanent content address. You can withdraw copies you share to a commons, but anyone who already downloaded one keeps it."
            }
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
