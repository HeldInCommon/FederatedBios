//! Held in Common — the commons server.
//!
//! One self-contained service: creator identity (human handles, keypairs held
//! server-side), content-addressed publishing, the three visibility tiers
//! (public / commons / members-only, the last **encrypted to the member set**),
//! Creative-Commons license + CC-signal governance, the public directory and
//! per-creator profile pages, a JSON API, and ActivityPub-ready endpoints.
//!
//! Storage is the filesystem under $HIC_DATA (default ./data):
//!   data/handles/<handle>.json   creator identity + current pointer
//!   data/versions/<cid>.json     immutable, content-addressed versions
//!
//! Security notes live next to each measure below; see also the /about page.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;

use crypto_box::{
    aead::{Aead, AeadCore, OsRng as BoxRng},
    ChaChaBox, PublicKey as BoxPublic, SecretKey as BoxSecret,
};

const STYLE_CSS: &str = include_str!("style.css");

/// Content-Security-Policy: no scripts at all (every page is server-rendered),
/// styles and fonts only from self + Google Fonts, images from self + data URIs.
const CSP: &str = "default-src 'none'; style-src 'self' https://fonts.googleapis.com; \
     font-src https://fonts.gstatic.com; img-src 'self' data:; form-action 'self'; \
     base-uri 'none'; frame-ancestors 'none'";

const LICENSES: &[(&str, &str, &str)] = &[
    ("cc0", "CC0 1.0", "https://creativecommons.org/publicdomain/zero/1.0/"),
    ("cc-by", "CC BY 4.0", "https://creativecommons.org/licenses/by/4.0/"),
    ("cc-by-sa", "CC BY-SA 4.0", "https://creativecommons.org/licenses/by-sa/4.0/"),
    ("cc-by-nc", "CC BY-NC 4.0", "https://creativecommons.org/licenses/by-nc/4.0/"),
    ("cc-by-nc-sa", "CC BY-NC-SA 4.0", "https://creativecommons.org/licenses/by-nc-sa/4.0/"),
    ("cc-by-nd", "CC BY-ND 4.0", "https://creativecommons.org/licenses/by-nd/4.0/"),
    ("cc-by-nc-nd", "CC BY-NC-ND 4.0", "https://creativecommons.org/licenses/by-nc-nd/4.0/"),
    ("arr", "All rights reserved", ""),
];

/// CC signals — machine-readable preferences aimed at model/dataset reuse.
/// This is the "govern the models, not only the data" layer.
const SIGNALS: &[(&str, &str, &str)] = &[
    ("credit", "Credit", "Attribution expected when this informs or trains a model."),
    ("direct-contribution", "Direct Contribution", "Those who build on it should give back to the creators."),
    ("ecosystem", "Ecosystem", "Reuse should sustain the open commons it came from."),
    ("open", "Open", "Models and outputs derived from it should stay open."),
];

fn license_name(key: &str) -> &'static str {
    LICENSES.iter().find(|l| l.0 == key).map(|l| l.1).unwrap_or("All rights reserved")
}
fn license_url(key: &str) -> &'static str {
    LICENSES.iter().find(|l| l.0 == key).map(|l| l.2).unwrap_or("")
}
fn signal_name(key: &str) -> &'static str {
    SIGNALS.iter().find(|s| s.0 == key).map(|s| s.1).unwrap_or(key_fallback(key))
}
// maud escapes, but keep unknown signal keys from leaking surprising strings.
fn key_fallback(_k: &str) -> &'static str {
    "signal"
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
struct HandleRec {
    handle: String,
    display_name: String,
    created_at: u64,
    ed_pub: String,   // hex
    ed_priv: String,  // hex (server-held — never served)
    box_pub: String,  // hex
    box_priv: String, // hex (server-held — never served)
    secret_salt: String,
    secret_hash: String,
    current_cid: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct Profile {
    name: String,
    pronouns: String,
    role: String,
    bio: String,
    places: Vec<Place>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Place {
    name: String,
    city: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct MemberBox {
    handle: String,
    eph_pub: String, // hex ephemeral x25519 public key
    nonce: String,   // hex
    ct: String,      // base64 ciphertext of the profile JSON
}

#[derive(Serialize, Deserialize, Clone)]
struct Version {
    cid: String,
    handle: String,
    visibility: String, // public | commons | members
    license: String,
    signals: Vec<String>,
    created_at: u64,
    signature: String,            // hex ed25519 over cid bytes
    payload: Option<Profile>,     // public | commons
    enc: Option<Vec<MemberBox>>,  // members-only
}

/// What goes into the content hash. Field order is fixed, so the hash is stable.
#[derive(Serialize)]
struct Canonical<'a> {
    handle: &'a str,
    visibility: &'a str,
    license: &'a str,
    signals: &'a [String],
    created_at: u64,
    body: &'a serde_json::Value,
}

struct AppState {
    data_dir: PathBuf,
    sessions: Mutex<HashMap<String, String>>, // cookie token -> handle
    write: Mutex<()>,                          // serialize all writes
    base_url: String,
    domain: String,
}
type Shared = Arc<AppState>;

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn handle_path(st: &AppState, handle: &str) -> PathBuf {
    st.data_dir.join("handles").join(format!("{handle}.json"))
}
fn version_path(st: &AppState, cid: &str) -> PathBuf {
    st.data_dir.join("versions").join(format!("{cid}.json"))
}

fn load_handle(st: &AppState, handle: &str) -> Option<HandleRec> {
    // Security: handles become filesystem paths. Reject anything that isn't a
    // strict handle so a crafted path segment can never traverse the store.
    if !valid_handle(handle) {
        return None;
    }
    let s = std::fs::read_to_string(handle_path(st, handle)).ok()?;
    serde_json::from_str(&s).ok()
}
fn save_handle(st: &AppState, rec: &HandleRec) -> std::io::Result<()> {
    let p = handle_path(st, &rec.handle);
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, serde_json::to_vec_pretty(rec).unwrap())
}
fn load_version(st: &AppState, cid: &str) -> Option<Version> {
    let s = std::fs::read_to_string(version_path(st, cid)).ok()?;
    serde_json::from_str(&s).ok()
}
fn save_version(st: &AppState, v: &Version) -> std::io::Result<()> {
    let p = version_path(st, &v.cid);
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, serde_json::to_vec_pretty(v).unwrap())
}

