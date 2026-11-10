# 0010. A recovery key, made at sign-up and kept offline

- **Status:** Accepted, to build
- **Date:** 2026-11-10

## Context
Backups (ADR 0007) cover a *lost* key, not a *stolen* one. Whoever holds a stolen key can publish and withdraw as you on every commons, and today the only remedy is abandoning the handle. Key rotation (the old key signs "my new key is X") fixes theft, except when the thief rotates first.

## Decision
When a handle is created, the node also makes a **recovery key**. Its public half travels with the first share and each commons records it beside the handle. Its private half is shown once, to print or write down, and is not kept on the computer. A rotation signed by the recovery key overrides any rotation signed by the ordinary key, and it also sets a new recovery key. Handles created before this get a recovery key at their next share.

## Why
It gives the real owner the final word without involving anyone else, and it lives somewhere a laptop thief can't reach. Phoenix chose this over steward confirmation (which would make stewards judge disputed identities).

## Consequences
People must keep a piece of paper (or equivalent) safe. If the recovery key is put inside the encrypted backup instead, it protects against laptop theft but not against someone who has both the backup and the passphrase; the interface should say so. Losing both the key and the recovery key still means a new handle.

## Alternatives considered
Steward confirmation of disputed rotations (not chosen; may be added later as a fallback); no rotation at all (rejected: no remedy for theft).
