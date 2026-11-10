//! The body of each HTML page, as pure functions from data to markup.
//!
//! Handlers (in `handlers.rs`) load the data and pass it in; nothing here reads
//! files, the clock or the session.

use maud::{html, Markup};

use crate::canonical::short;
use crate::governance::{LICENSES, SIGNALS};
use crate::keys::keyid;
use crate::model::{display_name_or_handle, HandleRec, Profile, Reach, RemoteEntry, Version};
use crate::web::layout::{gov_row, host_label, reach_badge, reach_bar_class, Site};

/// Where the "Get the app" button points.
const APP_DOWNLOAD: &str = "https://github.com/HeldInCommon/FederatedBios/releases/tag/node-app-preview";

/// The host a shared profile says it came from, or its node's name if it
/// sent no usable address. Both are the sender's own claims.
pub fn origin_host(r: &RemoteEntry) -> String {
    if r.origin.starts_with("http") {
        host_label(&r.origin)
    } else {
        r.origin_label.clone()
    }
}

/// The commons a member can share to in one click: the default first, then
/// every commons already shared to, without duplicates.
pub fn known_commons(default: &str, shared_to: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for u in std::iter::once(default).chain(shared_to.iter().map(String::as_str)) {
        let u = u.trim().trim_end_matches('/');
        if !u.is_empty() && !out.iter().any(|x| x == u) {
            out.push(u.to_string());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Directory
// ---------------------------------------------------------------------------

/// A member's card: the record and the version shown (with its readable
/// profile, if the viewer may read it).
pub struct Card {
    pub rec: HandleRec,
    pub version: Version,
}

/// The directory: shared-in profiles first, then this server's members.
pub fn directory(site: &Site, cards: &[Card], remotes: &[RemoteEntry]) -> Markup {
    html! {
        header class="intro" {
            h1 { "The commons" }
            p class="lede" { "Creators keep their work on their own node. The commons lets it travel — and lets the community set the terms." }
        }
        @if cards.is_empty() && remotes.is_empty() {
            @if site.is_commons() {
                p class="empty-note" { "Nothing here yet. " a href="/join" { "Run a node" } " and share your profile in — that's how profiles arrive here." }
            } @else {
                p class="empty-note" { "Nothing here yet. " a href="/join" { "Add the first profile" } " — or run your own node and share it in." }
            }
        }
        div class="dir-grid" {
            @for r in remotes {
                a class="dir-card" href={ "/federated/" (keyid(&r.ed_pub)) } {
                    span class="dir-bar vis-federated" {}
                    span class="dir-body" {
                        @if !r.profile.avatar.is_empty() { img class="dir-avatar" src=(r.profile.avatar) alt=""; }
                        span class="dir-name" { (display_name_or_handle(&r.profile.name, &r.handle)) }
                        span class="dir-handle" { "@" (r.handle) }
                        @if !r.profile.role.is_empty() { span class="dir-role" { (r.profile.role) } }
                        // Where it came from, as prominent as the name: the node's
                        // address and a short key fingerprint (the one part the
                        // commons checked). Two cards with the same name still differ.
                        span class="dir-origin" { "from " (origin_host(r)) }
                        span class="dir-key" { "key " (short(&r.ed_pub)) }
                    }
                }
            }
            @for c in cards {
                @let reach = c.version.reach();
                a class="dir-card" href={ "/@" (c.rec.handle) } {
                    span class={ "dir-bar " (reach_bar_class(reach)) } {}
                    span class="dir-body" {
                        @if let Some(p) = &c.version.payload { @if !p.avatar.is_empty() { img class="dir-avatar" src=(p.avatar) alt=""; } }
                        span class="dir-name" {
                            @if let Some(p) = &c.version.payload { (p.name) } @else { "@" (c.rec.handle) }
                        }
                        span class="dir-handle" { "@" (c.rec.handle) }
                        @if let Some(p) = &c.version.payload {
                            @if !p.role.is_empty() { span class="dir-role" { (p.role) } }
                        } @else if reach == Reach::Members {
                            span class="dir-role locked" { "Members-only — encrypted" }
                        }
                        (reach_badge(reach))
                    }
                }
            }
        }
        div class="actions" {
            @if site.is_commons() {
                a class="primary" href="/join" { "Run a node to join" }
            } @else {
                a class="primary" href="/join" { "Add your profile" }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A member's profile
// ---------------------------------------------------------------------------

/// What the owner sees under their own profile.
pub struct OwnerPanel {
    pub csrf: String,
    /// Whether the current version may be shared (not members-only).
    pub shareable: bool,
    /// Commons offered as one-click buttons.
    pub known: Vec<String>,
    /// Commons already shared to.
    pub shared_to: Vec<String>,
    /// Whether a recovery key has been made for this handle.
    pub has_recovery: bool,
    /// Whether a key change is waiting to reach some commons.
    pub rotation_pending: bool,
}

/// A member's profile page. `readable` is the profile the viewer may read
/// (for members-only versions, only if it decrypted for them).
pub fn profile(
    rec: &HandleRec,
    version: Option<&Version>,
    readable: Option<&Profile>,
    owner: Option<&OwnerPanel>,
) -> Markup {
    html! {
        @match version {
            None => {
                header class="intro" {
                    h1 { "@" (rec.handle) }
                    p class="lede" { "This creator has not published a profile yet." }
                }
            }
            Some(v) => {
                header class="intro profile-head" {
                    (reach_badge(v.reach()))
                    @match readable {
                        Some(p) => {
                            @if !p.avatar.is_empty() { img class="profile-avatar" src=(p.avatar) alt=""; }
                            h1 { (display_name_or_handle(&p.name, &rec.handle)) }
                            @if !p.pronouns.is_empty() { p class="pronouns" { (p.pronouns) } }
                            @if !p.role.is_empty() { p class="lede" { (p.role) } }
                        }
                        None => {
                            h1 { "@" (rec.handle) }
                            p class="lede" { "This profile is members-only and encrypted to the member set. Sign in as a member to read it." }
                        }
                    }
                }
                @if let Some(p) = readable { (profile_details(p)) }
                section class="profile-meta" {
                    (gov_row(&v.license, &v.signals))
                    p class="cid" { "Version " span class="mono" { (short(&v.cid)) } " · permanent content address" }
                }
            }
        }
        @if let Some(o) = owner { (owner_panel(&rec.handle, o)) }
    }
}

/// Bio and places, shared by member and shared-in profile pages.
fn profile_details(p: &Profile) -> Markup {
    html! {
        @if !p.bio.is_empty() { section class="profile-bio" { p { (p.bio) } } }
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
}

/// The owner's "update" button and "share to a commons" controls.
fn owner_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        div class="owner-panel" {
            div class="actions owner-actions" { a class="primary" href="/new" { "Update your profile" } }
            @if o.shareable {
                section class="share-panel" {
                    h2 class="field-label" { "Share to a commons" }
                    p class="hint" { "Push a signed, verifiable copy to a shared commons \u{2014} one click. Members-only versions never leave your node." }
                    @if !o.known.is_empty() {
                        ul class="commons-list" {
                            @for c in &o.known {
                                @let shared = o.shared_to.iter().any(|s| s == c);
                                li class="commons-item" {
                                    form method="post" action="/share" class="commons-share" {
                                        input type="hidden" name="handle" value=(handle);
                                        input type="hidden" name="csrf" value=(o.csrf);
                                        input type="hidden" name="commons_url" value=(c);
                                        span class="commons-meta" {
                                            span class="commons-name" { (host_label(c)) }
                                            @if shared { span class="commons-status" { "shared \u{2713}" } }
                                        }
                                        button class="secondary" type="submit" { (if shared { "Re-share" } else { "Share" }) }
                                    }
                                }
                            }
                        }
                    }
                    details class="add-commons" open[o.known.is_empty()] {
                        summary { (if o.known.is_empty() { "Add a commons" } else { "Share to a different commons" }) }
                        form method="post" action="/share" class="share-form" {
                            input type="hidden" name="handle" value=(handle);
                            input type="hidden" name="csrf" value=(o.csrf);
                            div class="share-row" {
                                input name="commons_url" type="url" placeholder="https://commons.example";
                                button class="secondary" type="submit" { "Share" }
                            }
                        }
                    }
                }
            }
            // Shown whatever the current reach: someone who narrows a profile
            // to members-only may also want the shared copies gone.
            @if !o.shared_to.is_empty() { (withdraw_panel(handle, o)) }
            (backup_panel(handle, &o.csrf))
            (keys_panel(handle, o))
        }
    }
}

/// One "Withdraw" button per commons this member has shared to.
fn withdraw_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        section class="share-panel withdraw-panel" {
            h2 class="field-label" { "Withdraw from a commons" }
            p class="hint" { "Asks that commons to remove your shared copy. It keeps only a note that @" (handle) " is yours, so nobody else can take the name there. Copies other people already downloaded stay with them." }
            ul class="commons-list" {
                @for c in &o.shared_to {
                    li class="commons-item" {
                        form method="post" action="/withdraw" class="commons-share" {
                            input type="hidden" name="handle" value=(handle);
                            input type="hidden" name="csrf" value=(o.csrf);
                            input type="hidden" name="commons_url" value=(c);
                            span class="commons-meta" { span class="commons-name" { (host_label(c)) } }
                            button class="secondary" type="submit" { "Withdraw" }
                        }
                    }
                }
            }
        }
    }
}

/// Download an encrypted backup of this identity.
fn backup_panel(handle: &str, csrf: &str) -> Markup {
    html! {
        section class="share-panel backup-panel" {
            h2 class="field-label" { "Back up your identity" }
            p class="hint" { "Your keys live only on this computer, and each commons ties @" (handle) " to them. Download a backup, keep it somewhere safe, and you can restore @" (handle) " on a new computer. It's locked with your passphrase." }
            form method="post" action="/backup" class="share-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(csrf);
                div class="share-row" {
                    input name="passphrase" type="password" placeholder="Your passphrase" required;
                    button class="secondary" type="submit" { "Download backup" }
                }
            }
        }
    }
}

/// Recovery key and key replacement.
fn keys_panel(handle: &str, o: &OwnerPanel) -> Markup {
    html! {
        section class="share-panel keys-panel" {
            h2 class="field-label" { "Your keys" }
            @if !o.has_recovery {
                p class="hint" { "@" (handle) " has no recovery key yet. A recovery key, kept on paper, lets you take your handle back if your keys are ever stolen." }
                form method="post" action="/recovery/new" class="share-form" {
                    input type="hidden" name="handle" value=(handle);
                    input type="hidden" name="csrf" value=(o.csrf);
                    button class="secondary" type="submit" { "Make a recovery key" }
                }
            }
            p class="hint" { "Think someone else may have your keys (a lost laptop, a leaked backup)? Replace them. Every commons you've shared to moves @" (handle) " to the new key, and the old one stops working." }
            form method="post" action="/rotate" class="share-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(o.csrf);
                button class="secondary" type="submit" { (if o.rotation_pending { "Finish replacing my key" } else { "Replace my key" }) }
            }
            details class="add-commons" {
                summary { "Someone else has already replaced my key" }
                p class="hint" { "Use the recovery code you printed when you made @" (handle) ". It overrides any key change made with your old key." }
                form method="post" action="/recover" class="share-form" {
                    input type="hidden" name="handle" value=(handle);
                    input type="hidden" name="csrf" value=(o.csrf);
                    div class="share-row" {
                        input name="code" type="text" placeholder="xxxxxxxx-xxxxxxxx-…" required autocomplete="off";
                        button class="secondary" type="submit" { "Recover" }
                    }
                }
            }
        }
    }
}

/// The one-time display of a recovery code.
pub fn recovery_code(handle: &str, code: &str, next: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Your recovery code" }
            p class="lede" { "Write this down or print this page, and keep it somewhere safe and offline. It's shown only once." }
        }
        section class="recovery" {
            p class="recovery-code mono" { (code) }
            p class="hint" {
                "If your keys are ever stolen, this code lets you take @" (handle) " back on every commons, even if the thief changed your key first. "
                "Don't keep it on this computer or in your backup file: anyone with your laptop or backup could then use it too."
            }
        }
        div class="actions" { a class="primary" href=(next) { "I've saved it" } }
    }
}

