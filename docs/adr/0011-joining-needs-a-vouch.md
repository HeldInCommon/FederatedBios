# 0011. Joining a commons needs a vouch

- **Status:** Accepted, to design in detail
- **Date:** 2026-11-10

## Context
Keys are free, so anyone can script thousands of identities, squat everyone's names on a commons, fill its disk and slow it down. A rate limit only slows this down.

## Decision
A new handle is listed on a commons only once an existing member vouches for it. The first person on a commons is vouched for by its steward. A vouch is a signed object from the member's node (ADR 0009), so the commons can check it and keep it as a record of who brought whom in.

## Why
It stops mass sign-ups completely rather than slowing them, and it matches how a community actually grows: people bring people they know. It also gives governance a real record ("vouched for by @x") to reason about later.

## Consequences
A newcomer needs a member to vouch for them. Proposed details, to confirm before building:
- **How:** a member's node makes a single-use **invite code** (a vouch made in advance) that the newcomer's node includes in its first share; or the newcomer shares, waits in a "pending" list, and a member vouches from their node. Invite codes are smoother; both may be useful.
- **The steward:** the person who starts a commons gets a one-time founding invite (shown in the app or the server log) to bring in the first member.
- **Limits:** each member can vouch for a few people per week, so one compromised member can't let in thousands.
- **Existing profiles:** profiles already on a commons stay listed.
- **Withdrawal:** if a voucher later withdraws, the people they vouched for stay.
A per-address rate limit stays as a cheap first layer against junk traffic.

## Alternatives considered
Rate limits alone (rejected: slows, doesn't stop); steward approval of every newcomer (rejected: puts all the work on one person).
