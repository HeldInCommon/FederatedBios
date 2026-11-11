# 0021. "Only while I'm here": a commons that lists but doesn't keep

- **Status:** Proposed
- **Date:** 2026-11-11

## Context
The project's promise is that originals stay put and signals travel. Stories (decision record 0020) are shared as copies: the commons keeps the text until it's withdrawn. In a workshop it should be possible to choose the opposite: *people in the room can read my story while I'm here, and nothing is left behind when I go.*

Reading straight from the author's node isn't an option: a node accepts connections only from its own computer (decision record 0004), because it also holds the sign-in page and keys. Opening a read-only door on every participant's laptop would add attack surface everywhere.

## Proposal
A second way to share, chosen per story in "How it travels":

1. The node sends the commons only a **signed card**: title, author, first line, licence, signals, and the SHA-256 of the full text. No text.
2. The node keeps **one outgoing connection** open to that commons (a WebSocket or long-poll it opens itself; the node still accepts nothing).
3. When someone opens the story, the commons asks for it over that connection, passes the text through to the reader, and **keeps nothing**: no file, no cache.
4. The reader's page (and any reader's node) checks the text against the signed hash, so a commons can't alter it in transit.
5. When the author's laptop sleeps, the app closes, or they withdraw, the connection closes and the card shows "not here right now", or disappears on withdrawal.

## Trade-offs
- **For:** no copy at rest on the commons; withdrawal is instant and complete; the federation model made tangible for participants.
- **Against:** readable only while the author is online; a persistent connection per node; the commons sees the text in passing (it could log it, which the protocol can't prevent: only trust in the steward can); more to test (reconnects, timeouts, many readers).
- **Unchanged:** anyone who reads a story can copy it. This controls storage and withdrawal, not memory or screenshots. Licence and signals remain how reuse is governed.

## To decide before building
Transport (WebSocket vs long-poll); whether a commons may cache for seconds to serve a room at once; what readers see when the author is away; whether the card itself should expire.
