//! The diagrams on "How this works" and "Run your own commons", drawn after
//! the workshop slides (heldincommon.org/workshop-slides).
//!
//! Each is inline SVG with `aria-hidden`: the caption beside it (see
//! [`figure`]) says the same thing in words, so a screen reader loses
//! nothing. Parts are named with classes (`fig-pink`, `fig-box`…) and
//! coloured in `style.css` from the site's tokens, never in the markup.

use maud::{html, Markup, PreEscaped};

/// A diagram with its caption. `svg` is one of the constants below: fixed
/// markup written here, never anything a visitor typed, so `PreEscaped`
/// puts it in as it is.
pub fn figure(svg: &'static str, caption: Markup) -> Markup {
    html! {
        figure class="schematic" {
            (PreEscaped(svg))
            figcaption { (caption) }
        }
    }
}

/// "How your work is held": you, your profile and stories, a version, and
/// the licence and signals it carries.
pub const HELD_SVG: &str = r##"<svg class="fig" viewBox="0 0 1000 300" aria-hidden="true">
  <g class="fig-line">
    <path d="M90 150 H250"/>
    <path d="M300 150 C 360 150, 360 90, 430 90"/>
    <path d="M300 150 C 360 150, 360 210, 430 210"/>
    <path d="M560 90 C 620 90, 620 150, 680 150"/>
    <path d="M560 210 C 620 210, 620 150, 680 150"/>
    <path d="M712 146 C 770 120, 780 100, 830 100"/>
    <path d="M712 154 C 770 180, 780 200, 830 200"/>
  </g>
  <circle class="fig-ink" cx="70" cy="150" r="15"/>
  <text class="fig-label" x="70" y="196" text-anchor="middle">YOU</text>
  <text class="fig-note" x="70" y="220" text-anchor="middle">handle + key</text>
  <circle class="fig-pink" cx="275" cy="150" r="15"/>
  <text class="fig-label" x="275" y="196" text-anchor="middle">PROFILE</text>
  <text class="fig-note" x="275" y="220" text-anchor="middle">who you are</text>
  <circle class="fig-blue" cx="450" cy="90" r="13"/>
  <text class="fig-label" x="474" y="84">STORIES</text>
  <text class="fig-note" x="474" y="106">your writing</text>
  <circle class="fig-blue" cx="450" cy="210" r="13"/>
  <text class="fig-label" x="474" y="204">WORKS</text>
  <text class="fig-note" x="474" y="226">planned</text>
  <circle class="fig-ink" cx="696" cy="150" r="15"/>
  <circle class="fig-pink" cx="696" cy="150" r="6"/>
  <text class="fig-label" x="696" y="196" text-anchor="middle">VERSION</text>
  <text class="fig-note" x="696" y="220" text-anchor="middle">fingerprint</text>
  <circle class="fig-jade" cx="844" cy="100" r="12"/>
  <text class="fig-label fig-jade-text" x="866" y="106">LICENCE</text>
  <circle class="fig-jade" cx="844" cy="200" r="12"/>
  <text class="fig-label fig-jade-text" x="866" y="206">SIGNALS</text>
</svg>"##;

