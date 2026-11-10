//! Handlers for a member's keys: making a recovery key, replacing a key, and
//! recovering with a recovery code (decision record 0010).
//!
//! A key change has to reach every commons the member shared to. The node
//! keeps it as a [`PendingRotation`] and sends it to each commons; only when
//! all have accepted does the node switch to the new keys, re-sign the
//! current profile and share it again. Until then nothing changes locally, so
//! no commons is left on a key the node no longer has.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use crate::app::{AppState, Shared};
use crate::form::Form;
use crate::identity::{self, RECOVERY_ROTATION};
use crate::model::{HandleRec, PendingRotation};
use crate::publishing::{self, Draft};
use crate::web::handlers::{authorised_as, post_to_commons, readable_profile, send_share};
use crate::web::layout::{error_page, render, server_error, Page};
use crate::web::pages;
use crate::{handle, keys, now};

/// Load the signed-in member's record, or the response to send instead.
fn member(st: &AppState, headers: &HeaderMap, f: &Form, action: &str) -> Result<HandleRec, Box<Response>> {
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(st, headers, f, &h) {
        return Err(Box::new(error_page(&st.site, &format!("Please sign in as this handle to {action}."))));
    }
    match st.store.handle(&h) {
        Ok(Some(r)) => Ok(r),
        Ok(None) => Err(Box::new(error_page(&st.site, "Unknown handle."))),
        Err(e) => Err(Box::new(server_error(&st.site, &e))),
    }
}

