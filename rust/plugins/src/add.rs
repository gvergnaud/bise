//! The add menu (docs/plugins.md, "Add"): an MCP server, an import of
//! another agent's MCP servers, a plugin from a folder or a git URL (a
//! marketplace's plugin is a git URL with its folder), as library calls
//! the terminal and the desktop's core both make.
//!
//! Two steps, so nothing of a plugin runs before his click:
//!
//! 1. [`stage`] builds the plugin in a staging folder
//!    ([`staging_dir`]: `<user root>/.staging/<token>/<name>`; a dot
//!    folder is never a plugin) and checks it with the resolver. A git
//!    source is cloned there ([`git_clone`]: no hook, no submodule, no
//!    `ext::`, a timeout); a Codex or Claude plugin is converted to an
//!    Agent Plugin (a bise `plugin.json`, its `.mcp.json` through
//!    [`crate::import::plan_for`]). Its [`Preview`] lists every server
//!    it will start (the exact argv) or call (transport and host), its
//!    skills, the names of its secrets and what was left out. Nothing of
//!    the plugin runs; `stage` may run git, so never on a UI thread.
//! 2. [`install`], on his click: the secrets first
//!    ([`crate::secrets`]), then one rename into `<user root>/<name>`,
//!    the one commit point. [`discard`] drops a preview he closed.
//!
//! "Make a plugin with an agent" is no call here: both apps send
//! [`agent_brief`] to main the way "start an agent" does, and the agent's
//! plugin comes back through a folder add, the same preview and click.
//!
//! Secret values (an env var's, a header's) never reach `mcp.json`, a
//! preview or an error: `mcp.json` keeps `${VAR}`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bise_proto::app_cmd::{ImportFrom, SecretValue};
use bise_proto::app_rows::{PluginPreview, PluginRun, RunTransport};
use serde_json::{json, Map, Value};

use crate::id::PluginId;
use crate::model::StateV2;
use crate::import::{self, Home, Plan};
use crate::resolve::{self, Roots, Severity, State, PLUGIN_SCHEMA};

/// What a plugin will run, shown before his click (the wire's type).
pub type Preview = PluginPreview;

/// How long a git clone may take.
pub const GIT_TIMEOUT: Duration = Duration::from_secs(120);

/// A staged plugin nobody installed or discarded (a crash, a closed
/// window) goes at the next [`stage`] after this long.
pub const STAGING_TTL: Duration = Duration::from_secs(24 * 3600);

/// Why an add can't go on. The apps pick their words from the kind,
/// never from the text (no secret in any of them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddError {
    /// not a plugin name: [a-z0-9.-], 1-64, alphanumeric at both ends
    BadName { name: String },
    /// he has a plugin of that name already (not one bise wrote for the
    /// same kind of add)
    Exists { name: String },
    /// no `plugin.json` (nor `.codex-plugin/` or `.claude-plugin/`) there
    NoPlugin,
    /// the resolver refuses it: its diagnostics
    Invalid { diagnostics: Vec<String> },
    /// an MCP add or import with no server bise can run (`why`: each
    /// server's reason)
    NoServer { why: Vec<String> },
    /// a source that can't be read (a missing folder, a bad URL, a
    /// subfolder outside the repo, a config that does not parse)
    BadSource { why: String },
    Git { why: String },
    Secrets { why: String },
    Io { why: String },
    /// `plugin_add_confirm`/`cancel` with a token the core doesn't hold
    /// (a preview already used, closed, or from before a restart)
    UnknownToken,
    /// a value for a name the preview doesn't list: nothing written
    UnknownSecret { name: String },
}

impl std::fmt::Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddError::BadName { name } => write!(f, "{name:?} is not a plugin name: [a-z0-9.-], 1-64 chars, alphanumeric at both ends"),
            AddError::Exists { name } => write!(f, "you have a plugin named {name} already: pick another name"),
            AddError::NoPlugin => write!(f, "no plugin.json there"),
            AddError::Invalid { diagnostics } => write!(f, "not a valid plugin: {}", diagnostics.join("; ")),
            AddError::NoServer { why } => write!(f, "no server bise can run: {}", why.join("; ")),
            AddError::UnknownToken => write!(f, "that preview is gone: add it again"),
            AddError::UnknownSecret { name } => write!(f, "{name} is not a secret of this plugin"),
            AddError::BadSource { why } | AddError::Git { why } | AddError::Secrets { why } | AddError::Io { why } => write!(f, "{why}"),
        }
    }
}

fn io(e: std::io::Error) -> AddError {
    AddError::Io { why: e.to_string() }
}

/// Where an add reads and writes.
#[derive(Clone, Debug)]
pub struct Ctx {
    /// `~/.agents/plugins` (or `$BEND_PLUGINS_HOME`)
    pub user_root: PathBuf,
    /// his home folder: where Claude Code's, Codex's and Cursor's
    /// configs are
    pub home: PathBuf,
    /// `Home::mcp_secrets_dir()`
    pub secrets_dir: PathBuf,
    /// bise's built-in plugins (a plugin of the same name shadows one)
    pub builtin: Option<PathBuf>,
    /// where `${PLUGIN_DATA}` roots live
    pub data: PathBuf,
    /// where `${PLUGIN_DOCUMENTS}` roots live (`Roots::documents`)
    pub documents: PathBuf,
    /// `plugins.json` ([`crate::state::state_path`]): a removed
    /// plugin's ids go
    pub state: PathBuf,
    /// the servers' last states ([`crate::status::dir`]): the try after
    /// his confirm writes there, a remove drops them
    pub status_dir: PathBuf,
    /// remote servers' logins ([`crate::oauth::store_dir`])
    pub oauth_dir: PathBuf,
}

impl Ctx {
    pub fn standard() -> Ctx {
        let roots = Roots::standard(None);
        Ctx {
            user_root: roots.user.unwrap_or_else(|| resolve::home().join(".agents/plugins")),
            home: resolve::home(),
            secrets_dir: bise_home::Home::from_env().mcp_secrets_dir(),
            builtin: roots.builtin,
            data: roots.data,
            documents: roots.documents,
            state: crate::state::state_path(),
            status_dir: crate::status::dir(),
            oauth_dir: crate::oauth::store_dir(),
        }
    }

