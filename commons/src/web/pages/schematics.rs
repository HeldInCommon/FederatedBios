//! The diagrams on "How this works" and "Run a commons". The workshop
//! slides (`docs/architecture.html`) draw the same pictures.
//!
//! Each is inline SVG with `aria-hidden`: the caption beside it (see
//! [`figure`]) says the same thing in words, so a screen reader loses
//! nothing. Parts are named with classes (`fig-card`, `fig-pink`…) and
//! coloured in `style.css` from the site's tokens, never in the markup.
//!
//! One drawing vocabulary runs through all of them:
//!
//! - a square card with a hard shadow is a node (a computer someone runs);
//!   a pink shadow marks yours;
//! - a circle is a commons;
//! - a small tab on a line is a signed copy;
//! - colour lives in shapes, never in words: pink is yours, blue a commons
//!   or its steward, violet the room (the same Wi-Fi), jade online, listed
//!   or agreed;
//! - lines are solid ink with one arrowhead, so you can see which way
//!   things go; a dashed line is a shortcut or something that comes later.
//!
//! Each SVG names its arrowhead marker with an id of its own (`held-head`,
//! `join-head`…), because the diagrams share one page and ids must be
//! unique on a page.

use maud::{html, Markup, PreEscaped};

/// A diagram with its caption. `svg` is one of the constants below: fixed
/// markup written here, never anything a visitor typed, so `PreEscaped`
/// puts it in as it is.
///
/// On a phone the diagram keeps a readable size and scrolls sideways
/// inside its frame (see `.fig-frame` in `style.css`); the frame takes
/// keyboard focus so it can be scrolled without a mouse.
pub fn figure(svg: &'static str, caption: Markup) -> Markup {
    html! {
        figure class="schematic" {
            div class="fig-frame" tabindex="0" role="group" aria-label="Diagram" {
                (PreEscaped(svg))
            }
            figcaption { (caption) }
        }
    }
}

/// A diagram with a second drawing for phones, and one caption. The
/// stylesheet shows `wide` on larger screens and `narrow` (stacked top to
/// bottom) on small ones, so a phone never has to scroll it.
pub fn figure_with_phone(wide: &'static str, narrow: &'static str, caption: Markup) -> Markup {
    html! {
        figure class="schematic" {
            div class="fig-wide" { (PreEscaped(wide)) }
            div class="fig-narrow" { (PreEscaped(narrow)) }
            figcaption { (caption) }
        }
    }
}

/// "How your work is held": you, then your profile, stories and (planned)
/// works on your node, then a version, which carries a licence and
/// signals.
pub const HELD_SVG: &str = r##"<svg class="fig" viewBox="40 96 920 364" aria-hidden="true">
  <defs><marker id="held-head" orient="auto" markerWidth="5" markerHeight="5" refX="2" refY="2"><path class="fig-head" d="M0 0 L4 2 L0 4 Z"/></marker></defs>
  <rect class="fig-band" x="304" y="112" width="264" height="320"/>
  <text class="fig-band-label" x="328" y="143">On your node</text>
  <path class="fig-line" d="M240 280 H297" marker-end="url(#held-head)"/>
  <path class="fig-line" d="M568 280 H657" marker-end="url(#held-head)"/>
  <path class="fig-line" d="M800 328 V352 M728 352 H872 M728 352 V384 M872 352 V384"/>
  <rect class="fig-pink" x="70" y="238" width="176" height="96"/>
  <rect class="fig-card" x="64" y="232" width="176" height="96"/>
  <text class="fig-label" x="152" y="276" text-anchor="middle">You</text>
  <text class="fig-note" x="152" y="298" text-anchor="middle">a handle and a key</text>
  <rect class="fig-ink" x="342" y="166" width="200" height="64"/>
  <rect class="fig-card" x="336" y="160" width="200" height="64"/>
  <text class="fig-label" x="436" y="188" text-anchor="middle">Profile</text>
  <text class="fig-note" x="436" y="210" text-anchor="middle">who you are</text>
  <rect class="fig-ink" x="342" y="254" width="200" height="64"/>
  <rect class="fig-card" x="336" y="248" width="200" height="64"/>
  <text class="fig-label" x="436" y="276" text-anchor="middle">Stories</text>
  <text class="fig-note" x="436" y="298" text-anchor="middle">your writing</text>
  <rect class="fig-planned" x="336" y="336" width="200" height="64"/>
  <text class="fig-label" x="436" y="364" text-anchor="middle">Works</text>
  <text class="fig-note" x="436" y="386" text-anchor="middle">planned</text>
  <rect class="fig-ink" x="670" y="238" width="272" height="96"/>
  <rect class="fig-card" x="664" y="232" width="272" height="96"/>
  <text class="fig-label" x="800" y="276" text-anchor="middle">Version</text>
  <text class="fig-note" x="800" y="298" text-anchor="middle">a permanent fingerprint</text>
  <rect class="fig-jade" x="670" y="390" width="128" height="56"/>
  <rect class="fig-card" x="664" y="384" width="128" height="56"/>
  <text class="fig-label" x="728" y="418" text-anchor="middle">Licence</text>
  <rect class="fig-jade" x="814" y="390" width="128" height="56"/>
  <rect class="fig-card" x="808" y="384" width="128" height="56"/>
  <text class="fig-label" x="872" y="418" text-anchor="middle">Signals</text>
