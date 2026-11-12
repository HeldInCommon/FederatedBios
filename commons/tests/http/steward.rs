//! The steward view of a commons (decision records 0017 and 0023: it opens
//! only with the steward key or the app's launch token).

use crate::common::{closed_commons, get, person, post_form, post_json, send, session_from, share_of, steward_session};
use axum::http::StatusCode;

#[tokio::test]
async fn steward_approves_and_declines_people_waiting() {
    let (commons, store) = closed_commons(5);
    let (sid, csrf) = steward_session(&commons, &store).await;
    let (amy, bo) = (person("amy"), person("bo"));
    send(&commons, post_json("/inbox", &share_of(&amy, None))).await;
    send(&commons, post_json("/inbox", &share_of(&bo, None))).await;
    let page = send(&commons, get("/", Some(&sid))).await.text;
    assert!(page.contains("/steward/approve") && page.contains("/steward/decline"), "buttons for the steward");
    assert!(!send(&commons, get("/", None)).await.text.contains("/steward/approve"), "but not for visitors");

    // Without the steward's token, nothing happens.
    let forged = send(&commons, post_form("/steward/approve", &format!("key={}", amy.ed_pub), Some(&sid))).await;
    assert_eq!(forged.status, StatusCode::BAD_REQUEST);
    assert_eq!(store.pending().len(), 2);

    let ok =
        send(&commons, post_form("/steward/approve", &format!("csrf={csrf}&key={}", amy.ed_pub), Some(&sid))).await;
    assert_eq!(ok.status, StatusCode::SEE_OTHER);
    let no = send(&commons, post_form("/steward/decline", &format!("csrf={csrf}&key={}", bo.ed_pub), Some(&sid))).await;
    assert_eq!(no.status, StatusCode::SEE_OTHER);
    assert!(store.pending().is_empty());
    assert_eq!(store.remotes().len(), 1);
    assert_eq!(store.intros()[0].route, "steward");
}

#[tokio::test]
async fn steward_invite_codes_work_once_and_can_be_emailed() {
    let (commons, store) = closed_commons(5);
    let (sid, csrf) = steward_session(&commons, &store).await;
    let page = send(&commons, post_form("/steward/invite", &format!("csrf={csrf}"), Some(&sid))).await.text;
    assert!(page.contains("mailto:?subject="), "an Email this invite button");
    let code =
        page.split("hic-invite-").nth(1).unwrap().chars().take_while(|c| c.is_ascii_hexdigit()).collect::<String>();
    let code = format!("hic-invite-{code}");
    assert_eq!(send(&commons, post_json("/inbox", &share_of(&person("guest"), Some(&code)))).await.text, "accepted");
    let again = send(&commons, post_json("/inbox", &share_of(&person("other"), Some(&code)))).await;
    assert_eq!(again.status, StatusCode::FORBIDDEN);
}

/// ADR 0023: arriving from this computer is not proof of being the steward.
/// Behind Tailscale Funnel (or any reverse proxy on the same machine) every
/// visitor arrives from 127.0.0.1, so a bare `/steward` must be refused.
#[tokio::test]
async fn a_visitor_through_a_proxy_on_this_computer_is_not_the_steward() {
    use axum::extract::ConnectInfo;
    use std::net::SocketAddr;
    let (commons, _store) = commons_with_launch_token("app-secret");
    let mut proxied = get("/steward", None);
    proxied.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))));
    let r = send(&commons, proxied).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "no key, no launch token: refused");
    assert!(session_from(&r).is_none(), "and no session handed out");
    assert!(
        !send(&commons, get("/", None)).await.text.contains("Open the steward view"),
        "nobody is told they're at the commons' computer"
    );
}

/// The desktop app opens the steward view with its private launch token.
#[tokio::test]
async fn the_desktop_app_opens_the_steward_view_with_its_launch_token() {
    let (commons, _store) = commons_with_launch_token("app-secret");
    assert_eq!(send(&commons, get("/steward?launch=guess", None)).await.status, StatusCode::BAD_REQUEST);
    let r = send(&commons, get("/steward?launch=app-secret", None)).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    let sid = session_from(&r).expect("steward session");
    assert!(send(&commons, get("/", Some(&sid))).await.text.contains("/steward/invite"), "the steward's tools");

    // A commons the app didn't start has no launch token: an empty one is refused.
    let (plain, _) = closed_commons(3);
    assert_eq!(send(&plain, get("/steward?launch=", None)).await.status, StatusCode::BAD_REQUEST);
}

/// A commons started as if by the desktop app, with launch token `token`.
fn commons_with_launch_token(token: &'static str) -> (axum::Router, hic_commons::store::Store) {
    use hic_commons::app::{router, AppState};
    use hic_commons::config::Config;
    use hic_commons::role::Role;
    use hic_commons::store::Store;
    let dir = std::env::temp_dir().join(format!("hic-http-launch-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let config = Config::from_lookup(move |k| (k == "HIC_LAUNCH_TOKEN").then(|| token.to_string()));
    (router(std::sync::Arc::new(AppState::new(config, Role::Commons, store.clone()))), store)
}