    /// Every path under `root` (tests and throwaway hubs).
    pub fn under(root: &Path) -> Ctx {
        Ctx {
            user_root: root.join("plugins"),
            home: root.join("home"),
            secrets_dir: root.join("secrets"),
            builtin: None,
            data: root.join("data"),
            documents: root.join("documents"),
            state: root.join("plugins.json"),
            status_dir: root.join("mcp-status"),
            oauth_dir: root.join("mcp-oauth"),
        }
    }
}

/// What he adds (the wire's `PluginAddKind`, with his secret values).
#[derive(Clone, PartialEq, Eq)]
pub enum Request {
    /// an MCP server: a command line, or a remote server's URL;
    /// `secrets`: (env var name for a command, header name for a URL,
    /// its value)
    Mcp { name: String, command: Option<String>, url: Option<String>, secrets: Vec<(String, String)> },
    /// another agent's MCP servers, from its config on disk (`name`:
    /// none, `from-claude-code`, `from-codex`, `from-cursor`)
    Import { from: ImportFrom, name: Option<String> },
    /// a plugin folder on his Mac
    Folder { path: PathBuf },
    /// a plugin in a git repo, at its root or in `subdir`
    Git { url: String, subdir: Option<String> },
}

impl std::fmt::Debug for Request {
    /// never a secret's value
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Request::Mcp { name, command, url, secrets } => {
                let names: Vec<&str> = secrets.iter().map(|(k, _)| k.as_str()).collect();
                write!(f, "Mcp {{ name: {name:?}, command: {command:?}, url: {url:?}, secrets: {names:?} }}")
            }
            Request::Import { from, name } => write!(f, "Import {{ from: {from:?}, name: {name:?} }}"),
            Request::Folder { path } => write!(f, "Folder {{ path: {path:?} }}"),
            Request::Git { url, subdir } => write!(f, "Git {{ url: {url:?}, subdir: {subdir:?} }}"),
        }
    }
}

impl Request {
    /// The wire's `plugin_add` as a request (the desktop's core): an MCP
    /// server's secrets come as names only, so `mcp.json` gets `${NAME}`
    /// and no value is written (the bridge takes it from the environment
    /// or the secrets store). None: a marketplace or an agent, not an add
    /// of a plugin (`marketplace::add`, [`agent_brief`]).
    pub fn from_wire(add: &bise_proto::app_cmd::PluginAddKind) -> Option<Request> {
        use bise_proto::app_cmd::PluginAddKind as K;
        Some(match add {
            K::Mcp { name, command, url, secrets } => Request::Mcp {
                name: name.clone(),
                command: command.clone(),
                url: url.clone(),
                secrets: secrets.iter().map(|n| (n.clone(), String::new())).collect(),
            },
            K::Import { from } => Request::Import { from: *from, name: None },
            K::Folder { path } => Request::Folder { path: PathBuf::from(path) },
            K::Git { url, subdir } => Request::Git { url: url.clone(), subdir: subdir.clone() },
            K::Marketplace { .. } | K::Agent { .. } => return None,
        })
    }
}

/// A plugin built in the staging folder, waiting for his click.
pub struct Staged {
    pub preview: Preview,
    token: String,
    /// `<staging>/<token>`
    dir: PathBuf,
    /// `<staging>/<token>/<name>`
    plugin: PathBuf,
    secrets: Vec<(String, String)>,
}

impl Staged {
    /// What the desktop's core keys it by (`plugin_preview {token}`).
    pub fn token(&self) -> &str {
        &self.token
    }
}

impl std::fmt::Debug for Staged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Staged {{ token: {:?}, preview: {:?} }}", self.token, self.preview)
    }
}

/// The staging folder of `user_root`: a dot folder, never a plugin.
pub fn staging_dir(user_root: &Path) -> PathBuf {
    user_root.join(".staging")
}

/// Build what `req` adds in the staging folder, check it, and say what
/// it will run. Nothing of the plugin runs (git may: off the UI thread).
pub fn stage(req: &Request, ctx: &Ctx) -> Result<Staged, AddError> {
    let staging = staging_dir(&ctx.user_root);
    sweep(&staging, STAGING_TTL);
    let token = token();
    let dir = staging.join(&token);
    std::fs::create_dir_all(&dir).map_err(io)?;
    match build(req, ctx, &dir) {
        Ok(b) => check(b, ctx, token, dir),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(e)
        }
    }
}

/// What a source made in the staging folder, before the check.
struct Built {
    name: String,
    plugin: PathBuf,
    from: String,
    secrets: Vec<(String, String)>,
    dropped: Vec<String>,
    notes: Vec<String>,
    /// bise writes this folder (an MCP add or import): it may replace
    /// bise's own folder of that name
    bise_mcp: bool,
}

fn build(req: &Request, ctx: &Ctx, dir: &Path) -> Result<Built, AddError> {
    match req {
        Request::Mcp { name, command, url, secrets } => mcp(name, command.as_deref(), url.as_deref(), secrets, dir),
        Request::Import { from, name } => {
            let name = name.clone().unwrap_or_else(|| default_import_name(*from).to_string());
            let (path, input) = import_input(*from, &ctx.home)?;
            let plan = import::plan(&input).map_err(|why| AddError::BadSource { why })?;
            let shown = shown(&path, &ctx.home);
            let b = write_servers(&name, &plan, import::MARK, dir)?;
            Ok(Built { from: format!("{} ({})", import_title(*from), shown), ..b })
        }
        Request::Folder { path } => {
            if !path.is_dir() {
                return Err(AddError::BadSource { why: format!("{} is not a folder", path.display()) });
            }
            let mut b = adopt(path, dir)?;
            b.from = shown(path, &ctx.home);
            Ok(b)
        }
        Request::Git { url, subdir } => {
            let url = git_url(url)?;
            let src = dir.join(".src");
            git_clone(&url, &src, GIT_TIMEOUT)?;
            let at = match subdir.as_deref().filter(|s| !s.is_empty() && *s != ".") {
                None => src.clone(),
                Some(s) => {
                    let p = src.join(s.trim_start_matches("./"));
                    if s.starts_with('/') || s.split('/').any(|c| c == "..") || !p.is_dir() {
                        return Err(AddError::BadSource { why: format!("no folder {s} in the repo") });
                    }
                    p
                }
            };
            let mut b = adopt(&at, dir)?;
            let _ = std::fs::remove_dir_all(&src);
            b.from = match subdir.as_deref().filter(|s| !s.is_empty() && *s != ".") {
                Some(s) => format!("git {} ({})", url_shown(&url), s.trim_start_matches("./")),
                None => format!("git {}", url_shown(&url)),
            };
            Ok(b)
        }
    }
}

