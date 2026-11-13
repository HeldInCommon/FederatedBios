//! The page frame every HTML response shares, and small shared pieces
//! (badges, the licence row, redirects, error pages).
//!
//! All functions here are pure: they turn data into markup or a response.
//! One documented exception: [`name_default_commons`] records, once at
//! start-up, what to call the node's default commons, and [`host_label`]
//! reads it.

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use maud::{html, Markup, DOCTYPE};
use std::sync::OnceLock;

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
    /// What to call it ("Held in Common Collective Commons"; blank: its address).
    pub share_default_name: String,
    /// A practice commons, for learning (`HIC_PRACTICE=1`).
    pub practice: bool,
    /// Started by the desktop app, so pages may offer its actions.
    pub in_app: bool,
    /// While open to the room (decision record 0027): the address the room
    /// reaches it at. `None` while closed.
    pub room: Option<String>,
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
    /// How this works.
    About,
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

/// Where a server is and who can reach it, as the rail says it. Holding this
/// apart is the idea: your work stays on your node, and a commons is a shared
/// place, so the rail always says which is which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Your node: on this computer, only you.
    Node,
    /// A local commons (`HIC_PRACTICE`): on its host's computer, reached by
    /// anyone on the same Wi-Fi.
    Local,
    /// A commons whose address is on a local network (a room commons).
    OnThisWifi,
    /// A commons anyone can reach: a public address, its own or one it is
    /// also known by (Tailscale, a server).
    Online,
}

impl Place {
    /// Where `site` is.
    pub fn of(site: &Site) -> Place {
        match (site.is_commons(), site.practice) {
            (false, _) => Place::Node,
            (true, true) => Place::Local,
            (true, false) if !site.also_known_as.is_empty() || !is_local_address(&site.base_url) => Place::Online,
            (true, false) => Place::OnThisWifi,
        }
    }

    /// The kind, as the rail's card names it: "Your node", "Local commons"…
    pub fn kind_name(self) -> &'static str {
        match self {
            Place::Node => "Your node",
            Place::Local => "Local commons",
            Place::OnThisWifi => "Commons on this Wi-Fi",
            Place::Online => "Online commons",
        }
    }

    /// Where it is and who can reach it, in a few words, given whether it's
    /// `open` to the room (decision record 0027; only a node or a local
    /// commons has the switch).
    pub fn reach(self, open: bool) -> &'static str {
        match (self, open) {
            (Place::Node, false) => "on this computer, only you",
            (Place::Node, true) => "open to the room: public pages only",
            (Place::Local, false) => "closed to the room",
            (Place::Local, true) => "open to the room",
            (Place::OnThisWifi, _) => "anyone on this Wi-Fi",
            (Place::Online, _) => "anyone with its address",
        }
    }

    /// The class that picks the card's starburst colour.
    fn class(self) -> &'static str {
        match self {
            Place::Node => "is-node",
            Place::Local => "is-practice",
            Place::OnThisWifi | Place::Online => "is-commons",
        }
    }
}

