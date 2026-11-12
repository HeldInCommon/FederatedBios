//! Writing and changing a story on a node, in two steps (decision record
//! 0022): `GET|POST /stories/new` and `GET|POST /story/{id}/edit` for the
//! words, then `GET|POST /story/{id}/rights` for who can read it, its
//! licence and signals, ending in "Share to the room" or "Keep it on this
//! computer for now". Every change is a new signed version with the same id.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;

use super::story::{authors_story, known_commons_for, owned_story, share_story_to, update_story};
use super::{authorised_as, session};
use crate::app::Shared;
use crate::form::Form;
use crate::governance::DEFAULT_LICENSE;
use crate::now;
use crate::sessions;
use crate::story::{self, OwnStory, TEXT_MAX, TITLE_MAX};
use crate::web::layout::{error_page, host_label, not_found, redirect, render, server_error, Page, Section};
use crate::web::pages;

/// The terms a new story starts with, as on the design: listed in the
/// commons (the room can read it, search engines are asked not to), CC BY,
/// and the Credit signal. The rights step comes next to change them.
const NEW_STORY_REACH: &str = "commons";
/// The signals a new story starts with (see above).
const NEW_STORY_SIGNALS: [&str; 1] = ["credit"];

/// `GET /stories/new` — step one: write it.
pub async fn story_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if st.site.is_commons() {
        return not_found(&st.site, None);
    }
    let Some(s) = session(&st, &headers) else { return redirect("/login") };
    render(
        &st.site,
        Page::new("Write a story", Some(&s.handle), pages::write_form(&s.handle, &s.csrf, None)).at(Section::Stories),
    )
}

/// `POST /stories/new` — sign the story with the member's key, keep it on
/// this node, and go on to its rights. Nothing is sent anywhere. A form that
/// carries its own terms (`visibility`, `license`, `signals`) keeps them.
pub async fn story_create(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    if st.site.is_commons() {
        return not_found(&st.site, None);
    }
    let f = Form::parse(&body);
    let h = crate::handle::normalise(&f.one("handle"));
    if !authorised_as(&st, &headers, &f, &h) {
        return error_page(&st.site, "Please sign in to write a story.");
    }
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return error_page(&st.site, "Unknown handle."),
        Err(e) => return server_error(&st.site, &e),
    };
    let (visibility, license, signals) = if f.one("visibility").is_empty() {
        let defaults: Vec<String> = NEW_STORY_SIGNALS.iter().map(|s| s.to_string()).collect();
        (NEW_STORY_REACH.to_string(), DEFAULT_LICENSE.to_string(), defaults)
    } else {
        (f.one("visibility"), f.one("license"), f.many("signals"))
    };
    // Clamped a little past the limits so new_story can say what was too long.
    let draft = story::new_story(
        &sessions::random_hex(8),
        &h,
        &f.clamped("title", TITLE_MAX + 1),
        &f.clamped("text", TEXT_MAX + 1),
        &visibility,
        &license,
        &signals,
    );
    let s = match draft {
        Ok(s) => s,
        Err(why) => return error_page(&st.site, why),
    };
    let signed = match story::sign(&s, &rec.ed_pub, &rec.ed_priv, now()) {
        Ok(x) => x,
        Err(e) => return server_error(&st.site, &e),
    };
    let own = OwnStory { story: s, signed, shared_to: vec![], shared_at: Default::default() };
    let saved = {
        let _g = st.write.lock().unwrap();
        st.store.save_own_story(&own)
    };
    match saved {
        Ok(()) => redirect(&format!("/story/{}/rights", own.story.id)),
        Err(e) => server_error(&st.site, &e),
    }
}

/// `GET /story/{id}/edit` — change the words of a story.
pub async fn story_edit_form(State(st): State<Shared>, Path(id): Path<String>, headers: HeaderMap) -> Response {
    let (s, _rec, sess) = match owned_story(&st, &headers, &id) {
        Ok(x) => x,
        // `*resp` takes the page out of its `Box` (see `owned_story`).
        Err(resp) => return *resp,
    };
    let body = pages::write_form(&sess.handle, &sess.csrf, Some(&s.story));
    render(&st.site, Page::new("Edit story", Some(&sess.handle), body).at(Section::Stories))
}

/// `POST /story/{id}/edit` — save the new words as a new signed version.
/// Copies already shared stay the earlier version until re-shared.
pub async fn story_edit(
    State(st): State<Shared>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let f = Form::parse(&body);
    let (s, rec) = match authors_story(&st, &headers, &f, &id) {
        Ok(x) => x,
        Err(resp) => return *resp,
    };
    let revised = match s.story.revised(&f.clamped("title", TITLE_MAX + 1), &f.clamped("text", TEXT_MAX + 1)) {
        Ok(r) => r,
        Err(why) => return error_page(&st.site, why),
    };
    let signed = match story::sign(&revised, &rec.ed_pub, &rec.ed_priv, s.next_version_at(now())) {
        Ok(x) => x,
        Err(e) => return server_error(&st.site, &e),
    };
    match update_story(&st, &id, |old| old.with_version(revised, signed)) {
        Ok(()) => redirect(&format!("/story/{id}?done=saved")),
        Err(e) => server_error(&st.site, &e),
    }
}

/// `GET /story/{id}/rights` — step two: who can read it, and how.
pub async fn rights_form(State(st): State<Shared>, Path(id): Path<String>, headers: HeaderMap) -> Response {
    let (s, rec, sess) = match owned_story(&st, &headers, &id) {
        Ok(x) => x,
        Err(resp) => return *resp,
    };
    let suggested = known_commons_for(&st, &rec).into_iter().next();
    let body = pages::rights_form(&s.story, &sess.csrf, suggested.as_deref());
    render(&st.site, Page::new("Who can read it", Some(&sess.handle), body).at(Section::Stories))
}

/// `POST /story/{id}/rights` — save the terms (a new signed version if they
/// changed), then share to the suggested commons or keep it here.
pub async fn rights_submit(
    State(st): State<Shared>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let f = Form::parse(&body);
    let (s, rec) = match authors_story(&st, &headers, &f, &id) {
        Ok(x) => x,
        Err(resp) => return *resp,
    };
    let terms = s.story.with_terms(&f.one("visibility"), &f.one("license"), &f.many("signals"));
    let s = if terms == s.story {
        s
    } else {
        let signed = match story::sign(&terms, &rec.ed_pub, &rec.ed_priv, s.next_version_at(now())) {
            Ok(x) => x,
            Err(e) => return server_error(&st.site, &e),
        };
        if let Err(e) = update_story(&st, &id, |old| old.with_version(terms.clone(), signed.clone())) {
            return server_error(&st.site, &e);
        }
        s.with_version(terms, signed)
    };
    if f.one("action") != "share" {
        return redirect(&format!("/story/{id}?done=saved"));
    }
    // Only to a commons this member is offered, never one a form makes up.
    let commons = f.one("commons_url").trim_end_matches('/').to_string();
    if !known_commons_for(&st, &rec).contains(&commons) {
        return error_page(&st.site, "Share your profile to that commons first.");
    }
    match share_story_to(&st, s, &rec, &commons).await {
        Ok(()) => redirect(&format!("/story/{id}?done=shared&at={}", host_label(&commons))),
        Err(e) => error_page(&st.site, &e),
    }
}
