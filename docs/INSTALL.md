# Held in Common — Workshop Install Guide

You're going to run a **node** — your own personal commons — and (if you want)
**share** a copy of your profile to a shared commons so others can find it. Your
work stays on your machine unless you choose to send a copy.

**In a hurry?** Path 1 needs nothing installed. Path 2 runs a real node on your
own computer (recommended). Pick one.

---

## Path 1 — Use the shared commons (nothing to install)

Best if you just want to be in the directory in two minutes.

1. Open the **workshop commons address** (your facilitator shares this separately — it is not committed to this repo)
2. Click **Add your profile**.
3. Choose a **handle** and a **passphrase** (you sign back in with these — there's no recovery, so pick something memorable).
4. Fill in your profile → choose **who can see it** → pick a **license** and **CC signals** → **Publish**.
5. ✅ You now appear in the directory. Done.

> This shared commons is a **preview server** — treat it as scratch space; data may be reset between sessions.

---

## Path 2 — Run your own node: the app (no Docker) — recommended

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

> **Windows** and **Intel macOS** builds aren't ready yet — use **Path 1** or **Path 3 (Docker)** for now.

### 2. Open it — get past the "unsigned" warning (one time only)

- **macOS:** open the `.dmg`, drag the app into **Applications**. First launch: **right-click the app → Open → Open**. If macOS still blocks it ("…cannot be opened because Apple cannot check it for malicious software"), go to **System Settings → Privacy & Security**, scroll down, click **Open Anyway**, then launch the app again.
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

## Path 3 — Run a node with Docker (technical)

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
- **Linux AppImage won't run** — make sure it's executable (`chmod +x`), and install `libfuse2` on distros that need it.
- **Forgot your passphrase** — there's no recovery (we store only a salted hash); create a new handle.

## How it all fits together (slides)

**https://phoenixperry.github.io/FederatedBios/architecture.html**
