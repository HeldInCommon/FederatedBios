# 0006. Withdrawal leaves a tombstone; old versions can't be replayed

- **Status:** Accepted
- **Date:** 2026-11-10

## Context
Anyone holding an old signed copy of a profile could resend it to a commons and roll the profile back. Separately, the interface promised withdrawal, but there was none. Withdrawal is also central to the BRAID fellowship (the Model Withdrawal Protocol) and to data-protection erasure requests.

## Decision
A commons refuses a share that isn't newer than the version it holds for that key. A member can withdraw a shared copy: their node sends a signed withdrawal addressed to that commons, and the commons deletes the copy but keeps a **tombstone** (key, handle, time; no profile content).

## Why
The tombstone does two jobs with almost no data: the handle stays reserved for its owner (so nobody grabs the name the moment you leave), and the withdrawn copy can't be replayed. Addressing the withdrawal to one commons stops it being replayed to others. An older withdrawal can't remove a later re-share, so leaving and rejoining works.

## Consequences
Copies someone already downloaded can't be recalled; the interface says so plainly. The same pattern will apply to withdrawing an adapter from a collective model.

## Alternatives considered
Deleting with no record (rejected: the old copy could be replayed and the name grabbed).
