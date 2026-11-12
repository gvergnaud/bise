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
//! Folders (docs/artifacts.md, "Folders"): an artifact's library folder
//! is `folder` in its meta.json; a page's is in `artifacts/filed.json`
//! (`{"<page id>": "<folder id>"}`: the page store owns the page's
//! meta.json). Both are written only by [`Store::move_to`] (and a new
//! artifact's add), and read in one place, [`Store::all`]; the join with
//! the library (`bise_home::library`) is [`resolve_folders`], for this
//! hub's rows and another project's ([`rows_of`]).
//!
//! A pull request is a stored artifact of kind `pr` keyed by its link in
//! one form (`forge::pr_link`), made, retitled and archived by the hub
//! as the forge says (`forge::artifact`, daemon/pr_arts.rs); `pr_ended`
//! marks the archives the hub made, the only ones a reopen undoes.
//!
//! A site's localStorage, `<id>/storage.json`, is crate::artifacts_storage's
//! (the page server writes it, under its lock).
//!
//! A new version is told by the file's `sig`, `<bytes>:<mtime ms>`
//! ([`Version::sig`]). Known limit: a rewrite of the same size within
//! the same millisecond keeps the sig, so it makes no new version. Two
//! saves of one file within 1 ms are rare, so no content hash is taken
//! on every add; a test that writes versions gives each its own size
//! (pages/artifact/notes_tests.rs, `write_version`).
//!
//! Plain functions over the state folder; the daemon's thread is the only
//! writer, so there is no lock.

use crate::artifacts_assets;
use crate::artifacts_turns::{self, Place, Rev, Sitting, Turn};
use bise_proto::anchor::Anchor;
use bise_proto::rows::SavedBy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The largest file (or folder, summed) bise keeps a copy of.
pub const MAX_COPY: u64 = 50 * 1024 * 1024;
/// A version's `no_copy` when it is over [`MAX_COPY`]: the one name for it,
/// written here and read by pages::site::served (a video too big to copy is
/// streamed from its own file; video-artifact, architect m_54229).
pub const TOO_BIG: &str = "over 50 MB";
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
    /// The turn of its first save, stamped by the hub (artifacts_turns):
    /// a save in the same turn by the same agent replaces this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<Turn>,
    /// His sitting in the viewer that made it (artifacts_turns::Sitting):
    /// his next save in it replaces this version; its `seq` the newest
    /// save applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sitting: Option<Sitting>,
    /// Its saves, oldest first (none in a store from before them: one).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revs: Vec<Rev>,
    /// A local site's recipe (artifacts_serve, site-revive): how its
    /// server runs again; written by the hub after the add, never from a
    /// request. None: a file, another link, or no listener at the add.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serve: Option<crate::artifacts_serve::Serve>,
}

impl Version {
    /// Who saved it: him (a sitting), else its turn's agent; none for a
    /// version from before both (a document tab's "perf saved a newer
    /// version" line, docs/plugin-apps.md A6b).
    pub fn saved_by(&self) -> Option<SavedBy> {
        match (&self.sitting, &self.turn) {
            (Some(_), _) => Some(SavedBy::You),
            (None, Some(t)) => Some(SavedBy::Agent { name: t.agent.clone() }),
            (None, None) => None,
        }
    }

    /// Its newest save's number (1 for a version from before revs).
    pub fn rev(&self) -> u32 {
        self.revs.last().map_or(1, |r| r.rev)
    }

    /// What it changed from the version before, merged over its saves;
    /// None: a save said nothing, so no detail.
    pub fn changed(&self) -> Option<Vec<Anchor>> {
        artifacts_turns::merged(&self.revs)
    }
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
    /// Its folder's id in the library (`bise_home::library`), raw: a
    /// reader resolves it ([`folder_of`]); none: unfiled. One folder per
    /// artifact, written only by its own hub ([`Store::move_to`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// pr-integration rule 4: the hub archived this PR artifact because
    /// the PR was merged or closed (forge::artifact). Only such an archive
    /// is undone when the PR is reopened; his own archive never is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pr_ended: bool,
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
    /// Only a next version of a file that already is an artifact
    /// (`bise office run`'s `run.write`, plan §2): another file is not
    /// added, the answer says so ([`Added::not_artifact`]).
    pub known_only: bool,
    /// The turn it came in, stamped by the hub from sb-core's fact (never
    /// from the request); None outside a turn.
    pub turn: Option<Turn>,
    /// His edits in the viewer's grid (pages/artifact/save.rs): the sitting
    /// they came in; None for any other add.
    pub sitting: Option<Sitting>,
    /// What the save changed, as the writer said it (`bise office run`);
    /// None: it said nothing.
    pub changed: Option<Vec<Anchor>>,
    /// The folder it goes in (a library folder's id), see [`Filed`].
    pub folder: Option<Filed>,
    /// A new artifact's id when the hub names it itself (a feature's
    /// diff: `<feature>-diff`); taken by another one, or none: from its
    /// title. A next version keeps its id.
    pub id: Option<String>,
    /// A copy staged off the hub's loop ([`Store::stage_copy`], a view's
    /// write, docs/plugin-apps.md A4): renamed in as the version's copy
    /// instead of copying the file on the loop; the caller already
    /// decided it is a new save.
    pub staged: Option<PathBuf>,
}

