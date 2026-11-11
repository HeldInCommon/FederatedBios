//! The page frame every HTML response shares, and small shared pieces
//! (badges, the licence row, redirects, error pages).
//!
//! All functions here are pure: they turn data into markup or a response.

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use maud::{html, Markup, DOCTYPE};

use crate::model::Reach;
use crate::role::Role;

/// The project's own site, linked from every page's footer.
pub const PROJECT_URL: &str = "https://heldincommon.org";

/// What every page needs to know about this server.
#[derive(Clone, Debug)]
pub struct Site {
    pub role: Role,
    /// This server's name ("Phoenix's node").
    pub instance: String,
    /// This server's public address.
    pub base_url: String,
    /// Its other addresses (`HIC_ALSO_KNOWN_AS`), shown first on a commons:
    /// usually the one people outside the room use.
    pub also_known_as: Vec<String>,
    /// The default commons a node shares to (blank if none).
    pub share_default: String,
    /// A practice commons, for learning (`HIC_PRACTICE=1`).
    pub practice: bool,
    /// Started by the desktop app, so pages may offer its actions.
    pub in_app: bool,
}

impl Site {
    /// True when this server is a commons (not a node).
    pub fn is_commons(&self) -> bool {
        self.role == Role::Commons
    }
}

/// News for the signed-in member, shown as a banner with a Dismiss button.
pub struct NoticeBar {
    pub texts: Vec<String>,
    /// The session's CSRF token, for the Dismiss form.
    pub csrf: String,
}

/// One HTML page to send.
pub struct Page<'a> {
    pub title: &'a str,
    /// The signed-in handle, if any (changes the top links).
    pub me: Option<&'a str>,
    pub body: Markup,
    /// Ask search engines not to index this page (the `commons` reach).
    pub noindex: bool,
    pub status: StatusCode,
    /// News for the signed-in member, if any.
    pub notices: Option<NoticeBar>,
}

impl<'a> Page<'a> {
    /// A normal, indexable page with status 200.
    pub fn new(title: &'a str, me: Option<&'a str>, body: Markup) -> Page<'a> {
        Page { title, me, body, noindex: false, status: StatusCode::OK, notices: None }
    }

    /// Mark this page as not for search engines.
    pub fn noindex(mut self, yes: bool) -> Self {
        self.noindex = yes;
        self
    }

    /// Show news for the signed-in member.
    pub fn notices(mut self, n: Option<NoticeBar>) -> Self {
        self.notices = n;
        self
    }

    /// Use a different status code.
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }
}

/// The coloured bar at the top of every page, saying whether this is
/// someone's node, a shared commons or a practice commons, so nobody confuses
/// them.
fn role_banner(site: &Site) -> Markup {
    let (class, label) = match (site.is_commons(), site.practice) {
        (true, true) => ("role-practice", "Practice commons"),
        (true, false) => ("role-commons", "Shared commons"),
        (false, _) => ("role-node", "Your node"),
    };
    html! {
        div class={ "role-bar " (class) } {
            span class="role-dot" {}
            span class="role-label" { (label) }
            @if !site.instance.is_empty() { span class="role-instance" { "· " (site.instance) } }
            @if !site.is_commons() && !site.share_default.is_empty() {
                a class="role-link" href=(site.share_default) { "Visit the commons \u{2197}" }
            }
            @if site.is_commons() {
                span class="role-address" {
                    "Nodes share to "
                    @for url in &site.also_known_as { code { (url) } " or " }
                    code { (site.base_url) }
                }
            }
        }
    }
}

