# 0029. Sharing your terms for research

- **Status:** Accepted (built)
- **Date:** 2026-11-18
- **Builds on:** [0025](0025-held-in-common-signals.md) (own terms for AI),
  [0012](0012-block-internal-addresses-on-share.md) (where a node may send)

## Context
People write their own terms for AI on a story's rights step (ADR 0025), and
choose a licence, CC signals, who can see it and how it travels. The project
wants to learn how people set terms for their work. Phoenix asked for a way
for people to send us those terms, kept on the project's machine (the online
commons behind Tailscale Funnel) in a folder called `terms`, along with how
each person describes themselves in terms of how they might use Held in
Common ("an artist sharing work with a collaborator", "a member of a local
art collective"). Phoenix was clear that we want the terms, not anyone's
data.

## Decision
**D1. Opt in, every time.** Under "Your terms for AI, in your own words" on
a story's rights step, a checkbox: "Share these terms with the Held in
Common project for research". It is never ticked when the page loads and is
never remembered. Each save with it ticked sends one copy. The page names where it goes by name ("the Held in Common
Collective Commons node", from `HIC_SHARE_DEFAULT_NAME`), never by address.

**D2. The terms, never the person.** A gift (`research::TermsGift`, format
`hic-terms-research/1`) holds exactly: the own terms, "withdraw even after
training", who can see it, the licence, the CC signals, how it travels, and
the self-description (optional, up to 300 characters). No handle, key, name,
story title, story text or story id. The page says so in those words. The
commons refuses a gift with any other field (`deny_unknown_fields`), so a
later change can't quietly send more.

**D3. Where it goes.** A node sends `POST <address>/research/terms`, where
the address is `HIC_RESEARCH_URL`, or else the node's default online
commons (`HIC_SHARE_DEFAULT`, which release builds set to the project's
commons). With neither, the option isn't offered. The send goes through the
same address rules as sharing (ADR 0012).

**D4. How it's kept.** Any commons answers `/research/terms`; a node answers
"not found". The commons checks the format, the lengths and that every
choice is one the form offers, then writes the gift with the time it
arrived to `terms/<time>-<random>.json` in its data folder. It doesn't store
the sender's address; that is used only for the rate limit shared with
`/inbox`.

**D5. Saving comes first.** The terms are saved on the node before anything
is sent. If the project can't be reached, the story's page says "Not shared
for research … Nothing was sent", and the terms stay saved.

## Why
The terms are the research question; who wrote them is not. Keeping the
gift to a fixed list of fields, refused if anything is added, makes "only
the terms" something the code enforces, not only a promise on the page.
Asking each time keeps consent close to what's being sent.

## Consequences
- Own terms are free text: someone could write their name or other personal
  details into them. The page says the terms themselves are sent.
- The project's commons must be running (the app open, Funnel on) for a
  gift to arrive; otherwise the person is told nothing was sent.
- Any commons whose address a node is set to will collect gifts; only the
  project's address is set in release builds.
- Only the story rights step offers it; the profile form does not.
- Tests: `commons/src/research.rs` (each rule) and
  `commons/tests/http/research.rs` (a node sending to a commons on a real
  port, nothing sent without the box, an unreachable address, extra fields
  refused).

## Alternatives considered
Remembering consent per member (rejected: a one-time yes would keep sending
later terms without asking); sending the handle so answers can be linked
over time (rejected: Phoenix asked for the terms, not the person's data); a
separate research server (not needed: a commons can keep the files, and the
address is configurable).
