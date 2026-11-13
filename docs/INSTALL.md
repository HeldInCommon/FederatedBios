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
| **Linux — Ubuntu / Debian** | the `.deb` file: `_amd64` for x86, `_arm64` for ARM |
| **Linux — any distro** | the `.AppImage` file: `_amd64` for x86, `_aarch64` for ARM |
| **Linux — Fedora / RHEL** | the `.rpm` file: `x86_64` or `aarch64` |

Not sure which? Run `uname -m`: `x86_64` is x86; `aarch64` is ARM (a DGX
Spark, a Raspberry Pi 5).

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
bar: **Go → Rename your node…**.

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
- **Waiting for a vouch** (on the People tab): everyone waiting on each
  commons you're listed on, with their name, handle and key fingerprint.
  Press **Vouch for @…** for someone you know, after checking the
  fingerprint with them.
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

**Already have a node?** You don't need a second computer. Choose
**Go → Set up a commons…** (or the button on **Run a commons**, linked
at the bottom of every node page). The commons runs beside your node, and the
buttons in the corner of the window move between them. **Hosting a commons
and want your own profile in it?** Choose **Go → Add your own node…**, then
share to that commons like anyone else.

**Running a workshop?** Members can write a short **story** on their node
(**Stories → Write a story**; the prompt is "Listen to your street for a
minute. Write a small story from what you hear."). It stays on their computer
until they press **Share**, and goes to a commons where they're listed. They
can **Withdraw** it from there at any time, and **Export** it as a text file.

**A local commons, for a workshop or a session.** **Go → Start a local
commons** starts a commons on this computer. It starts closed: only this
computer can reach it. Name it (**Go → Rename local commons…**), and when the
room is ready choose **Go → Open … to the room…**: anyone on the same Wi-Fi
can then reach it at this computer's address (it looks like
`http://192.168.1.20:8082`). It closes when you choose Close, quit the app, or
this computer's network address changes. See `docs/ROOM-GUIDE.md` for running a session.
People share to it from their own nodes and are listed once a member vouches
(one click on their People tab) or you approve them. **Go → Remove local
commons…** deletes it when the session's over.

**Also public (Tailscale Funnel, a reverse proxy)?** A commons started by the
app knows itself only by its room address, so invite codes and vouches made
for its public address are refused as "for a different commons". Tell it its
public address: quit the app, add one line to `app.json` in the app's data
folder (`~/Library/Application Support/org.heldincommon.node/` on a Mac,
`~/.local/share/org.heldincommon.node/` on Linux), and reopen it:

```
"public_address": "https://your-commons.example.ts.net"
```

The commons then accepts both addresses, shows the public one first ("Nodes
share to …"), and has a steward link for each.

**Your steward link** opens the steward view from any computer: in the app,
choose **Go → Show steward link…** and press **Copy**. Keep it private:
anyone who has it can approve people and make invite codes.

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
- **Linux window is blank or gray:** the app already turns off the graphics
  path that fails on NVIDIA drivers (`WEBKIT_DISABLE_DMABUF_RENDERER=1`). If
  you set that variable yourself, the app uses your value instead.
- **Reporting a problem:** send the log file with a short description. On a
  Mac it's `~/Library/Logs/org.heldincommon.node/held-in-common.log` (Finder →
  **Go → Go to Folder…** → paste `~/Library/Logs/org.heldincommon.node`). It
  contains no passphrases or keys.

## How it all fits together

Slides: **https://heldincommon.github.io/FederatedBios/architecture.html**
