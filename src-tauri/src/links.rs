//! How pages move between this computer's servers. Pure: no window, no
//! state.
//!
//! - **`hic-app://open/<kind>` links.** A page served by one of the app's
//!   servers (the node's "Run your own commons" guide, a commons' steward
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
}

/// Read a `hic-app://` link. Anything else, or a link this app doesn't
/// know, is `None`.
pub fn parse(url: &str) -> Option<AppLink> {
    let rest = url.strip_prefix(SCHEME)?.strip_prefix("://")?;
    let rest = rest.trim_end_matches('/');
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
    /// The colour of the role bar on that server's pages.
    pub tone: &'static str,
}

/// The buttons for a page whose own server is `here` (`None` for a page
/// somewhere else): every other server this computer runs. Elsewhere, the
/// labels say "back to".
pub fn places(saved: &Saved, here: Option<Kind>) -> Vec<Place> {
    saved
        .running()
        .into_iter()
        .filter(|k| Some(*k) != here)
        .map(|k| Place {
            label: if here.is_some() {
                k.label().to_string()
            } else {
                format!("\u{2190} Back to {}", k.label().to_lowercase())
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
        })
        .collect()
}

/// The script the app adds to a server page to show `places` in the
/// bottom-right corner (or to remove an earlier row, when there are none).
/// The labels and addresses go in as JSON, so nothing in them can break out.
pub fn switcher_js(places: &[Place]) -> String {
    let data = serde_json::to_string(places).unwrap_or_else(|_| "[]".into());
    format!(
        r#"(function(){{
  var id='hic-switch', places={data};
  var old=document.getElementById(id); if(old){{old.remove();}}
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
            public_address: None,
        }
    }

    #[test]
    fn app_links_are_read() {
        assert_eq!(parse("hic-app://open/practice"), Some(AppLink::Open(Kind::Practice)));
        assert_eq!(parse("hic-app://open/commons/"), Some(AppLink::Open(Kind::Commons)));
        assert_eq!(parse("hic-app://open/node"), Some(AppLink::Open(Kind::Node)));
        assert_eq!(parse("hic-app://open/elsewhere"), None);
        assert_eq!(parse("hic-app://remove/practice"), None, "pages can't remove anything");
        assert_eq!(parse("https://open/node"), None);
    }

    #[test]
    fn the_switcher_lists_the_other_servers() {
        let s = saved(Some("Room 4"), true);
        let here_node: Vec<String> = places(&s, Some(Kind::Node)).into_iter().map(|p| p.url).collect();
        assert_eq!(here_node, vec!["hic-app://open/commons", "hic-app://open/practice"]);
        assert!(here_node.iter().all(|u| !u.contains("launch")), "the token never goes into a page");
        let labels: Vec<String> = places(&s, Some(Kind::Practice)).into_iter().map(|p| p.label).collect();
        assert_eq!(labels, vec!["Your node", "Your commons"]);
    }

    #[test]
    fn a_node_on_its_own_has_no_switcher_but_elsewhere_has_a_way_back() {
        let s = saved(None, false);
        assert!(places(&s, Some(Kind::Node)).is_empty());
        let away = places(&s, None);
        assert_eq!(away.len(), 1);
        assert_eq!(away[0].label, "\u{2190} Back to your node");
        assert_eq!(away[0].url, "http://127.0.0.1:8080/");
    }

    #[test]
    fn servers_are_found_by_port() {
        let s = saved(Some("Room 4"), true);
        assert_eq!(kind_on(&s, 8081), Some(Kind::Commons));
        assert_eq!(kind_on(&s, 8082), Some(Kind::Practice));
        assert_eq!(kind_on(&saved(None, false), 8082), None, "no practice commons yet");
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
        let js = switcher_js(&[Place { label: "'); alert(1); ('".into(), url: "x".into(), tone: "#000" }]);
        assert!(js.contains(r#""label":"'); alert(1); ('""#), "inside a JSON string");
        assert!(switcher_js(&[]).contains("places=[]"));
    }
}
