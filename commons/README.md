# hic-commons

The Held in Common **commons server**: creator identity (human handles, keypairs
held server-side), content-addressed publishing, three visibility tiers
(public / commons / members-only — the last **encrypted to the member set**,
"Option B"), Creative-Commons license + CC-signal governance, the public
directory and per-creator profile pages, a JSON API, and ActivityPub-ready
endpoints (WebFinger + actor + outbox).

## Run

```
cargo run            # serves http://localhost:8080
HIC_DATA=./data PORT=8080 HIC_BASE_URL=https://your.host cargo run --release
```

Storage is the filesystem under `$HIC_DATA` (default `./data`):
`handles/<handle>.json` (identity + current pointer) and
`versions/<cid>.json` (immutable, content-addressed).

Env: `PORT`, `HIC_DATA`, `HIC_BASE_URL` (used for absolute links + WebFinger/actor ids).

## Federation (node ↔ commons)

The same binary is both a **node** (an artist's personal commons) and a **shared
commons** that can receive federated shares. Run it on your own machine to be a
node; point others at your URL to be their commons.

- **Share out:** on your profile, "Share to a commons" pushes a signed,
  ActivityPub-shaped `Create` to `<commons>/inbox`. The payload embeds your
  Ed25519 public key, the profile, its content-address (`cid`) and a signature.
- **Receive:** `POST /inbox` recomputes the `cid` from the content (tamper
  check) and verifies the Ed25519 signature, then TOFU-pins the key to the
  (handle, origin) pair and lists it in the directory, marked "federated from …".
- **Members-only never federates** — it stays encrypted on your node.

Env: `HIC_INSTANCE` (human label for this instance), `HIC_SHARE_DEFAULT`
(prefilled commons URL in the share form).