/// "Nodes federate into a commons": four nodes, each sending a signed copy
/// to the commons in the middle.
pub const FEDERATE_SVG: &str = r##"<svg class="fig" viewBox="0 0 980 400" aria-hidden="true">
  <g class="fig-line">
    <line x1="170" y1="110" x2="440" y2="195"/>
    <line x1="810" y1="110" x2="540" y2="195"/>
    <line x1="170" y1="320" x2="440" y2="235"/>
    <line x1="810" y1="320" x2="540" y2="235"/>
  </g>
  <rect class="fig-pink" x="300" y="140" width="20" height="26" rx="3"/>
  <rect class="fig-blue" x="660" y="140" width="20" height="26" rx="3"/>
  <rect class="fig-jade" x="300" y="262" width="20" height="26" rx="3"/>
  <rect class="fig-violet" x="660" y="262" width="20" height="26" rx="3"/>
  <g transform="translate(110,70)"><rect class="fig-box fig-box-pink" width="120" height="82" rx="12"/><circle class="fig-ring" cx="60" cy="41" r="15"/><circle class="fig-pink" cx="60" cy="41" r="4"/></g>
  <g transform="translate(750,70)"><rect class="fig-box" width="120" height="82" rx="12"/><circle class="fig-ring" cx="60" cy="41" r="15"/><circle class="fig-ink" cx="60" cy="41" r="4"/></g>
  <g transform="translate(110,280)"><rect class="fig-box" width="120" height="82" rx="12"/><circle class="fig-ring" cx="60" cy="41" r="15"/><circle class="fig-ink" cx="60" cy="41" r="4"/></g>
  <g transform="translate(750,280)"><rect class="fig-box" width="120" height="82" rx="12"/><circle class="fig-ring" cx="60" cy="41" r="15"/><circle class="fig-ink" cx="60" cy="41" r="4"/></g>
  <circle class="fig-hub" cx="490" cy="215" r="64"/>
  <g class="fig-spokes">
    <line x1="490" y1="215" x2="463" y2="192"/><line x1="490" y1="215" x2="517" y2="192"/>
    <line x1="490" y1="215" x2="463" y2="240"/><line x1="490" y1="215" x2="517" y2="240"/>
  </g>
  <circle class="fig-blue" cx="463" cy="192" r="6"/><circle class="fig-blue" cx="517" cy="192" r="6"/>
  <circle class="fig-blue" cx="463" cy="240" r="6"/><circle class="fig-blue" cx="517" cy="240" r="6"/>
  <circle class="fig-pink" cx="490" cy="215" r="7"/>
  <text class="fig-label fig-pink-text" x="170" y="175" text-anchor="middle">YOUR NODE</text>
  <text class="fig-note" x="810" y="175" text-anchor="middle">ANOTHER STUDIO</text>
  <text class="fig-note" x="170" y="385" text-anchor="middle">A COLLABORATOR</text>
  <text class="fig-note" x="810" y="385" text-anchor="middle">A GALLERY'S NODE</text>
  <text class="fig-label fig-blue-text" x="490" y="312" text-anchor="middle">THE COMMONS</text>
</svg>"##;

/// "No sign-up. Someone vouches.": share, wait, a vouch or the steward's
/// approval, listed.
pub const JOIN_SVG: &str = r##"<svg class="fig" viewBox="0 0 1200 240" aria-hidden="true">
  <g class="fig-line">
    <path d="M150 110 H330"/>
    <path d="M410 110 C 480 110, 480 60, 560 60"/>
    <path d="M410 110 C 480 110, 480 160, 560 160"/>
    <path d="M920 60 C 990 60, 990 110, 1050 110"/>
    <path d="M920 160 C 990 160, 990 110, 1050 110"/>
  </g>
  <rect class="fig-box fig-box-pink" x="30" y="78" width="120" height="64" rx="12"/>
  <text class="fig-label" x="90" y="116" text-anchor="middle">SHARE</text>
  <text class="fig-note" x="90" y="176" text-anchor="middle">a signed copy</text>
  <circle class="fig-hub" cx="370" cy="110" r="40"/>
  <circle class="fig-pink" cx="370" cy="110" r="7"/>
  <text class="fig-label" x="370" y="182" text-anchor="middle">WAITING</text>
  <text class="fig-note" x="370" y="206" text-anchor="middle">on the commons</text>
  <circle class="fig-jade" cx="580" cy="60" r="13"/>
  <text class="fig-label fig-jade-text" x="602" y="67">A MEMBER VOUCHES</text>
  <circle class="fig-violet" cx="580" cy="160" r="13"/>
  <text class="fig-label fig-violet-text" x="602" y="167">THE STEWARD APPROVES</text>
  <circle class="fig-ink" cx="1080" cy="110" r="18"/>
  <circle class="fig-jade" cx="1080" cy="110" r="7"/>
  <text class="fig-label" x="1080" y="160" text-anchor="middle">LISTED</text>
  <text class="fig-note" x="1080" y="184" text-anchor="middle">in the directory</text>
