//! Plugin apps (docs/plugin-apps.md A1, A2): the files a plugin's openers
//! open, and the one rule that picks the app a file opens in. Pure.
//!
//! - [`parse`]: one opener from a tool's `_meta` as `tools/list` lists it
//!   (a running server's, or a static view's: `views::Virtual::tools`
//!   lists its `view.json` `_meta` the same way), with a
//!   `plugin.opener.invalid` warning for each entry dropped;
//! - [`type_key`]: the one canonical form of a type, the key of his picks
//!   (`plugins.json` `openers`, [`crate::state::picks`]): Vibe's
//!   extension `.XLSX` and the glob `*.xlsx` are both `*.xlsx`;
//! - [`fold`]: the one case rule, applied to a pattern ([`type_key`]) and
//!   to a file's name and path ([`pick`]): APFS is case-insensitive;
//! - [`pick`]: which app opens a target, and the others (Open with);
//! - [`BUILTIN`]: bise's own types not migrated yet, candidates like any
//!   opener (`builtin: true` on the row), each row deleted by its
//!   migration step with its `viewer_kind` arm (a test in
//!   switchboard's pages/site.rs pins them together).

use bise_proto::rows::{OpenRank, Opener, PluginMake, PluginOpens};
use serde_json::Value;

use crate::resolve::{Diagnostic, Severity};
use crate::state::{OpenerRef, Picks};

/// The longest type pattern kept (a glob or an extension).
pub const MAX_PATTERN: usize = 128;
/// At most this many openers a plugin (more: a warning, the rest dropped).
pub const MAX_OPENERS: usize = 16;
/// At most this many entries an opener (`files` plus Vibe's extensions).
pub const MAX_ENTRIES: usize = 32;
/// At most this many globs an entry's `match`.
pub const MAX_GLOBS: usize = 16;
/// The longest title kept (a type's, an app's, a make's).
pub const MAX_TITLE: usize = 40;
/// Vibe's `_meta` key (Plugins v2 RFC §6: file entrypoints).
pub const VIBE_UI: &str = "ai.mistral.vibe/ui";
/// bise's own `_meta` key, which Vibe ignores.
pub const BISE_UI: &str = "dev.bise/ui";
/// A pattern's scope: relative to its plugin's documents.
pub const DOCUMENTS: &str = "@documents/";

/// One type an opener opens (A1's `dev.bise/ui.files[]`, or one of
/// Vibe's extensions no `files` entry names).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// its patterns, canonical ([`type_key`]), 1 to [`MAX_GLOBS`]
    pub keys: Vec<String>,
    /// the type's name (`Excel workbook`)
    pub title: String,
    pub rank: OpenRank,
    /// its patterns name a folder by what it holds (`*/index.html`)
    pub folder: bool,
}

/// One kind of file an opener makes (A1's `makes[]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Make {
    /// `.xlsx`, lowercase
    pub ending: String,
    /// the New › row's word (`Sheet`)
    pub title: String,
    /// a file inside the plugin's root (`./templates/empty.xlsx`),
    /// checked lexically here and by realpath where it is copied
    pub template: Option<String>,
}

/// An opener: a tool with a view and the files it opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenerDecl {
    /// the server its tool lives on, in its plugin: a stdio or remote
    /// server's id, or [`crate::views::SERVER`] for a static view (where
    /// view/open routes its `tools/call` and reads its `ui://`)
    pub server: String,
    /// its tool's name
    pub tool: String,
    /// its view (`ui://…`)
    pub uri: String,
    /// the app's title (`Sheets`): its first file entrypoint's, else the
    /// plugin's name
    pub app: String,
    pub entries: Vec<Entry>,
    pub makes: Vec<Make>,
    /// its view may save (A1's `write`: a grant he gives at the add)
    pub write: bool,
    /// its view's origin (A5's `isolate`)
    pub isolate: Isolate,
    /// its app's icon (A1's `icon`): `./…` a `.svg`, `.png` or `.webp`
    /// inside the plugin, checked lexically here; the hub's reader at
    /// [`ICON_ROUTE`] checks its realpath and size
    pub icon: Option<String>,
    /// its view may be drawn in `displayMode: "thumbnail"` (A6b): a
    /// static view always, a server's only with `thumbnail: true`
    pub thumbnail: bool,
}

