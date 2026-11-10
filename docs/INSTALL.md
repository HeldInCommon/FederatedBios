# Held in Common — Install guide

You'll run a **node**: your own personal commons, on your own computer. Then,
if you like, you **share** a copy of your profile to a shared **commons** so
others can find it. Your work stays on your machine unless you choose to send a
copy.

> **Why can't I just sign up on the commons?** A commons only holds signed
> copies that people share from their own nodes; your identity and keys live on
> your computer. Open a commons directly and you'll see its directory and a
> "Run a node to join" page. That's intentional.

---

## Part 1 — Run your node (everyone)

### 1. Download

From the releases page: **https://github.com/HeldInCommon/FederatedBios/releases/latest**

| Your computer | Download |
|---|---|
| **macOS — Apple Silicon (M1–M4)** | `Held-In-Common-macOS-AppleSilicon.dmg` |
| **Linux — Ubuntu / Debian** | the `.deb` file |
| **Linux — any distro** | the `.AppImage` file |
| **Linux — Fedora / RHEL** | the `.rpm` file |

Windows and Intel Macs aren't supported by the app yet; use Docker (Part 4).

### 2. Install

- **macOS:** open the `.dmg` and drag **Held In Common** into **Applications**,
  then open it from Applications. The app is signed and notarized by Apple, so
  it opens normally.
- **Linux (.AppImage):** `chmod +x` the `.AppImage`, then run it
  (install `libfuse2` if it won't start).
- **Linux (.deb):** `sudo apt install ./<the .deb file>`, then launch
  **Held In Common**.

### 3. Choose "A node" and name it

The first time you open the app it asks **what you want to run**. Choose
**A node**. (A *commons* is for whoever hosts the shared directory; see Part 3.)

Then **name it**, for example "Studio laptop". People see this name next to your
profile on every commons you share to; if you use Held in Common on more than
one computer, give each its own name. You can rename it later from the menu
bar: **Node → Rename…**.

### 4. Create your handle — and keep your recovery code

Click **Add your profile**, pick a **handle** (your `@name`) and a
**passphrase** (the eye button shows what you've typed). There's no way to
recover a forgotten passphrase, so choose one you'll remember.

Next you'll see your **recovery code**, once. **Write it down or print it, and
keep it offline**, not on this computer. If your keys are ever stolen, this code
takes your handle back on every commons.

### 5. Publish your profile

Fill in your profile, choose **who can see it** (Public, Listed in the commons,
or Members only), a **licence** and **signals**, and **Publish**. It's saved on
your computer.

---

## Part 2 — Join a commons

On your profile, under **Share to a commons**, open
**Share with an invite code, or to another commons**, enter the commons'
address (your host will give it to you), and press **Share**.

- **With an invite code** (someone may email you one): paste it into
  **Invite code** and you're listed straight away.
- **Without one:** you'll see **Waiting for a vouch**. A member who knows you
  can vouch for you, or the commons' steward can approve you. Your node checks
  each time it starts, and you can press **Check now** on your profile. When
  you're listed you'll see a banner, and the app shows a notification.

Members-only profiles are never shared: they stay encrypted on your computer.

**Once you're on a commons**, your profile also offers:

- **Invite someone:** makes a one-time code; **Email this invite** opens your
  mail app with the invitation written for you.
- **Vouch for someone:** for people you know who are waiting on that commons.
- **Withdraw:** asks the commons to remove your shared copy.

### Keep your identity safe

- **Download backup** (on your profile) saves your identity, locked with your
  passphrase. On a new computer, use **Restore from a backup** on the sign-up
  page to keep the same handle everywhere.
- **Replace my key** if you think someone else has your keys. If they replaced
  it first, use your **recovery code** under "Someone else has already replaced
  my key".

---

## Part 3 — Host a commons (workshop leaders, collectives)

1. Install the app as in Part 1, but choose **A commons**, pick **This room**,
   and name it.
2. The app opens the **steward view**. It shows a one-time **founding invite**
   for the first person (it can be you), and the address people share to
   ("Nodes share to …").
3. People waiting appear under **Waiting for a vouch** with **Approve** and
   **Decline**. **Make an invite** gives you a code to send, with an
   **Email this invite** button.

To host on a server or in Docker instead, see [commons/README.md](../commons/README.md).

---

## Part 4 — Run a node with Docker (technical)

```
docker run -d --name hic-node -p 8080:8080 -v hic-node:/data \
  -e HIC_INSTANCE="My node" \
  ghcr.io/heldincommon/hic-node:latest
```

Open **http://localhost:8080**. Your data lives in the `hic-node` volume.

---

## Troubleshooting

- **"Another program is using port 8080"**: quit that program (or another copy
  of Held in Common), then press **Start**.
- **Forgot your passphrase:** it can't be recovered. Create a new handle.
- **"Waiting for a vouch" for a long time:** ask someone on that commons to
  vouch for you, ask its steward, or share again with an invite code. Press
  **Check now** on your profile to see if you've been listed.
- **Linux AppImage won't run:** `chmod +x` it, and install `libfuse2`.
- **Reporting a problem:** send the log file with a short description. On a
  Mac it's `~/Library/Logs/org.heldincommon.node/held-in-common.log` (Finder →
  **Go → Go to Folder…** → paste `~/Library/Logs/org.heldincommon.node`). It
  contains no passphrases or keys.

## How it all fits together

Slides: **https://phoenixperry.github.io/FederatedBios/architecture.html**