</svg>"##;

/// "Nodes federate into a commons": four nodes around one commons, each
/// sending it a signed copy.
pub const FEDERATE_SVG: &str = r##"<svg class="fig" viewBox="40 112 920 576" aria-hidden="true">
  <defs><marker id="federate-head" orient="auto" markerWidth="5" markerHeight="5" refX="2" refY="2"><path class="fig-head" d="M0 0 L4 2 L0 4 Z"/></marker></defs>
  <path class="fig-line" d="M264 400 H413" marker-end="url(#federate-head)"/>
  <path class="fig-line" d="M736 400 H587" marker-end="url(#federate-head)"/>
  <path class="fig-line" d="M500 224 V313" marker-end="url(#federate-head)"/>
  <path class="fig-line" d="M500 576 V487" marker-end="url(#federate-head)"/>
  <rect class="fig-ink" x="406" y="134" width="200" height="96"/>
  <rect class="fig-card" x="400" y="128" width="200" height="96"/>
  <text class="fig-label" x="500" y="182" text-anchor="middle">A collaborator</text>
  <rect class="fig-pink" x="70" y="358" width="200" height="96"/>
  <rect class="fig-card" x="64" y="352" width="200" height="96"/>
  <text class="fig-label" x="164" y="396" text-anchor="middle">Your node</text>
  <text class="fig-note" x="164" y="418" text-anchor="middle">the original stays</text>
  <rect class="fig-ink" x="742" y="358" width="200" height="96"/>
  <rect class="fig-card" x="736" y="352" width="200" height="96"/>
  <text class="fig-label" x="836" y="406" text-anchor="middle">Another studio</text>
  <rect class="fig-ink" x="406" y="582" width="200" height="96"/>
  <rect class="fig-card" x="400" y="576" width="200" height="96"/>
  <text class="fig-label" x="500" y="630" text-anchor="middle">A gallery's node</text>
  <circle class="fig-hub" cx="500" cy="400" r="78"/>
  <text class="fig-label" x="500" y="396" text-anchor="middle">The commons</text>
  <text class="fig-note" x="500" y="418" text-anchor="middle">one directory</text>
  <rect class="fig-chip fig-pink" x="368" y="388" width="24" height="32"/>
  <rect class="fig-chip fig-paper" x="624" y="388" width="24" height="32"/>
  <rect class="fig-chip fig-paper" x="488" y="248" width="24" height="32"/>
  <rect class="fig-chip fig-paper" x="488" y="520" width="24" height="32"/>
  <text class="fig-tag" x="320" y="446" text-anchor="middle">signed copy</text>
</svg>"##;

