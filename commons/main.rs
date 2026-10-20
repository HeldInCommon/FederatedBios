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
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
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
const AVATAR_JS: &str = include_str!("avatar.js");

// (role, instance label, commons url) for the top role banner, set once at
// startup. role is "node" (your personal node) or "commons" (a shared/federated
// instance like the RunPod one). This makes "am I on my node or a remote commons"
// obvious in ANY browser, not just the desktop app.
static BANNER: OnceLock<(String, String, String)> = OnceLock::new();

/// Content-Security-Policy: no scripts at all (every page is server-rendered),
/// styles and fonts only from self + Google Fonts, images from self + data URIs.
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' https://fonts.googleapis.com; \
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
    #[serde(default)]
    shared_to: Vec<String>, // commons URLs this handle has federated to
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct Profile {
    name: String,
    pronouns: String,
    role: String,
    bio: String,
    places: Vec<Place>,
    #[serde(default)]
    avatar: String,
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

struct Sess {
    handle: String,
    csrf: String,
}

struct AppState {
    data_dir: PathBuf,
    sessions: Mutex<HashMap<String, Sess>>, // cookie token -> session
    write: Mutex<()>,                        // serialize all writes
    base_url: String,
    domain: String,
    instance: String,       // human label for this instance ("Phoenix's node")
    share_default: String,  // prefilled commons URL in the share form
    secure_cookie: bool,    // set the Secure flag on session cookies (https only)
}
type Shared = Arc<AppState>;

/// A profile federated in from another node and stored here. The ed25519 public
/// key is the portable identity; `cid` + `signature` make it tamper-evident.
#[derive(Serialize, Deserialize, Clone)]
struct RemoteEntry {
    ed_pub: String,
    box_pub: String,
    handle: String,
    display_name: String,
    origin: String,       // node base url (may be a LAN/localhost addr; informational)
    origin_label: String, // human label the node advertised
    cid: String,
    visibility: String,   // public | commons (never members)
    license: String,
    signals: Vec<String>,
    created_at: u64,
    profile: Profile,
    signature: String,
    received_at: u64,
}

/// Stable id for a federated identity: first 16 hex of sha256(ed_pub).
fn keyid(ed_pub: &str) -> String {
    hex::encode(Sha256::digest(ed_pub.as_bytes()))[..16].to_string()
}

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

fn remote_path(st: &AppState, ed_pub: &str) -> PathBuf {
    st.data_dir.join("remote").join(format!("{}.json", keyid(ed_pub)))
}
fn save_remote(st: &AppState, r: &RemoteEntry) -> std::io::Result<()> {
    let p = remote_path(st, &r.ed_pub);
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, serde_json::to_vec_pretty(r).unwrap())
}
fn load_remote(st: &AppState, kid: &str) -> Option<RemoteEntry> {
    // kid is the keyid (already a safe hex slug); guard anyway.
    if kid.len() != 16 || !kid.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let s = std::fs::read_to_string(st.data_dir.join("remote").join(format!("{kid}.json"))).ok()?;
    serde_json::from_str(&s).ok()
}
fn all_remotes(st: &AppState) -> Vec<RemoteEntry> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(st.data_dir.join("remote")) {
        for e in rd.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path()) {
                if let Ok(r) = serde_json::from_str::<RemoteEntry>(&s) {
                    out.push(r);
                }
            }
        }
    }
    out.sort_by(|a, b| b.received_at.cmp(&a.received_at));
    out
}

// ---------------------------------------------------------------------------
// Federation: sign a share, verify an inbound share
// ---------------------------------------------------------------------------

/// Build the signed ActivityPub-shaped `Create` a node pushes to a commons.
fn build_share_activity(st: &AppState, rec: &HandleRec, v: &Version) -> serde_json::Value {
    let profile = v.payload.clone().unwrap_or_default();
    serde_json::json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "actor": {
            "type": "Person",
            "preferredUsername": rec.handle,
            "name": rec.display_name,
            "id": format!("{}/ap/@{}", st.base_url, rec.handle),
            "origin": st.base_url,
            "originLabel": st.instance,
            "ed25519PublicKey": rec.ed_pub,
            "boxPublicKey": rec.box_pub,
        },
        "object": {
            "type": "Profile",
            "cid": v.cid,
            "visibility": v.visibility,
            "license": v.license,
            "signals": v.signals,
            "createdAt": v.created_at,
            "profile": profile,
        },
        "signature": { "type": "ed25519", "value": v.signature },
    })
}