/// The result of replacing or recovering a key.
pub fn key_change(handle: &str, done: &[String], failed: &[(String, String)], new_code: Option<&str>) -> Markup {
    html! {
        header class="intro" {
            @if failed.is_empty() {
                h1 { "Key replaced" }
                p class="lede" { "@" (handle) " now uses a new key on this node and on every commons you've shared to. The old key no longer works." }
            } @else {
                h1 { "Not finished yet" }
                p class="lede" { "Some commons couldn't be reached. This node keeps using your current key until every commons has the change; press the button again later to finish." }
            }
        }
        @if !done.is_empty() {
            p { "Accepted by: " @for (i, c) in done.iter().enumerate() { @if i > 0 { ", " } (host_label(c)) } }
        }
        @if !failed.is_empty() {
            ul { @for (c, why) in failed { li { strong { (host_label(c)) } ": " (why) } } }
        }
        @if let Some(code) = new_code {
            section class="recovery" {
                h2 class="field-label" { "Your new recovery code" }
                p class="recovery-code mono" { (code) }
                p class="hint" { "Your old recovery code no longer works. Write this one down; it's shown only once." }
            }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}

/// Restore an identity from a backup file (nodes only).
pub fn restore_form() -> Markup {
    html! {
        header class="intro" {
            h1 { "Restore from a backup" }
            p class="lede" { "Open your backup file in a text editor, copy everything in it, and paste it below with the passphrase you used." }
        }
        form method="post" action="/restore" class="stack-form" {
            div class="field" {
                label class="field-label" for="backup" { "Backup file contents" }
                textarea id="backup" name="backup" rows="10" required placeholder="{ \"format\": \"hic-identity-backup/1\", … }" {}
            }
            div class="field" {
                label class="field-label" for="passphrase" { "Passphrase" }
                input id="passphrase" name="passphrase" type="password" maxlength="200" required;
            }
            div class="actions" { button class="primary" type="submit" { "Restore" } }
        }
    }
}

/// Confirmation after a withdrawal.
pub fn withdrawn(handle: &str, commons_label: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Withdrawn" }
            p class="lede" { (commons_label) " has removed its copy of @" (handle) ". You can share again whenever you like." }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}

// ---------------------------------------------------------------------------
// Sign-up, sign-in, the commons gate
// ---------------------------------------------------------------------------

/// The sign-up form (nodes only).
pub fn join_form() -> Markup {
    html! {
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
        p class="hint" { "Moving to a new computer? " a href="/restore" { "Restore from a backup" } "." }
    }
}

/// The sign-in form (nodes only).
pub fn login_form() -> Markup {
    html! {
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
    }
}

/// Shown instead of sign-up and sign-in on a commons: explains that profiles
/// arrive from people's own nodes.
pub fn commons_gate(site: &Site) -> Markup {
    html! {
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
                    @if !site.instance.is_empty() { " (" (site.instance) ")" } "."
                }
            }
            div class="actions" { a class="primary" href=(APP_DOWNLOAD) { "Get the app \u{2192}" } }
            p class="hint" {
                "A signed copy of your profile then appears here. Members-only profiles stay "
                "encrypted on your node and are never shared — not even to a commons."
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Publishing
// ---------------------------------------------------------------------------

/// The publish form, prefilled from the current version.
pub fn publish_form(
    me: &str,
    csrf: &str,
    p: &Profile,
    reach: Reach,
    license: &str,
    signals: &[String],
    first_time: bool,
) -> Markup {
    html! {
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
            (profile_fields(p))
            (reach_and_governance(reach, license, signals))
            div class="actions" { button class="primary" type="submit" { "Publish to the commons" } }
        }
        script src="/avatar.js" {}
    }
}

fn profile_fields(p: &Profile) -> Markup {
    let place = p.places.first();
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
        div class="field" {
            label class="field-label" for="place" { "Where you work" }
            input id="place" name="place_name" type="text" maxlength="120" value=(place.map(|x| x.name.as_str()).unwrap_or("")) placeholder="Studio or institution";
            input id="place_city" name="place_city" type="text" maxlength="80" value=(place.map(|x| x.city.as_str()).unwrap_or("")) placeholder="City" class="city-input";
        }
    }
}

fn reach_and_governance(sel: Reach, sel_license: &str, sel_sig: &[String]) -> Markup {
    html! {
        fieldset class="reach-options" {
            legend class="field-label" { "Who can see this" }
            @for (val, title, desc) in [
                (Reach::Public, "Public", "Listed in the commons directory and open to search engines."),
                (Reach::Commons, "Listed in the commons", "In the directory; anyone with the link can read it, kept out of search."),
                (Reach::Members, "Members only", "Encrypted to the member set. Only signed-in members can read it."),
            ] {
                label class="reach" {
                    input type="radio" name="visibility" value=(val.as_str()) checked[val == sel];
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
            p {
                strong { "Publishing cannot be fully undone." }
                " Each version gets a permanent content address. You can withdraw copies you share to a commons, but anyone who already downloaded one keeps it."
            }
        }
    }
}

/// Confirmation after a successful share.
pub fn shared(handle: &str, commons: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Shared" }
            p class="lede" { "Your profile is now federated to " a href=(commons) { (commons) } ". They have a signed, verifiable copy." }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}

// ---------------------------------------------------------------------------
// Shared-in profiles and the about page
// ---------------------------------------------------------------------------

/// A profile shared in from another node, with where it came from.
pub fn federated(r: &RemoteEntry) -> Markup {
    let p = &r.profile;
    html! {
        header class="intro profile-head" {
            span class="vis-badge vis-federated" { "Federated" }
            @if !p.avatar.is_empty() { img class="profile-avatar" src=(p.avatar) alt=""; }
            h1 { (display_name_or_handle(&p.name, &r.handle)) }
            @if !p.pronouns.is_empty() { p class="pronouns" { (p.pronouns) } }
            @if !p.role.is_empty() { p class="lede" { (p.role) } }
        }
        (profile_details(p))
        section class="profile-meta" {
            (gov_row(&r.license, &r.signals))
            div class="origin-box" {
                p class="origin-line" {
                    "Shared from "
                    @if r.origin.starts_with("http") { a href=(r.origin) { (origin_host(r)) } } @else { strong { (origin_host(r)) } }
                    @if !r.origin_label.is_empty() { span class="origin-label" { " — the node calls itself “" (r.origin_label) "”" } }
                }
                p class="origin-key" { "Signed by key " span class="mono" { (short(&r.ed_pub)) } }
                @if r.signed.is_some() {
                    p class="origin-note" { "On this commons, @" (r.handle) " belongs to that key alone. Everything on this page is signed by it, so nobody could change it on the way here. The address and node name are still what the node says about itself." }
                } @else {
                    p class="origin-note" { "On this commons, @" (r.handle) " belongs to that key alone. Shared in the older format: the profile text is signed, but the picture, name, address and node name are not." }
                }
            }
            p class="cid" { "Version " span class="mono" { (short(&r.cid)) } }
        }
    }
}

/// How the system works, in plain language.
pub fn about() -> Markup {
    html! {
        header class="intro" { h1 { "How this works" } p class="lede" { "A federated commons for creative data — govern the models, not only the data." } }
        section class="prose" {
            h2 class="section-h" { "Your node, your keys" }
            p { "You claim a human handle. The node mints a signing keypair and an encryption keypair for you and holds them — you never touch a private key." }
            h2 class="section-h" { "Content-addressed versions" }
            p { "Every publish is hashed into a permanent content address and signed by your key. You can withdraw a copy you shared to a commons, and it is removed there; a copy someone has already downloaded stays with them." }
            h2 class="section-h" { "Three reaches" }
            p { "Public is listed and indexable. Commons is listed but asks search engines not to index it. Members-only is encrypted to every member's public key — only signed-in members of this node can read it. The node itself holds those keys, so the encryption protects copies outside this node, not from whoever runs it." }
            h2 class="section-h" { "Govern the models" }
            p { "Each version carries a Creative Commons license and CC signals — machine-readable preferences for how models and datasets may learn from your work." }
            h2 class="section-h" { "Federation" }
            p { "A node sends a signed copy of a profile to a commons, which checks the signature before listing it. The messages are shaped like ActivityStreams, but this isn't yet full ActivityPub: other fediverse servers can't follow it." }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_commons_dedups_and_keeps_default_first() {
        let got = known_commons("http://a/", &["http://b".into(), "http://a".into()]);
        assert_eq!(got, vec!["http://a".to_string(), "http://b".to_string()]);
        assert!(known_commons("", &[]).is_empty());
    }

    #[test]
    fn caution_no_longer_promises_withdrawal() {
        let m = publish_form("tega", "x", &Profile::default(), Reach::Public, "cc-by", &[], true).into_string();
        assert!(m.contains("anyone who already downloaded"));
        assert!(!m.contains("stops serving"));
    }
}
