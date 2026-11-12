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
    /// The address to tell people: the public one (`HIC_ALSO_KNOWN_AS`, the
    /// one people outside the room use) when there is one, else this
    /// server's own. Shown in the rail and written into invites.
    pub fn public_url(&self) -> &str {
        self.also_known_as.first().unwrap_or(&self.base_url)
    }

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

/// Which main-menu link a page belongs to, so the nav can mark it as the
/// current page (`aria-current`, drawn as a dot above the link).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// The directory, the front page.
    Directory,
    /// The front page as the steward sees it, with their tools.
    Steward,
    /// Story lists and story pages.
    Stories,
    /// The signed-in member's own profile and its forms.
    You,
    /// Signing up (or, on a commons, how to join).
    Join,
    /// Signing in.
    SignIn,
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
    /// The main-menu section this page belongs to, if any.
    pub section: Option<Section>,
}

impl<'a> Page<'a> {
    /// A normal, indexable page with status 200.
    pub fn new(title: &'a str, me: Option<&'a str>, body: Markup) -> Page<'a> {
        Page { title, me, body, noindex: false, status: StatusCode::OK, notices: None, section: None }
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

    /// Say which main-menu section this page belongs to.
    pub fn at(mut self, section: Section) -> Self {
        self.section = Some(section);
        self
    }

    /// Use a different status code.
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }
}

/// Which of the eight quilt backgrounds a page shows (1 to 8), from its
/// title: the same page always gets the same quilt, different pages usually
/// differ. FNV-1a, a simple and stable hash (the standard library's hasher
/// may change between Rust versions, which would reshuffle every page).
pub fn quilt_for(title: &str) -> usize {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in title.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash % 8) as usize + 1
}

/// What kind of server this is, as the rail names it, and the class that
/// picks its starburst colour: your node, a shared commons or a practice
/// commons, so nobody confuses them.
fn server_kind(site: &Site) -> (&'static str, &'static str) {
    match (site.is_commons(), site.practice) {
        (true, true) => ("is-practice", "Practice commons"),
        (true, false) if site.in_app => ("is-commons", "Your commons"),
        (true, false) => ("is-commons", "Shared commons"),
        (false, _) => ("is-node", "Your node"),
    }
}

/// The side rail: the logo, the server this page belongs to, a slot the
/// desktop app fills with the other servers on this computer (see
/// `src-tauri/src/links.rs`), and a line about where this server reaches.
fn rail(site: &Site) -> Markup {
    let (kind, label) = server_kind(site);
    html! {
        aside class="rail" {
            a class="wordmark" href="/" {
                img class="wordmark-logo" src="/logo.png" alt="Held in Common" width="173" height="46";
                span class="tagline" { "Federated data commons for creative communities" }
            }
            section class="servers" aria-labelledby="hic-servers-label" {
                p class="rail-label" id="hic-servers-label" {
                    @if site.in_app { "On this computer" } @else { "You're on" }
                }
                div class={ "server-card is-here " (kind) } aria-current="true" {
                    span class="star" aria-hidden="true" {}
                    span class="server-text" {
                        span class="server-name" { (label) }
                        @if !site.instance.is_empty() {
                            span class="server-sub" {
                                (site.instance) @if site.in_app { ", open now" }
                            }
                        }
                    }
                }
                div class="server-list" id="hic-servers" hidden {}
            }
            @if site.is_commons() && site.practice {
                p class="rail-foot" { "Only this computer can reach a practice commons. Nothing here is seen by anyone else." }
            } @else if site.is_commons() {
                p class="rail-foot" {
                    "Nodes share to "
                    code { (site.public_url()) }
                }
            } @else if !site.share_default.is_empty() {
                a class="rail-foot" href=(site.share_default) { "Visit the commons \u{2197}" }
            }
        }
    }
}

/// `Some("page")` when `page` belongs to `section`, for a nav link's
/// `aria-current` attribute; `None` leaves the attribute off.
fn current(page: &Page, section: Section) -> Option<&'static str> {
    (page.section == Some(section)).then_some("page")
}

