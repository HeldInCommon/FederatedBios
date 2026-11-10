//! Following up on a share that is waiting for a vouch, and the notices
//! the desktop app collects.

use crate::common::{closed_commons, csrf_in, get, join_and_publish, person, post_form, post_json, send, share_of};
use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use hic_commons::app::{router, AppState};
use hic_commons::config::Config;
use hic_commons::role::Role;
use hic_commons::store::Store;
use std::sync::Arc;

#[tokio::test]
async fn commons_answers_share_status() {
    let (commons, store) = closed_commons(5);
    let newbie = person("newbie");
    let status = |h: &str, k: &str| format!("/api/status?handle={h}&key={k}");
    assert!(send(&commons, get(&status("newbie", &newbie.ed_pub), None)).await.text.contains("not_listed"));
    send(&commons, post_json("/inbox", &share_of(&newbie, None))).await;
    assert!(send(&commons, get(&status("newbie", &newbie.ed_pub), None)).await.text.contains("\"waiting\""));
    store.save_founding_code("hic-founding-x9").unwrap();
    send(&commons, post_json("/inbox", &share_of(&person("other"), Some("hic-founding-x9")))).await;
    let o = store.remotes()[0].clone();
    assert!(send(&commons, get(&status("other", &o.ed_pub), None)).await.text.contains("\"listed\""));
}

#[tokio::test]
async fn only_the_desktop_app_can_collect_notices() {
    let dir = std::env::temp_dir().join(format!("hic-http-notices-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let config = Config::from_lookup(|k| (k == "HIC_LAUNCH_TOKEN").then(|| "tok".to_string()));
    let app = router(Arc::new(AppState::new(config, Role::Node, store.clone())));
    let sid = join_and_publish(&app, "ada", "public").await;
    store
        .add_notice(hic_commons::model::Notice {
            handle: "ada".into(),
            text: "@ada is now listed on c.".into(),
            at: 1,
            seen: false,
            pushed: false,
        })
        .unwrap();
    // The banner shows on the member's pages.
    let page = send(&app, get("/@ada", Some(&sid))).await.text;
    assert!(page.contains("@ada is now listed on c.") && page.contains("/notices/dismiss"));
    // Without the launch token, nothing is collected.
    let no = Request::post("/api/notices/claim").body(Body::empty()).unwrap();
    assert_eq!(send(&app, no).await.status, StatusCode::FORBIDDEN);
    let yes = Request::post("/api/notices/claim").header("x-hic-launch", "tok").body(Body::empty()).unwrap();
    assert!(send(&app, yes).await.text.contains("now listed"));
    let again = Request::post("/api/notices/claim").header("x-hic-launch", "tok").body(Body::empty()).unwrap();
    assert_eq!(send(&app, again).await.text, "[]", "each notice is collected once");
    // Dismissing hides the banner.
    let csrf = csrf_in(&page);
    send(&app, post_form("/notices/dismiss", &format!("csrf={csrf}"), Some(&sid))).await;
    assert!(!send(&app, get("/@ada", Some(&sid))).await.text.contains("now listed"));
}
