//! Open to the room (decision record 0027): the owner's room page with its
//! QR code, the check page anyone in the room can open, and "Can't reach
//! someone?".
//!
//! Pure: the handler (`handlers/room.rs`) decides who may see what, makes the
//! QR code and passes everything in.

use maud::{html, Markup, PreEscaped};

/// Everything the owner's room page shows.
pub struct RoomView<'a> {
    /// This server's name ("Ana's laptop", "Thursday session").
    pub name: &'a str,
    /// True for a local commons, false for a node.
    pub is_commons: bool,
    /// The address people in the room open, which the QR code holds
    /// (`http://192.168.1.23:8080/@ana`). `None` while closed to the room.
    pub address: Option<&'a str>,
    /// The QR code for `address`, as an SVG drawing (made by the handler).
    pub qr_svg: Option<&'a str>,
    /// On a local commons open to the room: the code people type to join.
    pub room_code: Option<&'a str>,
    /// A hint about this computer's firewall, when the app could read one.
    pub firewall_hint: Option<&'a str>,
}

/// Group a room code in threes so it's easy to read out: `KTRMWX` → `KTR MWX`.
/// Pure.
fn spaced(code: &str) -> String {
    let letters: Vec<char> = code.chars().collect();
    letters.chunks(3).map(|chunk| chunk.iter().collect::<String>()).collect::<Vec<_>>().join(" ")
}

/// `GET /room`, for the owner only: the QR code and address to show the room
/// (big enough for a projector), the room code on a local commons, and how
/// to close it. While closed, it says how to open it.
pub fn room(view: &RoomView) -> Markup {
    html! {
        header class="intro" {
            h1 { "Open to the room" }
            @match view.address {
                Some(_) => p class="lede" { "People on this Wi-Fi can scan the code or type the address." },
                None => p class="lede" { (view.name) " is closed to the room. Only this computer can reach it." },
            }
        }
        @if let Some(address) = view.address {
            section class="room" aria-labelledby="room-address-label" {
                @if let Some(svg) = view.qr_svg {
                    // The drawing is made from the address alone; the address
                    // is also written out below, so nobody needs to scan.
                    figure class="room-qr" role="img" aria-label={ "QR code for " (address) } {
                        (PreEscaped(svg))
                    }
                }
                div class="room-words" {
                    p class="field-label" id="room-address-label" { "People in this room can open" }
                    p class="room-address" { code { (address) } }
                    @if let Some(code) = view.room_code {
                        p class="hint" { "Paste the link above into Share to a commons on your node, and you're in. Or type the address without the end, and the room code below." }
                        p class="field-label" { "Room code, to join this commons" }
                        // Shown in two groups of three for reading aloud;
                        // screen readers get the letters one by one instead.
                        p class="room-code" {
                            span aria-hidden="true" { (spaced(code)) }
                            span class="visually-hidden" { (code.chars().map(String::from).collect::<Vec<_>>().join(" ")) }
                        }
                        p class="hint" { "A new code each time the room opens. Closing the room retires it." }
                    }
                    @if view.is_commons {
                        p class="hint" { "Your steward view and this page stay on this computer." }
                    } @else {
                        p class="hint" { "They see your public profile and public stories. Signing in, your keys and anything members-only stay on this computer." }
                    }
                }
            }
        }
        section class="prose" {
            h2 class="section-h" { "Opening and closing" }
            p {
                "Open or close it from the app's " strong { "Go" } " menu. It also closes when you quit the app, "
                "if the app stops unexpectedly, and when this computer's network address changes (the app checks every minute)."
            }
            p { "Only open it on a network you trust: a classroom, a studio, your own hotspot." }
            @if let Some(hint) = view.firewall_hint {
                p class="hint" { (hint) }
            }
            p { a href="/room/help" { "Can't reach someone?" } }
        }
    }
}