/// Where the hub serves a plugin's icon: `/icon/<plugin>`, an
/// [`Opener`]'s `icon` for a plugin that declares one (a built-in's is a
/// design-system icon name, `builtin: true`).
pub const ICON_ROUTE: &str = "/icon/";

/// The icon files a plugin may declare (lowercase endings).
const ICON_ENDINGS: [&str; 3] = [".svg", ".png", ".webp"];

/// A path inside the plugin, as A1 writes it (`./templates/empty.xlsx`):
/// `./` then segments, none empty, `.` or `..`, no `\`. Lexical only.
fn inside_plugin(path: &str) -> bool {
    path.strip_prefix("./").is_some_and(|rel| !rel.is_empty() && !rel.contains(['\\', '\0']) && !rel.split('/').any(|s| s.is_empty() || s == "." || s == ".."))
}

/// Which origin a document's view runs on (A5, `dev.bise/ui.isolate`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Isolate {
    /// one origin per view (the default): its documents share storage
    #[default]
    View,
    /// `"document"`: the origin's hash includes the target's id, so two
    /// documents never share storage
    Document,
}

fn invalid(plugin: &str, tool: &str, why: String) -> Diagnostic {
    Diagnostic { code: "plugin.opener.invalid", severity: Severity::Warning, plugin: plugin.to_string(), message: format!("opener {tool:?}: {why}") }
}

/// The one case rule: what a pattern and a name are compared in.
pub fn fold(s: &str) -> String {
    s.to_lowercase()
}

/// The canonical key of a type pattern ([`fold`]ed): an extension
/// (`.xlsx`, Vibe's `entrypoints[].extensions`) is the glob of its files
/// (`*.xlsx`); a glob (`*.board.json`, `*/index.html`,
/// `@documents/boards/*.board.json`) stays itself. None for a pattern no
/// entry may claim: empty, too long, a bare `*` or `**`, absolute, a
/// `..`, an empty segment, a backslash or a control character, an
/// `@documents` scope with nothing after it.
pub fn type_key(pattern: &str) -> Option<String> {
    let p = fold(pattern.trim());
    let p = match p.strip_prefix('.') {
        Some(ext) if !ext.is_empty() && !ext.contains(['/', '*', '?']) => format!("*.{ext}"),
        _ => p,
    };
    let body = p.strip_prefix(DOCUMENTS).unwrap_or(&p);
    let bad = p.is_empty()
        || p.len() > MAX_PATTERN
        || p.chars().any(|c| c.is_control() || c == '\\')
        || body.starts_with('/')
        || body.split('/').any(|s| s.is_empty() || s == "." || s == "..")
        || body.split('/').all(|s| s == "*" || s == "**");
    (!bad).then_some(p)
}

