//! Discovery, manifest validation, precedence and components. Pure over
//! the file system: no process is started here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

pub const PLUGIN_SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";
pub const MCP_SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json";
pub const RESERVED: [&str; 6] = ["file_system", "process", "self", "skill", "subagent", "vibe"];

/// The user's home (`~/.agents/plugins` is under it).
pub fn home() -> PathBuf {
    bise_home::Home::from_env().user_home().to_path_buf()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// bise's own plugins, in the app root's `plugins/` (computer use)
    BuiltIn,
    User,
    Workspace,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::BuiltIn => "built-in",
            Scope::User => "user",
            Scope::Workspace => "workspace",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// the plugin is dropped
    Error,
    /// one component is dropped
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    /// the plugin name when known, else its folder
    pub plugin: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Loaded,
    Disabled,
    Shadowed,
    Invalid,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Loaded => "loaded",
            State::Disabled => "disabled",
            State::Shadowed => "shadowed",
            State::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Skill {
    /// `<namespace>:<name>`
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

/// A server's own limits in mcp.json, Codex's keys (`config.toml`'s
/// `[mcp_servers.<id>]`, also Vibe's): `startup_timeout_sec` (or
/// `startup_timeout_ms`), `tool_timeout_sec`, `enabled_tools`,
/// `disabled_tools`. None: the bridge's defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Limits {
    /// start, handshake and the first tools/list
    pub startup_timeout: Option<std::time::Duration>,
    /// one tools/call (and any request an agent sends)
    pub tool_timeout: Option<std::time::Duration>,
    /// only these tools (the server's own names); None: all
    pub enabled_tools: Option<Vec<String>>,
    /// never these tools, even when enabled
    pub disabled_tools: Vec<String>,
}

impl Limits {
    /// Codex's rule: in `enabled_tools` when it is set, and not in
    /// `disabled_tools`.
    pub fn allows(&self, tool: &str) -> bool {
        self.enabled_tools.as_ref().is_none_or(|e| e.iter().any(|t| t == tool)) && !self.disabled_tools.iter().any(|t| t == tool)
    }

    /// The keys this struct reads, and `"enabled"` (`false`: the server
    /// is left out, Codex's switch; read by [`load_mcp`]).
    pub const KEYS: [&'static str; 6] = ["startup_timeout_sec", "startup_timeout_ms", "tool_timeout_sec", "enabled_tools", "disabled_tools", "enabled"];

    pub fn parse(o: &Map<String, Value>) -> Result<Limits, String> {
        let secs = |k: &str| -> Result<Option<std::time::Duration>, String> {
            match o.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(v) => v
                    .as_f64()
                    .filter(|s| *s > 0.0 && s.is_finite())
                    .and_then(|s| std::time::Duration::try_from_secs_f64(s).ok())
                    .map(Some)
                    .ok_or(format!("{:?} must be a number of seconds above 0", k)),
            }
        };
        let names = |k: &str| -> Result<Option<Vec<String>>, String> {
            match o.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|s| s.as_str().map(String::from))
                    .collect::<Option<Vec<_>>>()
                    .map(Some)
                    .ok_or(format!("{:?} must be an array of tool names", k)),
                Some(_) => Err(format!("{:?} must be an array of tool names", k)),
            }
        };
        let startup = match (secs("startup_timeout_sec")?, o.get("startup_timeout_ms")) {
            (Some(d), _) => Some(d),
            (None, None | Some(Value::Null)) => None,
            (None, Some(v)) => Some(std::time::Duration::from_millis(
                v.as_u64().filter(|ms| *ms > 0).ok_or("\"startup_timeout_ms\" must be a whole number of milliseconds above 0")?,
            )),
        };
        Ok(Limits {
            startup_timeout: startup,
            tool_timeout: secs("tool_timeout_sec")?,
            enabled_tools: names("enabled_tools")?,
            disabled_tools: names("disabled_tools")?.unwrap_or_default(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StdioServer {
    pub id: String,
    /// resolved: a bare executable name or an absolute path
    pub command: String,
    pub args: Vec<String>,
    /// the declared env, placeholders expanded (PLUGIN_ROOT/DATA are
    /// added at spawn time)
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
    pub limits: Limits,
}

/// A remote server's transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    /// Streamable HTTP (MCP 2025-03-26+): `"type": "http"` (Claude
    /// Code, Cursor) or `"streamable-http"` (Vibe, the plugin schema)
    Streamable,
    /// the legacy HTTP+SSE transport (MCP 2024-11-05): `"type": "sse"`
    Sse,
}

impl Transport {
    pub fn as_str(self) -> &'static str {
        match self {
            Transport::Streamable => "http",
            Transport::Sse => "sse",
        }
    }
}

/// A remote MCP server (`"type": "http" | "streamable-http" | "sse"`).
/// `${PLUGIN_ROOT}`/`${PLUGIN_DATA}` are expanded here; `${VAR}` and
/// `${VAR:-default}` stay until the bridge connects ([`expand_env`]), so
/// a token never sits in a resolution, a report or a listing.
#[derive(Clone, PartialEq)]
pub struct HttpServer {
    pub id: String,
    pub transport: Transport,
    pub url: String,
    /// never printed: the values may hold tokens
    pub headers: Vec<(String, String)>,
    /// `"oauth"`: a registered client for servers without dynamic
    /// registration (Claude Code's `clientId`, `callbackPort`)
    pub oauth: Option<crate::oauth::Config>,
    pub limits: Limits,
}

impl std::fmt::Debug for HttpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(k, _)| k.as_str()).collect();
        write!(f, "HttpServer {{ id: {:?}, transport: {:?}, host: {:?}, headers: {:?} }}", self.id, self.transport, self.host(), names)
    }
}

