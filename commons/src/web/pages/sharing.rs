//! What a member sees after sharing to a commons, vouching or withdrawing.

use maud::{html, Markup};

use crate::canonical::short;
use crate::vouch_list::VouchResult;
use crate::web::layout::host_label;

/// Confirmation after a successful share. `held_signals_left_out` is true
/// when the commons runs an older version that took the profile without its
/// Held in Common Signals (decision record 0025).
pub fn shared(handle: &str, commons: &str, held_signals_left_out: bool) -> Markup {
    html! {
        header class="intro" {
            h1 { "Shared" }
            p class="lede" { "Your profile is now federated to " a href=(commons) { (commons) } ". They have a signed, verifiable copy." }
        }
        @if held_signals_left_out { (left_out_note(commons)) }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}

/// Says that an older commons took a share without its Held in Common
/// Signals: what it holds, and what still holds.
fn left_out_note(commons: &str) -> Markup {
    html! {
        aside class="caution" {
            p {
                strong { "Your Held in Common Signals didn't travel." }
                " " (host_label(commons)) " runs an older version of Held in Common, so it has your profile, licence and signals, but not your choice about withdrawing after training or your own terms. They'll go with the next share once it's updated."
            }
        }
    }
}

/// After sharing to a commons that lists newcomers only once vouched for.
/// `held_signals_left_out` as for [`shared`].
pub fn shared_pending(handle: &str, commons: &str, key: &str, csrf: &str, held_signals_left_out: bool) -> Markup {
    html! {
        header class="intro" {
            h1 { "Waiting for a vouch" }
            p class="lede" { (host_label(commons)) " has your signed profile, and will list it once a member vouches for you or its steward approves you." }
        }
        @if held_signals_left_out { (left_out_note(commons)) }
        p { "Ask someone you know there to vouch for you from their node. They'll need your handle and key:" }
        p class="recovery-code mono" { "@" (handle) " · " (short(key)) }
        section class="share-panel" {
            h2 class="field-label" id="code-label" { "Got an invite code or a room code?" }
            form method="post" action="/share" class="share-form" {
                input type="hidden" name="handle" value=(handle);
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="commons_url" value=(commons);
                div class="share-row" {
                    input name="invite" type="text" placeholder="Paste the invite code, or type the room code" required autocomplete="off" aria-labelledby="code-label";
                    button class="secondary" type="submit" { "Share with the code" }
                }
            }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}

/// After pressing Vouch for `@who`: listed, or the commons' reason in plain
/// words (decided by [`crate::vouch_list::vouch_result`]).
pub fn vouch_outcome(handle: &str, who: &str, commons_label: &str, result: &VouchResult) -> Markup {
    let people = format!("/@{handle}?tab=people");
    html! {
        header class="intro" {
            @match result {
                VouchResult::Listed => {
                    h1 { "Vouched" }
                    p class="lede" { "@" (who) " is now listed on " (commons_label) ", with a record that you vouched for them." }
                }
                VouchResult::Refused(why) => {
                    h1 { "Not vouched" }
                    p class="lede" role="alert" { (why) }
                }
            }
        }
        div class="actions" { a class="primary" href=(people) { "Back to People" } }
    }
}

/// Confirmation after a withdrawal.
pub fn withdrawn(handle: &str, commons_label: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Withdrawn" }
            p class="lede" { (commons_label) " has removed your profile, and any stories you shared there. It keeps only a note that @" (handle) " is yours, so nobody else can take the name there. You can share again whenever you like." }
        }
        div class="actions" { a class="primary" href={ "/@" (handle) } { "Back to your profile" } }
    }
}
