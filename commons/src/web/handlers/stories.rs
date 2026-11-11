//! Stories (see `crate::story`).
//!
//! On a node: `GET /stories`, `GET|POST /stories/new`, `GET /story/{id}`,
//! `GET /story/{id}/text` (export), `POST /story/share`, `POST /story/withdraw`.
//! On a commons: `GET /stories`, `GET /stories/{keyid}/{id}`.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use super::{authorised_as, post_to_commons, session};
use crate::app::{AppState, Shared};
use crate::form::Form;
use crate::model::Reach;
use crate::story::{self, OwnStory, TEXT_MAX, TITLE_MAX};
use crate::web::layout::{error_page, host_label, not_found, redirect, render, server_error, Page};
use crate::web::pages::{self, StoryOwner};
use crate::{now, sessions};

/// May this viewer read a story on this node? Members-only stories are for
/// signed-in members only.
fn may_read(st: &AppState, headers: &HeaderMap, s: &OwnStory) -> bool {
    s.story.reach() != Reach::Members || session(st, headers).is_some()
}

/// `GET /stories` — a node's stories, or the stories shared to a commons.
pub async fn stories(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    if st.site.is_commons() {
        let held = st.store.held_stories();
        let noindex = held.iter().any(|h| h.story.reach() == Reach::Commons);
        return render(&st.site, Page::new("Stories", me.as_deref(), pages::commons_stories(&held)).noindex(noindex));
    }
    let visible: Vec<OwnStory> =
        st.store.own_stories().into_iter().filter(|s| me.is_some() || s.story.reach() != Reach::Members).collect();
    render(&st.site, Page::new("Stories", me.as_deref(), pages::node_stories(&visible, me.is_some())))
}

/// `GET /stories/new` — the writing form.
pub async fn story_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if st.site.is_commons() {
        return not_found(&st.site, None);
    }
    let Some(s) = session(&st, &headers) else { return redirect("/login") };
    render(&st.site, Page::new("Write a story", Some(&s.handle), pages::story_form(&s.handle, &s.csrf)))
}

/// `POST /stories/new` — sign the story with the member's key and keep it on
/// this node. Nothing is sent anywhere.
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
    // Clamped a little past the limits so new_story can say what was too long.
    let draft = story::new_story(
        &sessions::random_hex(8),
        &h,
        &f.clamped("title", TITLE_MAX + 1),
        &f.clamped("text", TEXT_MAX + 1),
        &f.one("visibility"),
        &f.one("license"),
        &f.many("signals"),
    );
    let s = match draft {
        Ok(s) => s,
        Err(why) => return error_page(&st.site, why),
    };
    let signed = match story::sign(&s, &rec.ed_pub, &rec.ed_priv, now()) {
        Ok(x) => x,
        Err(e) => return server_error(&st.site, &e),
    };
    let own = OwnStory { story: s, signed, shared_to: vec![] };
    let saved = {
        let _g = st.write.lock().unwrap();
        st.store.save_own_story(&own)
    };
    match saved {
        Ok(()) => redirect(&format!("/story/{}", own.story.id)),
        Err(e) => server_error(&st.site, &e),
    }
}

/// What just happened to a story, shown once at the top of its page
/// (`?done=shared|withdrawn&at=<commons host>`).
#[derive(serde::Deserialize)]
pub struct DoneQ {
    pub done: Option<String>,
    pub at: Option<String>,
}

/// The confirmation to show for `q`, if it's one we send: a known outcome and
/// a plausible host (letters, digits, dots, colons, hyphens).
fn outcome(q: &DoneQ) -> Option<pages::StoryOutcome> {
    let at = q.at.as_deref().filter(|a| {
        !a.is_empty() && a.len() <= 100 && a.chars().all(|c| c.is_ascii_alphanumeric() || ".:-".contains(c))
    })?;
    match q.done.as_deref() {
        Some("shared") => Some(pages::StoryOutcome::Shared(at.to_string())),
        Some("withdrawn") => Some(pages::StoryOutcome::Withdrawn(at.to_string())),
        _ => None,
    }
}