impl HttpServer {
    /// No `Authorization` header in mcp.json: bise's login may supply one.
    pub fn may_login(&self) -> bool {
        !self.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("authorization"))
    }

    /// The URL's host (and port), for listings: the path may hold a key.
    pub fn host(&self) -> String {
        let rest = self.url.split_once("://").map(|(_, r)| r).unwrap_or(&self.url);
        let hp = rest.split(['/', '?', '#']).next().unwrap_or("");
        hp.rsplit_once('@').map(|(_, h)| h).unwrap_or(hp).to_string()
    }
}

/// `${VAR}` and `${VAR:-default}` replaced from `lookup` (Claude Code's
/// .mcp.json syntax). A variable that is unset (or empty, with no
/// default) is an error naming it. `$VAR` without braces and an
/// unclosed `${` stay as they are.
pub fn expand_env(s: &str, lookup: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("${") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let Some(j) = after.find('}') else {
            out.push_str(&rest[i..]);
            return Ok(out);
        };
        let inner = &after[..j];
        let (name, default) = match inner.split_once(":-") {
            Some((n, d)) => (n, Some(d)),
            None => (inner, None),
        };
        let ok_name = !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.starts_with(|c: char| c.is_ascii_digit());
        if !ok_name {
            out.push_str("${");
            rest = after;
            continue;
        }
        match (lookup(name).filter(|v| !v.is_empty() || default.is_none()), default) {
            (Some(v), _) => out.push_str(&v),
            (None, Some(d)) => out.push_str(d),
            (None, None) => return Err(format!("${{{}}} is not set", name)),
        }
        rest = &after[j + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plugin {
    /// the manifest name; the folder name when the manifest is invalid
    pub name: String,
    pub namespace: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub scope: Scope,
    pub root: PathBuf,
    pub data_root: PathBuf,
    pub state: State,
    pub skills: Vec<Skill>,
    pub servers: Vec<StdioServer>,
    /// remote servers (Streamable HTTP, SSE)
    pub remotes: Vec<HttpServer>,
    /// components present on disk but not supported yet
    pub unsupported: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Resolution {
    pub plugins: Vec<Plugin>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Resolution {
    pub fn loaded(&self) -> impl Iterator<Item = &Plugin> {
        self.plugins.iter().filter(|p| p.state == State::Loaded)
    }
}

#[derive(Clone, Debug)]
pub struct Roots {
    /// the app root's `plugins/` (scope `built-in`): always discovered,
    /// shadowed by a user or workspace plugin of the same name
    pub builtin: Option<PathBuf>,
    pub user: Option<PathBuf>,
    pub workspace: Option<PathBuf>,
    /// where `${PLUGIN_DATA}` roots live (one folder per plugin name)
    pub data: PathBuf,
    pub disabled: Vec<String>,
    /// the opt-in plugins turned on (a manifest with
    /// `"extensions": {"dev.bise": {"default": "off"}}` loads only when
    /// its name is here)
    pub enabled: Vec<String>,
}

impl Roots {
    /// The standard roots: the built-in one ([`builtin_root`]),
    /// `$BEND_PLUGINS_HOME` or `~/.agents/plugins`, and
    /// `<workspace>/.agents/plugins`.
    pub fn standard(workspace: Option<&Path>) -> Roots {
        let user = match std::env::var("BEND_PLUGINS_HOME") {
            Ok(p) if !p.is_empty() => PathBuf::from(p),
            _ => home().join(".agents").join("plugins"),
        };
        let data = bise_home::Home::from_env().plugin_data_dir();
        Roots {
            builtin: builtin_root(),
            user: Some(user),
            workspace: workspace.map(|w| w.join(".agents").join("plugins")),
            data,
            disabled: crate::state::disabled(&crate::state::state_path()),
            enabled: crate::state::enabled(&crate::state::state_path()),
        }
    }
}

/// What a session's plugins depend on, as one number: the enable state and,
/// in each root, every plugin folder's name and the size and mtime of its
/// `plugin.json`, `mcp.json` and `skills/*/SKILL.md`. Cheap (stats only):
/// the hub compares it every 2 s and relaunches a REPL whose plugins
/// changed at its next idle (an installed, removed, enabled or edited
/// plugin), same session.
pub fn fingerprint(roots: &Roots) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    roots.disabled.hash(&mut h);
    roots.enabled.hash(&mut h);
    let stat = |p: &Path, h: &mut std::collections::hash_map::DefaultHasher| {
        if let Ok(m) = std::fs::metadata(p) {
            m.len().hash(h);
            m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).hash(h);
        }
    };
    let sorted = |d: &Path| -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).collect();
        v.sort();
        v
    };
    for root in [&roots.builtin, &roots.user, &roots.workspace].into_iter().flatten() {
        root.hash(&mut h);
        for p in sorted(root) {
            if !p.is_dir() {
                continue;
            }
            p.hash(&mut h);
            stat(&p.join("plugin.json"), &mut h);
            stat(&p.join("mcp.json"), &mut h);
            for s in sorted(&p.join("skills")) {
                s.hash(&mut h);
                stat(&s.join("SKILL.md"), &mut h);
            }
        }
    }
    h.finish()
}

