//! The Go menu's items, worked out once: the app's menu bar shows them, and
//! so does the "This computer" panel in every page's sidebar (filled in by
//! the switcher script, `links.rs`), because a menu in the window's menu bar
//! is easy to miss, especially on Linux. Pure: no window, no menu objects.
//!
//! Each item has an id the app acts on (`on_menu` in `lib.rs`). A page asks
//! for one with a `hic-app://menu/<id>` link, which the app follows only from
//! its own servers' pages, like every `hic-app://` link; anything that
//! changes something still asks first in the setup window, except closing
//! the room, which only ever makes a server less reachable.

use serde::Serialize;

use crate::room;
use crate::settings::{Kind, Saved};

/// One entry in the Go menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    /// What the app does when it's chosen (`open:node`, `room-close:commons`,
    /// `public-address`…).
    pub id: String,
    /// What it says.
    pub label: String,
    /// Which part of the menu it's in, so the menu bar can put lines between
    /// them and the sidebar can leave out what it already shows.
    pub group: Group,
}

/// The parts of the Go menu, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    /// Show (or add) each server.
    Servers,
    /// Open or close each to the room; show its QR code.
    Room,
    /// Rename, the steward link, the public address, removing the local
    /// commons.
    Settings,
}

/// An item, for building the list. Pure.
fn item(id: String, label: String, group: Group) -> Item {
    Item { id, label, group }
}

/// Every Go-menu item for what this computer runs (`saved`), with `open`
/// listing the servers open to the room. Before the first-launch choice,
/// none. Pure.
pub fn items(saved: &Saved, open: &[Kind]) -> Vec<Item> {
    if !saved.ready() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for k in Kind::ALL {
        let label = match (k, saved.runs(k)) {
            // Every server says where it is: "Your node (on this computer,
            // only you)", "Thursday session (local commons, open to the room)".
            (_, true) => format!("{} ({})", saved.shown_name(k), room::place_text(saved, k, open.contains(&k))),
            (Kind::Node, false) => "Add your own node\u{2026}".to_string(),
            (Kind::Commons, false) => "Set up a commons\u{2026}".to_string(),
            (Kind::Practice, false) => "Start a local commons".to_string(),
        };
        out.push(item(format!("open:{}", k.key()), label, Group::Servers));
    }
    // Open or close each server to the room. Opening asks first (in the
    // setup window); closing doesn't need to.
    for k in Kind::ALL.into_iter().filter(|k| saved.runs(*k) && room::has_switch(*k)) {
        let is_open = open.contains(&k);
        let id = if is_open { format!("room-close:{}", k.key()) } else { format!("room-open:{}", k.key()) };
        out.push(item(id, room::menu_label(k, &saved.shown_name(k), is_open), Group::Room));
        if is_open {
            let whose = if k == Kind::Node { "my node".to_string() } else { saved.shown_name(k) };
            out.push(item(format!("room-show:{}", k.key()), format!("Show the QR code for {whose}"), Group::Room));
        }
    }
    for k in Kind::ALL.into_iter().filter(|k| saved.runs(*k)) {
        let label = match k {
            Kind::Node => "Rename your node\u{2026}".to_string(),
            _ => format!("Rename {}\u{2026}", saved.shown_name(k)),
        };
        out.push(item(format!("rename:{}", k.key()), label, Group::Settings));
    }
    // ADR 0024: the steward link isn't in the log, so the menu is where a
    // commons' steward finds it. ADR 0028: so is its public address.
    if saved.runs(Kind::Commons) {
        out.push(item("steward-link".into(), "Show steward link\u{2026}".into(), Group::Settings));
        out.push(item("public-address".into(), "Public address\u{2026}".into(), Group::Settings));
    }
    if saved.practice {
        out.push(item("remove-practice".into(), "Remove local commons\u{2026}".into(), Group::Settings));
    }
    out
}

/// Where an item sits in the sidebar's "This computer" panel. Each server's
/// actions live on its own card, so they say what they act on by where they
/// are, not by repeating its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    /// A server this computer doesn't run yet: a dashed "+" card at the end.
    Add,
    /// Shown on the server's card itself, not tucked away: only the actions
    /// for a server that is open to the room (close it, show its QR code),
    /// because that's the one state someone may need to act on quickly.
    Card,
    /// Under the card's Options button.
    Option,
    /// Under Options, set apart at the bottom: removes something.
    Danger,
}

/// One item in the sidebar's "This computer" panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SidebarItem {
    /// The Go-menu id the app acts on, as in [`Item::id`].
    pub id: String,
    /// What it says on the card: shorter than the menu bar's label, because
    /// the card already names the server ("Rename…", not "Rename spark…").
    pub label: String,
    /// The server it acts on (`node`, `commons`, `practice`): whose card it
    /// goes on, or for an [`Slot::Add`] card, which colour it takes.
    pub server: Option<&'static str>,
    /// Where on the panel it goes.
    pub slot: Slot,
}

/// Which server a Go-menu item acts on, from its id: the kind after the
/// colon (`rename:practice`), or the one server a stand-alone item is for.
/// Pure.
pub fn server_of(id: &str) -> Option<Kind> {
    match id {
        // Only a commons has a steward and a public address.
        "steward-link" | "public-address" => Some(Kind::Commons),
        "remove-practice" => Some(Kind::Practice),
        _ => id.split_once(':').and_then(|(_, kind)| Kind::parse(kind)),
    }
}