fn all_handles(st: &AppState) -> Vec<HandleRec> {
    let dir = st.data_dir.join("handles");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path()) {
                if let Ok(r) = serde_json::from_str::<HandleRec>(&s) {
                    out.push(r);
                }
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

// ---------------------------------------------------------------------------
// Identity, secrets, crypto
// ---------------------------------------------------------------------------

fn rand_hex(n: usize) -> String {
    use rand::RngCore;
    let mut b = vec![0u8; n];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

fn hash_secret(salt: &str, secret: &str) -> String {
    let mut h = Sha256::new();
    h.update(salt.as_bytes());
    h.update(b"|");
    h.update(secret.as_bytes());
    hex::encode(h.finalize())
}

/// Constant-ish-time compare (lengths are equal hex digests here).
fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

fn valid_handle(h: &str) -> bool {
    let n = h.len();
    n >= 2 && n <= 30 && h.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Create a fresh identity. Returns (record, one-time secret shown to the user).
fn mint_identity(handle: &str, display_name: &str) -> (HandleRec, String) {
    let ed = SigningKey::generate(&mut rand::rngs::OsRng);
    let bx = BoxSecret::generate(&mut BoxRng);
    let secret = rand_hex(24); // 192-bit credential
    let salt = rand_hex(16);
    let rec = HandleRec {
        handle: handle.to_string(),
        display_name: display_name.to_string(),
        created_at: now(),
        ed_pub: hex::encode(ed.verifying_key().to_bytes()),
        ed_priv: hex::encode(ed.to_bytes()),
        box_pub: hex::encode(bx.public_key().as_bytes()),
        box_priv: hex::encode(bx.to_bytes()),
        secret_salt: salt.clone(),
        secret_hash: hash_secret(&salt, &secret),
        current_cid: None,
    };
    (rec, secret)
}

fn box_pub_from_hex(h: &str) -> Option<BoxPublic> {
    let b: [u8; 32] = hex::decode(h).ok()?.try_into().ok()?;
    Some(BoxPublic::from(b))
}
fn box_secret_from_hex(h: &str) -> Option<BoxSecret> {
    let b: [u8; 32] = hex::decode(h).ok()?.try_into().ok()?;
    Some(BoxSecret::from(b))
}

/// Encrypt a profile to every member's public key (Option B, sealed-box style:
/// a fresh ephemeral keypair per recipient, so there is no sender identity to
/// leak and each member gets their own ciphertext).
fn encrypt_to_members(st: &AppState, profile: &Profile) -> Vec<MemberBox> {
    let plain = serde_json::to_vec(profile).unwrap();
    let mut out = Vec::new();
    for m in all_handles(st) {
        let Some(mpub) = box_pub_from_hex(&m.box_pub) else { continue };
        let eph = BoxSecret::generate(&mut BoxRng);
        let boxx = ChaChaBox::new(&mpub, &eph);
        let nonce = ChaChaBox::generate_nonce(&mut BoxRng);
        if let Ok(ct) = boxx.encrypt(&nonce, plain.as_ref()) {
            out.push(MemberBox {
                handle: m.handle.clone(),
                eph_pub: hex::encode(eph.public_key().as_bytes()),
                nonce: hex::encode(nonce),
                ct: B64.encode(ct),
            });
        }
    }
    out
}

/// Decrypt a members-only version for a given viewer handle, if they are in the
/// member set. Uses the viewer's server-held private key.
fn decrypt_for(st: &AppState, v: &Version, viewer: &str) -> Option<Profile> {
    let boxes = v.enc.as_ref()?;
    let mb = boxes.iter().find(|b| b.handle == viewer)?;
    let vrec = load_handle(st, viewer)?;
    let vsec = box_secret_from_hex(&vrec.box_priv)?;
    let epub = box_pub_from_hex(&mb.eph_pub)?;
    let nonce_bytes = hex::decode(&mb.nonce).ok()?;
    let ct = B64.decode(&mb.ct).ok()?;
    let boxx = ChaChaBox::new(&epub, &vsec);
    let nonce = crypto_box::Nonce::from_slice(&nonce_bytes);
    let plain = boxx.decrypt(nonce, ct.as_ref()).ok()?;
    serde_json::from_slice(&plain).ok()
}

fn content_id(handle: &str, visibility: &str, license: &str, signals: &[String], created_at: u64, body: &serde_json::Value) -> String {
    let c = Canonical { handle, visibility, license, signals, created_at, body };
    let bytes = serde_json::to_vec(&c).unwrap();
    hex::encode(Sha256::digest(&bytes))
}

// ---------------------------------------------------------------------------
// Sessions / auth
// ---------------------------------------------------------------------------

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("hic_session=") {
            return Some(v.to_string());
        }
    }
    None
}

fn session_handle(st: &AppState, headers: &HeaderMap) -> Option<String> {
    let tok = cookie_token(headers)?;
    st.sessions.lock().unwrap().get(&tok).cloned()
}

fn set_session_cookie(token: &str) -> HeaderValue {
    // HttpOnly + SameSite=Lax + Secure (RunPod serves over https).
    HeaderValue::from_str(&format!(
        "hic_session={token}; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=86400"
    ))
    .unwrap()
}
fn clear_session_cookie() -> HeaderValue {
    HeaderValue::from_static("hic_session=; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=0")
}

// ---------------------------------------------------------------------------
// Form parsing (repeated keys -> Vec, which serde_urlencoded cannot do)
// ---------------------------------------------------------------------------

fn parse_form(body: &str) -> HashMap<String, Vec<String>> {
    let mut m: HashMap<String, Vec<String>> = HashMap::new();
    for (k, v) in form_urlencoded::parse(body.as_bytes()) {
        m.entry(k.into_owned()).or_default().push(v.into_owned());
    }
    m
}
fn one(m: &HashMap<String, Vec<String>>, k: &str) -> String {
    m.get(k).and_then(|v| v.first()).cloned().unwrap_or_default().trim().to_string()
}
fn many(m: &HashMap<String, Vec<String>>, k: &str) -> Vec<String> {
    m.get(k).cloned().unwrap_or_default()
}
fn clamp(s: String, max: usize) -> String {
    s.chars().take(max).collect()
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

fn shell(title: &str, me: Option<&str>, body: Markup) -> Response {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " — Held in Common" }
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link href="https://fonts.googleapis.com/css2?family=Space+Mono:ital,wght@0,400;0,700;1,400&family=Space+Grotesk:wght@400;500;600;700&family=Cormorant:ital,wght@0,500;0,600;1,500&display=swap" rel="stylesheet";
                link rel="stylesheet" href="/style.css";
            }
            body {
                main class="page" {
                    nav class="topbar" {
                        a class="wordmark" href="/" { "Held in Common" span class="dot" { "." } }
                        span class="topbar-links" {
                            a href="/" { "Directory" }
                            @if let Some(h) = me {
                                a href={ "/@" (h) } { "@" (h) }
                                form method="post" action="/logout" class="inline-form" {
                                    button class="linklike" type="submit" { "Sign out" }
                                }
                            } @else {
                                a href="/join" { "Add your profile" }
                                a href="/login" { "Sign in" }
                            }
                        }
                    }
                    (body)
                    footer class="site-foot" {
                        span { "A federated commons for creative data." }
                        a href="/about" { "How this works" }
                    }
                }
            }
        }
    };
    Html(markup.into_string()).into_response()
}

