//! Artifacts (docs/artifacts.md): what agents made for the user to look
//! at, in one list. Two sources, merged at read time:
//!
//! - `<state>/artifacts/<id>/meta.json`: what `sb artifact add` and the
//!   TUI's `/artifacts add` registered, with a copy of each version of a
//!   file (`<id>/v<n>/<name>`, 50 MB at most) so it survives the agent's
//!   folder;
//! - `<state>/pages/<id>/meta.json`: bise pages, read as they are (the
//!   page store's layout, written by the pages module): every page gets
//!   in by itself, with its versions and open notes.
//!
//! Plain functions over the state folder; the daemon's thread is the only
//! writer, so there is no lock.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The largest file (or folder, summed) bise keeps a copy of.
pub const MAX_COPY: u64 = 50 * 1024 * 1024;
/// A folder with more files than this is not copied.
const MAX_COPY_FILES: usize = 5000;

/// The kinds of the list (design: "what counts").
pub const KINDS: &[&str] = &[
    "page", "site", "doc", "sheet", "slides", "code", "image", "video", "sound", "pr", "release", "link", "folder",
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Version {
    pub v: u64,
    pub at_ms: u64,
    /// The absolute path or the link of this version.
    pub target: String,
    /// bise's copy, relative to the artifact's folder (`v2/plan.xlsx`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
    /// Why there is no copy of a file (`over 50 MB`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_copy: Option<String>,
    /// Size and newest mtime of a file or folder: `<bytes>:<mtime ms>`;
    /// empty for a link.
    #[serde(default)]
    pub sig: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub id: String,
    pub title: String,
    pub kind: String,
    /// The agent it belongs to (its name when added; a rename keeps the
    /// old name here, the daemon maps it).
    pub agent: String,
    /// Who added it: the agent, or `you` (`/artifacts add`).
    pub by: String,
    pub created_ms: u64,
    /// What makes it the same artifact again: the canonical absolute path
    /// or the normalized link.
    pub source: String,
    /// The folder the target was given from (the agent's place): the
    /// relative form of a path in a reply resolves from there.
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub versions: Vec<Version>,
}

impl Meta {
    pub fn current(&self) -> Option<&Version> {
        self.versions.last()
    }
}

/// What `sb artifact add` or `/artifacts add` asks.
#[derive(Clone, Debug, Default)]
pub struct Add {
    /// A path (absolute, or relative to `cwd`) or a link.
    pub target: String,
    pub title: Option<String>,
    pub kind: Option<String>,
    pub agent: String,
    /// `you` or the agent.
    pub by: String,
    pub cwd: String,
}

/// The answer to an add.
#[derive(Clone, Debug, PartialEq)]
pub struct Added {
    pub meta: Meta,
    /// A new version was made (false: added again, unchanged).
    pub new_version: bool,
    /// The add named a bise page: nothing stored, the page is its artifact.
    pub page: bool,
}

/// A bise page as the page store keeps it (only what the list reads).
#[derive(Clone, Debug, Default, Deserialize)]
struct PageMeta {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    agent: String,
    #[serde(default)]
    created_ms: u64,
    #[serde(default)]
    versions: Vec<PageVersion>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct PageVersion {
    #[serde(default)]
    n: u64,
    #[serde(default)]
    at_ms: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct PageNote {
    #[serde(default)]
    version: u64,
    #[serde(default = "draft")]
    status: String,
}

fn draft() -> String {
    "draft".into()
}

/// What the daemon knows of an agent, for a row: its name now and
/// whether it is archived. `None`: an agent the hub does not know (`you`,
/// a name from another workspace) — shown as written.
pub type Who<'a> = &'a dyn Fn(&str) -> Option<(String, bool)>;

pub struct Store {
    pub state: PathBuf,
}

fn read_json<T: serde::de::DeserializeOwned>(p: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

fn write_atomic(p: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, p).map_err(|e| e.to_string())
}

/// An id from a title: lowercase letters, digits and dashes, at most 40.
pub fn slug(title: &str) -> String {
    let mut s = String::new();
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    let mut s: String = s.trim_end_matches('-').chars().take(40).collect();
    while s.ends_with('-') {
        s.pop();
    }
    if s.is_empty() {
        "artifact".into()
    } else {
        s
    }
}

/// `artifact:<id>` or `artifact:<id>@v<n>` (the reply link form):
/// (id, version).
pub fn parse_ref(s: &str) -> Option<(String, Option<u64>)> {
    let rest = s.trim().strip_prefix("artifact:")?;
    let (id, v) = match rest.split_once('@') {
        Some((id, v)) => (id, Some(v.trim_start_matches('v').parse::<u64>().ok()?)),
        None => (rest, None),
    };
    let ok = !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    ok.then(|| (id.to_string(), v))
}

pub fn is_link(t: &str) -> bool {
    t.starts_with("http://") || t.starts_with("https://")
}

/// A link without its scheme and trailing slash: the form a reply may
/// write it in (`bise.dev/m/artifacts`).
fn bare_link(u: &str) -> String {
    u.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string()
}

/// `bise.dev/m/x` (no scheme, a host with a dot, no space): a link.
fn looks_like_host(t: &str) -> bool {
    let host = t.split('/').next().unwrap_or("");
    !t.starts_with('.')
        && !t.starts_with('/')
        && !t.starts_with('~')
        && host.contains('.')
        && t.contains('/')
        && !t.chars().any(char::is_whitespace)
        && host.rsplit('.').next().is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
}

/// The kind of a link.
pub fn link_kind(url: &str) -> &'static str {
    let b = bare_link(url);
    let host = b.split('/').next().unwrap_or("");
    let path = &b[host.len()..];
    let local = host.starts_with("127.0.0.1") || host.starts_with("localhost") || host.starts_with("0.0.0.0");
    if host == "github.com" {
        if pr_of(url).is_some() {
            return "pr";
        }
        if path.contains("/releases") {
            return "release";
        }
    }
    if host == "docs.google.com" {
        if path.starts_with("/spreadsheets") {
            return "sheet";
        }
        if path.starts_with("/presentation") {
            return "slides";
        }
        if path.starts_with("/document") {
            return "doc";
        }
    }
    if local && path.starts_with("/p/") {
        return "page";
    }
    if host == "bise.dev" && path.starts_with("/m/") {
        return "page";
    }
    if local || host.ends_with(".vercel.app") || host.ends_with(".netlify.app") || host.ends_with(".pages.dev") {
        return "site";
    }
    "link"
}

/// A GitHub PR link: (owner/repo, number).
pub fn pr_of(url: &str) -> Option<(String, u64)> {
    let b = bare_link(url);
    let parts: Vec<&str> = b.split('/').collect();
    if parts.first() != Some(&"github.com") || parts.len() < 5 || parts[3] != "pull" {
        return None;
    }
    let n = parts[4].split(['#', '?']).next()?.parse::<u64>().ok()?;
    Some((format!("{}/{}", parts[1], parts[2]), n))
}

/// The kind of a file or folder.
pub fn path_kind(p: &Path) -> &'static str {
    if p.is_dir() {
        return if p.join("index.html").is_file() { "site" } else { "folder" };
    }
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" | "pdf" | "txt" | "docx" | "doc" | "rtf" | "odt" | "pages" => "doc",
        "xlsx" | "xls" | "csv" | "tsv" | "numbers" | "ods" => "sheet",
        "pptx" | "ppt" | "key" | "odp" => "slides",
        "html" | "htm" => "page",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "heic" | "avif" => "image",
        "mp4" | "mov" | "webm" | "mkv" | "m4v" => "video",
        "wav" | "mp3" | "m4a" | "aac" | "flac" | "ogg" => "sound",
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "sh" | "go" | "rb" | "swift" | "c" | "h" | "cpp" | "java"
        | "kt" | "bend" | "sql" | "toml" | "yaml" | "yml" | "json" | "css" | "lua" | "zig" => "code",
        _ => "doc",
    }
}

