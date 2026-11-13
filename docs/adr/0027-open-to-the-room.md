# 0027. Open to the room

- **Status:** Accepted (built; listing who's in the room deferred; see `docs/ROOM-PLAN.md`)
- **Date:** 2026-11-13
- **Amends:** [0004](0004-node-listens-only-locally.md) (a node may listen on
  the room's network while its owner says so), [0011](0011-joining-needs-a-vouch.md)
  (a room code admits people to a local commons), and
  [0012](0012-block-internal-addresses-on-share.md) (a node open to the room
  counts as personal)
- **Supersedes:** the "no read-only door on every participant's laptop" part
  of [0021](0021-held-not-copied.md)

## Context
In a classroom or a gallery, people want to reach each other's work
directly: everyone's laptop joins the same router, and each person's node is
reachable at the address the router gave it (`http://192.168.1.23:8080/@ana`),
with a QR code on the screen to scan.

Three earlier decisions stand in the way. A node listens only on its own
computer (0004), because it holds the sign-in page, keys and members-only
stories. ADR 0021 rejected opening "a read-only door on every participant's
laptop" as attack surface everywhere. And a node reachable from the network
refuses to share to local-network addresses (0012).

We trust the people in the room **moderately**: we don't expect them to
attack, but a mistake must not expose anyone's keys, passphrase or private
stories. We don't design for hotels or cafés.

## Decision
**D1. A door with an allow-list on a node.** When a node is open to the room
(`HIC_ROOM=1`), every request goes through a door (`commons/src/room.rs`).
The owner, at their own computer, gets everything. Someone in the room gets
only `GET` (or `HEAD`) of public pages: the front page, a profile (`/@ana`),
the story list and one story (`/story/<id>`), How this works and Run a
commons, the stylesheet, scripts and pictures those pages load, `/health`,
`/room/check` and `/room/help`. Everything else,
including `/login`, `/join`, `/restore`, `/steward`, `/api/…`, story tools
and every `POST`, answers 403: "This page of Ana's laptop stays on its owner's computer."
Anything not on the list is refused, so a page added later stays closed to
the room until someone adds it on purpose.

**D2. Never saved as open.** Whether a server is open lives only in the
desktop app's memory. Every launch starts closed.

**D3. It closes by itself.** Close, quitting the app, the app crashing (the
server exits when the app's end of its stdin closes, `HIC_EXIT_WITH_APP=1`),
and any change of network address all close the room.

**D4. A room code for a local commons** (decided by Phoenix). Opening a local
commons to the room makes a fresh code, shown on the host's screen. Typing it
admits someone the way a vouch does, without waiting for steward approval
(0017). Closing the room retires the code. Vouching still works. Real commons
are unchanged.

How it's built: six capitals without I, L and O (about 148 million codes),
from the operating system's random source, passed to the server as
`HIC_ROOM_CODE`. It travels in the share's existing `invite` field, which is
outside the signed bytes, so the signed format doesn't change; the plan's
`X-HIC-Room-Code` header turned out unnecessary. The commons compares it in
constant time and allows five wrong codes per sender address, then a
fifteen-minute wait (as for passphrases, 0005). It's recorded as an admission
with route `room-code`. It never appears in the server's output.

**Update (13 November 2026): a full commons can be opened to the room too**
(Phoenix). It already listens on the network, so opening it adds the door
(the room never reaches the steward view), the room page, and a room code
with a **join link** (`http://172.20.10.10:8180/join/KTRMWX`) that the QR code
holds. Pasted into a node's "Share to a commons" box, the link fills in the
commons and the code. The join page never says whether a code is right, so
it can't be used to guess; the share checks it, with the limits above.
Closing, quitting, a crash or a new address retires the code, as for a local
commons.

**D5. Firewalls: one port, private networks only.** The guide never tells
people to switch a firewall off; one left off stays off at the next café.
The app never changes the firewall. When a server opens, the app reads what
it can without admin rights (macOS's `socketfilterfw`, Windows' network
profile; nothing on Linux) and the room page shows a hint
(`HIC_FIREWALL_HINT`).

**D6. QR codes with the `qrcode` crate** (decided by Phoenix), SVG output
only, default features off.

**D7. One switch per server.** A node and a local commons are opened
separately. The host usually opens both; students open their node.

**D8. A local commons keeps its visitors' pages.** A local commons is already
reachable on the Wi-Fi. Open to the room, its door only adds the owner's
pages to what the room can't reach: `/steward…`, `/room` and every page
under it except `/room/check` and `/room/help`, and `/api/notices/claim`.

**D9. A proxy makes everyone look like the owner.** The door trusts requests
from `127.0.0.1`, which is only true while nothing on the computer forwards
traffic (the lesson of 0023). So a request carrying `X-Forwarded-For`,
`Forwarded`, `X-Real-IP`, `X-Forwarded-Host`, `X-Forwarded-Proto` or `Via`
counts as a room visitor even from `127.0.0.1`,
and the app won't open the room on a port that Tailscale Funnel or `serve`
points at. A request whose sender isn't known also counts as a room visitor.

**A node open to the room may share to the room (amends 0012).** Request
forgery matters when someone can make the server send requests to places they
couldn't reach. On a node open to the room, the door refuses every `POST`
from the room, so only the owner can press Share. `Config::share_policy`
therefore lets such a node reach local-network addresses and this machine
(the room's local commons may be on the same laptop), but still never
link-local ones such as the cloud credentials address: if a forwarder on the
computer hid where visitors come from (D9), that is the address worth
protecting. (The plan first said to set `HIC_ALLOW_PRIVATE_SHARE`; that would
still have refused a commons at `127.0.0.1`.) `HIC_ROOM=1` also makes a node
listen on the network unless `HIC_BIND` says otherwise.

## Why
The switch, the door and the closing rules together change what 0021 feared.
The room can't reach anything on a node that isn't already public on a
commons; it can't sign in, guess a passphrase or post. The door is open only
while its owner chooses, on the network where they chose it, and only while
the app runs.

**Where an open server listens.** Only on the room's address (the
computer's private LAN address, which must be in `10/8`, `172.16/12`,
`192.168/16` or `fc00::/7`), plus `127.0.0.1` for its owner, not on every
network: `0.0.0.0` would also include a VPN's or Tailscale's interface. The
app refuses to open on a `100.64/10` (Tailscale) or public address, which is
what a VPN or exit node makes the computer use.

## Consequences
- Plain `http://` on a shared Wi-Fi can be read by others on that network.
  Public pages are public anyway, and signing in never happens over the room.
- A forwarder that adds none of those headers (nginx with a plain
  `proxy_pass`, `ssh -R`, `socat`, `tailscale serve --tcp`) would still make
  visitors look like the owner, who could then sign in if they knew the
  passphrase. The guide says not to forward ports to the app, and link-local
  addresses stay refused for sharing either way.
- Someone in the room can read every public page of every open node. That's
  the point.
- Tests: `commons/src/room.rs` (each rule), `commons/tests/http/room.rs`
  (through the router), and the room checks in `scripts/test-federation.sh`,
  which reach the node at this computer's own network address.

## Alternatives considered
Bluetooth instead of the room's network (rejected: browsers can't load pages
over it, and it connects about seven devices); a VPN built into the app
(rejected: far more to build and secure); turning the firewall off in the
guide (rejected: D5); a door that lists what's refused instead of what's
allowed (rejected: a new page would be open by default).
