# Held in Common signed formats

This is the contract for everything Held in Common hashes and signs. Any
program in any language that follows it produces the same identifiers and
signatures, and can check ours. The code (`commons/src/jcs.rs`,
`commons/src/envelope.rs`) implements this document; where they disagree, fix
whichever is wrong and add a test vector. Why this exists:
[decision record 0009](../adr/0009-signed-format-specification.md).

The key words MUST, MUST NOT and SHOULD are used as in RFC 2119.

## 1. Keys and encodings

- Signing keys are **Ed25519** (RFC 8032). Encryption keys are **X25519**.
- Keys, hashes and signatures are written as **lowercase hexadecimal**.
- A **cid** (content identifier) is the lowercase hex SHA-256 of a byte string, 64 characters.

## 2. Format `hic/v1`: the envelope

Every `hic/v1` object is an envelope:

```json
{
  "schema": "hic/v1",
  "type": "KeyRotation",
  "author": "<Ed25519 public key of the signer>",
  "created_at": 1700000100,
  "prev": null,
  "body": { "handle": "tega", "new_key": "…", "new_box_key": "…" }
}
```

| Field | Meaning |
|---|---|
| `schema` | MUST be `"hic/v1"`. |
| `type` | What the object is (section 5). |
| `author` | The public key whose signature is required. |
| `created_at` | Seconds since 1970-01-01 UTC, as the author's clock says. |
| `prev` | The `cid` of the author's previous object in the same chain, or `null`. Links an author's objects into a tamper-evident history. |
| `body` | The type-specific content. |

**Allowed JSON values:** strings, integers between −(2⁵³−1) and 2⁵³−1,
`true`, `false`, `null`, arrays and objects. Fractions and larger integers
MUST NOT appear anywhere in an envelope. (They would make the canonical form
depend on floating-point formatting.)

### 2.1 Canonical bytes

The envelope is written as canonical JSON by **RFC 8785** (the JSON
Canonicalization Scheme):

