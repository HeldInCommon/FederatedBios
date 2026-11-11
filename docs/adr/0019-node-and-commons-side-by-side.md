# 0019. One computer can run a node, a commons and a practice commons

- **Status:** Accepted (built)
- **Date:** 2026-11-11

## Context
The desktop app asked once whether this computer runs a node or a commons, and never again. A steward then had no node of their own, so they couldn't put their own profile in their own commons, and someone with a node had no way to see what running a commons involves before committing to host one.

## Decision
The app runs up to three servers side by side, each a separate `hic-commons` process with its own data folder:

| Server | Port | Folder | Reachable from |
|---|---|---|---|
| The role chosen at first launch | 8080 | `node-data/` or `commons-data/` | as before |
| The other role, if added later | 8081 | the other of those | a node: this computer; a commons: the room |
| A practice commons, while there is one | 8082 | `practice-data/` | this computer only |

- The first-launch role keeps port 8080, so existing nodes and room commons keep their addresses.
- The **Go** menu shows each server, or sets it up, and removes the practice commons. A row of buttons in the corner of every server page moves between them.
- A node's footer links to **Run your own commons** (`/run-a-commons`): questions and answers about hosting, and, in the app, buttons to open a practice commons or set up a real one. Those buttons are `hic-app://open/<kind>` links. The main window catches them (as it does `mailto:`) and acts only when the page they were followed from is one of this computer's servers.
- The practice commons is an ordinary commons with `HIC_BIND=127.0.0.1` and `HIC_PRACTICE=1` (its pages say "Practice commons"). Removing it deletes its folder.
- Each server gets its own sign-in cookie name (`HIC_SESSION_COOKIE`; `hic_session` on 8080, `hic_session_<port>` otherwise).

## Why
Decision record 0001 fixes the role per *data folder*, not per computer, so two folders can hold two roles without one ever becoming the other. Running them together, rather than switching, means a room commons stays up while its steward uses their node, and the steward's node can share to it. The practice commons can't be reached from the network, so mistakes made while learning stay on this computer.

Browsers keep cookies per host, not per port: with one cookie name, signing in to the steward view on `127.0.0.1:8081` would sign the node on `127.0.0.1:8080` out.

## Consequences
- A node shares to a commons on the same computer like any other (ADR 0012 lets a personal node share to local addresses).
- A node or commons added second is on port 8081, so its address differs from one chosen at first launch.
- Pages can only ask the app to *open* something. Nothing a page links to can remove or change data.

## Alternatives considered
Switch between roles on one port (rejected: the commons goes offline while its steward uses their node, and the two can't share to each other). Open a real room commons for practice (rejected: anyone on the Wi-Fi could see and share to the experiment).
