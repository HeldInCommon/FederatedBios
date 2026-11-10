# 0005. Passphrases are hashed with Argon2id, and guessing is limited

- **Status:** Accepted
- **Date:** 2026-11-10

## Context
Passphrases were stored as a single SHA-256 hash. SHA-256 is designed to be fast, so a copied data folder could be cracked offline quickly.

## Decision
New passphrases are hashed with Argon2id, a deliberately slow, memory-hard hash. Old hashes still work and are replaced at the next successful sign-in. After 5 wrong passphrases for a handle, sign-in waits 15 minutes. Sessions expire after 24 hours.

## Why
Argon2id is the current standard recommendation for passphrase storage (it won the Password Hashing Competition and is recommended by OWASP). Slowness that a person never notices makes large-scale guessing impractical.

## Consequences
Signing in takes a fraction of a second longer. Nobody has to reset their passphrase.

## Alternatives considered
bcrypt or scrypt (fine, but Argon2id is the current recommendation).
