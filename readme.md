# Held in Common

A **federated commons for creative work.** Everyone runs their own **node** — a
personal commons on a machine they control — and can **share signed copies** to a
**commons** that anyone can host. Your work stays on your machine unless you
choose to send a copy, and you attach the terms (license + CC signals) it travels
under. Members-only work is **encrypted** and never leaves your node readable.

- **Node** — the app on your own computer. Holds your profile, works, and datasets.
  This is where you create your identity and sign in.
- **Commons** — a shared directory others can browse. Anyone can host one; the same
  binary is both a node and a commons (set `HIC_ROLE=commons`). A commons is a
  **federation target, not an identity store** — there's no sign-up on it. Open
  one directly and you get the directory plus a "run a node to join" page.
- **Federation** — your node posts a signed, content-addressed copy to a commons'
  `/inbox`; the commons verifies the signature + fingerprint and lists it.

**How it all fits together (slides):** https://phoenixperry.github.io/FederatedBios/architecture.html

**Architecture deep-dive (for developers):** [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

**Shipping from the org (maintainers):** [docs/MAINTAINER.md](docs/MAINTAINER.md)

---

## Run it — no building required

Non-technical users should follow the **[workshop install guide](docs/INSTALL.md)**.
Everyone runs a node, then shares up to a commons — there's no sign-up on the
commons itself. Two paths:

1. **Desktop app** — download the unsigned preview (macOS Apple Silicon / Linux),
   open it, and you're a node. See [docs/INSTALL.md](docs/INSTALL.md).
2. **Docker** — `ghcr.io/<owner>/hic-node:latest` (see [commons/README.md](commons/README.md)).

---

## Build from source (developers)

**Prerequisites:** [Rust](https://rustup.rs) (stable). For the desktop app also:
`cargo install tauri-cli --version "^2" --locked`, and on Linux the webview libs
(`libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev patchelf`).

### 1. The node / commons server (`commons/`)

A self-contained axum server — this one binary is both a node and a commons.

```
cargo run --manifest-path commons/Cargo.toml
```

Open **http://localhost:8080**. Configure with env vars:

| Variable | Default | Meaning |
|---|---|---|
| `PORT` | `8080` | Port to listen on |
| `HIC_DATA` | `./data` | Directory for this node's data |
| `HIC_INSTANCE` | `My node` | Human label for this instance |
| `HIC_ROLE` | inferred | `node` or `commons`. Inferred from `HIC_INSTANCE` (contains "commons"/"federated" → `commons`). A `commons` gates `/join` + `/login` (federation-only); a `node` allows full sign-up |
| `HIC_SHARE_DEFAULT` | *(blank)* | Commons address pre-filled in "Share to a commons" |
| `HIC_BASE_URL` | `http://localhost:$PORT` | Public base URL (for federation/ActivityPub) |

### 2. The desktop app (`src-tauri/`)

A native Tauri shell that bundles the `hic-commons` server as a **sidecar** and
starts it — no Docker, no terminal. The sidecar binary is gitignored, so build
and place it first (named for your platform triple):

```
# build the node server
cargo build --release --manifest-path commons/Cargo.toml

# copy it in as the Tauri sidecar for your platform
TRIPLE=$(rustc -vV | sed -n 's/host: //p')
mkdir -p src-tauri/binaries
cp commons/target/release/hic-commons "src-tauri/binaries/hic-commons-$TRIPLE"

# run the app
cargo tauri dev            # or: cargo tauri build
```

The app opens to **http://localhost:8080** — your node. CI
(`.github/workflows/desktop-app.yml`) does these same steps to build the
macOS + Linux installers.

---

## Test the federation

End-to-end check — stands up a node and a separate commons, then proves
publish → sign → federate, tamper rejection, and the members-only guard:

```
./scripts/test-federation.sh
```

## Repo layout

```
commons/       the node + commons server (Rust / axum) — identity, content-addressed
               signed publishing, encrypted members-only, CC governance, directory,
               JSON API, ActivityPub + /inbox federation. Dockerfile here.
src-tauri/     the native desktop app that runs the server as a sidecar
docs/          INSTALL.md (workshop guide) + architecture.html (reveal.js slides)
node/, src/    legacy Leptos onboarding prototype (Docker-based, port 4321) — kept
               for reference; the current product is commons/ + src-tauri/
```

## License / governance model

- **Visibility** — *Public* (listed + searchable), *Commons* (listed, out of search),
  *Members-only* (encrypted to a member set; never federates).
- **License** — Creative Commons: how people may reuse the work.
- **CC signals** — how AI models may learn from it (Credit / Direct contribution /
  Ecosystem / Open). Terms travel with every copy.
