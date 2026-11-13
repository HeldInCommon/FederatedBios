//! How pages move between this computer's servers. Pure: no window, no
//! state.
//!
//! - **`hic-app://open/<kind>` links.** A page served by one of the app's
//!   servers (the node's "Run a commons" guide, a commons' steward
//!   view) can't call the app, but it can link somewhere. The main window
//!   catches navigations to `hic-app://` (as it does `mailto:`) and the app
//!   opens that kind, setting it up first if needed. Only links followed
//!   from the app's own servers count (see `lib.rs`).
//! - **The switcher.** A row of buttons, added to every server page, for
//!   the other servers this computer runs, so moving between them is one
//!   click. On a commons elsewhere it is the way back.

use serde::Serialize;

use crate::network::local_url;
use crate::settings::{Kind, Saved};

/// The scheme of the app's own links.
pub const SCHEME: &str = "hic-app";

/// The project's site: "About the project" in the Go menu and on every page.
pub const PROJECT_URL: &str = "https://heldincommon.org";

/// Should a link to `host` open in the person's own browser rather than in
/// the app's window? Only the project's site: it isn't a node or a commons,
/// so showing it in the window would only hide their node.
pub fn opens_in_browser(host: Option<&str>) -> bool {
    host.is_some_and(|h| h == "heldincommon.org" || h.ends_with(".heldincommon.org"))
}

/// What a `hic-app://` link asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum AppLink {
    /// Show this kind of server, setting it up (or, for practice, starting
    /// it) if this computer doesn't run it yet.
    Open(Kind),
    /// Do what this Go-menu item does (`go_menu.rs`), from the sidebar's
    /// "This computer" panel: `hic-app://menu/<id>`.
    Menu(String),
}

/// Read a `hic-app://` link. Anything else, or a link this app doesn't
/// know, is `None`.
pub fn parse(url: &str) -> Option<AppLink> {
    let rest = url.strip_prefix(SCHEME)?.strip_prefix("://")?;
    let rest = rest.trim_end_matches('/');
    if let Some(id) = rest.strip_prefix("menu/") {
        return crate::go_menu::is_menu_id(id).then(|| AppLink::Menu(id.to_string()));
    }
    let kind = rest.strip_prefix("open/")?;
    Kind::parse(kind).map(AppLink::Open)
}

/// The kind of server shown on `port`, if it is one of ours.
pub fn kind_on(saved: &Saved, port: u16) -> Option<Kind> {
    saved.running().into_iter().find(|k| saved.port_of(*k) == port)
}

/// One switcher button.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Place {
    /// What the button says.
    pub label: String,
    /// Where it goes: that server's page in this window. A commons' steward
    /// view goes through the app (`hic-app://open/…`), which adds the launch
    /// token, so the token is never written into a page (ADR 0023).
    pub url: String,
    /// That server's colour: the background of its button in the corner bar
    /// the app adds to pages that have no rail of their own.
    pub tone: &'static str,
    /// Which kind of server it is (`node`, `commons`, `practice`): the rail's
    /// card picks its starburst colour from this.
    pub kind: &'static str,
    /// The address other people reach it at (`172.20.10.10:8180`), for a
    /// commons or a server open to the room: shown under its card, as text
    /// to copy. `None` for a server only this computer can reach.
    pub address: Option<String>,
    /// What kind of server it is ("Local commons"), or for your node its
    /// name ("Studio laptop"): the card's second line. `None` when it would
    /// only repeat the card's name.
    pub sub: Option<String>,
    /// Who can reach it ("Only you", "Closed to the room"): the chip on the
    /// card.
    pub chip: String,
    /// Is it open to the room? Its chip turns oxblood, and closing it and its
    /// QR code come out from under Options onto the card.
    pub open: bool,
}

