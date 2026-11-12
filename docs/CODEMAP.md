# Code map

Where everything lives, and the path a request takes through it. Read this
before changing code; read [ARCHITECTURE.md](ARCHITECTURE.md) for how the system
fits together, and [adr/](adr/README.md) for why each major decision was made.

## The two programs

| Folder | What it is |
|---|---|
| `commons/` | `hic-commons`, the server. One binary that runs as a **node** or a **commons**. |
| `src-tauri/` | The desktop app. Starts `hic-commons` as a "sidecar" and shows its pages. |

Everything else: `scripts/test-federation.sh` (end-to-end test), `docs/`,
`.github/workflows/` (CI). An older onboarding prototype is kept at the git
tag `legacy-onboarding`, not on `main`.

## The server (`commons/src/`)

The rule: **no hidden effects** ([decision record 0016](adr/0016-no-hidden-effects.md)).
Decisions are pure functions that take data and return data (the new value, or
a plan of what to write), so they can be tested with a one-line assertion.
Writes happen in clearly named places that say what they change.

| File | One job | Pure? |
|---|---|---|
| `main.rs` | Read config, decide the role, open the store, serve. Nothing else. | — |
| `lib.rs` | Lists the modules; `now()` is the only clock read. | — |
| `app.rs` | `AppState` (what handlers can reach) and `router()` (every route). | yes |
| `config.rs` | Environment variables → `Config`; where to listen. | yes (`from_lookup`) |
| `role.rs` | Is this data folder a node or a commons? `decide()` is the rule; `resolve()` reads/writes `instance.json`. | `decide` yes |
| `model.rs` | The stored and sent types: `HandleRec`, `Profile`, `Version`, `RemoteEntry`, `Reach`. | yes |
| `canonical.rs` | The content address (`cid`) of a profile version (format version 0). **Its bytes are a contract.** | yes |
| `jcs.rs` | Canonical JSON (RFC 8785) for format `hic/v1`. | yes |
| `envelope.rs` | Signed `hic/v1` objects: build, sign, cosign, verify. See `docs/spec/`. | yes |
| `waiting.rs` | Following up on waiting shares (at start-up and on Check now) and recording news. | rule yes; asks and writes |
| `vouching.rs` | Who may join a commons: invites, vouches, the founding invite, the weekly limit; who may see the waiting list (`check_waiting_request`). | yes |
| `vouch_list.rs` | Vouching with a click (decision record 0026): asking each commons who is waiting, what the answer means (`Lookup`), and the result of a vouch in plain words. | rules yes; asks (reads only) |
| `ratelimit.rs` | At most N requests per minute per sender (the inbox). | in-memory state |
| `identity.rs` | Who holds a handle on a commons; recovery keys and key rotation rules. | yes |
| `member.rs` | Pure changes to a member's record (moving to new keys). | yes |
| `publishing.rs` | Sign a profile version (used by publish and by key rotation). | yes, except members-only encryption |
| `outbound.rs` | Which addresses this server may contact for a member (ADR 0012). | yes, except the DNS lookup |
| `keys.rs` | Generate keys, sign/verify a `cid`, encrypt to members. | yes, except key generation |
| `passphrase.rs` | Argon2id hashing and checking; upgrades old SHA-256 hashes. | yes, except salt generation |
| `backup.rs` | Encrypted identity backups (seal and open). | yes, except salt/nonce generation |
| `federation.rs` | Build and verify shares and withdrawals; `admit()` a share or not; `apply_withdraw()`. | yes |
| `ramp.rs` | The member's next step and which profile tabs they see (decision record 0022). | yes |
| `story.rs` | Stories (decision record 0020): check a written story, sign it, verify a shared one, `admit()` it on a commons (author must be listed), withdrawals. | yes |
| `here.rs` | "Only while I'm here" (decision record 0021): the signed `StoryCard` (no text, its SHA-256) and `StoryRelay` (a node's poll with its answers); a commons' rules for them (`admit_card`, `text_matches`). | yes |
| `relay.rs` | A commons' side of "Only while I'm here": who is here (`Presence`) and readers waiting (`RequestState`), as enums with pure transitions, kept in memory only. | in-memory state |
| `relay_node.rs` | A node's side: which commons need a connection, what to answer, the connection's state (`Link`), and the loop that keeps each long poll open. | rules yes; asks commons |
| `text_links.rs` | Finds web addresses and `[words](https://…)` links in a story's plain text, for display only (`http`/`https` only; the signed text never changes). | yes |
| `governance.rs` | The licence and signal vocabularies. | yes |
| `handle.rs` | What a handle may contain (also guards file names). | yes |
| `form.rs` | Parse form posts (keeps repeated keys). | yes |
| `sessions.rs` | Sessions (expire after 24 h) and the sign-in attempt limit. | cookie helpers yes |
| `store.rs` | **The only module that touches files.** Atomic writes; "missing" vs "damaged"; the member and shared-profile lists cached in memory, cleared by every write. | no |
| `web/layout.rs` | The page frame, badges, error pages, redirects. | yes |
| `web/pages/` | The body of each page, one file per page (`directory.rs`, `profile.rs`, `owner.rs`, `publish.rs`, `signin.rs`, `stories.rs`, `story.rs`, `story_form.rs`, …); `mod.rs` lists them. Shared pieces of the heldincommon.org look (page heading, section label, starburst, terms panel, "2 min ago") are in `parts.rs`; the Sharing and rights fields, used by profiles and stories, in `rights.rs`. | yes |
| `web/handlers/` | Request handlers, one file per page or flow: load → decide (pure) → render. `mod.rs` lists each file's routes and holds the shared helpers (session, CSRF check, `update_member`). | no |
| `web/handlers/inbox.rs` | What a commons receives from nodes: shares, vouches, withdrawals, identity changes. | no |
| `web/handlers/share.rs` | Sharing to and withdrawing from a commons; sending to a commons' inbox. | no |
| `web/handlers/steward.rs` | The steward view of a commons: open it, approve, decline, make invites. | no |
| `web/handlers/membership.rs` | Bringing people in: invite codes and vouches (the People tab's Vouch buttons post here). | no |
| `web/handlers/waiting_list.rs` | `POST /api/waiting` on a commons: who is waiting for a vouch, for a listed member's signed request only. | no |
| `web/pages/owner_people.rs` | The People tab: who is waiting on each commons, with Vouch buttons; invite someone; vouch by handle and key. | yes |
| `web/handlers/account.rs` | A member's keys: recovery key, replacing a key, recovering. | no |
| `web/handlers/stories.rs` | Story lists (a node's; a commons', with recent withdrawals) and a story on a commons. | no |
| `web/handlers/here.rs` | "Only while I'm here" on a commons: `/here/card`, the long poll `/here/poll`, and reading a card's story (page and `here.json`), never cached. | no |
| `web/handlers/story.rs` | One story on its author's node: the page, export, share (`share_story_to`), withdraw. | no |
| `web/handlers/story_edit.rs` | Writing a story in two steps (the words, then its rights), editing it, changing its terms. | no |
| `web/email.rs` | Ready-written invitation emails as `mailto:` links. | yes |
| `api.rs` | JSON, WebFinger and ActivityStreams-shaped endpoints. | builders yes |
| `../static/` | `style.css`, `avatar.js` and `password.js`, compiled into the binary. | — |
| `../static/quilt/` | The eight quilt backgrounds (`quilt-1.svg` … `quilt-8.svg`), made by `scripts/make-quilts.py`, served at `/quilt/N.svg`; `layout::quilt_for` picks one per page from its title. | — |
| `../tests/http/` | In-process HTTP tests of the whole router, one file per behaviour (`sharing.rs`, `vouching.rs`, `steward.rs`, …); helpers in `common.rs`. | — |
| `../tests/spec_vectors.rs` | Checks `docs/spec/vectors/` against this implementation. | — |

### Following a request: someone shares a profile to a commons

1. **Node**, `web/handlers/share.rs::share` — checks the session and CSRF token, loads
   the member (`store.rs`) and their current version.
2. `federation.rs::build_share` — turns the version into the signed message.
3. The node posts it to `https://<commons>/inbox`.
4. **Commons**, `web/handlers/inbox.rs::inbox` — parses the JSON.
5. `federation.rs::verify_share` — recomputes the `cid` with
   `canonical.rs::content_id` and checks the signature with
   `keys.rs::verify_cid`.
6. `federation.rs::admit` — refuses a handle held by another key, or a version
   older than the one already held.
7. `store.rs::save_remote` — writes `remote/<keyid>.json` atomically.
8. `web/handlers/directory.rs` → `web/pages/directory.rs` lists it.

### Following a request: reading a story shared "Only while I'm here"

1. **Node**, `relay_node::run` (started in `main.rs`) sees a story shared as a
   card and keeps `POST <commons>/here/poll` open (`keep_open`).
2. **Commons**, `web/handlers/here.rs::here_poll` — `here::verify_relay`
   (signed, addressed, recent), `relay::Relay::open_poll` (refuses a replay),
   then holds the request open.
3. A reader opens `/stories/<keyid>/<id>`: `handlers/stories.rs::held_story`
   finds a card, not a copy → `here::card_page` → `fetch_text` records a
   request; the open poll hands it over (`Relay::hand_over`).
4. **Node**, `relay_node::answers_for` → the next poll carries the text.
5. **Commons**, `Relay::apply_answers` checks it with `here::text_matches`
   against the card's signed hash → the reader's page takes it
   (`take_reading`) and the text is dropped. Nothing is written to disk.

### Following a request: someone withdraws

`handlers::withdraw` (node) → `federation::build_withdraw` (signed, addressed
to that commons) → `POST <commons>/inbox` → `handlers::inbox` sees
`"type": "Withdraw"` → `federation::verify_withdraw` → `apply_withdraw`
(refuses if a newer version was shared after it) → `store.save_tombstone` and
`store.delete_remote`. Later, `federation::admit` consults the tombstones.

### Following a request: a member vouches with a click

`handlers::profile` (node, `?tab=people`) → `vouch_list::ask_each` → for each
commons the member is listed on, `vouching::make_waiting_request` (signed with
the member's key) → `POST <commons>/api/waiting` →
`handlers::waiting_list` (commons: rate limit, then
`vouching::check_waiting_request`, then `vouch_list::waiting_from`) → back on
the node, `vouch_list::lookup_from_answer` → `pages/owner_people.rs` shows
each person with a **Vouch** button. Pressing it: `membership::vouch` →
`vouching::make_vouch` → `POST <commons>/inbox` (the usual vouch, ADR 0011) →
`vouch_list::vouch_result` → `pages::vouch_outcome`.

### Following a request: someone publishes

`handlers::publish` → `Form` → `profile_from_form` (pure) →
`canonical::content_id` → `keys::sign_cid` → `store.save_version` and
`store.save_handle` (under `AppState::write`, so two publishes can't
interleave) → redirect to the profile.

## The desktop app (`src-tauri/`)

| File | One job |
|---|---|
| `src/lib.rs` | Wires the modules together; adds the switcher to each server page. |
| `src/settings.rs` | `app.json` (what this computer runs and the names); each `Kind`'s port, cookie name and data folder. |
| `src/network.rs` | Is *our* server on a port answering (launch token check)? This computer's LAN address. |
| `src/sidecar.rs` | Start and stop the servers (one per `Kind`, side by side); restart one after a crash (up to 3 times). |
| `src/steward.rs` | The commons' steward link: hidden from the log, read for **Go → Show steward link…** (ADR 0024). |
| `src/links.rs` | `hic-app://open/<kind>` links and the switcher's buttons (pure). |
| `src/commands.rs` | What the screens, the Go menu and links can do: `open` a server (setting it up if needed), `remove_practice`; commands `setup_state`, `choose_role`, `add_role`, `set_name`, `close_setup`, `node_up`, `server_url`, `restart_node`, `remove_practice_now`. |
| `src/lib.rs` (also) | The main window (hands `mailto:` to the mail app, `hic-app://` to `commands::open`), the **Go** menu, the log file. |
| `ui/index.html`, `ui/app.js`, `ui/style.css` | The first-launch chooser, loading screen, and the setup window (add, rename, remove practice). |
| `capabilities/default.json` | What the screens may call (core APIs only). |
| `tauri.conf.json` | Window, bundle, and the screens' Content-Security-Policy. |

## Tests

| Command | What it covers | Time |
|---|---|---|
| `cargo test --manifest-path commons/Cargo.toml` | Unit tests in every module, plus `tests/http/` (the whole router in-process; `cargo test --test http vouching` runs one area). | ~5 s |
| `cargo test --manifest-path src-tauri/Cargo.toml` | The app's pure rules (role migration, health check, labels). | ~1 s once built |
| `./scripts/test-federation.sh` | Real processes: nodes, commons, an impostor, tampering, members-only, vouching (including with a click), steward approval, Check now, withdrawal, key rotation and recovery, and "Only while I'm here" (stop and restart a node). | ~25 s |
| `python3 scripts/check_spec_vectors.py` | An independent (Python) check of the spec's canonical bytes and ids. | <1 s |

All four run on every pull request (`.github/workflows/pr-checks.yml`), with
`cargo fmt --check`, `cargo clippy -D warnings`, `./scripts/lint-css.sh`
(the stylesheets' layout and rules; needs Node), and
`python3 scripts/check-docs.py` (every Rust file, function and type has a
doc comment).
