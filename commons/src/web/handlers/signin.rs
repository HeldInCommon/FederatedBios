//! Sign-up, sign-in and sign-out (nodes only; a commons shows its gate page).

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::{session, update_member, with_cookie, WithStatus};
use crate::app::{AppState, Shared};
use crate::form::Form;
use crate::model::HandleRec;
use crate::passphrase::{self, Check};
use crate::sessions;
use crate::web::layout::{error_page, redirect_with, render, server_error, Page};
use crate::web::pages;
use crate::{handle, identity, keys, now};

/// `GET /join` — the sign-up form (a commons shows its gate page instead).
pub async fn join_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    let body = if st.site.is_commons() { pages::commons_gate(&st.site) } else { pages::join_form(me.as_deref()) };
    render(&st.site, Page::new(if st.site.is_commons() { "Shared commons" } else { "Join" }, me.as_deref(), body))
}

/// Ask the node's default commons whether `h` is free there. `None` when
/// there is no default commons or it can't be reached.
async fn free_on_default_commons(st: &AppState, h: &str) -> Option<bool> {
    let default = &st.site.share_default;
    if default.is_empty() {
        return None;
    }
    let reply = st
        .http
        .get(format!("{default}/api/handle/{h}"))
        .timeout(std::time::Duration::from_secs(4))
        .send()
        .await
        .ok()?;
    let j: serde_json::Value = reply.json().await.ok()?;
    j.get("available").and_then(|v| v.as_bool())
}

/// `POST /join` — create a handle and its keys, then sign in.
pub async fn join_submit(State(st): State<Shared>, body: String) -> Response {
    if st.site.is_commons() {
        return render(&st.site, Page::new("Shared commons", None, pages::commons_gate(&st.site)));
    }
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    let display = f.clamped("display_name", 80);
    let pass = f.one("passphrase");
    if !handle::is_valid(&h) {
        return error_page(&st.site, "That handle isn't valid — use 2–30 lowercase letters, numbers or underscores.");
    }
    if display.is_empty() {
        return error_page(&st.site, "Please add a display name.");
    }
    if !passphrase::long_enough(&pass) {
        return error_page(&st.site, "Please choose a passphrase of at least 8 characters.");
    }
    // Checked before anything is saved: a mistyped passphrase would lock the
    // member out of a handle they just made.
    if pass != f.one("passphrase_again") {
        return error_page(&st.site, "The two passphrases don't match, so nothing was created. Please type it again.");
    }
    // Handles are unique per commons. Ask the default commons first, so a clash
    // shows now rather than at the first share. If it can't be reached, carry
    // on: the commons still refuses a clash when the profile is shared.
    if free_on_default_commons(&st, &h).await == Some(false) {
        return error_page(
            &st.site,
            &format!(
                "@{h} is already used on {}. Each commons keeps its handles unique — try another.",
                crate::web::layout::host_label(&st.site.share_default)
            ),
        );
    }
    let (salt, hash) = passphrase::hash(&pass);
    let mut rec = HandleRec::new(&h, &display, now(), keys::generate(), salt, hash);
    // The recovery key: its public half is kept and declared to commons; its
    // private half is shown once below and never stored (decision record 0010).
    let (recovery_pub, recovery_priv) = keys::generate_signing();
    match identity::declare_recovery(&h, &rec.ed_pub, &rec.ed_priv, &recovery_pub, now()) {
        Ok(d) => (rec.recovery_pub, rec.recovery_decl) = (Some(recovery_pub), Some(d)),
        Err(e) => return server_error(&st.site, &e),
    }
    {
        let _g = st.write.lock().unwrap();
        match st.store.handle(&h) {
            Ok(None) => {}
            Ok(Some(_)) => return error_page(&st.site, "That handle is taken on this node. Try another."),
            // A damaged record must never be overwritten by a new sign-up.
            Err(e) => return server_error(&st.site, &e),
        }
        if let Err(e) = st.store.save_handle(&rec) {
            return server_error(&st.site, &e);
        }
    }
    let token = st.sessions.start(&h, now());
    let page = render(
        &st.site,
        Page::new(
            "Your recovery code",
            Some(&h),
            pages::recovery_code(&h, &keys::recovery_code(&recovery_priv), "/new"),
        ),
    );
    with_cookie(page, sessions::set_cookie(&st.config.session_cookie, &token, st.config.secure_cookie()))
}

/// `GET /login` — the sign-in form (a commons shows its gate page instead).
pub async fn login_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session(&st, &headers).map(|s| s.handle);
    let body = if st.site.is_commons() { pages::commons_gate(&st.site) } else { pages::login_form() };
    render(&st.site, Page::new(if st.site.is_commons() { "Shared commons" } else { "Sign in" }, me.as_deref(), body))
}

/// `POST /login` — check the passphrase and start a session.
pub async fn login_submit(State(st): State<Shared>, body: String) -> Response {
    if st.site.is_commons() {
        return render(&st.site, Page::new("Shared commons", None, pages::commons_gate(&st.site)));
    }
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    let pass = f.one("passphrase");
    let t = now();
    if !st.attempts.allowed(&h, t) {
        return error_page(&st.site, "Too many wrong passphrases for this handle. Wait 15 minutes and try again.")
            .into_response_with(StatusCode::TOO_MANY_REQUESTS);
    }
    /// One message for both mistakes, so the page never says which handles exist.
    const WRONG: &str = "No such handle, or the passphrase is wrong.";
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => {
            st.attempts.failed(&h, t);
            return error_page(&st.site, WRONG);
        }
        Err(e) => return server_error(&st.site, &e),
    };
    match passphrase::check(&rec.secret_salt, &rec.secret_hash, &pass) {
        Check::Wrong => {
            st.attempts.failed(&h, t);
            return error_page(&st.site, WRONG);
        }
        Check::Upgrade => {
            // Replace the old-format hash now that we know the passphrase.
            // A failure only delays the upgrade, so sign-in still succeeds.
            if let Err(e) = update_member(&st, &h, |r| r.with_passphrase_hash(passphrase::hash(&pass))) {
                eprintln!("warning: couldn't upgrade the passphrase hash for @{h}: {e}");
            }
        }
        Check::Ok => {}
    }
    st.attempts.succeeded(&h);
    let token = st.sessions.start(&h, t);
    redirect_with(
        &format!("/@{h}"),
        Some(sessions::set_cookie(&st.config.session_cookie, &token, st.config.secure_cookie())),
    )
}

/// `POST /logout` — end the session.
pub async fn logout(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Some(t) = sessions::token_from(&headers, &st.config.session_cookie) {
        st.sessions.end(&t);
    }
    redirect_with("/", Some(sessions::clear_cookie(&st.config.session_cookie, st.config.secure_cookie())))
}
