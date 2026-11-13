//! The HTML side of the server.
//!
//! - [`layout`]: the page frame and shared pieces (pure);
//! - [`pages`]: the body of each page, one file per page (pure);
//! - [`handlers`]: the request handlers that load data and render pages, one
//!   file per page or flow (including the steward view, invites and vouches,
//!   and a member's keys);
//! - [`email`]: ready-written invitation emails as `mailto:` links (pure);
//! - [`qr`]: QR codes for the room page, as SVG (pure).
//!
//! To add a page: write its body in `pages/<name>.rs`, its handler in
//! `handlers/<name>.rs`, re-export both from their `mod.rs`, and add the route
//! in [`crate::app`].

pub mod email;
pub mod handlers;
pub mod layout;
pub mod pages;
pub mod qr;

/// The stylesheet, compiled into the binary.
pub const STYLE_CSS: &str = include_str!("../../static/style.css");

/// The show/hide button for passphrase boxes, loaded on every page.
pub const PASSWORD_JS: &str = include_str!("../../static/password.js");

/// The profile-picture picker used on the publish form.
pub const AVATAR_JS: &str = include_str!("../../static/avatar.js");

/// Keeps the "What travels with every copy" summary in step with the rights choices.
pub const RIGHTS_JS: &str = include_str!("../../static/rights.js");

/// The eight quilt backgrounds, made by `scripts/make-quilts.py` and served at
/// `/quilt/1.svg` … `/quilt/8.svg`. Each page shows one, chosen from its title
/// by [`layout::quilt_for`].
pub const QUILTS: [&str; 8] = [
    include_str!("../../static/quilt/quilt-1.svg"),
    include_str!("../../static/quilt/quilt-2.svg"),
    include_str!("../../static/quilt/quilt-3.svg"),
    include_str!("../../static/quilt/quilt-4.svg"),
    include_str!("../../static/quilt/quilt-5.svg"),
    include_str!("../../static/quilt/quilt-6.svg"),
    include_str!("../../static/quilt/quilt-7.svg"),
    include_str!("../../static/quilt/quilt-8.svg"),
];

/// The quilt a `/quilt/<file>` request names (`"1.svg"` … `"8.svg"`), or
/// `None` for anything else, which the route answers with 404.
pub fn quilt_svg(file: &str) -> Option<&'static str> {
    let number: usize = file.strip_suffix(".svg")?.parse().ok()?;
    QUILTS.get(number.checked_sub(1)?).copied()
}

/// The Held in Common logo (the same file heldincommon.org uses).
pub const LOGO_PNG: &[u8] = include_bytes!("../../static/logo.png");

/// `/robots.txt`: asks search engines to skip the sign-in and sign-up pages.
/// Profiles with the `commons` reach are marked `noindex` on the page itself.
pub const ROBOTS_TXT: &str = "User-agent: *\nDisallow: /join\nDisallow: /login\nDisallow: /new\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_quilt_has_its_own_address_and_nothing_else_does() {
        for n in 1..=8 {
            assert!(quilt_svg(&format!("{n}.svg")).is_some_and(|svg| svg.starts_with("<svg")), "{n}");
        }
        for bad in ["0.svg", "9.svg", "1", "1.png", "../style.css", "-1.svg", ""] {
            assert_eq!(quilt_svg(bad), None, "{bad}");
        }
    }
}