/// "Many commons, joined by people": three commons and four nodes. Two
/// nodes share to two commons each; no line runs between commons, because
/// commons never contact each other or a node (ARCHITECTURE.md §1, §5).
pub const TOPOLOGY_SVG: &str = r##"<svg class="fig" viewBox="104 72 800 560" aria-hidden="true">
  <defs><marker id="topology-head" orient="auto" markerWidth="5" markerHeight="5" refX="2" refY="2"><path class="fig-head" d="M0 0 L4 2 L0 4 Z"/></marker></defs>
  <path class="fig-line" d="M360 160 V208 M220 208 H500"/>
  <path class="fig-line" d="M220 208 V265" marker-end="url(#topology-head)"/>
  <path class="fig-line" d="M500 208 V265" marker-end="url(#topology-head)"/>
  <path class="fig-line" d="M780 160 V265" marker-end="url(#topology-head)"/>
  <path class="fig-line" d="M640 544 V496 M500 496 H780"/>
  <path class="fig-line" d="M500 496 V439" marker-end="url(#topology-head)"/>
  <path class="fig-line" d="M780 496 V439" marker-end="url(#topology-head)"/>
  <path class="fig-line" d="M220 544 V439" marker-end="url(#topology-head)"/>
  <rect class="fig-pink" x="266" y="94" width="200" height="72"/>
  <rect class="fig-card" x="260" y="88" width="200" height="72"/>
  <text class="fig-label" x="360" y="120" text-anchor="middle">Your node</text>
  <text class="fig-note" x="360" y="142" text-anchor="middle">on two commons</text>
  <rect class="fig-ink" x="686" y="94" width="200" height="72"/>
  <rect class="fig-card" x="680" y="88" width="200" height="72"/>
  <text class="fig-label" x="780" y="130" text-anchor="middle">A studio's node</text>
  <circle class="fig-hub fig-hub-violet" cx="220" cy="352" r="78"/>
  <text class="fig-label" x="220" y="334" text-anchor="middle">A workshop's</text>
  <text class="fig-label" x="220" y="356" text-anchor="middle">commons</text>
  <text class="fig-note" x="220" y="378" text-anchor="middle">this Wi-Fi only</text>
  <circle class="fig-hub" cx="500" cy="352" r="78"/>
  <text class="fig-label" x="500" y="346" text-anchor="middle">A collective</text>
  <text class="fig-label" x="500" y="368" text-anchor="middle">commons</text>
  <circle class="fig-hub" cx="780" cy="352" r="78"/>
  <text class="fig-label" x="780" y="346" text-anchor="middle">A gallery's</text>
  <text class="fig-label" x="780" y="368" text-anchor="middle">commons</text>
  <rect class="fig-ink" x="126" y="550" width="200" height="72"/>
  <rect class="fig-card" x="120" y="544" width="200" height="72"/>
  <text class="fig-label" x="220" y="586" text-anchor="middle">A student's node</text>
  <rect class="fig-ink" x="518" y="550" width="256" height="72"/>
  <rect class="fig-card" x="512" y="544" width="256" height="72"/>
  <text class="fig-label" x="640" y="576" text-anchor="middle">A collaborator's node</text>
  <text class="fig-note" x="640" y="598" text-anchor="middle">on two commons</text>
  <rect class="fig-chip fig-pink" x="208" y="224" width="24" height="32"/>
  <rect class="fig-chip fig-pink" x="488" y="224" width="24" height="32"/>
</svg>"##;