/// An ending (`.xlsx`, `.board.json`): a dot, then letters, digits,
/// dots, `-` and `_`; no wildcard. Vibe's extensions and A1's `makes`.
fn ending_ok(e: &str) -> bool {
    e.len() > 1 && e.len() <= MAX_PATTERN && e.starts_with('.') && e[1..].chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// A title: trimmed, non-empty, at most [`MAX_TITLE`] characters.
fn title_of(v: Option<&Value>, what: &str) -> Result<String, String> {
    let t = v.and_then(Value::as_str).map(str::trim).unwrap_or("");
    if t.is_empty() {
        return Err(format!("{what} needs a \"title\""));
    }
    if t.chars().count() > MAX_TITLE {
        return Err(format!("{what}'s title is over {MAX_TITLE} characters"));
    }
    Ok(t.to_string())
}

/// One `files[]` entry, checked: its globs canonical and shaped for
/// what it names (a name; a folder's `<folder>/<child>`; a path under
/// `@documents/`).
fn file_entry(i: usize, v: &Value) -> Result<Entry, String> {
    let at = format!("files[{i}]");
    let o = v.as_object().ok_or(format!("{at} must be an object"))?;
    let folder = match o.get("folder") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(format!("{at}.folder must be true or false")),
    };
    let rank = match o.get("rank") {
        None => OpenRank::Default,
        Some(r) => serde_json::from_value(r.clone()).map_err(|_| format!("{at}.rank must be owner, default or option"))?,
    };
    let globs = o.get("match").and_then(Value::as_array).ok_or(format!("{at}.match must be a list of globs"))?;
    if globs.is_empty() || globs.len() > MAX_GLOBS {
        return Err(format!("{at}.match must hold 1 to {MAX_GLOBS} globs"));
    }
    let mut keys = Vec::new();
    for g in globs {
        let raw = g.as_str().ok_or(format!("{at}.match holds a non-string"))?;
        let key = type_key(raw).ok_or(format!("{at}: {raw:?} is not a glob an app may claim"))?;
        let docs = key.starts_with(DOCUMENTS);
        let nested = key.contains('/') && !docs;
        if folder && (docs || !nested) {
            return Err(format!("{at}: a folder rule's {raw:?} must be <folder>/<what it holds>"));
        }
        if !folder && nested {
            return Err(format!("{at}: {raw:?} names a path: a file's glob is on its name (or under @documents/)"));
        }
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    Ok(Entry { keys, title: title_of(o.get("title"), &at)?, rank, folder })
}

fn make_of(i: usize, v: &Value) -> Result<Make, String> {
    let at = format!("makes[{i}]");
    let o = v.as_object().ok_or(format!("{at} must be an object"))?;
    let ending = o.get("ending").and_then(Value::as_str).map(fold).unwrap_or_default();
    if !ending_ok(&ending) {
        return Err(format!("{at}.ending must be like \".xlsx\""));
    }
    let template = match o.get("template") {
        None => None,
        Some(Value::String(t)) if !t.starts_with("./") => return Err(format!("{at}.template must start with ./ (a file in the plugin)")),
        Some(Value::String(t)) if !inside_plugin(t) => return Err(format!("{at}.template must stay inside the plugin")),
        Some(Value::String(t)) => Some(t.clone()),
        Some(_) => return Err(format!("{at}.template must be a string")),
    };
    Ok(Make { ending, title: title_of(o.get("title"), &at)?, template })
}

/// The opener a tool is, from its `_meta` as `tools/list` lists it, and
/// a warning for each part dropped. None (and no warning) for a tool
/// that declares no file: no `file` entrypoint and no `dev.bise/ui`
/// `files`. A tool that declares files but has no view: none, warned.
/// `server`: the server that listed it ([`OpenerDecl::server`]).
pub fn parse(plugin: &str, server: &str, tool: &Value) -> (Option<OpenerDecl>, Vec<Diagnostic>) {
    let name = tool.get("name").and_then(Value::as_str).unwrap_or("");
    let meta = tool.get("_meta").cloned().unwrap_or(Value::Null);
    let file_eps: Vec<&Value> = meta
        .get(VIBE_UI)
        .and_then(|u| u.get("entrypoints"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|e| e.get("type").and_then(Value::as_str) == Some("file"))
        .collect();
    let bise = meta.get(BISE_UI);
    let files = bise.and_then(|b| b.get("files"));
    if file_eps.is_empty() && files.is_none() {
        return (None, Vec::new());
    }
    let mut diags = Vec::new();
    let Some(uri) = crate::views::tool_ui(tool).uri else {
        diags.push(invalid(plugin, name, "it declares files but has no view (_meta.ui.resourceUri)".into()));
        return (None, diags);
    };
    let mut entries: Vec<Entry> = Vec::new();
    match files {
        None => {}
        Some(Value::Array(list)) => {
            for (i, v) in list.iter().enumerate() {
                match file_entry(i, v) {
                    Ok(e) => entries.push(e),
                    Err(why) => diags.push(invalid(plugin, name, format!("{why}; dropped"))),
                }
            }
        }
        Some(_) => diags.push(invalid(plugin, name, "dev.bise/ui.files must be a list; dropped".into())),
    }
    // Vibe's extensions: each one no `files` entry names is its own
    // entry, titled by the extension
    let mut app = None;
    for e in &file_eps {
        let t = e.get("title").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty());
        if app.is_none() {
            app = t.map(|t| t.chars().take(MAX_TITLE).collect::<String>());
        }
        for x in e.get("extensions").and_then(Value::as_array).into_iter().flatten() {
            let raw = x.as_str().unwrap_or("");
            let key = match ending_ok(&fold(raw)).then(|| type_key(raw)).flatten() {
                Some(k) => k,
                None => {
                    diags.push(invalid(plugin, name, format!("extension {x} must be like \".xlsx\"; dropped")));
                    continue;
                }
            };
            if !entries.iter().any(|en| en.keys.contains(&key)) {
                let title = format!("{} file", key.trim_start_matches("*.").to_uppercase());
                entries.push(Entry { keys: vec![key], title, rank: OpenRank::Default, folder: false });
            }
        }
    }
    if entries.len() > MAX_ENTRIES {
        diags.push(invalid(plugin, name, format!("more than {MAX_ENTRIES} entries; the rest are dropped")));
        entries.truncate(MAX_ENTRIES);
    }
    let mut makes = Vec::new();
    match bise.and_then(|b| b.get("makes")) {
        None => {}
        Some(Value::Array(list)) => {
            for (i, v) in list.iter().enumerate() {
                match make_of(i, v) {
                    Ok(m) if makes.iter().any(|x: &Make| x.ending == m.ending) => diags.push(invalid(plugin, name, format!("makes[{i}]: {} again; dropped", m.ending))),
                    Ok(m) => makes.push(m),
                    Err(why) => diags.push(invalid(plugin, name, format!("{why}; dropped"))),
                }
            }
        }
        Some(_) => diags.push(invalid(plugin, name, "dev.bise/ui.makes must be a list; dropped".into())),
    }
    let write = match bise.and_then(|b| b.get("write")) {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            diags.push(invalid(plugin, name, "dev.bise/ui.write must be true or false; taken as false".into()));
            false
        }
    };
    let isolate = match bise.and_then(|b| b.get("isolate")) {
        None => Isolate::View,
        Some(v) if v.as_str() == Some("document") => Isolate::Document,
        Some(v) if v.as_str() == Some("view") => Isolate::View,
        Some(v) => {
            diags.push(invalid(plugin, name, format!("dev.bise/ui.isolate {v} must be \"document\" (or none); taken as none")));
            Isolate::View
        }
    };
    let icon = match bise.and_then(|b| b.get("icon")) {
        None => None,
        Some(Value::String(p)) if inside_plugin(p) && ICON_ENDINGS.iter().any(|e| fold(p).ends_with(e)) => Some(p.clone()),
        Some(v) => {
            diags.push(invalid(plugin, name, format!("dev.bise/ui.icon {v} must be a ./ path to a .svg, .png or .webp inside the plugin; dropped")));
            None
        }
    };
    // a static view is bise's to draw; a server's runs its own code, so
    // only when it says so
    let thumbnail = match bise.and_then(|b| b.get("thumbnail")) {
        None => server == crate::views::SERVER,
        Some(Value::Bool(b)) => *b || server == crate::views::SERVER,
        Some(_) => {
            diags.push(invalid(plugin, name, "dev.bise/ui.thumbnail must be true or false; taken as false".into()));
            server == crate::views::SERVER
        }
    };
    if entries.is_empty() {
        diags.push(invalid(plugin, name, "no valid file entry; dropped".into()));
        return (None, diags);
    }
    let app = app.unwrap_or_else(|| plugin.to_string());
    (Some(OpenerDecl { server: server.to_string(), tool: name.to_string(), uri, app, entries, makes, write, isolate, icon, thumbnail }), diags)
}

