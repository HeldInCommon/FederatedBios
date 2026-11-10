# 0012. Sharing can't be aimed at internal addresses

- **Status:** Accepted (built)
- **Date:** 2026-11-10

## Context
When someone presses Share, the node *server* sends the request, not their browser. A server can reach places an outsider can't: other programs on the same machine (`127.x`), the local network, and on cloud hosts a special address (`169.254.169.254`) that hands out the machine's own credentials. On a hosted node with several members, a member could aim "Share" at those (server-side request forgery).

## Decision
The guard depends on who could be pressing Share:
- A **personal node** (listening only on its own computer, ADR 0004) may share to any address. Whoever uses it is at that computer and can reach those addresses anyway, so the server grants them nothing new.
- A node **reachable from the network** refuses its own machine's addresses (`127.x`, `::1`), link-local addresses including the cloud credentials address (`169.254.x`, `fe80::`) and unspecified ones, and refuses local-network addresses (`10.x`, `172.16–31.x`, `192.168.x`, `100.64/10`, `fc00::/7`) unless `HIC_ALLOW_PRIVATE_SHARE=1`.

The address is checked after the DNS lookup; every address a name resolves to must pass; the request is pinned to the checked address so a second lookup can't swap it; redirects are off. Code: `commons/src/outbound.rs`.

## Why
Request forgery only matters when the server can reach places the person pressing the button can't. A first draft refused loopback everywhere, which would have broken sharing from a laptop to a commons on the same laptop (how local testing and `scripts/test-federation.sh` work) for no security gain.

## Consequences
A hosted or Docker node that shares to a commons on its own private network needs `HIC_ALLOW_PRIVATE_SHARE=1` (for example a Docker node sharing to `host.docker.internal`).

## Alternatives considered
Block all private addresses (rejected: breaks room commons); block nothing (rejected: unsafe on hosted nodes).