/// "No sign-up. Someone vouches.": share, wait on the commons' list, a
/// member's vouch or the steward's approval, listed; and the shortcut an
/// invite code gives.
pub const JOIN_SVG: &str = r##"<svg class="fig" viewBox="40 90 920 340" aria-hidden="true">
  <defs><marker id="join-head" orient="auto" markerWidth="5" markerHeight="5" refX="2" refY="2"><path class="fig-head" d="M0 0 L4 2 L0 4 Z"/></marker></defs>
  <path class="fig-line fig-dash" d="M136 260 V128 H856 V253" marker-end="url(#join-head)"/>
  <text class="fig-tag" x="496" y="113" text-anchor="middle">with an invite code</text>
  <path class="fig-line" d="M208 300 H259" marker-end="url(#join-head)"/>
  <path class="fig-line" d="M388 300 H432 M432 232 V368"/>
  <path class="fig-line" d="M432 232 H473" marker-end="url(#join-head)"/>
  <path class="fig-line" d="M432 368 H473" marker-end="url(#join-head)"/>
  <path class="fig-line" d="M680 232 H728 M680 368 H728 M728 232 V368"/>
  <path class="fig-line" d="M728 300 H769" marker-end="url(#join-head)"/>
  <rect class="fig-pink" x="70" y="266" width="144" height="80"/>
  <rect class="fig-card" x="64" y="260" width="144" height="80"/>
  <text class="fig-label" x="136" y="296" text-anchor="middle">Share</text>
  <text class="fig-note" x="136" y="318" text-anchor="middle">a signed copy</text>
  <circle class="fig-hub" cx="328" cy="300" r="60"/>
  <text class="fig-label" x="328" y="296" text-anchor="middle">Waiting</text>
  <text class="fig-note" x="328" y="318" text-anchor="middle">on the list</text>
  <rect class="fig-ink" x="486" y="198" width="200" height="80"/>
  <rect class="fig-card" x="480" y="192" width="200" height="80"/>
  <text class="fig-label" x="580" y="228" text-anchor="middle">A member vouches</text>
  <text class="fig-note" x="580" y="250" text-anchor="middle">one click</text>
  <rect class="fig-blue" x="486" y="334" width="200" height="80"/>
  <rect class="fig-card" x="480" y="328" width="200" height="80"/>
  <text class="fig-label" x="580" y="364" text-anchor="middle">The steward</text>
  <text class="fig-label" x="580" y="386" text-anchor="middle">approves</text>
  <rect class="fig-jade" x="782" y="266" width="160" height="80"/>
  <rect class="fig-card" x="776" y="260" width="160" height="80"/>
  <text class="fig-label" x="856" y="296" text-anchor="middle">Listed</text>
  <text class="fig-note" x="856" y="318" text-anchor="middle">in the directory</text>
</svg>"##;

/// [`JOIN_SVG`] stacked top to bottom for a phone's width.
pub const JOIN_NARROW_SVG: &str = r##"<svg class="fig" viewBox="16 64 360 616" aria-hidden="true">
  <defs><marker id="join-narrow-head" orient="auto" markerWidth="5" markerHeight="5" refX="2" refY="2"><path class="fig-head" d="M0 0 L4 2 L0 4 Z"/></marker></defs>
  <path class="fig-line fig-dash" d="M295 116 H358 V628 H302" marker-end="url(#join-narrow-head)"/>
  <text class="fig-tag" x="306" y="269" text-anchor="middle">invite code</text>
  <path class="fig-line" d="M195 152 V195" marker-end="url(#join-narrow-head)"/>
  <path class="fig-line" d="M195 324 V360 M92 360 H252"/>
  <path class="fig-line" d="M92 360 V401" marker-end="url(#join-narrow-head)"/>
  <path class="fig-line" d="M252 360 V401" marker-end="url(#join-narrow-head)"/>
  <path class="fig-line" d="M92 496 V544 M252 496 V544 M92 544 H252"/>
  <path class="fig-line" d="M195 544 V585" marker-end="url(#join-narrow-head)"/>
  <rect class="fig-pink" x="101" y="86" width="200" height="72"/>
  <rect class="fig-card" x="95" y="80" width="200" height="72"/>
  <text class="fig-label" x="195" y="112" text-anchor="middle">Share</text>
  <text class="fig-note" x="195" y="134" text-anchor="middle">a signed copy</text>
  <circle class="fig-hub" cx="195" cy="264" r="60"/>
  <text class="fig-label" x="195" y="260" text-anchor="middle">Waiting</text>
  <text class="fig-note" x="195" y="282" text-anchor="middle">on the list</text>
  <rect class="fig-ink" x="30" y="414" width="136" height="88"/>
  <rect class="fig-card" x="24" y="408" width="136" height="88"/>
  <text class="fig-label" x="92" y="448" text-anchor="middle">A member</text>
  <text class="fig-label" x="92" y="470" text-anchor="middle">vouches</text>
  <rect class="fig-blue" x="190" y="414" width="136" height="88"/>
  <rect class="fig-card" x="184" y="408" width="136" height="88"/>
  <text class="fig-label" x="252" y="448" text-anchor="middle">The steward</text>
  <text class="fig-label" x="252" y="470" text-anchor="middle">approves</text>
  <rect class="fig-jade" x="101" y="598" width="200" height="72"/>
  <rect class="fig-card" x="95" y="592" width="200" height="72"/>
  <text class="fig-label" x="195" y="624" text-anchor="middle">Listed</text>
  <text class="fig-note" x="195" y="646" text-anchor="middle">in the directory</text>