fn visibility_badge(v: &str) -> Markup {
    let (label, class) = match v {
        "public" => ("Public", "vis-public"),
        "commons" => ("Commons", "vis-commons"),
        _ => ("Members only", "vis-members"),
    };
    html! { span class={ "vis-badge " (class) } { (label) } }
}

fn gov_row(license: &str, signals: &[String]) -> Markup {
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

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

async fn directory(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session_handle(&st, &headers);
    let mut cards = Vec::new();
    for h in all_handles(&st) {
        let Some(cid) = h.current_cid.clone() else { continue };
        let Some(v) = load_version(&st, &cid) else { continue };
        if v.visibility == "members" {
            // Listed only as a locked card, and only to signed-in members.
            if me.is_none() {
                continue;
            }
        }
        cards.push((h, v));
    }

    let body = html! {
        header class="intro" {
            h1 { "The commons" }
            p class="lede" { "Creators keep their work on their own node. The commons lets it travel — and lets the community set the terms." }
        }
        @if cards.is_empty() {
            p class="empty-note" { "No profiles published yet. " a href="/join" { "Add the first one." } }
        }
        div class="dir-grid" {
            @for (h, v) in &cards {
                a class="dir-card" href={ "/@" (h.handle) } {
                    span class={ "dir-bar " (match v.visibility.as_str() { "public" => "vis-public", "commons" => "vis-commons", _ => "vis-members" }) } {}
                    span class="dir-body" {
                        @let locked = v.visibility == "members";
                        span class="dir-name" {
                            @if let Some(p) = &v.payload { (p.name) } @else { "@" (h.handle) }
                        }
                        span class="dir-handle" { "@" (h.handle) }
                        @if let Some(p) = &v.payload {
                            @if !p.role.is_empty() { span class="dir-role" { (p.role) } }
                        } @else if locked {
                            span class="dir-role locked" { "Members-only — encrypted" }
                        }
                        (visibility_badge(&v.visibility))
                    }
                }
            }
        }
        div class="actions" {
            a class="primary" href="/join" { "Add your profile" }
        }
    };
    shell("Directory", me.as_deref(), body)
}

async fn profile_page(State(st): State<Shared>, Path(handle): Path<String>, headers: HeaderMap) -> Response {
    let me = session_handle(&st, &headers);
    let Some(h) = load_handle(&st, &handle) else {
        return not_found(me.as_deref());
    };
    let version = h.current_cid.as_ref().and_then(|c| load_version(&st, c));

    let body = html! {
        @match version {
            None => {
                header class="intro" {
                    h1 { "@" (h.handle) }
                    p class="lede" { "This creator has not published a profile yet." }
                }
            }
            Some(v) => {
                @let visible_profile = match v.visibility.as_str() {
                    "members" => me.as_deref().and_then(|viewer| decrypt_for(&st, &v, viewer)),
                    _ => v.payload.clone(),
                };
                header class="intro profile-head" {
                    (visibility_badge(&v.visibility))
                    @match &visible_profile {
                        Some(p) => {
                            h1 { (if p.name.is_empty() { h.handle.clone() } else { p.name.clone() }) }
                            @if !p.pronouns.is_empty() { p class="pronouns" { (p.pronouns) } }
                            @if !p.role.is_empty() { p class="lede" { (p.role) } }
                        }
                        None => {
                            h1 { "@" (h.handle) }
                            p class="lede" { "This profile is members-only and encrypted to the member set. Sign in as a member to read it." }
                        }
                    }
                }
                @if let Some(p) = &visible_profile {
                    @if !p.bio.is_empty() {
                        section class="profile-bio" { p { (p.bio) } }
                    }
                    @if !p.places.is_empty() {
                        section class="profile-places" {
                            h2 class="section-h" { "Where they work" }
                            ul {
                                @for pl in &p.places {
                                    li { (pl.name) @if !pl.city.is_empty() { span class="place-city" { " · " (pl.city) } } }
                                }
                            }
                        }
                    }
                }
                section class="profile-meta" {
                    (gov_row(&v.license, &v.signals))
                    p class="cid" { "Version " span class="mono" { (short(&v.cid)) } " · permanent content address" }
                }
            }
        }
    };
    shell(&format!("@{}", h.handle), me.as_deref(), body)
}

async fn join_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session_handle(&st, &headers);
    let body = html! {
        header class="intro" {
            h1 { "Join the commons" }
            p class="lede" { "Pick a handle. We mint your keys on the node — you never handle a private key yourself." }
        }
        form method="post" action="/join" class="stack-form" {
            div class="field" {
                label class="field-label" for="handle" { "Handle" }
                div class="handle-input" { span class="at" { "@" } input id="handle" name="handle" type="text" maxlength="30" placeholder="tega" required; }
                p class="hint" { "Lowercase letters, numbers and underscores. 2–30 characters." }
            }
            div class="field" {
                label class="field-label" for="display_name" { "Display name" }
                input id="display_name" name="display_name" type="text" maxlength="80" placeholder="Tega Brain" required;
            }
            div class="actions" { button class="primary" type="submit" { "Create handle" } }
        }
    };
    shell("Join", me.as_deref(), body)
}