/// The add menu's MCP server: its one server, through import's plan.
fn mcp(name: &str, command: Option<&str>, url: Option<&str>, secrets: &[(String, String)], dir: &Path) -> Result<Built, AddError> {
    let bad = |why: &str| AddError::BadSource { why: why.to_string() };
    let server = match (command.map(str::trim).filter(|c| !c.is_empty()), url.map(str::trim).filter(|u| !u.is_empty())) {
        (Some(c), None) => {
            let argv = split_words(c).ok_or_else(|| bad("the command has an unclosed quote"))?;
            let mut env = Map::new();
            for (k, _) in secrets {
                if !valid_var(k) {
                    return Err(bad(&format!("{k:?} is not an env var name")));
                }
                env.insert(k.clone(), json!(format!("${{{k}}}")));
            }
            let mut s = json!({ "command": argv[0], "args": argv[1..] });
            if !env.is_empty() {
                s["env"] = Value::Object(env);
            }
            s
        }
        (None, Some(u)) => {
            let mut headers = Map::new();
            for (k, _) in secrets {
                if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                    return Err(bad(&format!("{k:?} is not a header name")));
                }
                headers.insert(k.clone(), json!(format!("${{{}}}", header_var(k))));
            }
            let ty = if u.trim_end_matches('/').ends_with("/sse") { "sse" } else { "http" };
            let mut s = json!({ "type": ty, "url": u });
            if !headers.is_empty() {
                s["headers"] = Value::Object(headers);
            }
            s
        }
        _ => return Err(bad("give a command or a URL")),
    };
    let plan = import::plan(&json!({ "mcpServers": { name: server } })).map_err(|why| AddError::BadSource { why })?;
    // a header's value is kept under its env-var-shaped name
    let secrets: Vec<(String, String)> = match url {
        Some(_) if command.is_none() => secrets.iter().map(|(k, v)| (header_var(k), v.clone())).collect(),
        _ => secrets.to_vec(),
    };
    let b = write_servers(name, &plan, import::MARK_ADD, dir)?;
    Ok(Built { from: "an MCP server you typed".into(), secrets, ..b })
}

/// `Authorization` -> `AUTHORIZATION`, `X-Api-Key` -> `X_API_KEY`.
fn header_var(h: &str) -> String {
    h.chars().map(|c| if c == '-' { '_' } else { c.to_ascii_uppercase() }).collect()
}

fn valid_var(v: &str) -> bool {
    !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !v.starts_with(|c: char| c.is_ascii_digit())
}

/// A plan written as bise's MCP folder `name` in the staging folder.
fn write_servers(name: &str, plan: &Plan, mark: &str, dir: &Path) -> Result<Built, AddError> {
    if !resolve::valid_name(name) {
        return Err(AddError::BadName { name: name.to_string() });
    }
    if plan.servers.is_empty() {
        return Err(AddError::NoServer { why: plan.dropped.clone() });
    }
    let plugin = import::write(dir, name, plan, mark).map_err(|why| AddError::Io { why })?;
    Ok(Built {
        name: name.to_string(),
        plugin,
        from: String::new(),
        secrets: Vec::new(),
        dropped: plan.dropped.clone(),
        notes: Vec::new(),
        bise_mcp: true,
    })
}

/// A plugin folder (his, or a clone's) copied into the staging folder as
/// an Agent Plugin: as is when it has a `plugin.json`, converted when it
/// is Codex's (`.codex-plugin/plugin.json`) or Claude Code's
/// (`.claude-plugin/plugin.json`). `.git` is left out.
fn adopt(src: &Path, dir: &Path) -> Result<Built, AddError> {
    let foreign = [".codex-plugin", ".claude-plugin"].into_iter().map(|d| src.join(d).join("plugin.json")).find(|p| p.is_file());
    let own = src.join("plugin.json");
    let (name, manifest) = if own.is_file() {
        let text = std::fs::read_to_string(&own).map_err(io)?;
        let name = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("name").and_then(Value::as_str).map(str::to_string))
            .ok_or_else(|| AddError::Invalid { diagnostics: vec![resolve::parse_manifest(&text).err().unwrap_or_else(|| "no /name".into())] })?;
        (name, None)
    } else if let Some(f) = foreign {
        let v: Value = std::fs::read_to_string(&f)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .ok_or_else(|| AddError::Invalid { diagnostics: vec![format!("{}: not JSON", shown_rel(&f, src))] })?;
        let name = v.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        (name, Some(v))
    } else {
        return Err(AddError::NoPlugin);
    };
    if !resolve::valid_name(&name) {
        return Err(AddError::BadName { name });
    }
    let plugin = dir.join(&name);
    copy_tree(src, &plugin).map_err(io)?;
    let mut dropped = Vec::new();
    let mut notes = Vec::new();
    if let Some(m) = manifest {
        let c = convert(&m, &plugin)?;
        dropped = c.dropped;
        notes.push(c.note);
    }
    Ok(Built { name, plugin, from: String::new(), secrets: Vec::new(), dropped, notes, bise_mcp: false })
}

