# Plan: open to the room

**Status:** ready to run. Written 13 November 2026. An agent builds this from
start to finish; see "How to run this plan" below.

**Goal.** In a classroom or a gallery, everyone's laptop joins the same router.
Each person can press **Open to the room**, and then anyone on that router can
reach their node (and the host's local commons) at the address the router gave
their computer, for example `http://192.168.1.23:8080/@tega`. A QR code on each
screen carries that address, so people can scan each other's. Pressing
**Close**, quitting the app, the app crashing, or the laptop moving to another
network all close the room again.

**Who we trust.** People in the same room on a network the host chose: a
classroom router, a gallery's own Wi-Fi, a phone hotspot. Being in the room
counts as **moderate trust**: we expect nobody there to attack, but a mistake
must not expose anyone's keys, passphrase or private stories. We don't design
for hotels or cafés, and the app tells people not to open the room there.

---

## How to run this plan

Give Claude Code this one prompt, from the repository folder:

> Build docs/ROOM-PLAN.md until it's done. Follow "How to run this plan" exactly.

For a long run that keeps going between turns, use the self-paced loop instead:

```
/loop Build docs/ROOM-PLAN.md until it's done. Follow "How to run this plan" exactly.
```

The agent keeps its place in the **Progress** table and the **Run log** at the
end of this file. If a run stops (a context limit, a crash, a closed laptop),
the next run reads those two sections and carries on from the first phase that
isn't **Done**.

### Preflight (once, before Phase 1)

1. `git status --short` must show nothing except this file. If anything else
   has changed, stop and ask Phoenix: that work is theirs.
2. If on `main`, create and switch to `feat/open-to-the-room`. If that branch
   exists already, switch to it.
3. Load the `house-style` skill and read `docs/ARCHITECTURE.md`,
   `docs/CODEMAP.md` and ADRs 0004, 0005, 0011, 0012, 0017, 0021, 0023 and 0024.
4. Build, then run every check (see "Checks") to get a green starting point.
   If something already fails, write it in the Run log as "failing before we
   started" and don't count it against any phase.
5. Check the one-computer trick works here (see "Testing from another computer
   on one computer"): start a node with `HIC_BIND=0.0.0.0` in a temporary
   folder, fetch `/health` from the LAN address, and confirm the request
   arrives from the LAN address, not `127.0.0.1`. Note whether `ufw` or
   `firewalld` is installed (read-only, no sudo). Write both in the Run log.

### How each phase runs

1. Set the phase to **In progress** in the Progress table, and note the
   starting commit.
2. Do the steps. Where the phase lists tests, write them **first**, so each
   rule is a failing test before it's a passing one.
3. Run the phase's own tests and the full "Checks" list. If something fails,
   fix it and run them again, **at most three times**. After that, mark the
   phase **Blocked**, put what failed in the Run log with the output, and go on
   to the next phase that doesn't depend on it (see "Order").
4. **Fresh-eyes review.** Start one subagent (`general-purpose`) and give it
   only the diff for this phase (`git diff <starting commit>`), the phase's
   text from this plan, and ADR 0027. Don't pass on your own view of the
   change. Ask it for correctness and security bugs, each with a file and line.
   For Phases 1 and 6, which decide who gets in, ask it specifically to try to
   reach an owner-only page as a room visitor. Fix what it shows is real, then
   run the checks again.
5. Commit on this branch with only this phase's files (`git add <paths>`,
   never `git add -A`). Write the message in plain words, in the repo's style,
   and end it with the attribution line. **Never push**, and never open a PR
   or make a release.
6. Mark the phase **Done** with its commit in the Progress table, add a Run log
   entry (what was built, what was checked, what the reviewer found, anything
   Phoenix should look at), and start the next phase.

### Subagents

Phoenix has given permission to start subagents and to run tests. Use them
where the work really is separate:

- **A reviewer** at the end of every phase, as above.
- **The teacher's guide** (Phase 5, step 1) is writing, not code. A subagent
  can draft it while Phases 2 to 4 are built.
- **Phase 2 doesn't depend on Phase 1.** If Phase 1 is blocked, Phase 2 can go
  ahead.

Never run two subagents that change code at the same time on this branch;
they'd overwrite each other's edits. Keep to about ten subagents for the whole
run.

### When to stop and ask Phoenix

Only when one of these happens. Otherwise keep going.

- Preflight finds changes that aren't this plan's.
- A step needs a decision that isn't in this plan (D1 to D9), and the choice
  would be expensive to undo or would change who can reach what.
- A new dependency other than `qrcode` looks necessary.
- A step would change signed bytes or the spec (ADR 0009).
- Everything left is **Blocked** or **Needs Phoenix**.

### Never

- Push, open a PR, merge or make a release.
- Use `sudo`, change any firewall, or turn Tailscale Funnel or `serve` on or off.
- Edit an accepted ADR. ADR 0027 amends or supersedes what it needs to.
- Add `prefers-reduced-motion` rules, 1px or 1.5px lines, animated transforms
  or `will-change` on hover, or meaning carried by colour alone.
- Touch the installed app's data in `~/.local/share/org.heldincommon.node/`.
  Test servers use the scratchpad or `mktemp -d` folders, and are stopped
  afterwards.
- Mark a phase **Done** with failing checks, or skip a check without writing in
  the Run log that it was skipped and why.

### Checks

Run all of these at the end of every phase:

```bash
cargo test --manifest-path commons/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path commons/Cargo.toml -- -D warnings
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
python3 scripts/check-docs.py
python3 scripts/check_spec_vectors.py
cargo build --release --manifest-path commons/Cargo.toml && ./scripts/test-federation.sh
./scripts/lint-css.sh        # whenever CSS changed
```

### Testing "from another computer" on one computer

The agent has no second device, so the tests use a trick. When this machine
connects to **its own LAN address** (for example
`curl http://192.168.1.40:8090/`), the server sees the request come from
`192.168.1.40`, not from `127.0.0.1`. To the door, that's a room visitor:

- **Owner:** `curl http://127.0.0.1:<port>/…`
- **Room visitor:** `curl http://$(hostname -I | awk '{print $1}'):<port>/…`

If the machine has no LAN address, the `test-federation.sh` checks that need
one print `SKIP` with the reason instead of failing. The HTTP tests in
`commons/tests/` don't need a network at all: they set the peer address
directly with `ConnectInfo`.

What really needs other devices (phones, a Mac, Windows, real firewalls,
client isolation) is listed in **Needs Phoenix**. The agent never ticks those.

---

## Words used in this plan

- **Listening address (bind).** Which network door a server accepts
  connections on. `127.0.0.1` (*loopback*) means "only this computer";
  `0.0.0.0` means "every network this computer is on".
- **LAN address.** The address the router gives a computer on its network
  (`192.168.x.x`, `10.x.x.x`, `172.16–31.x.x`). It's handed out by *DHCP*, so it
  can change when the laptop reconnects.
- **Peer address.** The address a request came from. The door uses it to tell
  the owner (`127.0.0.1`) from a room visitor.
- **Client isolation.** A router setting that stops devices on the same Wi-Fi
  from talking to each other. Common on guest, school and university networks.
  If it's on, nothing in the app can get around it.
- **Firewall.** Software on each computer that decides which incoming
  connections are let in. Every operating system has one.

---

## What's in the way today

We found these in the code. Each one has a step below.

1. **A node only listens on its own computer.** That's ADR 0004
   (`commons/src/config.rs`, `bind_addr`), and it's deliberate: the node also
   holds the sign-in page, keys and members-only stories.
2. **ADR 0021 already said no to the room opening every laptop:** "Opening a
   read-only door on every participant's laptop would add attack surface
   everywhere." This plan reverses that, so ADR 0027 explains why the switch,
   the door and the closing rules make it acceptable.
3. **Once a node is open to the room, it can't share to a local commons.**
   With a non-loopback bind, `outbound.rs` treats the node as "reachable from
   the network" and refuses `192.168.x` addresses unless
   `HIC_ALLOW_PRIVATE_SHARE=1` (ADR 0012).
4. **A local commons reopens to the Wi-Fi on every launch.** `app.json` saves
   `"practice": true`, and the app starts every saved server at launch
   (`saved.running()` in `src-tauri/src/lib.rs`). Nobody chose to open it that
   day.
5. **A crash leaves the server open.** Quitting stops the servers (the
   `RunEvent::Exit` hook in `lib.rs`), but a crash or a force-quit skips that
   hook, and the `hic-commons` process keeps listening.
6. **There are no QR codes,** and no page that says "this is your address in
   the room".
7. **Firewalls and client isolation** block people, and the app says nothing
   to help.
8. **macOS Local Network permission (unverified).** Since macOS 15, an app
   needs permission to *connect to* devices on the local network. We have no
   `NSLocalNetworkUsageDescription` in the bundle. Only a Mac can show whether
   this matters (Needs Phoenix).
9. **A proxy makes everyone look like the owner.** The door trusts requests
   from `127.0.0.1`. Anything on the same computer that forwards traffic, such
   as Tailscale Funnel or `tailscale serve`, makes outside visitors arrive from
   `127.0.0.1`. ADR 0023 learned this the hard way for the steward view.

---

## Decisions

These go into **ADR 0027, "Open to the room"**. It supersedes the "no
read-only door" part of 0021 and amends 0004 and 0011. **Decided** rows were
agreed by Phoenix on 13 November 2026. The other rows are this plan's answers:
Phoenix can change them before a run starts, and the agent treats them as
settled.

| # | Question | Answer |
|---|---|---|
| D1 | What can a visitor from the room reach on a **node**? | Only what's already public: profile pages, public stories, the stylesheet, scripts and images, `/health` and `/room/check`. Everything else answers "This page is only on Tega's own computer." The list says what's allowed in; anything not on it is refused. |
| D2 | Is the room ever saved as open? | No. It lives in the app's memory only, and every launch starts closed. |
| D3 | What closes it? | Close, quit, crash, and any change of network address. "Open until…" is optional (Phase 3, last step). |
| D4 | How do people join the host's local commons? | **Decided:** with a room code shown on the host's screen. It changes each time the room opens, and closing the room retires it. Vouching still works too. This applies to local commons only. Real commons keep ADR 0011 unchanged. |
| D5 | Firewalls: turn them off, or open one port? | Open one port, on private networks only. Never tell people to switch the firewall off. A firewall left off stays off at the next café. |
| D6 | New dependency for QR codes? | **Decided:** `qrcode` crate, SVG output only, default features off. Phoenix approved the dependency. |
| D7 | Can someone open the room on a node and a local commons separately? | Yes, as one switch per server. The host usually opens both, and students open their node only. |
| D8 | What can a room visitor reach on a **local commons**? | Everything a commons visitor can reach today, except the owner-only pages: `/steward*`, `/room` and `/api/notices/claim`. A local commons is already reachable on the Wi-Fi; this puts a door in front of it and takes nothing away from visitors. |
| D9 | What if something forwards traffic to the server (gap 9)? | The app won't open the room on a port that Tailscale Funnel or `serve` points at, and says why. A server with `HIC_ROOM=1` also treats a request from `127.0.0.1` as a room visitor if it carries `X-Forwarded-For` or `Forwarded`: a proxy adds these headers, and the owner's browser never does. |

---

## How it works, in one picture

```
                      classroom router (192.168.1.x)
        ┌───────────────────────┼────────────────────────┐
  Teacher's laptop        Student A laptop          Student B phone
  local commons :8082     node :8080                 (browser only)
  node :8080              OPEN → 0.0.0.0             scans A's QR
  OPEN → 0.0.0.0          door: public pages only    → http://192.168.1.23:8080/@ana
  QR + room code          QR on screen
```

Each server has three states. **Closed** listens on `127.0.0.1` and is exactly
today's node. **Open** listens on `0.0.0.0` with the door in front of it.
**Gone** means the app quit or crashed and the server stopped with it.

---

## Order

```
Preflight → 1 ─┐
            2 ─┴→ 3 → 4 → 6 → 7
                       └→ 5 (help page; the guide can be drafted any time)
```

- Phases 1 and 2 don't depend on each other. Phase 3 needs both.
- Phase 4 needs Phase 3, which provides the "open" state and the LAN address.
- Phase 5's guide can be drafted any time. Its help page is linked from
  `/room`, so it needs Phase 4.
- Phase 6 needs Phases 3 and 4, and Phase 7 needs Phase 6.
- Phase 8 is Phoenix's rehearsal with real devices.

Each phase ends with something that works and has been tested, so the work can
stop after any one of them.

---

## The phases

### Phase 1: the door on the server

The server learns to be open to the room safely. The app doesn't change yet.

1. Write **ADR 0027** with D1 to D9, and add it to `docs/adr/README.md`.
2. New config `HIC_ROOM=1` in `commons/src/config.rs`: "this server is open to
   the room."
3. New module `commons/src/room.rs` with a **pure function**:
   `door(role, method, path, peer_ip, forwarded: bool) -> Door::{In, OnlyHere}`.
   - **Owner:** peer `127.0.0.1` or `::1`, not forwarded → `In`, on any server.
   - **Node, room visitor:** `In` only for `GET` of `/`, `/@{handle}`,
     `/stories/{kid}/{id}`, `/style.css`, `/logo.png`, `/quilt/*`, `/avatar.js`,
     `/health` and `/room/check`. Everything else gets `OnlyHere`, including
     `/login`, `/join`, `/restore`, `/steward`, `/api/*`, `/story/*`, `/backup`
     and every `POST`.
   - **Local commons, room visitor:** `In` except for `/steward*`, `/room` and
     `/api/notices/claim` (D8).
   - **Forwarded** (D9): a request from `127.0.0.1` that carries
     `X-Forwarded-For` or `Forwarded` is a room visitor.
4. A middleware on `router` in `commons/src/app.rs` runs only when
   `HIC_ROOM=1`. It reads the peer address, which `main.rs` already provides
   through `into_make_service_with_connect_info`, asks `door`, and answers
   `OnlyHere` with a plain page and status 403: "This page is only on *name*'s
   own computer." The page follows the house style.
5. A node with `HIC_ROOM=1` also gets `allow_private_share = true`, so it can
   still share to the local commons (gap 3).
6. Check that public pages never list members-only stories to a room visitor,
   since a room visitor is never signed in.

**Tests (write these first):**
- Unit tests in `room.rs`, one per rule, named as sentences:
  `a_room_visitor_cannot_reach_the_sign_in_page`,
  `the_owner_reaches_everything`, `an_unknown_path_is_refused_by_default`,
  `a_post_from_the_room_is_refused`, `a_forwarded_request_is_not_the_owner`,
  `a_room_visitor_cannot_open_the_steward_view_of_a_local_commons`.
- HTTP tests in a new `commons/tests/http/room.rs`, using
  `ConnectInfo("192.168.1.50:5000")` for a room visitor and `127.0.0.1` for the
  owner. They check that the profile is visible, sign-in is refused, a
  members-only story stays invisible, sharing to a `192.168.x` commons is
  allowed, and that without `HIC_ROOM` nothing changes.
- In `test-federation.sh`, start a node with `HIC_BIND=0.0.0.0 HIC_ROOM=1`
  and curl it at the LAN address: `/@tega` answers 200 and `/login` answers
  403. The same node at `127.0.0.1` answers 200 for `/login`. Print `SKIP` if
  there's no LAN address.

### Phase 2: the server stops when the app does

This closes gap 5, crashes and force-quits.

1. With `HIC_EXIT_WITH_APP=1`, the server reads stdin on a background thread
   and **exits when it reaches the end** (EOF). This needs no new dependency.
   Without the variable (Docker, people running it by hand) nothing changes,
   because Docker often has no stdin and would otherwise exit at once.
2. The app sets `HIC_EXIT_WITH_APP=1` and keeps the sidecar's stdin open
   (`sidecar.rs`). When the app dies for any reason, the operating system
   closes that pipe. Check that the Tauri shell plugin's `CommandChild` really
   keeps stdin open, and write in the Run log how you checked.

**Tests:**
- A test in `commons/tests/` that spawns the binary with a piped stdin, checks
  `/health` answers, closes the pipe, and checks the process exits within 2
  seconds and the port is free.
- The same with the variable unset: closing stdin does **not** stop it.

### Phase 3: the switch in the app

1. Add a `Room` state in `src-tauri`: which servers are open right now. It's
   held in memory and **never written to `app.json`** (D2).
2. `server_env` in `sidecar.rs` gets an `open: bool` input:
   - closed → `HIC_BIND=127.0.0.1`, `HIC_BASE_URL=http://127.0.0.1:<port>`
   - open → `HIC_BIND=0.0.0.0`, `HIC_BASE_URL=<lan url>`, `HIC_ROOM=1`, and
     for a node also `HIC_ALLOW_PRIVATE_SHARE=1`
   - A **local commons now starts closed** (gap 4). It is no longer opened
     just because a network exists. A full commons (`Kind::Commons`) is
     unchanged.
3. Commands `open_room(kind)` and `close_room(kind)` stop the server, wait for
   the port to free up (`wait_until_free`), and start it again with the new
   environment, the same way `watch_addresses` already restarts servers.
4. If the computer has **no LAN address**, "Open to the room" says "This
   computer isn't on a network" and stays closed.
5. D9: before opening, check whether `tailscale serve status` or
   `tailscale funnel status` points at this port. The check is read-only, and
   is skipped if `tailscale` isn't installed. If something points at the port,
   refuse and say why.
6. `watch_addresses`: if an **open** server's address changes, **close it**
   instead of moving it, and notify: "The room closed because this computer
   changed network. Open it again if you're still in the room." (D3)
7. Menu items: "Open my node to the room" / "Close my node to the room", and
   the same for the local commons. Before opening, the app asks for
   confirmation: "Only open this on a network you trust: a classroom, a
   studio, your own hotspot."
8. The rail shows the state in words, as "Open to the room" with an oxblood
   pop, never colour alone. Follow the colour-blind-safe palette, use no thin
   lines, and use no animated hover.
9. Optional, only once everything above is done and passing: "Open until…"
   (an hour, the end of the day), with the app closing the room at that time.

**Tests:**
- Pure tests for `server_env`: closed node, open node, closed and open local
  commons, open with no LAN address, and a full commons unchanged.
- A pure test for "address changed while open → close" (split out of
  `watch_addresses`, as `moved_to` already is).
- A pure test for reading `tailscale serve status` output, with sample outputs
  saved as fixtures.
- A test that `Saved` round-trips through `app.json` with **no** room field.
- If the `run` skill can start the app here (with
  `WEBKIT_DISABLE_DMABUF_RENDERER=1`): open the node to the room, curl it from
  the LAN address, close it, and check the curl now fails. Write the result in
  the Run log. If the app won't *launch* on this machine, that isn't a phase
  failure.

### Phase 4: the address and its QR code

1. Add the `qrcode` crate to `commons/Cargo.toml`, with default features off
   and only SVG output (D6, approved). Say why in the commit message.
2. New page `/room`, owner only (the door refuses it to visitors). It shows a
   large QR code, the address in big mono text ("People in this room can open:
   `http://192.168.1.23:8080/@ana`"), the server's state, and the Close button.
   It's laid out to work on a projector.
   - On a node, the QR leads to the member's profile.
   - On a local commons, it leads to the commons' front page, with the room
     code next to it (once Phase 6 is built).
3. New page `/room/check`, open to room visitors. It says "It works: you
   reached Ana's laptop", so people can test the connection from a phone
   without anything else needing to work.
4. The app opens `/room` when someone presses "Open to the room".

**Tests:**
- HTTP tests: `/room` from the owner shows the LAN address and an `<svg>`;
  `/room` from a room visitor gets `OnlyHere`; `/room/check` works for both.
- A unit test that the QR encodes exactly `HIC_BASE_URL` plus the path: decode
  it back if the crate allows, otherwise compare against a known-good matrix.
- Accessibility: the address is real text beside the QR, never only inside the
  image, and the QR has an `aria-label` with the address.

### Phase 5: firewall and network help

1. **A teacher's guide,** `docs/ROOM-GUIDE.md`, in plain words. A subagent can
   draft it early. It covers what to bring (a travel router or a phone hotspot
   is the surest thing), how to open the room, and what to do when someone
   can't connect. Every step not yet tried on that OS says "(not yet tried on
   a real Mac)" or "(… Windows machine)" until Phoenix confirms it.
2. **A help page in the app,** "Can't reach someone?", linked from `/room`. It
   walks through these checks in order:
   1. Are you both on the same Wi-Fi? (Compare the first three numbers of the
      address.)
   2. Does `/room/check` open on your own phone? If not, the firewall on that
      laptop is blocking it.
   3. If no device can reach any other, the network has client isolation.
      Switch to the hotspot or travel router.
3. **Firewall instructions,** opening **one port on private networks only**
   (D5):
   - **macOS.** When asked "Do you want the application hic-commons to accept
     incoming network connections?", choose **Allow**. If someone clicked Deny:
     System Settings → Network → Firewall → Options → find `hic-commons` → set
     it to "Allow incoming connections". Signed builds (ADR 0015) remember the
     answer; unsigned ones ask at every launch. If the Local Network prompt
     turns out to matter (Needs Phoenix), also cover System Settings → Privacy
     & Security → Local Network → Held in Common → on.
   - **Windows.** In the Defender dialog, tick **Private networks** and choose
     Allow. Windows marks a newly joined Wi-Fi as *Public*, and Public blocks
     this, so also go to Settings → Network & internet → Wi-Fi → the network →
     Network profile type → **Private**.
   - **Linux (ufw).** `sudo ufw allow from 192.168.0.0/16 to any port 8080:8082 proto tcp`,
     and `sudo ufw delete allow …` afterwards. On firewalld:
     `sudo firewall-cmd --zone=home --add-port=8080-8082/tcp` (without
     `--permanent`, so it's gone after a reboot).
4. **Hints on `/room`,** where the app can read the state without admin rights:
   - macOS: `/usr/libexec/ApplicationFirewall/socketfilterfw --getglobalstate`
   - Windows: the network profile, via `Get-NetConnectionProfile`
   - Linux: skip, since `ufw status` needs sudo

   The app never changes the firewall.

**Tests:**
- Unit tests for reading each hint: pure functions over sample outputs of each
  command, saved as test fixtures.
- An HTTP test that `/room` links to the help page and that the help page
  renders.
- A subagent proofreads the guide against the code. Proofreading on real
  machines is in Needs Phoenix.

### Phase 6: joining the local commons with a room code

1. ADR 0027 already records it: on a local commons that's **open to the
   room**, a room code admits someone the way a vouch does, without waiting
   for steward approval (ADR 0017).
2. Opening the room makes a code: 6 letters, without look-alikes such as 0/O
   and 1/l, from the operating system's random source (`OsRng`, as
   `launch_token` uses). The app passes it to the server as `HIC_ROOM_CODE`.
   It's shown on `/room`, and it's added to the log redaction (ADR 0024) so it
   never appears in the log.
3. The commons reports that it takes a room code, wherever the share status is
   already reported. "Share to this commons" on a node then asks for the code
   and sends it as an HTTP header (`X-HIC-Room-Code`) along with the share.
   The code is **not** part of the signed message, so no signed bytes change.
   A matching code lists the member straight away.
4. Wrong codes are limited the same way passphrase guesses are (ADR 0005):
   5 tries, then a wait, per peer address.
5. Closing the room retires the code. Members already in stay in. The host can
   remove someone from the steward view.

**Tests:**
- HTTP tests: the right code lists you, a wrong code doesn't, the sixth wrong
  try waits, an old code fails after a close and reopen, and a real commons
  (not local) ignores `HIC_ROOM_CODE` and the header completely.
- A test that the code never appears in log output (passed through
  `redact_for_log`).
- `python3 scripts/check_spec_vectors.py` and `cargo test --test spec_vectors`
  still pass unchanged, which shows the signed format didn't change.
- `test-federation.sh`: a node shares to a local commons with the right code
  and is listed, and with a wrong code isn't.

### Phase 7: who's in the room (only once 1 to 6 are Done)

The local commons page lists the members whose nodes are open right now, each
with a link and QR to their node. Each open node tells the commons its room
address in its existing `here` long-poll (ADR 0021), as an unsigned hint that
the commons only shows to people in the room. It changes **no signed bytes**.
If that turns out not to work, mark the phase **Deferred** with the reason.
Don't design a new signed type without asking.

**Tests:** an HTTP test that an open node's address appears on the local
commons page, and disappears within the `here` timeout after its poll stops.

### Phase 8: a full rehearsal

For Phoenix, with real devices. Not for the agent. The checklist is in Needs
Phoenix.

---

## What this does not protect against

- **Someone in the room who means harm** can read every public page of every
  open node. That's the point of opening it, and it's no more than they'd see
  on the commons. They can't sign in or post anything.
- **The connection isn't encrypted.** Plain `http://` on a shared Wi-Fi can be
  read by others on that network. Public pages are public anyway; signing in
  and keys never go over the room, because those pages only open on the
  owner's own computer. HTTPS on a LAN needs certificates every device trusts,
  which is out of scope.
- **A host who opens the room in a café.** The switch warns them, but we can't
  tell a café from a classroom.
- **A forwarder we can't see.** D9 catches Tailscale and standard proxy
  headers. A homemade forwarder that strips those headers would still make
  visitors look like the owner. The guide says not to forward ports to the app.

---

## Progress

The agent updates this table as it goes. Statuses: **Not started**, **In
progress**, **Done**, **Blocked** (reason in the Run log), **Deferred**,
**Needs Phoenix**.

| Phase | Status | Commit |
|---|---|---|
| Preflight | Done | (no commit) |
| 1 The door | Done | `a450fa5` |
| 2 Stop with the app | Done | `60213dd`, fix in the next commit |
| 3 The switch | Done | `ae33c42` |
| 4 Address and QR | Done | `ae33c42` |
| 5 Firewall and network help | Done | (see Run log) |
| 6 Room code | Done | (see Run log) |
| 7 Who's in the room | Deferred | (see Run log) |
| 8 Rehearsal | Needs Phoenix | |

**The plan is done** when Phases 1 to 6 are Done, Phase 7 is Done or Deferred
with a reason, every check passes on the branch, and Needs Phoenix is up to
date. The agent then stops and writes a summary at the top of the Run log:
what was built, what to try first, and anything it was unsure of.

---

## Needs Phoenix

Things only a person with other devices can do. The agent adds to this list
and never ticks these boxes itself.

- [ ] From a phone on the same router, open a node's `/room/check` and profile,
      and check that `/login` is refused.
- [ ] On a Mac with the installed app: the firewall prompt's wording, and
      whether sharing to a local commons on another computer brings up the
      Local Network prompt (gap 8). Update `docs/ROOM-GUIDE.md`.
- [ ] On Windows: the Defender dialog, and whether a Public network blocks it
      as expected.
- [ ] On a phone hotspot and, if possible, a school or guest network: see
      client isolation happen.
- [ ] Scan the QR codes with an iPhone and an Android phone.
- [ ] On a Mac with the firewall on, and on Windows on a Public network: open
      to the room and check the room page shows the firewall hint, and that
      following it works. Then remove "(not yet tried …)" from the guide.
- [ ] On Windows: no console window flashes when opening to the room.
- [ ] Join a local commons with the room code from a second computer, typing
      it with a space as shown on screen.
- [ ] Force-quit the app (`kill -9`) with the room open, and check nothing
      still listens: `ss -ltnp | grep 808` (Linux) or
      `lsof -iTCP -sTCP:LISTEN | grep 808` (Mac).
- [ ] Switch Wi-Fi with the room open: it closes, with the notice.
- [ ] **Rehearsal** with this Linux ARM machine, a Mac, a Windows laptop if
      there is one, and two phones, all on a travel router or hotspot:
  - [ ] The host opens a local commons and shows `/room` on the big screen.
  - [ ] Each laptop opens its node to the room, and phones scan the QR codes.
  - [ ] A phone opens someone's profile and a public story.
  - [ ] A phone tries `/login` on someone's node and is turned away.
  - [ ] A node shares to the local commons with the room code.
  - [ ] The host closes the room, and phones can no longer reach it.
  - [ ] One laptop force-quits, and its address stops answering at once.
  - [ ] One laptop changes Wi-Fi, and its room closes with the notice.
  - [ ] Next day, every app opens closed.
  - [ ] Write up what went wrong in `docs/ROOM-GUIDE.md`.
- [ ] Run the app built from this branch (`cargo tauri dev` or a build) and
      try the Go menu: open your node to the room (the confirm screen, then
      the room page with its QR code), "Show the QR code for my node", Close.
      Quit with it open and relaunch: it starts closed. Start a local commons
      and check it says "closed to the room" until you open it.
- [ ] With Tailscale serve or Funnel on a port, try to open that server: the
      app should refuse and say why.
- [ ] Read the branch, then push it and open the PR when happy.

---

## Run log

The agent adds an entry per phase, newest at the bottom: the date, the phase,
what was built, which checks ran (and any skipped, with why), what the reviewer
found, and the commit.

### 2026-11-13, preflight

- Working tree held only this plan. Phoenix's branch had been merged (#72),
  so we were on `main`; created `feat/open-to-the-room` from `cb15782`.
- Baseline, all green: 262 unit and 90 HTTP tests (`commons`), 44 tests
  (`src-tauri`), 93 end-to-end checks in `test-federation.sh`,
  `check-docs.py`, spec vectors 10/10.
- One-computer trick works here: a node on `HIC_BIND=0.0.0.0`, fetched at this
  machine's own address (`10.106.66.28`), saw the connection come from
  `10.106.66.28`, not `127.0.0.1`.
- Firewall: `ufw` is installed (`/usr/sbin/ufw`); its state needs sudo to read,
  so it wasn't checked. No `firewalld`.

### 2026-11-13, Phase 1 (the door), notes while building

- **Deviation from the plan, step 5.** Setting `HIC_ALLOW_PRIVATE_SHARE` on an
  open node would still refuse a commons at `127.0.0.1` (the host's own local
  commons on the same laptop). Instead `Config::is_personal` counts a node with
  `HIC_ROOM=1` as personal, because the door refuses every `POST` from the
  room, so only the owner can press Share. Recorded in ADR 0027.
- **Deviation, D1 paths.** A node's public story lives at `/story/<id>`, not
  `/stories/<kid>/<id>` (that's a commons page), so the door allows
  `/story/<id>` (one segment only; `/story/<id>/edit` stays closed) and
  `/stories` (the list).
- A request whose sender address is unknown counts as a room visitor. The door
  also treats `X-Real-IP` as a proxy header, besides the two in D9.

### 2026-11-13, Phase 1 (the door): done

- **Built:** `commons/src/room.rs` (the door, pure, 14 unit tests), the
  middleware on `router` when `HIC_ROOM=1`, the "only on this computer" page
  (403), `HIC_ROOM` in `Config`, `Config::share_policy`, ADR 0027, and the
  docs (ADR index, `ARCHITECTURE.md` settings, `CODEMAP.md`).
- **Tests:** 8 HTTP tests in `commons/tests/http/room.rs`; 5 new checks in
  `test-federation.sh`, reaching an open node at this computer's own address.
- **Reviewer (fresh subagent, attacked it as a room visitor):** found no way
  past the door (it tried percent-encoding, `//`, trailing `/`, case,
  `HEAD`, `PUT`/`DELETE`/`OPTIONS`, `?tab=account`, the 404 fallback). It
  found, and these were fixed:
  1. `/password.js` (loaded by every page) and `/run-a-commons` (footer link)
     were refused to the room: now allowed.
  2. Treating an open node as fully "personal" also allowed link-local
     addresses such as `169.254.169.254`, which matters if a forwarder hides
     where visitors come from. Now `Policy::in_room` allows the room's
     network and this machine but never link-local; tested.
  3. `HIC_ROOM=1` alone didn't make a node listen on the network: now it
     does (unless `HIC_BIND` says otherwise).
  4. The share test only tried `127.0.0.1`: it now also tries `192.168.x`
     (allowed) and `169.254.169.254` (refused).
  Also taken from its suggestions: `X-Forwarded-Host`, `X-Forwarded-Proto`
  and `Via` count as proxy headers, and on a local commons every page under
  `/room/` is owner-only except `/room/check` and `/room/help`.
- **Checks:** all green except `commons/tests/exit_with_app.rs`, Phase 2's
  test written first, which fails until Phase 2 is built (not in this commit).
- **For Phoenix:** a forwarder that adds no proxy header (nginx `proxy_pass`
  without headers, `ssh -R`, `socat`, `tailscale serve --tcp`) would make
  outside visitors look like the owner. ADR 0027 records this; the app's
  Tailscale check (Phase 3) and the guide cover the common cases.

### 2026-11-13, Phase 2 (stop with the app): done

- **Built:** `HIC_EXIT_WITH_APP=1` (in `Config`), which the app now sets for
  every server. The server reads stdin on a thread and exits when it ends.
- **How the stdin pipe was checked:** read `tauri-plugin-shell` 2.5.0's
  `Command::spawn`. It pipes stdin, and the writing end lives in
  `CommandChild`, which the app keeps in `Sidecar::child` until it stops the
  server. The reviewer confirmed the pipe is close-on-exec (os_pipe uses
  `pipe2(O_CLOEXEC)` on Linux), so other children don't keep it open.
- **Order:** committed before the review (`60213dd`) so Phase 3 could change
  `sidecar.rs` cleanly; the review's fix is a separate commit.
- **Reviewer found a critical bug, now fixed:** when the app really dies, its
  ends of the server's stdout and stderr close too. The goodbye `println!`
  then panicked on the broken pipe, which stopped only that thread, so the
  server **kept running**. My test and my first `kill -9` check had sent the
  server's output to `/dev/null` and missed it. Now nothing is printed on the
  way out; an interrupted read is retried. The test now closes all three
  pipes (it fails on the old code and passes on the fix), waits for this
  server's own `/health` reply rather than any listener, kills the server and
  deletes its folder even when an assert fails.
- **Checked by hand:** a parent process started the server with all three
  pipes, as the app does, and was killed with `kill -9`. The server was gone
  within 1.5 seconds.
- **Not changed (reviewer, unverified):** on macOS the pipe is made
  close-on-exec in two steps, so a sidecar started at that exact moment could
  inherit it; the server would then stop when that sidecar stops. Low risk.
- **Checks:** all green for `commons` (275 unit, 97 HTTP, 2 exit tests, 98
  end-to-end).

### 2026-11-13, Phases 3 (the switch) and 4 (address and QR): done, one commit

- **Why one commit:** Phase 3's review fixes and Phase 4 both change
  `config.rs`, the layout and the room tests; split, the commit between them
  wouldn't build.
- **Phase 3 built:** an in-memory `open` flag per server (never in
  `app.json`), `set_room` and `open_room_now` (only from the setup window,
  run off the main thread), Go-menu items "Open my node to the room…" /
  "Close … to the room" / "Show the QR code for …", a confirmation screen,
  the read-only Tailscale check, `watch_addresses` closing an open server
  when the address changes, crash handling, and the switcher and menu
  wording. A local commons now starts closed ("Start a local commons").
- **Phase 4 built:** `/room` (owner only: QR code as inline SVG, the address
  as text), `/room/check` and `/room/help` (anyone in the room), the
  `qrcode` crate (SVG only), the rail saying "open to the room" in words with
  an oxblood "Open to the room" pop, and the app opening `/room` after
  opening.
- **Two reviews.** The first (Phase 3 only) found, all fixed:
  - a removed local commons came back open with no confirmation
    (`Sidecar::stop` now clears the flag);
  - a crash restart racing an Open could leave an untracked server on the
    network (a per-server `changing` lock, the restart re-checks after its
    pause, and `set` kills any server it replaces);
  - a crash without restart, or a failed start, left the flag open (cleared);
  - the confirm command froze the window (now `async`), and could be reached
    from the main window (now only from the setup window);
  - `0.0.0.0` meant every network, including Tailscale's: an open server now
    listens only on the room's address, plus `127.0.0.1` for its owner
    (`Config::listen_addrs`, a second listener in `main.rs`);
  - the Tailscale check failed open (now: no answer means refuse; any spelling
    of loopback counts);
  - "closes when this computer changes network" overpromised: it's when the
    address changes, checked every minute; wording fixed everywhere.
  The second review (fixes plus Phase 4) confirmed those, and found, all
  fixed: room pages served on a full commons (now 404 there); the rail
  depended on `HIC_ROOM_ADDRESS` (now on `HIC_ROOM`); `restart_node` didn't
  refresh the menu; no oxblood pop; a VPN or Tailscale exit node would make
  the room address a `100.x` or public one (now refused, "not a room
  network"); a failing `tailscale` command counted as "nothing forwarded"
  (now "couldn't check", unless Tailscale simply isn't running);
  `remove_practice`, `restart_node` and `add_role` now take the lock; the QR
  code leads to the profile only when the room can read it.
- **Also caught while building:** Phase 1's fix allowing `/password.js` and
  `/run-a-commons` had silently not applied (`rustfmt` had reflowed the line
  my edit looked for). Applied now, with a test listing them.
- **Not done, on purpose:** `/room` has no Close button; closing is an app
  action, so the page points to the Go menu. The app isn't launched here (it
  would use the installed app's data and ports); see Needs Phoenix.
- **Checks:** all green: 286 unit, 102 HTTP, 4 spec, 2 exit tests (`commons`);
  60 (`src-tauri`); 101 end-to-end; docs, spec vectors, CSS lint.

### 2026-11-13, Phase 7 (who's in the room): deferred

The plan said to carry each open node's room address in its existing `here`
long-poll (ADR 0021), or defer. It doesn't work:

- Only a node with an "Only while I'm here" story keeps that poll open, so
  most nodes in the room would never appear.
- Putting the address in the poll's signed body would change a signed type
  (`StoryRelay`), which the plan says not to do without asking; putting it
  outside the signature would let anyone in the room claim to be anyone, and
  the commons would show their link and QR code under someone else's name.

Doing it properly needs a small new signed type (say `RoomHere`: handle, key,
room address, time, signed by the member's key, sent by a node when it opens
to the room) with a spec vector (ADR 0009). That's a decision for Phoenix.
Until then, people scan each other's QR codes directly.

### 2026-11-13, Phases 5 (firewall and help) and 6 (room code): done, one commit

- **Why one commit:** both pass their extras (firewall hint, room code)
  through the same new path in `sidecar.rs` and `config.rs`.
- **Phase 5 built:** `docs/ROOM-GUIDE.md` (drafted by a subagent, then
  proofread against the code by another, 12 fixes applied: menu names, the
  check page's wording, the share form's box, Close being in the Go menu,
  local commons vs node in "what stays private", Linux marked not yet tried,
  `10.`/`172.` networks, firewalld zones, macOS "Block all"); the help page
  `/room/help` (built in Phase 4); `src-tauri/src/firewall.rs`, which reads
  macOS's `socketfilterfw` and Windows' network profile without admin rights
  and gives the room page a hint (`HIC_FIREWALL_HINT`), tested on sample
  outputs. Nothing changes a firewall.
- **Phase 6 built:** the app makes a fresh six-letter code (no I, L or O;
  `OsRng`, no bias) each time a local commons opens, passed as
  `HIC_ROOM_CODE`; `/room` shows it in two groups of three (screen readers get
  it letter by letter); the share form's box is now "Invite or room code"; the
  commons admits with it (route `room-code`), compared in constant time.
- **Deviations from the plan, recorded in ADR 0027:** the code travels in the
  share's existing unsigned `invite` field, not an `X-HIC-Room-Code` header,
  so no new plumbing and no signed bytes change; the commons doesn't advertise
  that it takes a code (the form's box always accepts one); the code isn't
  added to `redact_for_log`, because nothing prints it (the end-to-end test
  checks the server's log never contains it).
- **Reviewer (fresh subagent):** no way to be admitted without the code.
  Fixed from its report: after five wrong codes the person was told the
  commons "may not accept federation" (the wait is now passed on); guessing
  spread across many addresses wasn't capped (now 200 wrong codes in all stop
  the code until the room reopens); the room code accepted older-format shares,
  where some of what's shown isn't signed (now only signed `hic/v1`); a console
  window could flash on Windows (now `CREATE_NO_WINDOW`). Noted in the guide:
  anyone with the code can join as often as they like, so keep it on the
  screen, not in a chat.
- **Not changed (reviewer, by design or low):** everyone behind one NAT shares
  a wrong-code counter; opening on a Mac can take a few seconds while the
  firewall is read (it runs off the main thread).
- **Checks:** all green (see the commit).
