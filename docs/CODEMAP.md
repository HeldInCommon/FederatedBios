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

The rule: **decisions are pure functions; effects live at the edges.** A pure
function takes data and returns data. It doesn't read files, the clock,
randomness or the network, so it can be tested with a one-line assertion.

| File | One job | Pure? |
|---|---|---|
| `main.rs` | Read config, decide the role, open the store, serve. Nothing else. | — |
| `lib.rs` | Lists the modules; `now()` is the only clock read. | — |
| `app.rs` | `AppState` (what handlers can reach) and `router()` (every route). | yes |
| `config.rs` | Environment variables → `Config`; where to listen. | yes (`from_lookup`) |
| `role.rs` | Is this data folder a node or a commons? `decide()` is the rule; `resolve()` reads/writes `instance.json`. | `decide` yes |
| `model.rs` | The stored and sent types: `HandleRec`, `Profile`, `Version`, `RemoteEntry`, `Reach`. | yes |
| `canonical.rs` | The content address (`cid`) of a version. **Its bytes are a contract.** | yes |
| `keys.rs` | Generate keys, sign/verify a `cid`, encrypt to members. | yes, except key generation |
| `passphrase.rs` | Argon2id hashing and checking; upgrades old SHA-256 hashes. | yes, except salt generation |
| `backup.rs` | Encrypted identity backups (seal and open). | yes, except salt/nonce generation |
| `federation.rs` | Build and verify shares and withdrawals; `admit()` a share or not; `apply_withdraw()`. | yes |
| `governance.rs` | The licence and signal vocabularies. | yes |
| `handle.rs` | What a handle may contain (also guards file names). | yes |
| `form.rs` | Parse form posts (keeps repeated keys). | yes |
| `sessions.rs` | Sessions (expire after 24 h) and the sign-in attempt limit. | cookie helpers yes |
| `store.rs` | **The only module that touches files.** Atomic writes; "missing" vs "damaged". | no |
| `web/layout.rs` | The page frame, badges, error pages, redirects. | yes |
| `web/pages.rs` | The body of each page. | yes |
| `web/handlers.rs` | Request handlers: load → decide (pure) → render. | no |
| `api.rs` | JSON, WebFinger and ActivityStreams-shaped endpoints. | builders yes |
| `../static/` | `style.css` and `avatar.js`, compiled into the binary. | — |
| `../tests/http.rs` | In-process HTTP tests of the whole router. | — |

### Following a request: someone shares a profile to a commons

1. **Node**, `web/handlers.rs::share` — checks the session and CSRF token, loads
   the member (`store.rs`) and their current version.
2. `federation.rs::build_share` — turns the version into the signed message.
3. The node posts it to `https://<commons>/inbox`.
4. **Commons**, `web/handlers.rs::inbox` — parses the JSON.
5. `federation.rs::verify_share` — recomputes the `cid` with
   `canonical.rs::content_id` and checks the signature with
   `keys.rs::verify_cid`.
6. `federation.rs::admit` — refuses a handle held by another key, or a version
   older than the one already held.
7. `store.rs::save_remote` — writes `remote/<keyid>.json` atomically.
8. `web/handlers.rs::directory` → `web/pages.rs::directory` lists it.

### Following a request: someone withdraws

`handlers::withdraw` (node) → `federation::build_withdraw` (signed, addressed
to that commons) → `POST <commons>/inbox` → `handlers::inbox` sees
`"type": "Withdraw"` → `federation::verify_withdraw` → `apply_withdraw`
(refuses if a newer version was shared after it) → `store.save_tombstone` and
`store.delete_remote`. Later, `federation::admit` consults the tombstones.

### Following a request: someone publishes

`handlers::publish` → `Form` → `profile_from_form` (pure) →
`canonical::content_id` → `keys::sign_cid` → `store.save_version` and
`store.save_handle` (under `AppState::write`, so two publishes can't
interleave) → redirect to the profile.

## The desktop app (`src-tauri/`)

| File | One job |
|---|---|
| `src/lib.rs` | Wires the modules together; injects the "Back to your node" button. |
| `src/settings.rs` | `app.json` (the chosen role) and the per-role data folders. |
| `src/network.rs` | Is *our* server answering (launch token check)? This computer's LAN address. |
| `src/sidecar.rs` | Start and stop the server process. |
| `src/commands.rs` | What the app's screens can call: `setup_state`, `choose_role`, `node_up`, `restart_node`. |
| `ui/index.html`, `ui/app.js`, `ui/style.css` | The first-launch chooser and loading screen. |
| `capabilities/default.json` | What the screens may call (core APIs only). |
| `tauri.conf.json` | Window, bundle, and the screens' Content-Security-Policy. |

## Tests

| Command | What it covers | Time |
|---|---|---|
| `cargo test --manifest-path commons/Cargo.toml` | Unit tests in every module, plus `tests/http.rs` (the whole router in-process). | ~5 s |
| `cargo test --manifest-path src-tauri/Cargo.toml` | The app's pure rules (role migration, health check, labels). | ~1 s once built |
| `./scripts/test-federation.sh` | Real processes: nodes, commons, an impostor, tampering, members-only. | ~15 s |

All three run on every pull request (`.github/workflows/pr-checks.yml`), with
`cargo fmt --check` and `cargo clippy -D warnings`.