/// A Codex or Claude plugin made an Agent Plugin, in place.
struct Converted {
    dropped: Vec<String>,
    note: String,
}

/// The manifest keys the 1.0 schema has.
const MANIFEST_KEYS: [&str; 7] = ["name", "version", "description", "author", "homepage", "repository", "license"];

/// Write `plugin.json` (and `mcp.json`) of an Agent Plugin in `dir` from
/// a Codex or Claude manifest `m`: the 1.0 keys kept, its MCP servers
/// (`mcpServers`: a path, `./.mcp.json` by default, or the map) through
/// [`import::plan_for`] in the plugin's own folder, `${CLAUDE_PLUGIN_ROOT}`
/// read as `${PLUGIN_ROOT}`; every other key listed as dropped.
fn convert(m: &Value, dir: &Path) -> Result<Converted, AddError> {
    let o = m.as_object().ok_or(AddError::Invalid { diagnostics: vec!["the manifest is not a JSON object".into()] })?;
    let mut out = Map::new();
    out.insert("$schema".into(), json!(PLUGIN_SCHEMA));
    let mut dropped = Vec::new();
    for (k, v) in o {
        match k.as_str() {
            "author" if v.is_object() || v.is_string() => {
                let a = if v.is_string() { json!({ "name": v }) } else { v.clone() };
                out.insert(k.clone(), a);
            }
            "keywords" if v.as_array().is_some_and(|a| !a.is_empty() && a.iter().all(Value::is_string)) => {
                out.insert(k.clone(), v.clone());
            }
            k2 if MANIFEST_KEYS.contains(&k2) && v.is_string() => {
                out.insert(k.clone(), v.clone());
            }
            "mcpServers" | "skills" | "$schema" | "keywords" | "interface" => {}
            _ => dropped.push(format!("plugin.json: {k}")),
        }
    }
    let iface = o.get("interface").and_then(Value::as_object);
    if let Some(i) = iface {
        let described = !out.contains_key("description");
        if described {
            if let Some(d) = i.get("shortDescription").and_then(Value::as_str) {
                out.insert("description".into(), json!(d));
            }
        }
        let (prompts, cut) = codex_prompts(i.get("defaultPrompt"));
        if !prompts.is_empty() {
            out.insert("extensions".into(), json!({ resolve::BISE_EXTENSION: { "prompts": prompts } }));
        }
        if cut {
            dropped.push(format!(
                "plugin.json: interface.defaultPrompt beyond {} prompts of {} characters",
                resolve::MAX_PROMPTS,
                resolve::PROMPT_CHARS
            ));
        }
        for k in i.keys() {
            let kept = k == "defaultPrompt" || (k == "shortDescription" && described && out.contains_key("description"));
            if !kept {
                dropped.push(format!("plugin.json: interface.{k}"));
            }
        }
    } else if o.contains_key("interface") {
        dropped.push("plugin.json: interface".into());
    }
    // the MCP servers: a file of the plugin's, or the map itself
    let servers: Option<Value> = match o.get("mcpServers") {
        Some(Value::Object(m)) => Some(json!({ "mcpServers": m })),
        Some(Value::String(p)) => Some(read_in(dir, p)?),
        Some(_) => return Err(AddError::Invalid { diagnostics: vec!["mcpServers is not a path or an object".into()] }),
        None if dir.join(".mcp.json").is_file() => Some(read_in(dir, ".mcp.json")?),
        None => None,
    };
    if let Some(s) = servers {
        let text = serde_json::to_string(&s).unwrap_or_default().replace("${CLAUDE_PLUGIN_ROOT}", "${PLUGIN_ROOT}");
        let s: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let plan = import::plan_for(&s, Home::Plugin).map_err(|e| AddError::Invalid { diagnostics: vec![e] })?;
        dropped.extend(plan.dropped);
        if !plan.servers.is_empty() {
            let mcp = json!({ "$schema": resolve::MCP_SCHEMA, "mcpServers": Value::Object(plan.servers) });
            std::fs::write(dir.join("mcp.json"), serde_json::to_string_pretty(&mcp).unwrap_or_default() + "\n").map_err(io)?;
        }
    }
    let manifest = serde_json::to_string_pretty(&Value::Object(out)).unwrap_or_default() + "\n";
    std::fs::write(dir.join("plugin.json"), manifest).map_err(io)?;
    let note = if dir.join(".codex-plugin").is_dir() { "a Codex plugin, made an Agent Plugin" } else { "a Claude Code plugin, made an Agent Plugin" };
    Ok(Converted { dropped, note: note.into() })
}

/// Codex's `interface.defaultPrompt` (a string or a list of strings) as
/// `dev.bise.prompts`, cut by the resolver's own rule
/// ([`resolve::cut_prompts`]): the prompts kept, and whether any was cut.
fn codex_prompts(v: Option<&Value>) -> (Vec<String>, bool) {
    let list: Vec<String> = match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        _ => Vec::new(),
    };
    // one line each: a prompt is a one-liner on the row
    let lines: Vec<String> = list.iter().map(|p| p.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    resolve::cut_prompts(&lines)
}

/// The JSON file at `rel` inside `dir` (never outside it).
fn read_in(dir: &Path, rel: &str) -> Result<Value, AddError> {
    let bad = |why: String| AddError::Invalid { diagnostics: vec![why] };
    let p = dir.join(rel.trim_start_matches("./"));
    if rel.starts_with('/') || rel.split('/').any(|c| c == "..") {
        return Err(bad(format!("{rel} is outside the plugin")));
    }
    let text = std::fs::read_to_string(&p).map_err(|e| bad(format!("{rel}: {e}")))?;
    serde_json::from_str(&text).map_err(|_| bad(format!("{rel} is not JSON")))
}

