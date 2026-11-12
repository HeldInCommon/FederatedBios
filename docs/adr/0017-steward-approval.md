# 0017. The steward can approve people, and invites can be emailed

- **Status:** Accepted; recognising the steward "at this computer" is replaced by [0023](0023-steward-needs-a-key.md)
- **Date:** 2026-11-10

## Context
With vouching (ADR 0011), someone without an invite waits until a member
vouches for them. Phoenix asked that whoever runs a commons can also approve
them "by a click", and that invites can be generated and emailed. A commons
has no member sign-in, so it needed a way to recognise its steward.

## Decision
- **The steward view.** A commons recognises its steward when they open
  `/steward` either **at the commons' own computer** (the desktop app's
  commons window opens there directly) or with the **steward link** the
  commons prints in its log at startup (`/steward?key=…`; the key is kept in
  `steward.json` in the data folder). That starts a steward session.
- In the steward view, each person waiting has **Approve** and **Decline**.
  Approval is recorded as an admission with route `steward`.
- The steward can **make invite codes** (`hic-invite-…`, single use), which
  admit a newcomer like a member's invite (route `steward-invite`).
- **Emailing invites:** every invite page (member or steward) has
  **Email this invite**, a `mailto:` link that opens the person's own mail
  app with a ready-written message (how to install the app, the commons
  address, the code). Servers never send mail, so no email address passes
  through a node or a commons. The desktop app hands `mailto:` links to the
  system mail app.
- Every steward form carries the session's CSRF token.

## Why
"At this computer" covers the common case (a workshop host running a room
commons from the app) with no setup. The steward link covers Docker and
hosted commons, where requests don't come from the same computer. `mailto:`
gives a ready email without the commons holding anyone's address or running
a mail server.

## Consequences
Anyone who has the steward link, or who sits at the commons' computer, can
approve people; the link must be kept private (the log line says so). To
change it, delete `steward.json` and restart. Steward approvals aren't
limited by `HIC_VOUCH_LIMIT`.

## Alternatives considered
Steward accounts with passwords on the commons (rejected for now: more to
build and secure, and against "no sign-up on a commons"); sending invites by
email from the server (rejected: needs a mail server and handles addresses).