/// The built-in root: `plugins/` in bise's app root (`$BISE_APP_ROOT`;
/// else the executable's folder when it is a version dir or a bundle, i.e.
/// holds `VERSION`; else, in dev, the source tree the executable was built
/// in or this crate's tree). None when there is none.
pub fn builtin_root() -> Option<PathBuf> {
    let has = |root: &Path| root.join("plugins").is_dir();
    if let Some(r) = std::env::var_os("BISE_APP_ROOT").filter(|v| !v.is_empty()) {
        let r = PathBuf::from(r);
        return has(&r).then(|| r.join("plugins"));
    }
    let exe = std::env::current_exe().ok().map(|e| e.canonicalize().unwrap_or(e));
    if let Some(dir) = exe.as_deref().and_then(Path::parent) {
        if dir.join("VERSION").exists() {
            return has(dir).then(|| dir.join("plugins"));
        }
        // rust/target/<profile>/bise -> the repo
        for up in dir.ancestors().take(4) {
            if up.join("rust/Cargo.toml").exists() && has(up) {
                return Some(up.join("plugins"));
            }
        }
    }
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    (cfg!(debug_assertions) && has(&tree)).then(|| tree.canonicalize().unwrap_or(tree).join("plugins"))
}

/// The bise a built-in plugin's `"command": "bise"` runs: the harness that
/// resolves it (the bridge is `bise plugins serve`), not a `bise` on PATH
/// (another version, or none).
fn bise_exe() -> String {
    if let Some(b) = std::env::var_os("BEND_HARNESS_BIN").filter(|v| !v.is_empty()) {
        return PathBuf::from(b).to_string_lossy().into_owned();
    }
    let exe = std::env::current_exe().ok();
    let named = exe.as_ref().and_then(|e| e.file_name()).is_some_and(|n| n == "bise" || n == "bend-harness");
    match exe.filter(|_| named) {
        Some(e) => e.to_string_lossy().into_owned(),
        None => "bise".into(),
    }
}

fn diag(code: &'static str, severity: Severity, plugin: &str, message: String) -> Diagnostic {
    Diagnostic {
        code,
        severity,
        plugin: plugin.to_string(),
        message,
    }
}

/// Any character outside `[A-Za-z0-9_$]` becomes `_`; a leading digit
/// gets a `_` in front; empty becomes `_`.
pub fn identifier(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '$' { c } else { '_' })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

