# 0013. Pages keep loading Google Fonts

- **Status:** Accepted
- **Date:** 2026-11-10

## Context
The review (M10) noted that each page load sends the visitor's IP address to Google, and fonts don't load without internet access.

## Decision
Keep Google Fonts.

## Why
Phoenix's call: the look matters and the privacy cost is acceptable for now. Pages still work offline, in fallback fonts.

## Consequences
Revisit if a commons needs to work fully offline or promises no third-party requests.

## Alternatives considered
Serving the fonts from the server (possible later).
