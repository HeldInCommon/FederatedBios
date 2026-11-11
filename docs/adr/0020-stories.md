# 0020. Stories: written on a node, shared as signed copies

- **Status:** Accepted (built)
- **Date:** 2026-11-11

## Context
Workshops need something people make together and choose how to share. The first one: listen to your street for a minute, then write a small story in ten minutes. A profile can't hold it (the bio is capped at 600 characters), and a story should carry its own terms, separate from the person's profile.

## Decision
A **story** is a new `hic/v1` type, `Story` (spec §5.4), signed by the member's key: an `id` that stays the same across versions, the handle, a title, the text (up to 12,000 characters), and its own reach, licence and signals. It is written and kept on the node (`my-stories/`), and can be exported as a text file.

A member shares it to a commons where they are **already listed**; the commons keeps the latest signed copy per (author, `id`) and shows it on `/stories`. A signed `StoryWithdraw`, addressed to one commons, deletes it there and leaves a record so the same copy can't come back. Withdrawing your profile from a commons takes your stories with it.

## Why
- **Joining stays the only gate.** Accepting stories only from listed members reuses vouching (decision record 0011) instead of inventing a second admission rule, and keeps strangers from filling a room's commons.
- **It's what already works.** Shares, withdrawals and tombstones follow the profile's proven path, so it's dependable on a workshop's Wi-Fi.
- **Its own terms.** A story is a piece of work; its licence and signals shouldn't be whatever the profile says.

## Consequences
- The commons holds a copy until it's withdrawn. "Only while I'm here" is proposed separately (decision record 0021).
- Members-only stories never leave the node, and aren't encrypted there (the node holds the keys anyway; see the Three reaches in ARCHITECTURE).
- A commons won't take a withdrawn copy back, so after a withdrawal the node re-signs its copy (dated after the withdrawal); sharing it again sends that newer version.
- A key change can't move a story (its signature is the old key's), so commons take down stories signed by a replaced key, and the node forgets where they were shared. Sharing again re-signs them with the new key.
- No editing yet: write a new story instead.

## Alternatives considered
Put the story in the bio (rejected: too short, and it would share the profile's terms). Accept stories from anyone (rejected: no gate). Read stories straight from the author's node (rejected for now: nodes don't accept connections, decision record 0004).
