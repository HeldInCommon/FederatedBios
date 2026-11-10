//! The page frame every HTML response shares, and small shared pieces
//! (badges, the licence row, redirects, error pages).
//!
//! All functions here are pure: they turn data into markup or a response.

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use maud::{html, Markup, DOCTYPE};

use crate::governance::{license_name, license_url, signal_name};
use crate::model::Reach;
use crate::role::Role;

/// What every page needs to know about this server.
#[derive(Clone, Debug)]
pub struct Site {
    pub role: Role,
    /// This server's name ("Phoenix's node").
    pub instance: String,
    /// This server's public address.
    pub base_url: String,
    /// The default commons a node shares to (blank if none).
    pub share_default: String,
}

impl Site {
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
/// someone's node or a shared commons, so nobody confuses the two.
fn role_banner(site: &Site) -> Markup {
    html! {
        div class={ "role-bar " (if site.is_commons() { "role-commons" } else { "role-node" }) } {
            span class="role-dot" {}
            span class="role-label" { (if site.is_commons() { "Shared commons" } else { "Your node" }) }
            @if !site.instance.is_empty() { span class="role-instance" { "· " (site.instance) } }
            @if !site.is_commons() && !site.share_default.is_empty() {
                a class="role-link" href=(site.share_default) { "Visit the commons \u{2197}" }
            }
            @if site.is_commons() {
                span class="role-address" { "Nodes share to " code { (site.base_url) } }
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
                        a class="wordmark" href="/" { "Held in Common" span class="dot" { "." } }
                        span class="topbar-links" {
                            a href="/" { "Directory" }
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
                        a href="/about" { "How this works" }
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

/// The licence link and signal chips for a version.
pub fn gov_row(license: &str, signals: &[String]) -> Markup {
    html! {
        div class="gov" {
            @if license_url(license).is_empty() {
                span class="gov-license" { (license_name(license)) }
            } @else {
                a class="gov-license" rel="license" href=(license_url(license)) { (license_name(license)) }
            }
            @for s in signals {
                span class="signal-chip" { (signal_name(s)) }
            }
        }
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

    fn site(role: Role) -> Site {
        Site { role, instance: "Test".into(), base_url: "http://c".into(), share_default: String::new() }
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
    }

    #[test]
    fn host_label_strips_scheme_and_slash() {
        assert_eq!(host_label("https://c.example/"), "c.example");
        assert_eq!(host_label("http://localhost:8081"), "localhost:8081");
    }
}
