
## Run a node with Docker

Prebuilt image (published by CI to GHCR, multi-arch amd64/arm64):

```
docker run -d --name hic-node -p 8080:8080 -v hic-node:/data \
  -e HIC_INSTANCE="My node" \
  -e HIC_SHARE_DEFAULT="https://<the-shared-commons>" \
  ghcr.io/<owner>/hic-node:latest
```

Then open http://localhost:8080 — you're a node. Publish locally, then
"Share to a commons" to federate. Your data lives in the `hic-node` volume.

Build it yourself instead:

```
docker build -t hic-node ./commons
docker run -d -p 8080:8080 -v hic-node:/data hic-node
```

## Run a shared commons (federation target)

The same binary becomes a **commons** — a shared directory others federate into —
when `HIC_ROLE=commons` (also auto-inferred when `HIC_INSTANCE` contains
"commons" or "federated"):

```
docker run -d --name hic-commons -p 8080:8080 -v hic-commons:/data \
  -e HIC_ROLE=commons \
  -e HIC_INSTANCE="Held in Common (federated)" \
  -e HIC_BASE_URL="https://<your-public-host>" \
  ghcr.io/<owner>/hic-node:latest
```

On a commons there is **no sign-up**: `/join` and `/login` show a "run a node and
share" page instead of minting an identity. Profiles appear only as signed copies
posted to `/inbox` by people's own nodes. Keys never live on the commons. Set
`HIC_BASE_URL` to the public https address so federation and the `Secure` session
cookie work correctly.
