//! The HTML side of the server.
//!
//! - [`layout`]: the page frame and shared pieces (pure);
//! - [`pages`]: the body of each page, one file per page (pure);
//! - [`handlers`]: the request handlers that load data and render pages, one
//!   file per page or flow (including the steward view, invites and vouches,
//!   and a member's keys);
//! - [`email`]: ready-written invitation emails as `mailto:` links (pure).
//!
//! To add a page: write its body in `pages/<name>.rs`, its handler in
//! `handlers/<name>.rs`, re-export both from their `mod.rs`, and add the route
//! in [`crate::app`].

pub mod email;
pub mod handlers;
pub mod layout;
pub mod pages;

/// The stylesheet, compiled into the binary.
pub const STYLE_CSS: &str = include_str!("../../static/style.css");

/// The show/hide button for passphrase boxes, loaded on every page.
pub const PASSWORD_JS: &str = include_str!("../../static/password.js");

/// The profile-picture picker used on the publish form.
pub const AVATAR_JS: &str = include_str!("../../static/avatar.js");

/// Tells search engines to skip the forms. Profiles with the `commons` reach
/// are marked `noindex` on the page itself.
/// The Held in Common logo (the same file heldincommon.org uses).
pub const LOGO_PNG: &[u8] = include_bytes!("../../static/logo.png");

pub const ROBOTS_TXT: &str = "User-agent: *\nDisallow: /join\nDisallow: /login\nDisallow: /new\n";
