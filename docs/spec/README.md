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

Check them:

```text
cargo test --manifest-path commons/Cargo.toml --test spec_vectors   # everything, including signatures
python3 scripts/check_spec_vectors.py                              # canonical bytes, cid and signing input, independently
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

### 5.2 Planned

Dataset listings, rights records, licences, adapters, model versions,
vouches and governance decisions will be added as `hic/v1` types as they are
built (see `docs/REVIEW-2026-11.md` §4 and decision record 0011).

## 6. Version 0 (profiles and withdrawals, before `hic/v1`)

Profiles and withdrawals predate the envelope. They remain valid, and new
profiles still use this format until profiles move to `hic/v1`.

**Profile version.** The cid is the SHA-256 of the compact JSON

```json
{"handle":…,"visibility":…,"license":…,"signals":[…],"created_at":…,"body":{…}}
```

with the six top-level fields **in that order** (not sorted), and `body` the
profile **without** `avatar`, its keys sorted. This happens to equal RFC 8785
output for the body, but the top level is not sorted. The signature is Ed25519
over the cid **text** (64 hex characters), with no type prefix.
`commons/src/canonical.rs` holds a golden value.

Known limits of version 0, which `hic/v1` fixes: the signature doesn't say
what kind of object it covers; the avatar, display name, node address, node
name and encryption key travel alongside but aren't signed; there is no
`prev` link.

**Withdrawal.** Ed25519 over the text
`hic/v1/withdraw\n<handle>\n<commons host>\n<created_at>`, sent as
`{"type":"Withdraw","actor":{…},"object":{"handle","audience","createdAt"},"signature":{…}}`.
It moves to a `hic/v1` `Withdraw` type with the profile migration.
