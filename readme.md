# Held in Common

A **federated commons for creative work.** Everyone runs their own **node**, a
personal commons on a computer they control, and can **share signed copies** of
their profile to a **commons** that anyone can host. Your work stays on your
machine unless you choose to send a copy, and the terms you attach (licence and
CC signals) travel with it. Members-only work is encrypted and never leaves your
node.

- **Node:** the app on your own computer. You create your identity here, publish,
  and choose what to share and where.
- **Commons:** a shared directory, run by a **steward** (a workshop host, a
  collective, a server). There's no sign-up on a commons: profiles arrive as
  signed copies from people's nodes, and a newcomer is listed once a member
  vouches for them or the steward approves them.
- **Same program, two roles:** one binary runs as either. The desktop app asks
  which when it first starts, and can run both side by side, plus a practice
  commons to learn on (**Go** menu).

Current version: **0.4.0**, signed and notarized for macOS.

| I want to… | Read |
|---|---|
| Install the app and join a commons | [docs/INSTALL.md](docs/INSTALL.md) |
| Understand how it works | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Find my way around the code | [docs/CODEMAP.md](docs/CODEMAP.md) |
| Contribute | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Know why something is the way it is | [docs/adr/](docs/adr/README.md) (decision records) |
| Implement the signed formats elsewhere | [docs/spec/](docs/spec/README.md) |
| Release, sign, or host it | [docs/MAINTAINER.md](docs/MAINTAINER.md) |
| Run a commons in Docker | [commons/README.md](commons/README.md) |

Slides: https://phoenixperry.github.io/FederatedBios/architecture.html

---

## Run it

**Desktop app (recommended):** download it from
[the releases page](https://github.com/HeldInCommon/FederatedBios/releases/latest)
(macOS Apple Silicon, Linux). On a Mac, open the `.dmg` and drag
**Held In Common** into **Applications**. Then follow
[docs/INSTALL.md](docs/INSTALL.md).

**Docker:** see [commons/README.md](commons/README.md) for a node or a commons.

---

## Build from source

**You need:** [Rust](https://rustup.rs) (stable). For the desktop app also
`cargo install tauri-cli --version "^2" --locked`, and on Linux the web view
libraries (`libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev patchelf`).

**The server** (`commons/`), a node at http://127.0.0.1:8080:

```
cargo run --manifest-path commons/Cargo.toml
```

Settings are environment variables (`HIC_ROLE`, `HIC_DATA`, `PORT`,
`HIC_BASE_URL`, …), listed in [docs/ARCHITECTURE.md §9](docs/ARCHITECTURE.md#configuration).

**The desktop app** (`src-tauri/`) bundles the server, so build that first:

```
cargo build --release --manifest-path commons/Cargo.toml
TRIPLE=$(rustc -vV | sed -n 's/host: //p')
mkdir -p src-tauri/binaries
cp commons/target/release/hic-commons "src-tauri/binaries/hic-commons-$TRIPLE"
cd src-tauri && cargo tauri dev        # or: cargo tauri build
```

## Test

```
cargo test --manifest-path commons/Cargo.toml       # unit, HTTP and format tests
cargo test --manifest-path src-tauri/Cargo.toml     # desktop app
cargo build --release --manifest-path commons/Cargo.toml && ./scripts/test-federation.sh
python3 scripts/check_spec_vectors.py               # the signed format, checked in Python
```

The end-to-end script runs real nodes and commons and covers sharing, tampering,
impostors, vouching and steward approval, withdrawal, key replacement and
recovery. Every pull request runs all of these.

## Repository layout

```
commons/       the node + commons server (Rust, axum); Dockerfile
src-tauri/     the desktop app (Tauri), which runs the server as a sidecar
docs/          architecture, code map, install guide, decision records, format spec, review
scripts/       end-to-end test; independent format check
```

An earlier onboarding prototype is kept at the git tag `legacy-onboarding`.

## Licences and signals

- **Reach:** *Public* (listed, searchable), *Commons* (listed, kept out of
  search), *Members only* (encrypted on your node, never shared).
- **Licence:** Creative Commons: how people may reuse the work.
- **CC signals:** how AI models may learn from it (Credit, Direct contribution,
  Ecosystem, Open). They travel, signed, with every copy.
