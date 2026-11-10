# 0016. No hidden effects

- **Status:** Accepted
- **Date:** 2026-11-10

## Context
The restructure goal asked for "functions with no side effects". Taken
literally that is impossible: a server has to write files and answer
requests. Phoenix clarified what it means: *"when I have given you an
architecture directive, it's not without cases where it makes no sense, it's
more of a good practice guide than a hard rule. What I really mean is that
variables from outside of them get changed in them with no way to know. If a
function writes a file, it should ideally return it outside the function. The
idea is you don't want to blind your fellow devs to what is going on."*

A review against that found seven places where a function changed something
its name and signature didn't reveal: a `load` that also wrote, a role
`resolve` that wrote and printed, a rate limiter's `allow` that also counted,
three helpers that changed a member's file and swallowed errors, one helper
that hid four writes, a key switch that loaded, changed and saved in one go,
and an `announce` that also created a file.

## Decision
**No hidden effects.** Anyone reading a function's name and signature can
tell what it changes.
1. Work out changes in **pure functions** that take data and return the new
   value or a plan (`member::rekey`, `federation::plan_listing`,
   `HandleRec::with_shared_to`, `role::decide`).
2. Apply them in **one clearly named place** whose documentation lists what it
   writes (`Store::apply_listing`, `update_member`, `role::save`).
3. A function whose name sounds like a read (`load`, `check`, `inspect`)
   **only reads**. One that changes state says so in its name
   (`record_if_under_limit`, `save_…`, `update_…`, `apply_…`).
4. Functions that write **return a `Result`**; the caller decides what a
   failure means, visibly.
5. These are guides, not absolutes: shared in-memory state behind a lock
   (sessions, limits) and logging are fine when documented.

## Why
Hidden changes make code hard to follow, review and test: a collaborator
can't see what a call does without reading its body, and a swallowed error
hides a failure. Returning the change makes the pure part trivially testable
(see the tests in `member.rs` and `federation.rs`) and keeps every write in
sight.

## Consequences
Handlers read a little longer (load, compute, save), but every write is on
the page. New code is expected to follow this; `CONTRIBUTING.md` says so.

## Alternatives considered
Forbidding effects outright (not workable for a server); leaving effects
inside helpers with comments (rejected: comments drift, signatures don't).