/// `GET /join/{code}`: someone opened a room's join link: how to join from
/// their own node, with `link` to paste.
pub fn join_room(name: &str, link: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "Join " (name) }
            p class="lede" { "Join from the Held in Common app on your laptop." }
        }
        section class="prose" {
            ol {
                li { "Open Held in Common on your laptop (install it first if you need to) and make your profile on your node." }
                li { "On your profile, open " strong { "Sharing" } "." }
                li { "Paste this link into " strong { "Share to a commons" } " and press " strong { "Share" } ". It carries the room code, so you're listed straight away." }
            }
            p class="room-address" { code { (link) } }
            p class="hint" { "On a phone? Send this link to your laptop, or type it in there. If sharing says the code isn't right, the room has been opened again: scan the new code on the host's screen." }
        }
    }
}

/// `GET /room/check`: the page anyone in the room opens to test that they
/// can reach this computer at all.
pub fn room_check(name: &str) -> Markup {
    html! {
        header class="intro" {
            h1 { "It works" }
            p class="lede" { "You reached " (name) "." }
        }
        p { "Your device and this computer can talk to each other on this network." }
    }
}

/// `GET /room/help`: what to try when someone in the room can't reach
/// another person's node or the local commons, in the order to try it.
pub fn room_help() -> Markup {
    html! {
        header class="intro" {
            h1 { "Can't reach someone?" }
            p class="lede" { "Try these in order." }
        }
        section class="prose" {
            ol {
                li {
                    strong { "Are you both on the same Wi-Fi? " }
                    "Compare the first three numbers of the addresses: "
                    code { "192.168.1" } ".23 and " code { "192.168.1" } ".40 are on the same network."
                }
                li {
                    strong { "Does it reach its own phone? " }
                    "On the laptop that can't be reached, open its room page and scan the code with a phone on the same Wi-Fi, "
                    "then add " code { "/room/check" } " to the address. If that fails, that laptop's firewall is blocking it: "
                    "allow Held in Common on private networks."
                }
                li {
                    strong { "Can nobody reach anybody? " }
                    "Then the network keeps devices apart (\"client isolation\"), which is common on guest, school and "
                    "university Wi-Fi. Nothing in the app can get around it. Switch everyone to a phone hotspot or a small "
                    "travel router."
                }
            }
            p class="hint" { "The teacher's guide has the firewall steps for macOS, Windows and Linux." }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An open node's room page.
    fn open_node<'a>() -> RoomView<'a> {
        RoomView {
            name: "Ana's laptop",
            is_commons: false,
            address: Some("http://192.168.1.23:8080/@ana"),
            qr_svg: Some("<svg></svg>"),
            room_code: None,
            firewall_hint: None,
        }
    }

    #[test]
    fn the_address_is_written_out_beside_the_qr_code() {
        let page = room(&open_node()).into_string();
        assert!(page.contains("<svg></svg>"));
        assert!(page.contains(r#"aria-label="QR code for http://192.168.1.23:8080/@ana""#));
        assert!(page.contains("<code>http://192.168.1.23:8080/@ana</code>"), "the address is real text");
        assert!(page.contains(r#"href="/room/help""#));
    }

    #[test]
    fn a_closed_room_says_so_and_shows_no_code() {
        let page = room(&RoomView { address: None, qr_svg: None, ..open_node() }).into_string();
        assert!(page.contains("closed to the room"));
        assert!(!page.contains("<svg"));
    }

    #[test]
    fn a_local_commons_shows_its_room_code_spaced_for_reading_aloud() {
        let page = room(&RoomView { is_commons: true, room_code: Some("KTRMWX"), ..open_node() }).into_string();
        assert!(page.contains("KTR MWX"));
        assert!(page.contains("K T R M W X"), "screen readers get the letters one by one");
    }

    #[test]
    fn the_join_page_gives_the_link_to_paste() {
        let page = join_room("spark", "http://10.0.0.5:8180/join/KTRMWX").into_string();
        assert!(page.contains("<code>http://10.0.0.5:8180/join/KTRMWX</code>"));
        assert!(page.contains("Share to a commons"));
    }

    #[test]
    fn the_check_page_names_whose_computer_it_is() {
        assert!(room_check("Ana's laptop").into_string().contains("You reached Ana's laptop."));
    }
}