</svg>"##;

/// "Side by side": one computer running your node, a commons and a local
/// commons; the node shares to the commons like anyone else's would.
pub const SIDE_BY_SIDE_SVG: &str = r##"<svg class="fig" viewBox="48 96 904 312" aria-hidden="true">
  <defs><marker id="side-head" orient="auto" markerWidth="5" markerHeight="5" refX="2" refY="2"><path class="fig-head" d="M0 0 L4 2 L0 4 Z"/></marker></defs>
  <rect class="fig-band" x="64" y="112" width="872" height="280" rx="28"/>
  <text class="fig-band-label" x="96" y="151">This computer</text>
  <path class="fig-line" d="M328 260 H425" marker-end="url(#side-head)"/>
  <rect class="fig-pink" x="102" y="198" width="232" height="136"/>
  <rect class="fig-card" x="96" y="192" width="232" height="136"/>
  <text class="fig-label" x="212" y="256" text-anchor="middle">Your node</text>
  <text class="fig-note" x="212" y="278" text-anchor="middle">your keys, your profile</text>
  <circle class="fig-hub" cx="500" cy="260" r="66"/>
  <text class="fig-label" x="500" y="266" text-anchor="middle">A commons</text>
  <text class="fig-note" x="500" y="355" text-anchor="middle">signed copies only</text>
  <circle class="fig-hub fig-hub-violet" cx="788" cy="260" r="66"/>
  <text class="fig-label" x="788" y="256" text-anchor="middle">Local</text>
  <text class="fig-label" x="788" y="278" text-anchor="middle">commons</text>
  <text class="fig-note" x="788" y="355" text-anchor="middle">anyone on this Wi-Fi</text>
  <rect class="fig-chip fig-pink" x="368" y="244" width="24" height="32"/>
</svg>"##;

/// "Where people reach it": three rings, from your node (this computer) to
/// a local commons (the same Wi-Fi) to an online commons (anyone with its
/// address), with a key beside them.
pub const REACH_SVG: &str = r##"<svg class="fig" viewBox="48 100 904 404" aria-hidden="true">
  <circle class="fig-ring-jade" cx="260" cy="300" r="189"/>
  <circle class="fig-ring-violet" cx="260" cy="300" r="125"/>
  <circle class="fig-ring-pink" cx="260" cy="300" r="61"/>
  <rect class="fig-pink" x="250" y="290" width="24" height="24"/>
  <rect class="fig-card" x="246" y="286" width="24" height="24"/>
  <circle class="fig-ring-pink" cx="554" cy="194" r="11"/>
  <text class="fig-label" x="588" y="192">Your node</text>
  <text class="fig-note" x="588" y="214">this computer only</text>
  <circle class="fig-ring-violet" cx="554" cy="290" r="11"/>
  <text class="fig-label" x="588" y="288">Local commons</text>
  <text class="fig-note" x="588" y="310">anyone on this Wi-Fi,</text>
  <text class="fig-note" x="588" y="332">while the app is open</text>
  <circle class="fig-ring-jade" cx="554" cy="402" r="11"/>
  <text class="fig-label" x="588" y="400">Online commons</text>
  <text class="fig-note" x="588" y="422">anyone with its address:</text>
  <text class="fig-note" x="588" y="444">a server, or Tailscale Funnel</text>
</svg>"##;

