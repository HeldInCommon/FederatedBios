# 0014. Automatic app updates

- **Status:** Deferred (on the to-do list)
- **Date:** 2026-11-10

## Context
Formats will change over the fellowship. Without updates, workshop participants end up on versions that can't talk to each other.

## Decision
Later: Tauri's updater plugin, checking a signed `latest.json` on each versioned GitHub release; an updater signing key (separate from the Apple certificate) held in the protected `macos-signing` environment; a version number in every share so a commons can say "please update". Tracked as a GitHub issue.

## Why
Updates must be signed by us so a tampered download is refused. Losing the updater key would strand every installed copy, so it must be backed up.

## Consequences
Releases move from one overwritten preview to numbered versions.

## Alternatives considered
Manual reinstalls (the status quo).
