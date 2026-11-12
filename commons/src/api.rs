//! JSON endpoints: the directory API, handle availability, and the
//! ActivityStreams-shaped actor, outbox and WebFinger documents.
//!
//! The document builders are pure; the handlers only load and wrap them.
//! These endpoints never expose members-only content.

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::app::Shared;
use crate::federation;
use crate::governance::license_url;
use crate::handle;
use crate::held_signals::HeldSignals;
use crate::model::{HandleRec, Reach, Version};

/// `GET /health` — liveness, the service name and version. When the request
/// carries this server's launch token (`X-HIC-Launch`, sent only by the
/// desktop app that started it), it also says `"ours": true`. The token itself
/// is never sent back: it opens the steward view (ADR 0023).
pub async fn health(State(st): State<Shared>, headers: axum::http::HeaderMap) -> Json<Value> {
    let mut j = json!({ "status": "ok", "service": "hic-commons", "version": env!("CARGO_PKG_VERSION") });
    let given = headers.get("x-hic-launch").and_then(|v| v.to_str().ok()).unwrap_or("");
    if st.config.launch_token.as_deref().is_some_and(|t| !given.is_empty() && crate::keys::ct_eq(t, given)) {
        j["ours"] = json!(true);
    }
    Json(j)
}

/// One entry in `GET /api/directory`.
#[derive(Serialize)]
pub struct DirEntry {
    pub handle: String,
    pub display_name: String,
    pub name: String,
    pub role: String,
    pub visibility: String,
    pub cid: String,
    pub license: String,
    pub signals: Vec<String>,
    /// Held in Common Signals (decision record 0025). On a node these are the
    /// node's statement beside the version; they are signed when shared.
    pub held_signals: HeldSignals,
}

/// `GET /api/directory` — this server's members with a public or commons
/// version.
pub async fn directory(State(st): State<Shared>) -> Json<Vec<DirEntry>> {
    let out = st
        .store
        .handles()
        .into_iter()
        .filter_map(|h| {
            let v = st.store.current_version(&h).ok()??;
            (v.reach() != Reach::Members).then(|| {
                let p = v.payload.unwrap_or_default();
                DirEntry {
                    handle: h.handle,
                    display_name: h.display_name,
                    name: p.name,
                    role: p.role,
                    visibility: v.visibility,
                    cid: v.cid,
                    license: v.license,
                    signals: v.signals,
                    held_signals: v.held_signals,
                }
            })
        })
        .collect();
    Json(out)
}

/// The JSON for one profile in `GET /api/@{handle}`.
pub fn profile_json(h: &HandleRec, v: &Version) -> Value {
    json!({
        "handle": h.handle,
        "display_name": h.display_name,
        "visibility": v.visibility,
        "cid": v.cid,
        "license": v.license,
        "license_url": license_url(&v.license),
        "signals": v.signals,
        "held_signals": v.held_signals,
        "profile": v.payload,
        "ed25519_public": h.ed_pub,
        "box_public": h.box_pub,
        "signature": v.signature,
    })
}