/// The full HTML document around a page body: the quilt behind, the rail on
/// the left, and the page panel (nav, news, the page, the footer).
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
            body class={ "quilt-" (quilt_for(page.title)) } {
                div class="frame" {
                    (rail(site))
                    div class="stage" {
                        nav class="topbar-links" aria-label="Main" {
                            a href="/" aria-current=[current(page, Section::Directory)] { "Directory" }
                            a href="/stories" aria-current=[current(page, Section::Stories)] { "Stories" }
                            @if page.me == Some("@steward") {
                                a href="/" aria-current=[current(page, Section::Steward)] { "Steward view" }
                                form method="post" action="/logout" class="inline-form" {
                                    button class="linklike" type="submit" { "Sign out" }
                                }
                            } @else if let Some(h) = page.me {
                                a href={ "/@" (h) } aria-current=[current(page, Section::You)] { "@" (h) }
                                form method="post" action="/logout" class="inline-form" {
                                    button class="linklike" type="submit" { "Sign out" }
                                }
                            } @else if site.is_commons() {
                                a href="/join" aria-current=[current(page, Section::Join)] { "Run a node to join" }
                            } @else {
                                a href="/join" aria-current=[current(page, Section::Join)] { "Add your profile" }
                                a href="/login" aria-current=[current(page, Section::SignIn)] { "Sign in" }
                            }
                            a href=(PROJECT_URL) { "About the project \u{2197}" }
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
                        main class="page" { (page.body) }
                        footer class="site-foot" {
                            span { "A federated commons for creative data." }
                            span class="foot-links" {
                                @if !site.is_commons() { a href="/run-a-commons" { "Run your own commons" } }
                                a href="/about" { "How this works" }
                                a href=(PROJECT_URL) { "About the project \u{2197}" }
                            }
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

/// A decorative colour for a card, picked from its key so a member keeps the
/// same colour everywhere: one of the four palette tones. Decoration only:
/// where colour means something (reach), the card also says it in words.
pub fn tone_for_key(key_hex: &str) -> &'static str {
    /// The four palette tones a card can take (classes in style.css).
    const TONES: [&str; 4] = ["tone-pink", "tone-blue", "tone-jade", "tone-violet"];
    // The sum of the first twelve hex digits spreads tones more evenly than
    // one digit would.
    let sum: u32 = key_hex.chars().take(12).filter_map(|c| c.to_digit(16)).sum();
    TONES[sum as usize % TONES.len()]
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
        assert!(m.contains("Nodes share to <code>https://hic.tail.ts.net</code>"), "only the public address");
        assert!(!m.contains("<code>http://c</code>"), "not the local one as well");
    }

    #[test]
    fn each_page_gets_a_stable_quilt_and_pages_spread_across_them() {
        assert_eq!(quilt_for("The commons"), quilt_for("The commons"), "the same page, the same quilt");
        let titles =
            ["The commons", "Stories", "Sign in", "Join", "Your recovery code", "Publish", "Invite code", "Vouched"];
        let quilts: std::collections::HashSet<usize> = titles.iter().map(|t| quilt_for(t)).collect();
        assert!(quilts.iter().all(|q| (1..=8).contains(q)));
        assert!(quilts.len() >= 4, "eight pages use at least four different quilts: {quilts:?}");
        let page = frame(&site(Role::Node), &Page::new("Stories", None, html! {})).into_string();
        assert!(page.contains(&format!("class=\"quilt-{}\"", quilt_for("Stories"))));
    }

    #[test]
    fn the_rail_names_this_server_and_leaves_a_slot_for_the_others() {
        let node = frame(&site(Role::Node), &Page::new("t", None, html! {})).into_string();
        assert!(node.contains("Your node") && node.contains("You're on"));
        assert!(node.contains(r#"id="hic-servers""#), "the app fills this slot");
        let in_app = Site { in_app: true, ..site(Role::Commons) };
        let commons = frame(&in_app, &Page::new("t", None, html! {})).into_string();
        assert!(
            commons.contains("Your commons") && commons.contains("On this computer") && commons.contains("open now")
        );
    }

    #[test]
    fn a_card_keeps_its_tone_and_tones_vary_by_key() {
        assert_eq!(tone_for_key("c74bb4aca6fc"), tone_for_key("c74bb4aca6fc"), "the same key, the same tone");
        let tones: std::collections::HashSet<&str> = ["0", "1", "2", "3"].iter().map(|k| tone_for_key(k)).collect();
        assert_eq!(tones.len(), 4);
        assert_eq!(tone_for_key(""), "tone-pink", "an empty key still gets a tone");
    }

    #[test]
    fn the_nav_marks_only_the_section_you_are_in() {
        let page = frame(&site(Role::Node), &Page::new("t", None, html! {}).at(Section::Stories)).into_string();
        assert!(page.contains(r#"href="/stories" aria-current="page""#));
        assert_eq!(page.matches(r#"aria-current="page""#).count(), 1, "one section at a time");
        let none = frame(&site(Role::Node), &Page::new("t", None, html! {})).into_string();
        assert!(!none.contains(r#"aria-current="page""#), "a page outside the menu marks nothing");
        let steward = Page::new("t", Some("@steward"), html! {}).at(Section::Steward);
        let page = frame(&site(Role::Commons), &steward).into_string();
        assert!(page.contains(r#"aria-current="page">Steward view"#), "the steward view, not the directory");
        assert_eq!(page.matches(r#"aria-current="page""#).count(), 1);
    }

    #[test]
    fn people_are_told_the_public_address_when_there_is_one() {
        assert_eq!(site(Role::Commons).public_url(), "http://c", "no public address: this server's own");
        let public = Site { also_known_as: vec!["https://hic.tail.ts.net".into()], ..site(Role::Commons) };
        assert_eq!(public.public_url(), "https://hic.tail.ts.net");
        let invite = crate::web::pages::steward_invite(&public, "hic-invite-x").into_string();
        assert!(invite.contains("hic.tail.ts.net"), "the emailed invite names the public address");
        assert!(!invite.contains("http%3A%2F%2Fc"), "not the local one");
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