/// Resolve the staged plugin alone, refuse it when it doesn't load, and
/// make its preview.
fn check(b: Built, ctx: &Ctx, token: String, dir: PathBuf) -> Result<Staged, AddError> {
    let fail = |e: AddError, dir: &Path| {
        let _ = std::fs::remove_dir_all(dir);
        Err(e)
    };
    let roots = Roots { builtin: None, user: Some(dir.clone()), workspace: None, data: ctx.data.clone(), documents: ctx.documents.clone(), state: StateV2 { enabled: vec![b.name.clone()], ..StateV2::default() } };
    let res = resolve::resolve(&roots);
    let errors: Vec<String> = res.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect();
    let Some(p) = res.plugins.iter().find(|p| p.state == State::Loaded && p.name == b.name) else {
        let mut why = errors;
        if why.is_empty() {
            why.push(format!("its manifest's name is not {}", b.name));
        }
        return fail(AddError::Invalid { diagnostics: why }, &dir);
    };
    if !errors.is_empty() {
        return fail(AddError::Invalid { diagnostics: errors }, &dir);
    }
    let mut notes = b.notes.clone();
    notes.extend(res.diagnostics.iter().filter(|d| d.severity == Severity::Warning).map(|d| d.message.clone()));
    // his plugins of that name: bise's own folder of the same kind is
    // replaced, anything else refuses
    let dest = ctx.user_root.join(&b.name);
    let mine = resolve::resolve(&Roots { builtin: ctx.builtin.clone(), user: Some(ctx.user_root.clone()), workspace: None, data: ctx.data.clone(), documents: ctx.documents.clone(), state: StateV2::default() });
    let taken = mine.plugins.iter().any(|q| q.scope == resolve::Scope::User && q.name == b.name && q.root != dest.canonicalize().unwrap_or(dest.clone()));
    let mut replaces = None;
    if dest.exists() || taken {
        let ours = b.bise_mcp && !taken && description(&dest).is_some_and(|d| import::is_bise_mcp(&d));
        if !ours {
            return fail(AddError::Exists { name: b.name.clone() }, &dir);
        }
        replaces = Some(b.name.clone());
    }
    if mine.plugins.iter().any(|q| q.scope == resolve::Scope::BuiltIn && q.name == b.name) {
        notes.push(format!("it replaces bise's own {} part", b.name));
    }
    // what it runs, with the paths it will have once installed
    let staged_root = p.root.to_string_lossy().to_string();
    let final_root = dest.to_string_lossy().to_string();
    let at_home = |s: &str| s.replace(&staged_root, &final_root);
    let mut runs: Vec<PluginRun> = p
        .servers
        .iter()
        .map(|s| PluginRun {
            server: s.id.clone(),
            transport: RunTransport::Stdio,
            argv: std::iter::once(&s.command).chain(s.args.iter()).map(|a| at_home(a)).collect(),
            host: None,
        })
        .collect();
    runs.extend(p.remotes.iter().map(|r| PluginRun {
        server: r.id.clone(),
        transport: match r.transport {
            resolve::Transport::Streamable => RunTransport::Http,
            resolve::Transport::Sse => RunTransport::Sse,
        },
        argv: vec![],
        host: Some(r.host()),
    }));
    let preview = Preview {
        name: b.name.clone(),
        from: b.from.clone(),
        runs,
        skills: p.skills.iter().map(|s| s.name.clone()).collect(),
        secrets: b.secrets.iter().map(|(k, _)| k.clone()).collect(),
        dropped: b.dropped.clone(),
        notes,
        replaces,
        // its static views, their domains and permissions, never granted yet (C8: ticked on the add screen)
        views: crate::catalog::views::parts(&b.name, &p.views, &crate::state::Grants::new(), true),
        claims: Vec::new(),
    };
    Ok(Staged { preview, token, dir, plugin: b.plugin, secrets: b.secrets })
}

fn description(plugin_dir: &Path) -> Option<String> {
    let t = std::fs::read_to_string(plugin_dir.join("plugin.json")).ok()?;
    serde_json::from_str::<Value>(&t).ok()?.get("description")?.as_str().map(str::to_string)
}

/// His click: the secrets, then the staged folder renamed into his
/// plugins (the one commit point). A failed step leaves nothing
/// installed; the staged folder goes either way.
pub fn install(staged: Staged, ctx: &Ctx) -> Result<PathBuf, AddError> {
    let res = commit(&staged, ctx);
    let _ = std::fs::remove_dir_all(&staged.dir);
    res
}

fn commit(staged: &Staged, ctx: &Ctx) -> Result<PathBuf, AddError> {
    let name = &staged.preview.name;
    let dest = ctx.user_root.join(name);
    let replaces = staged.preview.replaces.is_some();
    if dest.exists() && !replaces {
        return Err(AddError::Exists { name: name.clone() });
    }
    if !staged.plugin.is_dir() {
        return Err(AddError::Io { why: "the staged plugin is gone: add it again".into() });
    }
    // a name without a value (the wire's): only its ${NAME} in mcp.json
    let values: Vec<(String, String)> = staged.secrets.iter().filter(|(_, v)| !v.is_empty()).cloned().collect();
    crate::secrets::write(&ctx.secrets_dir, &PluginId::plugin(name.clone()), &values).map_err(|e| AddError::Secrets { why: e.to_string() })?;
    let aside = staged.dir.join(".replaced");
    if replaces && dest.exists() {
        std::fs::rename(&dest, &aside).map_err(io)?;
    }
    if let Err(e) = std::fs::rename(&staged.plugin, &dest) {
        if aside.exists() {
            let _ = std::fs::rename(&aside, &dest);
        }
        return Err(io(e));
    }
    Ok(dest)
}

/// He closed the preview: its staged folder goes.
pub fn discard(staged: Staged) {
    let _ = std::fs::remove_dir_all(&staged.dir);
}

