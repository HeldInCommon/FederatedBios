# 0028. Setting up an online commons, without editing files

- **Status:** Accepted (built)
- **Date:** 2026-11-13
- **Builds on:** [0023](0023-steward-needs-a-key.md),
  [0024](0024-steward-link-out-of-the-log.md),
  [0027](0027-open-to-the-room.md)

## Context
There were three ways to run Held in Common: your own node, a local commons
for a room, and a commons for a community over time. The third worked on
the room's Wi-Fi, and on a server with the Docker image. But putting the
app's commons online meant setting up Tailscale Funnel by hand and typing
`public_address` into `app.json`, and first launch showed "Online, through
a tunnel: Coming soon" and "A server I host" greyed out. Only someone who
already knew how could do it.

Phoenix also found the Go menu, in the window's menu bar, easy to miss and
disorienting, especially on Linux.

## Decision
**A commons can be set up online from the app.** First launch's "Where will
people reach this commons?" offers three choices, each saying what to do:

- **This room:** as before.
- **Online, at an address I've set up:** the commons runs in the app; the
  person pastes its public address. The screen gives the exact Tailscale
  command for the commons' port (`tailscale funnel --bg 8080`) and, for a
  domain or fixed IP, which port to forward. Nothing is specific to
  Tailscale: any address that leads to the port works.
- **A server I host:** the commons runs elsewhere, with Docker. The screen
  shows the `docker run` command, with the typed address in `HIC_BASE_URL`,
  and says where the steward link and founding invite appear
  (`docker logs`). The app then sets up the person's own node instead.

**Go → Public address…** sets, checks or clears the address later. **Save**
writes `public_address` to `app.json` and restarts the commons.

**Check it** asks the commons whether the address really leads to it. The
commons makes a random, single-use **probe** and a secret **answer**, fetches
`<address>/health?probe=<probe>`, and the reply must carry the answer, which
only this very server knows (a server elsewhere sees the probe but can't
know the answer). The fetch starts on this computer, so a ✓ proves the
address leads here, not that people elsewhere can reach it; the screen says
to try it on a phone with Wi-Fi off. Only the
desktop app can ask (`POST /api/public-address/check` needs the launch
token). The **launch token is never sent to the address**: a mistyped
address could be anyone's server, and the token opens the steward view
(ADR 0023). A probe is harmless if seen: it's random, single use, and forgotten within
a minute. The check endpoint is also behind the door when the commons is
open to the room (ADR 0027).

(A first version had the reply say `"probe": true`; a review showed any
server could say that, so the answer is now a secret.)

**The Go menu is also in the page.** Every page of this computer's servers
has a "This computer" panel in its sidebar with the same items as the Go
menu: open or close a server to the room, show its QR code, rename, the
steward link, the public address, add a node or a commons, remove the local
commons. One pure list (`src-tauri/src/go_menu.rs`) feeds both. The panel's
links are `hic-app://menu/<id>`. Like every `hic-app://` link, the app
follows them only from its own servers' pages (and not while a new page is
still loading), and only items on the Go menu at that moment. Anything that changes something still asks first in the
setup window, except closing the room, which only makes a server less


## Why
"Make it possible, not hard-coded": an address is an address, whether it's
Tailscale's, a domain behind Caddy, or a fixed IP. The app says what to do
for the common cases and checks the result, so nobody has to know about
`app.json`. Checking from the commons itself proves the address leads to
this server, not just that something answers there.

## Consequences
- The app's commons is reachable online only while the computer is on and
  the app is open. The Docker route is for a commons that's always up.
- Tailscale Funnel must be allowed for the computer in the Tailscale admin
  page; the guide says so.
- A commons the app forwards to through Tailscale can't be opened to the
  room (D9 of ADR 0027 still applies), but it doesn't need to be: people in
  the room can share to its address like anyone else.
- Tests: `only_the_desktop_app_can_ask_a_commons_to_check_its_address`,
  `a_probe_has_an_answer_only_while_it_is_being_checked`, the menu's
  `go_menu.rs` tests, and three checks in `scripts/test-federation.sh`
  (a commons recognises its own address, tells another apart, refuses a
  request without the token, and isn't fooled by a server that claims to be
  it).

## Alternatives considered
Setting up Tailscale Funnel from the app (rejected for now: it needs admin
rights, and ties the app to one provider); checking the address by sending
the launch token (rejected: it could leak to whoever owns a mistyped
address); taking the Go menu out of the macOS menu bar too (rejected: macOS users
look there).
