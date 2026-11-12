//! bend-plugins: Agent Plugins 1.0 (the portable base) for the Bend
//! harness. Design: docs/plugins.md.
//!
//! - `id`: [`id::PluginId`], the one typed id of a plugin or a part;
//! - `model`: the plain data the modules share (connectors, tools,
//!   skills, the on/off file as read);
//! - `resolve`: discovery, manifest validation, precedence, components,
//!   diagnostics (pure over the file system, no process started);
//! - `skills`: the one scanner of the loose skill folders (roots, scan,
//!   on/off, the session's skills index, the hub's fingerprint);
//! - `state`: the on/off file (`plugins.json`), its view grants and his
//!   picks of the app that opens a type;
//! - `opens`: plugin apps, the files a plugin's openers open
//!   (docs/plugin-apps.md A1, A2);
//! - `connectors`: his Mistral connectors' catalog, its cache and index
//!   lines;
//! - `catalog`: the one builder of the unified Plugins list (the wire
//!   rows, `bise_proto::app_rows::Plugin`);
//! - `report`: the human and JSON listings;
//! - `stdio`: a stdio MCP client (one child process, JSON-RPC lines);
//! - `http`, `remote`: a remote MCP client (Streamable HTTP, SSE);
//! - `oauth`: the browser login and the tokens of remote servers;
//! - `login`: `/plugins login`, `bise plugins login|logout`;
//! - `status`: each remote server's last state, for `/plugins`;
//! - `bridge`: the per-session loopback HTTP bridge the Bend REPL calls;
//! - `cli`: the `bise plugins ...` subcommand;
//! - `import`: `bise plugins import-mcp`, another agent's MCP servers as
//!   one plugin (BISE-273);
//! - `add`: the add menu (an MCP server, an import found on disk, a
//!   folder, a git URL): staged, previewed, installed on his click;
//! - `remove`: a plugin he added removed, with its secrets, logins,
//!   statuses and on/off ids;
//! - `marketplace`: his plugin marketplaces and what they offer;
//! - `signals`: what his projects name (remotes, file names, env example
//!   keys; names only), the evidence the 'connect your tools' band ranks
//!   and says why by;
//! - `secrets`: the values of an MCP server's `${VAR}` he typed in;
//! - `views`: MCP Apps on the bridge's side: the UI extension, a tool's
//!   `_meta.ui`, the Vibe static views and their virtual server, the
//!   `ui://` resource cache (docs/mcp-apps.md);
//! - `visibility`: whether a view's call may run (its own server's
//!   app-visible tools only; docs/mcp-apps.md, risk 1).

// the tests run on a temp HOME, never the user's (bise_home::test_home)
bise_home::test_home!();

/// A real plugin's documents, `~/bise/Plugins/<name>/` (docs/plugin-apps.md
/// A6), given to its processes as `PLUGIN_DOCUMENTS`; its owner's for a
/// part. None for what is not a folder's plugin (a connector, a skills
/// folder) or a name that is not one safe path segment. With
/// [`documents_dir_in`], the one user of `bise_home::plugin_app_dir_in`
/// (a test holds it so).
pub fn documents_dir(home: &bise_home::Home, id: &id::PluginId) -> Option<std::path::PathBuf> {
    documents_dir_in(&home.plugin_documents_root(), id)
}

/// [`documents_dir`] under `root` (`resolve::Roots::documents`, which
/// `Roots::standard` fills from his Home). Pure.
pub fn documents_dir_in(root: &std::path::Path, id: &id::PluginId) -> Option<std::path::PathBuf> {
    match id.owner() {
        id::PluginId::Plugin { name } => bise_home::plugin_app_dir_in(root, name),
        _ => None,
    }
}

#[cfg(test)]
mod documents_tests {
    use super::*;

    #[test]
    fn a_plugins_documents_are_its_folder_under_the_bise_root() {
        let home = bise_home::Home::at(std::env::temp_dir().join("docs-dir"));
        let sheets = id::PluginId::plugin("sheets");
        assert_eq!(documents_dir(&home, &sheets), Some(home.bise_root().join("Plugins").join("sheets")));
        let part = id::PluginId::parse("sheets#tool:sheets/open").unwrap();
        assert_eq!(documents_dir(&home, &part), documents_dir(&home, &sheets), "a part's are its owner's");
        assert_eq!(documents_dir(&home, &id::PluginId::parse("mistral:gmail").unwrap()), None);
        assert_eq!(documents_dir(&home, &id::PluginId::Plugin { name: "../x".into() }), None);
        assert_eq!(documents_dir(&home, &id::PluginId::Plugin { name: ".hidden".into() }), None);
    }

    /// The Rust sources that spell `needle`, relative to rust/.
    fn sources_with(needle: &str) -> Vec<String> {
        let rust = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut hits = Vec::new();
        let mut stack = vec![rust.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if p.is_dir() && name != "target" && !name.starts_with('.') {
                    stack.push(p);
                } else if name.ends_with(".rs") && std::fs::read_to_string(&p).unwrap_or_default().contains(needle) {
                    hits.push(p.strip_prefix(&rust).unwrap_or(&p).display().to_string());
                }
            }
        }
        hits.sort();
        hits
    }

    /// A plugin's documents folder is named in one place each side: the
    /// rule in bise_home, its one user here, which passes a plugin's own
    /// name (architect m_60008).
    #[test]
    fn the_documents_rule_is_called_from_here_only() {
        assert_eq!(sources_with(&["plugin_app_dir_in", "("].concat()), ["home/src/lib.rs", "plugins/src/lib.rs"]);
    }
}

pub mod add;
pub mod bridge;
pub mod catalog;
pub mod cli;
pub mod connectors;
pub mod http;
pub mod id;
pub mod import;
pub mod login;
pub mod marketplace;
pub mod model;
pub mod oauth;
pub mod opens;
pub mod parent;
pub mod remote;
pub mod remove;
pub mod report;
pub mod resolve;
pub mod secrets;
pub mod signals;
pub mod skills;
pub mod state;
pub mod status;
pub mod stdio;
pub mod title;
pub mod tools_cache;
pub mod views;
pub mod visibility;
