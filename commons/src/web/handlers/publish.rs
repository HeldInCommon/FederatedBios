//! The publish form and publishing a new version.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;

use super::{authorised_as, readable_profile, session};
use crate::app::Shared;
use crate::form::Form;
use crate::governance::{license_from_form, signals_from_form, DEFAULT_LICENSE};
use crate::held_signals::HeldSignals;
use crate::model::{Place, Profile, Reach};
use crate::publishing::{self, Draft};
use crate::web::layout::{error_page, redirect, render, server_error, Page, Section};
use crate::web::pages::{self, Chosen};
use crate::{handle, now};

/// Largest accepted profile picture, as a data URI, in bytes.
const AVATAR_MAX: usize = 600_000;

/// The submitted picture if it is a small JPEG, PNG or WebP data URI;
/// otherwise empty (no picture).
pub fn clean_avatar(a: &str) -> String {
    let ok = ["data:image/jpeg;base64,", "data:image/png;base64,", "data:image/webp;base64,"]
        .iter()
        .any(|p| a.starts_with(p));
    if ok && a.len() <= AVATAR_MAX {
        a.to_string()
    } else {
        String::new()
    }
}

/// The profile described by a publish form.
pub fn profile_from_form(f: &Form) -> Profile {
    let place_name = f.clamped("place_name", 120);
    let places = if place_name.is_empty() {
        vec![]
    } else {
        vec![Place { name: place_name, city: f.clamped("place_city", 80) }]
    };
    Profile {
        name: f.clamped("name", 120),
        pronouns: f.clamped("pronouns", 40),
        role: f.clamped("role", 160),
        bio: f.clamped("bio", 600),
        places,
        avatar: clean_avatar(&f.one("avatar")),
    }
}

/// `GET /new` — the publish form for the signed-in member.
pub async fn publish_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let Some(s) = session(&st, &headers) else { return redirect("/login") };
    let rec = match st.store.handle(&s.handle) {
        Ok(Some(r)) => r,
        Ok(None) => return redirect("/login"),
        Err(e) => return server_error(&st.site, &e),
    };
    let current = match st.store.current_version(&rec) {
        Ok(v) => v,
        Err(e) => return server_error(&st.site, &e),
    };
    let body = match &current {
        Some(v) => {
            let p = readable_profile(&st, v, Some(&s.handle)).unwrap_or_default();
            let chosen =
                Chosen { reach: v.reach(), license: &v.license, signals: &v.signals, held_signals: &v.held_signals };
            pages::publish_form(&s.handle, &s.csrf, &p, &chosen, false)
        }
        None => {
            // A first profile starts from good defaults: public, CC BY, no
            // signals, no Held in Common Signals.
            let none = HeldSignals::default();
            let chosen = Chosen { reach: Reach::Public, license: DEFAULT_LICENSE, signals: &[], held_signals: &none };
            pages::publish_form(&s.handle, &s.csrf, &Profile::default(), &chosen, true)
        }
    };
    render(&st.site, Page::new("Publish", Some(&s.handle), body).at(Section::You))
}

/// `POST /publish` — sign and store a new version, and make it current.
pub async fn publish(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(&st, &headers, &f, &h) {
        return error_page(&st.site, "Please sign in as this handle to publish.");
    }
    let profile = profile_from_form(&f);
    let reach = Reach::from_form(&f.one("visibility"));
    let license = license_from_form(&f.one("license"));
    let signals = signals_from_form(&f.many("signals"));
    // Held in Common Signals (decision record 0025): too-long own terms are
    // refused with a plain message, never cut.
    let held_signals = match HeldSignals::from_form(&f.one("withdraw_after_training"), &f.one("own_terms")) {
        Ok(h) => h,
        Err(why) => return error_page(&st.site, why),
    };
    let created_at = now();

    let _g = st.write.lock().unwrap();
    let mut rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return error_page(&st.site, "Unknown handle."),
        Err(e) => return server_error(&st.site, &e),
    };
    let recipients: Vec<(String, String)> = st.store.handles().into_iter().map(|m| (m.handle, m.box_pub)).collect();
    let draft = Draft { profile, reach, license, signals, created_at, held_signals };
    let version = match publishing::sign_version(&rec, draft, &recipients) {
        Ok(v) => v,
        Err(e) => return server_error(&st.site, &format!("@{h}: {e}")),
    };
    if let Err(e) = st.store.save_version(&version) {
        return server_error(&st.site, &e);
    }
    rec.current_cid = Some(version.cid.clone());
    if let Err(e) = st.store.save_handle(&rec) {
        return server_error(&st.site, &e);
    }
    redirect(&format!("/@{h}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_must_be_a_small_image_data_uri() {
        assert_eq!(clean_avatar("data:image/png;base64,AAAA"), "data:image/png;base64,AAAA");
        assert_eq!(clean_avatar("javascript:alert(1)"), "");
        assert_eq!(clean_avatar("data:image/svg+xml;base64,AAAA"), "");
        assert_eq!(clean_avatar(&format!("data:image/png;base64,{}", "A".repeat(AVATAR_MAX))), "");
    }

    #[test]
    fn profile_from_form_clamps_and_skips_empty_place() {
        let f = Form::parse(&format!("name={}&place_name=&bio=hi", "x".repeat(200)));
        let p = profile_from_form(&f);
        assert_eq!(p.name.len(), 120);
        assert!(p.places.is_empty());
        assert_eq!(p.bio, "hi");
    }
}