/// `GET /story/{id}` — one story on this node; its author also gets the
/// sharing tools.
pub async fn own_story(
    State(st): State<Shared>,
    Path(id): Path<String>,
    Query(q): Query<DoneQ>,
    headers: HeaderMap,
) -> Response {
    if st.site.is_commons() {
        return not_found(&st.site, None);
    }
    let me = session(&st, &headers);
    let s = match st.store.own_story(&id) {
        Ok(Some(s)) if may_read(&st, &headers, &s) => s,
        Ok(_) => return not_found(&st.site, me.as_ref().map(|m| m.handle.as_str())),
        Err(e) => return server_error(&st.site, &e),
    };
    let owner = match &me {
        Some(m) if m.handle == s.story.handle => {
            let shared_to = st.store.handle(&m.handle).ok().flatten().map(|r| r.shared_to).unwrap_or_default();
            Some(StoryOwner { csrf: m.csrf.clone(), known: pages::known_commons(&st.site.share_default, &shared_to) })
        }
        _ => None,
    };
    let done = owner.as_ref().and_then(|_| outcome(&q));
    let page = pages::own_story(&s, owner.as_ref(), done.as_ref());
    let me = me.map(|m| m.handle);
    render(&st.site, Page::new(&s.story.title, me.as_deref(), page).noindex(s.story.reach() == Reach::Commons))
}

/// `GET /story/{id}/text` — the story as a plain text file to keep.
pub async fn story_text(State(st): State<Shared>, Path(id): Path<String>, headers: HeaderMap) -> Response {
    if st.site.is_commons() {
        return not_found(&st.site, None);
    }
    match st.store.own_story(&id) {
        Ok(Some(s)) if may_read(&st, &headers, &s) => (
            [
                (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_string()),
                (header::CONTENT_DISPOSITION, format!("attachment; filename=\"story-{id}.txt\"")),
            ],
            s.story.as_text_file(),
        )
            .into_response(),
        Ok(_) => not_found(&st.site, None),
        Err(e) => server_error(&st.site, &e),
    }
}

/// Load the story a form names, if the session is its author's.
fn authors_story(
    st: &AppState,
    headers: &HeaderMap,
    f: &Form,
) -> Result<(OwnStory, crate::model::HandleRec), Box<Response>> {
    let s = match st.store.own_story(&f.one("id")) {
        Ok(Some(s)) => s,
        Ok(None) => return Err(Box::new(error_page(&st.site, "That story isn't on this node."))),
        Err(e) => return Err(Box::new(server_error(&st.site, &e))),
    };
    if !authorised_as(st, headers, f, &s.story.handle) {
        return Err(Box::new(error_page(&st.site, "Please sign in as the story's author.")));
    }
    match st.store.handle(&s.story.handle) {
        Ok(Some(r)) => Ok((s, r)),
        Ok(None) => Err(Box::new(error_page(&st.site, "Unknown handle."))),
        Err(e) => Err(Box::new(server_error(&st.site, &e))),
    }
}

/// Save a change to a story under the write lock, re-reading it first so two
/// requests can't undo each other.
fn update_story(st: &AppState, id: &str, change: impl FnOnce(OwnStory) -> OwnStory) -> Result<(), String> {
    let _g = st.write.lock().unwrap();
    let s = st.store.own_story(id).map_err(|e| e.to_string())?.ok_or("the story is gone")?;
    st.store.save_own_story(&change(s)).map_err(|e| e.to_string())
}

/// Record that `h`'s stories are no longer on `only` (or on any commons, with
/// `None`). Used when a commons drops them: the member withdrew their profile
/// there, or replaced their key (a commons takes down stories signed by a
/// replaced key). **Writes story files.**
pub(crate) fn forget_shares(st: &AppState, h: &str, only: Option<&str>) {
    let _g = st.write.lock().unwrap();
    for s in st.store.own_stories().into_iter().filter(|s| s.story.handle == h && !s.shared_to.is_empty()) {
        let next = match only {
            Some(c) => s.without_shared_to(c),
            None => OwnStory { shared_to: vec![], ..s },
        };
        if let Err(e) = st.store.save_own_story(&next) {
            eprintln!("warning: couldn't update a story of @{h}: {e}");
        }
    }
}