async fn join_submit(State(st): State<Shared>, body: String) -> Response {
    let f = parse_form(&body);
    let handle = clamp(one(&f, "handle").to_lowercase(), 30);
    let display = clamp(one(&f, "display_name"), 80);
    if !valid_handle(&handle) {
        return error_page("That handle isn't valid — use 2–30 lowercase letters, numbers or underscores.");
    }
    if display.is_empty() {
        return error_page("Please add a display name.");
    }
    let _g = st.write.lock().unwrap();
    if load_handle(&st, &handle).is_some() {
        return error_page("That handle is taken. Try another.");
    }
    let (rec, secret) = mint_identity(&handle, &display);
    if save_handle(&st, &rec).is_err() {
        return error_page("Could not save the new handle. Try again.");
    }
    // Show the one-time secret. This is the credential to publish/sign in.
    let body = html! {
        header class="intro" {
            h1 { "You're in, @" (handle) }
            p class="lede" { "Save this secret somewhere safe. It's how you publish and sign in as @" (handle) ". We only keep a hash — we can't show it again." }
        }
        div class="secret-box" {
            span class="secret-label" { "Your secret" }
            code class="secret" { (secret) }
        }
        form method="post" action="/publish" class="stack-form" {
            input type="hidden" name="handle" value=(handle);
            input type="hidden" name="secret" value=(secret);
            (profile_fields(&Profile::default()))
            (visibility_and_governance("public", &[]))
            div class="actions" { button class="primary" type="submit" { "Publish your profile" } }
        }
    };
    shell("Welcome", Some(&handle), body)
}