/// The buttons for a page whose own server is `here` (`None` for a page
/// somewhere else): every other server this computer runs, each saying
/// whether it's open to the room (`open` lists those that are) and, where
/// others can reach it, its address (`addresses`, the address each was
/// started on). Elsewhere, the labels say "back to".
pub fn places(saved: &Saved, here: Option<Kind>, open: &[Kind], addresses: &[(Kind, String)]) -> Vec<Place> {
    saved
        .running()
        .into_iter()
        .filter(|k| Some(*k) != here)
        .map(|k| Place {
            label: match (here, k) {
                (Some(_), _) => saved.shown_name(k),
                (None, Kind::Node) => "\u{2190} Back to your node".to_string(),
                (None, _) => format!("\u{2190} Back to {}", saved.shown_name(k)),
            },
            url: match (k, here) {
                (Kind::Node, _) => format!("{}/", local_url(saved.port_of(k))),
                (_, Some(_)) => format!("{SCHEME}://open/{}", k.key()),
                // From a page elsewhere the app ignores hic-app links, so go
                // back to the commons' directory.
                (_, None) => format!("{}/", local_url(saved.port_of(k))),
            },
            tone: match k {
                Kind::Node => "#0a7a54",
                Kind::Commons => "#8b3fd9",
                Kind::Practice => "#1f5cff",
            },
            kind: k.key(),
            sub: card_sub(saved, k),
            chip: crate::room::chip_text(saved, k, open.contains(&k)).to_string(),
            open: open.contains(&k),
            address: addresses.iter().find(|(kind, _)| *kind == k).map(|(_, url)| without_scheme(url)),
        })
        .collect()
}

/// A card's second line: your node's own name under "Your node", or what
/// kind of server a commons is under its name. `None` when that would say
/// the name twice ("Local commons" / "Local commons"). Pure.
fn card_sub(saved: &Saved, kind: Kind) -> Option<String> {
    let sub = match kind {
        Kind::Node => saved.name_of(kind)?.to_string(),
        _ => crate::room::kind_text(saved, kind).to_string(),
    };
    let repeats_the_name = sub.trim().eq_ignore_ascii_case(saved.shown_name(kind).trim());
    (!sub.trim().is_empty() && !repeats_the_name).then_some(sub)
}

/// An address as people type it: `http://172.20.10.10:8180` gives
/// `172.20.10.10:8180`. Pure.
fn without_scheme(url: &str) -> String {
    url.strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url)
        .trim_end_matches('/')
        .to_string()
}

