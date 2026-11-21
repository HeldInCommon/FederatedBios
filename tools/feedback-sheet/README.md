# Feedback sheet

The Google Apps Script that receives feedback and mailing list sign-ups from
the app's **Feedback** page and adds them to a Google Sheet. Why it works this
way: [decision record 0030](../../docs/adr/0030-feedback-to-a-google-sheet.md).

| File | What it is |
|---|---|
| `Code.gs` | The whole script: checks each message, then adds a row to the **Feedback** tab and, for a sign-up, the **Mailing list** tab. |

## Setting it up (once)

1. Make a new Google Sheet in the project's shared Drive (`_Research/`), for
   example "Held in Common feedback". Only the team should have access.
2. In the sheet: **Extensions → Apps Script**. Delete the starter code and
   paste in all of `Code.gs`. Save.
3. **Deploy → New deployment**. Choose the type **Web app**, then:
   - *Execute as:* **Me** (so it can write to your sheet);
   - *Who has access:* **Anyone** (the app's server isn't signed in to
     Google; the script checks every message itself).
4. **Deploy**, allow the permissions it asks for, and copy the **Web app
   URL** (`https://script.google.com/macros/s/…/exec`).
5. Give that address to the server as `HIC_FEEDBACK_URL`:
   - for a server you run yourself, set the environment variable;
   - for the desktop app, set the repository variable `HIC_FEEDBACK_URL`
     (Settings → Secrets and variables → Actions → **Variables**) so release
     builds bake it in. It isn't a secret, so a variable is right.

The **Feedback** and **Mailing list** tabs (with header rows) are made the
first time a message arrives.

## Changing the script

After editing `Code.gs`, use **Deploy → Manage deployments → Edit → Version:
New version**. That keeps the same address. A *new deployment* gets a new
address, and every app would need the new one.

## Checking it works

Sign in to a node with `HIC_FEEDBACK_URL` set, open **Feedback**, and send a
test. A row should appear within a few seconds. If the page says "Nothing was
sent", the server's log says why (`feedback: couldn't reach the sheet: …`).
The usual cause is step 3: access not set to **Anyone**, so Google answers
with a sign-in page instead of `{"ok": true}`.

## What's in each row

**Feedback:** when it arrived, the kind (`bug`, `idea`, `other`), the
message, the email (if given), whether they may be emailed about it, whether
they joined the mailing list, the app version, and whether it came from a
node or a commons. **Status** starts as "New" and **Notes** is empty, for the
team to use when triaging.

**Mailing list:** when they joined, their email (once per address), and the
app version.

Nothing else is sent: no handle, name, profile or story (see
`commons/src/feedback.rs`).

## Limits

- The script refuses anything that isn't the shape the form sends, and
  takes at most 60 messages in any 10 minutes.
- "Only people with an account" is enforced by the app, not here: anyone who
  finds the address could post a well-formed message. If spam becomes a
  problem, the next step is in the decision record.
- People leave the mailing list by asking; whoever sends the mailing should
  include how (for example a reply address), and delete their row.
