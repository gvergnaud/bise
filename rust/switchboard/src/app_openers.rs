//! The apps that open the artifacts (docs/plugin-apps.md A2, A3): the hub
//! keeps every on plugin's openers ([`AppOpeners`], one field of the
//! shell) and puts on each artifact row the app `bend_plugins::opens::pick`
//! answers, so the window never matches a file itself.
//!
//! - Built off the loop (it reads the plugins and plugins.json) at boot and
//!   when the plugins change ([`AppOpeners::new`]); a server's openers come
//!   from its tools: the tools cache's cold list at first
//!   (`bend_plugins::bridge::cold_tools`), then each live `tools/list` the
//!   hub's views code gets ([`AppOpeners::seen`], architect m_61469).
//!   Nothing is persisted here: lost on a restart, built again.
//! - A row's target ([`target_of`]) is read from the row alone, no disk:
//!   its file's name, a page as `<id>.page` (A9), a site folder as a
//!   folder holding an `index.html`; a link has no app.
//! - `view/open` gathers again at the open: a row a stale list drew never
//!   decides (architect m_60316 #4).

use bend_plugins::opens::{self, Kind, Opened, Openers, ServerTools, Target};
use bend_plugins::resolve::Resolution;
use bend_plugins::state::Picks;
use serde_json::Value;

/// Every on plugin's openers and his picks, as the rows read them.
#[derive(Debug, Default)]
pub struct AppOpeners {
    res: Resolution,
    tools: ServerTools,
    /// the servers whose entry in `tools` is a live list (it wins over
    /// the cache's cold one at the next refresh)
    live: std::collections::BTreeSet<(String, String)>,
    openers: Openers,
    picks: Picks,
}

impl AppOpeners {
    /// From a resolution, the servers' tools known now (the cold lists)
    /// and his picks.
    pub fn new(res: Resolution, tools: ServerTools, picks: Picks) -> AppOpeners {
        let openers = Openers::gather(&res, &tools).0;
        AppOpeners { res, tools, live: Default::default(), openers, picks }
    }

    /// The plugins read again (a change, or the periodic look): the cold
    /// lists `cold` under the live ones kept. True when the rows' apps or
    /// his picks changed.
    pub fn refresh(&mut self, res: Resolution, cold: ServerTools, picks: Picks) -> bool {
        let mut tools = cold;
        for k in &self.live {
            if let Some(t) = self.tools.get(k) {
                tools.insert(k.clone(), t.clone());
            }
        }
        let openers = Openers::gather(&res, &tools).0;
        let changed = openers != self.openers || picks != self.picks;
        *self = AppOpeners { res, tools, live: std::mem::take(&mut self.live), openers, picks };
        changed
    }

    /// A live `tools/list` of `server` of `plugin`: kept, the openers
    /// gathered again. True when that changed them.
    pub fn seen(&mut self, plugin: &str, server: &str, tools: Vec<Value>) -> bool {
        let key = (plugin.to_string(), server.to_string());
        self.live.insert(key.clone());
        if self.tools.get(&key) == Some(&tools) {
            return false;
        }
        self.tools.insert(key, tools);
        let openers = Openers::gather(&self.res, &self.tools).0;
        let changed = openers != self.openers;
        self.openers = openers;
        changed
    }

    /// The app that opens an art store's row, and the others.
    pub fn opened(&self, row: &Value) -> Opened {
        target_of(row).map(|t| opens::pick(&t, &self.openers, &self.picks)).unwrap_or_default()
    }

    /// The openers, for `view/open`'s check that his pick is a candidate.
    pub fn openers(&self) -> &Openers {
        &self.openers
    }

    pub fn picks(&self) -> &Picks {
        &self.picks
    }

    /// The servers' tools known now (`view/open` gathers again over them).
    pub fn tools(&self) -> &ServerTools {
        &self.tools
    }

    /// The documents folders the openers gathered now use (A1): the hub
    /// makes them, off the loop, after each gather that changed them.
    pub fn documents(&self) -> Vec<std::path::PathBuf> {
        self.openers.documents_to_make(&self.res)
    }

    /// The plugins' app icons now (`/icon/<plugin>`, pages/icon.rs).
    pub fn icons(&self) -> crate::pages::icon::Icons {
        crate::pages::icon::icons_of(&self.openers, &self.res)
    }
}

/// Make the plugins' documents folders (`dirs`, [`AppOpeners::documents`]);
/// disk work: call it off the loop. A folder that fails stays unmade (its
/// opener's first write says so).
pub fn make_documents(dirs: &[std::path::PathBuf]) {
    for d in dirs {
        if let Err(e) = std::fs::create_dir_all(d) {
            eprintln!("apps: documents folder not made ({}): {e}", d.display());
        }
    }
}

/// What an art store row opens as (A2's target), from the row alone; a
/// link: none.
pub fn target_of(row: &Value) -> Option<Target> {
    let s = |k: &str| row.get(k).and_then(Value::as_str).unwrap_or("");
    if s("by") == "page" || s("kind") == "page" {
        return Some(Target::file(&format!("{}.page", s("id"))));
    }
    let target = s("target");
    if target.is_empty() || crate::artifacts::is_link(target) {
        return None;
    }
    let name = std::path::Path::new(target.trim_end_matches('/')).file_name()?.to_string_lossy().to_string();
    // a site artifact is a folder that holds an index.html (artifacts.rs
    // names it `site` by that); its other names are not read here
    if s("kind") == "site" {
        return Some(Target { name, kind: Kind::Folder(vec!["index.html".into()]), documents: None });
    }
    Some(Target::file(&name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_row_opens_as_its_file_a_page_or_a_site_never_a_link() {
        assert_eq!(target_of(&json!({"target": "/w/q3/plan.xlsx", "kind": "file"})), Some(Target::file("plan.xlsx")));
        assert_eq!(target_of(&json!({"id": "perf-notes", "by": "page", "target": "/st/pages/perf-notes/v2.html"})), Some(Target::file("perf-notes.page")));
        assert_eq!(
            target_of(&json!({"target": "/w/site/", "kind": "site"})),
            Some(Target { name: "site".into(), kind: Kind::Folder(vec!["index.html".into()]), documents: None })
        );
        assert_eq!(target_of(&json!({"target": "https://github.com/a/b/pull/1", "kind": "pr"})), None);
        assert_eq!(target_of(&json!({"target": ""})), None);
        assert_eq!(target_of(&json!({})), None);
    }

    /// No plugin: bise's own types still open on today's path (BUILTIN);
    /// an unknown ending opens on his Mac (no opener).
    #[test]
    fn the_builtin_types_open_with_no_plugin() {
        let a = AppOpeners::default();
        let o = a.opened(&json!({"target": "/w/plan.xlsx", "kind": "file"}));
        assert!(o.opener.as_ref().is_some_and(|o| o.builtin && o.type_key == "*.xlsx"), "{o:?}");
        assert_eq!(a.opened(&json!({"target": "/w/a.zip", "kind": "file"})), Opened::default());
        assert_eq!(a.opened(&json!({"target": "https://x.dev", "kind": "link"})), Opened::default());
    }

    #[test]
    fn a_live_list_counts_only_when_it_changes() {
        let mut a = AppOpeners::default();
        assert!(!a.seen("fake", "srv", vec![]), "no plugin loaded: no opener either way");
        assert!(!a.seen("fake", "srv", vec![]), "the same list again");
    }
}
