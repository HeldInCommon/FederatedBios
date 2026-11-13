# Open to the room: a guide for hosts

For teachers, workshop hosts and gallery organisers. This feature is being
built now (see [ROOM-PLAN.md](ROOM-PLAN.md) and
[ADR 0027](adr/0027-open-to-the-room.md)). Steps marked "not yet tried" haven't
been checked on that kind of computer yet.

## What it's for

Everyone in the room joins the same Wi-Fi. Each person opens their node to the
room, and a QR code appears on their screen. Others scan it with a phone or
laptop and see that person's public profile and stories, straight from their
computer. Nothing private leaves anyone's laptop, and when the session ends the
room closes.

## What to bring

- **A travel router or a phone hotspot.** This is the surest choice. You
  control it, and devices on it can reach each other.
- **Be wary of school, university and guest Wi-Fi.** These often have *client
  isolation*: a router setting that stops devices on the same Wi-Fi from
  talking to each other. If it's on, no app can get around it.
- A projector or big screen for your own QR code, if you're hosting a local
  commons.

## Before the session

1. Install the Held in Common app on your laptop. Ask participants to install
   it too, and to set up their node before they arrive.
2. Test with two devices, on the network you'll use if you can:
   1. On your laptop, open your node to the room (see below).
   2. On a phone on the same Wi-Fi, open your address with `/room/check` on
      the end, for example `http://192.168.1.23:8080/room/check`. It should say
      "It works" and "You reached" followed by your computer's name.
   3. If it doesn't, work through "When someone can't connect" now, not on the
      day.

## In the room

### For the host

1. Join your laptop to the room's Wi-Fi.
2. In the app, open the **Go** menu and choose **Open my node to the room…**.
   To open your local commons, choose **Open** followed by its name, for
   example **Open Thursday session to the room…**, from the same menu. (Start
   it first with **Go → Start a local commons**; it starts closed.)
3. The app asks you to confirm: "Only open this on a network you trust: a
   classroom, a studio, your own hotspot." Confirm only if that's where you are.
4. The `/room` page opens. It shows a big QR code and your address, for example
   `http://192.168.1.23:8080/@ana` (or `…:8080/` if your profile isn't public
   yet). Put it on the big screen. If you close that page, choose **Show the QR
   code for …** from the **Go** menu to get it back. It may also show a hint
   about your firewall.
5. On your local commons, `/room` also shows a **6-letter room code**. People
   type it to join your commons. A new code is made each time you open the
   room, and closing the room retires it. People who joined stay in.
   Anyone who has the code can join, as many times as they like, without
   anyone vouching for them: keep it on the screen in the room, not in a group
   chat. If lots of wrong codes are tried, the code stops working; close the
   room and open it again for a new one.

### For participants

1. Join the same Wi-Fi as the host.
2. In the app, open the **Go** menu and choose **Open my node to the room…**,
   then confirm.
3. Your `/room` page shows your QR code and address. Turn your screen so others
   can scan it, and scan theirs.
4. To join the host's local commons, share to it from your node as usual,
   and type the room code from the host's screen into the **Invite or room
   code** box. Spaces don't matter. Five wrong tries and that laptop has to
   wait a while before trying again.

## When someone can't connect

Go through these in order.

1. **Are you both on the same Wi-Fi?** Compare the first three numbers of the
   two addresses. `192.168.1.23` and `192.168.1.40` match; `192.168.1.23` and
   `10.0.0.5` don't.
2. **Does `/room/check` open on their own phone?** Ask the person whose node
   won't open to try their own address with `/room/check` on the end. If even
   their own phone can't reach it, the firewall on that laptop is blocking it.
   See the next section.
3. **Can any device reach any other?** If nobody can reach anybody, the network
   has client isolation. Switch to the hotspot or travel router.

## Firewalls

A *firewall* is software on each computer that decides which incoming
connections are let in. Open **one port, on private networks only**. Never
switch the firewall off: a firewall left off stays off at the next café.

The app never changes your firewall for you.

**macOS** (not yet tried on a real Mac)

- When asked "Do you want the application hic-commons to accept incoming
  network connections?", choose **Allow**.
- If someone clicked Deny: System Settings → Network → Firewall → Options →
  find `hic-commons` → set it to "Allow incoming connections".
- Signed builds remember the answer; unsigned ones ask at every launch.
- If "Block all incoming connections" is on in the same Options window, turn
  it off; otherwise nobody in the room can reach you.
- If macOS asks for Local Network access, allow it (not yet confirmed).

**Windows** (not yet tried on a real Windows machine)

- In the Defender dialog, tick **Private networks** and choose Allow.
- Windows marks a newly joined Wi-Fi as *Public*, and Public blocks this. So
  also go to Settings → Network & internet → Wi-Fi → the network → Network
  profile type → **Private**.

**Linux** (not yet tried)

On ufw:

```
sudo ufw allow from 192.168.0.0/16 to any port 8080:8082 proto tcp
```

and afterwards:

```
sudo ufw delete allow from 192.168.0.0/16 to any port 8080:8082 proto tcp
```

On firewalld:

```
sudo firewall-cmd --zone=home --add-port=8080-8082/tcp
```

This leaves out `--permanent`, so it's gone after a reboot. Use the zone your
Wi-Fi is in: `firewall-cmd --get-active-zones` shows it.

If your address starts with `10.` or `172.` rather than `192.168.`, put
`10.0.0.0/8` or `172.16.0.0/12` in place of `192.168.0.0/16`. (An iPhone
hotspot, for example, gives addresses starting `172.20.10.`.)

`sudo` runs a command as the administrator, so it asks for your password.

## After the session

1. Choose **Close my node to the room** from the **Go** menu, and **Close …
   to the room** for your local commons. Quitting the app closes them too.
2. On Linux, remove the ufw rule you added (above).
3. On Windows, if you set a network you don't control to Private, set it back
   to Public.

The room also closes by itself when the app quits or crashes, and when your
laptop's network address changes (the app checks every minute). Then the app
says: "… is closed to the room: this computer's network address changed. Open
it again from the Go menu if you're still in the room." Every time the app
starts, the room starts closed.

## What stays private, and what doesn't

**Stays on the owner's computer:** signing in, your keys and passphrase, your
private and members-only stories, and every page for changing things. Someone in
the room who tries one is told the page stays on its owner's computer. On a
node, nobody in the room can sign in or post anything. On a local commons,
people in the room can share to it and join with the room code, but the
steward view and the room page stay on the host's computer.

**Can be seen by the room:** your public profile and public stories, the same
as anyone would see on a commons. Someone in the room who means harm can read
every public page of every open node. That's the point of opening it.

**Things to know:**

- **The connection isn't encrypted.** Plain `http://` on a shared Wi-Fi can be
  read by others on that network. Public pages are public anyway, and on a
  node, signing in never happens over the room.
- **Open servers listen only on the room's Wi-Fi**, not on a VPN or Tailscale.
  If your laptop's address belongs to a VPN or Tailscale exit node, the app
  won't open the room until you turn that off.
- **Don't open the room in a café or hotel.** The app warns you, but it can't
  tell a café from a classroom.
- **Don't forward ports to the app.** A forwarder can make visitors from
  outside look like the owner. The app refuses to open the room on a port that
  Tailscale Funnel or `serve` points at, but it can't see every kind.