pub fn valid_name(name: &str) -> bool {
    let b = name.as_bytes();
    let ok_char = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-';
    let alnum = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    !b.is_empty()
        && b.len() <= 64
        && b.iter().all(|&c| ok_char(c))
        && alnum(b[0])
        && alnum(b[b.len() - 1])
        && !name.contains("--")
        && !name.contains("..")
}

#[derive(Debug, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    /// the extensions bise does not know (reported, ignored)
    pub extensions: Vec<String>,
    /// `"extensions": {"dev.bise": {"default": "off"}}`: opt-in, loaded
    /// only once enabled (computer use: /computer-use turns it on)
    pub default_off: bool,
}

/// bise's own manifest extension.
pub const BISE_EXTENSION: &str = "dev.bise";

fn opt_string(o: &Map<String, Value>, k: &str) -> Result<Option<String>, String> {
    match o.get(k) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("/{} must be a string", k)),
    }
}

/// The closed 1.0.0 manifest schema.
pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {}", e))?;
    let o = v.as_object().ok_or("the manifest must be a JSON object")?;
    for k in o.keys() {
        let known = [
            "$schema", "name", "version", "description", "author", "homepage", "repository",
            "license", "keywords", "extensions",
        ];
        if !known.contains(&k.as_str()) {
            return Err(format!("unknown field /{} (the schema is closed)", k));
        }
    }
    match o.get("$schema") {
        Some(Value::String(s)) if s == PLUGIN_SCHEMA => {}
        Some(_) => return Err(format!("/$schema must be \"{}\"", PLUGIN_SCHEMA)),
        None => return Err("missing /$schema".into()),
    }
    let name = match o.get("name") {
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err("/name must be a string".into()),
        None => return Err("missing /name".into()),
    };
    if !valid_name(&name) {
        return Err(format!(
            "/name {:?} must be 1-64 chars of [a-z0-9.-], start and end alphanumeric, no -- or ..",
            name
        ));
    }
    for k in ["homepage", "repository", "license"] {
        opt_string(o, k)?;
    }
    if let Some(a) = o.get("author") {
        let a = a.as_object().ok_or("/author must be an object")?;
        for (k, v) in a {
            if !["name", "email", "url"].contains(&k.as_str()) {
                return Err(format!("unknown field /author/{}", k));
            }
            if !v.is_string() {
                return Err(format!("/author/{} must be a string", k));
            }
        }
    }
    if let Some(kw) = o.get("keywords") {
        let ok = kw.as_array().is_some_and(|a| a.iter().all(Value::is_string));
        if !ok {
            return Err("/keywords must be an array of strings".into());
        }
    }
    let mut extensions = Vec::new();
    let mut default_off = false;
    if let Some(ex) = o.get("extensions") {
        let ex = ex.as_object().ok_or("/extensions must be an object")?;
        for (k, v) in ex {
            let Some(v) = v.as_object() else {
                return Err(format!("/extensions/{} must be an object", k));
            };
            if k == BISE_EXTENSION {
                for (f, fv) in v {
                    match (f.as_str(), fv.as_str()) {
                        ("default", Some("off")) => default_off = true,
                        ("default", Some("on")) => {}
                        ("default", _) => return Err(format!("/extensions/{}/default must be \"on\" or \"off\"", k)),
                        _ => return Err(format!("unknown field /extensions/{}/{}", k, f)),
                    }
                }
            } else {
                extensions.push(k.clone());
            }
        }
    }
    Ok(Manifest {
        name,
        version: opt_string(o, "version")?,
        description: opt_string(o, "description")?,
        extensions,
        default_off,
    })
}

/// `name:` and `description:` of a SKILL.md YAML frontmatter. Plain,
/// quoted and folded (`>` / `|`) scalars.
pub fn frontmatter(text: &str) -> Result<(String, String), String> {
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Err("no YAML frontmatter (the file must start with ---)".into());
    }
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut closed = false;
    for line in lines {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(k) = &current {
                let e = fields.entry(k.clone()).or_default();
                if !e.is_empty() {
                    e.push(' ');
                }
                e.push_str(line.trim());
            }
            continue;
        }
        current = None;
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_string();
            let v = v.trim();
            let v = if v == ">" || v == "|" || v == ">-" || v == "|-" { "" } else { v };
            let v = v
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(v);
            fields.insert(k.clone(), v.to_string());
            current = Some(k);
        }
    }
    if !closed {
        return Err("the frontmatter is not closed by ---".into());
    }
    let get = |k: &str| fields.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let name = get("name").ok_or("the frontmatter has no name")?;
    let desc = get("description").ok_or("the frontmatter has no description")?;
    Ok((name, desc))
}

