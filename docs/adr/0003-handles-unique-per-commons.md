# 0003. Handles are unique per commons, and belong to the first key

- **Status:** Accepted
- **Date:** 2026-11-09

## Context
Two people could both call themselves @tega. A test showed an impostor node could share a profile with someone else's handle.

## Decision
On one commons, a handle belongs to the first key that shared it; a different key using the same handle is refused (409). The same handle may exist on other commons. A node checks its default commons before allowing a sign-up. Where a share says it came from is not part of the rule.

## Why
The key is what the commons can actually check (the signature). The sender's address and node name are written by the sender, so an impostor could fake them; tying the handle to them would be no protection. People may want to belong to several commons, so uniqueness is per commons, not global.

## Consequences
Losing the key means losing the handle on every commons, which is why backups (ADR 0007) and a recovery key (ADR 0010) matter. Lookalike names ("phoenix" vs "phoenix4") are still possible; cards show the node's address and a key fingerprint so people can tell them apart.

## Alternatives considered
Global uniqueness (rejected: needs a central registry, and people want several commons); tying handles to the sender's address (rejected: the sender writes it).
