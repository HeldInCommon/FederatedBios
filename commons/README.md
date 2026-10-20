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
