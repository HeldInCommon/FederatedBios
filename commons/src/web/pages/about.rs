//! How the system works, in plain language, with three diagrams drawn after
//! the workshop slides (heldincommon.org/workshop-slides): how your work is
//! held, how nodes share into a commons, and how someone joins.
//!
//! The diagrams live in `schematics.rs`, shared with "Run your own commons".

use maud::{html, Markup};

use super::schematics::{figure, FEDERATE_SVG, HELD_SVG, JOIN_SVG};

/// How the system works, in plain language. `on_node` adds a pointer to
/// running a commons.
pub fn about(on_node: bool) -> Markup {
    html! {
        header class="intro" { h1 { "How this works" } p class="lede" { "A federated commons for creative data — govern the models, not only the data." } }
        section class="prose" {
            h2 class="section-h" { "Your node, your keys" }
            p { "You claim a human handle. The node mints a signing keypair and an encryption keypair for you and holds them — you never touch a private key." }
            (figure(HELD_SVG, html! { "Read it left to right: " strong { "you" } " hold a handle and a key. Your " strong { "profile" } " and " strong { "stories" } " live on your node. Every change is saved as a " strong { "version" } " with a permanent fingerprint, and each version carries its " strong { "licence" } " and " strong { "signals" } "." }))
            h2 class="section-h" { "Content-addressed versions" }
            p { "Every publish is hashed into a permanent content address and signed by your key. You can withdraw a copy you shared to a commons, and it is removed there; a copy someone has already downloaded stays with them." }
            h2 class="section-h" { "Three reaches" }
            p { "Public is listed and indexable. Commons is listed but asks search engines not to index it. Members-only is encrypted to every member's public key — only signed-in members of this node can read it. The node itself holds those keys, so the encryption protects copies outside this node, not from whoever runs it." }
            h2 class="section-h" { "Govern the models" }
            p { "Each version carries a Creative Commons licence and signals — machine-readable preferences for how models and datasets may learn from your work." }
            h2 class="section-h" { "Federation" }
            (figure(FEDERATE_SVG, html! { "You send a " strong { "signed copy" } " to a commons; your original never moves. The commons checks the signature and gathers everyone's copies into one directory people can browse: no central vault, no landlord." }))
            p { "The messages are shaped like ActivityStreams, but this isn't yet full ActivityPub: other fediverse servers can't follow it." }
            h2 class="section-h" { "No sign-up. Someone vouches." }
            (figure(JOIN_SVG, html! { "Share your profile and you wait on the commons' list. A " strong { "member vouches" } " for you, or the " strong { "steward approves" } " you, and you're listed. An invite code from a member lists you straight away." }))
            @if on_node {
                h2 class="section-h" { "Hosting a commons" }
                p { "Anyone can run a commons for their workshop, class or collective, alongside their own node. " a href="/run-a-commons" { "Run your own commons" } " answers the common questions and lets you try a practice one." }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_diagram_is_hidden_from_screen_readers_and_explained_in_words() {
        let page = about(true).into_string();
        assert_eq!(page.matches("<svg").count(), 3);
        assert_eq!(page.matches(r#"aria-hidden="true""#).count(), 3);
        assert_eq!(page.matches("<figcaption>").count(), 3, "every diagram has its words");
    }
}