/// A plugin's openers from one server's tools as its `tools/list` lists
/// them, at most [`MAX_OPENERS`], and the warnings.
pub fn openers_of(plugin: &str, server: &str, tools: &[Value]) -> (Vec<OpenerDecl>, Vec<Diagnostic>) {
    let mut out = Vec::new();
    let mut diags = Vec::new();
    for t in tools {
        let (o, d) = parse(plugin, server, t);
        diags.extend(d);
        if let Some(o) = o {
            if out.len() == MAX_OPENERS {
                diags.push(invalid(plugin, &o.tool, format!("more than {MAX_OPENERS} openers; the rest are dropped")));
                break;
            }
            out.push(o);
        }
    }
    (out, diags)
}

/// What a plugin's openers open and make, for its plugins-page row (A3):
/// each type once (its first opener's), each ending once.
pub fn plugin_rows(openers: &[OpenerDecl]) -> (Vec<PluginOpens>, Vec<PluginMake>) {
    let mut opens: Vec<PluginOpens> = Vec::new();
    let mut makes: Vec<PluginMake> = Vec::new();
    for o in openers {
        for e in &o.entries {
            for k in &e.keys {
                if !opens.iter().any(|x| x.type_key == *k) {
                    opens.push(PluginOpens { type_key: k.clone(), title: e.title.clone(), rank: e.rank, app: o.app.clone() });
                }
            }
        }
        for m in &o.makes {
            if !makes.iter().any(|x| x.ending == m.ending) {
                makes.push(PluginMake { ending: m.ending.clone(), title: m.title.clone(), app: o.app.clone() });
            }
        }
    }
    (opens, makes)
}