/// The default title: a file's name, a PR's `PR #6`, a link's host and
/// path.
fn default_title(target: &str, is_url: bool) -> String {
    if is_url {
        if let Some((_, n)) = pr_of(target) {
            return format!("PR #{}", n);
        }
        return crate::util::clip(&bare_link(target), 60);
    }
    Path::new(target)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| target.to_string())
}

/// Size and newest mtime (ms) of a file or folder (`.git` skipped), and
/// how many files it holds.
fn measure(p: &Path) -> (u64, u64, usize) {
    fn mtime(m: &std::fs::Metadata) -> u64 {
        m.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis() as u64)
    }
    let Ok(m) = std::fs::metadata(p) else { return (0, 0, 0) };
    if m.is_file() {
        return (m.len(), mtime(&m), 1);
    }
    let (mut bytes, mut newest, mut files) = (0u64, mtime(&m), 0usize);
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            if e.file_name() == ".git" {
                continue;
            }
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                stack.push(e.path());
            } else if m.is_file() {
                bytes += m.len();
                newest = newest.max(mtime(&m));
                files += 1;
                if files > MAX_COPY_FILES {
                    return (bytes, newest, files);
                }
            }
        }
    }
    (bytes, newest, files)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_file() {
        if let Some(d) = to.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::copy(from, to)?;
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        if e.file_name() == ".git" {
            continue;
        }
        let ft = e.file_type()?;
        if ft.is_dir() || ft.is_file() {
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        }
    }
    Ok(())
}