/// An add's folder: `Asked` (`--folder`) files it, a next version too;
/// `Default` (its task's folder) only files a new artifact, so a next
/// version stays where he put it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filed {
    Asked(String),
    Default(String),
}

/// The answer to an add.
#[derive(Clone, Debug, PartialEq)]
pub struct Added {
    pub meta: Meta,
    /// A new version was made (false: added again, unchanged).
    pub new_version: bool,
    /// The add named a bise page: nothing stored, the page is its artifact.
    pub page: bool,
    /// A `known_only` add of a file that is no artifact: nothing stored,
    /// `meta` is empty but for its `source`.
    pub not_artifact: bool,
    /// A save in the same turn as the newest version (artifacts_turns):
    /// its copy replaced, `v` the same, its rev grew.
    pub replaced: bool,
    /// An html file's links: how many files were kept beside its copy,
    /// what was not (artifacts_assets), and whether it runs scripts.
    pub assets: usize,
    pub unkept: Vec<String>,
    pub scripts: bool,
}

impl Added {
    fn none() -> Added {
        Added { meta: Meta::default(), new_version: false, page: false, not_artifact: false, replaced: false, assets: 0, unkept: Vec::new(), scripts: false }
    }
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
    // the rows built from the store are built again (crate::store_gen)
    crate::store_gen::bump();
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
    // a PR (GitHub, a GitLab merge request): the forge's one parse
    if crate::forge::pr_link(url).is_some() {
        return "pr";
    }
    if host == "github.com" && path.contains("/releases") {
        return "release";
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
        | "kt" | "bend" | "sql" | "toml" | "yaml" | "yml" | "json" | "css" | "lua" | "zig" | "diff" | "patch" => "code",
        _ => "doc",
    }
}

/// The default title: a file's name, a PR's `PR #6` (`MR !6`) until the
/// forge says its title, a link's host and path.
fn default_title(target: &str, is_url: bool) -> String {
    if is_url {
        if let Some(l) = crate::forge::pr_link(target) {
            let what = if l.provider == bise_proto::rows::PrProvider::Gitlab { "MR" } else { "PR" };
            return format!("{} {}", what, l.tag());
        }
        return crate::util::clip(&bare_link(target), 60);
    }
    Path::new(target)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| target.to_string())
}

