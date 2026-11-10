# 0008. The desktop app only shows its own server

- **Status:** Accepted
- **Date:** 2026-11-10

## Context
The app checked only that *something* answered on port 8080. If another program used that port, the window showed it, and someone could type their passphrase into it.

## Decision
Each launch of the app gives its server a token. The app opens a page only when `/health` reports `hic-commons` and echoes that token; otherwise it says another program is using the port.

## Why
Port 8080 is a common default for development tools and containers, so a clash is likely on a developer's or researcher's computer.

## Consequences
The token only identifies; it isn't a secret.

## Alternatives considered
Picking a random port each launch (possible later; it would change bookmarked addresses).
