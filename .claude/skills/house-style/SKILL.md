---
name: house-style
description: How code in Held in Common (FederatedBios) is written, structured and tested. Use before writing or changing any Rust, HTML (maud), CSS, JavaScript or test in this repo, before adding a page or route, and before opening a pull request.
---

# Held in Common house style

This is how Phoenix and the team like code written here. Treat it as a set of
good-practice guides, not hard rules: where following one makes the code less
clear, say so and do the clearer thing.

The team is **learning Rust**. Write code a learner can follow: plain over
clever, named steps over long chains, and comments that explain *why*.

The longer versions live in the repo. Read the one that matches the task:

- `CONTRIBUTING.md`: setting up, running tests, making a change.
- `docs/CODEMAP.md`: where everything lives, and a request followed end to end.
- `docs/ARCHITECTURE.md`: the moving parts and why.
- `docs/adr/`: the decision records. Read one before changing what it decided.

## 1. No hidden effects (ADR 0016)

This is the most important rule. Anyone reading a function's name and
signature should be able to tell what it changes.

- **Work out a change in a pure function** that takes data and returns the new
  value or a plan. Examples: `federation::admit`, `federation::plan_listing`,
  `HandleRec::with_shared_to`, `role::decide`.
- **Apply it in one clearly named place** whose doc comment lists what it
  writes. Examples: `Store::apply_listing`, `update_member`, `role::save`.
- **Read-sounding names only read.** `load`, `check` and `inspect` never write.
  If something changes state, its name says so: `save_…`, `update_…`,
  `apply_…`, `record_if_under_limit`.
- **Functions that write return a `Result`.** Don't swallow errors in helpers;
  the caller decides what a failure means, visibly.
- **Allowed exceptions, documented:** shared in-memory state behind a lock
  (sessions, rate limits) and logging.

## 2. Rust patterns we use

- **Enums for outcomes**, not booleans or strings. For example, `Admission`
  says why a listing was accepted or refused. Callers `match` on every case.
- **Withers** (`with_x(self, …) -> Self`) return an updated copy instead of
  changing a value in place.
- **The repository** (`store.rs`) is the only code that touches files on disk.
  It writes under a single writer lock.
- **Config is passed in** (`Config`, `AppState`), not read from globals or the
  environment deep inside a function.
- **New stored fields get `#[serde(default)]`** so older data files still load.
- **Visibility is as narrow as works.** Use `pub(super)` or `pub(crate)`, and
  re-export with `pub(crate) use` when the item is crate-private.
- **Never change the signed format.** `commons/src/canonical.rs` defines bytes
  that every existing signature depends on, and its golden test must never
  change. A new format needs a new schema version and a spec entry
  (`docs/spec/`, ADR 0009).

## 3. Structure: split early, one file per page or behaviour

Split a file by page or behaviour as soon as it starts collecting several,
before it gets long. "It will rapidly get huge as the site grows outward."

**Adding a page** takes four steps:

1. Put the body in `commons/src/web/pages/<name>.rs`.
2. Put the handler in `commons/src/web/handlers/<name>.rs`.
3. Re-export both from their `mod.rs`, and add the route in `app.rs`.
4. Put its HTTP tests in `commons/tests/http/<area>.rs`.

Don't extend an existing file with an unrelated page.

**Every module** starts with a `//!` comment saying what it is for. **Every
public function** has a `///` comment. Each `mod.rs` keeps its table of files
up to date. In those tables, write file names as plain code (`` `notices.rs` ``),
not intra-doc links, which break the `cargo doc` check (§5).

**When structure changes**, update `docs/CODEMAP.md` and `CONTRIBUTING.md` in
the same pull request.

**When you make a decision** that would be expensive to reverse, touches
security or the signed format, or someone will ask "why?" about, write a new
ADR. Copy an existing one's layout. Don't rewrite an accepted ADR; supersede it.

## 4. CSS and front end

There are two stylesheets:

- `commons/static/style.css` styles the site.
- `src-tauri/ui/style.css` styles the app's own screens.

