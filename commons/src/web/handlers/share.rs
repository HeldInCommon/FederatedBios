//! Sharing to a commons and withdrawing from it, and the helpers that send
//! signed messages to a commons' inbox.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::{authorised_as, update_member};
use crate::app::{AppState, Shared};
use crate::federation::{self, Sender};
use crate::form::Form;
use crate::model::{HandleRec, Reach, Version};
use crate::web::layout::{error_page, render, server_error, Page};
use crate::web::pages;
use crate::{handle, now, outbound};

/// `POST /share` — send the current version to a commons.
pub async fn share(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(&st, &headers, &f, &h) {
        return error_page(&st.site, "Please sign in as this handle to share.");
    }
    let commons = f.one("commons_url").trim_end_matches('/').to_string();
    if !(commons.starts_with("http://") || commons.starts_with("https://")) {
        return error_page(&st.site, "Enter the full address of a commons, e.g. https://example.proxy.runpod.net");
    }
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return error_page(&st.site, "Unknown handle."),
        Err(e) => return server_error(&st.site, &e),
    };
    let v = match st.store.current_version(&rec) {
        Ok(Some(v)) => v,
        Ok(None) => return error_page(&st.site, "Publish a profile before sharing it."),
        Err(e) => return server_error(&st.site, &e),
    };
    if v.reach() == Reach::Members {
        return error_page(&st.site, "Members-only profiles stay on your node — they are never shared.");
    }
    let invite = f.one("invite");
    let sent = send_share(&st, &commons, &rec, &v, Some(&invite)).await;
    let label = crate::web::layout::host_label(&commons);
    match sent {
        Ok(r) if r.status().is_success() => {
            let pending = r.text().await.unwrap_or_default().starts_with("pending");
            // Remember the commons, and whether we're waiting to be listed
            // there (the node follows up; see crate::waiting).
            let remembered = update_member(&st, &h, |r| {
                let r = r.with_shared_to(&commons);
                if pending {
                    r.with_waiting_on(&commons)
                } else {
                    r.without_waiting_on(&commons)
                }
            });
            if let Err(e) = remembered {
                eprintln!("warning: shared, but couldn't remember it for @{h}: {e}");
            }
            let body = if pending {
                pages::shared_pending(&h, &commons, &rec.ed_pub, &f.one("csrf"))
            } else {
                pages::shared(&h, &commons)
            };
            render(&st.site, Page::new("Shared", Some(&h), body))
        }
        Ok(r) if r.status() == StatusCode::FORBIDDEN => {
            let why = r.text().await.unwrap_or_default();
            error_page(&st.site, &format!("{label} didn't accept the invite code: {why}"))
        }
        Ok(r) if r.status() == StatusCode::CONFLICT => {
            let reason = r.text().await.unwrap_or_default();
            if reason.contains("newer") {
                error_page(
                    &st.site,
                    &format!("{label} already has a newer version of @{h}. Publish again, then share."),
                )
            } else {
                error_page(&st.site, &format!(
                    "@{h} is already used on {label} by someone else. Each commons keeps its handles unique, so this one can't join there — but you can still share it to other commons, or make a new handle for this one."
                ))
            }
        }
        Ok(r) => error_page(
            &st.site,
            &format!("That commons rejected the share (HTTP {}). It may not accept federation.", r.status().as_u16()),
        ),
        Err(e) => error_page(&st.site, &e),
    }
}

/// POST `body` to a commons' `/inbox`, checking first that this server may
/// contact that address (see [`crate::outbound`]). Errors are messages for
/// the member.
pub(crate) async fn post_to_commons(
    st: &AppState,
    commons: &str,
    body: &serde_json::Value,
) -> Result<reqwest::Response, String> {
    let policy = outbound::Policy {
        personal: st.config.bind_addr(st.site.role).is_loopback(),
        allow_private: st.config.allow_private_share,
    };
    let client = outbound::client_for(commons, policy).await?;
    client
        .post(format!("{commons}/inbox"))
        .json(body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Could not reach that commons: {e}"))
}

/// Share a version to a commons in format `hic/v1` (every field signed). A
/// commons that predates `hic/v1` answers 400; then send the older format so
/// sharing keeps working while commons are upgraded.
pub(crate) async fn send_share(
    st: &AppState,
    commons: &str,
    rec: &HandleRec,
    v: &Version,
    invite: Option<&str>,
) -> Result<reqwest::Response, String> {
    let sender = Sender { base_url: &st.config.base_url, instance: &st.site.instance };
    // An invite code, if the member has one, travels alongside (decision record 0011).
    let with_invite = |mut m: serde_json::Value| {
        if let Some(code) = invite.filter(|c| !c.trim().is_empty()) {
            m["invite"] = serde_json::json!(code.trim());
        }
        m
    };
    let v1 = with_invite(federation::build_share_v1(&sender, rec, v)?);
    let r = post_to_commons(st, commons, &v1).await?;
    if r.status() == StatusCode::BAD_REQUEST {
        return post_to_commons(st, commons, &with_invite(federation::build_share(&sender, rec, v))).await;
    }
    Ok(r)
}

/// `POST /withdraw` — ask a commons to remove this member's shared copy.
pub async fn withdraw(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(&st, &headers, &f, &h) {
        return error_page(&st.site, "Please sign in as this handle to withdraw.");
    }
    let commons = f.one("commons_url").trim_end_matches('/').to_string();
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return error_page(&st.site, "Unknown handle."),
        Err(e) => return server_error(&st.site, &e),
    };
    if !rec.shared_to.contains(&commons) {
        return error_page(&st.site, "You haven't shared to that commons from here.");
    }
    let message = match federation::build_withdraw(&rec, &commons, now()) {
        Ok(m) => m,
        Err(e) => return server_error(&st.site, &format!("@{h}: {e}")),
    };
    let sent = post_to_commons(&st, &commons, &message).await;
    let label = crate::web::layout::host_label(&commons);
    match sent {
        Ok(r) if r.status().is_success() => {
            if let Err(e) = update_member(&st, &h, |r| r.without_shared_to(&commons)) {
                eprintln!("warning: withdrawn, but couldn't update the list for @{h}: {e}");
            }
            render(&st.site, Page::new("Withdrawn", Some(&h), pages::withdrawn(&h, &label)))
        }
        Ok(r) => {
            let why = r.text().await.unwrap_or_default();
            error_page(&st.site, &format!("{label} didn't accept the withdrawal: {why}"))
        }
        Err(e) => error_page(&st.site, &e),
    }
}
