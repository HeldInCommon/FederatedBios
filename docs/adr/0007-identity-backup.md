# 0007. Identity backups, encrypted with the passphrase

- **Status:** Accepted
- **Date:** 2026-11-10

## Context
Keys live only on one computer, and every commons ties the handle to them (ADR 0003). A lost or broken laptop meant losing the handle everywhere.

## Decision
A member can download a backup file: their record (keys included) and current version, encrypted with XChaCha20-Poly1305 under a key derived from their passphrase with Argon2id. They can restore it on a new computer, which keeps the same keys.

## Why
The file is safe to keep anywhere (email, cloud drive) because it's useless without the passphrase. Restoring the same keys means every commons still recognises the handle.

## Consequences
Someone with both the file and the passphrase has the identity. Key rotation with a recovery key (ADR 0010) is the remedy for that.

## Alternatives considered
Storing keys with a service (rejected: someone else holds the keys).
