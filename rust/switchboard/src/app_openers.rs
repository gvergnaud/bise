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
//!   its file's name, a page as `<id>.page` (A9), a folder as its
//!   first-level names (the version's, artifacts::Version::names); a link
//!   has no app.
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

    /// What `sb artifact list --app` reads of each row: this hub's
    /// openers and picks now, a copy a thread can own (another project's
    /// store is listed off the loop with them).
    pub fn app_keys(&self) -> AppKeys {
        AppKeys { openers: self.openers.clone(), picks: self.picks.clone() }
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

/// The openers and picks at one time ([`AppOpeners::app_keys`]): each
/// store row's app key and word for `sb artifact list --app`, the rule
/// the window reads on the row's opener (artifacts_filter::app_of).
#[derive(Clone, Debug, Default)]
pub struct AppKeys {
    openers: Openers,
    picks: Picks,
}

impl AppKeys {
    /// A store row's app: its key (the opener's plugin, else `prs`,
    /// `links`, `mac`) and its word.
    pub fn of(&self, m: &crate::artifacts::Meta) -> (String, String) {
        let cur = m.current().cloned().unwrap_or_default();
        let row = serde_json::json!({
            "id": m.id, "kind": m.kind, "by": m.by, "target": cur.target,
            "names": cur.names.names, "names_more": cur.names.more,
        });
        let opener = target_of(&row).and_then(|t| opens::pick(&t, &self.openers, &self.picks).opener);
        let pr = crate::forge::pr_link(&cur.target).is_some();
        crate::artifacts_filter::app_of(pr, opener.as_ref().map(|o| (o.plugin.as_str(), o.app.as_str())), crate::artifacts::is_link(&cur.target))
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
    // a folder: its first-level names as the version kept them
    // (artifacts::Version::names, architect m_62393); a cut list says so
    let names: Vec<String> = row.get("names").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
    let more = row.get("names_more").and_then(Value::as_bool).unwrap_or(false);
    match (names.is_empty(), more) {
        (true, false) => Some(Target::file(&name)),
        (_, false) => Some(Target { name, kind: Kind::Folder(names), documents: None }),
        (_, true) => Some(Target { name, kind: Kind::FolderCut(names), documents: None }),
    }
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
            target_of(&json!({"target": "/w/site/", "kind": "site", "names": ["app.js", "index.html"]})),
            Some(Target { name: "site".into(), kind: Kind::Folder(vec!["app.js".into(), "index.html".into()]), documents: None })
        );
        // the kind is never read: a plain folder holding an index.html is the same target
        assert_eq!(target_of(&json!({"target": "/w/site/", "kind": "folder", "names": ["index.html"]})), target_of(&json!({"target": "/w/site/", "kind": "site", "names": ["index.html"]})));
        assert_eq!(
            target_of(&json!({"target": "/w/big/", "kind": "site", "names": ["a.css"], "names_more": true})),
            Some(Target { name: "big".into(), kind: Kind::FolderCut(vec!["a.css".into()]), documents: None })
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
        let o = a.opened(&json!({"target": "/w/plan.page", "kind": "file"}));
        assert!(o.opener.as_ref().is_some_and(|o| o.builtin && o.type_key == "*.page"), "{o:?}");
        // an xlsx is Sheets' (plugins/sheets, step 4): no built-in row left
        assert_eq!(a.opened(&json!({"target": "/w/plan.xlsx", "kind": "file"})), Opened::default());
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
