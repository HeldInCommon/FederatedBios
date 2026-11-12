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
- **Writing a story takes two steps**: the words (a new story starts as Listed in the commons, CC BY, Credit), then "Who can read it, and how", ending in "Share to the room" or "Keep it on this computer for now". The same rights step is "Change its terms" later, and "Edit story" reopens the first step.
- **The look follows heldincommon.org** (the "ramped experience" canvas): the logo and tagline, pink eyebrows over mono headings, section labels with a rule, numbered starbursts, choices in one outlined panel with the chosen one filled in ink, story cards with licence chips and "Verified", and a two-column story page with where it's shared and its terms. Members-only stories are described as staying on the node, never as encrypted (they aren't; decision record 0020).
- The rules live in one pure module, `commons/src/ramp.rs`.

**Update (2026-11-11):** the **People** tab is now always shown, so members can find vouching (Phoenix found it "gone" while not yet listed). Until a member is listed somewhere it explains how to get listed (publish, share, or wait for a vouch) instead of showing the invite and vouch forms, which a commons would refuse.

**Update (2026-11-12): a new look.** The heldincommon.org look above is replaced by the design Phoenix approved on 12 November: each page floats on a generated "quilt" of card patterns (one of eight, chosen by the page's title; `scripts/make-quilts.py`), dealt mostly around the edges and dialled right back on phones. The rail (logo, this computer's servers) and the page panel are rounded; everything inside is square and chunky, with no thin lines, and cards rise onto a hard ink shadow only under the pointer. Oxblood is used only for things that need someone to act (the next step, people waiting for a vouch). The ramp's next-step card carries its step number in a starburst, a different colour for each step; hue never carries meaning alone.

## Why
The rules from the review: one next step at a time; unlock by what happened, never by a setting; nothing disappears once shown for a reason that still holds; defaults over decisions, but rights explained in full for workshops; rare, risky actions out of the way.

## Consequences
- A member who never shares still sees Sharing and Account once they've published (the review had Account arriving with listing; keys and backup matter before that).
- The "new" dot on newly arrived tabs isn't built (it would need to remember which tabs a member has seen).
- Tests read forms from the tab that holds them (`?tab=account`, `?tab=sharing`).
