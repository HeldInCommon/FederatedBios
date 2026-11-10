# 0009. A written specification for everything we sign

- **Status:** Accepted (built for new types; profiles still use version 0)
- **Date:** 2026-11-10

## Context
The exact bytes that are hashed and signed are defined only by our Rust code and a JSON library's default behaviour. Key order, escaping and number formatting depend on that library. Another program (for example a Python script that trains an adapter) can only match it by reverse-engineering, and a dependency change could silently break every signature. Some fields travel unsigned, signatures don't say what kind of object they cover, and nothing carries a version number.

## Decision
Write the format down in `docs/spec/`:
1. every signed object carries a version (`"schema": "hic/v1"`);
2. one envelope for every kind of object (profile, dataset listing, rights record, licence, adapter, model version, withdrawal, key rotation, vouch, decision), with `author`, `created_at`, `prev` (the previous version's id) and `body`;
3. JSON written by the RFC 8785 JSON Canonicalization Scheme, a published standard with libraries in Python, JavaScript and Rust;
4. the signature covers a type prefix plus the id (`hic/v1/<Type>\n<cid>`), so a signature for one kind of object can't be passed off as another;
5. published test vectors (inputs, exact bytes, id, and a signature with a test key);
6. today's format stays readable and verifiable as "version 0".

## Why
A specification outlives code: the server can be rewritten without breaking anyone's history. It makes interoperability checkable (any implementation must match the test vectors), which the BRAID work needs as soon as a training script registers an adapter. `prev` chains give every author a tamper-evident history, much like the Guild Register.

## Consequences
All new objects use the envelope. Existing profiles keep verifying under version 0.

## Alternatives considered
Keep the format implicit in the code (rejected: fragile and Rust-only); invent our own canonical form (rejected: RFC 8785 already exists).