/// The one rule for a plugin's documents folder (`${PLUGIN_DOCUMENTS}`,
/// A6): it is made only for openers that make files or open types under
/// `@documents/`; a plain server or view never gets one.
pub fn uses_documents(openers: &[OpenerDecl]) -> bool {
    openers.iter().any(|o| !o.makes.is_empty() || o.entries.iter().any(|e| e.keys.iter().any(|k| k.starts_with(DOCUMENTS))))
}

/// What is opened: a file or a folder (A2's target).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    /// a folder, with the names it holds at its first level
    Folder(Vec<String>),
}

/// What [`pick`] opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// its own name (`plan.xlsx`, `site`)
    pub name: String,
    pub kind: Kind,
    /// it lies under a plugin's documents: (that plugin, its path
    /// relative to them, `boards/q3.board.json`)
    pub documents: Option<(String, String)>,
}

impl Target {
    pub fn file(name: &str) -> Target {
        Target { name: name.to_string(), kind: Kind::File, documents: None }
    }
}

/// Every on plugin's openers (static ones from disk, a server's from its
/// last `tools/list`), the caller's to gather: an off plugin is left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Openers {
    pub plugins: Vec<PluginOpeners>,
}

/// One on plugin's openers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginOpeners {
    pub plugin: String,
    /// its app's icon file: its first opener's that declares one, under
    /// its root (what the hub's [`ICON_ROUTE`] reads)
    pub icon: Option<std::path::PathBuf>,
    /// bise's own (scope built-in, from the resolver): wins a tie over a
    /// third party's claim (architect m_60451); his pick still decides
    pub first_party: bool,
    pub openers: Vec<OpenerDecl>,
}