</svg>"##;

/// "Side by side": one computer running your node, a commons and a local
/// commons, each with its own folder and port; the node shares to the
/// commons like anyone else's would.
pub const SIDE_BY_SIDE_SVG: &str = r##"<svg class="fig" viewBox="0 0 1000 300" aria-hidden="true">
  <rect class="fig-box" x="20" y="20" width="960" height="250" rx="22"/>
  <text class="fig-note" x="50" y="58">THIS COMPUTER</text>
  <g class="fig-line">
    <path d="M310 160 H400"/>
  </g>
  <rect class="fig-box fig-box-pink" x="50" y="100" width="260" height="120" rx="14"/>
  <circle class="fig-pink" cx="84" cy="140" r="12"/>
  <text class="fig-label" x="106" y="147">YOUR NODE</text>
  <text class="fig-note" x="72" y="190">your keys, your profile</text>
  <rect class="fig-pink" x="345" y="147" width="20" height="26" rx="3"/>
  <rect class="fig-box fig-box-blue" x="400" y="100" width="260" height="120" rx="14"/>
  <circle class="fig-blue" cx="434" cy="140" r="12"/>
  <text class="fig-label" x="456" y="147">A COMMONS</text>
  <text class="fig-note" x="422" y="190">signed copies only</text>
  <rect class="fig-box fig-box-violet" x="690" y="100" width="260" height="120" rx="14"/>
  <circle class="fig-violet" cx="724" cy="140" r="12"/>
  <text class="fig-label" x="746" y="147">LOCAL COMMONS</text>
  <text class="fig-note" x="712" y="190">anyone on this Wi-Fi</text>
</svg>"##;

/// "Where people reach it": three rings, from your node (this computer) to a
/// local commons (the same Wi-Fi) to an online commons (anyone with its
/// address).
pub const REACH_SVG: &str = r##"<svg class="fig" viewBox="0 0 1000 330" aria-hidden="true">
  <circle class="fig-ring-jade" cx="200" cy="165" r="150"/>
  <circle class="fig-ring-violet" cx="200" cy="165" r="100"/>
  <circle class="fig-ring-pink" cx="200" cy="165" r="50"/>
  <circle class="fig-ink" cx="200" cy="165" r="9"/>
  <circle class="fig-pink" cx="420" cy="90" r="12"/>
  <text class="fig-label fig-pink-text" x="444" y="97">YOUR NODE</text>
  <text class="fig-note" x="444" y="121">this computer only</text>
  <circle class="fig-violet" cx="420" cy="175" r="12"/>
  <text class="fig-label fig-violet-text" x="444" y="182">LOCAL COMMONS</text>
  <text class="fig-note" x="444" y="206">anyone on this Wi-Fi, while the app is open</text>
  <circle class="fig-jade" cx="420" cy="260" r="12"/>
  <text class="fig-label fig-jade-text" x="444" y="267">ONLINE COMMONS</text>
  <text class="fig-note" x="444" y="291">anyone with its address: a server, or Tailscale Funnel</text>
</svg>"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagrams_take_colours_from_the_stylesheet_not_the_markup() {
        for svg in [HELD_SVG, FEDERATE_SVG, JOIN_SVG, SIDE_BY_SIDE_SVG, REACH_SVG] {
            assert!(svg.contains(r#"aria-hidden="true""#), "hidden from screen readers: the caption says it");
            assert!(!svg.contains("fill=\"#") && !svg.contains("stroke=\"#"), "no colour values in the markup");
            assert!(!svg.contains("style="), "no inline styles");
        }
    }
}
