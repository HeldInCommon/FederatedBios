//! The steward view of a commons (decision record 0017).

use crate::common::{closed_commons, get, person, post_form, post_json, send, share_of, steward_session};
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
