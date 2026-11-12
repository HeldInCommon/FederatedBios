# Decision records

Short records of the decisions that shape Held in Common: what was decided,
why, and what it costs. They exist so anyone (including future us) can see
why the system is the way it is before changing it.

**When to write one:** a choice that would be expensive to reverse, that
affects security or the signed format, or that someone will ask "why?" about.
Copy the layout of an existing record. Don't rewrite an accepted record;
write a new one that replaces it and mark the old one "Superseded by NNNN".

Statuses: **Accepted** (in place), **Accepted, to build**, **Deferred**,
**Superseded**.

| # | Decision | Status | Date |
|---|---|---|---|
| [0001](0001-one-binary-two-roles.md) | One binary, two roles, fixed per data folder | Accepted | 2026-11-09 |
| [0002](0002-commons-has-no-sign-up.md) | A commons has no sign-up | Accepted | 2026-11-09 |
| [0003](0003-handles-unique-per-commons.md) | Handles are unique per commons, and belong to the first key | Accepted | 2026-11-09 |
| [0004](0004-node-listens-only-locally.md) | A node listens only on its own computer | Accepted | 2026-11-10 |
| [0005](0005-passphrases-argon2id.md) | Passphrases are hashed with Argon2id, and guessing is limited | Accepted | 2026-11-10 |
| [0006](0006-withdrawal-and-rollback.md) | Withdrawal leaves a tombstone; old versions can't be replayed | Accepted | 2026-11-10 |
| [0007](0007-identity-backup.md) | Identity backups, encrypted with the passphrase | Accepted | 2026-11-10 |
| [0008](0008-app-trusts-only-its-server.md) | The desktop app only shows its own server | Accepted | 2026-11-10 |
| [0009](0009-signed-format-specification.md) | A written specification for everything we sign | Accepted (built for new types) | 2026-11-10 |
| [0010](0010-recovery-key.md) | A recovery key, made at sign-up and kept offline | Accepted (built) | 2026-11-10 |
| [0011](0011-joining-needs-a-vouch.md) | Joining a commons needs a vouch | Accepted (details decided) | 2026-11-10 |
| [0012](0012-block-internal-addresses-on-share.md) | Sharing can't be aimed at internal addresses | Accepted (built) | 2026-11-10 |
| [0013](0013-google-fonts.md) | Pages keep loading Google Fonts | Accepted | 2026-11-10 |
| [0014](0014-automatic-updates.md) | Automatic app updates | Deferred (on the to-do list) | 2026-11-10 |
| [0015](0015-macos-signing.md) | macOS builds are signed with a personal Developer ID | Accepted | 2026-11-09 |
| [0016](0016-no-hidden-effects.md) | No hidden effects | Accepted | 2026-11-10 |
| [0017](0017-steward-approval.md) | The steward can approve people, and invites can be emailed | Accepted | 2026-11-10 |
| [0018](0018-hearing-you-were-listed.md) | How a node hears it was listed | Accepted | 2026-11-10 |
| [0019](0019-node-and-commons-side-by-side.md) | One computer can run a node, a commons and a practice commons | Accepted (built) | 2026-11-11 |
| [0020](0020-stories.md) | Stories: written on a node, shared as signed copies | Accepted (built) | 2026-11-11 |
| [0021](0021-held-not-copied.md) | "Only while I'm here": a commons that lists but doesn't keep | Accepted (built) | 2026-11-12 |
| [0022](0022-the-ramp.md) | The ramp: one next step, and tabs that arrive when they're useful | Accepted (built) | 2026-11-11 |
| [0023](0023-steward-needs-a-key.md) | The steward view needs a key, not "this computer" | Accepted (built) | 2026-11-11 |

Background for the 2026 decisions: [the November 2026 code review](../REVIEW-2026-11.md).
