# 0002. A commons has no sign-up

- **Status:** Accepted
- **Date:** 2026-11-09

## Context
If people could create identities directly on a commons, the commons would hold their private keys, and whoever runs it could act as them.

## Decision
A commons never creates identities. `/join` and `/login` show a page explaining how to run a node. Profiles arrive only as signed copies sent from people's own nodes.

## Why
Keys stay on the computer of the person they belong to. A commons only ever holds copies it can check, never the means to forge them.

## Consequences
Everyone needs a node (the desktop app makes that one download). A commons is a directory, not an account provider.

## Alternatives considered
Accounts on the commons (rejected: the commons operator would hold everyone's keys).
