# 0022. The ramp: one next step, and tabs that arrive when they're useful

- **Status:** Accepted (built)
- **Date:** 2026-11-11

## Context
A signed-in member's profile page showed everything at once: update, waiting, share, your commons, invite, withdraw, vouch, backup, keys. People met the whole system on day one. A design review (the "ramped experience" canvas) grouped it into five buckets (Profile, Sharing, People, Safety, Hosting) and a ramp that reveals them.

## Decision
- **One next step** at the top of the member's own profile, chosen from what has happened, in order: publish → share (to the suggested commons, one click) → waiting to be listed (with exactly what to send a voucher) → back up (once listed, or straight away for a members-only profile) → nothing. "Not now" on the backup step records it as done; backing up stays on the Account tab.
- **Tabs** on the member's own profile (`?tab=`): **Profile** always; **Sharing** and **Account** once a profile is published; **People** (invite, vouch) once they're listed somewhere. A tab not yet open falls back to Profile. Rare, risky actions (replace key, recover) sit behind "Advanced" on Account.
- **The front page** stops offering a signed-in member "Add your profile" (which would make a second identity). It offers their profile's next thing (publish, share to the commons, waiting, or their page on the commons) and, as a secondary link, "Create another profile for a new commons".
- **Rights are explained in full** on the publish form, open by default: licences are cards saying, in plain words, whether credit is required, whether changed versions are allowed, and whether commercial use is.
- **Confirmations** after sharing or withdrawing a story say what happened, where, and what it does and doesn't undo.
- The rules live in one pure module, `commons/src/ramp.rs`.

## Why
The rules from the review: one next step at a time; unlock by what happened, never by a setting; nothing disappears once shown for a reason that still holds; defaults over decisions, but rights explained in full for workshops; rare, risky actions out of the way.

## Consequences
- A member who never shares still sees Sharing and Account once they've published (the review had Account arriving with listing; keys and backup matter before that).
- The "new" dot on newly arrived tabs isn't built (it would need to remember which tabs a member has seen).
- Tests read forms from the tab that holds them (`?tab=account`, `?tab=sharing`).
