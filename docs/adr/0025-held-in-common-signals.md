# 0025. Held in Common Signals: signed, in new types `ProfileV2` and `StoryV2`

- **Status:** Accepted (built)
- **Date:** 2026-11-12
- **Builds on:** [0009](0009-signed-format-specification.md) (the signed format), [0020](0020-stories.md) (stories), [0022](0022-the-ramp.md) (the rights panels)

## Context
The Sharing and rights panels had three steps: who can see it, the licence,
and the CC signals (a fixed vocabulary of preferences for datasets and model
training). Phoenix asked for a fourth step, **Held in Common Signals**, with
two answers the CC signals don't cover:

1. **Withdraw even after training**: the person keeps the right to ask that
   their work be taken out of an AI model already trained on it, or that the
   model stop using it.
2. **Their own terms for AI, in their own words**: plain text, up to 1,000
   characters, about their permissions and relationship to AI systems for
   this piece of work.

Phoenix decided these answers are **signed**: they travel inside the signed
copy a node shares, so a commons can't change them, and they are shown
wherever the licence and signals are.

Two constraints shaped how. The signed bytes of what already exists must
never change (`canonical.rs`, its golden test, and the spec's vectors). And
commons are upgraded one at a time, so an older commons must still take
shares from a newer node, and a newer commons must still take older shares.

## Decision
- **Two new `hic/v1` types**, `ProfileV2` and `StoryV2` (spec §5.5). Each is
  the older type's body plus one object, `held_signals`:
  `{ "withdraw_after_training": true|false, "own_terms": "…" }`. The
  envelope, canonical JSON, cid and signing input are unchanged; "V2" is the
  version of the *body*, not of the envelope.
- **A node uses them only when something is chosen.** With the box unticked
  and the text empty, it sends `Profile` and `Story` exactly as before, byte
  for byte. So the upgrade changes nothing on the wire for anyone who doesn't
  use the step.
- **Fallback for older commons.** An older commons refuses a type it doesn't
  know with HTTP 400 (spec §4: "refuse types they don't understand rather
  than guess"). The node then sends the next older format: `ProfileV2` →
  `Profile` → version 0 (`federation::share_attempts`), and `StoryV2` →
  `Story` (`story::older_copy`, the same story signed again with the same
  date). The member is told, on the confirmation page, that their Held in
  Common Signals didn't travel to that commons and will go with the next
  share once it's updated.
- **Newer commons accept older shares.** A `Profile`, `Story` or version-0
  share carries no `held_signals`; that reads as "not chosen" (false, empty).
- **Strict on receipt.** A V2 object must carry `held_signals` with exactly
  those two keys, a boolean and a string of at most 1,000 characters; an
  older type must carry none. Anything else is refused (400), not guessed.
- **A separate field, not a new CC signal key.** "Withdraw even after
  training" is a field of `held_signals`, not a key in `governance::SIGNALS`.
- **On the node**, a profile's answers are stored beside its version
  (`Version::held_signals`, `#[serde(default)]`), because the version-0 cid
  and signature can't cover new fields. They are signed when shared. A
  story's answers are a field of the story, so they are in its signature
  from the start.
- **Plain text, limited on the server.** Own terms are trimmed, Windows line
  endings made `\n`, then counted in characters. Over 1,000 is refused with
  "Keep your own terms for AI to 1,000 characters or fewer." rather than
  cut, so nobody's words change without them seeing it. Pages escape them
  (maud) and keep line breaks.
- **Where they show:** step 4 of the rights panels (How it travels is now
  step 5); the "What travels with every copy" summary, kept current by
  `/rights.js`; profile pages on the node and on a commons (in the terms
  line, and the own terms in full); story pages; story cards (as chips);
  `/api/@{handle}` and `/api/directory`. A members-only profile shows its
  own terms only to those who can read the profile.

## Why
**Why a separate field and not a new signal key.** Stories refuse unknown
signal keys (`story::check`), and they have no older format to fall back to
inside the `Story` type. A new key in `signals` would make an older commons
refuse the whole story with no way round it. As a separate field in a new
type, an older commons refuses the type, the node falls back, and the story
still arrives. The meaning also differs: CC signals are preferences about
how work is used to *make* a model; this is a right the person *keeps* after
it's made. Keeping
them apart keeps each one's meaning plain.

**Why new types and not optional fields in the old ones.** The spec allows
new optional body fields within `hic/v1`, and an older commons would accept
them, but it would keep the signed copy and show the profile or story
*without* the person's statement about their rights, and nobody would know.
For a statement of rights, being silently dropped is worse than being
refused. A new type makes an older commons say "no", which the node can act
on and tell the member about.

**Why not `hic/v2`.** The spec reserves a new schema for changes to the
canonical bytes, cid or signatures. None of those change here; only the body
of two types does.

**Why only when chosen.** It keeps everything that worked before working
the same way, it keeps the published vectors and golden test untouched, and
it means a commons that hasn't upgraded only sees a difference from people
who have chosen something.

## Consequences
- During an upgrade, a member with Held in Common Signals who shares to an
  older commons is listed there without them, and is told so. Re-sharing
  after that commons upgrades sends them.
- The profile's answers on a node sit beside a version signed in version 0,
  unsigned there. That is the node's own disk, where the member's private
  key already lives; the signed statement is the `ProfileV2` a commons holds.
  `/api/@{handle}` on a node reports them as the node's statement.
- Two versions published in the same second that differ only in these
  answers share a version-0 cid, so the second replaces the first on the
  node. Publishing is a person pressing a button, so this is unlikely.
- A future Held in Common Signal needs a new V-type (or a deliberate change
  to this record), because commons refuse keys they don't know.
- The text box asks for plain text. Nothing stops someone writing a URL or
  an email address; it is shown as text, not as a link.
- Tests: unit tests in `held_signals.rs`, `federation.rs`, `story.rs` and
  the pages; HTTP tests in `commons/tests/http/held_signals.rs`; vectors
  `07-profile-v2.json` and `08-story-v2.json`, checked by
  `cargo test --test spec_vectors` and `scripts/check_spec_vectors.py`;
  and checks in `scripts/test-federation.sh`.
