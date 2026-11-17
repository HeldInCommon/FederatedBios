# ActivityPub vs Held in Common: how each federates

This document compares ActivityPub with the federation in Held in Common
(`HeldInCommon/FederatedBios`, version 0.9.3), one concern at a time. Each section
shows the ActivityPub way first, then the Held in Common way, then what differs.
The Held in Common diagrams follow `docs/ARCHITECTURE.md`, `docs/spec/README.md` and
`commons/src/web/handlers/inbox.rs`.

## Contents

1. [At a glance](#1-at-a-glance)
2. [Shape of the network](#2-shape-of-the-network)
3. [Identity and discovery](#3-identity-and-discovery)
4. [Verifying a message](#4-verifying-a-message)
5. [Connecting: following vs joining](#5-connecting-following-vs-joining)
6. [Publishing and delivery](#6-publishing-and-delivery)
7. [The inbox](#7-the-inbox)
8. [Changing and removing things](#8-changing-and-removing-things)
9. [Only in Held in Common](#9-only-in-held-in-common)
10. [Where the two meet](#10-where-the-two-meet)

---

## 1. At a glance

| Concern | ActivityPub | Held in Common |
|---|---|---|
| Who runs a server | Instance admins, hosting many users | Everyone runs a **node** on their own computer; anyone can host a **commons** |
| Direction of traffic | Server to server, both ways | Node to commons only; a commons never contacts a node |
| Identity | Actor URL on a server; the server holds the keys | Ed25519 key made and held on the person's node; a handle belongs to the first key to use it on each commons |
| What is signed | The HTTP request (HTTP Signatures) | The object itself, by its content address, so stored copies stay verifiable |
| Getting the key | Receiver fetches it from the actor's `keyId` | Travels inside the signed object (`envelope.author`) |
| Audience | Follow graph; delivery fans out to followers | No follows; the owner picks which commons get a copy |
| Admission | Open by default, moderation after | New handles need an invite, a vouch or steward approval |
| Updates | `Update` replaces the object | Each version is new and signed, and must be strictly newer than what the commons holds |
| Deletion | `Delete`, best effort | `Withdraw` leaves a tombstone that blocks replay |
| Private content | Addressed delivery (`to`/`cc`) to chosen inboxes | Members-only work is encrypted and never leaves the node |
| Content not held | Not a concept | "Only while I'm here": the commons holds a signed card and fetches the text from the node each time |

---

## 2. Shape of the network

### ActivityPub

Every server can push to every other server's inboxes, and pull objects by URL.

```mermaid
flowchart LR
    subgraph ServerA["Server A"]
        UA["Alice"] --> OA["Alice's outbox"]
        IA["Alice's inbox"] --> TA["Alice's timeline"]
    end
    subgraph ServerB["Server B"]
        UB["Bob"] --> OB["Bob's outbox"]
        IB["Bob's inbox"] --> TB["Bob's timeline"]
    end
    OA -- "signed POST of activity" --> IB
    OB -- "signed POST of activity" --> IA
    IA -. "GET actor / objects (dereference by URL)" .-> OB
    IB -. "GET actor / objects" .-> OA
```

### Held in Common

The same binary runs as a node or a commons. Nodes listen only on their own computer
(`127.0.0.1`), so all traffic starts at the node. Readers browse a commons.

```mermaid
flowchart LR
    NA["Alice's node (her computer, 127.0.0.1)"] -- "signed Share / Withdraw / Vouch to /inbox" --> CX["Commons X (run by a steward)"]
    NA -- "signed Share" --> CY["Commons Y"]
    NB["Bob's node"] -- "signed Share" --> CX
    NA -. "node asks: GET /api/status, POST /api/waiting" .-> CX
    NB -. "long poll /here/poll" .-> CX
    RD["Reader (browser)"] -- "GET /, /@handle" --> CX
```

### What differs

ActivityPub is a mesh of peers. Held in Common is a star around each commons with
one-way arrows: anything a node needs to hear (that it was listed, that someone wants
to read a story) it gets by asking. This is why a node can sit behind a home router
with no open ports.

---

## 3. Identity and discovery

### ActivityPub

A user is an actor document at a URL on their server. Other servers find it through
WebFinger and read the public key from it.

```mermaid
sequenceDiagram
    autonumber
    participant A as Server A
    participant B as Server B

    A->>B: GET /.well-known/webfinger?resource=acct:bob@b.example
    B-->>A: JRD with link rel="self" → https://b.example/users/bob
    A->>B: GET /users/bob (Accept: application/activity+json)
    B-->>A: Actor JSON (inbox, outbox, followers, publicKey)
    A->>A: Cache actor and public key
```

### Held in Common

A person's identity is a key pair made on their own node. A handle is claimed per
commons, by the first key that shares under it there.

```mermaid
flowchart TD
    J["Join on the node: choose handle and passphrase"] --> K["Node makes Ed25519 signing key and X25519 encryption key"]
    K --> RC["Show recovery code once, to keep offline"]
    K --> H["Store handles/HANDLE.json (keys, Argon2id passphrase hash)"]
    H --> S["First share to a commons carries the public key inside the signed Profile"]
    S --> Q{"Does this commons already hold the handle?"}
    Q -- "No" --> CL["Handle recorded against this key (identities/HANDLE.json)"]
    Q -- "Yes, same key" --> OK["Returning member"]
    Q -- "Yes, other key" --> NO["409: handle taken here"]
```

### What differs

In ActivityPub the server is the identity's home and holds its keys; moving server
means a new identity. In Held in Common the key is the identity and lives with the
person; backups, key rotation and a recovery key carry it between computers, and the
same handle can mean different people on different commons.

---

## 4. Verifying a message

### ActivityPub

The signature covers the HTTP request. The receiver has to fetch the sender's key.

```mermaid
flowchart TD
    R["Incoming POST"] --> SH["Read Signature header: keyId, signed headers, signature"]
    SH --> F["Fetch actor at keyId (network call, or cache)"]
    F --> PK["Take publicKeyPem"]
    PK --> SS["Rebuild signing string from the listed headers"]
    SS --> V{"Signature valid?"}
    V -- No --> X["Reject"]
    V -- Yes --> D{"Digest matches body, Date is fresh?"}
    D -- No --> X
    D -- Yes --> AC{"activity.actor owns keyId?"}
    AC -- No --> X
    AC -- Yes --> OK["Accept. The proof ends with the request"]
```

### Held in Common

The signature covers the object's content address. Everything needed to check it is
inside the object (spec §2).

```mermaid
flowchart TD
    R["Incoming signed object: envelope, cid, sig, cosigs"] --> SC{"envelope.schema is hic/v1?"}
    SC -- No --> X["Reject"]
    SC -- Yes --> CJ["Canonical JSON of envelope (RFC 8785)"]
    CJ --> H{"SHA-256 equals cid?"}
    H -- No --> X
    H -- Yes --> SI["Signing input: hic/v1/TYPE newline cid"]
    SI --> V{"sig valid for envelope.author?"}
    V -- No --> X
    V -- Yes --> CS{"Every cosignature valid?"}
    CS -- No --> X
    CS -- Yes --> TR["Apply the type's rules (handle owner, newer, vouch...)"]
    TR --> ST["Store the signed object itself as proof"]
```

### What differs

ActivityPub proves *who sent this request*; Held in Common proves *who made this
object*. Because the type is part of the signing input, a signature for one kind of
object can't be passed off as another. A copy held on a commons can be re-checked by
anyone later, and a commons can't alter a profile without breaking its signature.

---

## 5. Connecting: following vs joining

### ActivityPub: following someone

```mermaid
sequenceDiagram
    autonumber
    actor Alice
    participant A as Server A
    participant B as Server B
    actor Bob

    Alice->>A: Follow bob@b.example
    A->>B: POST /users/bob/inbox (Follow, signed)
    B->>B: Verify signature (section 4)
    alt Auto-accept
        B->>B: Add Alice to Bob's followers
    else Manual approval
        B->>Bob: Show follow request
        Bob->>B: Approve
        B->>B: Add Alice to Bob's followers
    end
    B->>A: POST /users/alice/inbox (Accept{Follow}, signed)
    A-->>Alice: Now following Bob
```

### Held in Common: joining a commons

There are no follows. The relationship is between a person and a commons, and a new
handle is listed only once someone already there vouches for it.

```mermaid
sequenceDiagram
    autonumber
    actor Bob as Bob (newcomer)
    participant NB as Bob's node
    participant C as Commons
    participant NM as Member's node
    actor M as Member
    actor S as Steward

    NB->>C: POST /inbox Share (no invite)
    C-->>NB: 202 pending
    alt A member vouches (ADR 0026)
        M->>NM: Open the People tab
        NM->>C: POST /api/waiting with signed WaitingListRequest
        C->>C: Signer is listed here, names this commons, dated within 5 min
        C-->>NM: waiting: handle, name, key (no profile content)
        M->>NM: Press Vouch beside Bob
        NM->>C: POST /inbox signed Vouch {commons, handle, key}
        C->>C: Voucher is a member, under weekly limit, handle and key match pending
        C->>C: List Bob's pending share, record intro with the vouch as proof
        C-->>NM: 202
    else The steward approves (ADR 0017)
        S->>C: Steward view, press Approve
        C->>C: List Bob's pending share
    end
    Note over C,NB: The commons cannot contact Bob's node
    NB->>C: GET /api/status (at startup or on Check now)
    C-->>NB: listed
    NB-->>Bob: Banner on his pages and a macOS notification
```

Other routes in: a member's invite code, a steward's invite code, the commons' one-time
founding invite, or a room code on a local commons. `HIC_OPEN_COMMONS=1` turns the
gate off.

### What differs

A follow is person-to-person and decides whose posts you receive. Joining is
person-to-community and decides whether you are listed at all. ActivityPub tells the
follower through an `Accept`; Held in Common can't push, so the node polls
`/api/status`.

---

## 6. Publishing and delivery

### ActivityPub

```mermaid
sequenceDiagram
    autonumber
    actor Bob
    participant B as Server B (origin)
    participant Q as B's delivery queue
    participant A as Server A
    participant C as Server C

    Bob->>B: Write a post
    B->>B: Create Note + wrap in Create activity
    B->>B: Store in Bob's outbox
    B->>B: Resolve audience (to/cc: followers, Public, mentions)
    B->>Q: Enqueue one job per unique inbox (sharedInbox where available)
    par Deliver to A
        Q->>A: POST /inbox (Create{Note}, signed)
        A->>A: Verify, store Note, add to followers' timelines
        A-->>Q: 202 Accepted
    and Deliver to C
        Q->>C: POST /inbox (Create{Note}, signed)
        C-->>Q: 5xx / timeout
        Q->>Q: Retry with exponential backoff
    end
```

### Held in Common

```mermaid
sequenceDiagram
    autonumber
    actor Alice
    participant N as Alice's node (127.0.0.1)
    participant C as Commons

    Alice->>N: Publish profile (reach, licence, CC signals, Held in Common Signals)
    N->>N: Build version, cid = SHA-256 of canonical JSON, sign with Ed25519
    N->>N: Store versions/CID.json
    alt Reach is Members only
        N->>N: Encrypt one copy per member (crypto_box). Never leaves the node
    else Reach is Public or Commons
        Alice->>N: Share to a commons (optionally paste an invite code)
        N->>N: Refuse internal addresses if networked (ADR 0012)
        N->>C: POST /inbox {type: Share, profile: signed ProfileV2, recovery, invite}
        opt Commons is older and answers 400
            N->>C: Retry as hic/v1 Profile, then as version 0
        end
        C->>C: Verify (section 4), then admit (section 7)
        C-->>N: 202 accepted, 202 pending, or 409
    end
```

### What differs

ActivityPub publishes once and the server fans out to every follower's inbox, retrying
in the background. Held in Common shares are explicit, one commons at a time, chosen
by the person, and the licence and signals travel signed inside every copy. Sharing
also degrades gracefully: a node steps down through older formats when a commons
hasn't been upgraded.

---

## 7. The inbox

### ActivityPub

```mermaid
flowchart TD
    A["POST to /inbox or /sharedInbox"] --> B{"Signature valid? (section 4)"}
    B -- No --> R1["Reject 401"]
    B -- Yes --> G{"Actor or domain blocked?"}
    G -- Yes --> R2["Drop / 403"]
    G -- No --> H["Return 202, enqueue for processing"]
    H --> I{"Activity type"}
    I -- Follow --> J["Record follow, send Accept or wait for approval"]
    I -- "Accept / Reject" --> K["Update pending follow state"]
    I -- Create --> L["Store object, attach to timelines / notifications"]
    I -- Update --> M["Check actor owns object, then replace"]
    I -- Delete --> N["Check ownership, tombstone object"]
    I -- "Like / Announce" --> O["Record reaction or boost, notify author"]
    I -- Undo --> P["Reverse the referenced activity"]
    I -- Unknown --> Q["Ignore safely"]
```

### Held in Common (commons only)

Follows the dispatch order in `inbox.rs`.

```mermaid
flowchart TD
    A["POST /inbox"] --> B{"Under 120 per minute for this IP?"}
    B -- No --> R429["429 too many requests"]
    B -- Yes --> P{"Valid JSON?"}
    P -- No --> R400a["400 invalid json"]
    P -- Yes --> T{"type field"}
    T -- Withdraw --> W["Check signature over handle + this host + created_at"]
    W --> W2["Delete copy, keep tombstone, delete their stories"]
    T -- StoryShare --> ST["Verify Story, author must be listed under that handle, must be newer"]
    T -- StoryWithdraw --> SW["Delete story, record the withdrawal"]
    T -- "anything else" --> E{"Bare signed envelope?"}
    E -- Yes --> ID["Identity change: Vouch, RecoveryKey, KeyRotation, RecoveryRotation"]
    E -- No --> V{"Verify share: ProfileV2, Profile or version 0"}
    V -- Fails --> R400b["400 rejected"]
    V -- OK --> AD{"admit()"}
    AD -- "Handle held by another key" --> R409a["409"]
    AD -- "Older than held, or than a withdrawal" --> R409b["409"]
    AD -- Accept --> RET{"Returning key or open commons?"}
    RET -- Yes --> L1["List: store copy and identity, 202"]
    RET -- No --> INV{"Invite, founding invite or room code?"}
    INV -- None --> PEND["Pending list, 202"]
    INV -- Valid --> L2["List and record intro, 202"]
    INV -- Invalid --> R403["403"]
```

### What differs

ActivityPub accepts first and processes later, ignoring types it doesn't know. Held in
Common decides synchronously and answers with a meaningful status, and refuses types
it doesn't understand rather than guessing (spec §4). Unknown formats get a 400,
which is what tells the node to fall back to an older one.

---

## 8. Changing and removing things

### ActivityPub: update and delete

```mermaid
sequenceDiagram
    autonumber
    actor Bob
    participant B as Server B
    participant A as Server A
    participant C as Server C (missed it)

    Bob->>B: Edit post
    B->>A: POST /inbox Update{Note}, signed
    A->>A: Check actor owns the Note, replace it
    Bob->>B: Delete post
    B->>A: POST /inbox Delete{Note}, signed
    A->>A: Replace Note with a Tombstone
    B--xC: Delivery fails after retries
    Note over C: C keeps the old copy. Nothing stops a replay of the old Create
```

### Held in Common: withdrawal and key replacement

```mermaid
sequenceDiagram
    autonumber
    actor Alice
    participant N as Alice's node
    participant C1 as Commons X
    participant C2 as Commons Y

    Note over Alice,C2: Withdrawing a copy
    Alice->>N: Withdraw from Commons X
    N->>C1: POST /inbox Withdraw (signed over handle, this host, created_at)
    C1->>C1: Delete the copy and her stories, keep tombstones/KEYID.json
    C1-->>N: 200 withdrawn
    N->>C1: (replay) older Share
    C1-->>N: 409 older than a withdrawal

    Note over Alice,C2: Replacing a key (ADR 0010)
    Alice->>N: Replace my key
    N->>N: Make new keys, sign KeyRotation with old key, cosign with new key
    par Every commons shared to
        N->>C1: POST /inbox KeyRotation
        C1->>C1: Newer than last rotation, move handle to new key, drop stories signed by old key
        C1-->>N: 200 applied
    and
        N->>C2: POST /inbox KeyRotation
        C2-->>N: 200 applied
    end
    N->>N: Change stays pending until all accept
    opt Old key was stolen
        Alice->>N: Recover with the offline recovery code
        N->>C1: RecoveryRotation (signed by recovery key, overrides rotations made with the stolen key)
    end
```

### What differs

ActivityPub edits replace an object in place, and deletes are best effort. Held in
Common versions are immutable; "newer" is enforced on every commons, and a withdrawal
leaves a tombstone so an old copy can't be sent again. ActivityPub has no standard way
to move an identity to a new key; Held in Common has rotation, plus a recovery key
that outranks a stolen one.

---

## 9. Only in Held in Common

### "Only while I'm here" stories

The commons holds a signed card but not the text, and fetches the text through a
connection the author's node keeps open.

```mermaid
sequenceDiagram
    autonumber
    participant NA as Author's node
    participant C as Commons
    actor R as Reader

    NA->>C: POST /here/card StoryCardShare (signed: title, first line, terms, text_sha256)
    C->>C: Same gate as stories, store card only, no text
    loop Long poll, held about 25s
        NA->>C: POST /here/poll signed StoryRelay (seq n)
        C-->>NA: requests: [] when hold time ends
    end
    R->>C: Open the story
    alt Author's node is connected
        C-->>NA: Answer open poll with requests: [request id, story id]
        NA->>C: POST /here/poll StoryRelay (seq n+1, answers with the text)
        C->>C: seq is higher, dated within 300s, SHA-256 of text equals card
        C-->>R: Story text (passed through, not stored)
    else Node is away
        C-->>R: "Not here right now" after about 8s
    end
```

### Members-only encryption

```mermaid
flowchart LR
    V["Members-only version"] --> E["Encrypt once per member: crypto_box (X25519 + XChaCha20-Poly1305), one-time key per copy"]
    E --> S["Stored on the node only"]
    S --> NS["Never shared to any commons"]
```

ActivityPub's closest equivalent is addressing a post to specific inboxes, which still
hands a plaintext copy to every receiving server.

---

## 10. Where the two meet

Held in Common exposes read-only, ActivityStreams-shaped discovery so other tools can
find a member: `GET /.well-known/webfinger`, `GET /ap/@HANDLE` and
`GET /ap/@HANDLE/outbox`. It does not accept ActivityPub activities, send them, or
take part in follows, so a Mastodon server can look someone up but can't federate
with a commons.