/// Right after his confirm installed `plugin_dir`: each of its MCP
/// servers started once with the bridge's own start (initialize +
/// tools/list within its startup timeout), then stopped, its state
/// written for the Plugins list (`connected · N tools`, `✗ <why>`, or
/// `needs a login` on a 401). Blocks up to the slowest server's
/// timeout: the apps call it off their UI thread. Never before his
/// click (that would run it unseen), never on a timer.
pub fn try_servers(plugin_dir: &Path, ctx: &Ctx) -> Vec<(String, crate::status::Status)> {
    let name = plugin_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let state = StateV2 { enabled: vec![name], ..StateV2::default() };
    let roots = Roots { builtin: None, user: Some(ctx.user_root.clone()), workspace: None, data: ctx.data.clone(), documents: ctx.documents.clone(), state };
    let Some(p) = resolve::resolve(&roots).plugins.into_iter().find(|p| p.root == plugin_dir && p.state == State::Loaded) else {
        return Vec::new();
    };
    // the stderr logs go with the stale previews (swept after a day)
    let logs = staging_dir(&ctx.user_root).join(format!("try-{}", token()));
    let out = crate::bridge::try_plugin(&p, &logs, Some(&ctx.oauth_dir), Some(&ctx.secrets_dir), &ctx.status_dir);
    let _ = std::fs::remove_dir_all(&logs);
    out
}

/// The previews a process shows, by token (the desktop's core:
/// `plugin_preview` -> `plugin_add_confirm`/`cancel`). Dropped whole when
/// the window closes or the core stops ([`Held::clear`], and on drop).
#[derive(Default)]
pub struct Held(std::collections::BTreeMap<String, (std::time::Instant, Staged)>);

/// How long a preview waits for his answer: a window that closed
/// without a cancel doesn't keep its token (and staged folder) forever
/// (architect m_39668).
pub const HELD_TTL: Duration = Duration::from_secs(30 * 60);

impl Held {
    /// Keep `staged` until his answer; its token and preview, for
    /// `plugin_preview`.
    pub fn hold(&mut self, staged: Staged) -> (String, Preview) {
        let out = (staged.token.clone(), staged.preview.clone());
        if let Some((_, old)) = self.0.insert(staged.token.clone(), (std::time::Instant::now(), staged)) {
            discard(old);
        }
        out
    }

    /// His click (`plugin_add_confirm {token, secrets}`): the values he
    /// typed for the preview's secrets (a name left out stays `${NAME}`),
    /// then the held plugin installed. A name the preview doesn't list
    /// refuses and writes nothing; the preview stays held.
    pub fn confirm(&mut self, token: &str, secrets: &[SecretValue], ctx: &Ctx) -> Result<PathBuf, AddError> {
        let (_, staged) = self.0.get_mut(token).ok_or(AddError::UnknownToken)?;
        if let Some(s) = secrets.iter().find(|s| !staged.preview.secrets.contains(&s.name)) {
            return Err(AddError::UnknownSecret { name: s.name.clone() });
        }
        for s in secrets.iter().filter(|s| !s.value.expose().is_empty()) {
            match staged.secrets.iter_mut().find(|(k, _)| *k == s.name) {
                Some(slot) => slot.1 = s.value.expose().to_string(),
                None => staged.secrets.push((s.name.clone(), s.value.expose().to_string())),
            }
        }
        let (_, staged) = self.0.remove(token).ok_or(AddError::UnknownToken)?;
        install(staged, ctx)
    }

    /// He closed the preview.
    pub fn cancel(&mut self, token: &str) -> Result<(), AddError> {
        discard(self.0.remove(token).ok_or(AddError::UnknownToken)?.1);
        Ok(())
    }

    /// The previews held longer than [`HELD_TTL`] at `now` dropped (the
    /// desktop's core, before each command it handles).
    pub fn sweep(&mut self, now: std::time::Instant) {
        let old: Vec<String> = self.0.iter().filter(|(_, (at, _))| now.saturating_duration_since(*at) >= HELD_TTL).map(|(k, _)| k.clone()).collect();
        for k in old {
            if let Some((_, s)) = self.0.remove(&k) {
                discard(s);
            }
        }
    }

    /// Every preview dropped (the window closed).
    pub fn clear(&mut self) {
        for (_, (_, s)) in std::mem::take(&mut self.0) {
            discard(s);
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Remove staging folders older than `ttl` (a crash between stage and
/// install leaves only those).
fn sweep(staging: &Path, ttl: Duration) {
    let Ok(rd) = std::fs::read_dir(staging) else { return };
    let now = std::time::SystemTime::now();
    for e in rd.flatten() {
        let old = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| now.duration_since(t).ok()).is_some_and(|age| age > ttl);
        if old {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// 16 hex digits from the system's random source.
pub(crate) fn token() -> String {
    use ring::rand::SecureRandom;
    let mut b = [0u8; 8];
    if ring::rand::SystemRandom::new().fill(&mut b).is_err() {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) as u64;
        b = (n ^ (std::process::id() as u64) << 32).to_be_bytes();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Copy a folder (files, folders, links as links), `.git` left out.
fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name();
        if name == ".git" {
            continue;
        }
        let (from, to) = (e.path(), dst.join(&name));
        let ty = e.file_type()?;
        if ty.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&from)?, &to)?;
        } else if ty.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Words of a command line, the way a shell splits them (quotes and
/// backslashes; no expansion). None: an unclosed quote.
pub fn split_words(s: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                has = true;
                loop {
                    match chars.next()? {
                        '\'' => break,
                        c => cur.push(c),
                    }
                }
            }
            '"' => {
                has = true;
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => match chars.next()? {
                            c @ ('"' | '\\' | '$' | '`') => cur.push(c),
                            c => {
                                cur.push('\\');
                                cur.push(c);
                            }
                        },
                        c => cur.push(c),
                    }
                }
            }
            '\\' => {
                has = true;
                cur.push(chars.next()?);
            }
            c if c.is_whitespace() => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                has = true;
                cur.push(c);
            }
        }
    }
    if has {
        out.push(cur);
    }
    (!out.is_empty()).then_some(out)
}

// ---- import from disk ------------------------------------------------

/// The plugin an import writes by default.
pub fn default_import_name(from: ImportFrom) -> &'static str {
    match from {
        ImportFrom::Claude => "from-claude-code",
        ImportFrom::Codex => "from-codex",
        ImportFrom::Cursor => "from-cursor",
    }
}

