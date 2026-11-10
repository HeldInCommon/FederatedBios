# Contributing

From a fresh clone to a running node and passing tests in about ten minutes.

## 1. Set up

- Install Rust with [rustup](https://rustup.rs) (stable).
- For the desktop app only: `cargo install tauri-cli --version "^2" --locked`.
  On Linux, also the webview libraries listed in [readme.md](readme.md).

## 2. Run a node

```
cargo run --manifest-path commons/Cargo.toml
```

Open http://127.0.0.1:8080, create a handle and publish a profile. Data goes
to `./data`. To run a commons alongside it:

```
HIC_ROLE=commons PORT=8081 HIC_DATA=./data-commons cargo run --manifest-path commons/Cargo.toml
```

Then share your profile to `http://127.0.0.1:8081` from your node.

## 3. Run the tests

```
cargo test --manifest-path commons/Cargo.toml     # unit + in-process HTTP tests
cargo test --manifest-path src-tauri/Cargo.toml   # desktop app rules
cargo build --release --manifest-path commons/Cargo.toml && ./scripts/test-federation.sh
```

## 4. Find your way around

Read [docs/CODEMAP.md](docs/CODEMAP.md): one table per program, one job per
file, and the path a request takes.

## 5. Make a change

- **No hidden effects** (decision record 0016). Anyone reading a function's
  name and signature should be able to tell what it changes. Work out a change
  in a pure function that returns the new value or a plan, and apply it in one
  clearly named place that says what it writes. A function named like a read
  (`load`, `check`) only reads. Functions that write return a `Result`. Test
  the pure part with a unit test in the same file.
- **Comment the why.** Every module starts with a `//!` comment saying what it
  is for; every public function has a `///` comment. Explain decisions that
  aren't obvious from the code.
- **Don't change the signed format.** `commons/src/canonical.rs` defines bytes
  that every existing signature depends on. Its golden test must never change.
  A new format needs a new schema version (see the review, M1).
- **Add stored fields with `#[serde(default)]`** so older data files still load.
- **Before pushing:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
  and the tests above. CI runs the same checks.

## 6. Open a pull request

Branch from `main` in `HeldInCommon/FederatedBios` (not a fork), describe what
changed and how you tested it, and wait for the checks. Security-sensitive
areas (`federation.rs`, `keys.rs`, `passphrase.rs`, `sessions.rs`, `store.rs`)
need a second reviewer.