1. no whitespace;
2. object keys sorted by their **UTF-16 code units** (this differs from byte order only for characters outside the Basic Multilingual Plane, such as emoji; see vector 02);
3. strings: `"` and `\` escaped; `\b \f \n \r \t` in their short forms; other characters below U+0020 as `\u00xx` with lowercase hex; everything else (including `/` and non-ASCII) as raw UTF-8;
4. integers as plain decimals.

### 2.2 The cid

`cid = hex(SHA-256(canonical bytes))`.

### 2.3 Signatures

The **signing input** is the UTF-8 text

```text
hic/v1/<type>\n<cid>
```

(a newline between the type and the cid). The author's Ed25519 signature over
that text, in hex, is `sig`. Including the type means a signature for one
kind of object can't be presented as another kind.

A **cosignature** is another key's signature over the same signing input.

### 2.4 The signed object, as stored and sent

```json
{ "envelope": { … }, "cid": "…", "sig": "…", "cosigs": [ { "key": "…", "sig": "…" } ] }
```

`cosigs` MAY be omitted when empty. To **verify**: check `schema`, recompute
the cid from `envelope` and compare, check `sig` against `author`, then check
every cosignature against its `key`.

## 3. Test vectors

`docs/spec/vectors/*.json` each hold an envelope, its canonical text, cid,
signing input and the full signed object, made with **published test keys**
(in each file, never to be used for anything real). Ed25519 signatures are
deterministic, so an implementation MUST reproduce them exactly.

| Vector | What it exercises |
|---|---|
| `01-basic.json` | The smallest useful object. |
| `02-ordering-and-escaping.json` | UTF-16 key order, escaping, negative integers, booleans, null, a non-null `prev`. |
| `03-key-rotation.json` | A real type with a cosignature. |
| `04-profile.json` | A shared profile, picture included. |
| `05-story.json` | A story, with its terms. |
| `06-story-withdraw.json` | Withdrawing a story from one commons. |
| `07-story-card.json` | An "Only while I'm here" card for the story in 05: no text, its SHA-256. |
| `08-story-relay.json` | A node's poll answering one request with that text (it must match 07's hash). |

Check them:

```text
cargo test --manifest-path commons/Cargo.toml --test spec_vectors   # everything, including signatures
python3 scripts/check_spec_vectors.py                              # canonical bytes, cid, signing input and the card hash, independently
```

## 4. Versions and change

- A change that alters any canonical bytes, cid or signature needs a **new
  schema** (`hic/v2`). Old schemas stay verifiable; objects are never
  rewritten in a newer schema.
- New `type`s and new optional body fields MAY be added within `hic/v1`.
  Implementations MUST refuse types they don't understand rather than guess.

## 5. Types

### 5.1 Identity (decision record 0010)

A commons keeps, per handle, the key that holds it, its encryption key and
(once declared) a recovery key.

| Type | `author` | `body` | Cosigned by | Rule on a commons |
|---|---|---|---|---|
| `RecoveryKey` | the handle's current key | `handle`, `recovery_key` | — | Recorded only if none is recorded yet. |
| `KeyRotation` | the handle's current key | `handle`, `new_key`, `new_box_key` | `new_key` | Moves the handle to `new_key`. `created_at` must be later than the last rotation and recovery. |
| `RecoveryRotation` | the handle's recovery key | `handle`, `new_key`, `new_box_key`, `new_recovery_key` | `new_key` | Moves the handle to `new_key` and sets the next recovery key. `created_at` is compared only with earlier recoveries, so a rotation made with a stolen key can't block it. |

For all three: changes dated more than one day ahead of the commons' clock
are refused, and re-sending an accepted change is harmless.

A node sends these to a commons' `/inbox` as the signed object (2.4). A
`RecoveryKey` declaration also travels inside a profile share, as the field
`recovery`.

### 5.2 Profile shares

A node shares a profile to a commons as

```json
{ "type": "Share", "schema": "hic/v1", "profile": <signed Profile>, "recovery": <signed RecoveryKey, optional> }
```

| `type` | `author` | `body` |
|---|---|---|
| `Profile` | the handle's key | `handle`, `display_name`, `visibility` (`public` or `commons`), `license`, `signals`, `profile` (`name`, `pronouns`, `role`, `bio`, `places`, `avatar`), `box_key`, `origin`, `origin_label`, `version_cid` |

`created_at` is the version's own time, so the same version always produces
the same object; `version_cid` names the node's stored version (section 6).
Every field is signed, including the picture and the node's address and
name, which version 0 left unsigned. (The address and name are still the
node's own claims; the signature means nobody else changed them.)

A commons MUST refuse `members` visibility, MUST refuse a share that isn't
newer than the one it holds for that key, and keeps the signed object as
proof. A node that gets HTTP 400 from a commons SHOULD retry in version 0, so
sharing keeps working while commons are upgraded.

### 5.3 Joining a commons (decision record 0011)

| `type` | `author` | `body` | Use |
|---|---|---|---|
| `Invite` | a member's key | `commons` (host), `code_id` (random hex), `expires_at` | Made in advance and handed to a newcomer as an **invite code**: the signed object as JSON, URL-safe base64 without padding. The newcomer's share carries it as the field `invite`. |
| `Vouch` | a member's key | `commons` (host), `handle`, `key` (the newcomer's) | Sent to the commons' `/inbox` for someone on its pending list. |
| `WaitingListRequest` | a member's key | `commons` (host) | Posted to the commons' `/api/waiting` to ask who is waiting for a vouch (decision record 0026). A request, not a record: it is checked and not kept. |

A commons answers a `WaitingListRequest` only when it is signed by a key it
lists, names this commons, and its `created_at` is within five minutes of the
commons' clock (either way). The answer is
`{"waiting": [{"handle", "name", "key"}, …]}`: no other profile content.

A commons lists a **new** handle only with a valid invite, a valid vouch, or
its one-time **founding invite** (`hic-founding-` followed by hex; not a
signed object, since there is no member yet to sign it). Otherwise the share
waits on the pending list (`GET /api/pending`). An invite or vouch MUST be
for this commons, by a current member, within the member's weekly limit
(`HIC_VOUCH_LIMIT`); an invite MUST be unexpired and unused, and a vouch MUST
name the waiting handle and key. Handles already listed, and returning keys,
need no vouch. Each admission is recorded with its route, voucher and proof.

### 5.4 Stories (decision record 0020)

A **story** is a short piece of writing a member makes on their node. A
node shares it to a commons' `/inbox` as

```json
{ "type": "StoryShare", "schema": "hic/v1", "story": <signed Story> }
```

and withdraws it with

```json
{ "type": "StoryWithdraw", "schema": "hic/v1", "withdraw": <signed StoryWithdraw> }
```

| `type` | `author` | `body` |
|---|---|---|
| `Story` | the member's key | `id` (16 lowercase hex, the same for every version of the story), `handle`, `title` (1–120 characters), `text` (1–12,000 characters), `visibility` (`public` or `commons`), `license`, `signals` |
| `StoryWithdraw` | the key the story was shared under | `story` (its `id`), `commons` (host) |

A commons:

- MUST refuse a story whose author it doesn't list under the story's
  `handle` (joining a commons is the gate, section 5.3);
- MUST refuse `members` visibility, unknown licences and unknown signals;
- keeps one story per (author, `id`): a version that isn't newer than the
  one held, or than a recorded withdrawal, MUST be refused, while re-sending
  the very same object is harmless;
- on `StoryWithdraw` addressed to it, deletes the story and records the
  withdrawal, so the withdrawn object can't be replayed;
- deletes an author's stories when it receives that author's profile
  withdrawal (section 6), and when that author's key is replaced
  (section 5.1): a story's signature can't move to a new key.

Members-only stories stay on the node that wrote them.

**Which host is "this commons".** Withdrawals, invites, vouches and story
withdrawals name the commons they are for by host. A commons MUST accept its
public host (from its base URL), and SHOULD also accept `127.0.0.1:<port>` and
`localhost:<port>` for its own port: a node on the commons' own computer
reaches it that way while the commons announces its network address.

### 5.5 "Only while I'm here" (decision record 0021)

A second way to share a story: the commons keeps a signed **card** (no
text), and fetches the text from the author's node each time someone reads
it, over a connection the node opens. Two new types; nothing in 5.4 changes.

A node shares a card to a commons' **`/here/card`** (not `/inbox`) as

```json
{ "type": "StoryCardShare", "schema": "hic/v1", "card": <signed StoryCard> }
```

and keeps a **long poll** open to `/here/poll`, sending

```json
{ "type": "StoryRelay", "schema": "hic/v1", "relay": <signed StoryRelay> }
```

The commons answers a poll with `{ "requests": [ { "request": "<32 hex>", "story": "<id>" } ] }`
when a reader asks, or with no requests after its hold time (about 25
seconds). The node answers those requests in its next poll.

| `type` | `author` | `body` |
|---|---|---|
| `StoryCard` | the member's key | `id`, `handle`, `title`, `first_line` (at most 140 characters), `words`, `visibility` (`public` or `commons`), `license`, `signals`, `text_sha256` (lowercase hex SHA-256 of the story's `text` as UTF-8), `commons` (host) |
| `StoryRelay` | the member's key | `commons` (host), `seq` (an integer higher than any earlier poll from this key), `answers`: at most 32 of `{ "request", "story", "text" }` |

A commons:

- MUST apply the same gate and terms checks to a card as to a story (5.4),
  and MUST refuse a card addressed to another commons;
- keeps one card *or* one copy per (author, `id`): a newer card replaces a
  copy and a newer copy replaces a card; anything not newer than what it
  holds, or than a recorded withdrawal, MUST be refused;
- MUST NOT store a card's text, and MUST NOT pass a text to a reader unless
  its SHA-256 equals the card's `text_sha256`;
- MUST accept a poll only if it is signed by a key with a card here,
  addressed to this commons, dated within 300 seconds of its clock, and
  numbered higher than the last poll it took from that key; MUST ignore an
  answer to a request it didn't hand to that key;
- withdraws a card with `StoryWithdraw` (5.4), at once.

A reader (or a reader's node) that wants to check the text without trusting
the commons fetches `GET /stories/<keyid>/<id>/here.json`, which returns the
signed card and the text, and compares the text's SHA-256 with the card's
signed `text_sha256`.

A commons that predates this section answers `/here/card` with 404; a node
SHOULD then tell the member that commons doesn't support "Only while I'm
here" yet. Vectors 07 and 08 are a card for the story in 05 and a poll
answering it.

### 5.6 Planned

Dataset listings, rights records, licences, adapters, model versions,
governance decisions will be added as `hic/v1` types as they are
built (see `docs/REVIEW-2026-11.md` §4 and decision record 0011).

## 6. Version 0 (profiles and withdrawals, before `hic/v1`)

Profiles and withdrawals predate the envelope. They remain valid. Nodes still
**store** each profile version in this format (its cid is the `version_cid`
in section 5.2), and send it to commons that don't understand `hic/v1` yet.

**Profile version.** The cid is the SHA-256 of the compact JSON

```json
{"handle":…,"visibility":…,"license":…,"signals":[…],"created_at":…,"body":{…}}
```

with the six top-level fields **in that order** (not sorted), and `body` the
profile **without** `avatar`, its keys sorted. This happens to equal RFC 8785
output for the body, but the top level is not sorted. The signature is Ed25519
over the cid **text** (64 hex characters), with no type prefix.
`commons/src/canonical.rs` holds a golden value.

Known limits of version 0, which `hic/v1` shares fix: the signature doesn't say
what kind of object it covers; the avatar, display name, node address, node
name and encryption key travel alongside but aren't signed; there is no
`prev` link.

**Withdrawal.** Ed25519 over the text
`hic/v1/withdraw\n<handle>\n<commons host>\n<created_at>`, sent as
`{"type":"Withdraw","actor":{…},"object":{"handle","audience","createdAt"},"signature":{…}}`.
It moves to a `hic/v1` `Withdraw` type with the profile migration.
