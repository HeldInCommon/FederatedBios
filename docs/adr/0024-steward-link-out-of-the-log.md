# 0024. The steward link stays out of the app's log

- **Status:** Accepted (built)
- **Date:** 2026-11-12
- **Builds on:** [0023](0023-steward-needs-a-key.md)

## Context
Since ADR 0023, the steward link (`/steward?key=…`) is the only way to open a
commons' steward view from another computer. Its key doesn't change, so
anyone who has it can approve people and make invite codes, from anywhere,
until the key is replaced.

The commons prints the link when it starts, for people who run it from the
command line. The desktop app copies everything its servers print into its
log file. The log folder is private to the user's account (ADR 0023), but the
log exists to be **sent to someone** for support, and people send logs
without reading them.

The November 2026 house-style review also found a second, related risk, and
this record settles both.

## Decision
- **The app hides the steward key before writing the log.** Each line its
  servers print goes through `steward::redact_for_log`, which replaces the
  key (and, in case a future message prints one, a launch token) with
  "(hidden: Go → Show steward link…)". The commons itself still prints the
  link, so the command line and Docker work as before.
- **The app shows the link on request.** For a computer that runs a commons,
  **Go → Show steward link…** opens a small window listing the link for each
  address the commons answers to (its public address first), each with a
  **Copy** button. The app reads the key from the commons' own
  `steward.json`. The command that returns it (`steward_links`) answers only
  that window, so no page shown in the main window can ask for it.
- **Accepted risk: the port check sends the launch token.** When the app
  checks whether its own server is on a port, it sends the launch token in
  `X-HIC-Launch` to whatever answers there. If another program holds that
  port, it receives the token and could open the steward view of this
  computer's commons until the app restarts (each launch makes a new token).
  We accept this: it needs a hostile program already running on the same
  computer, on that exact port, at launch, and such a program is a bigger
  problem than the commons. If that changes (for example, the app runs on
  shared machines), the fix is challenge and response: the app sends a fresh
  random value and the server answers with an HMAC of it under the token, so
  the token never leaves the app.

## Why
A log is the thing people most often hand to a stranger. A key that never
changes doesn't belong in it. Hiding it costs nothing for the command line,
which reads the commons' own output, and the menu keeps the link one click
away for the app.

## Consequences
- Logs from 0.7.0 and earlier still contain the key. Anyone who has sent one
  should treat that commons' steward key as known. (Replacing the key: stop
  the commons, delete `steward.json` from its data folder and start it again;
  it makes a new one.)
- The founding invite is still logged. It works once, for the first member,
  and is spent as soon as it is used.
- Tests: `the_steward_key_never_reaches_the_log`,
  `every_steward_key_and_launch_token_in_a_line_is_hidden`,
  `ordinary_lines_are_logged_as_they_are` and `a_link_for_each_address_public_first`
  (`src-tauri/src/steward.rs`).
