
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