/// A JSON error reply: `{"error": msg}` with the given status.
fn json_error(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

/// `GET /api/@{handle}` — one member's current public or commons version.
pub async fn profile(State(st): State<Shared>, Path(h): Path<String>) -> Response {
    let Ok(Some(rec)) = st.store.handle(&h) else { return json_error(StatusCode::NOT_FOUND, "not found") };
    let Ok(Some(v)) = st.store.current_version(&rec) else { return json_error(StatusCode::NOT_FOUND, "no profile") };
    if v.reach() == Reach::Members {
        return json_error(StatusCode::FORBIDDEN, "members only");
    }
    Json(profile_json(&rec, &v)).into_response()
}

/// `GET /api/handle/{handle}` — is this handle free here?
///
/// A handle is unique per commons, not globally: the same @handle may live on
/// several commons, but on any one it belongs to one key. Nodes call this at
/// sign-up against their default commons.
pub async fn handle_available(State(st): State<Shared>, Path(h): Path<String>) -> Json<Value> {
    let h = h.to_lowercase();
    let valid = handle::is_valid(&h);
    let local = st.store.handle(&h).ok().flatten().map(|r| r.ed_pub);
    let (held, withdrawn) = (st.store.remotes(), st.store.tombstones());
    let claimed = st.store.identity(&h).ok().flatten().map(|i| i.key);
    let holders = federation::Holders {
        local: local.as_deref(),
        held: &held,
        withdrawn: &withdrawn,
        claimed: claimed.as_deref(),
    };
    let taken = federation::handle_taken(&h, None, &holders);
    Json(json!({ "handle": h, "valid": valid, "available": valid && !taken }))
}

/// `GET /api/pending` — shares waiting for a vouch on this commons: handle,
/// full key (a vouch must name it), and where it came from. No profile content.
pub async fn pending(State(st): State<Shared>) -> Json<Value> {
    let list: Vec<Value> = st
        .store
        .pending()
        .into_iter()
        .map(|p| json!({ "handle": p.entry.handle, "key": p.entry.ed_pub, "origin": p.entry.origin, "received_at": p.entry.received_at }))
        .collect();
    Json(json!(list))
}

/// The query for `GET /api/status`.
#[derive(Deserialize)]
pub struct StatusQ {
    pub handle: String,
    pub key: String,
}

/// `GET /api/status?handle=…&key=…` — is this handle, with this key, listed
/// here, waiting for a vouch, or neither? Nodes ask this to follow up on a
/// waiting share (see [`crate::waiting`]).
pub async fn share_status(State(st): State<Shared>, Query(q): Query<StatusQ>) -> Json<Value> {
    let listed = st.store.remotes().iter().any(|r| r.handle == q.handle && r.ed_pub == q.key);
    let waiting = st.store.pending().iter().any(|p| p.entry.handle == q.handle && p.entry.ed_pub == q.key);
    let status = if listed {
        "listed"
    } else if waiting {
        "waiting"
    } else {
        "not_listed"
    };
    Json(json!({ "handle": q.handle, "status": status }))
}

/// `POST /api/notices/claim` — for the desktop app: the notices it hasn't yet
/// shown as system notifications, which are then marked as shown. Only
/// answers a request carrying this server's launch token
/// (`X-HIC-Launch`), so a web page can't read or clear them.
pub async fn claim_notices(State(st): State<Shared>, headers: axum::http::HeaderMap) -> Response {
    let given = headers.get("x-hic-launch").and_then(|v| v.to_str().ok()).unwrap_or("");
    let ok = st.config.launch_token.as_deref().is_some_and(|t| !given.is_empty() && crate::keys::ct_eq(t, given));
    if !ok {
        return json_error(StatusCode::FORBIDDEN, "only the desktop app can collect notices");
    }
    match st.store.claim_unpushed_notices() {
        Ok(list) => Json(json!(list)).into_response(),
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// The query for WebFinger.
#[derive(Deserialize)]
pub struct WebfingerQ {
    pub resource: String,
}

/// The WebFinger document for `acct:handle@domain`.
pub fn webfinger_json(handle_: &str, domain: &str, base_url: &str) -> Value {
    json!({
        "subject": format!("acct:{handle_}@{domain}"),
        "links": [{ "rel": "self", "type": "application/activity+json", "href": format!("{base_url}/ap/@{handle_}") }]
    })
}

/// `GET /.well-known/webfinger?resource=acct:handle@domain`.
pub async fn webfinger(State(st): State<Shared>, Query(q): Query<WebfingerQ>) -> Response {
    let acct = q.resource.strip_prefix("acct:").unwrap_or(&q.resource);
    let h = acct.split('@').next().unwrap_or("");
    if !matches!(st.store.handle(h), Ok(Some(_))) {
        return (StatusCode::NOT_FOUND, "no such account").into_response();
    }
    (
        [(header::CONTENT_TYPE, "application/jrd+json")],
        Json(webfinger_json(h, &st.config.domain(), &st.config.base_url)),
    )
        .into_response()
}

/// The actor document for a member.
///
/// Not yet interoperable ActivityPub (see `docs/REVIEW-2026-11.md`, M3):
/// `publicKeyPem` carries the hex Ed25519 key, and there is no actor inbox.
pub fn actor_json(base_url: &str, h: &HandleRec) -> Value {
    let id = format!("{base_url}/ap/@{}", h.handle);
    json!({
        "@context": ["https://www.w3.org/ns/activitystreams"],
        "id": id,
        "type": "Person",
        "preferredUsername": h.handle,
        "name": h.display_name,
        "url": format!("{base_url}/@{}", h.handle),
        "inbox": format!("{id}/inbox"),
        "outbox": format!("{id}/outbox"),
        "publicKeyPem": h.ed_pub,
        "hicBoxPublicKey": h.box_pub,
    })
}

/// `GET /ap/@{handle}`.
pub async fn actor(State(st): State<Shared>, Path(h): Path<String>) -> Response {
    let Ok(Some(rec)) = st.store.handle(&h) else { return (StatusCode::NOT_FOUND, "no such actor").into_response() };
    ([(header::CONTENT_TYPE, "application/activity+json")], Json(actor_json(&st.config.base_url, &rec))).into_response()
}

/// The outbox for a member: their current version, unless members-only.
pub fn outbox_json(base_url: &str, h: &str, v: Option<&Version>) -> Value {
    let actor = format!("{base_url}/ap/@{h}");
    let items: Vec<Value> = v
        .filter(|v| v.reach() != Reach::Members)
        .map(|v| {
            let obj_id = format!("{actor}/versions/{}", v.cid);
            let to = if v.reach() == Reach::Public {
                "https://www.w3.org/ns/activitystreams#Public".to_string()
            } else {
                format!("{actor}/followers")
            };
            json!({
                "type": "Create",
                "id": format!("{obj_id}/create"),
                "actor": actor,
                "to": [to],
                "object": {
                    "id": obj_id,
                    "type": "Profile",
                    "attributedTo": actor,
                    "name": v.payload.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                    "license": license_url(&v.license),
                    "ccSignals": v.signals,
                }
            })
        })
        .into_iter()
        .collect();
    json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{actor}/outbox"),
        "type": "OrderedCollection",
        "totalItems": items.len(),
        "orderedItems": items,
    })
}

/// `GET /ap/@{handle}/outbox`.
pub async fn outbox(State(st): State<Shared>, Path(h): Path<String>) -> Response {
    let Ok(Some(rec)) = st.store.handle(&h) else { return (StatusCode::NOT_FOUND, "no such actor").into_response() };
    let v = st.store.current_version(&rec).ok().flatten();
    (
        [(header::CONTENT_TYPE, "application/activity+json")],
        Json(outbox_json(&st.config.base_url, &rec.handle, v.as_ref())),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bare version with the given reach, enough for the reach checks.
    fn version(reach: &str) -> Version {
        Version {
            cid: "c".into(),
            handle: "tega".into(),
            visibility: reach.into(),
            license: "cc-by".into(),
            signals: vec![],
            created_at: 0,
            signature: String::new(),
            payload: None,
            enc: None,
            held_signals: Default::default(),
        }
    }

    #[test]
    fn outbox_never_lists_members_only() {
        assert_eq!(outbox_json("http://n", "tega", Some(&version("members")))["totalItems"], 0);
        assert_eq!(outbox_json("http://n", "tega", Some(&version("public")))["totalItems"], 1);
        assert_eq!(outbox_json("http://n", "tega", None)["totalItems"], 0);
    }

    #[test]
    fn webfinger_subject() {
        assert_eq!(webfinger_json("tega", "c.example", "https://c.example")["subject"], "acct:tega@c.example");
    }
}