fn profile_fields(p: &Profile) -> Markup {
    html! {
        div class="field-row name-row" {
            div class="field" {
                label class="field-label" for="name" { "Name" }
                input id="name" name="name" type="text" maxlength="120" value=(p.name) placeholder="Tega Brain";
            }
            div class="field" {
                label class="field-label" for="pronouns" { "Pronouns" }
                input id="pronouns" name="pronouns" type="text" maxlength="40" value=(p.pronouns) placeholder="she/her";
            }
        }
        div class="field" {
            label class="field-label" for="role" { "Role and organisation" }
            input id="role" name="role" type="text" maxlength="160" value=(p.role) placeholder="Artist & researcher, UNSW";
        }
        div class="field" {
            label class="field-label" for="bio" { "Short bio" }
            textarea id="bio" name="bio" maxlength="600" placeholder="A couple of sentences about your work." { (p.bio) }
        }
        div class="field" {
            label class="field-label" for="place" { "Where you work" }
            input id="place" name="place_name" type="text" maxlength="120" placeholder="Studio or institution";
            input id="place_city" name="place_city" type="text" maxlength="80" placeholder="City" class="city-input";
        }
    }
}

fn visibility_and_governance(sel_vis: &str, sel_sig: &[String]) -> Markup {
    html! {
        fieldset class="reach-options" {
            legend class="field-label" { "Who can see this" }
            @for (val, title, desc) in [
                ("public", "Public", "Listed in the commons directory and open to search engines."),
                ("commons", "Listed in the commons", "In the directory; anyone with the link can read it, kept out of search."),
                ("members", "Members only", "Encrypted to the member set. Only signed-in members can read it."),
            ] {
                label class="reach" {
                    input type="radio" name="visibility" value=(val) checked[val == sel_vis];
                    span class="reach-text" {
                        span class="reach-title" { (title) }
                        span class="reach-body" { (desc) }
                    }
                }
            }
        }
        fieldset class="gov-options" {
            legend class="field-label" { "License" }
            select name="license" class="license-select" {
                @for (key, name, _) in LICENSES {
                    option value=(key) { (name) }
                }
            }
        }
        fieldset class="gov-options" {
            legend class="field-label" { "Signals — how models may learn from this" }
            div class="signal-grid" {
                @for (key, name, desc) in SIGNALS {
                    label class="signal" {
                        input type="checkbox" name="signals" value=(key) checked[sel_sig.iter().any(|s| s == key)];
                        span { span class="signal-name" { (name) } span class="signal-desc" { (desc) } }
                    }
                }
            }
        }
        aside class="caution" {
            p { strong { "Publishing cannot be fully undone." } " Each version gets a permanent content address. If you later narrow visibility or withdraw, the commons stops serving that version, but a copy already fetched stays with the fetcher." }
        }
    }
}

