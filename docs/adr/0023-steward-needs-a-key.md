# 0023. The steward view needs a key, not "this computer"

- **Status:** Accepted (built)
- **Date:** 2026-11-11
- **Replaces:** the "at the commons' own computer" part of [0017](0017-steward-approval.md)

## Context
ADR 0017 let anyone who opened `/steward` from the commons' own computer
become its steward, judged by the request coming from `127.0.0.1`. That
holds only while nothing else on the computer passes requests on.

The workshop commons runs in the desktop app on a DGX Spark and is made
public with Tailscale Funnel at `heldincommon.cerberus-broadnose.ts.net`.
Funnel hands every visitor's request to the commons from `127.0.0.1`, so on
11 November anyone on the internet who opened `/steward` got a steward
session: they could approve or decline people waiting and make invite codes.
The same happens behind any reverse proxy or tunnel on the same machine
(Caddy, nginx, cloudflared, ngrok).

A second problem made the obvious fix unsafe: the desktop app's launch token,
which tells the app which servers it started, was echoed by `/health` to
anyone, and was made from the clock and process id, so it was guessable.

## Decision
- **Where a request comes from proves nothing.** The steward view opens only
  with the **steward link** (`/steward?key=…`, printed in the commons' log at
  start-up) or the **desktop app's launch token** (`/steward?launch=…`).
- **The launch token is a secret.** The app makes it from 32 random bytes and
  passes it only to the servers it starts. `/health` never sends it back:
  the app sends it in `X-HIC-Launch` and the server answers `"ours": true` when
  it matches.
- **The app opens the steward view with the token** in its own window. The
  switcher row it adds to pages links to a commons through `hic-app://open/…`,
  so the token is never written into a page.
- The "You're at this commons' computer" hint on the directory is gone.

## Why
A proxy on the same machine is the normal way to put a home or lab server on
the internet, and the person setting one up can't be expected to know that it
turns "this computer" into "everyone". A key works the same whatever sits in
front of the commons.

## Consequences
- Someone running a commons from the command line, without the app, now uses
  the steward link from the log even on their own computer.
- An app and a server from different versions don't recognise each other
  (the health check changed). They ship together in one bundle, so this only
  matters to someone running a hand-built server under the app.
- Tests: `a_visitor_through_a_proxy_on_this_computer_is_not_the_steward`,
  `the_desktop_app_opens_the_steward_view_with_its_launch_token` and
  `health_never_reveals_the_launch_token` (`commons/tests/http/`).