/// A file's sig now, as [`Version::sig`] has it for a file that links no
/// other (`<bytes>:<mtime ms>`); None: not a file. The page server checks
/// it before it streams a file bise kept no copy of (pages::media::own_holds).
pub fn file_sig(p: &Path) -> Option<String> {
    std::fs::metadata(p).ok().filter(std::fs::Metadata::is_file)?;
    let (bytes, mtime, _) = measure(p);
    Some(format!("{}:{}", bytes, mtime))
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

/// Copy into a sibling `.<name>.part`, then rename over `to`: a reader
/// (the viewer fetching the copy mid-save, architect m_23911) gets the
/// old bytes or the new, never half a file.
fn copy_whole(from: &Path, to: &Path) -> std::io::Result<()> {
    let name = to.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let part = to.with_file_name(format!(".{name}.part"));
    if part.is_dir() {
        std::fs::remove_dir_all(&part)?;
    }
    copy_tree(from, &part)?;
    if to.is_dir() {
        std::fs::remove_dir_all(to)?;
    }
    crate::store_gen::bump();
    std::fs::rename(&part, to)
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

    /// A stored artifact's title, set (a PR's: the forge's,
    /// forge::artifact). An unknown id: an error.
    pub fn retitle(&self, id: &str, title: &str) -> Result<(), String> {
        let m = self.stored().into_iter().find(|m| m.id == id).ok_or_else(|| format!("no artifact {id}"))?;
        self.save(&Meta { title: title.to_string(), ..m })
    }

    /// Mark a PR artifact as archived by the hub at the PR's end (or no
    /// more): [`Meta::pr_ended`].
    pub fn set_pr_ended(&self, id: &str, ended: bool) -> Result<(), String> {
        let m = self.stored().into_iter().find(|m| m.id == id).ok_or_else(|| format!("no artifact {id}"))?;
        if m.pr_ended == ended {
            return Ok(());
        }
        self.save(&Meta { pr_ended: ended, ..m })
    }

    /// name-free (sb-core's `retarget` fx): another agent takes the old
    /// name `from`, so the artifacts that still say `from` say `to`, the
    /// agent that had it. The number moved.
    pub fn retarget(&self, from: &str, to: &str) -> usize {
        self.stored()
            .into_iter()
            .filter(|m| m.agent == from)
            .filter(|m| self.save(&Meta { agent: to.to_string(), ..m.clone() }).is_ok())
            .count()
    }

    /// The page server's base (`http://127.0.0.1:<port>`), from
    /// `<state>/pages.port`.
    pub fn pages_base(&self) -> Option<String> {
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

    /// The user looked at `at` (the client's clock, when the screen
    /// opened); the hub reads it at `now`. What came between the two
    /// (an add served before this request, which waited in the
    /// client's socket) stays new: seen moves to `at`, never past
    /// `now`, never back.
    pub fn saw(&self, at: u64, now: u64) -> Result<(), String> {
        let before = self.seen_ms(0);
        self.set_seen(at.min(now).max(before))
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
                return Ok(Added { meta: page_meta(&p, self.pages_base().as_deref()), new_version: false, page: true, not_artifact: false, replaced: false, ..Added::none() });
            }
        }
        let (source, target, kind) = match (&path, &url) {
            (Some(p), _) => {
                let s = p.to_string_lossy().to_string();
                (s.clone(), s, path_kind(p))
            }
            // a PR: its one key (forge::pr_link), so `…/pull/6/files` and
            // the hub's own add land on the same artifact
            (None, Some(u)) => match crate::forge::pr_link(u) {
                Some(l) => (l.url.clone(), l.url, "pr"),
                None => (u.trim_end_matches('/').to_string(), u.clone(), link_kind(u)),
            },
            _ => unreachable!(),
        };
        if a.known_only && !stored.iter().any(|m| m.source == source) {
            let meta = Meta { source, ..Meta::default() };
            return Ok(Added { meta, new_version: false, page: false, not_artifact: true, replaced: false, ..Added::none() });
        }
        let kind = a.kind.clone().unwrap_or_else(|| kind.to_string());
        let mut meta = match stored.iter().find(|m| m.source == source) {
            Some(m) => m.clone(),
            None => {
                let title = a.title.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| default_title(&target, url.is_some()));
                let taken = |id: &str| stored.iter().any(|m| m.id == id) || pages.iter().any(|p| p.id == id);
                let named = a.id.as_deref().map(slug).filter(|id| !taken(id));
                let base = named.unwrap_or_else(|| slug(&title));
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
                    folder: match &a.folder {
                        Some(Filed::Asked(f) | Filed::Default(f)) => Some(f.clone()),
                        None => None,
                    },
                    pr_ended: false,
                }
            }
        };
        if let Some(Filed::Asked(f)) = &a.folder {
            meta.folder = Some(f.clone());
        }
        if let Some(t) = a.title.as_ref().filter(|t| !t.trim().is_empty()) {
            meta.title = t.trim().to_string();
        }
        if a.kind.is_some() {
            meta.kind = kind;
        }
        // an html file: the files it links beside it go with it (artifacts_assets);
        // its size, sig and caps count them all
        let assets = path.as_deref().filter(|p| artifacts_assets::is_html(p)).map(|p| artifacts_assets::gather(p, MAX_COPY_FILES)).unwrap_or_default();
        let root = path.as_deref().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
        let size = path.as_deref().map(|p| {
            assets.files.iter().fold(measure(p), |(b, t, n), f| {
                let (fb, ft, _) = measure(&root.join(f));
                (b + fb, t.max(ft), n + 1)
            })
        });
        let sig = match size {
            Some((bytes, mtime, _)) if assets.files.is_empty() => format!("{}:{}", bytes, mtime),
            Some((bytes, mtime, n)) => format!("{}:{}:{}", bytes, mtime, n),
            None => String::new(),
        };
        // a file added again, unchanged: the same version
        if let (Some(p), Some(last)) = (&path, meta.versions.last()) {
            let same_sig = last.sig == sig && last.target == target;
            let same_bytes = || {
                let copy = last.copy.as_ref().map(|c| self.dir().join(&meta.id).join(c));
                match copy {
                    Some(c) if p.is_file() && assets.files.is_empty() => std::fs::read(&c).ok().is_some_and(|b| std::fs::read(p).ok() == Some(b)),
                    _ => false,
                }
            };
            // a staged save was decided new by its caller: never read both
            // files on the hub's loop to doubt it
            if a.staged.is_none() && (same_sig || same_bytes()) {
                self.save(&meta)?;
                return Ok(Added { meta, new_version: false, page: false, not_artifact: false, replaced: false, ..Added::none() });
            }
        }
        // a save in the same turn by the same agent, or in the same sitting of
        // his edits: the newest version again
        let last = meta.versions.last();
        let same = match &a.sitting {
            Some(_) => artifacts_turns::place_sitting(last.and_then(|l| l.sitting.as_ref()), a.sitting.as_ref()),
            None => artifacts_turns::place(last.and_then(|l| l.turn.as_ref()), a.turn.as_ref()),
        } == Place::Same;
        let v = match meta.versions.last() {
            Some(l) if same => l.v,
            Some(l) => l.v + 1,
            None => 1,
        };
        let (copy, no_copy) = match (&path, size) {
            (Some(p), Some((bytes, _, files))) => {
                if bytes > MAX_COPY {
                    (None, Some(TOO_BIG.to_string()))
                } else if files > MAX_COPY_FILES {
                    (None, Some(format!("over {} files", MAX_COPY_FILES)))
                } else {
                    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
                    let rel = format!("v{}/{}", v, name);
                    let vdir = self.dir().join(&meta.id).join(format!("v{}", v));
                    // its files first, the html last: a reader of the html finds them there
                    let linked = assets.files.iter().try_for_each(|f| copy_whole(&root.join(f), &vdir.join(f)));
                    let to = self.dir().join(&meta.id).join(&rel);
                    let copied = match &a.staged {
                        // staged off the loop: a rename, the copy's bytes the file's
                        Some(s) if assets.files.is_empty() => std::fs::create_dir_all(&vdir).and_then(|()| {
                            crate::store_gen::bump();
                            std::fs::rename(s, &to)
                        }),
                        _ => copy_whole(p, &to),
                    };
                    match linked.and(copied) {
                        Ok(()) => (Some(rel), None),
                        Err(e) => (None, Some(format!("copy failed: {}", e))),
                    }
                }
            }
            _ => (None, None),
        };
        let rev = |revs: &[Rev]| Rev { rev: artifacts_turns::next_rev(revs), at_ms: now, changed: a.changed.clone() };
        match meta.versions.last_mut() {
            Some(last) if same => {
                // the copy was replaced in place; the version moves to this save
                if last.revs.is_empty() {
                    last.revs.push(Rev { rev: 1, at_ms: last.at_ms, changed: None });
                }
                let r = rev(&last.revs);
                last.revs.push(r);
                *last = Version { at_ms: now, target, copy, no_copy, sig, sitting: a.sitting.clone().or(last.sitting.clone()), ..last.clone() };
            }
            _ => {
                let revs = vec![rev(&[])];
                meta.versions.push(Version { v, at_ms: now, target, copy, no_copy, sig, turn: a.turn.clone(), sitting: a.sitting.clone(), revs, serve: None });
            }
        }
        self.save(&meta)?;
        Ok(Added { meta, new_version: true, page: false, not_artifact: false, replaced: same, assets: assets.files.len(), unkept: assets.unkept, scripts: assets.scripts })
    }

    /// A view's save of artifact `id`, staged as its next copy off the
    /// hub's loop (docs/plugin-apps.md A4): `bytes` in a temp file inside
    /// the artifact's folder, so [`Store::add`] with [`Add::staged`] only
    /// renames it. The caller deletes it when the save loses its decide.
    pub fn stage_copy(&self, id: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = self.dir().join(id);
        std::fs::create_dir_all(&dir)?;
        let p = dir.join(format!(".staged-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        std::fs::write(&p, bytes)?;
        std::fs::File::open(&p)?.sync_all()?;
        Ok(p)
    }

    /// The bytes of bise's copy of version `v` (None: no copy, or unreadable).
    pub fn copy_bytes(&self, meta: &Meta, v: &Version) -> Option<Vec<u8>> {
        std::fs::read(self.copy_path(meta, v)?).ok()
    }

    /// Where bise's copy of version `v` is (None: no copy kept).
    pub fn copy_path(&self, meta: &Meta, v: &Version) -> Option<PathBuf> {
        Some(self.dir().join(&meta.id).join(v.copy.as_ref()?))
    }

    /// Every artifact, stored ones and pages, newest first (the time of
    /// the current version); an id both use goes to the stored one first
    /// and the page gets `-page`.
    pub fn all(&self) -> Vec<Meta> {
        let mut out = self.stored();
        let base = self.pages_base();
        let filed = self.pages_filed();
        for p in self.pages() {
            let mut m = page_meta(&p, base.as_deref());
            if out.iter().any(|o| o.id == m.id) {
                m.id = format!("{}-page", m.id);
            }
            m.folder = filed.get(&m.id).cloned();
            out.push(m);
        }
        out.sort_by(|a, b| {
            let t = |m: &Meta| m.current().map_or(m.created_ms, |v| v.at_ms);
            t(b).cmp(&t(a)).then(a.id.cmp(&b.id))
        });
        out
    }

    /// `<state>/artifacts/<id>/storage.json`: a site's localStorage, every
    /// version's (crate::artifacts_storage, its one writer).
    pub fn storage_path(&self, id: &str) -> PathBuf {
        self.dir().join(id).join("storage.json")
    }

    /// One artifact by id (`artifact:<id>`).
    pub fn get(&self, id: &str) -> Option<Meta> {
        self.all().into_iter().find(|m| m.id == id)
    }

    /// `artifacts/filed.json`: the pages' folders, `{"<page id>": "<folder
    /// id>"}` (a page's meta.json is the page store's; this store keeps
    /// what it files).
    fn filed_file(&self) -> PathBuf {
        self.dir().join("filed.json")
    }

    fn pages_filed(&self) -> BTreeMap<String, String> {
        read_json(&self.filed_file()).unwrap_or_default()
    }

    /// File `ids` in `folder` (a library folder's id, checked by the
    /// caller; none: unfiled), all or none: an unknown id moves nothing.
    /// Each one's folder before, raw (his undo puts it back).
    pub fn move_to(&self, ids: &[String], folder: Option<&str>) -> Result<Vec<(String, Option<String>)>, String> {
        let all = self.all();
        let metas = ids
            .iter()
            .map(|id| all.iter().find(|m| &m.id == id).ok_or_else(|| format!("no artifact {id}")))
            .collect::<Result<Vec<_>, _>>()?;
        let mut filed = self.pages_filed();
        let mut out = Vec::new();
        for m in metas {
            out.push((m.id.clone(), m.folder.clone()));
            if m.by == "page" {
                match folder {
                    Some(f) => filed.insert(m.id.clone(), f.to_string()),
                    None => filed.remove(&m.id),
                };
            } else {
                self.save(&Meta { folder: folder.map(str::to_string), ..m.clone() })?;
            }
        }
        if out.iter().any(|(id, _)| all.iter().any(|m| &m.id == id && m.by == "page")) {
            let bytes = serde_json::to_vec_pretty(&filed).map_err(|e| e.to_string())?;
            write_atomic(&self.filed_file(), &bytes)?;
        }
        Ok(out)
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
        let put_away = self.archived();
        self.all().iter().map(|m| self.row(m, who, workspace, put_away.get(&m.id).copied())).collect()
    }

    /// `archived_at`: when the artifact itself was archived (`sb artifact
    /// archive`), or None; `archived` is its agent's.
    pub fn row(&self, m: &Meta, who: Who, workspace: &str, archived_at: Option<u64>) -> Value {
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
                "v": v.v, "ts_ms": v.at_ms, "target": v.target, "by": v.saved_by(),
                "copy": v.copy.as_ref().map(|c| dir.join(c).to_string_lossy().to_string()),
                // why it has no copy: a video too big to copy is framed from its own file (site::served)
                "no_copy": v.no_copy,
                "note": if page {
                    notes(v.v)
                } else {
                    // an archived artifact's older copy is simply gone (designer m_16873)
                    v.no_copy.clone().map(|w| if w == "archived" { "not kept".to_string() } else { format!("no copy: {}", w) }).unwrap_or_default()
                },
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
            "archived": archived, "archived_at": archived_at, "ts_ms": cur.at_ms, "created_ms": m.created_ms, "v": cur.v,
            "target": cur.target,
            "copy": cur.copy.as_ref().map(|c| dir.join(c).to_string_lossy().to_string()),
            "no_copy": cur.no_copy,
            "gone": gone,
            "detail": detail,
            "pr": crate::forge::pr_link(&cur.target).map(|l| json!({"repo": l.repo, "number": l.number, "tag": l.tag()})),
            "keys": keys(m, workspace),
            "versions": m.versions.iter().map(version).collect::<Vec<_>>(),
            // raw: the hub resolves it against the library (artifacts_ev)
            "folder": m.folder,
            "turn": cur.turn.as_ref().map(|t| t.started_ms),
            // a local site bise serves again (artifacts_serve): the page
            // server's door, what every client opens; `target` stays the link
            "live": self.live_url(m),
        })
    }

    /// The door of a local site bise can serve again
    /// (`<pages base>/a/<id>/live`, artifacts_serve::has_door); None
    /// otherwise or with no page server.
    pub fn live_url(&self, m: &Meta) -> Option<String> {
        m.current().filter(|v| m.by != "page" && crate::artifacts_serve::has_door(v))?;
        Some(format!("{}{}", self.pages_base()?, crate::artifacts_serve::live_href(&m.id)))
    }

    /// Version `v` of artifact `id` gets its recipe (artifacts_serve,
    /// found off the hub's loop after the add) and, for a static site,
    /// bise's copy of the folder it serves (`root`; the caps of any
    /// folder). False: no such version, or nothing to write.
    pub fn set_serve(&self, id: &str, v: u64, serve: crate::artifacts_serve::Serve, root: Option<&Path>) -> Result<bool, String> {
        let Some(mut meta) = self.stored().into_iter().find(|m| m.id == id) else { return Ok(false) };
        let Some(ver) = meta.versions.iter_mut().find(|x| x.v == v) else { return Ok(false) };
        if ver.serve.as_ref() == Some(&serve) && (root.is_none() || ver.copy.is_some()) {
            return Ok(false);
        }
        ver.serve = Some(serve);
        if let Some(root) = root.filter(|_| ver.copy.is_none()) {
            let (bytes, _, files) = measure(root);
            if bytes > MAX_COPY {
                ver.no_copy = Some(TOO_BIG.into());
            } else if files > MAX_COPY_FILES {
                ver.no_copy = Some(format!("over {} files", MAX_COPY_FILES));
            } else {
                let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "site".into());
                let rel = format!("v{}/{}", v, name);
                match copy_whole(root, &self.dir().join(id).join(&rel)) {
                    Ok(()) => ver.copy = Some(rel),
                    Err(e) => ver.no_copy = Some(format!("copy failed: {}", e)),
                }
            }
        }
        self.save(&meta)?;
        Ok(true)
    }

    /// How many came after the user last looked (the archived ones never).
    pub fn new_count(&self, now: u64) -> usize {
        let seen = self.seen_ms(now);
        let put_away = self.archived();
        self.all()
            .iter()
            .filter(|m| m.by != "you" && !put_away.contains_key(&m.id) && m.current().is_some_and(|v| v.at_ms > seen))
            .count()
    }

    /// What `sb artifact list` prints: one line each, newest first,
    /// filtered by words (title, agent, kind, id) and agent; the archived
    /// ones only with `archived` (else a last line counts them).
    pub fn list_text(&self, words: &str, agent: Option<&str>, archived: bool, now: u64) -> String {
        let put_away = self.archived();
        let all = self.all();
        let hidden = all.iter().filter(|m| put_away.contains_key(&m.id) != archived).count();
        let foot = if archived || hidden == 0 {
            String::new()
        } else {
            format!("\n{} archived · sb artifact list --archived", hidden)
        };
        let all: Vec<Meta> = all.into_iter().filter(|m| put_away.contains_key(&m.id) == archived).collect();
        let hits: Vec<&Meta> = all
            .iter()
            .filter(|m| agent.is_none_or(|a| m.agent == a))
            .filter(|m| matches(m, words))
            .collect();
        if hits.is_empty() {
            return if archived {
                "nothing archived.".into()
            } else if all.is_empty() && hidden == 0 {
                "no artifacts yet. add what you make for the user with sb artifact add <path or link>.".into()
            } else {
                format!("nothing matches.{}", foot)
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
            + &foot
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
        folder: None,
        pr_ended: false,
        versions: p
            .versions
            .iter()
            .map(|v| Version { v: v.n, at_ms: v.at_ms, target: url(v.n, v.n == last), ..Default::default() })
            .collect(),
    }
}

/// pr-integration rules 1 and 3: each PR row's `pr.status`, the hub's
/// row of that PR (`Hub::pr_rows_by_url`, by the link's one form), joined
/// when the event is sent; none when the hub does not follow it (no
/// answer yet, no gh), so the row never shows a made-up state.
pub fn join_pr_status(rows: &mut [Value], by_url: &BTreeMap<String, bise_proto::rows::Pr>) {
    for r in rows.iter_mut() {
        let Some(url) = r["target"].as_str().and_then(crate::forge::pr_link).map(|l| l.url) else { continue };
        let Some(pr) = r.get_mut("pr").filter(|p| p.is_object()) else { continue };
        match by_url.get(&url).and_then(|s| serde_json::to_value(s).ok()) {
            Some(s) => pr["status"] = s,
            None => {
                if let Some(o) = pr.as_object_mut() {
                    o.remove("status");
                }
            }
        }
    }
}

/// The rows' `folder`, raw from their metas, as readers read it: the
/// live library folder it resolves to (a merged one's target), else
/// none (a deleted or unknown id: unfiled). The one join of the
/// artifacts and the library, for this hub's rows and another
/// project's (artifacts/library).
pub fn resolve_folders(rows: &mut [Value], lib: &bise_home::library::Library) {
    for r in rows.iter_mut() {
        let f = r.get("folder").and_then(Value::as_str).and_then(|f| bise_home::library::resolve(lib, f)).map(|f| f.id.clone());
        r["folder"] = json!(f);
    }
}

/// Another project's rows, read from its state folder (artifacts/library):
/// the same store reader as this hub's own, agents by their stored names,
/// folders resolved.
pub fn rows_of(state: &Path, workspace: &str, lib: &bise_home::library::Library) -> Vec<Value> {
    let store = Store::new(state);
    let mut rows = store.rows(&|_: &str| None, workspace);
    resolve_folders(&mut rows, lib);
    rows
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

/// The thread record of an add (its id, agent, title, kind, version), for
/// the maker's thread and main's.
pub fn thread_rec(m: &Meta) -> bise_proto::core::Feed {
    bise_proto::core::Feed::Artifact(bise_proto::core::feed::Artifact {
        id: m.id.clone(),
        agent: m.agent.clone(),
        title: m.title.clone(),
        kind: m.kind.clone(),
        version: m.current().map_or(1, |v| v.v),
    })
}

/// What the adder reads: `added pricing-plans.xlsx (sheet) · v1 · link it
/// as [pricing-plans.xlsx](artifact:pricing-plans-xlsx)`.
pub fn added_text(a: &Added) -> String {
    let m = &a.meta;
    let v = m.current().map_or(1, |v| v.v);
    let link = format!("[{}](artifact:{})", m.title, m.id);
    if a.not_artifact {
        return format!("{} is not an artifact: written only", m.source);
    }
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
    let with = if a.new_version && a.assets > 0 && copy.is_empty() { format!(" · with {} files it links", a.assets) } else { String::new() };
    let mut out = format!("{}{}{} · link it as {}", what, with, copy, link);
    if a.new_version {
        for line in artifacts_assets::unkept_line(&a.unkept).into_iter().chain(a.scripts.then(|| artifacts_assets::SCRIPTS_LINE.to_string())) {
            out.push('\n');
            out.push_str(&line);
        }
    }
    out
}

#[path = "artifacts_archive.rs"]
mod archive;
pub use archive::{archived_text, parse_before, unarchived_text, Archived, Pick};

#[cfg(test)]
#[path = "artifacts_tests.rs"]
mod tests;
