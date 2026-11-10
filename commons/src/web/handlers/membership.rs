//! Handlers for bringing people into a commons (decision record 0011):
//! making an invite code, and vouching for someone on a commons' pending list.
//! Both are signed by the member's key; the commons checks them.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use crate::app::{AppState, Shared};
use crate::form::Form;
use crate::model::HandleRec;
use crate::web::handlers::{authorised_as, post_to_commons};
use crate::web::layout::{error_page, host_label, render, server_error, Page};
use crate::web::pages;
use crate::{handle, now, outbound, vouching};

/// The signed-in member and the commons they named, which must be one they
/// have shared to (members can only vouch where they are listed).
fn member_and_commons(st: &AppState, headers: &HeaderMap, f: &Form) -> Result<(HandleRec, String), Box<Response>> {
    let h = handle::normalise(&f.one("handle"));
    if !authorised_as(st, headers, f, &h) {
        return Err(Box::new(error_page(&st.site, "Please sign in as this handle first.")));
    }
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return Err(Box::new(error_page(&st.site, "Unknown handle."))),
        Err(e) => return Err(Box::new(server_error(&st.site, &e))),
    };
    let commons = f.one("commons_url").trim_end_matches('/').to_string();
    if !rec.shared_to.contains(&commons) {
        return Err(Box::new(error_page(&st.site, "You can only bring people into a commons you're listed on.")));
    }
    Ok((rec, commons))
}

/// `POST /invite` — make a single-use invite code for a commons.
pub async fn invite(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let (rec, commons) = match member_and_commons(&st, &headers, &f) {
        Ok(x) => x,
        Err(r) => return *r,
    };
    match vouching::make_invite(&rec.ed_pub, &rec.ed_priv, &commons, now()) {
        Ok(inv) => {
            let code = vouching::invite_to_code(&inv);
            let body = pages::invite_code(&rec.handle, &commons, &code, vouching::INVITE_TTL / 86_400);
            render(&st.site, Page::new("Invite code", Some(&rec.handle), body))
        }
        Err(e) => server_error(&st.site, &e),
    }
}

/// The full key of someone waiting on `commons` with this handle, whose key
/// starts with `prefix` (as shown on the commons' page).
async fn find_waiting(st: &AppState, commons: &str, who: &str, prefix: &str) -> Result<String, String> {
    let policy = outbound::Policy {
        personal: st.config.bind_addr(st.site.role).is_loopback(),
        allow_private: st.config.allow_private_share,
    };
    let client = outbound::client_for(commons, policy).await?;
    let list: serde_json::Value = client
        .get(format!("{commons}/api/pending"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Could not reach that commons: {e}"))?
        .json()
        .await
        .map_err(|_| "That commons didn't send a waiting list.".to_string())?;
    let matches: Vec<String> = list
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p.get("handle").and_then(|v| v.as_str()) == Some(who))
        .filter_map(|p| p.get("key").and_then(|v| v.as_str()).map(String::from))
        .filter(|k| k.starts_with(prefix))
        .collect();
    match matches.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(format!("Nobody called @{who} with that key is waiting on {}.", host_label(commons))),
        _ => Err("More than one person matches; use more of the key.".into()),
    }
}

/// `POST /vouch` — vouch for someone waiting on a commons' pending list.
pub async fn vouch(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = Form::parse(&body);
    let (rec, commons) = match member_and_commons(&st, &headers, &f) {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let who = handle::normalise(&f.one("who"));
    let prefix: String = f.one("key").chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>().to_lowercase();
    if !handle::is_valid(&who) || prefix.len() < 8 {
        return error_page(
            &st.site,
            "Enter their handle and at least the first 8 characters of their key, as shown on the commons.",
        );
    }
    let key = match find_waiting(&st, &commons, &who, &prefix).await {
        Ok(k) => k,
        Err(e) => return error_page(&st.site, &e),
    };
    let signed = match vouching::make_vouch(&rec.ed_pub, &rec.ed_priv, &commons, &who, &key, now()) {
        Ok(v) => v,
        Err(e) => return server_error(&st.site, &e),
    };
    let msg = serde_json::to_value(&signed).expect("signed objects serialise");
    match post_to_commons(&st, &commons, &msg).await {
        Ok(r) if r.status() == StatusCode::ACCEPTED || r.status().is_success() => {
            let body = pages::vouched(&rec.handle, &who, &host_label(&commons));
            render(&st.site, Page::new("Vouched", Some(&rec.handle), body))
        }
        Ok(r) => {
            let why = r.text().await.unwrap_or_default();
            error_page(&st.site, &format!("{} didn't accept the vouch: {why}", host_label(&commons)))
        }
        Err(e) => error_page(&st.site, &e),
    }
}
