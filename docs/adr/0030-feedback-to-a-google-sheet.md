# 0030. Feedback and the mailing list go to a Google Sheet

- **Status:** Accepted (built)
- **Date:** 2026-11-21
- **Builds on:** [0013](0013-google-fonts.md) (the only other third-party
  request), [0029](0029-share-terms-for-research.md) (sending something to
  the project, and only what's needed)

## Context
Phoenix asked for a place in the main menu where people can tell the team
what they think of the app (bugs, feature requests, anything else), and a
place to join the project's mailing list. Submissions should land in a
Google Sheet the team already uses, through a Google Apps Script, and only
people with an account should be able to send them. Phoenix asked for it to
look like writing a story.

## Decision
**D1. A Feedback link in the main menu, for everyone.** `GET /feedback` shows
the form only to someone signed in: a member on their node, or the steward on
a commons. Everyone else sees "Sign in to send feedback". On a commons, where
there is no sign-up (ADR 0002), it says to run a node. A node open to the
room keeps `/feedback` for its owner (it's not on the room's list, ADR 0027).

**D2. The form.** In the story form's style: three numbered steps (what it's
about, as cards: *something's broken*, *an idea*, *something else*; the
message, up to 5,000 characters; staying in touch), then "Send to the team".
Staying in touch is optional: an email, "Email me about this", and
"Join the mailing list". Nothing is ticked when the page
loads. Joining the mailing list needs no message. The page says where it
goes (a Google Sheet the team reads) and what is never sent.

**D3. What travels.** `feedback::Feedback`, format `hic-feedback/1`: the
kind, the message, the email (only if typed), the two ticks, the app's
version and `node` or `commons`. No handle, key, name, profile or story. The
rules (`feedback::check`) are pure and unit-tested.

**D4. Where it goes.** The server, not the browser, posts the JSON to a
Google Apps Script web app at `HIC_FEEDBACK_URL`, falling back to an address
compiled in at build time (`option_env!`, set from the repository variable
`HIC_FEEDBACK_URL` by the release builds). The script
(`tools/feedback-sheet/Code.gs`) checks the message again and adds a row to
the **Feedback** tab, and to **Mailing list** once per email. It answers
`{"ok": true}`; only that answer shows "Sent". The server follows Google's
redirect to read the answer. With no address set, the page says feedback
isn't set up.

**D5. A failed send keeps the words.** If the sheet can't be reached or says
no, the form comes back with everything typed, and "Nothing was sent". The
server writes nothing to its own data folder. The log says the send failed,
never what was written.

## Why
A sheet is what the team already reads and triages, with no new server to
run. Posting from the server keeps the page working without JavaScript and
avoids the browser talking to Google. Sending no handle keeps feedback
separate from anyone's identity unless they choose to give an email.

## Consequences
- **A new third-party request** (to Google), made only when someone presses
  Send. Google sees the sending computer's internet address and the message.
  The page says it goes to a Google Sheet.
- **The address isn't secret.** It ships in every app, so anyone could post
  to it directly; "only people with an account" holds for the app, not for
  the script. The script refuses anything not shaped like our form, makes
  formula-looking text inert, and takes at most 60 messages in 10 minutes.
  If spam arrives, the next step is for a node to sign each message with the
  member's key and the project's commons to relay only signed ones (Apps
  Script can't check Ed25519 signatures itself).
- **The `outbound` address check (ADR 0012) doesn't apply**: it guards
  addresses members type. This one is set by whoever runs or builds the
  server, like `HIC_RESEARCH_URL`.
- **The mailing list is a sheet tab, not a mailing service.** Whoever sends
  the mailing must say how to leave and remove people who ask. The form
  promises "You can ask to leave at any time".
- Tests: `commons/src/feedback.rs` (each rule), `web/pages/feedback.rs`, and
  `commons/tests/http/feedback.rs` (signed out and wrong CSRF send nothing; a
  fake script on a real port that redirects like Google; the sheet saying no;
  nowhere reachable; the steward on a commons).

## Alternatives considered
Keeping feedback on the project's commons, like research terms (ADR 0029)
(rejected for now: Phoenix wants it in a sheet, and the commons isn't always
running); a Google Form (rejected: no way to limit it to people with an
account, and a different look); posting from the browser (rejected: needs
JavaScript, and the page should work without it).