Both use the same layout: 1 Tokens, 2 Base, 3 Shared patterns, then the site
frame or screens, page parts, and small screens.

- **Tokens first.** Colours, fonts, type sizes (in rem) and spacing (a 4px
  scale) are custom properties on `:root`. Use a token in rules, not a raw
  value. To restyle everywhere, change the token, not the rules.
- **Reuse patterns before adding classes.** Use the mono label, buttons and
  panels. For variants, set a `--tone` variable instead of copying
  declarations. Merge near-duplicate rules; too many is "overkill".
- **Element defaults** go in Base, wrapped in `:where()`, *before* the
  patterns, so they never out-rank a pattern.
- **No dead rules and no inline styles.** To show or hide something, use the
  `hidden` attribute.
- **Always leave CSS linted:** run `./scripts/lint-css.sh --fix`, then
  `./scripts/lint-css.sh`. The layout is one declaration per line. CI fails
  on lint errors.
- **Check what it looks like.** For a visual change, compare before and after
  screenshots of the affected pages at desktop and phone widths (1280 and
  390px). Report any differences.
- **Keep JavaScript small**, and only where the server can't do the job. It
  toggles attributes and classes rather than writing styles.

## 5. Testing

Every change comes with tests at the right level:

| Level | Where | For |
|---|---|---|
| Unit | `#[cfg(test)] mod tests` at the bottom of the same file | The pure functions: every rule, every `Admission` case. |
| HTTP | `commons/tests/http/<area>.rs` (shared helpers in `common.rs`) | A page or route through the whole router, in process. |
| End to end | `./scripts/test-federation.sh` | Real nodes and commons talking: tampering, impostors, withdrawal, recovery. |
| Spec vectors | `python3 scripts/check_spec_vectors.py` | Canonical bytes and content ids, checked independently. |

- **Name tests as sentences** that state the rule, for example
  `recovery_beats_a_thief` and `older_version_from_same_key_is_stale`.
- **Test the pure part directly.** If something is hard to test, it probably
  has a hidden effect (§1).
- **Security changes** need a test that the attack fails, not only that the
  happy path works.
- **Golden tests** (`GOLDEN_CID` and the spec vectors) are never edited to make
  them pass.

**Before saying a change is done**, run all of these. Report what you ran and
the results, including failures:

```
cargo fmt --manifest-path commons/Cargo.toml
cargo clippy --manifest-path commons/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path commons/Cargo.toml --no-deps
cargo test --manifest-path commons/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml        # if src-tauri changed
cargo build --release --manifest-path commons/Cargo.toml && ./scripts/test-federation.sh
python3 scripts/check_spec_vectors.py
./scripts/lint-css.sh                                  # if any CSS changed
```

## 6. Git and pull requests

- Work in the org repo **HeldInCommon/FederatedBios**. Never push to a
  personal fork.
- **Branch from `main`** with a short prefix: `feat/`, `fix/`, `refactor/`,
  `docs/` or `chore/`. Never push directly to `main`.
- **One topic per pull request.** Its description says what changed, why, and
  how it was tested.
- **Commit subjects** are plain sentences saying what changed, for example
  "Split pages, handlers and HTTP tests per page".
- **Security-sensitive files need a second reviewer:** `federation.rs`,
  `identity.rs`, `keys.rs`, `passphrase.rs`, `sessions.rs`, `store.rs`,
  `vouching.rs`, `outbound.rs` and `web/handlers/steward.rs`.
- **Secrets never go in chat, code or CI config.** That covers signing keys,
  passwords and team IDs (`docs/MAINTAINER.md`).

## 7. Writing (comments, docs, PRs)

- **Plain language, short sentences, British spelling** ("colour",
  "organised").
- **Explain the why.** For security or protocol work, say what the threat is
  and the trade-off, in words a newcomer can follow, then make the change.
- **Cite sources** for factual claims (RFCs, docs), and mark anything you
  haven't verified. Never invent references.
- **Assume teammates will read everything.**