fn inside(root: &Path, p: &Path) -> bool {
    match (root.canonicalize(), p.canonicalize()) {
        (Ok(r), Ok(t)) => t.starts_with(r),
        _ => false,
    }
}

fn load_skills(p: &mut Plugin, out: &mut Vec<Diagnostic>) {
    let dir = p.root.join("skills");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for d in entries {
        let file = d.join("SKILL.md");
        if !d.is_dir() || !file.exists() {
            continue;
        }
        let label = d.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if !inside(&p.root, &file) {
            out.push(diag("plugin.path.outside_root", Severity::Warning, &p.name,
                format!("skills/{}/SKILL.md resolves outside the plugin root; skipped", label)));
            continue;
        }
        let parsed = std::fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|t| frontmatter(&t));
        match parsed {
            Ok((name, desc)) => {
                let full = format!("{}:{}", p.namespace, name);
                if p.skills.iter().any(|s| s.name == full) {
                    out.push(diag("plugin.skill.invalid", Severity::Warning, &p.name,
                        format!("skills/{}: duplicate skill name {:?}; skipped", label, name)));
                    continue;
                }
                p.skills.push(Skill {
                    name: full,
                    description: desc,
                    path: file.canonicalize().unwrap_or(file),
                });
            }
            Err(e) => out.push(diag("plugin.skill.invalid", Severity::Warning, &p.name,
                format!("skills/{}/SKILL.md: {}; skipped", label, e))),
        }
    }
}

fn expand(v: &str, root: &Path, data: &Path) -> String {
    v.replace("${PLUGIN_ROOT}", &root.to_string_lossy())
        .replace("${PLUGIN_DATA}", &data.to_string_lossy())
}

/// Lexical normalization (`.` and `..` segments) for paths that may
/// not exist yet.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c.as_os_str()),
        }
    }
    out
}

fn contained(base: &Path, p: &Path) -> bool {
    let base = base.canonicalize().unwrap_or_else(|_| normalize(base));
    let target = p.canonicalize().unwrap_or_else(|_| normalize(p));
    target.starts_with(base)
}

/// One server of mcp.json.
enum Parsed {
    Stdio(StdioServer),
    Http(HttpServer),
}

/// A remote server: `url` (http or https, `${VAR}` allowed) and
/// optional string `headers`.
fn parse_http(id: &str, transport: Transport, o: &Map<String, Value>, root: &Path, data: &Path) -> Result<HttpServer, String> {
    for k in o.keys() {
        if !["type", "url", "headers", "oauth"].contains(&k.as_str()) && !Limits::KEYS.contains(&k.as_str()) {
            return Err(format!("unknown field {:?}", k));
        }
    }
    let raw = o.get("url").and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or("\"url\" must be a non-empty string")?;
    let url = expand(raw, root, data);
    let checked = if url.contains("${") { None } else { Some(crate::http::Url::parse(&url).map_err(|e| format!("\"url\": {}", e))?) };
    if checked.is_none() && !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("${")) {
        return Err("\"url\" must start with http:// or https://".into());
    }
    let mut headers = Vec::new();
    if let Some(h) = o.get("headers") {
        let h = h.as_object().ok_or("\"headers\" must be an object")?;
        for (k, v) in h {
            if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c)) {
                return Err(format!("header name {:?} is not a valid HTTP header name", k));
            }
            let v = v.as_str().ok_or(format!("header {} must be a string", k))?;
            if v.contains(['\r', '\n']) {
                return Err(format!("header {} has a line break", k));
            }
            headers.push((k.clone(), expand(v, root, data)));
        }
    }
    let oauth = o.get("oauth").map(crate::oauth::Config::parse).transpose()?;
    let limits = Limits::parse(o)?;
    Ok(HttpServer { id: id.to_string(), transport, url, headers, oauth, limits })
}

fn parse_server(id: &str, v: &Value, root: &Path, data: &Path) -> Result<Parsed, String> {
    let o = v.as_object().ok_or("must be an object")?;
    // no "type": stdio with a command, http with a url (Claude Code's
    // and Cursor's files leave it out)
    let ty = match o.get("type") {
        Some(t) => t.as_str().ok_or("\"type\" must be a string")?,
        None if o.contains_key("url") && !o.contains_key("command") => "http",
        None => "stdio",
    };
    match ty {
        "stdio" => parse_stdio(id, o, root, data).map(Parsed::Stdio),
        "http" | "streamable-http" | "streamable_http" | "streamableHttp" => {
            parse_http(id, Transport::Streamable, o, root, data).map(Parsed::Http)
        }
        "sse" => parse_http(id, Transport::Sse, o, root, data).map(Parsed::Http),
        t => Err(format!("unsupported:type {:?} (stdio, http, streamable-http or sse)", t)),
    }
}

