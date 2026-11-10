//! Backing up an identity to an encrypted file, and restoring from one.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};

use super::{authorised_as, session};
use crate::app::Shared;
use crate::form::Form;
use crate::passphrase::{self, Check};
use crate::sessions;
use crate::web::layout::{error_page, redirect_with, render, server_error, Page};
use crate::web::pages;
use crate::{backup, handle, keys, now};

/// `POST /backup` — download an encrypted backup of the signed-in member's
/// identity. Asks for the passphrase again, which also encrypts the file.
pub async fn backup(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(&st, &headers, &f, &h) {
        return error_page(&st.site, "Please sign in as this handle to back it up.");
    }
    let pass = f.one("passphrase");
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return error_page(&st.site, "Unknown handle."),
        Err(e) => return server_error(&st.site, &e),
    };
    if passphrase::check(&rec.secret_salt, &rec.secret_hash, &pass) == Check::Wrong {
        return error_page(
            &st.site,
            "That passphrase is wrong. The backup is locked with your passphrase, so it must match.",
        );
    }
    let version = st.store.current_version(&rec).ok().flatten();
    let file = match backup::seal(&rec, version.as_ref(), &pass) {
        Ok(t) => t,
        Err(e) => return server_error(&st.site, &e),
    };
    let disposition = format!("attachment; filename=\"held-in-common-{h}-backup.json\"");
    (
        [
            (axum::http::header::CONTENT_TYPE, "application/json".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, disposition),
            (axum::http::header::CACHE_CONTROL, "no-store".to_string()),
        ],
        file,
    )
        .into_response()
}

/// `GET /restore` — the form for restoring an identity from a backup.
pub async fn restore_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    let body = if st.site.is_commons() { pages::commons_gate(&st.site) } else { pages::restore_form() };
    render(&st.site, Page::new("Restore", me.as_deref(), body))
}

/// `POST /restore` — bring an identity (and its current version) back from a
/// backup, then sign in.
pub async fn restore_submit(State(st): State<Shared>, body: String) -> Response {
    if st.site.is_commons() {
        return render(&st.site, Page::new("Shared commons", None, pages::commons_gate(&st.site)));
    }
    let f = Form::parse(&body);
    let pass = f.one("passphrase");
    let (mut rec, version) = match backup::open(&f.one("backup"), &pass) {
        Ok(x) => x,
        Err(e) => return error_page(&st.site, &e),
    };
    // Only bring back a version that really is this identity's, signed by its key.
    let version = version.filter(|v| {
        v.handle == rec.handle
            && Some(&v.cid) == rec.current_cid.as_ref()
            && keys::verify_cid(&rec.ed_pub, &v.cid, &v.signature).is_ok()
    });
    (rec.secret_salt, rec.secret_hash) = passphrase::hash(&pass);
    if version.is_none() {
        rec.current_cid = None;
    }
    {
        let _g = st.write.lock().unwrap();
        match st.store.handle(&rec.handle) {
            Ok(None) => {}
            Ok(Some(_)) => return error_page(&st.site, &format!("@{} already exists on this node.", rec.handle)),
            Err(e) => return server_error(&st.site, &e),
        }
        if let Some(v) = &version {
            if let Err(e) = st.store.save_version(v) {
                return server_error(&st.site, &e);
            }
        }
        if let Err(e) = st.store.save_handle(&rec) {
            return server_error(&st.site, &e);
        }
    }
    let token = st.sessions.start(&rec.handle, now());
    redirect_with(&format!("/@{}", rec.handle), Some(sessions::set_cookie(&token, st.config.secure_cookie())))
}