/// True when `url`'s host is on this computer or a local network (a private,
/// loopback or `.local` address), so only people nearby can reach it.
fn is_local_address(url: &str) -> bool {
    let host = url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or("");
    // Drop the port; keep an IPv6 address in brackets intact.
    let host = match host.rsplit_once(':') {
        Some((h, port)) if port.chars().all(|c| c.is_ascii_digit()) && !h.ends_with(':') => h,
        _ => host,
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    // A name with no dot ("studio", "localhost") can only be found nearby.
    if !host.contains('.') && !host.contains(':') || host.ends_with(".local") {
        return true;
    }
    host.parse::<std::net::IpAddr>().is_ok_and(|ip| !matches!(crate::outbound::kind(ip), crate::outbound::Kind::Public))
}

/// Said beside a Wi-Fi address: the router hands it out and can change it,
/// and the desktop app moves the commons when it does (`watch_addresses` in
/// `src-tauri/src/sidecar.rs`).
const ADDRESS_CAN_CHANGE: &str = "Your Wi-Fi gives this address and can change it; if it does, the app moves the commons to the new one and tells you.";

/// Said beside the address of a node or local commons open to the room: it
/// closes by itself (decision record 0027, D3).
const ROOM_CLOSES: &str =
    "It closes when you close it in the Go menu, quit the app, or this computer's network address changes.";

/// The side rail: the logo, the server this page belongs to, a slot the
/// desktop app fills with the other servers on this computer (see
/// `src-tauri/src/links.rs`), and a line about where this server reaches.
fn rail(site: &Site) -> Markup {
    let place = Place::of(site);
    let reach = place.reach(site.room.is_some());
    html! {
        aside class="rail" {
            a class="wordmark" href="/" {
                img class="wordmark-logo" src="/logo.png" alt="Held in Common" width="173" height="46";
                span class="tagline" { "Federated data commons for creative communities" }
            }
            section class="servers" aria-labelledby="hic-servers-label" {
                // Visitors to a commons reach it over the network, so only
                // a node says "On this computer".
                p class="rail-label" id="hic-servers-label" {
                    @if site.in_app && !site.is_commons() { "On this computer" } @else { "You're on" }
                }
                div class={ "server-card is-here " (place.class()) } aria-current="true" {
                    span class="star" aria-hidden="true" {}
                    span class="server-text" {
                        // A commons leads with its own name ("Held in Common
                        // Collective Commons"), then what it is and who can
                        // reach it; a node with "Your node", then its name.
                        @if site.is_commons() && !site.instance.trim().is_empty() {
                            span class="server-name" { (site.instance) }
                            span class="server-sub" { (place.kind_name()) ": " (reach) }
                        } @else if site.is_commons() {
                            span class="server-name" { (place.kind_name()) }
                            span class="server-sub" { (reach) }
                        } @else {
                            span class="server-name" { (place.kind_name()) }
                            span class="server-sub" {
                                @if !site.instance.trim().is_empty() { (site.instance) ": " }
                                (reach)
                            }
                        }
                    }
                }
                // Open to the room: the one state here someone may need to
                // act on (close it), so an oxblood pop, said in words.
                @if site.room.is_some() && (site.practice || !site.is_commons()) {
                    p class="room-pop" { "Open to the room" }
                }
                div class="server-list" id="hic-servers" hidden {}
            }
            @if site.is_commons() && site.practice {
                @if site.room.is_some() {
                    p class="rail-foot" {
                        "Anyone on this Wi-Fi can share to "
                        code { (site.base_url) }
                        ". It's here while this computer is awake and the app is open. "
                        (ROOM_CLOSES)
                    }
                } @else {
                    p class="rail-foot" {
                        "Closed to the room: only this computer can reach it. Open it from the app's Go menu when the room is ready."
                    }
                }
            } @else if site.is_commons() {
                p class="rail-foot" {
                    "Nodes share to "
                    code { (site.public_url()) }
                    ". It holds signed copies; the originals stay on each person's node."
                    @if place == Place::OnThisWifi { " " (ADDRESS_CAN_CHANGE) }
                }
            } @else if !site.share_default.is_empty() {
                // The address can be a long Tailscale name: the button says
                // where it goes instead (HIC_SHARE_DEFAULT_NAME).
                div class="rail-go-wrap" {
                    a class="btn-solid rail-go" href=(site.share_default) {
                        @if site.share_default_name.is_empty() { "Go to the commons" } @else { "Go to the " (site.share_default_name) }
                        " \u{2197}"
                    }
                    p class="rail-go-note" { "An online commons, hosted elsewhere. Your node and your originals stay on this computer." }
                }
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
                            a href="/about" aria-current=[current(page, Section::About)] { "How this works" }
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
                                @if !site.is_commons() { a href="/run-a-commons" { "Run a commons" } }
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

/// The answer a room visitor gets for a page that stays on its owner's
/// computer while the server is open to the room (decision record 0027),
/// with status 403.
pub fn only_here(site: &Site) -> Response {
    let msg = format!(
        "This page of {} stays on its owner's computer. People in the room can see public profiles and stories here.",
        site.instance
    );
    message_page(site, "Only on this computer", "Not from the room", &msg, StatusCode::FORBIDDEN)
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

/// The node's default commons and its name, once known (address, name).
///
/// The house style passes config in rather than reading globals. This is
/// the one exception, and only for display: [`host_label`] is called from
/// dozens of views that don't otherwise need the config, and the name never
/// changes while the server runs. Set once by [`name_default_commons`].
static DEFAULT_COMMONS: OnceLock<(String, String)> = OnceLock::new();

/// Record, once at start-up, that the commons at `url` is called `name`, so
/// [`host_label`] shows the name instead of a long address (a Tailscale one,
/// for example). Does nothing when either is blank, or if already set.
/// Writes only that in-memory note.
pub fn name_default_commons(url: &str, name: &str) {
    let (url, name) = (url.trim().trim_end_matches('/'), name.trim());
    if !url.is_empty() && !name.is_empty() {
        let _ = DEFAULT_COMMONS.set((url.to_string(), name.to_string()));
    }
}

/// A readable label for a server address: the default commons' name if this
/// is the default commons and it has one (see [`name_default_commons`]),
/// else its host, without scheme or trailing slash. A commons on this same
/// computer (`127.0.0.1:8082`) is shown at this computer's network address
/// (`10.0.0.5:8082`), the one other people use: this node keeps talking to
/// it through `127.0.0.1`, which never changes.
pub fn host_label(url: &str) -> String {
    label_for(url, DEFAULT_COMMONS.get().map(|(u, n)| (u.as_str(), n.as_str())), this_computer_ip())
}

/// [`host_label`] with what it looks up passed in: `named` is (address,
/// name), `here` this computer's network address. Pure, so it is tested
/// directly.
fn label_for(url: &str, named: Option<(&str, &str)>, here: Option<std::net::IpAddr>) -> String {
    let url = url.trim_end_matches('/');
    if let Some((named_url, name)) = named {
        if url == named_url {
            return name.to_string();
        }
    }
    let host = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url);
    // "127.0.0.1:8082" or "localhost:8082" means this computer: show the
    // address the rest of the network knows it by.
    let this_computer = ["127.0.0.1:", "localhost:"].iter().find_map(|p| host.strip_prefix(p));
    match (this_computer, here) {
        (Some(port), Some(ip)) if port.chars().all(|c| c.is_ascii_digit()) => format!("{ip}:{port}"),
        _ => host.to_string(),
    }
}

/// This computer's address on its local network (`10.0.0.5`), if it has
/// one, looked up at most every ten seconds: the router can change it.
///
/// It asks the operating system which address it would send from to reach
/// the internet; connecting a UDP socket sends nothing. Like
/// [`DEFAULT_COMMONS`] this is shared state for display only, behind a lock.
fn this_computer_ip() -> Option<std::net::IpAddr> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    /// When the address was last looked up, and what it was.
    static CACHE: Mutex<Option<(Instant, Option<std::net::IpAddr>)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap();
    if let Some((at, ip)) = *cache {
        if at.elapsed() < Duration::from_secs(10) {
            return ip;
        }
    }
    let ip = std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("8.8.8.8:80").map(|_| s))
        .and_then(|s| s.local_addr())
        .ok()
        .map(|a| a.ip())
        .filter(|ip| !ip.is_loopback() && !ip.is_unspecified());
    *cache = Some((Instant::now(), ip));
    ip
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
            share_default_name: String::new(),
            practice: false,
            in_app: false,
            room: None,
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
        assert!(m.contains("Commons on this Wi-Fi"), "a commons at a nearby address");
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
        let in_app = Site { in_app: true, instance: "Held in Common Collective Commons".into(), ..site(Role::Commons) };
        let commons = frame(&in_app, &Page::new("t", None, html! {})).into_string();
        assert!(commons.contains("Held in Common Collective Commons") && commons.contains("Commons on this Wi-Fi"));
        assert!(!commons.contains("Your commons"), "a commons belongs to its community, not to whoever visits");
        assert!(!commons.contains("On this computer"), "visitors reach a commons over the network");
        let node = frame(&Site { in_app: true, ..site(Role::Node) }, &Page::new("t", None, html! {})).into_string();
        assert!(node.contains("On this computer") && node.contains("on this computer, only you"));
    }

    #[test]
    fn each_server_says_where_it_is_and_who_can_reach_it() {
        assert_eq!(Place::of(&site(Role::Node)), Place::Node);
        let local = Site { practice: true, ..site(Role::Commons) };
        assert_eq!(Place::of(&local), Place::Local);
        let room = Site { base_url: "http://192.168.1.5:8081".into(), ..site(Role::Commons) };
        assert_eq!(Place::of(&room), Place::OnThisWifi);
        let tunnel = Site { also_known_as: vec!["https://hic.tail.ts.net".into()], ..room.clone() };
        assert_eq!(Place::of(&tunnel), Place::Online);
        let server = Site { base_url: "https://commons.example".into(), ..site(Role::Commons) };
        assert_eq!(Place::of(&server), Place::Online);
        let page =
            frame(&Site { instance: "Studio laptop".into(), ..site(Role::Node) }, &Page::new("t", None, html! {}))
                .into_string();
        assert!(page.contains("Studio laptop: on this computer, only you"));
        let open =
            Site { instance: "Studio laptop".into(), room: Some("http://192.168.1.5:8080".into()), ..site(Role::Node) };
        let page = frame(&open, &Page::new("t", None, html! {})).into_string();
        assert!(page.contains("Studio laptop: open to the room: public pages only"), "says so in words");
        assert!(page.contains(r#"class="room-pop""#), "with the oxblood pop");
        let closed = frame(&site(Role::Node), &Page::new("t", None, html! {})).into_string();
        assert!(!closed.contains(r#"class="room-pop""#));
    }

    #[test]
    fn local_addresses_are_told_apart_from_online_ones() {
        for local in [
            "http://192.168.1.5:8081",
            "http://10.0.0.2",
            "http://127.0.0.1:8082",
            "http://localhost:8080",
            "http://studio.local:8081",
            "http://[::1]:8080",
        ] {
            assert!(is_local_address(local), "{local}");
        }
        for online in ["https://hic.tail.ts.net", "https://commons.example:8443", "http://8.8.8.8"] {
            assert!(!is_local_address(online), "{online}");
        }
    }

    #[test]
    fn a_node_offers_its_default_commons_by_name() {
        let named = Site {
            share_default: "https://hic.example.ts.net".into(),
            share_default_name: "Held in Common Collective Commons".into(),
            ..site(Role::Node)
        };
        let page = frame(&named, &Page::new("t", None, html! {})).into_string();
        assert!(page.contains("Go to the Held in Common Collective Commons"));
        assert!(!page.contains(">hic.example.ts.net<"), "the address isn't shown as text");
        let local = Site {
            practice: true,
            base_url: "http://192.168.1.20:8082".into(),
            room: Some("http://192.168.1.20:8082".into()),
            ..site(Role::Commons)
        };
        let page = frame(&local, &Page::new("t", None, html! {})).into_string();
        assert!(
            page.contains("<code>http://192.168.1.20:8082</code>"),
            "an open local commons shows its Wi-Fi address"
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
    fn a_local_commons_says_so_and_shows_its_wifi_address() {
        let local = Site {
            practice: true,
            base_url: "http://192.168.1.20:8082".into(),
            room: Some("http://192.168.1.20:8082".into()),
            ..site(Role::Commons)
        };
        let m = frame(&local, &Page::new("t", None, html! {})).into_string();
        assert!(m.contains("Local commons"));
        assert!(m.contains("Local commons: open to the room") || m.contains("open to the room"));
        assert!(m.contains("Anyone on this Wi-Fi can share to <code>http://192.168.1.20:8082</code>"));
        assert!(m.contains("network address changes"), "says it closes when the address changes");
        let closed = Site { room: None, ..local.clone() };
        let m = frame(&closed, &Page::new("t", None, html! {})).into_string();
        assert!(m.contains("Closed to the room: only this computer can reach it."));
        assert!(!m.contains("Anyone on this Wi-Fi"), "a closed local commons doesn't invite the room");
        assert!(!m.contains("Shared commons") && !m.contains("Online commons"));
        let online = Site { also_known_as: vec!["https://hic.tail.ts.net".into()], ..site(Role::Commons) };
        let m = frame(&online, &Page::new("t", None, html! {})).into_string();
        assert!(m.contains("Online commons"), "a commons with a public address is the online kind");
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
        assert_eq!(label_for("http://localhost:8081", None, None), "localhost:8081");
    }

    #[test]
    fn a_commons_on_this_computer_is_shown_at_its_network_address() {
        let here = Some("10.106.64.25".parse().unwrap());
        assert_eq!(label_for("http://127.0.0.1:8082", None, here), "10.106.64.25:8082");
        assert_eq!(label_for("http://localhost:8081/", None, here), "10.106.64.25:8081");
        assert_eq!(label_for("http://127.0.0.1:8082", None, None), "127.0.0.1:8082", "no network: as it is");
        assert_eq!(label_for("http://10.0.0.9:8082", None, here), "10.0.0.9:8082", "another computer stays itself");
        assert_eq!(label_for("http://127.0.0.1.evil.example:80", None, here), "127.0.0.1.evil.example:80");
    }

    #[test]
    fn the_default_commons_is_called_by_its_name() {
        let named = Some(("https://hic.example.ts.net", "Held in Common Collective Commons"));
        assert_eq!(label_for("https://hic.example.ts.net/", named, None), "Held in Common Collective Commons");
        assert_eq!(label_for("http://10.0.0.5:8081", named, None), "10.0.0.5:8081", "other commons keep their address");
        assert_eq!(label_for("https://hic.example.ts.net", None, None), "hic.example.ts.net");
    }
}