/// Coming soon, "Collectives in agreement": four collectives, each with its
/// own members; a dashed jade line is terms two collectives have agreed.
/// A and D share nothing, though both have agreed terms with B.
pub const COLLECTIVES_SVG: &str = r##"<svg class="fig" viewBox="48 216 904 496" aria-hidden="true">
  <defs><marker id="collectives-head" orient="auto" markerWidth="5" markerHeight="5" refX="2" refY="2"><path class="fig-head" d="M0 0 L4 2 L0 4 Z"/></marker></defs>
  <path class="fig-agree" d="M330 300 H670 M750 380 V540 M330 620 H670"/>
  <path class="fig-line" d="M104 268 H170" marker-end="url(#collectives-head)"/>
  <path class="fig-line" d="M104 332 H170" marker-end="url(#collectives-head)"/>
  <path class="fig-line" d="M104 588 H170" marker-end="url(#collectives-head)"/>
  <path class="fig-line" d="M104 652 H170" marker-end="url(#collectives-head)"/>
  <path class="fig-line" d="M896 268 H830" marker-end="url(#collectives-head)"/>
  <path class="fig-line" d="M896 332 H830" marker-end="url(#collectives-head)"/>
  <path class="fig-line" d="M896 588 H830" marker-end="url(#collectives-head)"/>
  <path class="fig-line" d="M896 652 H830" marker-end="url(#collectives-head)"/>
  <rect class="fig-ink" x="68" y="252" width="40" height="40"/><rect class="fig-card" x="64" y="248" width="40" height="40"/>
  <rect class="fig-ink" x="68" y="316" width="40" height="40"/><rect class="fig-card" x="64" y="312" width="40" height="40"/>
  <rect class="fig-ink" x="68" y="572" width="40" height="40"/><rect class="fig-card" x="64" y="568" width="40" height="40"/>
  <rect class="fig-ink" x="68" y="636" width="40" height="40"/><rect class="fig-card" x="64" y="632" width="40" height="40"/>
  <rect class="fig-ink" x="900" y="252" width="40" height="40"/><rect class="fig-card" x="896" y="248" width="40" height="40"/>
  <rect class="fig-ink" x="900" y="316" width="40" height="40"/><rect class="fig-card" x="896" y="312" width="40" height="40"/>
  <rect class="fig-ink" x="900" y="572" width="40" height="40"/><rect class="fig-card" x="896" y="568" width="40" height="40"/>
  <rect class="fig-ink" x="900" y="636" width="40" height="40"/><rect class="fig-card" x="896" y="632" width="40" height="40"/>
  <circle class="fig-hub" cx="250" cy="300" r="80"/>
  <text class="fig-label" x="250" y="306" text-anchor="middle">Collective A</text>
  <circle class="fig-hub" cx="750" cy="300" r="80"/>
  <text class="fig-label" x="750" y="306" text-anchor="middle">Collective B</text>
  <circle class="fig-hub" cx="250" cy="620" r="80"/>
  <text class="fig-label" x="250" y="626" text-anchor="middle">Collective C</text>
  <circle class="fig-hub" cx="750" cy="620" r="80"/>
  <text class="fig-label" x="750" y="626" text-anchor="middle">Collective D</text>
  <rect class="fig-pill" x="432" y="284" width="136" height="32"/>
  <text class="fig-agree-label" x="500" y="305" text-anchor="middle">Terms agreed</text>
  <rect class="fig-pill" x="682" y="444" width="136" height="32"/>
  <text class="fig-agree-label" x="750" y="465" text-anchor="middle">Terms agreed</text>
  <rect class="fig-pill" x="432" y="604" width="136" height="32"/>
  <text class="fig-agree-label" x="500" y="625" text-anchor="middle">Terms agreed</text>
  <text class="fig-note" x="64" y="400">A's members, each</text>
  <text class="fig-note" x="64" y="421">on their own node</text>
</svg>"##;

