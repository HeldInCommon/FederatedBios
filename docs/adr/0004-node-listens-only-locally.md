# 0004. A node listens only on its own computer

- **Status:** Accepted
- **Date:** 2026-11-10

## Context
The desktop app's node was reachable by anyone on the same Wi-Fi (café, conference, workshop room), including its sign-in page.

## Decision
A node listens on `127.0.0.1` (this computer only). A commons listens on every network interface, because being reached is its purpose. `HIC_BIND` overrides this; the Docker image sets `0.0.0.0` because Docker's port mapping needs it.

## Why
Nobody else needs to reach your node: you use it in your own window, and it reaches commons by sending, not by being contacted.

## Consequences
The desktop window opens `127.0.0.1:8080` instead of `localhost` (which can mean IPv6 on some systems).

## Alternatives considered
Keep listening everywhere and rely on the passphrase (rejected: exposes the sign-in page to guessing).
