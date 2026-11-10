
## Run a node with Docker

Prebuilt image (published by CI to GHCR, multi-arch amd64/arm64):

```
docker run -d --name hic-node -p 8080:8080 -v hic-node:/data \
  -e HIC_INSTANCE="My node" \
  -e HIC_SHARE_DEFAULT="https://<the-shared-commons>" \
  ghcr.io/<owner>/hic-node:latest
```

Then open http://localhost:8080 — you're a node. (The image sets `HIC_BIND=0.0.0.0` so Docker's port mapping can reach it.) Publish locally, then
"Share to a commons" to federate. Your data lives in the `hic-node` volume.

Build it yourself instead:

```
docker build -t hic-node ./commons
docker run -d -p 8080:8080 -v hic-node:/data hic-node
```

## Run a shared commons (federation target)

The same binary becomes a **commons** — a shared directory others federate into —
when `HIC_ROLE=commons`. The role is saved in `instance.json` in the data folder
the first time it starts, and is fixed for that folder: starting the same folder
with a different `HIC_ROLE` refuses to run. (Older setups that relied on
`HIC_INSTANCE` containing "commons" or "federated" still work, with a
deprecation warning. Set `HIC_ROLE` explicitly.)

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

### Who can join a commons

A commons lists a new person only when someone already there vouches for
them (decision record `docs/adr/0011-joining-needs-a-vouch.md`). When a new
commons starts it prints a one-time **founding invite** in its log (also in
`founding-invite.json` in the data folder, and on the commons' page when
opened from the same computer). Give it to the first member; after that,
members bring people in with invite codes or vouches from their own nodes.

- `HIC_VOUCH_LIMIT=5`: how many people one member may bring in per week.
- `HIC_OPEN_COMMONS=1`: no vouching (for example a test commons).

## Run a test commons with Docker (for development)

Use this to test your node against a commons on your own computer. It needs
Docker Desktop running.

1. **Build the image** from the code you have checked out (repeat after
   pulling new code):
   ```
   docker build -t hic-commons-test ./commons
   ```
2. **Start the commons** on port 8081 (your node app uses 8080):
   ```
   docker run -d --name hic-test-commons -p 8081:8080 \
     -v hic-test-commons-data:/data \
     -e HIC_ROLE=commons -e HIC_INSTANCE="Test commons (Docker)" \
     -e HIC_BASE_URL=http://localhost:8081 \
     hic-commons-test
   ```
   For a commons where anyone can share without a vouch, add
   `-e HIC_OPEN_COMMONS=1` before the image name.
3. **Get the founding invite** (a new commons only; one use):
   ```
   docker logs hic-test-commons | grep founding
   ```
   The commons page shows it only to someone at the commons' own computer,
   and Docker doesn't count as that, so read it from the log.
4. **Open the commons** at http://localhost:8081.
5. **Share from your node:** open your profile, under **Share to a commons**
   open **Add a commons**, enter `http://localhost:8081`, paste the founding
   invite into **Invite code**, and press **Share**.

**Stop it:** `docker stop hic-test-commons` (start again with
`docker start hic-test-commons`; its data stays).
**Start over:** `docker rm -f hic-test-commons && docker volume rm hic-test-commons-data`,
then repeat step 2 (a new founding invite appears).