/// Where `item` goes in the sidebar, and what it says there. Pure.
fn sidebar_entry(item: Item) -> SidebarItem {
    let server = server_of(&item.id).map(Kind::key);
    // The part of the id before the colon says what the item does.
    let action = item.id.split_once(':').map_or(item.id.as_str(), |(action, _)| action);
    let (slot, label) = match action {
        "open" => (Slot::Add, item.label),
        "room-close" => (Slot::Card, "Close to the room".to_string()),
        "room-show" => (Slot::Card, "Show QR code".to_string()),
        "room-open" => (Slot::Option, "Open to the room\u{2026}".to_string()),
        "rename" => (Slot::Option, "Rename\u{2026}".to_string()),
        "remove-practice" => (Slot::Danger, item.label),
        _ => (Slot::Option, item.label),
    };
    SidebarItem { id: item.id, label, server, slot }
}

/// The items for the sidebar's "This computer" panel: all but the servers
/// already there as cards (adding one that isn't running stays, as an
/// [`Slot::Add`] card), each placed on its server's card. Pure.
pub fn sidebar_items(saved: &Saved, open: &[Kind]) -> Vec<SidebarItem> {
    items(saved, open)
        .into_iter()
        .filter(|i| {
            let shown_as_card = i.group == Group::Servers
                && i.id.strip_prefix("open:").and_then(Kind::parse).is_some_and(|k| saved.runs(k));
            !shown_as_card
        })
        .map(sidebar_entry)
        .collect()
}

/// Is `id` one of the Go menu's actions, as a page may ask for it? Only the
/// shapes [`items`] makes, with a real kind of server. Pure.
pub fn is_menu_id(id: &str) -> bool {
    /// Ids that end in a kind of server (`room-close:node`).
    const WITH_KIND: [&str; 5] = ["open:", "room-open:", "room-close:", "room-show:", "rename:"];
    /// Ids that stand alone.
    const ALONE: [&str; 3] = ["steward-link", "public-address", "remove-practice"];
    ALONE.contains(&id) || WITH_KIND.iter().any(|p| id.strip_prefix(p).and_then(Kind::parse).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A computer that runs a commons "spark" first, a node "potato", and a
    /// local commons.
    fn spark() -> Saved {
        Saved {
            role: Some("commons".into()),
            name: Some("spark".into()),
            other_name: Some("potato".into()),
            practice: true,
            ..Default::default()
        }
    }

    #[test]
    fn the_menu_lists_servers_then_the_room_then_settings() {
        let all = items(&spark(), &[]);
        assert_eq!(all[0].id, "open:node");
        assert!(all.iter().any(|i| i.id == "room-open:commons" && i.label == "Open spark to the room\u{2026}"));
        assert!(all.iter().any(|i| i.id == "public-address"));
        assert!(all.iter().any(|i| i.id == "remove-practice"));
        let groups: Vec<Group> = all.iter().map(|i| i.group).collect();
        let mut sorted = groups.clone();
        sorted.sort_by_key(|g| *g as u8);
        assert_eq!(groups, sorted, "in order");
    }

    #[test]
    fn an_open_server_offers_close_and_its_qr_code() {
        let all = items(&spark(), &[Kind::Node]);
        assert!(all.iter().any(|i| i.id == "room-close:node" && i.label == "Close my node to the room"));
        assert!(all.iter().any(|i| i.id == "room-show:node" && i.label == "Show the QR code for my node"));
        assert!(!all.iter().any(|i| i.id == "room-open:node"));
    }

    #[test]
    fn the_sidebar_leaves_out_the_servers_it_shows_as_cards() {
        let only_node = Saved { role: Some("node".into()), name: Some("potato".into()), ..Default::default() };
        let side = sidebar_items(&only_node, &[]);
        assert!(!side.iter().any(|i| i.id == "open:node"), "your node is a card already");
        assert!(side.iter().any(|i| i.id == "open:commons" && i.label == "Set up a commons\u{2026}"));
        assert!(side.iter().any(|i| i.id == "room-open:node"));
    }

    #[test]
    fn each_sidebar_action_goes_on_its_own_servers_card() {
        let side = sidebar_items(&spark(), &[Kind::Practice]);
        let find = |id: &str| side.iter().find(|i| i.id == id).unwrap_or_else(|| panic!("{id}"));
        // The card names the server, so the label doesn't.
        assert_eq!(find("rename:commons").label, "Rename\u{2026}");
        assert_eq!(find("rename:commons").server, Some("commons"));
        assert_eq!(find("room-open:node").slot, Slot::Option);
        // Open to the room: close it and its QR code are on the card itself.
        assert_eq!(find("room-close:practice").slot, Slot::Card);
        assert_eq!(find("room-show:practice").label, "Show QR code");
        assert_eq!(find("steward-link").server, Some("commons"));
        assert_eq!(find("remove-practice").slot, Slot::Danger);
        assert_eq!(find("remove-practice").server, Some("practice"));
    }

    #[test]
    fn a_server_this_computer_could_add_is_an_add_card() {
        let only_node = Saved { role: Some("node".into()), name: Some("potato".into()), ..Default::default() };
        let side = sidebar_items(&only_node, &[]);
        let add = side.iter().find(|i| i.id == "open:practice").expect("start a local commons");
        assert_eq!((add.slot, add.server), (Slot::Add, Some("practice")));
    }

    #[test]
    fn nothing_before_the_first_choice() {
        assert!(items(&Saved::default(), &[]).is_empty());
    }

    #[test]
    fn only_real_menu_actions_count() {
        for good in ["open:node", "room-close:practice", "rename:commons", "public-address", "remove-practice"] {
            assert!(is_menu_id(good), "{good}");
        }
        for bad in ["about-project", "open:elsewhere", "room-close:", "rm -rf", ""] {
            assert!(!is_menu_id(bad), "{bad}");
        }
    }
}