async fn publish(State(st): State<Shared>, body: String) -> Response {
    let f = parse_form(&body);
    let handle = clamp(one(&f, "handle").to_lowercase(), 30);
    let secret = one(&f, "secret");
    let Some(rec) = load_handle(&st, &handle) else {
        return error_page("Unknown handle.");
    };
    if !ct_eq(&hash_secret(&rec.secret_salt, &secret), &rec.secret_hash) {
        return error_page("That secret doesn't match this handle.");
    }

    let mut places = Vec::new();
    let pname = clamp(one(&f, "place_name"), 120);
    if !pname.is_empty() {
        places.push(Place { name: pname, city: clamp(one(&f, "place_city"), 80) });
    }
    let profile = Profile {
        name: clamp(one(&f, "name"), 120),
        pronouns: clamp(one(&f, "pronouns"), 40),
        role: clamp(one(&f, "role"), 160),
        bio: clamp(one(&f, "bio"), 600),
        places,
    };

    let visibility = match one(&f, "visibility").as_str() {
        "commons" => "commons",
        "members" => "members",
        _ => "public",
    }
    .to_string();
    let license = {
        let l = one(&f, "license");
        if LICENSES.iter().any(|x| x.0 == l) { l } else { "cc-by".to_string() }
    };
    let mut signals: Vec<String> = many(&f, "signals").into_iter().filter(|s| SIGNALS.iter().any(|x| x.0 == *s)).collect();
    signals.sort();
    signals.dedup();

    let created_at = now();
    let _g = st.write.lock().unwrap();

    let (payload, enc, body_json) = if visibility == "members" {
        let boxes = encrypt_to_members(&st, &profile);
        let bj = serde_json::to_value(&boxes).unwrap();
        (None, Some(boxes), bj)
    } else {
        let bj = serde_json::to_value(&profile).unwrap();
        (Some(profile.clone()), None, bj)
    };

    let cid = content_id(&handle, &visibility, &license, &signals, created_at, &body_json);
    let ed = SigningKey::from_bytes(&hex_to_32(&rec.ed_priv));
    let signature = hex::encode(ed.sign(cid.as_bytes()).to_bytes());

    let version = Version { cid: cid.clone(), handle: handle.clone(), visibility, license, signals, created_at, signature, payload, enc };
    if save_version(&st, &version).is_err() {
        return error_page("Could not store the version.");
    }
    let mut rec2 = rec;
    rec2.current_cid = Some(cid);
    let _ = save_handle(&st, &rec2);

    redirect(&format!("/@{handle}"))
}

fn hex_to_32(h: &str) -> [u8; 32] {
    let v = hex::decode(h).unwrap_or_default();
    let mut out = [0u8; 32];
    for (i, b) in v.iter().take(32).enumerate() {
        out[i] = *b;
    }
    out
}

async fn login_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session_handle(&st, &headers);
    let body = html! {
        header class="intro" { h1 { "Sign in" } p class="lede" { "Sign in as a member to read members-only profiles." } }
        form method="post" action="/login" class="stack-form" {
            div class="field" {
                label class="field-label" for="handle" { "Handle" }
                div class="handle-input" { span class="at" { "@" } input id="handle" name="handle" type="text" maxlength="30" required; }
            }
            div class="field" {
                label class="field-label" for="secret" { "Secret" }
                input id="secret" name="secret" type="password" maxlength="200" required;
            }
            div class="actions" { button class="primary" type="submit" { "Sign in" } }
        }
    };
    shell("Sign in", me.as_deref(), body)
}

async fn login_submit(State(st): State<Shared>, body: String) -> Response {
    let f = parse_form(&body);
    let handle = clamp(one(&f, "handle").to_lowercase(), 30);
    let secret = one(&f, "secret");
    let Some(rec) = load_handle(&st, &handle) else {
        return error_page("No such handle, or the secret is wrong.");
    };
    if !ct_eq(&hash_secret(&rec.secret_salt, &secret), &rec.secret_hash) {
        return error_page("No such handle, or the secret is wrong.");
    }
    let token = rand_hex(32);
    st.sessions.lock().unwrap().insert(token.clone(), handle.clone());
    let mut h = HeaderMap::new();
    h.insert(header::SET_COOKIE, set_session_cookie(&token));
    h.insert(header::LOCATION, HeaderValue::from_str(&format!("/@{handle}")).unwrap());
    (StatusCode::SEE_OTHER, h).into_response()
}

async fn logout(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Some(tok) = cookie_token(&headers) {
        st.sessions.lock().unwrap().remove(&tok);
    }
    let mut h = HeaderMap::new();
    h.insert(header::SET_COOKIE, clear_session_cookie());
    h.insert(header::LOCATION, HeaderValue::from_static("/"));
    (StatusCode::SEE_OTHER, h).into_response()
}