/// A server's tools as its last `tools/list` gave them, by (plugin,
/// server id): the hub's views bridge keeps it fresh.
pub type ServerTools = std::collections::BTreeMap<(String, String), Vec<Value>>;

impl Openers {
    /// Every on plugin's openers: a loaded plugin's static views first
    /// (from the resolution), then each of its on servers' (stdio, then
    /// remote) from `server_tools`, at most [`MAX_OPENERS`] a plugin; an
    /// off plugin (not loaded) and an off server (not in the resolution's
    /// lists) give none. Plugins by name. With the warnings of the
    /// servers' openers (the static ones' are said at resolve).
    pub fn gather(res: &crate::resolve::Resolution, server_tools: &ServerTools) -> (Openers, Vec<Diagnostic>) {
        let mut plugins = Vec::new();
        let mut diags = Vec::new();
        let mut loaded: Vec<_> = res.loaded().collect();
        loaded.sort_by(|a, b| a.name.cmp(&b.name));
        for p in loaded {
            let statics: Vec<Value> = p.views.iter().map(crate::views::StaticView::tool).collect();
            let mut list = openers_of(&p.name, crate::views::SERVER, &statics).0;
            let servers = p.servers.iter().map(|s| s.id.as_str()).chain(p.remotes.iter().map(|s| s.id.as_str()));
            for sv in servers {
                let Some(tools) = server_tools.get(&(p.name.clone(), sv.to_string())) else { continue };
                let (more, d) = openers_of(&p.name, sv, tools);
                diags.extend(d);
                list.extend(more);
            }
            if list.len() > MAX_OPENERS {
                diags.push(invalid(&p.name, &list[MAX_OPENERS].tool, format!("more than {MAX_OPENERS} openers; the rest are dropped")));
                list.truncate(MAX_OPENERS);
            }
            if !list.is_empty() {
                let icon = list.iter().find_map(|o| o.icon.as_deref()).and_then(|i| i.strip_prefix("./")).map(|rel| p.root.join(rel));
                plugins.push(PluginOpeners { plugin: p.name.clone(), icon, first_party: p.scope == crate::resolve::Scope::BuiltIn, openers: list });
            }
        }
        (Openers { plugins }, diags)
    }

    /// An on plugin's opener by its tool's name (an [`Opener`]'s
    /// `plugin` and `tool`): what view/open routes on. None when that
    /// plugin is off or has no such opener now.
    pub fn decl(&self, plugin: &str, tool: &str) -> Option<&OpenerDecl> {
        self.plugins.iter().find(|p| p.plugin == plugin)?.openers.iter().find(|o| o.tool == tool)
    }

    /// The documents folders to make after a [`gather`](Self::gather):
    /// each plugin whose openers (static or a server's) [`uses_documents`],
    /// its resolved `documents_root`; pure, the hub makes them. A plugin
    /// with no documents root (an invalid name) gives none.
    pub fn documents_to_make(&self, res: &crate::resolve::Resolution) -> Vec<std::path::PathBuf> {
        self.plugins
            .iter()
            .filter(|p| uses_documents(&p.openers))
            .filter_map(|p| res.loaded().find(|r| r.name == p.plugin))
            .map(|r| r.documents_root.clone())
            .filter(|d| !d.as_os_str().is_empty())
            .collect()
    }
}

/// [`pick`]'s answer: the app it opens in (none: no candidate, the
/// window falls back to his Mac's app) and the others, for Open with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Opened {
    pub opener: Option<Opener>,
    pub others: Vec<Opener>,
}