/// The full HTML document around a page body.
fn frame(site: &Site, page: &Page) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                @if page.noindex { meta name="robots" content="noindex"; }
                title { (page.title) " — Held in Common" }
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link href="https://fonts.googleapis.com/css2?family=Space+Mono:ital,wght@0,400;0,700;1,400&family=Space+Grotesk:wght@400;500;600;700&family=Cormorant:ital,wght@0,500;0,600;1,500&display=swap" rel="stylesheet";
                link rel="stylesheet" href="/style.css";
            }
            body {
                (role_banner(site))
                main class="page" {
                    nav class="topbar" {
                        a class="wordmark" href="/" {
                            img class="wordmark-logo" src="/logo.png" alt="Held in Common" width="173" height="46";
                            span class="tagline" { "Federated data commons for creative communities" }
                        }
                        span class="topbar-links" {
                            a href="/" { "Directory" }
                            a href="/stories" { "Stories" }
                            @if page.me == Some("@steward") {
                                a href="/" { "Steward view" }
                                form method="post" action="/logout" class="inline-form" {
                                    button class="linklike" type="submit" { "Sign out" }
                                }
                            } @else if let Some(h) = page.me {
                                a href={ "/@" (h) } { "@" (h) }
                                form method="post" action="/logout" class="inline-form" {
                                    button class="linklike" type="submit" { "Sign out" }
                                }
                            } @else if site.is_commons() {
                                a href="/join" { "Run a node to join" }
                            } @else {
                                a href="/join" { "Add your profile" }
                                a href="/login" { "Sign in" }
                            }
                            a href=(PROJECT_URL) { "About the project \u{2197}" }
                        }
                    }
                    @if let Some(n) = &page.notices {
                        section class="notice-bar" role="status" {
                            @for t in &n.texts { p { (t) } }
                            form method="post" action="/notices/dismiss" {
                                input type="hidden" name="csrf" value=(n.csrf);
                                button class="secondary" type="submit" { "Dismiss" }
                            }
                        }
                    }
                    (page.body)
                    footer class="site-foot" {
                        span { "A federated commons for creative data." }
                        span class="foot-links" {
                            @if !site.is_commons() { a href="/run-a-commons" { "Run your own commons" } }
                            a href="/about" { "How this works" }
                            a href=(PROJECT_URL) { "About the project \u{2197}" }
                        }
                    }
                }
                script src="/password.js" {}
            }
        }
    }
}

/// Render a page into a response (with `X-Robots-Tag: noindex` when asked).
pub fn render(site: &Site, page: Page) -> Response {
    let markup = frame(site, &page);
    let mut resp = (page.status, Html(markup.into_string())).into_response();
    if page.noindex {
        resp.headers_mut().insert("x-robots-tag", HeaderValue::from_static("noindex"));
    }
    resp
}

/// The coloured label for a reach.
pub fn reach_badge(reach: Reach) -> Markup {
    let (label, class) = match reach {
        Reach::Public => ("Public", "vis-public"),
        Reach::Commons => ("Commons", "vis-commons"),
        Reach::Members => ("Members only", "vis-members"),
    };
    html! { span class={ "vis-badge " (class) } { (label) } }
}

/// The CSS class for a reach's colour bar on a directory card.
pub fn reach_bar_class(reach: Reach) -> &'static str {
    match reach {
        Reach::Public => "vis-public",
        Reach::Commons => "vis-commons",
        Reach::Members => "vis-members",
    }
}

/// A page explaining a problem the person can fix (status 400).
pub fn error_page(site: &Site, msg: &str) -> Response {
    message_page(site, "Error", "Hmm", msg, StatusCode::BAD_REQUEST)
}

/// A page for a failure on our side (status 500). The details go to the log,
/// not the page.
pub fn server_error(site: &Site, detail: &dyn std::fmt::Display) -> Response {
    eprintln!("error: {detail}");
    message_page(
        site,
        "Error",
        "Something went wrong",
        "The server couldn't complete that. Try again; if it keeps happening, tell whoever runs this server.",
        StatusCode::INTERNAL_SERVER_ERROR,
    )
}

