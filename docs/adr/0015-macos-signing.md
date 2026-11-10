# 0015. macOS builds are signed with a personal Developer ID

- **Status:** Accepted
- **Date:** 2026-11-09

## Context
Unsigned builds made macOS say the disk image was "damaged", which workshop participants had to work around in Terminal. A UAL-inherited certificate had broken CI.

## Decision
Sign with Phoenix's personal Apple Developer ID and notarize with Apple. The six signing secrets live in the GitHub environment `macos-signing`, usable only from `main` and release tags. TestFlight comes later.

## Why
Developer ID plus notarization is what makes a downloaded app open normally. Keeping the secrets in a protected environment means a workflow on any other branch can't read them; the repository has several collaborators.

## Consequences
Phoenix sets the secrets; they are never handled in chat. Signing can't run from feature branches.

## Alternatives considered
The UAL team certificate (rejected: broken and not ours to control).