pub(crate) fn import_title(from: ImportFrom) -> &'static str {
    match from {
        ImportFrom::Claude => "Claude Code",
        ImportFrom::Codex => "Codex",
        ImportFrom::Cursor => "Cursor",
    }
}

/// Where each agent keeps its MCP servers, under his home.
pub fn import_path(from: ImportFrom, home: &Path) -> PathBuf {
    match from {
        ImportFrom::Claude => home.join(".claude.json"),
        ImportFrom::Codex => home.join(".codex/config.toml"),
        ImportFrom::Cursor => home.join(".cursor/mcp.json"),
    }
}

/// The config's MCP servers as import's JSON input.
fn import_input(from: ImportFrom, home: &Path) -> Result<(PathBuf, Value), AddError> {
    let path = import_path(from, home);
    let text = std::fs::read_to_string(&path).map_err(|_| AddError::BadSource { why: format!("no {}", shown(&path, home)) })?;
    let bad = || AddError::BadSource { why: format!("{} does not parse", shown(&path, home)) };
    let v = match from {
        ImportFrom::Codex => {
            let t: toml::Table = toml::from_str(&text).map_err(|_| bad())?;
            let servers = t.get("mcp_servers").cloned().unwrap_or(toml::Value::Table(Default::default()));
            json!({ "mcp_servers": serde_json::to_value(servers).map_err(|_| bad())? })
        }
        _ => {
            let v: Value = serde_json::from_str(&text).map_err(|_| bad())?;
            json!({ "mcpServers": v.get("mcpServers").cloned().unwrap_or(json!({})) })
        }
    };
    Ok((path, v))
}

/// An agent's MCP servers found on his Mac (the import item: "found on
/// this Mac: Claude Code 4, Codex 2, Cursor 1").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub from: ImportFrom,
    pub path: PathBuf,
    /// the servers an import would keep
    pub servers: Vec<String>,
    /// the plugin that already holds this import (none: not imported)
    pub added: Option<String>,
}

/// Claude Code's, Codex's and Cursor's MCP configs on disk, with a server
/// bise can run (reads files only).
pub fn found(ctx: &Ctx) -> Vec<Found> {
    [ImportFrom::Claude, ImportFrom::Codex, ImportFrom::Cursor]
        .into_iter()
        .filter_map(|from| {
            let (path, input) = import_input(from, &ctx.home).ok()?;
            let plan = import::plan(&input).ok()?;
            if plan.servers.is_empty() {
                return None;
            }
            let name = default_import_name(from);
            let added = description(&ctx.user_root.join(name)).filter(|d| d.starts_with(import::MARK)).map(|_| name.to_string());
            Some(Found { from, path, servers: plan.servers.keys().cloned().collect(), added })
        })
        .collect()
}

// ---- make one with an agent ------------------------------------------

/// The brief both apps send to main for "make a plugin with an agent"
/// (the "start an agent" path, no new message kind): the agent writes it
/// with the plugin-creator skill, tests it, and hands him its folder,
/// which he adds through the same preview and click.
pub fn agent_brief(words: &str) -> String {
    format!(
        "make a plugin for him (Agent Plugins 1.0, the plugin-creator skill): {}. \
build and test it in a folder of your own, never in ~/.agents/plugins. \
when it works, give him its folder: he adds it with /plugins add, a plugin from a folder, \
which shows him what it runs before his click. never install it yourself.",
        words.trim()
    )
}

// ---- git ---------------------------------------------------------------

/// A git source as git takes it: `https://`, `ssh://`, `git@host:path`,
/// `file://`, a local path, or GitHub's `owner/repo`.
pub fn git_url(url: &str) -> Result<String, AddError> {
    let u = url.trim();
    let bad = || AddError::BadSource { why: format!("{u:?} is not a git URL") };
    if u.is_empty() || u.starts_with('-') || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(bad());
    }
    if ["https://", "http://", "ssh://", "file://", "git://"].iter().any(|s| u.starts_with(s)) || u.starts_with('/') {
        return Ok(u.to_string());
    }
    if let Some((user_host, path)) = u.split_once(':') {
        if user_host.contains('@') && !user_host.contains('/') && !path.is_empty() {
            return Ok(u.to_string());
        }
        return Err(bad());
    }
    let parts: Vec<&str> = u.trim_start_matches("github.com/").split('/').collect();
    let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && p != "." && p != "..";
    match parts.as_slice() {
        [o, r] if ok(o) && ok(r) => Ok(format!("https://github.com/{o}/{}.git", r.trim_end_matches(".git"))),
        _ => Err(bad()),
    }
}

/// A URL as he reads it: no scheme, no user or password, no `.git`.
pub fn url_shown(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let rest = match rest.split_once('/') {
        Some((hp, p)) => format!("{}/{}", hp.rsplit_once('@').map(|(_, h)| h).unwrap_or(hp), p),
        None => rest.rsplit_once('@').map(|(_, h)| h).unwrap_or(rest).to_string(),
    };
    rest.split(['?', '#']).next().unwrap_or("").trim_end_matches('/').trim_end_matches(".git").to_string()
}

/// `git clone --depth 1` of `url` into `dest`: no hook, no submodule, no
/// `ext::` transport, no prompt, killed after `timeout`. The add menu
/// and the marketplaces both clone through it.
pub fn git_clone(url: &str, dest: &Path, timeout: Duration) -> Result<(), AddError> {
    use std::process::{Command, Stdio};
    let mut child = Command::new("git")
        .args(["-c", "core.hooksPath=/dev/null", "-c", "protocol.ext.allow=never", "clone", "--depth", "1", "--no-recurse-submodules", "--quiet", "--"])
        .arg(url)
        .arg(dest)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AddError::Git { why: format!("git: {e}") })?;
    let start = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(dest);
                return Err(AddError::Git { why: format!("git clone took over {} s", timeout.as_secs()) });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(AddError::Git { why: e.to_string() }),
        }
    };
    if status.success() {
        return Ok(());
    }
    let mut err = String::new();
    if let Some(mut s) = child.stderr.take() {
        let _ = std::io::Read::read_to_string(&mut s, &mut err);
    }
    let _ = std::fs::remove_dir_all(dest);
    // git's last line, the URL as he reads it (no password)
    let last = err.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("git clone failed").replace(url, &url_shown(url));
    Err(AddError::Git { why: last.trim_start_matches("fatal: ").to_string() })
}