/// Coming soon, "How two collectives agree terms": a sequence read top to
/// bottom, one column each for A's members, collective A, collective B and
/// B's members. Each member decides for themselves; a withdrawal reaches
/// B the way the listings did. Dashed arrows come later.
pub const AGREE_SVG: &str = r##"<svg class="fig" viewBox="48 100 904 616" aria-hidden="true">
  <defs>
    <marker id="agree-head" orient="auto" markerWidth="5" markerHeight="5" refX="2" refY="2"><path class="fig-head" d="M0 0 L4 2 L0 4 Z"/></marker>
    <marker id="agree-head-jade" orient="auto" markerWidth="5" markerHeight="5" refX="2" refY="2"><path class="fig-head-jade" d="M0 0 L4 2 L0 4 Z"/></marker>
  </defs>
  <path class="fig-lifeline" d="M160 184 V712 M387 184 V712 M613 184 V712 M840 184 V712"/>
  <rect class="fig-ink" x="78" y="118" width="176" height="72"/>
  <rect class="fig-card" x="72" y="112" width="176" height="72"/>
  <text class="fig-label" x="160" y="144" text-anchor="middle">A's members</text>
  <text class="fig-note" x="160" y="166" text-anchor="middle">on their own nodes</text>
  <rect class="fig-blue" x="305" y="118" width="176" height="72"/>
  <rect class="fig-card" x="299" y="112" width="176" height="72"/>
  <text class="fig-label" x="387" y="144" text-anchor="middle">Collective A</text>
  <text class="fig-note" x="387" y="166" text-anchor="middle">its commons</text>
  <rect class="fig-blue" x="531" y="118" width="176" height="72"/>
  <rect class="fig-card" x="525" y="112" width="176" height="72"/>
  <text class="fig-label" x="613" y="144" text-anchor="middle">Collective B</text>
  <text class="fig-note" x="613" y="166" text-anchor="middle">its commons</text>
  <rect class="fig-ink" x="758" y="118" width="176" height="72"/>
  <rect class="fig-card" x="752" y="112" width="176" height="72"/>
  <text class="fig-label" x="840" y="144" text-anchor="middle">B's members</text>
  <text class="fig-note" x="840" y="166" text-anchor="middle">on their own nodes</text>
  <path class="fig-line" d="M393 240 H601" marker-end="url(#agree-head)"/>
  <text class="fig-tag" x="500" y="228" text-anchor="middle">B reads A's listings</text>
  <path class="fig-line" d="M607 304 H399" marker-end="url(#agree-head)"/>
  <text class="fig-tag" x="500" y="292" text-anchor="middle">B proposes terms, signed</text>
  <path class="fig-line" d="M381 368 H172" marker-end="url(#agree-head)"/>
  <text class="fig-tag" x="273" y="356" text-anchor="middle">A asks each member</text>
  <path class="fig-line" d="M166 432 H375" marker-end="url(#agree-head)"/>
  <text class="fig-tag" x="273" y="420" text-anchor="middle">Each says yes or no</text>
  <path class="fig-agree-arrow" d="M393 496 H599" marker-end="url(#agree-head-jade)"/>
  <text class="fig-agree-label" x="500" y="484" text-anchor="middle">Signs for each yes</text>
  <path class="fig-line" d="M619 560 H828" marker-end="url(#agree-head)"/>
  <text class="fig-tag" x="726" y="548" text-anchor="middle">Used only as agreed</text>
  <path class="fig-line fig-dash" d="M166 624 H375" marker-end="url(#agree-head)"/>
  <text class="fig-tag" x="273" y="612" text-anchor="middle">A member withdraws</text>
  <path class="fig-line fig-dash" d="M393 688 H601" marker-end="url(#agree-head)"/>
  <text class="fig-tag" x="500" y="676" text-anchor="middle">Next version leaves it out</text>
</svg>"##;

/// Every diagram, for tests that check them all.
#[cfg(test)]
const ALL: [&str; 9] = [
    HELD_SVG,
    FEDERATE_SVG,
    TOPOLOGY_SVG,
    JOIN_SVG,
    JOIN_NARROW_SVG,
    SIDE_BY_SIDE_SVG,
    REACH_SVG,
    COLLECTIVES_SVG,
    AGREE_SVG,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagrams_take_colours_from_the_stylesheet_not_the_markup() {
        for svg in ALL {
            assert!(svg.contains(r#"aria-hidden="true""#), "hidden from screen readers: the caption says it");
            assert!(!svg.contains("fill=\"#") && !svg.contains("stroke=\"#"), "no colour values in the markup");
            assert!(!svg.contains("style="), "no inline styles");
        }
    }

    #[test]
    fn arrowhead_ids_are_unique_across_diagrams() {
        // The diagrams share a page, and two markers with one id would make
        // every arrow use whichever the browser found first.
        let mut ids: Vec<&str> = ALL
            .iter()
            .flat_map(|svg| svg.split("<marker id=\"").skip(1))
            .map(|rest| rest.split('"').next().unwrap_or(""))
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "a marker id is used twice");
    }

    #[test]
    fn every_arrow_points_at_a_marker_its_own_diagram_defines() {
        for svg in ALL {
            for rest in svg.split("marker-end=\"url(#").skip(1) {
                let id = rest.split(')').next().unwrap_or("");
                assert!(svg.contains(&format!("<marker id=\"{id}\"")), "{id} is not defined in this diagram");
            }
        }
    }
}
