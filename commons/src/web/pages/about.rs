//! How the system works, in plain language.

use maud::{html, Markup};

/// How the system works, in plain language.
pub fn about() -> Markup {
    html! {
        header class="intro" { h1 { "How this works" } p class="lede" { "A federated commons for creative data — govern the models, not only the data." } }
        section class="prose" {
            h2 class="section-h" { "Your node, your keys" }
            p { "You claim a human handle. The node mints a signing keypair and an encryption keypair for you and holds them — you never touch a private key." }
            h2 class="section-h" { "Content-addressed versions" }
            p { "Every publish is hashed into a permanent content address and signed by your key. You can withdraw a copy you shared to a commons, and it is removed there; a copy someone has already downloaded stays with them." }
            h2 class="section-h" { "Three reaches" }
            p { "Public is listed and indexable. Commons is listed but asks search engines not to index it. Members-only is encrypted to every member's public key — only signed-in members of this node can read it. The node itself holds those keys, so the encryption protects copies outside this node, not from whoever runs it." }
            h2 class="section-h" { "Govern the models" }
            p { "Each version carries a Creative Commons license and CC signals — machine-readable preferences for how models and datasets may learn from your work." }
            h2 class="section-h" { "Federation" }
            p { "A node sends a signed copy of a profile to a commons, which checks the signature before listing it. The messages are shaped like ActivityStreams, but this isn't yet full ActivityPub: other fediverse servers can't follow it." }
        }
    }
}