/// `POST /story/share` — send the signed story to a commons.
pub async fn story_share(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let (mut s, rec) = match authors_story(&st, &headers, &f) {
        Ok(x) => x,
        Err(resp) => return *resp,
    };
    if s.story.reach() == Reach::Members {
        return error_page(&st.site, "Members-only stories stay on your node.");
    }
    let commons = f.one("commons_url").trim().trim_end_matches('/').to_string();
    if !(commons.starts_with("http://") || commons.starts_with("https://")) {
        return error_page(&st.site, "That isn't a commons address.");
    }
    // After a key change the old signature no longer names the member's key:
    // sign again with the current one.
    if s.signed.envelope.author != rec.ed_pub {
        match story::sign(&s.story, &rec.ed_pub, &rec.ed_priv, now()) {
            Ok(signed) => s.signed = signed.clone(),
            Err(e) => return server_error(&st.site, &e),
        }
        if let Err(e) = update_story(&st, &s.story.id, |mut old| {
            old.signed = s.signed.clone();
            old
        }) {
            return server_error(&st.site, &e);
        }
    }
    let label = host_label(&commons);
    match post_to_commons(&st, &commons, &story::share_message(&s.signed)).await {
        Ok(r) if r.status().is_success() => {
            if let Err(e) = update_story(&st, &s.story.id, |old| old.with_shared_to(&commons)) {
                eprintln!("warning: shared, but couldn't record it: {e}");
            }
            redirect(&format!("/story/{}?done=shared&at={label}", s.story.id))
        }
        Ok(r) => {
            let why = r.text().await.unwrap_or_default();
            error_page(&st.site, &format!("{label} didn't take the story: {why}"))
        }
        Err(e) => error_page(&st.site, &e),
    }
}

/// `POST /story/withdraw` — ask a commons to delete its copy of the story.
pub async fn story_withdraw(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let (s, _rec) = match authors_story(&st, &headers, &f) {
        Ok(x) => x,
        Err(resp) => return *resp,
    };
    let commons = f.one("commons_url").trim_end_matches('/').to_string();
    if !s.shared_to.contains(&commons) {
        return error_page(&st.site, "This story hasn't been shared to that commons from here.");
    }
    // Signed by the key the commons holds the story under.
    let author = s.signed.envelope.author.clone();
    let Some(rec) = st.store.handle(&s.story.handle).ok().flatten().filter(|r| r.ed_pub == author) else {
        return error_page(&st.site, "Your key has changed since you shared this; share it again, then withdraw.");
    };
    let withdrawn_at = now();
    let msg = match story::build_withdraw(&rec.ed_pub, &rec.ed_priv, &s.story.id, &commons, withdrawn_at) {
        Ok(m) => m,
        Err(e) => return server_error(&st.site, &e),
    };
    let label = host_label(&commons);
    match post_to_commons(&st, &commons, &msg).await {
        Ok(r) if r.status().is_success() || r.status() == StatusCode::CONFLICT => {
            // 409: the commons no longer has it (already withdrawn) — nothing left to undo here.
            // The commons won't take the withdrawn copy back, so sign a new
            // version, dated after the withdrawal, for sharing it again later.
            let resigned = story::sign(&s.story, &rec.ed_pub, &rec.ed_priv, withdrawn_at + 1);
            if let Err(e) = update_story(&st, &s.story.id, |mut old| {
                if let Ok(signed) = resigned {
                    old.signed = signed;
                }
                old.without_shared_to(&commons)
            }) {
                eprintln!("warning: withdrawn, but couldn't record it: {e}");
            }
            redirect(&format!("/story/{}?done=withdrawn&at={label}", s.story.id))
        }
        Ok(r) => {
            let why = r.text().await.unwrap_or_default();
            error_page(&st.site, &format!("{label} didn't accept the withdrawal: {why}"))
        }
        Err(e) => error_page(&st.site, &e),
    }
}

/// `GET /stories/{keyid}/{id}` — a story shared to this commons.
pub async fn held_story(
    State(st): State<Shared>,
    Path((kid, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    if !st.site.is_commons() {
        return not_found(&st.site, me.as_deref());
    }
    match st.store.held_story(&kid, &id) {
        Ok(Some(h)) => {
            let noindex = h.story.reach() == Reach::Commons;
            render(&st.site, Page::new(&h.story.title, me.as_deref(), pages::held_story(&h)).noindex(noindex))
        }
        Ok(None) => not_found(&st.site, me.as_deref()),
        Err(e) => server_error(&st.site, &e),
    }
}