async fn about(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session_handle(&st, &headers);
    let body = html! {
        header class="intro" { h1 { "How this works" } p class="lede" { "A federated commons for creative data — govern the models, not only the data." } }
        section class="prose" {
            h2 class="section-h" { "Your node, your keys" }
            p { "You claim a human handle. The node mints a signing keypair and an encryption keypair for you and holds them — you never touch a private key." }
            h2 class="section-h" { "Content-addressed versions" }
            p { "Every publish is hashed into a permanent content address and signed by your key. Narrowing visibility stops the commons serving a version, but a copy already fetched stays with the fetcher." }
            h2 class="section-h" { "Three reaches" }
            p { "Public is listed and indexable. Commons is listed but kept out of search. Members-only is encrypted to every member's public key — only signed-in members can read it." }
            h2 class="section-h" { "Govern the models" }
            p { "Each version carries a Creative Commons license and CC signals — machine-readable preferences for how models and datasets may learn from your work." }
            h2 class="section-h" { "Federation-ready" }
            p { "Handles resolve over WebFinger and expose an ActivityPub actor and outbox, so other servers can follow this commons." }
        }
    };
    shell("How this works", me.as_deref(), body)
}

// ---------------------------------------------------------------------------
// JSON API
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DirEntry {
    handle: String,
    display_name: String,
    name: String,
    role: String,
    visibility: String,
    cid: String,
    license: String,
    signals: Vec<String>,
}

async fn api_directory(State(st): State<Shared>) -> Json<Vec<DirEntry>> {
    let mut out = Vec::new();
    for h in all_handles(&st) {
        let Some(cid) = h.current_cid.clone() else { continue };
        let Some(v) = load_version(&st, &cid) else { continue };
        if v.visibility == "members" {
            continue; // never exposed over the public API
        }
        let p = v.payload.unwrap_or_default();
        out.push(DirEntry {
            handle: h.handle,
            display_name: h.display_name,
            name: p.name,
            role: p.role,
            visibility: v.visibility,
            cid,
            license: v.license,
            signals: v.signals,
        });
    }
    Json(out)
}

async fn api_profile(State(st): State<Shared>, Path(handle): Path<String>) -> Response {
    let Some(h) = load_handle(&st, &handle) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not found"}))).into_response();
    };
    let Some(v) = h.current_cid.as_ref().and_then(|c| load_version(&st, c)) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"no profile"}))).into_response();
    };
    if v.visibility == "members" {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error":"members only"}))).into_response();
    }
    Json(serde_json::json!({
        "handle": h.handle,
        "display_name": h.display_name,
        "visibility": v.visibility,
        "cid": v.cid,
        "license": v.license,
        "license_url": license_url(&v.license),
        "signals": v.signals,
        "profile": v.payload,
        "ed25519_public": h.ed_pub,
        "box_public": h.box_pub,
        "signature": v.signature,
    }))
    .into_response()
}

// ---------------------------------------------------------------------------
// ActivityPub-ready endpoints
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct WebfingerQ {
    resource: String,
}

async fn webfinger(State(st): State<Shared>, Query(q): Query<WebfingerQ>) -> Response {
    // acct:handle@domain
    let acct = q.resource.strip_prefix("acct:").unwrap_or(&q.resource);
    let handle = acct.split('@').next().unwrap_or("");
    if load_handle(&st, handle).is_none() {
        return (StatusCode::NOT_FOUND, "no such account").into_response();
    }
    let jrd = serde_json::json!({
        "subject": format!("acct:{handle}@{}", st.domain),
        "links": [{
            "rel": "self",
            "type": "application/activity+json",
            "href": format!("{}/ap/@{handle}", st.base_url),
        }]
    });
    ([(header::CONTENT_TYPE, "application/jrd+json")], Json(jrd)).into_response()
}

async fn ap_actor(State(st): State<Shared>, Path(handle): Path<String>) -> Response {
    let Some(h) = load_handle(&st, &handle) else {
        return (StatusCode::NOT_FOUND, "no such actor").into_response();
    };
    let id = format!("{}/ap/@{handle}", st.base_url);
    let actor = serde_json::json!({
        "@context": ["https://www.w3.org/ns/activitystreams"],
        "id": id,
        "type": "Person",
        "preferredUsername": h.handle,
        "name": h.display_name,
        "url": format!("{}/@{handle}", st.base_url),
        "inbox": format!("{id}/inbox"),
        "outbox": format!("{id}/outbox"),
        // Signing key is published; the encryption/box key is advertised for members-only delivery.
        "publicKeyPem": h.ed_pub,
        "hicBoxPublicKey": h.box_pub,
    });
    ([(header::CONTENT_TYPE, "application/activity+json")], Json(actor)).into_response()
}

