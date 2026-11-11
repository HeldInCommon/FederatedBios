//! One story on its author's node: `GET /story/{id}` (with a confirmation of
//! what just happened), `GET /story/{id}/text` (export), `POST /story/share`
//! and `POST /story/withdraw`. Writing and changing a story are in
//! `story_edit.rs`.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use super::{authorised_as, post_to_commons, session};
use crate::app::{AppState, Shared};
use crate::form::Form;
use crate::model::{HandleRec, Reach};
use crate::now;
use crate::sessions::Session;
use crate::story::{self, OwnStory};
use crate::web::layout::{error_page, host_label, not_found, redirect, render, server_error, Page};
use crate::web::pages::{self, StoryOwner};

/// May this viewer read a story on this node? Members-only stories are for
/// signed-in members only.
fn may_read(st: &AppState, headers: &HeaderMap, s: &OwnStory) -> bool {
    s.story.reach() != Reach::Members || session(st, headers).is_some()
}

/// What just happened to a story, shown once at the top of its page
/// (`?done=shared|withdrawn&at=<commons host>`, or `?done=saved`).
#[derive(serde::Deserialize)]
pub struct DoneQ {
    /// `shared`, `withdrawn` or `saved`.
    pub done: Option<String>,
    /// The commons' host, for shared and withdrawn (checked before it's shown).
    pub at: Option<String>,
}

/// The confirmation to show for `q`, if it's one we send: a known outcome
/// and, where it names one, a plausible host (letters, digits, `.:-`).
fn outcome(q: &DoneQ) -> Option<pages::StoryOutcome> {
    let at = q.at.as_deref().filter(|a| {
        !a.is_empty() && a.len() <= 100 && a.chars().all(|c| c.is_ascii_alphanumeric() || ".:-".contains(c))
    });
    match (q.done.as_deref(), at) {
        (Some("shared"), Some(at)) => Some(pages::StoryOutcome::Shared(at.to_string())),
        (Some("withdrawn"), Some(at)) => Some(pages::StoryOutcome::Withdrawn(at.to_string())),
        (Some("saved"), _) => Some(pages::StoryOutcome::Saved),
        _ => None,
    }
}

/// The commons a member can share to in one click: the node's default, then
/// those their profile is shared to.
pub(crate) fn known_commons_for(st: &AppState, rec: &HandleRec) -> Vec<String> {
    pages::known_commons(&st.site.share_default, &rec.shared_to)
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
        Some(m) if m.handle == s.story.handle => st
            .store
            .handle(&m.handle)
            .ok()
            .flatten()
            .map(|rec| StoryOwner { csrf: m.csrf.clone(), known: known_commons_for(&st, &rec) }),
        _ => None,
    };
    let done = owner.as_ref().and_then(|_| outcome(&q));
    let page = pages::own_story(&s, owner.as_ref(), done.as_ref(), now());
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

/// The story `id`, its author's record and session, if the signed-in member
/// wrote it (for pages; forms also check the CSRF token, see
/// [`authors_story`]).
///
/// Rust note: the error is a whole page to send back (`Response`), which is
/// large. `Box` keeps it on the heap so this `Result` stays small (clippy's
/// `result_large_err`); callers take the page back out with `*resp`.
pub(crate) fn owned_story(
    st: &AppState,
    headers: &HeaderMap,
    id: &str,
) -> Result<(OwnStory, HandleRec, Session), Box<Response>> {
    // `let … else`: if there is no session, leave here (sign in first).
    let Some(sess) = session(st, headers) else { return Err(Box::new(redirect("/login"))) };
    // Only the author: anyone else gets "not found", not a hint it exists.
    let s = match st.store.own_story(id) {
        Ok(Some(s)) if s.story.handle == sess.handle => s,
        Ok(_) => return Err(Box::new(not_found(&st.site, Some(&sess.handle)))),
        Err(e) => return Err(Box::new(server_error(&st.site, &e))),
    };
    match st.store.handle(&s.story.handle) {
        Ok(Some(r)) => Ok((s, r, sess)),
        Ok(None) => Err(Box::new(error_page(&st.site, "Unknown handle."))),
        Err(e) => Err(Box::new(server_error(&st.site, &e))),
    }
}