/// Verify an inbound share and turn it into a RemoteEntry. Checks: the content
/// hash the node claims matches the content it sent (no tampering), and the
/// ed25519 signature over that hash is valid for the advertised key.
fn verify_inbound(activity: &serde_json::Value) -> Result<RemoteEntry, String> {
    let actor = activity.get("actor").ok_or("missing actor")?;
    let object = activity.get("object").ok_or("missing object")?;
    let ed_pub = actor.get("ed25519PublicKey").and_then(|v| v.as_str()).ok_or("missing key")?;
    let handle = actor.get("preferredUsername").and_then(|v| v.as_str()).ok_or("missing handle")?;
    if !valid_handle(handle) {
        return Err("invalid handle".into());
    }
    let visibility = object.get("visibility").and_then(|v| v.as_str()).unwrap_or("public").to_string();
    if visibility == "members" {
        return Err("members-only profiles are not federated".into());
    }
    let license = object.get("license").and_then(|v| v.as_str()).unwrap_or("arr").to_string();
    let signals: Vec<String> = object
        .get("signals")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let created_at = object.get("createdAt").and_then(|v| v.as_u64()).unwrap_or(0);
    let profile: Profile = serde_json::from_value(object.get("profile").cloned().unwrap_or_default())
        .map_err(|_| "bad profile")?;
    let claimed_cid = object.get("cid").and_then(|v| v.as_str()).ok_or("missing cid")?;
    let sig_hex = activity
        .get("signature")
        .and_then(|s| s.get("value"))
        .and_then(|v| v.as_str())
        .ok_or("missing signature")?;

    // Recompute the content address from what was actually sent (tamper check).
    let body_json = hash_body(&profile);
    let cid = content_id(handle, &visibility, &license, &signals, created_at, &body_json);
    if cid != claimed_cid {
        return Err("content hash mismatch".into());
    }

    // Verify the signature over the cid with the advertised key.
    let vk_bytes: [u8; 32] = hex::decode(ed_pub).ok().and_then(|b| b.try_into().ok()).ok_or("bad key")?;
    let vk = VerifyingKey::from_bytes(&vk_bytes).map_err(|_| "bad key")?;
    let sig_bytes: [u8; 64] = hex::decode(sig_hex).ok().and_then(|b| b.try_into().ok()).ok_or("bad signature")?;
    let sig = Signature::from_bytes(&sig_bytes);
    vk.verify(cid.as_bytes(), &sig).map_err(|_| "signature does not verify")?;

    Ok(RemoteEntry {
        ed_pub: ed_pub.to_string(),
        box_pub: actor.get("boxPublicKey").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        handle: handle.to_string(),
        display_name: actor.get("name").and_then(|v| v.as_str()).unwrap_or(handle).to_string(),
        origin: actor.get("origin").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        origin_label: actor.get("originLabel").and_then(|v| v.as_str()).unwrap_or("a node").to_string(),
        cid: cid.clone(),
        visibility,
        license,
        signals,
        created_at,
        profile,
        signature: sig_hex.to_string(),
        received_at: now(),
    })
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

/// Create a fresh identity whose credential is the passphrase the member chose.
fn mint_identity(handle: &str, display_name: &str, passphrase: &str) -> HandleRec {
    let ed = SigningKey::generate(&mut rand::rngs::OsRng);
    let bx = BoxSecret::generate(&mut BoxRng);
    let salt = rand_hex(16);
    HandleRec {
        handle: handle.to_string(),
        display_name: display_name.to_string(),
        created_at: now(),
        ed_pub: hex::encode(ed.verifying_key().to_bytes()),
        ed_priv: hex::encode(ed.to_bytes()),
        box_pub: hex::encode(bx.public_key().as_bytes()),
        box_priv: hex::encode(bx.to_bytes()),
        secret_salt: salt.clone(),
        secret_hash: hash_secret(&salt, passphrase),
        current_cid: None,
        shared_to: Vec::new(),
    }
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

/// The profile JSON that feeds the content hash. The avatar is display metadata,
/// not signed content: excluding it keeps the content address stable when a photo
/// is added or changed, and — crucially — lets a node federate to a commons that
/// predates the avatar field (it would otherwise recompute a different hash and
/// reject the signature). Members-only bodies hash their ciphertext, not this.
fn hash_body(profile: &Profile) -> serde_json::Value {
    let mut v = serde_json::to_value(profile).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.remove("avatar");
    }
    v
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
    st.sessions.lock().unwrap().get(&tok).map(|s| s.handle.clone())
}

fn session_csrf(st: &AppState, headers: &HeaderMap) -> Option<String> {
    let tok = cookie_token(headers)?;
    st.sessions.lock().unwrap().get(&tok).map(|s| s.csrf.clone())
}

/// Start a session for `handle` and return the cookie token. The per-session
/// CSRF token guards the cookie-authorised publish form.
fn new_session(st: &AppState, handle: &str) -> String {
    let token = rand_hex(32);
    let csrf = rand_hex(16);
    st.sessions.lock().unwrap().insert(token.clone(), Sess { handle: handle.to_string(), csrf });
    token
}

fn set_session_cookie(token: &str, secure: bool) -> HeaderValue {
    // HttpOnly + SameSite=Lax. `Secure` only when served over https (e.g. the
    // RunPod commons). On the local desktop node (http://localhost) Secure must
    // be omitted — WKWebView drops Secure cookies on http, which locks the user
    // in a sign-in loop.
    let sec = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "hic_session={token}; Path=/; HttpOnly; SameSite=Lax{sec}; Max-Age=86400"
    ))
    .unwrap()
}
fn clear_session_cookie(secure: bool) -> HeaderValue {
    let sec = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "hic_session=; Path=/; HttpOnly; SameSite=Lax{sec}; Max-Age=0"
    ))
    .unwrap()
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

