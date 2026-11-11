# 0001. One binary, two roles, fixed per data folder

- **Status:** Accepted
- **Date:** 2026-11-09

## Context
People need two things: their own server (a **node**) and shared places to gather (a **commons**). Two separate programs would double the code to maintain and test.

## Decision
One program, `hic-commons`, runs as either. The role is saved in `instance.json` the first time a data folder is used and can never change for that folder. The desktop app asks once, on first launch.

## Why
A node's folder holds its members' private keys; a commons' folder holds other people's shared copies. If a setting could quietly turn one into the other, a node's members could suddenly find their sign-up page on a public commons, or a commons could start minting identities. Fixing the role to the folder makes that impossible.

## Consequences
Running the other role means a new data folder. The desktop app can run both, each in its own folder, side by side ([0019](0019-node-and-commons-side-by-side.md)). Older hosted commons that relied on guessing the role from their name still work, with a warning, until they set `HIC_ROLE`.

## Alternatives considered
A setting that can change at any time (rejected: too easy to get wrong); two programs (rejected: double maintenance).
