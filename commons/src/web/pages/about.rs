//! How the system works, in plain language, with the diagrams the
//! workshop slides use (`docs/architecture.html`): how your work is held,
//! how nodes share into a commons, how many commons are joined by the
//! people on them, and how someone joins. A last section shows what's
//! coming soon: collectives agreeing terms with each other.
//!
//! The diagrams live in `schematics.rs`, shared with "Run a commons".

use maud::{html, Markup};

use super::schematics::{
    figure, figure_with_phone, AGREE_SVG, COLLECTIVES_SVG, FEDERATE_SVG, HELD_SVG, JOIN_NARROW_SVG, JOIN_SVG,
    TOPOLOGY_SVG,
};

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
            h2 class="section-h" { "Many commons, joined by people" }
            p { "There's no central network of commons. Each commons is its own directory, and your node can share to as many as you like." }
            (figure(TOPOLOGY_SVG, html! { "Commons never connect to each other or contact a node: every line starts at a " strong { "node" } ". Each node shares its own " strong { "signed copy" } " to the commons it chooses, so people are the links between commons, with one key on each." }))
            h2 class="section-h" { "No sign-up. Someone vouches." }
            (figure_with_phone(JOIN_SVG, JOIN_NARROW_SVG, html! { "Share your profile and you wait on the commons' list. A " strong { "member vouches" } " for you, or the " strong { "steward approves" } " you, and you're listed. An invite code from a member lists you straight away." }))
            h2 class="section-h" { "Coming soon: collectives in agreement" }
            p { "Collectives will be able to agree terms with each other, so one collective can draw on another's work under terms both sign. Each member still decides for their own work." }
            (figure(COLLECTIVES_SVG, html! { "Each pair of collectives agrees its own terms, and the dashed line is that agreement, signed by both stewards. No agreement, no line: A and D share nothing, even though both deal with B." }))
            (figure(AGREE_SVG, html! { "Read it top to bottom. Collective B reads what A lists and on what licence, then proposes terms. A's steward can't sign away anyone's work: " strong { "each member decides" } ", and the agreement covers only those who said yes. A " strong { "withdrawal" } " reaches B the same way the listings did, and the next version leaves it out. Files stay on members' nodes." }))
            @if on_node {
                h2 class="section-h" { "Hosting a commons" }
                p { "Anyone can run a commons for their workshop, class or collective, alongside their own node. " a href="/run-a-commons" { "Run a commons" } " answers the common questions and lets you open a local one for a workshop or a session." }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_diagram_is_hidden_from_screen_readers_and_explained_in_words() {
        // Six figures; the vouching one has a second drawing for phones,
        // so seven SVGs, every one hidden and every figure captioned.
        let page = about(true).into_string();
        assert_eq!(page.matches("<svg").count(), 7);
        assert_eq!(page.matches(r#"aria-hidden="true""#).count(), 7);
        assert_eq!(page.matches("<figcaption>").count(), 6, "every diagram has its words");
    }

    #[test]
    fn what_is_not_built_yet_is_labelled_coming_soon() {
        // Collectives agreeing terms isn't built: the page must not read as
        // if it were.
        let page = about(true).into_string();
        assert!(page.contains("Coming soon: collectives in agreement"));
    }
}