impl Store {
    pub fn new(state: &Path) -> Store {
        Store { state: state.to_path_buf() }
    }

    fn dir(&self) -> PathBuf {
        self.state.join("artifacts")
    }

    fn pages_dir(&self) -> PathBuf {
        self.state.join("pages")
    }

    /// Every stored artifact (not the pages), any order.
    pub fn stored(&self) -> Vec<Meta> {
        let Ok(rd) = std::fs::read_dir(self.dir()) else { return Vec::new() };
        rd.flatten()
            .filter_map(|e| read_json::<Meta>(&e.path().join("meta.json")))
            .filter(|m| !m.id.is_empty() && !m.versions.is_empty())
            .collect()
    }

    fn save(&self, m: &Meta) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(m).map_err(|e| e.to_string())?;
        write_atomic(&self.dir().join(&m.id).join("meta.json"), &bytes)
    }

    /// The page server's base (`http://127.0.0.1:<port>`), from
    /// `<state>/pages.port`.
    fn pages_base(&self) -> Option<String> {
        let port = std::fs::read_to_string(self.state.join("pages.port")).ok()?;
        let port: u16 = port.trim().parse().ok()?;
        Some(format!("http://127.0.0.1:{}", port))
    }

    fn pages(&self) -> Vec<PageMeta> {
        let Ok(rd) = std::fs::read_dir(self.pages_dir()) else { return Vec::new() };
        rd.flatten()
            .filter_map(|e| read_json::<PageMeta>(&e.path().join("meta.json")))
            .filter(|p| !p.id.is_empty() && !p.versions.is_empty())
            .collect()
    }

    /// The page a link names (`http://127.0.0.1:<port>/p/<id>[/v/<n>]`).
    fn page_of_link(&self, url: &str) -> Option<String> {
        let base = self.pages_base()?;
        let rest = url.strip_prefix(&base)?.strip_prefix("/p/")?;
        let id = rest.split('/').next()?.to_string();
        self.pages().iter().any(|p| p.id == id).then_some(id)
    }

    /// `<state>/artifacts/seen.json`: when the user last looked (the
    /// header's "↗ N new" counts what came after). Missing: now, written.
    pub fn seen_ms(&self, now: u64) -> u64 {
        let p = self.dir().join("seen.json");
        match read_json::<Value>(&p).and_then(|v| v.get("seen_ms").and_then(|x| x.as_u64())) {
            Some(s) => s,
            None => {
                let _ = self.set_seen(now);
                now
            }
        }
    }

    pub fn set_seen(&self, now: u64) -> Result<(), String> {
        write_atomic(&self.dir().join("seen.json"), json!({"seen_ms": now}).to_string().as_bytes())
    }

    /// Register a file, a folder or a link; the same one again is its next
    /// version (unchanged: the same version). Err: the words for the user
    /// or the agent.
    pub fn add(&self, a: &Add, now: u64) -> Result<Added, String> {
        let raw = a.target.trim();
        if raw.is_empty() {
            return Err("no file or link given.".into());
        }
        if let Some(k) = &a.kind {
            if !KINDS.contains(&k.as_str()) {
                return Err(format!("unknown kind {}: {}.", k, KINDS.join(", ")));
            }
        }
        // a path first (relative to where it was asked), then a link
        let path = {
            let p = if let Some(h) = raw.strip_prefix("~/") {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(h))
            } else {
                Some(Path::new(&a.cwd).join(raw))
            };
            p.filter(|p| !is_link(raw) && p.exists()).and_then(|p| p.canonicalize().ok())
        };
        let url = if path.is_some() {
            None
        } else if is_link(raw) {
            Some(raw.to_string())
        } else if looks_like_host(raw) {
            Some(format!("https://{}", raw))
        } else {
            return Err(format!("no file or link at {}.", raw));
        };
        let stored = self.stored();
        let pages = self.pages();
        if let Some(u) = &url {
            if let Some(id) = self.page_of_link(u) {
                let p = pages.iter().find(|p| p.id == id).cloned().unwrap_or_default();
                return Ok(Added { meta: page_meta(&p, self.pages_base().as_deref()), new_version: false, page: true });
            }
        }
        let (source, target, kind) = match (&path, &url) {
            (Some(p), _) => {
                let s = p.to_string_lossy().to_string();
                (s.clone(), s, path_kind(p))
            }
            (None, Some(u)) => (u.trim_end_matches('/').to_string(), u.clone(), link_kind(u)),
            _ => unreachable!(),
        };
        let kind = a.kind.clone().unwrap_or_else(|| kind.to_string());
        let mut meta = match stored.iter().find(|m| m.source == source) {
            Some(m) => m.clone(),
            None => {
                let title = a.title.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| default_title(&target, url.is_some()));
                let base = slug(&title);
                let taken = |id: &str| stored.iter().any(|m| m.id == id) || pages.iter().any(|p| p.id == id);
                let mut id = base.clone();
                let mut n = 2;
                while taken(&id) {
                    id = format!("{}-{}", base, n);
                    n += 1;
                }
                Meta {
                    id,
                    title,
                    kind: kind.clone(),
                    agent: a.agent.clone(),
                    by: a.by.clone(),
                    created_ms: now,
                    source: source.clone(),
                    cwd: a.cwd.clone(),
                    versions: Vec::new(),
                }
            }
        };
        if let Some(t) = a.title.as_ref().filter(|t| !t.trim().is_empty()) {
            meta.title = t.trim().to_string();
        }
        if a.kind.is_some() {
            meta.kind = kind;
        }
        let sig = match &path {
            Some(p) => {
                let (bytes, mtime, _) = measure(p);
                format!("{}:{}", bytes, mtime)
            }
            None => String::new(),
        };
        // a file added again, unchanged: the same version
        if let (Some(p), Some(last)) = (&path, meta.versions.last()) {
            let same_sig = last.sig == sig && last.target == target;
            let same_bytes = || {
                let copy = last.copy.as_ref().map(|c| self.dir().join(&meta.id).join(c));
                match copy {
                    Some(c) if p.is_file() => std::fs::read(&c).ok().is_some_and(|b| std::fs::read(p).ok() == Some(b)),
                    _ => false,
                }
            };
            if same_sig || same_bytes() {
                self.save(&meta)?;
                return Ok(Added { meta, new_version: false, page: false });
            }
        }
        let v = meta.versions.last().map_or(1, |l| l.v + 1);
        let (copy, no_copy) = match &path {
            Some(p) => {
                let (bytes, _, files) = measure(p);
                if bytes > MAX_COPY {
                    (None, Some("over 50 MB".to_string()))
                } else if files > MAX_COPY_FILES {
                    (None, Some(format!("over {} files", MAX_COPY_FILES)))
                } else {
                    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
                    let rel = format!("v{}/{}", v, name);
                    match copy_tree(p, &self.dir().join(&meta.id).join(&rel)) {
                        Ok(()) => (Some(rel), None),
                        Err(e) => (None, Some(format!("copy failed: {}", e))),
                    }
                }
            }
            None => (None, None),
        };
        meta.versions.push(Version { v, at_ms: now, target, copy, no_copy, sig });
        self.save(&meta)?;
        Ok(Added { meta, new_version: true, page: false })
    }

    /// Every artifact, stored ones and pages, newest first (the time of
    /// the current version); an id both use goes to the stored one first
    /// and the page gets `-page`.
    pub fn all(&self) -> Vec<Meta> {
        let mut out = self.stored();
        let base = self.pages_base();
        for p in self.pages() {
            let mut m = page_meta(&p, base.as_deref());
            if out.iter().any(|o| o.id == m.id) {
                m.id = format!("{}-page", m.id);
            }
            out.push(m);
        }
        out.sort_by(|a, b| {
            let t = |m: &Meta| m.current().map_or(m.created_ms, |v| v.at_ms);
            t(b).cmp(&t(a)).then(a.id.cmp(&b.id))
        });
        out
    }

    /// One artifact by id (`artifact:<id>`).
    pub fn get(&self, id: &str) -> Option<Meta> {
        self.all().into_iter().find(|m| m.id == id)
    }

    /// The open notes of a page, per version: (open, done).
    fn page_notes(&self, id: &str, v: u64) -> (usize, usize) {
        let notes: Vec<PageNote> = read_json(&self.pages_dir().join(id).join("notes.json")).unwrap_or_default();
        let of_v: Vec<&PageNote> = notes.iter().filter(|n| n.version == v).collect();
        let open = of_v.iter().filter(|n| matches!(n.status.as_str(), "draft" | "sent")).count();
        (open, of_v.len() - open)
    }

    /// The `artifacts` event's rows (docs/artifacts.md, "the row").
    pub fn rows(&self, who: Who, workspace: &str) -> Vec<Value> {
        self.all().iter().map(|m| self.row(m, who, workspace)).collect()
    }

    pub fn row(&self, m: &Meta, who: Who, workspace: &str) -> Value {
        let (agent, archived) = who(&m.agent).unwrap_or_else(|| (m.agent.clone(), false));
        let page = m.by == "page";
        let notes = |v: u64| -> String {
            if !page {
                return String::new();
            }
            let (open, done) = self.page_notes(&m.id, v);
            let s = |n: usize| if n == 1 { "" } else { "s" };
            match (open, done) {
                (0, 0) => String::new(),
                (0, d) => format!("{} note{} done", d, s(d)),
                (o, _) => format!("{} note{} open", o, s(o)),
            }
        };
        let dir = self.dir().join(&m.id);
        let version = |v: &Version| {
            json!({
                "v": v.v, "ts_ms": v.at_ms, "target": v.target,
                "copy": v.copy.as_ref().map(|c| dir.join(c).to_string_lossy().to_string()),
                "note": if page { notes(v.v) } else { v.no_copy.clone().map(|w| format!("no copy: {}", w)).unwrap_or_default() },
            })
        };
        let cur = m.current().cloned().unwrap_or_default();
        let gone = !is_link(&cur.target) && !Path::new(&cur.target).exists();
        let detail = if page {
            notes(cur.v)
        } else if m.kind == "site" && is_link(&cur.target) {
            bare_link(&cur.target)
        } else {
            String::new()
        };
        json!({
            "id": m.id, "title": m.title, "kind": m.kind, "agent": agent, "by": m.by,
            "archived": archived, "ts_ms": cur.at_ms, "created_ms": m.created_ms, "v": cur.v,
            "target": cur.target,
            "copy": cur.copy.as_ref().map(|c| dir.join(c).to_string_lossy().to_string()),
            "gone": gone,
            "detail": detail,
            "pr": pr_of(&cur.target).map(|(repo, n)| json!({"repo": repo, "number": n})),
            "keys": keys(m, workspace),
            "versions": m.versions.iter().map(version).collect::<Vec<_>>(),
        })
    }

    /// How many came after the user last looked.
    pub fn new_count(&self, now: u64) -> usize {
        let seen = self.seen_ms(now);
        self.all().iter().filter(|m| m.by != "you" && m.current().is_some_and(|v| v.at_ms > seen)).count()
    }

    /// What `sb artifact list` prints: one line each, newest first,
    /// filtered by words (title, agent, kind, id) and agent.
    pub fn list_text(&self, words: &str, agent: Option<&str>, now: u64) -> String {
        let all = self.all();
        let hits: Vec<&Meta> = all
            .iter()
            .filter(|m| agent.is_none_or(|a| m.agent == a))
            .filter(|m| matches(m, words))
            .collect();
        if hits.is_empty() {
            return if all.is_empty() {
                "no artifacts yet. add what you make for the user with sb artifact add <path or link>.".into()
            } else {
                "nothing matches.".into()
            };
        }
        hits.iter()
            .take(50)
            .map(|m| {
                let cur = m.current().cloned().unwrap_or_default();
                let gone = !is_link(&cur.target) && !Path::new(&cur.target).exists();
                format!(
                    "[{}](artifact:{}) · {} · v{} · {} · {} ago · {}{}",
                    m.title,
                    m.id,
                    m.kind,
                    cur.v,
                    m.agent,
                    crate::util::age(cur.at_ms, now),
                    cur.target,
                    if gone { if cur.copy.is_some() { " · ▲ gone from disk · bise kept a copy" } else { " · ▲ gone from disk" } } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A page as an artifact (`by` = `page`).
fn page_meta(p: &PageMeta, base: Option<&str>) -> Meta {
    let url = |v: u64, last: bool| match base {
        Some(b) if last => format!("{}/p/{}", b, p.id),
        Some(b) => format!("{}/p/{}/v/{}", b, p.id, v),
        None => format!("page:{}", p.id),
    };
    let last = p.versions.iter().map(|v| v.n).max().unwrap_or(0);
    Meta {
        id: p.id.clone(),
        title: if p.title.is_empty() { p.id.clone() } else { p.title.clone() },
        kind: "page".into(),
        agent: p.agent.clone(),
        by: "page".into(),
        created_ms: p.created_ms,
        source: url(last, true),
        cwd: String::new(),
        versions: p
            .versions
            .iter()
            .map(|v| Version { v: v.n, at_ms: v.at_ms, target: url(v.n, v.n == last), ..Default::default() })
            .collect(),
    }
}

/// Every form a reply may name it by (E's plain links and paths): the
/// link with and without its scheme; the path absolute, relative to the
/// folder it was added from and to the workspace.
pub fn keys(m: &Meta, workspace: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    };
    for v in &m.versions {
        let t = &v.target;
        if is_link(t) {
            push(t.clone());
            push(t.trim_end_matches('/').to_string());
            push(bare_link(t));
        } else if t.starts_with('/') {
            push(t.clone());
            for base in [m.cwd.as_str(), workspace] {
                if base.is_empty() {
                    continue;
                }
                let canon = Path::new(base).canonicalize().unwrap_or_else(|_| PathBuf::from(base));
                if let Ok(rel) = Path::new(t).strip_prefix(&canon).or_else(|_| Path::new(t).strip_prefix(base)) {
                    push(rel.to_string_lossy().to_string());
                }
            }
        }
    }
    out
}

/// Does every word appear in the title, agent, kind or id (any case)?
pub fn matches(m: &Meta, words: &str) -> bool {
    let hay = format!("{} {} {} {}", m.title, m.agent, m.kind, m.id).to_lowercase();
    words.split_whitespace().all(|w| hay.contains(&w.to_lowercase()))
}

/// The thread line of an add (`sb artifact : id : agent : title : kind :
/// v`), for the maker's thread and main's.
pub fn thread_line(m: &Meta) -> String {
    let fields: Vec<String> = vec![
        m.id.clone(),
        m.agent.clone(),
        m.title.clone(),
        m.kind.clone(),
        m.current().map_or(1, |v| v.v).to_string(),
    ];
    crate::core::join_fields(&fields)
}

/// What the adder reads: `added pricing-plans.xlsx (sheet) · v1 · link it
/// as [pricing-plans.xlsx](artifact:pricing-plans-xlsx)`.
pub fn added_text(a: &Added) -> String {
    let m = &a.meta;
    let v = m.current().map_or(1, |v| v.v);
    let link = format!("[{}](artifact:{})", m.title, m.id);
    if a.page {
        return format!("{} is a bise page (v{}): it's in already. link it as {}", m.id, v, link);
    }
    let what = if a.new_version {
        format!("added {} ({}) · v{}", m.title, m.kind, v)
    } else {
        format!("{} is unchanged: still v{}", m.title, v)
    };
    let copy = match m.current().and_then(|c| c.no_copy.clone()) {
        Some(why) if a.new_version => format!(" · no copy kept ({})", why),
        _ => String::new(),
    };
    format!("{}{} · link it as {}", what, copy, link)
}

#[cfg(test)]
#[path = "artifacts_tests.rs"]
mod tests;
