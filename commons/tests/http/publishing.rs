//! Publishing a profile and how its reach shows in the directory.

use crate::common::{get, join_and_publish, send, server};
use hic_commons::role::Role;

#[tokio::test]
async fn publish_then_listed_in_directory() {
    let (app, _) = server(Role::Node);
    join_and_publish(&app, "tega", "public").await;
    let r = send(&app, get("/", None)).await;
    assert!(r.text.contains("@tega"));
}

#[tokio::test]
async fn commons_reach_is_marked_noindex() {
    let (app, _) = server(Role::Node);
    join_and_publish(&app, "tega", "commons").await;
    let r = send(&app, get("/@tega", None)).await;
    assert_eq!(r.headers.get("x-robots-tag").unwrap(), "noindex");
    assert!(r.text.contains(r#"<meta name="robots" content="noindex">"#));

    join_and_publish(&app, "mo", "public").await;
    assert!(send(&app, get("/@mo", None)).await.headers.get("x-robots-tag").is_none());
}
