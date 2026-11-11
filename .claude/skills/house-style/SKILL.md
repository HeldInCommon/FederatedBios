---
name: house-style
description: How code in Held in Common (FederatedBios) is written, structured and tested, including design (MVC, traits, states), errors, naming, accessibility, dependencies, logging and when to ask before deciding. Use before writing or changing any Rust, HTML (maud), CSS, JavaScript or test in this repo, before adding a page, route or dependency, and before opening a pull request.
---

# Held in Common house style

This is how Phoenix and the team like code written here. Treat it as a set of
good-practice guides, not hard rules: where following one makes the code less
clear, say so and do the clearer thing.

The team is **learning Rust**. Write code a learner can follow: plain over
clever, named steps over long chains, and comments that explain *why*.
**Comment all code well** (§8): comments are part of the code here, not an
extra.

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

## 2. Design: layers, interfaces, composition and states

Phoenix leans towards abstraction, interfaces (in the C# sense), composition,
MVC-style layering and explicit states, with a clean line between data and
display. Here is how that looks in Rust and in this repo.

- **Keep data and display apart (MVC).**
  - *Model:* `model.rs`, `member.rs`, `identity.rs`, `federation.rs` and
    friends hold the data and the rules. They contain no HTML and no HTTP.
  - *View:* `web/pages/` takes data and returns markup. It never reads the
    store and never decides a rule. If a page needs a decision, the handler
    or model makes it and passes the answer in, for example the `Viewer`
    enum in `pages/directory.rs`.
  - *Controller:* `web/handlers/` reads the request, asks the model, applies
    changes through the store and picks the view. Keep handlers thin; when one
    grows logic, move that logic into the model and unit-test it there.
  - *Persistence:* `store.rs` (the repository) sits behind the model.

  The desktop app follows the same split: Rust decides, `src-tauri/ui/` shows.
- **Make states explicit.** When something can be in several states, model it
  as an enum with one variant per state. Each variant carries only the data
  that is valid in that state. `Viewer::Steward(&StewardView)` is an example:
  only the steward state has steward data.
  - Write transitions as pure functions that take the current state and an
    event and return the next state, or an error for a move that isn't
    allowed.
  - Prefer this to combinations of `bool` flags (`pending && !approved`), so
    impossible states can't be written down at all. Other examples:
    `Admission`, `waiting::Outcome`, `Role`.
- **Interfaces are traits.** A Rust trait is close to a C# interface: a set of
  methods a type promises to provide. The differences worth knowing:
  - Traits carry no data, and there is no inheritance.
  - You can implement a trait for a type you didn't write.
  - `impl Trait` and generics are resolved at compile time. `dyn Trait`
    works like holding an interface reference in C#, resolved at run time.
- **Abstract at real seams.** Add a trait where there are two
  implementations, or where a test needs a stand-in: storage, outbound HTTP,
  the clock, randomness. Today the store is a concrete type, and the only
  trait is `WithStatus` in `web/handlers/mod.rs`. Don't add a trait for a
  single implementation "in case": while the team is learning Rust, each
  trait is one more hop to follow. When you do add one, document what an
  implementation must guarantee.
- **Compose, don't inherit.** Rust has no inheritance. Build bigger types out
  of smaller ones (`AppState` holds `Config`, `Store`, `Sessions` and the
  limiters). Share behaviour through traits and plain functions, and pass
  the pieces a function needs rather than the whole world.

## 3. Rust patterns we use

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

### Errors

- **Errors are typed enums** that say what went wrong (see `StoreError`),
  returned in a `Result` and handled with `?` and `match`. The repo uses
  neither `anyhow` nor `thiserror` today. Ask before adding either.
- **No `unwrap()` outside tests.** There is one accepted exception:
  `mutex.lock().unwrap()`, because a poisoned lock means another thread
  already panicked.
- **`expect("…")` only where failure is impossible**, and the message says
  why it is impossible: `expect("argon2 with default settings cannot fail")`.
  If you can't write that sentence honestly, return an error instead.
- **Failures people see** are plain sentences that say what happened and
  what to do next. Never show a raw error or a stack trace.

### Naming

- **Use full words, not abbreviations:** `member`, not `mbr`. Short names
  are fine for tiny closures and loop counters.
- **Use the domain's words, consistently:** node, commons, handle, member,
  steward, listing, share, vouch, withdraw, tombstone. Don't invent a
  synonym for one of these. If a new concept needs a word, define it once in
  `docs/ARCHITECTURE.md` and use it everywhere.
- **Names say what happens** (§1): a read sounds like a read, and a write
  says what it writes.
- **Booleans read as questions:** `is_listed`, `has_avatar`.

## 4. Structure: split early, one file per page or behaviour

Split a file by page or behaviour as soon as it starts collecting several,
before it gets long. "It will rapidly get huge as the site grows outward."

**Adding a page** takes four steps:

1. Put the body in `commons/src/web/pages/<name>.rs`.
2. Put the handler in `commons/src/web/handlers/<name>.rs`.
3. Re-export both from their `mod.rs`, and add the route in `app.rs`.
4. Put its HTTP tests in `commons/tests/http/<area>.rs`.

Don't extend an existing file with an unrelated page.

**Every module** starts with a `//!` comment saying what it is for. **Every
function**, public or private, has a `///` comment (see "Comment all code
well" in §8). Each `mod.rs` keeps its table of files
up to date. In those tables, write file names as plain code (`` `notices.rs` ``),
not intra-doc links, which break the `cargo doc` check (§6).

**When structure changes**, update `docs/CODEMAP.md` and `CONTRIBUTING.md` in
the same pull request.

**When you make a decision** that would be expensive to reverse, touches
security or the signed format, or someone will ask "why?" about, write a new
ADR. Copy an existing one's layout. Don't rewrite an accepted ADR; supersede it.

**Size prompts.** These are not hard limits; when one is reached, look for a
seam:

- A file passing **about 400 lines** probably holds more than one page or
  behaviour. Split it (today's largest, `federation.rs` and `store.rs`, are
  about 600).
- A function passing **about 50 lines**, or nesting more than three levels,
  probably hides named steps. Pull them out as small functions, pure ones
  where possible.

## 5. CSS and front end

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

### Accessibility

Every page should work for someone using a keyboard, a screen reader or a
large text size:

- **Semantic HTML first.** Use `button` for actions and `a` for navigation,
  with real headings in order, `main`, `nav` and lists. Use `aria-*` only
  where HTML has no element for the job.
- **Every input has a visible `label`.** Placeholder text is not a label.
- **Keep the focus ring visible.** If a style removes the outline (for
  example `all: unset`), put it back with the `--focus` token.
- **Colour contrast** meets WCAG AA (4.5:1 for body text). Colour is never
  the only signal: pair it with text or an icon.
- **Images have `alt`.** Use `alt=""` when the image is decoration (avatars
  next to a name).
- **Pages work without JavaScript.** Forms post to the server, and
  JavaScript only adds polish.
- **Text sizes are in rem**, so the page follows the reader's font size.

## 6. Testing

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
python3 scripts/check-docs.py
./scripts/lint-css.sh                                  # if any CSS changed
```

## 7. Git and pull requests

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

## 8. Writing (comments, docs, PRs)

- **Plain language, short sentences, British spelling** ("colour",
  "organised").
- **Explain the why.** For security or protocol work, say what the threat is
  and the trade-off, in words a newcomer can follow, then make the change.
- **Cite sources** for factual claims (RFCs, docs), and mark anything you
  haven't verified. Never invent references.
- **Assume teammates will read everything.**

### Comment all code well

The team is learning Rust, so comments are part of the code, not an extra.
Every change leaves the code it touches well commented:

- **Every file** starts with a comment saying what it is for: `//!` in Rust,
  a header comment in CSS and JavaScript.
- **Every function**, public or private, has a `///` comment saying what it
  does and what it returns. If it writes anything, the comment says what
  (§1). So does every struct, enum and variant, and every field whose
  meaning isn't obvious from its name.
- **Inside a function**, comment each step a newcomer wouldn't follow at a
  glance: why a check is there, what an edge case is, why the order matters.
  A long function should read as a sequence of commented steps.
- **Explain why, not what.** `// Re-sign after a key change: the old
  signature names the old key` helps; `// sign the story` repeats the code.
  Don't narrate obvious lines.
- **Security, protocol and privacy code** says what it protects against and
  cites its decision record, for example `// ADR 0012: a hosted node must not
  reach internal addresses`.
- **Rust that may surprise a learner** (lifetimes, `?`, `Box<…>`, `move`
  closures, `impl Trait`, `dyn Trait`) gets a short plain-English note the
  first time it appears in a file.
- **Tests** state their rule in their name (§6). A comment explains any setup
  that isn't obvious.
- **CSS:** each group of rules has a comment saying which part of a page it
  styles, and any non-obvious value says why (this width, this order). The
  linter wants a blank line before each comment.
- **Keep comments true.** When code changes, change its comments in the same
  commit: a wrong comment is worse than none. Delete commented-out code
  instead of keeping it.
- **The check.** `python3 scripts/check-docs.py` (run in CI) lists any Rust
  file without its `//!` comment, and any function, struct, enum, trait,
  type, const or static without a `///` comment (test functions excepted).
  It can't judge variants, fields or whether a comment explains *why*:
  those are still yours to get right.

## 9. Dependencies

- **Few and well known.** Before adding a crate or npm package, check
  whether the standard library or an existing dependency already does the
  job. A new dependency needs a reason in the PR description.
- **Cryptography comes from established crates** (the RustCrypto family and
  `ed25519-dalek`, as now). Never write our own.
- **Pin tool versions** with a lockfile, as in `tools/css-lint/`. Use exact
  versions for CI and scripts; never pull `latest` from the network.
- **Don't pipe remote scripts into a shell** (`curl … | sh`), and don't run
  unknown installers with `sudo`.

## 10. Logging and privacy

- **Never log or print secrets or personal data.** That covers passphrases,
  private keys, session tokens, recovery keys, backup contents and profile
  fields.
- **One deliberate exception:** at start-up, `main.rs` prints one-time links
  (the founding invite and the steward link) to the operator's own terminal.
  Keep it that way, and don't copy that output anywhere else.
- **Messages** should be useful to an operator: what happened and which
  handle or commons, not the data involved.
- **No analytics or tracking**, and no third-party requests beyond those an
  ADR already accepts (for example ADR 0013, Google Fonts).
- **Personal data never goes in URLs.** It goes in the request body, behind
  the session.

## 11. When to ask and when to decide

**Ask first** (stop, explain the options in plain language, recommend one)
before:

- changing the signed format, `canonical.rs`, the spec or the golden
  vectors
- touching the security-sensitive files (§7) in a way that changes
  behaviour, not just layout
- changing anything a member sees as policy: who can join, what is shared,
  withdrawal, recovery
- changing user-facing wording that carries meaning (consent, warnings,
  steward actions)
- adding a dependency, a third-party request, or a new ADR-level decision
- deleting data, rewriting git history, pushing to `main`, or handling
  signing secrets (never handle those in chat)

**Decide, then report what you did** for:

- refactors that keep behaviour the same (splitting files, extracting
  functions, renaming locals)
- adding tests and fixing lint
- doc fixes, and CSS changes that only use existing tokens and patterns
- bug fixes with a test that shows the bug

When unsure, make the smallest reversible change and say what you'd do next.
