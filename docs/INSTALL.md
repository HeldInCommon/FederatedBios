# Held in Common — Workshop Install Guide

You're going to run a **node** — your own personal commons — and (if you want)
**share** your profile to a shared commons so others can find it. Your work
stays on your machine unless you choose to send a copy.

Pick whichever path suits you. **Path 1 is the fastest for the workshop.**

---

## Path 1 — Use the shared commons (nothing to install)

1. Open **https://drh1in8ipba68q-8080.proxy.runpod.net**
2. Click **Add your profile**.
3. Choose a **handle** and a **passphrase** (you'll use these to sign in again — there's no recovery, so pick something memorable).
4. Fill in your profile → choose **who can see it** → pick a **license** and **CC signals** → **Publish**.
5. You now appear in the directory. Done.

---

## Path 2 — Run your own node: the app (no Docker) — recommended

1. Download the installer for your computer from the releases page:
   **https://github.com/phoenixperry/FederatedBios/releases/tag/node-app-preview**
   - **macOS (Apple Silicon):** `.dmg`
   - **Windows:** `.msi` or `.exe`
   - **Linux:** `.AppImage` or `.deb`
2. Open it (these preview builds are **unsigned**, so your OS will warn once):
   - **macOS:** right-click the app → **Open** → **Open**.
   - **Windows:** "Windows protected your PC" → **More info** → **Run anyway**.
   - **Linux:** `chmod +x *.AppImage` then run it, or install the `.deb`.
3. The app starts your node and opens to **http://localhost:8080** — that's *your* node.
4. Create your **handle + passphrase**, fill your profile, and **Publish** (it's saved on your machine).
5. To appear in the shared commons: on your profile, use **"Share to a commons"** (the shared address is already filled in) → **Share**. A signed copy now shows in the shared directory.
6. **Members-only** profiles stay **encrypted on your machine** — they are never shared.

---

## Path 3 — Run a node with Docker (technical)

```
docker run -d --name hic-node -p 8080:8080 -v hic-node:/data \
  -e HIC_INSTANCE="My node" \
  -e HIC_SHARE_DEFAULT="https://drh1in8ipba68q-8080.proxy.runpod.net" \
  ghcr.io/phoenixperry/hic-node:latest
```

Open **http://localhost:8080**. Your data lives in the `hic-node` volume.

---

## What the choices mean

- **Visibility** — *Public* (listed + searchable) · *Commons* (listed, kept out of search) · *Members-only* (encrypted; only signed-in members can read it).
- **License** — Creative Commons (CC0, CC BY, …): how people may reuse your work.
- **CC signals** — how AI models may learn from it: *Credit*, *Direct Contribution*, *Ecosystem*, *Open*.

## Troubleshooting

- **Port 8080 is busy** — quit whatever is using it, then reopen the app.
- **"Can't be opened" (macOS)** — right-click the app → **Open** (only needed once).
- **Forgot your passphrase** — there's no recovery (we store only a salted hash); create a new handle.

## How it all fits together (slides)

**https://raw.githack.com/phoenixperry/FederatedBios/feat/federation/docs/architecture.html**
