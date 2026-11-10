//! The HTML side of the server.
//!
//! - [`layout`]: the page frame and shared pieces (pure);
//! - [`pages`]: the body of each page (pure);
//! - [`handlers`]: the request handlers that load data and render pages;
//! - [`account`]: handlers for a member's keys (recovery key, replacing a key, recovering);
//! - [`membership`]: handlers for bringing people into a commons (invites, vouches);
//! - [`steward`]: the steward view of a commons (approve, decline, make invites);
//! - [`email`]: ready-written invitation emails as `mailto:` links (pure).

pub mod account;
pub mod email;
pub mod handlers;
pub mod layout;
pub mod membership;
pub mod pages;
pub mod steward;

/// The stylesheet, compiled into the binary.
pub const STYLE_CSS: &str = include_str!("../../static/style.css");

/// The show/hide button for passphrase boxes, loaded on every page.
pub const PASSWORD_JS: &str = include_str!("../../static/password.js");

/// The profile-picture picker used on the publish form.
pub const AVATAR_JS: &str = include_str!("../../static/avatar.js");

/// Tells search engines to skip the forms. Profiles with the `commons` reach
/// are marked `noindex` on the page itself.
pub const ROBOTS_TXT: &str = "User-agent: *\nDisallow: /join\nDisallow: /login\nDisallow: /new\n";
