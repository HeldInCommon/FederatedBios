# 0018. How a node hears it was listed

- **Status:** Accepted
- **Date:** 2026-11-10

## Context
When a share waits for a vouch or the steward's approval (ADR 0011, 0017),
the person had no way to know they'd been listed except to look at the
commons. A first version had the node ask the commons every 30 seconds.
Phoenix found that too much, and asked whether a callback was possible.

## Decision
- **No callback.** A commons can't reach a node: a node listens only on its
  own computer (ADR 0004) and is usually behind a home router. A callback
  would mean exposing nodes to the network or running a relay, undoing a
  deliberate privacy and security choice.
- **The node asks only when there's a reason:** once **each time it
  starts**, and when the member presses **Check now** (shown on their profile
  while they're waiting). No background polling of commons.
- The node remembers which commons it's waiting on (`waiting_on` in the
  member's record) and asks each `GET /api/status?handle=…&key=…`, which
  answers `listed`, `waiting` or `not_listed`.
- A change becomes a **notice**: a banner on the member's pages with
  **Dismiss**, and a **system notification** from the desktop app. The app
  collects notices from its own node (`POST /api/notices/claim`, with its
  launch token) a few times in the first minute after start-up; nothing
  leaves the computer.

## Consequences
News arrives when the node starts or the member checks, not the moment the
steward approves. If declined, the notice says how to try again.

## Alternatives considered
Polling every 30 seconds (rejected: unnecessary traffic); callbacks or a
relay (rejected: exposes nodes); email notifications (rejected: the commons
would need people's addresses).