/// The story `id` and its author's record, if the form comes from its
/// signed-in author (session and CSRF token).
pub(crate) fn authors_story(
    st: &AppState,
    headers: &HeaderMap,
    f: &Form,
    id: &str,
) -> Result<(OwnStory, HandleRec), Box<Response>> {
    let s = match st.store.own_story(id) {
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

/// Apply `change` to story `id` and save it, under the write lock, re-reading
/// it first so two requests can't undo each other. **Writes the story file.**
///
/// Rust note: `change` is any function called once with the story that
/// returns the changed one, usually a closure written at the call site
/// (`|old| old.with_shared_to(…)`). That's what `impl FnOnce(OwnStory) ->
/// OwnStory` means.
pub(crate) fn update_story(st: &AppState, id: &str, change: impl FnOnce(OwnStory) -> OwnStory) -> Result<(), String> {
    // `_g` holds the write lock until this function ends; nothing else can
    // change a record in between our read and our write.
    let _g = st.write.lock().unwrap();
    // `?` hands an error straight back to the caller; `ok_or` turns
    // "no such story" into one.
    let s = st.store.own_story(id).map_err(|e| e.to_string())?.ok_or("the story is gone")?;
    st.store.save_own_story(&change(s)).map_err(|e| e.to_string())
}

/// Record that `h`'s stories are no longer on `only` (or on any commons, with
/// `None`). Used when a commons drops them: the member withdrew their profile
/// there, or replaced their key (a commons takes down stories signed by a
/// replaced key). **Writes story files.**
pub(crate) fn forget_shares(st: &AppState, h: &str, only: Option<&str>) -> Result<(), String> {
    let _g = st.write.lock().unwrap();
    for s in st.store.own_stories().into_iter().filter(|s| s.story.handle == h && !s.shared_to.is_empty()) {
        let next = match only {
            Some(c) => s.without_shared_to(c),
            None => OwnStory { shared_to: vec![], shared_at: Default::default(), ..s },
        };
        st.store.save_own_story(&next).map_err(|e| format!("couldn't update a story of @{h}: {e}"))?;
    }
    Ok(())
}

/// Send story `s` to `commons` and, if it's taken, record when. After a key
/// change the story is signed again with the member's current key first.
/// **Writes the story file.** The error is a sentence to show the member.
pub(crate) async fn share_story_to(
    st: &AppState,
    mut s: OwnStory,
    rec: &HandleRec,
    commons: &str,
) -> Result<(), String> {
    if s.story.reach() == Reach::Members {
        return Err("Members-only stories stay on your node.".into());
    }
    if !(commons.starts_with("http://") || commons.starts_with("https://")) {
        return Err("That isn't a commons address.".into());
    }
    if s.signed.envelope.author != rec.ed_pub {
        let signed = story::sign(&s.story, &rec.ed_pub, &rec.ed_priv, s.next_version_at(now()))?;
        s.signed = signed.clone();
        update_story(st, &s.story.id, |old| OwnStory { signed, ..old })?;
    }
    let label = host_label(commons);
    let r = post_to_commons(st, commons, &story::share_message(&s.signed)).await?;
    if !r.status().is_success() {
        let why = r.text().await.unwrap_or_default();
        return Err(format!("{label} didn't take the story: {why}"));
    }
    update_story(st, &s.story.id, |old| old.with_shared_to(commons, now()))
        .map_err(|e| format!("Shared to {label}, but this node couldn't record it: {e}"))
}

/// `POST /story/share` — send the signed story to a commons.
pub async fn story_share(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let (s, rec) = match authors_story(&st, &headers, &f, &f.one("id")) {
        Ok(x) => x,
        Err(resp) => return *resp,
    };
    let commons = f.one("commons_url").trim().trim_end_matches('/').to_string();
    let id = s.story.id.clone();
    match share_story_to(&st, s, &rec, &commons).await {
        Ok(()) => redirect(&format!("/story/{id}?done=shared&at={}", host_label(&commons))),
        Err(e) => error_page(&st.site, &e),
    }
}

/// `POST /story/withdraw` — ask a commons to delete its copy of the story.
pub async fn story_withdraw(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let (s, _rec) = match authors_story(&st, &headers, &f, &f.one("id")) {
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
        // 409: the commons no longer has it (already withdrawn): nothing left to undo there.
        Ok(r) if r.status().is_success() || r.status() == StatusCode::CONFLICT => {
            // The commons won't take the withdrawn copy back, so sign a new
            // version, dated after the withdrawal, for sharing it again later.
            let resigned = story::sign(&s.story, &rec.ed_pub, &rec.ed_priv, s.next_version_at(withdrawn_at + 1));
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
