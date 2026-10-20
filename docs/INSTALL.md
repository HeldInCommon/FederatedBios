# Held in Common — Workshop Install Guide

You're going to run a **node** — your own personal commons — and (if you want)
**share** a copy of your profile to a shared commons so others can find it. Your
work stays on your machine unless you choose to send a copy.

**How it works:** you run a **node** (the app or Docker), create your identity
there, then **Share** a signed copy up to a shared commons. There is **no
sign-up on the commons itself** — your keys live on your node and never leave
your machine, so the only way to appear in a shared directory is to share from a
node. Path 1 (the app) is recommended.

> **Why can't I just sign up on the commons?** A shared commons is a *federation
> target*, not an identity store. If you open one directly you'll see the
> directory and a "Run a node to join" page — that's intentional. Author on your
> node; share up.

---

## Path 1 — Run your own node: the app (no Docker) — recommended

A node is just an app on your computer. These preview builds are **unsigned** (no
Apple/Windows certificate yet), so your OS warns once — that's expected, and the
steps below get past it.

### 1. Download

From the releases page: **https://github.com/phoenixperry/FederatedBios/releases/tag/node-app-preview**

| Your computer | Download this file |
|---|---|
| **macOS — Apple Silicon (M1/M2/M3/M4)** | `Held-In-Common-macOS-AppleSilicon.dmg` |
| **Linux — Ubuntu / Debian** | `Held-In-Common-Linux.deb` |
| **Linux — any distro** | `Held-In-Common-Linux.AppImage` |
| **Linux — Fedora / RHEL** | `Held-In-Common-Linux.rpm` |

> **Windows** and **Intel macOS** app builds aren't ready yet — use **Path 2 (Docker)** for now.

### 2. Open it — get past the "unsigned" warning (one time only)

- **macOS:** open the `.dmg`, drag the app into **Applications**. First launch: **right-click the app → Open → Open**. If macOS still blocks it ("…cannot be opened because Apple cannot check it for malicious software"), go to **System Settings → Privacy & Security**, scroll down, click **Open Anyway**, then launch the app again.
  - **If the `.dmg` itself says "disk is damaged — move to Trash" and won't even open:** that's not a broken download — it's macOS quarantining an app that isn't notarized yet. Clear the quarantine flag in **Terminal**, then open it:
    ```bash
    xattr -c ~/Downloads/Held-In-Common-macOS-AppleSilicon.dmg
    open ~/Downloads/Held-In-Common-macOS-AppleSilicon.dmg
    ```
    Drag the app into **Applications**, then clear it on the app too and launch:
    ```bash
    xattr -cr "/Applications/Held In Common.app"
    open "/Applications/Held In Common.app"
    ```
- **Linux (.AppImage):** `chmod +x Held-In-Common-Linux.AppImage`, then run it. (If it won't start, install `libfuse2`.)
- **Linux (.deb):** `sudo apt install ./Held-In-Common-Linux.deb`, then launch **Held In Common**.

### 3. You're up and running when…

The app opens your browser to **http://localhost:8080** and you see the Held in
Common page. That's *your* node, running on your own machine.

### 4. Make your profile

Create your **handle + passphrase**, fill in your profile, choose visibility +
license + CC signals, and **Publish**. It's saved on your computer.

### 5. (Optional) Share to the commons

On your profile, click **"Share to a commons"** (the shared address is pre-filled)
→ **Share**. A **signed copy** now appears in the shared directory. Your original
never leaves your node.

> **Members-only** profiles stay **encrypted on your machine** and are never shared — not even to the commons.

---

## Path 2 — Run a node with Docker (technical)

```
docker run -d --name hic-node -p 8080:8080 -v hic-node:/data \
  -e HIC_INSTANCE="My node" \
  -e HIC_SHARE_DEFAULT="<workshop commons address>" \
  ghcr.io/phoenixperry/hic-node:latest
```

Open **http://localhost:8080**. Your data lives in the `hic-node` volume (it
survives restarts).

---

## What the choices mean

- **Visibility** — *Public* (listed + searchable) · *Commons* (listed, kept out of search) · *Members-only* (encrypted; only signed-in members can read it).
- **License** — Creative Commons (CC0, CC BY, …): how people may reuse your work.
- **CC signals** — how AI models may learn from it: *Credit*, *Direct Contribution*, *Ecosystem*, *Open*.

## Troubleshooting

- **Port 8080 is busy** — quit whatever is using it, then reopen the app. (Docker: use `-p 8081:8080` and open `http://localhost:8081`.)
- **macOS "cannot be opened / Apple cannot check it"** — right-click → **Open**, or **System Settings → Privacy & Security → Open Anyway** (only needed once).
- **macOS "disk is damaged — move to Trash" (the `.dmg` won't open at all)** — the download is fine; macOS is quarantining an app that isn't notarized. In **Terminal**: `xattr -c ~/Downloads/Held-In-Common-macOS-AppleSilicon.dmg` then open it. After moving the app to **Applications**: `xattr -cr "/Applications/Held In Common.app"`, then open it.
- **Linux AppImage won't run** — make sure it's executable (`chmod +x`), and install `libfuse2` on distros that need it.
- **Forgot your passphrase** — there's no recovery (we store only a salted hash); create a new handle.

## How it all fits together (slides)

**https://phoenixperry.github.io/FederatedBios/architecture.html**