/// The 404 page.
pub fn not_found(site: &Site, me: Option<&str>) -> Response {
    let body = html! {
        header class="intro" { h1 { "Not found" } p class="lede" { "No such page or handle." } }
        div class="actions" { a class="primary" href="/" { "Back to the directory" } }
    };
    render(site, Page::new("Not found", me, body).status(StatusCode::NOT_FOUND))
}

/// A page with one heading and one message, sent with `status`: the base of the error pages.
fn message_page(site: &Site, title: &str, heading: &str, msg: &str, status: StatusCode) -> Response {
    let body = html! {
        header class="intro" { h1 { (heading) } p class="lede" { (msg) } }
        div class="actions" { a class="primary" href="/" { "Back to the directory" } }
    };
    render(site, Page::new(title, None, body).status(status))
}

/// A `303 See Other` to `to`.
pub fn redirect(to: &str) -> Response {
    redirect_with(to, None)
}

/// A `303 See Other` to `to`, optionally setting a cookie.
pub fn redirect_with(to: &str, set_cookie: Option<String>) -> Response {
    let mut h = HeaderMap::new();
    h.insert(header::LOCATION, HeaderValue::from_str(to).unwrap_or(HeaderValue::from_static("/")));
    if let Some(c) = set_cookie.and_then(|c| HeaderValue::from_str(&c).ok()) {
        h.insert(header::SET_COOKIE, c);
    }
    (StatusCode::SEE_OTHER, h).into_response()
}

/// A readable label for a server address: its host, without scheme or
/// trailing slash.
pub fn host_label(url: &str) -> String {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url)
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A site in `role`, for these tests.
    fn site(role: Role) -> Site {
        Site {
            role,
            instance: "Test".into(),
            base_url: "http://c".into(),
            also_known_as: Vec::new(),
            share_default: String::new(),
            practice: false,
            in_app: false,
        }
    }

    #[test]
    fn noindex_sets_meta_and_header() {
        let r = render(&site(Role::Node), Page::new("t", None, html! {}).noindex(true));
        assert_eq!(r.headers().get("x-robots-tag").unwrap(), "noindex");
        let r = render(&site(Role::Node), Page::new("t", None, html! {}));
        assert!(r.headers().get("x-robots-tag").is_none());
    }

    #[test]
    fn commons_banner_shows_share_address() {
        let m = frame(&site(Role::Commons), &Page::new("t", None, html! {})).into_string();
        assert!(m.contains("Nodes share to"));
        assert!(m.contains("Shared commons"));
        let public = Site { also_known_as: vec!["https://hic.tail.ts.net".into()], ..site(Role::Commons) };
        let m = frame(&public, &Page::new("t", None, html! {})).into_string();
        assert!(
            m.contains("<code>https://hic.tail.ts.net</code> or <code>http://c</code>"),
            "the public address comes first"
        );
    }

    #[test]
    fn practice_commons_says_so() {
        let site = Site { practice: true, ..site(Role::Commons) };
        let m = frame(&site, &Page::new("t", None, html! {})).into_string();
        assert!(m.contains("Practice commons"));
        assert!(!m.contains("Shared commons"));
    }

    #[test]
    fn only_a_node_links_to_the_commons_guide() {
        let node = frame(&site(Role::Node), &Page::new("t", None, html! {})).into_string();
        let commons = frame(&site(Role::Commons), &Page::new("t", None, html! {})).into_string();
        assert!(node.contains("/run-a-commons"));
        assert!(!commons.contains("/run-a-commons"));
    }

    #[test]
    fn every_page_has_the_logo_and_the_project_link() {
        for role in [Role::Node, Role::Commons] {
            let m = frame(&site(role), &Page::new("t", None, html! {})).into_string();
            assert!(m.contains("src=\"/logo.png\""));
            assert!(m.contains(PROJECT_URL));
        }
    }

    #[test]
    fn host_label_strips_scheme_and_slash() {
        assert_eq!(host_label("https://c.example/"), "c.example");
        assert_eq!(host_label("http://localhost:8081"), "localhost:8081");
    }
}
