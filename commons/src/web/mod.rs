//! The HTML side of the server.
//!
//! - [`layout`]: the page frame and shared pieces (pure);
//! - [`pages`]: the body of each page (pure);
//! - [`handlers`]: the request handlers that load data and render pages.

pub mod handlers;
pub mod layout;
pub mod pages;

/// The stylesheet, compiled into the binary.
pub const STYLE_CSS: &str = include_str!("../../static/style.css");

/// The profile-picture picker used on the publish form.
pub const AVATAR_JS: &str = include_str!("../../static/avatar.js");

/// Tells search engines to skip the forms. Profiles with the `commons` reach
/// are marked `noindex` on the page itself.
pub const ROBOTS_TXT: &str = "User-agent: *\nDisallow: /join\nDisallow: /login\nDisallow: /new\n";
