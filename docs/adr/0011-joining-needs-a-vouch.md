# 0011. Joining a commons needs a vouch

- **Status:** Accepted (details decided 2026-11-10)
- **Date:** 2026-11-10

## Context
Keys are free, so anyone can script thousands of identities, squat everyone's names on a commons, fill its disk and slow it down. A rate limit only slows this down.

## Decision
A new handle is listed on a commons only once an existing member vouches for it. The first person on a commons is vouched for by its steward. A vouch is a signed object from the member's node (ADR 0009), so the commons can check it and keep it as a record of who brought whom in.

## Why
It stops mass sign-ups completely rather than slowing them, and it matches how a community actually grows: people bring people they know. It also gives governance a real record ("vouched for by @x") to reason about later.

## Details (Phoenix, 2026-11-10)
- **Both routes.** A member's node makes a single-use **invite code** (a vouch made in advance) that the newcomer pastes when they first share. Without a code, the newcomer's share waits on the commons' **pending list** until a member vouches for it from their node.
- **The first person** comes in with a one-time **founding invite** that the commons makes when it starts. It is shown only to someone at the commons' own computer (the desktop app's window counts), and written to the data folder and the server log, so nobody else on the network can see it.
- **The limit** on how many people one member can bring in is set by the steward. Until commons have a steward page, the host is the steward and sets it when starting the commons: `HIC_VOUCH_LIMIT` (people per member per week, default 5).
- **Everyone stays:** profiles already on a commons stay listed, and people stay if the member who vouched for them later leaves.
- Vouches and invites are signed `hic/v1` objects (`Invite`, `Vouch`; spec §5.3); every admission is recorded with who vouched.
- `HIC_OPEN_COMMONS=1` turns vouching off for a commons that wants open sharing (for example a test commons).

## Consequences
A newcomer needs a member (or the steward's founding invite) to vouch for them. The pending list shows each waiting handle with a short key fingerprint and where it came from, so members can recognise who they're vouching for; it shows no profile content.

## Alternatives considered
Rate limits alone (rejected: slows, doesn't stop); steward approval of every newcomer (rejected: puts all the work on one person).
