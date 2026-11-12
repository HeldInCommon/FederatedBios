# 0021. "Only while I'm here": a commons that lists but doesn't keep

- **Status:** Accepted (built)
- **Date:** 2026-11-11 (proposed); 2026-11-12 (decided and built, for stories)

## Context
The project's promise is that originals stay put and signals travel. Stories (decision record 0020) are shared as copies: the commons keeps the text until it's withdrawn. In a workshop it should be possible to choose the opposite: *people in the room can read my story while I'm here, and nothing is left behind when I go.*

Reading straight from the author's node isn't an option: a node accepts connections only from its own computer (decision record 0004), because it also holds the sign-in page and keys. Opening a read-only door on every participant's laptop would add attack surface everywhere.

## Decision
A second way to share a story, chosen per story in "How it travels": **Keep a copy on the commons** (as before) or **Only while I'm here**.

1. The node sends the commons only a **signed card** (`StoryCard`, spec §5.5): title, author, first line, word count, licence, signals, the SHA-256 of the full text, and the commons it's for. No text. It goes to `/here/card`, not `/inbox`.
2. The node keeps **one outgoing connection** per (commons, member) open: a **long poll** to `/here/poll`, signed each time (`StoryRelay`). The node opens it; it still accepts nothing.
3. When someone opens the story, the commons hands the request to that poll. The node answers in its next poll with the text. The commons checks the text against the card's signed hash, passes it to that reader, and **keeps nothing**: no file, no cache.
4. Anyone can check the text without trusting the commons: `GET /stories/<keyid>/<id>/here.json` returns the signed card with the text.
5. When the author's laptop sleeps or the app closes, the polls stop and readers see the card with **"Not here right now: @handle shares this only while they're online."** A withdrawal (the existing `StoryWithdraw`) deletes the card at once.

### What was decided, and why
- **Transport: long-polling, not WebSocket.** Long-polling is plain HTTP requests, which the server (axum) and the node's client (reqwest) already make, so it needs **no new dependency**: axum's WebSocket support needs its `ws` feature, and the node would need a WebSocket client crate. It also passes through every proxy a plain request does (Tailscale Funnel, hosted proxies). The cost is a fresh request about every 25 seconds per connected author, and up to a tenth of a second extra for a reader. For a workshop room this is small.
- **No caching on the commons, not even for seconds.** It works without one: each reader's request is answered by the node in about one round trip. The text is held in memory only between the node's answer and the reader's page taking it, then dropped; it is never written to disk. Pages and `here.json` are sent with `Cache-Control: no-store` so browsers and proxies don't keep a copy either. A busy room is protected by a limit instead: at most 16 readers wait for one author at once; the next is asked to try again in a moment.
- **When the author is away**, readers see the signed card (title, first line, terms) and "Not here right now: @handle shares this only while they're online." The commons counts an author as here while a poll is open, or for 10 seconds after one ends. If the node doesn't answer a reader within 8 seconds, the author is marked away, so the next reader is told at once.
- **The card doesn't expire by itself.** It stays listed (as "not here right now") until withdrawn; withdrawal removes it at once, and readers waiting at that moment are told it's gone. Withdrawing a profile, or a key change, removes the cards too, as with copies.
- **Only the author's node can answer.** Every poll is signed by the author's key and must come from a key with a card on that commons. An answer counts only for a request handed to that same key's poll. Each poll carries a number higher than the last (`seq`) and a signed time within five minutes, so an old poll can't be replayed. Another node that knows the words still can't answer for the author.
- **Older commons refuse clearly.** A commons from before this decision has no `/here/card` and answers 404; the node then tells the member "this commons doesn't support Only while I'm here yet", and suggests keeping a copy instead.
- **No change to existing signed bytes.** `StoryCard` and `StoryRelay` are new `hic/v1` types (spec §4 allows new types within `hic/v1`); `Story`, `StoryWithdraw` and the golden vectors are untouched. The "How it travels" choice is stored beside the story on the node, not inside its signed body. New vectors 07 and 08 are checked by `cargo test --test spec_vectors` and, independently, by `scripts/check_spec_vectors.py`.
- **A copy and a card replace each other.** A commons keeps one or the other per story; the newer wins. The node dates whatever it sends after whatever is there, so switching always moves forward.

## Trade-offs
- **For:** no copy at rest on the commons; withdrawal is instant and complete; the federation model made tangible for participants.
- **Against:** readable only while the author is online; one request held open per connected author; the commons sees the text in passing (it could log it, which the protocol can't prevent: only trust in the steward can); a commons restart forgets the poll numbers (the signed time still limits replays to five minutes). A reader's browser on a plain-`http` room commons can't check the hash itself (browsers only offer SHA-256 to scripts on secure pages), so the page reports the commons' check, and `here.json` is there for a reader's own node or a script.
- **Unchanged:** anyone who reads a story can copy it. This controls storage and withdrawal, not memory or screenshots. Licence and signals remain how reuse is governed.

## Consequences
- Code: `commons/src/here.rs` (the card and poll, pure), `relay.rs` (the commons' presence and waiting readers, in memory, as explicit state enums), `relay_node.rs` (the node's connections), `web/handlers/here.rs`, `web/pages/here.rs`. Tests: unit tests in each, `tests/http/here.rs` (including the attacks), and a real run in `scripts/test-federation.sh` (share, read, stop the node, "Not here right now", restart, read again, withdraw).
- `HIC_RELAY_HOLD` (default 25 s) and `HIC_RELAY_WAIT` (default 8 s) tune the timings.
- A commons shutting down waits for open polls to finish (up to the hold time).
- Only stories use this for now; profiles always travel as copies.

## Alternatives considered
WebSocket (rejected for now: new dependencies, more to proxy, little gain at workshop scale). A short cache on the commons to serve a room at once (rejected: not needed to work, and it would leave the text at rest, which is what this choice promises not to do). Expiring cards (rejected: the card is the author's signed, withdrawable statement; "not here right now" says enough). Reading straight from the author's node (rejected: nodes don't accept connections, decision record 0004).
