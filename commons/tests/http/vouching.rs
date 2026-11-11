//! Joining a commons that needs a vouch (decision record 0011): waiting,
//! founding invites, members' invites and vouches, the weekly limit.

use crate::common::{closed_commons, get, join_and_publish, person, post_json, send, server, share_of, COMMONS_URL};
use axum::http::StatusCode;
use hic_commons::role::Role;

#[tokio::test]
async fn without_a_vouch_a_newcomer_waits() {
    let (commons, _) = closed_commons(5);
    let r = send(&commons, post_json("/inbox", &share_of(&person("newbie"), None))).await;
    assert_eq!(r.status, StatusCode::ACCEPTED);
    assert!(r.text.starts_with("pending"));
    let dir = send(&commons, get("/", None)).await.text;
    assert!(dir.contains("Waiting for a vouch") && !dir.contains("dir-card"), "listed only as waiting");
    assert!(send(&commons, get("/api/pending", None)).await.text.contains("newbie"));
}

#[tokio::test]
async fn founding_invite_admits_the_first_person_once() {
    let (commons, store) = closed_commons(5);
    store.save_founding_code("hic-founding-abc123").unwrap();
    let first = send(&commons, post_json("/inbox", &share_of(&person("steward"), Some("hic-founding-abc123")))).await;
    assert_eq!(first.text, "accepted");
    let again = send(&commons, post_json("/inbox", &share_of(&person("other"), Some("hic-founding-abc123")))).await;
    assert_eq!(again.status, StatusCode::FORBIDDEN);
    assert_eq!(store.intros()[0].route, "founding");
}

#[tokio::test]
async fn a_members_invite_admits_once_and_is_recorded() {
    let (commons, store) = closed_commons(5);
    store.save_founding_code("hic-founding-x1").unwrap();
    let m = person("member");
    send(&commons, post_json("/inbox", &share_of(&m, Some("hic-founding-x1")))).await;

    let inv = hic_commons::vouching::make_invite(&m.ed_pub, &m.ed_priv, COMMONS_URL, hic_commons::now()).unwrap();
    let code = hic_commons::vouching::invite_to_code(&inv);
    let r = send(&commons, post_json("/inbox", &share_of(&person("friend"), Some(&code)))).await;
    assert_eq!(r.text, "accepted");
    let reused = send(&commons, post_json("/inbox", &share_of(&person("third"), Some(&code)))).await;
    assert_eq!(reused.status, StatusCode::FORBIDDEN);
    assert!(reused.text.contains("already been used"));
    let friend = store.intros().into_iter().find(|i| i.handle == "friend").unwrap();
    assert_eq!(friend.voucher.as_deref(), Some(m.ed_pub.as_str()));
}

#[tokio::test]
async fn a_vouch_lists_someone_waiting() {
    let (commons, store) = closed_commons(5);
    store.save_founding_code("hic-founding-x2").unwrap();
    let m = person("member");
    send(&commons, post_json("/inbox", &share_of(&m, Some("hic-founding-x2")))).await;
    let newbie = person("newbie");
    send(&commons, post_json("/inbox", &share_of(&newbie, None))).await;

    let stranger = person("stranger");
    let bad = hic_commons::vouching::make_vouch(
        &stranger.ed_pub,
        &stranger.ed_priv,
        COMMONS_URL,
        "newbie",
        &newbie.ed_pub,
        hic_commons::now(),
    )
    .unwrap();
    assert_eq!(
        send(&commons, post_json("/inbox", &serde_json::to_value(&bad).unwrap())).await.status,
        StatusCode::FORBIDDEN
    );

    let v = hic_commons::vouching::make_vouch(
        &m.ed_pub,
        &m.ed_priv,
        COMMONS_URL,
        "newbie",
        &newbie.ed_pub,
        hic_commons::now(),
    )
    .unwrap();
    let r = send(&commons, post_json("/inbox", &serde_json::to_value(&v).unwrap())).await;
    assert_eq!(r.text, "accepted");
    assert!(store.pending().is_empty());
    assert!(send(&commons, get("/", None)).await.text.contains("@newbie"));
}