/// `*` (any run but `/`), `**` (any run), `?` (one character but `/`).
fn glob(pat: &[char], text: &[char]) -> bool {
    match pat.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => {
            let (deep, rest) = match rest.split_first() {
                Some(('*', r)) => (true, r),
                _ => (false, rest),
            };
            (0..=text.len()).take_while(|&i| deep || i == 0 || text[i - 1] != '/').any(|i| glob(rest, &text[i..]))
        }
        Some(('?', rest)) => text.first().is_some_and(|c| *c != '/') && glob(rest, &text[1..]),
        Some((c, rest)) => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

/// Whether canonical `key` matches `text` ([`fold`]ed here).
pub fn matches(key: &str, text: &str) -> bool {
    let p: Vec<char> = key.chars().collect();
    let t: Vec<char> = fold(text).chars().collect();
    glob(&p, &t)
}

/// How specific a matching key is (A2): its plugin's own documents scope
/// first, then the longest literal ending (`*.board.json` over `*.json`).
fn specificity(key: &str) -> (bool, usize) {
    let lit = key.rsplit(['*', '?']).next().unwrap_or(key).chars().count();
    (key.starts_with(DOCUMENTS), lit)
}

/// The key of `entry` that matches `target` for `plugin`, the most
/// specific one first.
fn entry_match(plugin: &str, entry: &Entry, target: &Target) -> Option<String> {
    let hit = |k: &&String| match (&target.kind, entry.folder, k.strip_prefix(DOCUMENTS)) {
        (Kind::File, false, Some(body)) => target.documents.as_ref().is_some_and(|(p, rel)| p == plugin && matches(body, rel)),
        (Kind::File, false, None) => matches(k, &target.name),
        (Kind::Folder(names), true, None) => names.iter().any(|n| matches(k, &format!("{}/{}", target.name, n))),
        _ => false,
    };
    entry.keys.iter().filter(hit).max_by_key(|k| specificity(k)).cloned()
}

struct Candidate {
    opener: Opener,
    rank: OpenRank,
    spec: (bool, usize),
    first_party: bool,
}

/// One built-in type (A3): bise draws it on today's path until its
/// migration step lands and deletes the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Builtin {
    /// a canonical key
    pub key: &'static str,
    /// the plugin its step moves it to
    pub plugin: &'static str,
    pub app: &'static str,
    pub type_title: &'static str,
    /// today's viewer kind (`pages::site::viewer_kind`), its tool's name
    pub kind: &'static str,
    pub folder: bool,
    /// its design-system icon (one of [`BUILTIN_ICONS`]), its opener's
    /// `icon`: the one per-type icon table, which goes with its row
    pub icon: &'static str,
}

/// The design-system icons a built-in type may carry (A6b).
pub const BUILTIN_ICONS: [&str; 6] = ["image", "video", "doc", "sheet", "deck", "page"];

const fn b(key: &'static str, plugin: &'static str, app: &'static str, type_title: &'static str, kind: &'static str, icon: &'static str) -> Builtin {
    Builtin { key, plugin, app, type_title, kind, folder: false, icon }
}

/// bise's own types not migrated yet (docs/plugin-apps.md A3, migration
/// table): each step deletes its rows with its `viewer_kind` arms.
pub const BUILTIN: &[Builtin] = &[
    b("*.png", "media", "Preview", "PNG image", "image", "image"),
    b("*.jpg", "media", "Preview", "JPEG image", "image", "image"),
    b("*.jpeg", "media", "Preview", "JPEG image", "image", "image"),
    b("*.gif", "media", "Preview", "GIF image", "image", "image"),
    b("*.webp", "media", "Preview", "WebP image", "image", "image"),
    b("*.md", "docs", "Writer", "Markdown document", "markdown", "doc"),
    b("*.markdown", "docs", "Writer", "Markdown document", "markdown", "doc"),
    b("*.pdf", "media", "Preview", "PDF document", "pdf", "doc"),
    b("*.mp4", "media", "Preview", "MP4 video", "video", "video"),
    b("*.mov", "media", "Preview", "QuickTime video", "video", "video"),
    b("*.webm", "media", "Preview", "WebM video", "video", "video"),
    b("*.m4v", "media", "Preview", "MPEG-4 video", "video", "video"),
    b("*.docx", "docs", "Writer", "Word document", "docx", "doc"),
    b("*.pptx", "slides", "Slides", "PowerPoint deck", "pptx", "deck"),
    b("*.xlsx", "sheets", "Sheets", "Excel workbook", "xlsx", "sheet"),
    b("*.html", "sites", "Sites", "Web page", "site", "page"),
    b("*.htm", "sites", "Sites", "Web page", "site", "page"),
    Builtin { key: "*/index.html", plugin: "sites", app: "Sites", type_title: "Website", kind: "site", folder: true, icon: "page" },
    // a bise page: the hub names a page artifact `<id>.page` for A2 (A9,
    // architect m_60436); today's frame is its `/p/<id>`
    b("*.page", "pages", "Pages", "bise page", "page", "page"),
];