async fn ap_outbox(State(st): State<Shared>, Path(handle): Path<String>) -> Response {
    let Some(h) = load_handle(&st, &handle) else {
        return (StatusCode::NOT_FOUND, "no such actor").into_response();
    };
    let mut items = Vec::new();
    if let Some(v) = h.current_cid.as_ref().and_then(|c| load_version(&st, c)) {
        if v.visibility != "members" {
            let obj_id = format!("{}/ap/@{handle}/versions/{}", st.base_url, v.cid);
            let to = if v.visibility == "public" {
                vec!["https://www.w3.org/ns/activitystreams#Public".to_string()]
            } else {
                vec![format!("{}/ap/@{handle}/followers", st.base_url)]
            };
            items.push(serde_json::json!({
                "type": "Create",
                "id": format!("{obj_id}/create"),
                "actor": format!("{}/ap/@{handle}", st.base_url),
                "to": to,
                "object": {
                    "id": obj_id,
                    "type": "Profile",
                    "attributedTo": format!("{}/ap/@{handle}", st.base_url),
                    "name": v.payload.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                    "license": license_url(&v.license),
                    "ccSignals": v.signals,
                }
            }));
        }
    }
    let outbox = serde_json::json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/ap/@{handle}/outbox", st.base_url),
        "type": "OrderedCollection",
        "totalItems": items.len(),
        "orderedItems": items,
    });
    ([(header::CONTENT_TYPE, "application/activity+json")], Json(outbox)).into_response()
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

fn short(cid: &str) -> String {
    cid.chars().take(12).collect()
}

async fn style_css() -> Response {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], STYLE_CSS).into_response()
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok","service":"hic-commons","version":env!("CARGO_PKG_VERSION")}))
}

fn not_found(me: Option<&str>) -> Response {
    let body = html! {
        header class="intro" { h1 { "Not found" } p class="lede" { "No such page or handle." } }
        div class="actions" { a class="primary" href="/" { "Back to the directory" } }
    };
    (StatusCode::NOT_FOUND, shell("Not found", me, body)).into_response()
}

fn error_page(msg: &str) -> Response {
    let body = html! {
        header class="intro" { h1 { "Hmm" } p class="lede" { (msg) } }
        div class="actions" { a class="primary" href="/" { "Back to the directory" } }
    };
    (StatusCode::BAD_REQUEST, shell("Error", None, body)).into_response()
}

fn redirect(to: &str) -> Response {
    let mut h = HeaderMap::new();
    h.insert(header::LOCATION, HeaderValue::from_str(to).unwrap_or(HeaderValue::from_static("/")));
    (StatusCode::SEE_OTHER, h).into_response()
}

// unused but kept for parity with the typed Html return
fn _assert_markup(_: PreEscaped<String>) {}

#[tokio::main]
async fn main() {
    let data_dir = PathBuf::from(std::env::var("HIC_DATA").unwrap_or_else(|_| "./data".into()));
    std::fs::create_dir_all(data_dir.join("handles")).ok();
    std::fs::create_dir_all(data_dir.join("versions")).ok();

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let base_url = std::env::var("HIC_BASE_URL").unwrap_or_else(|_| format!("http://localhost:{port}"));
    let domain = base_url.split("://").nth(1).unwrap_or("localhost").trim_end_matches('/').to_string();

    let state = Arc::new(AppState {
        data_dir,
        sessions: Mutex::new(HashMap::new()),
        write: Mutex::new(()),
        base_url,
        domain,
    });

    let security = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP)))
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")))
        .layer(SetResponseHeaderLayer::overriding(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer")))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
        ));

    let app = Router::new()
        .route("/", get(directory))
        .route("/about", get(about))
        .route("/join", get(join_form).post(join_submit))
        .route("/publish", post(publish))
        .route("/login", get(login_form).post(login_submit))
        .route("/logout", post(logout))
        .route("/@{handle}", get(profile_page))
        .route("/style.css", get(style_css))
        .route("/health", get(health))
        .route("/api/directory", get(api_directory))
        .route("/api/@{handle}", get(api_profile))
        .route("/.well-known/webfinger", get(webfinger))
        .route("/ap/@{handle}", get(ap_actor))
        .route("/ap/@{handle}/outbox", get(ap_outbox))
        .layer(security)
        .layer(RequestBodyLimitLayer::new(64 * 1024))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    println!("hic-commons listening on {addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; })
        .await
        .expect("server");
}