fn parse_stdio(id: &str, o: &Map<String, Value>, root: &Path, data: &Path) -> Result<StdioServer, String> {
    for k in o.keys() {
        if !["type", "command", "args", "env", "cwd"].contains(&k.as_str()) && !Limits::KEYS.contains(&k.as_str()) {
            return Err(format!("unknown field {:?}", k));
        }
    }
    let raw = o
        .get("command")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("\"command\" must be a non-empty string")?;
    let command = expand(raw, root, data);
    let command = if let Some(rel) = raw.strip_prefix("./") {
        let p = root.join(rel);
        if !contained(root, &p) {
            return Err("\"command\" resolves outside the plugin root".into());
        }
        p.to_string_lossy().to_string()
    } else if raw.starts_with("${PLUGIN_ROOT}/") {
        if !contained(root, Path::new(&command)) {
            return Err("\"command\" resolves outside the plugin root".into());
        }
        command
    } else if raw.contains('/') || raw == "." || raw == ".." {
        return Err("\"command\" must be a bare executable or start with ./".into());
    } else {
        command
    };
    let args = match o.get("args") {
        None => Vec::new(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|s| s.as_str().map(|s| expand(s, root, data)))
            .collect::<Option<Vec<_>>>()
            .ok_or("\"args\" must be an array of strings")?,
        Some(_) => return Err("\"args\" must be an array of strings".into()),
    };
    let mut env = Vec::new();
    if let Some(e) = o.get("env") {
        let e = e.as_object().ok_or("\"env\" must be an object")?;
        for (k, v) in e {
            if k == "PLUGIN_ROOT" || k == "PLUGIN_DATA" {
                return Err(format!("\"env\" must not set {} (reserved)", k));
            }
            let v = v.as_str().ok_or(format!("env {} must be a string", k))?;
            env.push((k.clone(), expand(v, root, data)));
        }
    }
    let cwd = match o.get("cwd") {
        None => root.to_path_buf(),
        Some(Value::String(c)) => {
            let (base, p) = if let Some(rel) = c.strip_prefix("./") {
                (root, root.join(rel))
            } else if c == "${PLUGIN_ROOT}" || c.starts_with("${PLUGIN_ROOT}/") {
                (root, PathBuf::from(expand(c, root, data)))
            } else if c == "${PLUGIN_DATA}" || c.starts_with("${PLUGIN_DATA}/") {
                (data, PathBuf::from(expand(c, root, data)))
            } else {
                return Err("\"cwd\" must start with ./, ${PLUGIN_ROOT} or ${PLUGIN_DATA}".into());
            };
            if !contained(base, &p) {
                return Err("\"cwd\" resolves outside its root".into());
            }
            normalize(&p)
        }
        Some(_) => return Err("\"cwd\" must be a string".into()),
    };
    Ok(StdioServer {
        id: id.to_string(),
        command,
        args,
        env,
        cwd,
        limits: Limits::parse(o)?,
    })
}

