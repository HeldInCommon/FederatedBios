# 0026. Vouching with a click, and who may see the waiting list

- **Status:** Accepted (built)
- **Date:** 2026-11-12
- **Builds on:** [0011](0011-joining-needs-a-vouch.md) (joining needs a vouch),
  [0018](0018-hearing-you-were-listed.md) (nodes ask, commons don't call)

## Context

With vouching (ADR 0011), someone without an invite waits on a commons'
pending list until a member vouches for them. Until now a member did that by
hand: open the commons' page, find the person under **Waiting for a vouch**,
copy their handle and the start of their key, go back to their own node, and
type both into **Vouch for someone**. The node then looked the full key up on
the commons' public `GET /api/pending`.

Phoenix asked to "add people who need vouching to everyone's vouching screen
for the commons they are in, and make it possible to vouch with a click."

To do that a member's node has to ask each commons who is waiting. The
commons should answer its own members, and only them. A commons has no member
sign-in (ADR 0002), so the question is how a node proves it speaks for a
listed member.

## Decision

- **The People tab lists who is waiting.** On a member's own node, the People
  tab shows, for each commons the member is listed on, everyone waiting for a
  vouch there: their name, handle and key fingerprint, each with a
  **Vouch for @…** button. One press signs and sends the same `Vouch` as
  before (no change to its format or to `canonical.rs`), and the page says
  what happened: "@amy is now listed on …", or the commons' refusal in plain
  words (for example the weekly limit). Typing a handle and key still works,
  folded under **Vouch by handle and key**, for a commons that hasn't been
  updated.
- **Proof of membership is a signature, not a secret.** The node signs a fresh
  `WaitingListRequest` (a `hic/v1` object; body: the commons' host) with the
  member's key and posts it to the commons' new `POST /api/waiting`. The
  commons answers only if:
  1. the signature is valid, and the object really is a `WaitingListRequest`
     (the type is part of what is signed, so a vouch can't be passed off as
     one);
  2. it names this commons (so a request made for one commons can't be
     replayed at another where the same person is listed);
  3. its author is a key listed here: the same test as for invites and
     vouches, using the same code (`check_signed_by_member` in
     `vouching.rs`); and
  4. it was made within **five minutes** of the commons' clock, either way.
  Otherwise it answers 403 with the reason. Someone still waiting is not
  listed, so they can't read the list they are on.
- **Only what was sent for review.** The answer holds each person's handle,
  name and full key (a vouch must name the key; the page shows its first 12
  characters). Not their bio, picture, places or node address. The People tab
  says so.
- **Asked on page load only.** The node asks when the member opens the People
  tab, all commons at once, with a 5-second timeout each. A commons that
  doesn't answer gets "Couldn't reach … Reload this page to try again." No
  background polling (as in ADR 0018).
- **Rate-limited with the inbox.** `/api/waiting` counts against the same
  limit as `/inbox` (120 requests a minute per address), checked before any
  signature work.
- **The Vouch button is a form** with the session's CSRF token, like every
  other form on a node, so another website can't press it for the member.
- **Steward behaviour is unchanged.**

## Why a signature (the trade-off, in plain words)

A node already holds the member's private key and already signs vouches with
it, and the commons already knows every listed member's public key. So the
node can prove "I hold a listed member's key" by signing something, and the
commons can check it, without either side learning anything new or storing
anything.

The alternatives each add a secret to keep: a password per member on the
commons (against "no sign-up on a commons", ADR 0002), or a token the commons
hands out when someone is listed (it would have to be stored on both sides,
could leak from either, and would need its own way to be replaced).

What the signature doesn't stop: if someone copies a request off the network
(possible over plain `http` on a shared network), they can replay it to the
same commons for up to five minutes and read the same list the member could.
They can't change it, use it on another commons, or make a new one. Five
minutes allows for computers whose clocks are a little out; a commons-issued
one-time challenge would close the gap, but needs a second round trip and
state on the commons, which isn't worth it for a list of handles that the
person who copied the request was already in a position to see on the wire.

## Consequences

- A new `hic/v1` type, `WaitingListRequest` (spec §5.3). It is checked and
  thrown away, never kept.
- The People tab takes as long as the slowest commons to answer, up to five
  seconds. Other tabs don't ask.
- **Unchanged, and worth a follow-up:** the commons' front page and
  `GET /api/pending` still show the pending list (handle, short key, the
  node's address) to anyone, as ADR 0011 decided. The new members-only list
  adds the person's name, which the public list doesn't show. Closing the
  public list is a separate decision (who is waiting becomes members-only
  everywhere), and the manual **Vouch by handle and key** route and the
  steward still depend on the commons page; it is left for Phoenix to decide.
- Tests: unit tests in `vouching.rs`, `vouch_list.rs` and
  `pages/owner_people.rs`; HTTP tests in
  `commons/tests/http/one_click_vouching.rs`, including the attacks (a
  stranger, someone still waiting, a member of another commons, a request
  for another commons, an old request, a forged author, a vouch posted as a
  request, a vouch for someone not waiting, a press without the CSRF token,
  the rate limit); and a real two-node-and-commons run in
  `scripts/test-federation.sh`.

## Alternatives considered

- **Leave it public** like `GET /api/pending` (rejected: Phoenix asked for
  members only, and names should not be public before someone is listed).
- **A password or token per member on the commons** (rejected: a new secret
  on both sides, and a sign-up of sorts on the commons).
- **A one-time challenge from the commons** (not now: closes the five-minute
  replay window at the cost of a second request and stored state; can be
  added later without changing the `Vouch` format).
- **The node checks in the background and keeps a copy** (rejected: polling,
  which ADR 0018 turned down, and a stale list).