// ---- bise plugins add ---------------------------------------------------

/// The preview as lines for a terminal: what it runs first.
pub fn preview_lines(p: &Preview) -> Vec<String> {
    let mut out = vec![format!("{} · from {}", p.name, p.from)];
    if let Some(r) = &p.replaces {
        out.push(format!("replaces your {r}"));
    }
    if p.runs.is_empty() {
        out.push("runs nothing on your Mac".into());
    }
    for r in &p.runs {
        out.push(match r.transport {
            RunTransport::Stdio => format!("runs  {}: {}", r.server, shell_line(&r.argv)),
            t => format!("calls {}: {} {}", r.server, match t {
                RunTransport::Sse => "sse",
                _ => "http",
            }, r.host.as_deref().unwrap_or("")),
        });
    }
    if !p.skills.is_empty() {
        out.push(format!("skills {}", p.skills.join(", ")));
    }
    if !p.secrets.is_empty() {
        out.push(format!("kept in bise's secrets: {}", p.secrets.join(", ")));
    }
    if !p.dropped.is_empty() {
        out.push(format!("left out: {}", p.dropped.join(", ")));
    }
    out.extend(p.notes.iter().cloned());
    out
}

/// An argv as he could type it (quoted where a shell needs it).
fn shell_line(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if !a.is_empty() && a.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:@%+,".contains(c)) {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub const USAGE: &str = "usage:
  bise plugins add FOLDER|GIT-URL [--subdir DIR] [--yes]
  bise plugins add --mcp NAME --command 'CMD ARGS' | --url URL [--secret NAME]... [--yes]
      (each --secret's value on stdin, one NAME=value line each: never in an argument)
  bise plugins add --import claude|codex|cursor [--name NAME] [--yes]
  bise plugins add --found     the MCP configs an import would read
Without --yes it shows what the plugin will run and adds nothing.";

/// `bise plugins add ...`; the exit code.
pub fn main(args: &[String]) -> i32 {
    use bise_home::style::Style;
    let (out, err) = (Style::stdout(), Style::stderr());
    let val = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let has = |k: &str| args.iter().any(|a| a == k);
    let ctx = Ctx::standard();
    if has("--help") || has("-h") {
        println!("{USAGE}");
        return 0;
    }
    if has("--found") {
        let f = found(&ctx);
        if f.is_empty() {
            println!("{}", out.dim("no Claude Code, Codex or Cursor MCP server on this Mac"));
        }
        for x in f {
            let added = x.added.map(|a| format!(" · imported as {a}")).unwrap_or_default();
            println!("{} ({}): {}{}", import_title(x.from), shown(&x.path, &ctx.home), x.servers.join(", "), added);
        }
        return 0;
    }
    let req = if let Some(name) = val("--mcp") {
        let names: Vec<String> = args.iter().enumerate().filter(|(_, a)| *a == "--secret").filter_map(|(i, _)| args.get(i + 1).cloned()).collect();
        let mut secrets = Vec::new();
        if !names.is_empty() {
            let mut text = String::new();
            let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text);
            for n in &names {
                match text.lines().find_map(|l| l.strip_prefix(&format!("{n}="))) {
                    Some(v) => secrets.push((n.clone(), v.to_string())),
                    None => {
                        eprintln!("{}", err.fail(&format!("no {n}=... line on stdin")));
                        return 2;
                    }
                }
            }
        }
        Request::Mcp { name, command: val("--command"), url: val("--url"), secrets }
    } else if let Some(from) = val("--import") {
        let from = match from.as_str() {
            "claude" => ImportFrom::Claude,
            "codex" => ImportFrom::Codex,
            "cursor" => ImportFrom::Cursor,
            _ => {
                eprintln!("{USAGE}");
                return 2;
            }
        };
        Request::Import { from, name: val("--name") }
    } else {
        let skip = ["--subdir"];
        let pos: Vec<&String> = args.iter().enumerate().filter(|(i, a)| !a.starts_with("--") && !(*i > 0 && skip.contains(&args[i - 1].as_str()))).map(|(_, a)| a).collect();
        let [src] = pos.as_slice() else {
            eprintln!("{USAGE}");
            return 2;
        };
        let p = PathBuf::from(src.as_str());
        if p.is_dir() && val("--subdir").is_none() {
            Request::Folder { path: p.canonicalize().unwrap_or(p) }
        } else {
            Request::Git { url: src.to_string(), subdir: val("--subdir") }
        }
    };
    let staged = match stage(&req, &ctx) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", err.fail(&e.to_string()));
            return 1;
        }
    };
    for l in preview_lines(&staged.preview) {
        println!("{}", out.dim(&l));
    }
    if !has("--yes") {
        discard(staged);
        println!("{}", out.ask("nothing added: run it again with --yes to add it"));
        return 0;
    }
    match install(staged, &ctx) {
        Ok(dir) => {
            println!("{}", out.ok(&format!("added {}", shown(&dir, &ctx.home))));
            println!("{}", out.dim("your agents get it at their next turn."));
            0
        }
        Err(e) => {
            eprintln!("{}", err.fail(&e.to_string()));
            1
        }
    }
}

fn shown(p: &Path, home: &Path) -> String {
    match p.strip_prefix(home) {
        Ok(r) => format!("~/{}", r.display()),
        Err(_) => p.display().to_string(),
    }
}

fn shown_rel(p: &Path, base: &Path) -> String {
    p.strip_prefix(base).unwrap_or(p).display().to_string()
}

#[cfg(test)]
pub(crate) mod tests;