fn load_mcp(p: &mut Plugin, out: &mut Vec<Diagnostic>) {
    let file = p.root.join("mcp.json");
    if !file.exists() {
        return;
    }
    if !inside(&p.root, &file) {
        out.push(diag("plugin.path.outside_root", Severity::Warning, &p.name,
            "mcp.json resolves outside the plugin root; no MCP server loads".into()));
        return;
    }
    let bad = |m: String| diag("plugin.mcp.invalid", Severity::Warning, &p.name,
        format!("mcp.json: {}; no MCP server loads", m));
    let v: Value = match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| {
        serde_json::from_str(&t).map_err(|e| format!("not JSON: {}", e))
    }) {
        Ok(v) => v,
        Err(e) => return out.push(bad(e)),
    };
    let Some(o) = v.as_object() else {
        return out.push(bad("must be a JSON object".into()));
    };
    if let Some(k) = o.keys().find(|k| *k != "$schema" && *k != "mcpServers") {
        return out.push(bad(format!("unknown field /{} (the schema is closed)", k)));
    }
    // no $schema: a .mcp.json copied from Claude Code, Cursor or Vibe
    // works as is; a different one is still an error
    if o.get("$schema").is_some_and(|s| s.as_str() != Some(MCP_SCHEMA)) {
        return out.push(bad(format!("/$schema must be \"{}\"", MCP_SCHEMA)));
    }
    let Some(servers) = o.get("mcpServers").and_then(Value::as_object) else {
        return out.push(bad("/mcpServers must be an object".into()));
    };
    let data = p.data_root.clone();
    for (id, sv) in servers {
        match sv.get("enabled") {
            None | Some(Value::Bool(true)) => {}
            Some(Value::Bool(false)) => {
                out.push(diag("plugin.mcp.server_off", Severity::Info, &p.name,
                    format!("mcp.json server {:?} is off (\"enabled\": false)", id)));
                continue;
            }
            Some(_) => {
                out.push(diag("plugin.mcp.server_invalid", Severity::Warning, &p.name,
                    format!("mcp.json server {:?}: \"enabled\" must be true or false; skipped", id)));
                continue;
            }
        }
        match parse_server(id, sv, &p.root, &data) {
            Ok(Parsed::Stdio(s)) => p.servers.push(s),
            Ok(Parsed::Http(s)) => p.remotes.push(s),
            Err(e) => match e.strip_prefix("unsupported:") {
                Some(why) => {
                    p.unsupported.push(format!("mcp server {}", id));
                    out.push(diag("plugin.component.unsupported", Severity::Warning, &p.name,
                        format!("mcp.json server {:?}: {}; skipped", id, why)));
                }
                None => out.push(diag("plugin.mcp.server_invalid", Severity::Warning, &p.name,
                    format!("mcp.json server {:?}: {}; skipped", id, e))),
            },
        }
    }
}

fn load_unsupported(p: &mut Plugin, extensions: &[String], out: &mut Vec<Diagnostic>) {
    let vibe = p.root.join("ai.mistral.vibe");
    let found = [
        (vibe.join("hooks.toml"), "hooks (ai.mistral.vibe/hooks.toml)"),
        (vibe.join("agents"), "agents (ai.mistral.vibe/agents)"),
        (vibe.join("knowledge"), "knowledge (ai.mistral.vibe/knowledge)"),
        (vibe.join("views"), "views (ai.mistral.vibe/views)"),
        (p.root.join("connectors.json"), "connectors (connectors.json)"),
        (p.root.join("libraries.json"), "libraries (libraries.json)"),
    ];
    for (path, what) in found {
        if path.exists() {
            p.unsupported.push(what.to_string());
            out.push(diag("plugin.component.unsupported", Severity::Info, &p.name,
                format!("{} is not supported yet; ignored", what)));
        }
    }
    for ext in extensions {
        p.unsupported.push(format!("extension {}", ext));
        out.push(diag("plugin.extension.unsupported", Severity::Info, &p.name,
            format!("manifest extension {:?} is not supported; ignored", ext)));
    }
}

struct Candidate {
    plugin: Plugin,
    extensions: Vec<String>,
    default_off: bool,
}

fn discover(root: &Path, scope: Scope, data: &Path, out: &mut Vec<Diagnostic>) -> Vec<Candidate> {
    let rd = match std::fs::read_dir(root) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            out.push(diag("plugin.discovery.root_unreadable", Severity::Warning,
                &root.to_string_lossy(), format!("{} root {}: {}", scope.as_str(), root.display(), e)));
            return Vec::new();
        }
    };
    let mut dirs: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
    dirs.sort();
    let mut found = Vec::new();
    for dir in dirs {
        let manifest = dir.join("plugin.json");
        if !manifest.exists() {
            continue;
        }
        let folder = dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let root = dir.canonicalize().unwrap_or(dir.clone());
        let mut p = Plugin {
            name: folder.clone(),
            namespace: identifier(&folder),
            version: None,
            description: None,
            scope,
            root,
            data_root: PathBuf::new(),
            state: State::Invalid,
            skills: Vec::new(),
            servers: Vec::new(),
            remotes: Vec::new(),
            unsupported: Vec::new(),
        };
        let parsed = std::fs::read_to_string(&manifest)
            .map_err(|e| format!("unreadable: {}", e))
            .and_then(|t| parse_manifest(&t));
        let mut extensions = Vec::new();
        let mut default_off = false;
        match parsed {
            Ok(m) => {
                p.namespace = identifier(&m.name);
                p.data_root = data.join(&m.name);
                p.name = m.name;
                p.version = m.version;
                p.description = m.description;
                p.state = State::Loaded;
                extensions = m.extensions;
                default_off = m.default_off;
            }
            Err(e) => out.push(diag("plugin.manifest.invalid", Severity::Error, &folder,
                format!("{}: {}", manifest.display(), e))),
        }
        found.push(Candidate { plugin: p, extensions, default_off });
    }
    found
}