/// `POST /recovery/new` — make a recovery key for a handle that has none,
/// show its code once, and tell the commons already shared to.
pub async fn recovery_new(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let mut rec = match member(&st, &headers, &f, "make a recovery key") {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    if rec.recovery_pub.is_some() {
        return error_page(&st.site, "This handle already has a recovery key.");
    }
    let (recovery_pub, recovery_priv) = keys::generate_signing();
    let decl = match identity::declare_recovery(&rec.handle, &rec.ed_pub, &rec.ed_priv, &recovery_pub, now()) {
        Ok(d) => d,
        Err(e) => return server_error(&st.site, &e),
    };
    rec.recovery_pub = Some(recovery_pub);
    rec.recovery_decl = Some(decl.clone());
    {
        let _g = st.write.lock().unwrap();
        if let Err(e) = st.store.save_handle(&rec) {
            return server_error(&st.site, &e);
        }
    }
    // Best effort: commons not reached now get the declaration with the next share.
    let decl_json = serde_json::to_value(&decl).expect("signed objects serialise");
    for c in &rec.shared_to {
        let _ = post_to_commons(&st, c, &decl_json).await;
    }
    let next = format!("/@{}", rec.handle);
    let code = keys::recovery_code(&recovery_priv);
    render(
        &st.site,
        Page::new("Your recovery code", Some(&rec.handle), pages::recovery_code(&rec.handle, &code, &next)),
    )
}

/// `POST /rotate` — replace the member's key (or finish a replacement that
/// couldn't reach every commons last time).
pub async fn rotate(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let rec = match member(&st, &headers, &f, "replace your key") {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    let pending = match rec.pending_rotation.clone() {
        Some(p) => p,
        None => {
            let new = keys::generate();
            match identity::rotate(&rec.handle, &rec.ed_pub, &rec.ed_priv, &new, now()) {
                Ok(change) => PendingRotation { keys: new, change, next_recovery: None, done: vec![] },
                Err(e) => return server_error(&st.site, &e),
            }
        }
    };
    finish_key_change(&st, &rec.handle, pending).await
}

/// `POST /recover` — take the handle back with the recovery code, moving it
/// to fresh keys and a new recovery key.
pub async fn recover(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let rec = match member(&st, &headers, &f, "recover your handle") {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    let Some(recovery_priv) = keys::parse_recovery_code(&f.one("code")) else {
        return error_page(
            &st.site,
            "That isn't a recovery code. It should be 64 letters and numbers, in groups of eight.",
        );
    };
    let recovery_pub = keys::public_from_private(&recovery_priv).expect("parsed codes are valid keys");
    if rec.recovery_pub.as_ref().is_some_and(|p| *p != recovery_pub) {
        return error_page(&st.site, &format!("That isn't the recovery code for @{}.", rec.handle));
    }
    // Retrying a recovery reuses it, so every commons ends on the same keys.
    let pending = match rec.pending_rotation.clone() {
        Some(p) if p.change.envelope.kind == RECOVERY_ROTATION && p.change.envelope.author == recovery_pub => p,
        _ => {
            let new = keys::generate();
            let next_recovery = keys::generate_signing();
            match identity::recover(&rec.handle, &recovery_pub, &recovery_priv, &new, &next_recovery.0, now()) {
                Ok(change) => PendingRotation { keys: new, change, next_recovery: Some(next_recovery), done: vec![] },
                Err(e) => return server_error(&st.site, &e),
            }
        }
    };
    finish_key_change(&st, &rec.handle, pending).await
}

/// Send a pending key change to every commons that hasn't accepted it; if all
/// have, switch this node to the new keys. Shows what happened.
async fn finish_key_change(st: &AppState, h: &str, mut pending: PendingRotation) -> Response {
    let Ok(Some(rec)) = st.store.handle(h) else { return error_page(&st.site, "Unknown handle.") };
    let change = serde_json::to_value(&pending.change).expect("signed objects serialise");
    let mut failed: Vec<(String, String)> = Vec::new();
    let todo: Vec<String> = rec.shared_to.iter().filter(|c| !pending.done.contains(c)).cloned().collect();
    for c in &todo {
        match post_to_commons(st, c, &change).await {
            // 404: that commons doesn't hold the handle, so there's nothing to move.
            Ok(r) if r.status().is_success() || r.status() == StatusCode::NOT_FOUND => pending.done.push(c.clone()),
            Ok(r) => failed.push((c.clone(), r.text().await.unwrap_or_default())),
            Err(e) => failed.push((c.clone(), e)),
        }
    }
    if !failed.is_empty() {
        let _g = st.write.lock().unwrap();
        if let Ok(Some(mut r)) = st.store.handle(h) {
            r.pending_rotation = Some(pending.clone());
            if let Err(e) = st.store.save_handle(&r) {
                return server_error(&st.site, &e);
            }
        }
        return render(
            &st.site,
            Page::new("Not finished", Some(h), pages::key_change(h, &pending.done, &failed, None)),
        );
    }
    let new_code = match switch_keys(st, h, &pending) {
        Ok(code) => code,
        Err(e) => return server_error(&st.site, &e),
    };
    reshare(st, h).await;
    let body = pages::key_change(h, &pending.done, &[], new_code.as_deref());
    render(&st.site, Page::new("Key replaced", Some(h), body))
}

/// Switch the member's record to the pending keys, re-declare the recovery
/// key with the new key, and re-sign the current profile. Returns the new
/// recovery code to show, if this was a recovery.
fn switch_keys(st: &AppState, h: &str, pending: &PendingRotation) -> Result<Option<String>, String> {
    let _g = st.write.lock().unwrap();
    let mut rec = st.store.handle(h).map_err(|e| e.to_string())?.ok_or("unknown handle")?;
    // Read the current profile while the old keys can still decrypt it.
    let current = st.store.current_version(&rec).map_err(|e| e.to_string())?;
    let readable = current.as_ref().and_then(|v| readable_profile(st, v, Some(h)));

    let k = &pending.keys;
    (rec.ed_pub, rec.ed_priv, rec.box_pub, rec.box_priv) =
        (k.ed_pub.clone(), k.ed_priv.clone(), k.box_pub.clone(), k.box_priv.clone());
    let mut new_code = None;
    if let Some((next_pub, next_priv)) = &pending.next_recovery {
        rec.recovery_pub = Some(next_pub.clone());
        new_code = Some(keys::recovery_code(next_priv));
    }
    if let Some(rp) = &rec.recovery_pub {
        rec.recovery_decl = Some(identity::declare_recovery(h, &rec.ed_pub, &rec.ed_priv, rp, now())?);
    }
    if let (Some(v), Some(profile)) = (current, readable) {
        let recipients: Vec<(String, String)> = st
            .store
            .handles()
            .into_iter()
            .map(|m| if m.handle == h { (m.handle, rec.box_pub.clone()) } else { (m.handle, m.box_pub) })
            .collect();
        let draft = Draft { profile, reach: v.reach(), license: v.license, signals: v.signals, created_at: now() };
        let version = publishing::sign_version(&rec, draft, &recipients)?;
        st.store.save_version(&version).map_err(|e| e.to_string())?;
        rec.current_cid = Some(version.cid);
    }
    rec.pending_rotation = None;
    st.store.save_handle(&rec).map_err(|e| e.to_string())?;
    Ok(new_code)
}

/// Share the (re-signed) current version to every commons again, so each
/// holds a copy signed by the new key. Best effort.
async fn reshare(st: &AppState, h: &str) {
    let Ok(Some(rec)) = st.store.handle(h) else { return };
    let Ok(Some(v)) = st.store.current_version(&rec) else { return };
    if v.reach() == crate::model::Reach::Members {
        return;
    }
    for c in &rec.shared_to {
        let _ = send_share(st, c, &rec, &v).await;
    }
}