/// Which app opens `target` (A2), and the others in the same order.
/// Candidates: every entry of `openers` and [`BUILTIN`] that matches.
/// Order: his pick (`picks`, by the matched key, when its opener is a
/// candidate for that key) > the most specific match > rank (`owner` >
/// `default`) > the plugin > the tool. An `option` entry is never picked
/// by itself, only listed; one opener is listed once, by its best match.
pub fn pick(target: &Target, openers: &Openers, picks: &Picks) -> Opened {
    let mut cands: Vec<Candidate> = Vec::new();
    for po in &openers.plugins {
        for o in &po.openers {
            let best = o
                .entries
                .iter()
                .filter_map(|e| entry_match(&po.plugin, e, target).map(|k| (specificity(&k), e.rank, k, &e.title)))
                .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
            if let Some((spec, rank, key, title)) = best {
                let opener = Opener { plugin: po.plugin.clone(), tool: o.tool.clone(), app: o.app.clone(), type_key: key, type_title: title.clone(), icon: po.icon.as_ref().map(|_| format!("{ICON_ROUTE}{}", po.plugin)), builtin: false };
                cands.push(Candidate { opener, rank, spec, first_party: po.first_party });
            }
        }
    }
    for bi in BUILTIN {
        let entry = Entry { keys: vec![bi.key.to_string()], title: bi.type_title.to_string(), rank: OpenRank::Owner, folder: bi.folder };
        if let Some(key) = entry_match(bi.plugin, &entry, target) {
            // its plugin opens this itself now (migrated, or both on a
            // branch): the plugin's own opener stands, listed once
            if cands.iter().any(|c| c.opener.plugin == bi.plugin) {
                continue;
            }
            let opener = Opener { plugin: bi.plugin.into(), tool: bi.kind.into(), app: bi.app.into(), type_key: key.clone(), type_title: bi.type_title.into(), icon: Some(bi.icon.into()), builtin: true };
            cands.push(Candidate { opener, rank: OpenRank::Owner, spec: specificity(&key), first_party: true });
        }
    }
    cands.sort_by(|a, b| {
        b.spec
            .cmp(&a.spec)
            .then(a.rank.cmp(&b.rank))
            .then(b.first_party.cmp(&a.first_party))
            .then_with(|| a.opener.plugin.cmp(&b.opener.plugin))
            .then_with(|| a.opener.tool.cmp(&b.opener.tool))
    });
    // his pick: the first key, most specific first, whose pick names one
    // of the candidates matched by that same key
    let picked = cands.iter().find_map(|c| {
        let r = picks.get(&c.opener.type_key)?;
        cands.iter().position(|x| x.opener.type_key == c.opener.type_key && *r == OpenerRef { plugin: x.opener.plugin.clone(), tool: x.opener.tool.clone() })
    });
    let chosen = picked.or_else(|| cands.iter().position(|c| c.rank != OpenRank::Option));
    let opener = chosen.map(|i| cands.remove(i).opener);
    Opened { opener, others: cands.into_iter().map(|c| c.opener).collect() }
}

#[cfg(test)]
mod tests;