#[tokio::test]
async fn the_steward_sets_the_weekly_limit() {
    let (commons, store) = closed_commons(1);
    store.save_founding_code("hic-founding-x3").unwrap();
    let m = person("member");
    send(&commons, post_json("/inbox", &share_of(&m, Some("hic-founding-x3")))).await;
    let code = |_: u8| {
        hic_commons::vouching::invite_to_code(
            &hic_commons::vouching::make_invite(&m.ed_pub, &m.ed_priv, COMMONS_URL, hic_commons::now()).unwrap(),
        )
    };
    assert_eq!(send(&commons, post_json("/inbox", &share_of(&person("one"), Some(&code(1))))).await.text, "accepted");
    let r = send(&commons, post_json("/inbox", &share_of(&person("two"), Some(&code(2))))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(r.text.contains("as many people"));
}

#[tokio::test]
async fn people_already_listed_stay_and_reshare_freely() {
    let (commons, store) = closed_commons(5);
    store.save_founding_code("hic-founding-x4").unwrap();
    let m = person("member");
    send(&commons, post_json("/inbox", &share_of(&m, Some("hic-founding-x4")))).await;
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let r = send(&commons, post_json("/inbox", &share_of(&m, None))).await;
    assert_eq!(r.text, "accepted", "a returning member needs no new vouch");
}

#[tokio::test]
async fn member_invite_page_can_be_emailed_and_passphrase_boxes_get_an_eye() {
    let (app, _) = server(Role::Node);
    assert!(send(&app, get("/join", None)).await.text.contains(r#"<script src="/password.js">"#));
    let js = send(&app, get("/password.js", None)).await;
    assert!(js.text.contains("Show passphrase"));
    // A members-only profile explains why it can't be shared.
    let sid = join_and_publish(&app, "mo", "members").await;
    assert!(send(&app, get("/@mo?tab=sharing", Some(&sid)))
        .await
        .text
        .contains("so it stays on your node and is never shared"));
}

/// A room commons that is also public (through Tailscale Funnel, say) accepts
/// vouches addressed to its public name, and still refuses ones for any
/// other commons.
#[tokio::test]
async fn a_vouch_for_the_commons_public_address_is_accepted() {
    use hic_commons::app::{router, AppState};
    use hic_commons::config::Config;
    use hic_commons::role::Role;
    use hic_commons::store::Store;
    let dir = std::env::temp_dir().join(format!("hic-http-aka-{}", hic_commons::sessions::random_hex(6)));
    let store = Store::open(&dir).unwrap();
    let config = Config::from_lookup(|k| match k {
        "PORT" => Some("8081".into()),
        "HIC_BASE_URL" => Some("http://10.0.0.5:8081".into()),
        "HIC_ALSO_KNOWN_AS" => Some("https://hic.tail.ts.net".into()),
        _ => None,
    });
    let commons = router(std::sync::Arc::new(AppState::new(config, Role::Commons, store.clone())));
    store.save_founding_code("hic-founding-p1").unwrap();
    let m = person("member");
    send(&commons, post_json("/inbox", &share_of(&m, Some("hic-founding-p1")))).await;
    let newbie = person("newbie");
    send(&commons, post_json("/inbox", &share_of(&newbie, None))).await;

    let vouch_for = |commons_url: &str| {
        let v = hic_commons::vouching::make_vouch(
            &m.ed_pub,
            &m.ed_priv,
            commons_url,
            "newbie",
            &newbie.ed_pub,
            hic_commons::now(),
        )
        .unwrap();
        serde_json::to_value(&v).unwrap()
    };
    let elsewhere = send(&commons, post_json("/inbox", &vouch_for("https://another.example"))).await;
    assert_eq!(elsewhere.status, StatusCode::FORBIDDEN, "a vouch for another commons is refused");
    let r = send(&commons, post_json("/inbox", &vouch_for("https://hic.tail.ts.net"))).await;
    assert_eq!(r.text, "accepted");
    assert!(store.pending().is_empty());
}
