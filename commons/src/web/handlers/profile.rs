//! `GET /@{handle}`: a member's profile, with the owner's tools when it's theirs.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;

use super::notices::notices_for;
use super::{readable_profile, session};
use crate::app::Shared;
use crate::model::Reach;
use crate::web::layout::{not_found, render, server_error, Page};
use crate::web::pages::{self, OwnerPanel};

/// `?tab=` on a member's own profile.
#[derive(serde::Deserialize)]
pub struct TabQ {
    /// `sharing`, `people` or `account`; anything else is the profile.
    pub tab: Option<String>,
}

/// `GET /@{handle}` — a member's profile (`?tab=` picks the owner's tab).
pub async fn profile(
    State(st): State<Shared>,
    Path(h): Path<String>,
    Query(q): Query<TabQ>,
    headers: HeaderMap,
) -> Response {
    let sess = session(&st, &headers);
    let me = sess.as_ref().map(|s| s.handle.as_str());
    let rec = match st.store.handle(&h) {
        Ok(Some(r)) => r,
        Ok(None) => return not_found(&st.site, me),
        Err(e) => return server_error(&st.site, &e),
    };
    let version = match st.store.current_version(&rec) {
        Ok(v) => v,
        Err(e) => return server_error(&st.site, &e),
    };
    let readable = version.as_ref().and_then(|v| readable_profile(&st, v, me));
    let owner = match &sess {
        Some(s) if s.handle == rec.handle => Some(OwnerPanel {
            csrf: s.csrf.clone(),
            published: version.is_some(),
            shareable: version.as_ref().is_some_and(|v| v.reach() != Reach::Members),
            known: pages::known_commons(&st.site.share_default, &rec.shared_to),
            shared_to: rec.shared_to.clone(),
            has_recovery: rec.recovery_pub.is_some(),
            rotation_pending: rec.pending_rotation.is_some(),
            waiting_on: rec.waiting_on.clone(),
            backup_done: rec.backup_done,
            key: rec.ed_pub.clone(),
            tab: crate::ramp::Tab::parse(q.tab.as_deref()),
        }),
        _ => None,
    };
    let noindex = version.as_ref().is_some_and(|v| v.reach() == Reach::Commons);
    let body = pages::profile(&rec, version.as_ref(), readable.as_ref(), owner.as_ref());
    let title = format!("@{}", rec.handle);
    let notices = notices_for(&st, &headers);
    render(&st.site, Page::new(&title, me, body).noindex(noindex).notices(notices))
}