fn role_banner() -> Markup {
    let (role, instance, commons) = BANNER
        .get()
        .cloned()
        .unwrap_or_else(|| ("node".into(), "Held in Common".into(), String::new()));
    let is_commons = role == "commons";
    html! {
        div class={ "role-bar " (if is_commons { "role-commons" } else { "role-node" }) } {
            span class="role-dot" {}
            span class="role-label" { (if is_commons { "Shared commons" } else { "Your node" }) }
            @if !instance.is_empty() { span class="role-instance" { "· " (instance) } }
            @if !is_commons && !commons.is_empty() {
                a class="role-link" href=(commons) { "Visit the commons \u{2197}" }
            }
        }
    }
}

/// True when this instance runs as a shared commons (HIC_ROLE=commons) rather
/// than someone's personal node. A commons is a federation target, not a place
/// you mint an identity: there's no direct sign-up — profiles arrive only as
/// signed copies federated from people's own nodes, so /join and /login steer
/// people to run a node instead.
fn is_commons() -> bool {
    BANNER.get().map(|(r, _, _)| r == "commons").unwrap_or(false)
}

/// Shown in place of /join and /login on a shared commons. Your keys live on
/// your node and never leave your machine, so the only way onto a commons is to
/// run a node and share a signed copy up. This explains that node-first flow.
fn commons_gate_page(me: Option<&str>) -> Response {
    let (_, instance, _) = BANNER.get().cloned().unwrap_or_default();
    let body = html! {
        header class="intro" {
            h1 { "This is a shared commons" }
            p class="lede" {
                "Profiles here are signed copies that people publish from their own nodes. "
                "There's no sign-up on a commons — your identity and keys live on your node "
                "and never leave your machine."
            }
        }
        div class="gate" {
            h2 { "To appear in this directory" }
            ol class="gate-steps" {
                li { "Run your own node — download the app (no account, no Docker)." }
                li { "Create your handle and profile on your node." }
                li {
                    "Click " strong { "“Share to a commons”" } " and paste this commons's address"
                    @if !instance.is_empty() { " (" (instance) ")" } "."
                }
            }
            div class="actions" {
                a class="primary" href="https://github.com/phoenixperry/FederatedBios/releases/tag/node-app-preview" { "Get the app \u{2192}" }
            }
            p class="hint" {
                "A signed copy of your profile then appears here. Members-only profiles stay "
                "encrypted on your node and are never shared — not even to a commons."
            }
        }
    };
    shell("Shared commons", me, body)
}

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
                (role_banner())
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
                            } @else if is_commons() {
                                a href="/join" { "Run a node to join" }
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
    let remotes = all_remotes(&st);

    let body = html! {
        header class="intro" {
            h1 { "The commons" }
            p class="lede" { "Creators keep their work on their own node. The commons lets it travel — and lets the community set the terms." }
        }
        @if cards.is_empty() && remotes.is_empty() {
            @if is_commons() {
                p class="empty-note" { "Nothing here yet. " a href="/join" { "Run a node" } " and share your profile in — that's how profiles arrive here." }
            } @else {
                p class="empty-note" { "Nothing here yet. " a href="/join" { "Add the first profile" } " — or run your own node and share it in." }
            }
        }
        div class="dir-grid" {
            @for r in &remotes {
                a class="dir-card" href={ "/federated/" (keyid(&r.ed_pub)) } {
                    span class="dir-bar vis-federated" {}
                    span class="dir-body" {
                        @if !r.profile.avatar.is_empty() { img class="dir-avatar" src=(r.profile.avatar) alt=""; }
                        span class="dir-name" { (if r.profile.name.is_empty() { r.handle.clone() } else { r.profile.name.clone() }) }
                        span class="dir-handle" { "@" (r.handle) }
                        @if !r.profile.role.is_empty() { span class="dir-role" { (r.profile.role) } }
                        span class="dir-origin" { "federated from " (r.origin_label) }
                    }
                }
            }
            @for (h, v) in &cards {
                a class="dir-card" href={ "/@" (h.handle) } {
                    span class={ "dir-bar " (match v.visibility.as_str() { "public" => "vis-public", "commons" => "vis-commons", _ => "vis-members" }) } {}
                    span class="dir-body" {
                        @let locked = v.visibility == "members";
                        @if let Some(p) = &v.payload { @if !p.avatar.is_empty() { img class="dir-avatar" src=(p.avatar) alt=""; } }
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
            @if is_commons() {
                a class="primary" href="/join" { "Run a node to join" }
            } @else {
                a class="primary" href="/join" { "Add your profile" }
            }
        }
    };
    shell("Directory", me.as_deref(), body)
}

/// A readable label for a commons URL — its host, without scheme or trailing slash.
fn commons_label(url: &str) -> String {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url)
        .trim_end_matches('/')
        .to_string()
}

/// De-duplicated, order-stable list of commons the owner can share to with one
/// click: the configured default first (if set), then everything already shared
/// to. This is what turns "paste a URL" into "pick one and click".
fn known_commons(default: &str, shared_to: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for u in std::iter::once(default).chain(shared_to.iter().map(|s| s.as_str())) {
        let u = u.trim().trim_end_matches('/');
        if !u.is_empty() && !out.iter().any(|x| x == u) {
            out.push(u.to_string());
        }
    }
    out
}

async fn profile_page(State(st): State<Shared>, Path(handle): Path<String>, headers: HeaderMap) -> Response {
    let me = session_handle(&st, &headers);
    let Some(h) = load_handle(&st, &handle) else {
        return not_found(me.as_deref());
    };
    let version = h.current_cid.as_ref().and_then(|c| load_version(&st, c));
    let is_owner = me.as_deref() == Some(h.handle.as_str());
    let shareable = version.as_ref().map_or(false, |v| v.visibility != "members");
    let csrf = if is_owner { session_csrf(&st, &headers).unwrap_or_default() } else { String::new() };
    let share_default = st.share_default.clone();

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
                            @if !p.avatar.is_empty() { img class="profile-avatar" src=(p.avatar) alt=""; }
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
        @if is_owner {
            div class="owner-panel" {
                div class="actions owner-actions" { a class="primary" href="/new" { "Update your profile" } }
                @if shareable {
                    @let known = known_commons(&share_default, &h.shared_to);
                    section class="share-panel" {
                        h2 class="field-label" { "Share to a commons" }
                        p class="hint" { "Push a signed, verifiable copy to a shared commons \u{2014} one click. Members-only versions never leave your node." }
                        @if !known.is_empty() {
                            ul class="commons-list" {
                                @for c in &known {
                                    @let shared = h.shared_to.iter().any(|s| s == c);
                                    li class="commons-item" {
                                        form method="post" action="/share" class="commons-share" {
                                            input type="hidden" name="handle" value=(h.handle);
                                            input type="hidden" name="csrf" value=(csrf);
                                            input type="hidden" name="commons_url" value=(c);
                                            span class="commons-meta" {
                                                span class="commons-name" { (commons_label(c)) }
                                                @if shared { span class="commons-status" { "shared \u{2713}" } }
                                            }
                                            button class="secondary" type="submit" { (if shared { "Re-share" } else { "Share" }) }
                                        }
                                    }
                                }
                            }
                        }
                        details class="add-commons" open[known.is_empty()] {
                            summary { (if known.is_empty() { "Add a commons" } else { "Share to a different commons" }) }
                            form method="post" action="/share" class="share-form" {
                                input type="hidden" name="handle" value=(h.handle);
                                input type="hidden" name="csrf" value=(csrf);
                                div class="share-row" {
                                    input name="commons_url" type="url" placeholder="https://commons.example";
                                    button class="secondary" type="submit" { "Share" }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    shell(&format!("@{}", h.handle), me.as_deref(), body)
}

async fn join_form(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let me = session_handle(&st, &headers);
    if is_commons() {
        return commons_gate_page(me.as_deref());
    }
    let body = html! {
        header class="intro" {
            h1 { "Join the commons" }
            p class="lede" { "Pick a handle. We mint your keys on the node — you never handle a private key yourself." }
        }
        form method="post" action="/join" class="stack-form" {
            div class="field" {
                label class="field-label" for="handle" { "Handle" }
                div class="handle-input" { span class="at" { "@" } input id="handle" name="handle" type="text" maxlength="30" placeholder="phoenixperry" required; }
                p class="hint" { "Lowercase letters, numbers and underscores. 2–30 characters." }
            }
            div class="field" {
                label class="field-label" for="display_name" { "Display name" }
                input id="display_name" name="display_name" type="text" maxlength="80" placeholder="Phoenix Perry" required;
            }
            div class="field" {
                label class="field-label" for="passphrase" { "Passphrase" }
                input id="passphrase" name="passphrase" type="password" minlength="8" maxlength="200" placeholder="At least 8 characters" required;
                p class="hint" { "You'll use this to sign in and to publish. Choose something you'll remember — we store only a salted hash and can't recover it." }
            }
            div class="actions" { button class="primary" type="submit" { "Create handle" } }
        }
    };
    shell("Join", me.as_deref(), body)
}

async fn join_submit(State(st): State<Shared>, body: String) -> Response {
    if is_commons() {
        return commons_gate_page(None);
    }
    let f = parse_form(&body);
    let handle = clamp(one(&f, "handle").to_lowercase(), 30);
    let display = clamp(one(&f, "display_name"), 80);
    let passphrase = one(&f, "passphrase");
    if !valid_handle(&handle) {
        return error_page("That handle isn't valid — use 2–30 lowercase letters, numbers or underscores.");
    }
    if display.is_empty() {
        return error_page("Please add a display name.");
    }
    if passphrase.chars().count() < 8 {
        return error_page("Please choose a passphrase of at least 8 characters.");
    }
    let _g = st.write.lock().unwrap();
    if load_handle(&st, &handle).is_some() {
        return error_page("That handle is taken. Try another.");
    }
    let rec = mint_identity(&handle, &display, &passphrase);
    if save_handle(&st, &rec).is_err() {
        return error_page("Could not save the new handle. Try again.");
    }
    // Sign them straight in and send them to the publish form.
    let token = new_session(&st, &handle);
    let mut h = HeaderMap::new();
    h.insert(header::SET_COOKIE, set_session_cookie(&token, st.secure_cookie));
    h.insert(header::LOCATION, HeaderValue::from_static("/new"));
    (StatusCode::SEE_OTHER, h).into_response()
}

/// The publish / edit form for a signed-in member, prefilled from their current
/// version where it can be read.
async fn new_profile(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let Some(me) = session_handle(&st, &headers) else {
        return redirect("/login");
    };
    let csrf = session_csrf(&st, &headers).unwrap_or_default();
    let rec = load_handle(&st, &me);
    let current = rec.as_ref().and_then(|r| r.current_cid.as_ref()).and_then(|c| load_version(&st, c));
    let (profile, sel_vis, sel_license, sel_sig) = match &current {
        Some(v) => {
            let p = match v.visibility.as_str() {
                "members" => decrypt_for(&st, v, &me).unwrap_or_default(),
                _ => v.payload.clone().unwrap_or_default(),
            };
            (p, v.visibility.clone(), v.license.clone(), v.signals.clone())
        }
        None => (Profile::default(), "public".to_string(), "cc-by".to_string(), Vec::new()),
    };
    let first_time = current.is_none();
    let body = html! {
        header class="intro" {
            h1 { @if first_time { "Your profile" } @else { "Update your profile" } }
            p class="lede" {
                @if first_time { "You're signed in as @" (me) ". Fill this in and publish it to the commons." }
                @else { "Signed in as @" (me) ". Change anything and republish — each version gets its own content address." }
            }
        }
        form method="post" action="/publish" class="stack-form" {
            input type="hidden" name="handle" value=(me);
            input type="hidden" name="csrf" value=(csrf);
            (profile_fields(&profile))
            (visibility_and_governance(&sel_vis, &sel_license, &sel_sig))
            div class="actions" { button class="primary" type="submit" { "Publish to the commons" } }
        }
        script src="/avatar.js" {}
    };
    shell("Publish", Some(&me), body)
}

fn profile_fields(p: &Profile) -> Markup {
    html! {
        div class="field" {
            label class="field-label" { "Profile picture" }
            div class="photo-field" {
                @if p.avatar.is_empty() {
                    img id="avatar-preview" class="avatar-preview empty" alt="";
                } @else {
                    img id="avatar-preview" class="avatar-preview" src=(p.avatar) alt="Current profile picture";
                }
                div class="photo-pick" {
                    input id="photo" type="file" accept="image/png,image/jpeg,image/webp";
                    p class="hint" { "A square photo works best — it's resized to about 320px on your device and saved with your profile." }
                    button id="avatar-clear" type="button" class="link-btn" style=(if p.avatar.is_empty() { "display:none" } else { "" }) { "Remove photo" }
                }
            }
            input id="avatar" type="hidden" name="avatar" value=(p.avatar);
        }
        div class="field-row name-row" {
            div class="field" {
                label class="field-label" for="name" { "Name" }
                input id="name" name="name" type="text" maxlength="120" value=(p.name) placeholder="Phoenix Perry";
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
        @let place = p.places.first();
        div class="field" {
            label class="field-label" for="place" { "Where you work" }
            input id="place" name="place_name" type="text" maxlength="120" value=(place.map(|x| x.name.as_str()).unwrap_or("")) placeholder="Studio or institution";
            input id="place_city" name="place_city" type="text" maxlength="80" value=(place.map(|x| x.city.as_str()).unwrap_or("")) placeholder="City" class="city-input";
        }
    }
}

fn visibility_and_governance(sel_vis: &str, sel_license: &str, sel_sig: &[String]) -> Markup {
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
                    option value=(key) selected[*key == sel_license] { (name) }
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

async fn publish(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = parse_form(&body);
    let handle = clamp(one(&f, "handle").to_lowercase(), 30);
    // Authorise via the signed-in session + a per-session CSRF token.
    let authed = session_handle(&st, &headers).as_deref() == Some(handle.as_str())
        && session_csrf(&st, &headers).map_or(false, |c| !c.is_empty() && ct_eq(&c, &one(&f, "csrf")));
    if !authed {
        return error_page("Please sign in as this handle to publish.");
    }
    let Some(rec) = load_handle(&st, &handle) else {
        return error_page("Unknown handle.");
    };

    let mut places = Vec::new();
    let pname = clamp(one(&f, "place_name"), 120);
    if !pname.is_empty() {
        places.push(Place { name: pname, city: clamp(one(&f, "place_city"), 80) });
    }
    // Avatar arrives as a small JPEG/PNG/WebP data URI, already resized on the
    // client. Validate the prefix and cap the size so it stays a profile photo.
    let avatar = {
        let a = one(&f, "avatar");
        let ok = a.starts_with("data:image/jpeg;base64,")
            || a.starts_with("data:image/png;base64,")
            || a.starts_with("data:image/webp;base64,");
        if ok && a.len() <= 600_000 { a } else { String::new() }
    };
    let profile = Profile {
        name: clamp(one(&f, "name"), 120),
        pronouns: clamp(one(&f, "pronouns"), 40),
        role: clamp(one(&f, "role"), 160),
        bio: clamp(one(&f, "bio"), 600),
        places,
        avatar,
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
        let bj = hash_body(&profile);
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
    if is_commons() {
        return commons_gate_page(me.as_deref());
    }
    let body = html! {
        header class="intro" { h1 { "Sign in" } p class="lede" { "Sign in as a member to read members-only profiles." } }
        form method="post" action="/login" class="stack-form" {
            div class="field" {
                label class="field-label" for="handle" { "Handle" }
                div class="handle-input" { span class="at" { "@" } input id="handle" name="handle" type="text" maxlength="30" placeholder="phoenixperry" required; }
            }
            div class="field" {
                label class="field-label" for="passphrase" { "Passphrase" }
                input id="passphrase" name="passphrase" type="password" maxlength="200" required;
            }
            div class="actions" { button class="primary" type="submit" { "Sign in" } }
        }
    };
    shell("Sign in", me.as_deref(), body)
}

async fn login_submit(State(st): State<Shared>, body: String) -> Response {
    if is_commons() {
        return commons_gate_page(None);
    }
    let f = parse_form(&body);
    let handle = clamp(one(&f, "handle").to_lowercase(), 30);
    let passphrase = one(&f, "passphrase");
    let Some(rec) = load_handle(&st, &handle) else {
        return error_page("No such handle, or the passphrase is wrong.");
    };
    if !ct_eq(&hash_secret(&rec.secret_salt, &passphrase), &rec.secret_hash) {
        return error_page("No such handle, or the passphrase is wrong.");
    }
    let token = new_session(&st, &handle);
    let mut h = HeaderMap::new();
    h.insert(header::SET_COOKIE, set_session_cookie(&token, st.secure_cookie));
    h.insert(header::LOCATION, HeaderValue::from_str(&format!("/@{handle}")).unwrap());
    (StatusCode::SEE_OTHER, h).into_response()
}

async fn logout(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Some(tok) = cookie_token(&headers) {
        st.sessions.lock().unwrap().remove(&tok);
    }
    let mut h = HeaderMap::new();
    h.insert(header::SET_COOKIE, clear_session_cookie(st.secure_cookie));
    h.insert(header::LOCATION, HeaderValue::from_static("/"));
    (StatusCode::SEE_OTHER, h).into_response()
}

/// A node pushes its current public/commons version to a chosen commons.
async fn share(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let f = parse_form(&body);
    let handle = clamp(one(&f, "handle").to_lowercase(), 30);
    let authed = session_handle(&st, &headers).as_deref() == Some(handle.as_str())
        && session_csrf(&st, &headers).map_or(false, |c| !c.is_empty() && ct_eq(&c, &one(&f, "csrf")));
    if !authed {
        return error_page("Please sign in as this handle to share.");
    }
    let commons = one(&f, "commons_url");
    let commons = commons.trim().trim_end_matches('/').to_string();
    if !(commons.starts_with("http://") || commons.starts_with("https://")) {
        return error_page("Enter the full address of a commons, e.g. https://example.proxy.runpod.net");
    }
    let Some(rec) = load_handle(&st, &handle) else {
        return error_page("Unknown handle.");
    };
    let Some(v) = rec.current_cid.as_ref().and_then(|c| load_version(&st, c)) else {
        return error_page("Publish a profile before sharing it.");
    };
    if v.visibility == "members" {
        return error_page("Members-only profiles stay on your node — they are never shared.");
    }
    let activity = build_share_activity(&st, &rec, &v);
    let url = format!("{commons}/inbox");
    let send = reqwest::Client::new()
        .post(&url)
        .json(&activity)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;
    match send {
        Ok(r) if r.status().is_success() => {
            {
                let _g = st.write.lock().unwrap();
                if let Some(mut rec2) = load_handle(&st, &handle) {
                    if !rec2.shared_to.contains(&commons) {
                        rec2.shared_to.push(commons.clone());
                        let _ = save_handle(&st, &rec2);
                    }
                }
            }
            let body = html! {
                header class="intro" {
                    h1 { "Shared" }
                    p class="lede" { "Your profile is now federated to " a href=(commons) { (commons) } ". They have a signed, verifiable copy." }
                }
                div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
            };
            shell("Shared", Some(&handle), body)
        }
        Ok(r) => error_page(&format!("That commons rejected the share (HTTP {}). It may not accept federation.", r.status().as_u16())),
        Err(e) => error_page(&format!("Could not reach that commons: {e}")),
    }
}

/// Receive a federated share from another node.
async fn inbox(State(st): State<Shared>, body: String) -> Response {
    let Ok(activity) = serde_json::from_str::<serde_json::Value>(&body) else {
        return (StatusCode::BAD_REQUEST, "invalid json").into_response();
    };
    match verify_inbound(&activity) {
        Ok(entry) => {
            let _g = st.write.lock().unwrap();
            // TOFU: pin a (handle, origin) pair to the first key we saw for it.
            for ex in all_remotes(&st) {
                if ex.handle == entry.handle && ex.origin_label == entry.origin_label && ex.ed_pub != entry.ed_pub {
                    return (StatusCode::CONFLICT, "handle already federated from that origin under a different key").into_response();
                }
            }
            if save_remote(&st, &entry).is_err() {
                return (StatusCode::INTERNAL_SERVER_ERROR, "could not store").into_response();
            }
            (StatusCode::ACCEPTED, "accepted").into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, format!("rejected: {e}")).into_response(),
    }
}

/// View a profile that was federated in from another node.
async fn federated_view(State(st): State<Shared>, Path(kid): Path<String>, headers: HeaderMap) -> Response {
    let me = session_handle(&st, &headers);
    let Some(r) = load_remote(&st, &kid) else {
        return not_found(me.as_deref());
    };
    let p = r.profile.clone();
    let body = html! {
        header class="intro profile-head" {
            span class="vis-badge vis-federated" { "Federated" }
            @if !p.avatar.is_empty() { img class="profile-avatar" src=(p.avatar) alt=""; }
            h1 { (if p.name.is_empty() { r.handle.clone() } else { p.name.clone() }) }
            @if !p.pronouns.is_empty() { p class="pronouns" { (p.pronouns) } }
            @if !p.role.is_empty() { p class="lede" { (p.role) } }
        }
        @if !p.bio.is_empty() { section class="profile-bio" { p { (p.bio) } } }
        @if !p.places.is_empty() {
            section class="profile-places" {
                h2 class="section-h" { "Where they work" }
                ul { @for pl in &p.places { li { (pl.name) @if !pl.city.is_empty() { span class="place-city" { " · " (pl.city) } } } } }
            }
        }
        section class="profile-meta" {
            (gov_row(&r.license, &r.signals))
            p class="federated-origin" {
                "Lives on " strong { (r.origin_label) }
                @if r.origin.starts_with("http") { " · " a href=(r.origin) { (r.origin) } }
            }
            p class="cid" { "Version " span class="mono" { (short(&r.cid)) } " · signed by key " span class="mono" { (short(&r.ed_pub)) } }
        }
    };
    shell(&format!("@{} (federated)", r.handle), me.as_deref(), body)
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
async fn avatar_js() -> Response {
    ([(header::CONTENT_TYPE, "application/javascript; charset=utf-8")], AVATAR_JS).into_response()
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
    std::fs::create_dir_all(data_dir.join("remote")).ok();

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let base_url = std::env::var("HIC_BASE_URL").unwrap_or_else(|_| format!("http://localhost:{port}"));
    let domain = base_url.split("://").nth(1).unwrap_or("localhost").trim_end_matches('/').to_string();
    let secure_cookie = base_url.starts_with("https://");
    let instance = std::env::var("HIC_INSTANCE").unwrap_or_else(|_| "Held in Common".into());
    let share_default = std::env::var("HIC_SHARE_DEFAULT").unwrap_or_default();
    // "node" vs "commons": explicit via HIC_ROLE, else inferred from the instance
    // label (RunPod sets HIC_INSTANCE="Held in Common (federated)").
    let role = std::env::var("HIC_ROLE").unwrap_or_else(|_| {
        let i = instance.to_lowercase();
        if i.contains("commons") || i.contains("federated") { "commons".into() } else { "node".into() }
    });
    let _ = BANNER.set((role, instance.clone(), share_default.clone()));

    let state = Arc::new(AppState {
        data_dir,
        sessions: Mutex::new(HashMap::new()),
        write: Mutex::new(()),
        base_url,
        domain,
        instance,
        share_default,
        secure_cookie,
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
        .route("/new", get(new_profile))
        .route("/publish", post(publish))
        .route("/share", post(share))
        .route("/inbox", post(inbox))
        .route("/federated/{kid}", get(federated_view))
        .route("/login", get(login_form).post(login_submit))
        .route("/logout", post(logout))
        .route("/@{handle}", get(profile_page))
        .route("/style.css", get(style_css))
        .route("/avatar.js", get(avatar_js))
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
