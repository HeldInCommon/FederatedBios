# 0012. Sharing can't be aimed at internal addresses

- **Status:** Accepted
- **Date:** 2026-11-10

## Context
When someone presses Share, the node *server* sends the request, not their browser. A server can reach places an outsider can't: other programs on the same machine (`127.x`), the local network, and on cloud hosts a special address (`169.254.169.254`) that hands out the machine's own credentials. On a hosted node with several members, a member could aim "Share" at those (server-side request forgery).

## Decision
Always refuse loopback (`127.x`, `::1`), link-local (`169.254.x`, `fe80::`) and unspecified addresses. Allow local-network addresses (`10.x`, `172.16–31.x`, `192.168.x`) on a node that listens only on this computer, because a workshop room commons lives at such an address; refuse them on a node reachable from the network unless `HIC_ALLOW_PRIVATE_SHARE=1`. Check the address after DNS lookup, and don't follow redirects.

## Why
Blocking all private addresses would break room commons. A personal node has one member, so the risk there is low; a hosted node is where the danger is.

## Consequences
A hosted node that genuinely shares to a commons on its private network needs the setting.

## Alternatives considered
Block all private addresses (rejected: breaks room commons); block nothing (rejected: unsafe on hosted nodes).