/// The script the app adds to a server page to show `places`. Pages served
/// by Held in Common have a "This computer" slot in their side rail
/// (`#hic-servers`): the script fills it with one card per other server and
/// shows it, puts each server's `actions` on its own card (the card you're
/// on is drawn by the page, and gets its actions here too), and ends with a
/// dashed card for each server this computer could add. Pages elsewhere have
/// no slot, so the script adds a row of buttons in the bottom-right corner
/// instead: the way back. Running it again replaces what it added before.
/// The labels and addresses go in as JSON and are set as text, so nothing in
/// them can break out.
pub fn switcher_js(places: &[Place], actions: &[crate::go_menu::SidebarItem]) -> String {
    let data = serde_json::to_string(places).unwrap_or_else(|_| "[]".into());
    // The Go menu's items for the "This computer" panel, as links the app
    // follows (`hic-app://menu/<id>`), each saying whose card it goes on.
    let acts: Vec<serde_json::Value> = actions
        .iter()
        .map(|a| {
            serde_json::json!({
                "label": a.label,
                "url": format!("{SCHEME}://menu/{}", a.id),
                "server": a.server,
                "slot": a.slot,
            })
        })
        .collect();
    let acts = serde_json::to_string(&acts).unwrap_or_else(|_| "[]".into());
    format!(
        r#"(function(){{
  var id='hic-switch', places={data}, actions={acts};
  var old=document.getElementById(id); if(old){{old.remove();}}
  var slot=document.getElementById('hic-servers');
  if(slot){{
    // An element with a class and text: always set as text, never as markup.
    function el(tag,cls,text){{var e=document.createElement(tag); if(cls){{e.className=cls;}} if(text){{e.textContent=text;}} return e;}}
    // A server's actions on its card: open-to-the-room ones on the card
    // itself, the rest in a list under its Options button.
    function addActions(box,kind){{
      var mine=actions.filter(function(x){{return x.server===kind&&x.slot!=='add';}});
      // The QR code comes first, as the main button: it's what the room needs.
      var onCard=mine.filter(function(x){{return x.slot==='card';}}).sort(function(a,b){{return (b.url.indexOf('room-show')>-1)-(a.url.indexOf('room-show')>-1);}});
      if(onCard.length){{
        var row=el('div','server-room');
        onCard.forEach(function(x,i){{var a=el('a',i===0?'btn-solid':'btn-line',x.label); a.href=x.url; row.appendChild(a);}});
        box.appendChild(row);
      }}
      var opts=mine.filter(function(x){{return x.slot==='option'||x.slot==='danger';}});
      var foot=box.querySelector('.server-foot');
      if(!opts.length||!foot){{return;}}
      var listId='hic-options-'+kind;
      var b=el('button','server-options-toggle','Options'); b.type='button';
      b.setAttribute('aria-expanded','false'); b.setAttribute('aria-controls',listId);
      var ul=el('ul','server-options'); ul.id=listId; ul.hidden=true;
      opts.forEach(function(x){{var li=el('li',x.slot==='danger'?'is-danger':''); var a=el('a','',x.label); a.href=x.url; li.appendChild(a); ul.appendChild(li);}});
      b.addEventListener('click',function(){{var show=ul.hidden; ul.hidden=!show; b.setAttribute('aria-expanded',show?'true':'false');}});
      foot.appendChild(b); box.appendChild(ul);
    }}
    while(slot.firstChild){{slot.removeChild(slot.firstChild);}}
    // The card you're on: drawn by the page, given its actions here (after
    // clearing any from an earlier run).
    var here=document.querySelector('.server.is-here');
    if(here){{
      here.querySelectorAll('.server-room,.server-options,.server-options-toggle').forEach(function(n){{n.remove();}});
      addActions(here,here.getAttribute('data-kind'));
    }}
    places.forEach(function(p){{
      var box=el('div','server'); box.setAttribute('data-kind',p.kind);
      var a=el('a','server-card'); a.href=p.url;
      var star=el('span','star'); star.setAttribute('aria-hidden','true');
      var text=el('span','server-text'); text.appendChild(el('span','server-name',p.label));
      if(p.sub){{text.appendChild(el('span','server-sub',p.sub));}}
      a.appendChild(star); a.appendChild(text); box.appendChild(a);
      var foot=el('div','server-foot'); foot.appendChild(el('span','server-chip'+(p.open?' is-open':''),p.chip)); box.appendChild(foot);
      // The address sits outside the link, so it can be selected and copied.
      if(p.address){{var ad=el('p','server-address'); ad.appendChild(el('code','',p.address)); box.appendChild(ad);}}
      addActions(box,p.kind);
      slot.appendChild(box);
    }});
    // Servers this computer could add: dashed cards at the end.
    actions.filter(function(x){{return x.slot==='add';}}).forEach(function(x){{
      var a=el('a','server-add'); a.href=x.url; if(x.server){{a.setAttribute('data-kind',x.server);}}
      var mark=el('span','server-add-mark','+'); mark.setAttribute('aria-hidden','true');
      a.appendChild(mark); a.appendChild(el('span','',x.label)); slot.appendChild(a);
    }});
    slot.hidden=!places.length&&!actions.length;
    var label=document.getElementById('hic-servers-label'); if(label){{label.hidden=false;}}
    return;
  }}
  if(!places.length||!document.body){{return;}}
  var bar=document.createElement('nav'); bar.id=id; bar.setAttribute('aria-label','Your servers on this computer');
  bar.style.cssText='position:fixed;right:12px;bottom:12px;z-index:2147483647;display:flex;flex-wrap:wrap;justify-content:flex-end;gap:6px;max-width:calc(100vw - 24px);';
  places.forEach(function(p){{
    var a=document.createElement('a'); a.href=p.url; a.textContent=p.label;
    a.style.cssText='padding:9px 12px;font:700 11px/1 ui-monospace,Menlo,monospace;letter-spacing:.07em;text-transform:uppercase;text-decoration:none;color:#fff;box-shadow:0 2px 10px rgba(0,0,0,.3);';
    a.style.background=p.tone;
    bar.appendChild(a);
  }});
  document.body.appendChild(bar);
}})();"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a computer with a node called "Laptop" runs, plus a commons and practice if asked.
    fn saved(other: Option<&str>, practice: bool) -> Saved {
        Saved {
            role: Some("node".into()),
            name: Some("Laptop".into()),
            other_name: other.map(String::from),
            practice,
            practice_name: None,
            public_address: None,
        }
    }

    #[test]
    fn a_commons_card_carries_its_address_without_the_scheme() {
        let s = saved(Some("Room 4"), true);
        let addresses = vec![(Kind::Commons, "http://172.20.10.10:8181".to_string())];
        let from_node = places(&s, Some(Kind::Node), &[], &addresses);
        let commons = from_node.iter().find(|p| p.kind == "commons").unwrap();
        assert_eq!(commons.address.as_deref(), Some("172.20.10.10:8181"));
        assert!(from_node.iter().find(|p| p.kind == "practice").unwrap().address.is_none());
    }

    #[test]
    fn app_links_are_read() {
        assert_eq!(parse("hic-app://open/practice"), Some(AppLink::Open(Kind::Practice)));
        assert_eq!(parse("hic-app://open/commons/"), Some(AppLink::Open(Kind::Commons)));
        assert_eq!(parse("hic-app://open/node"), Some(AppLink::Open(Kind::Node)));
        assert_eq!(parse("hic-app://open/elsewhere"), None);
        assert_eq!(parse("hic-app://remove/practice"), None, "pages can't remove anything");
        assert_eq!(parse("hic-app://menu/room-close:node"), Some(AppLink::Menu("room-close:node".into())));
        assert_eq!(
            parse("hic-app://menu/remove-practice"),
            Some(AppLink::Menu("remove-practice".into())),
            "only opens the confirmation in the setup window"
        );
        assert_eq!(parse("hic-app://menu/about-project"), None, "only the Go menu's actions");
        assert_eq!(parse("hic-app://menu/rm"), None);
        assert_eq!(parse("https://open/node"), None);
    }

    #[test]
    fn the_switcher_lists_the_other_servers() {
        let s = saved(Some("Room 4"), true);
        let here_node: Vec<String> = places(&s, Some(Kind::Node), &[], &[]).into_iter().map(|p| p.url).collect();
        assert_eq!(here_node, vec!["hic-app://open/commons", "hic-app://open/practice"]);
        assert!(here_node.iter().all(|u| !u.contains("launch")), "the token never goes into a page");
        let labels: Vec<String> = places(&s, Some(Kind::Practice), &[], &[]).into_iter().map(|p| p.label).collect();
        assert_eq!(labels, vec!["Your node", "Room 4"], "a commons is shown by its own name, never \"your commons\"");
    }

    #[test]
    fn each_rail_card_says_what_it_is_and_where() {
        let s = saved(Some("Room 4"), false);
        let from_node = places(&s, Some(Kind::Node), &[], &[]);
        assert_eq!(from_node.len(), 1);
        assert_eq!(from_node[0].label, "Room 4");
        assert_eq!(from_node[0].kind, "commons");
        assert_eq!(from_node[0].sub.as_deref(), Some("Commons on this Wi-Fi"));
        assert_eq!(from_node[0].chip, "Anyone on this Wi-Fi");
    }

    #[test]
    fn a_card_never_says_its_name_twice() {
        // An unnamed local commons is called "Local commons", so its kind
        // isn't repeated underneath.
        let s = saved(None, true);
        let practice = places(&s, Some(Kind::Node), &[], &[]).into_iter().find(|p| p.kind == "practice").unwrap();
        assert_eq!((practice.label.as_str(), practice.sub), ("Local commons", None));
        assert_eq!(practice.chip, "Closed to the room");
        // Your node shows its own name under "Your node".
        let node = places(&s, Some(Kind::Practice), &[], &[]).into_iter().find(|p| p.kind == "node").unwrap();
        assert_eq!((node.label.as_str(), node.sub.as_deref()), ("Your node", Some("Laptop")));
    }

    #[test]
    fn an_open_server_says_so_on_its_chip() {
        let s = saved(None, true);
        let practice = places(&s, Some(Kind::Node), &[Kind::Practice], &[]).into_iter().next().unwrap();
        assert!(practice.open);
        assert_eq!(practice.chip, "Open to the room");
    }

    #[test]
    fn a_node_on_its_own_has_no_switcher_but_elsewhere_has_a_way_back() {
        let s = saved(None, false);
        assert!(places(&s, Some(Kind::Node), &[], &[]).is_empty());
        let away = places(&s, None, &[], &[]);
        assert_eq!(away.len(), 1);
        assert_eq!(away[0].label, "\u{2190} Back to your node");
        assert_eq!(away[0].url, "http://127.0.0.1:8080/");
    }

    #[test]
    fn servers_are_found_by_port() {
        let s = saved(Some("Room 4"), true);
        assert_eq!(kind_on(&s, 8081), Some(Kind::Commons));
        assert_eq!(kind_on(&s, 8082), Some(Kind::Practice));
        assert_eq!(kind_on(&saved(None, false), 8082), None, "no local commons yet");
    }

    #[test]
    fn only_the_project_site_opens_in_the_browser() {
        assert!(opens_in_browser(Some("heldincommon.org")));
        assert!(opens_in_browser(Some("www.heldincommon.org")));
        assert!(!opens_in_browser(Some("notheldincommon.org")));
        assert!(!opens_in_browser(Some("heldincommon.org.evil.example")));
        assert!(!opens_in_browser(Some("127.0.0.1")));
        assert!(!opens_in_browser(None));
    }

    #[test]
    fn labels_cannot_break_out_of_the_script() {
        let evil = "'); alert(1); ('";
        let js = switcher_js(
            &[Place {
                label: evil.into(),
                url: "x".into(),
                tone: "#000",
                kind: "node",
                sub: Some(evil.into()),
                chip: evil.into(),
                open: false,
                address: Some(evil.into()),
            }],
            &[crate::go_menu::SidebarItem {
                id: "rename:node".into(),
                label: evil.into(),
                server: Some("node"),
                slot: crate::go_menu::Slot::Option,
            }],
        );
        assert!(js.contains(r#""label":"'); alert(1); ('""#), "inside a JSON string");
        assert!(js.contains(r#""sub":"'); alert(1); ('""#), "the second line too");
        assert!(js.contains(r#""chip":"'); alert(1); ('""#), "and the chip");
        assert!(js.contains(r#""address":"'); alert(1); ('""#), "and the address");
        // serde_json sorts an object's keys, so the menu item reads label,
        // server, slot, url.
        assert!(
            js.contains(
                r#""label":"'); alert(1); ('","server":"node","slot":"option","url":"hic-app://menu/rename:node""#
            ),
            "and the menu items"
        );
        assert!(!js.contains("innerHTML"), "names are set as text, never as markup");
        assert!(switcher_js(&[], &[]).contains("places=[]"));
    }
}