/// Discover, validate and load every plugin under `roots`.
pub fn resolve(roots: &Roots) -> Resolution {
    let mut diags = Vec::new();
    let mut cands = Vec::new();
    for (root, scope) in [(&roots.builtin, Scope::BuiltIn), (&roots.user, Scope::User), (&roots.workspace, Scope::Workspace)] {
        if let Some(r) = root {
            cands.extend(discover(r, scope, &roots.data, &mut diags));
        }
    }
    // same name twice in one root: both dropped
    let mut dup: Vec<(Scope, String)> = Vec::new();
    for (i, a) in cands.iter().enumerate() {
        let p = &a.plugin;
        if p.state == State::Loaded
            && cands[..i].iter().any(|b| b.plugin.state == State::Loaded && b.plugin.scope == p.scope && b.plugin.name == p.name)
            && !dup.contains(&(p.scope, p.name.clone()))
        {
            dup.push((p.scope, p.name.clone()));
        }
    }
    for c in cands.iter_mut() {
        if c.plugin.state == State::Loaded && dup.contains(&(c.plugin.scope, c.plugin.name.clone())) {
            c.plugin.state = State::Invalid;
            diags.push(diag("plugin.name.collision", Severity::Error, &c.plugin.name,
                format!("two {} plugins are named {:?} ({}); both dropped", c.plugin.scope.as_str(),
                    c.plugin.name, c.plugin.root.display())));
        }
    }
    // workspace over user over built-in
    for (lower, higher) in [(Scope::User, Scope::Workspace), (Scope::BuiltIn, Scope::User), (Scope::BuiltIn, Scope::Workspace)] {
        let names: Vec<String> = cands
            .iter()
            .filter(|c| c.plugin.state == State::Loaded && c.plugin.scope == higher)
            .map(|c| c.plugin.name.clone())
            .collect();
        for c in cands.iter_mut() {
            if c.plugin.state == State::Loaded && c.plugin.scope == lower && names.contains(&c.plugin.name) {
                c.plugin.state = State::Shadowed;
                diags.push(diag("plugin.shadowed", Severity::Info, &c.plugin.name,
                    format!("the {} plugin {:?} shadows the {} one at {}", higher.as_str(), c.plugin.name,
                        lower.as_str(), c.plugin.root.display())));
            }
        }
    }
    // namespaces
    for c in cands.iter_mut() {
        if c.plugin.state == State::Loaded && RESERVED.contains(&c.plugin.namespace.as_str()) {
            c.plugin.state = State::Invalid;
            diags.push(diag("plugin.namespace.reserved", Severity::Error, &c.plugin.name,
                format!("the namespace {:?} is reserved", c.plugin.namespace)));
        }
    }
    let live: Vec<(String, String)> = cands
        .iter()
        .filter(|c| c.plugin.state == State::Loaded)
        .map(|c| (c.plugin.name.clone(), c.plugin.namespace.clone()))
        .collect();
    for c in cands.iter_mut() {
        let p = &c.plugin;
        if p.state == State::Loaded && live.iter().any(|(n, ns)| *ns == p.namespace && *n != p.name) {
            c.plugin.state = State::Invalid;
            diags.push(diag("plugin.namespace.collision", Severity::Error, &c.plugin.name,
                format!("another plugin also maps to the namespace {:?}; both dropped", c.plugin.namespace)));
        }
    }
    // enable state, then components
    for c in cands.iter_mut() {
        let off = roots.disabled.contains(&c.plugin.name) || (c.default_off && !roots.enabled.contains(&c.plugin.name));
        if c.plugin.state == State::Loaded && off {
            c.plugin.state = State::Disabled;
        }
        if c.plugin.state == State::Loaded {
            load_skills(&mut c.plugin, &mut diags);
            load_mcp(&mut c.plugin, &mut diags);
            if c.plugin.scope == Scope::BuiltIn {
                for s in c.plugin.servers.iter_mut().filter(|s| s.command == "bise") {
                    s.command = bise_exe();
                }
            }
            load_unsupported(&mut c.plugin, &c.extensions, &mut diags);
        }
    }
    Resolution {
        plugins: cands.into_iter().map(|c| c.plugin).collect(),
        diagnostics: diags,
    }
}

#[cfg(test)]
mod tests;
