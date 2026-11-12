//! How the system works, in plain language, with three diagrams drawn after
//! the workshop slides (heldincommon.org/workshop-slides): how your work is
//! held, how nodes share into a commons, and how someone joins.
//!
//! The diagrams are inline SVG with `aria-hidden`: the caption under each
//! says the same thing in words, so a screen reader loses nothing. Colours
//! come from classes (`fig-pink`, `fig-ink`…) styled in `style.css` with the
//! site's tokens, not from colour values in the markup.

use maud::{html, Markup, PreEscaped};

/// How the system works, in plain language. `on_node` adds a pointer to
/// running a commons.
pub fn about(on_node: bool) -> Markup {
    html! {
        header class="intro" { h1 { "How this works" } p class="lede" { "A federated commons for creative data — govern the models, not only the data." } }
        section class="prose" {
            h2 class="section-h" { "Your node, your keys" }
            p { "You claim a human handle. The node mints a signing keypair and an encryption keypair for you and holds them — you never touch a private key." }
            // PreEscaped: the diagrams are fixed markup written here, never
            // anything a visitor typed, so they go in as they are.
            figure class="schematic" {
                (PreEscaped(HELD_SVG))
                figcaption { "Read it left to right: " strong { "you" } " hold a handle and a key. Your " strong { "profile" } " and " strong { "stories" } " live on your node. Every change is saved as a " strong { "version" } " with a permanent fingerprint, and each version carries its " strong { "licence" } " and " strong { "signals" } "." }
            }
            h2 class="section-h" { "Content-addressed versions" }
            p { "Every publish is hashed into a permanent content address and signed by your key. You can withdraw a copy you shared to a commons, and it is removed there; a copy someone has already downloaded stays with them." }
            h2 class="section-h" { "Three reaches" }
            p { "Public is listed and indexable. Commons is listed but asks search engines not to index it. Members-only is encrypted to every member's public key — only signed-in members of this node can read it. The node itself holds those keys, so the encryption protects copies outside this node, not from whoever runs it." }
            h2 class="section-h" { "Govern the models" }
            p { "Each version carries a Creative Commons licence and signals — machine-readable preferences for how models and datasets may learn from your work." }
            h2 class="section-h" { "Federation" }
            figure class="schematic" {
                (PreEscaped(FEDERATE_SVG))
                figcaption { "You send a " strong { "signed copy" } " to a commons; your original never moves. The commons checks the signature and gathers everyone's copies into one directory people can browse: no central vault, no landlord." }
            }
            p { "The messages are shaped like ActivityStreams, but this isn't yet full ActivityPub: other fediverse servers can't follow it." }
            h2 class="section-h" { "No sign-up. Someone vouches." }
            figure class="schematic" {
                (PreEscaped(JOIN_SVG))
                figcaption { "Share your profile and you wait on the commons' list. A " strong { "member vouches" } " for you, or the " strong { "steward approves" } " you, and you're listed. An invite code from a member lists you straight away." }
            }
            @if on_node {
                h2 class="section-h" { "Hosting a commons" }
                p { "Anyone can run a commons for their workshop, class or collective, alongside their own node. " a href="/run-a-commons" { "Run your own commons" } " answers the common questions and lets you try a practice one." }
            }
        }
    }
}

/// "How your work is held": you, your profile and stories, a version, and
/// the licence and signals it carries.
const HELD_SVG: &str = r##"<svg class="fig" viewBox="0 0 1000 300" aria-hidden="true">
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
const FEDERATE_SVG: &str = r##"<svg class="fig" viewBox="0 0 980 400" aria-hidden="true">
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
const JOIN_SVG: &str = r##"<svg class="fig" viewBox="0 0 1200 240" aria-hidden="true">
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_diagram_is_hidden_from_screen_readers_and_explained_in_words() {
        let page = about(true).into_string();
        assert_eq!(page.matches("<svg").count(), 3);
        assert_eq!(page.matches(r#"aria-hidden="true""#).count(), 3);
        assert_eq!(page.matches("<figcaption>").count(), 3, "every diagram has its words");
    }

    #[test]
    fn diagrams_take_colours_from_the_stylesheet_not_the_markup() {
        for svg in [HELD_SVG, FEDERATE_SVG, JOIN_SVG] {
            assert!(!svg.contains("fill=\"#") && !svg.contains("stroke=\"#"), "no colour values in the markup");
            assert!(!svg.contains("style="), "no inline styles");
        }
    }
}
