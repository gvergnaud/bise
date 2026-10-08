//! The projects registry (bise desktop, S1): the workspaces the user
//! works in, for the window's sidebar, `bise project` and bise's routing.
//!
//! One file, `<root>/projects.json` (bise layout only: in the legacy
//! layout the list is the home workspace alone and every write says to
//! run bise once), `{"v":1,"projects":[{path,name,order,added_ms}]}`.
//! User config, not hub state: no hub id (it is [`crate::hub_id`] of the
//! path), no facts about the hub (those are the hub's `view.json`).
//!
//! The home workspace (`~/bise`, the global main's) is row 0 of every
//! [`list`], implicit: never in the file, never removed.
//!
//! One writer: [`update`] (flock on `projects.json.lock`, read, change,
//! write tmp + rename), called by the CLI and the ambient core. The
//! changes themselves ([`add`], [`remove`], [`rename`], [`move_to`]) are
//! pure functions over the list, tested alone.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::{Home, Layout};

/// The file, in `Home::root`.
pub const FILE: &str = "projects.json";
/// What every write says in the legacy layout.
pub const LEGACY: &str = "the projects list needs ~/.bise: run bise once to move there";

/// One registered workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// Canonical (the caller canonicalizes: [`canonical`]).
    pub path: PathBuf,
    pub name: String,
    /// 0.. in the file; [`update`] renumbers after each change.
    pub order: u32,
    pub added_ms: u64,
}

/// A row of [`list`]: a project, or the home workspace (row 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub path: PathBuf,
    pub name: String,
    /// `hub_id(path)`: its hub's folder in `Home::hubs_dir`.
    pub id: String,
    pub home: bool,
    pub added_ms: u64,
}

/// The places a project can never be: the home workspace (it is row 0
/// already), bise's own state (hubs, task worktrees), and a git worktree
/// whose main repo is registered (an agent's task folder).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Places {
    pub home_ws: PathBuf,
    pub hubs: PathBuf,
    pub worktrees: PathBuf,
}

impl Places {
    /// The places of `home`, canonical when they exist.
    pub fn of(home: &Home, home_ws: &Path) -> Places {
        Places { home_ws: canonical(home_ws), hubs: canonical(&home.hubs_dir()), worktrees: canonical(&home.worktrees_dir()) }
    }
}

/// `p` canonical when it exists (symlinks resolved: `/tmp` is
/// `/private/tmp` on macOS), else as given.
pub fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The home workspace (docs/ambient-pages.md §5.1): `~/bise`, a plain
/// folder without git for non-code work; `$BISE_HOME_WORKSPACE` when set
/// (the tests: a throwaway folder, never the user's). The one rule: the
/// hub (switchboard::paths), the TUI and the ambient core call it.
pub fn home_workspace() -> PathBuf {
    if let Some(d) = crate::env::test_setting("BISE_HOME_WORKSPACE") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join("bise")
}

/// `ws` is the home workspace: its hub's main is bise (bise desktop S2).
pub fn is_home(ws: &Path) -> bool {
    canonical(ws) == canonical(&home_workspace())
}

/// Whether `ws` is in the projects registry (the desktop app writes it
/// when it shows a project).
pub fn is_registered(ws: &Path) -> bool {
    let ws = canonical(ws);
    read(&Home::from_env()).iter().any(|p| canonical(&p.path) == ws)
}

/// Whether the desktop's rules are on for a workspace (architect m_12156,
/// m_12576): its main prompt's page rules and the bise-pages skill
/// (prompts/skills-all) for its agents and its TUI's `$` popup. A fact
/// read once when a prompt is built or a REPL starts, never whether a
/// window is attached now. On: bise's home hub, or a registered project.
/// Off: a plain project, which behaves as before the desktop.
pub fn desktop_on(home: bool, registered: bool) -> bool {
    home || registered
}

/// [`desktop_on`] for `ws`, its two facts read now.
pub fn desktop_for(ws: &Path) -> bool {
    desktop_on(is_home(ws), is_registered(ws))
}

/// The main repo of a git worktree at `path` (its `.git` is a file
/// `gitdir: <repo>/.git/worktrees/<name>`), None for anything else.
pub fn worktree_main(path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path.join(".git")).ok()?;
    main_of_gitdir(&text)
}

/// The pure part of [`worktree_main`]: a `.git` file's text.
pub fn main_of_gitdir(text: &str) -> Option<PathBuf> {
    let dir = text.lines().find_map(|l| l.strip_prefix("gitdir:"))?.trim();
    let dir = Path::new(dir);
    // <repo>/.git/worktrees/<name>
    let wts = dir.parent()?;
    let git = wts.parent()?;
    let repo = git.parent()?;
    (wts.file_name()? == "worktrees" && git.file_name()? == ".git").then(|| canonical(repo))
}

/// What [`add`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Added {
    New(Project),
    /// Already there (same canonical path): unchanged.
    Already(Project),
}

/// Add `path` (canonical) to `list`, named `name` or its folder's name
/// (`name-2`, `-3`... on a clash). `main_repo`: the repo `path` is a git
/// worktree of ([`worktree_main`]).
pub fn add(
    list: &mut Vec<Project>,
    path: &Path,
    name: Option<&str>,
    now_ms: u64,
    places: &Places,
    main_repo: Option<&Path>,
) -> Result<Added, String> {
    if let Some(p) = list.iter().find(|p| p.path == path) {
        return Ok(Added::Already(p.clone()));
    }
    if path == places.home_ws || path.starts_with(&places.home_ws) {
        return Err(format!("{} is bise's own workspace: it is always in the list", path.display()));
    }
    for (dir, what) in [(&places.hubs, "a hub's state"), (&places.worktrees, "a task's worktree")] {
        if path.starts_with(dir) {
            return Err(format!("{} is in bise's own state ({what}), not a project", path.display()));
        }
    }
    if let Some(m) = main_repo.and_then(|m| list.iter().find(|p| p.path == m)) {
        return Err(format!("{} is a git worktree of the project {}", path.display(), m.name));
    }
    let base = match name.map(str::trim) {
        Some("") => return Err("a project's name cannot be empty".into()),
        Some(n) => {
            if list.iter().any(|p| p.name == n) {
                return Err(format!("a project is already named {n}"));
            }
            n.to_string()
        }
        None => free_name(list, &folder_name(path)),
    };
    let p = Project { path: path.to_path_buf(), name: base, order: list.len() as u32, added_ms: now_ms };
    list.push(p.clone());
    Ok(Added::New(p))
}

fn folder_name(path: &Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "root".into())
}

/// `base`, else `base-2`, `base-3`... the first no project has.
fn free_name(list: &[Project], base: &str) -> String {
    let taken = |n: &str| list.iter().any(|p| p.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|i| format!("{base}-{i}")).find(|n| !taken(n)).unwrap_or_default()
}

/// The index of the project `key` names: its name, else its path
/// (`key` canonical when it is one: the caller's job).
pub fn find(list: &[Project], key: &str) -> Option<usize> {
    list.iter().position(|p| p.name == key).or_else(|| list.iter().position(|p| p.path == Path::new(key)))
}

fn found(list: &[Project], key: &str) -> Result<usize, String> {
    find(list, key).ok_or_else(|| format!("no project {key}"))
}

/// Take `key` out of the list (its hub's state stays where it is).
pub fn remove(list: &mut Vec<Project>, key: &str) -> Result<Project, String> {
    let i = found(list, key)?;
    Ok(list.remove(i))
}

/// Name `key` `name` (unique, not empty).
pub fn rename(list: &mut [Project], key: &str, name: &str) -> Result<(), String> {
    let i = found(list, key)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("a project's name cannot be empty".into());
    }
    if list.iter().enumerate().any(|(j, p)| j != i && p.name == name) {
        return Err(format!("a project is already named {name}"));
    }
    list[i].name = name.to_string();
    Ok(())
}

/// Put `key` at `index` (0 = first after home; past the end = last).
pub fn move_to(list: &mut Vec<Project>, key: &str, index: usize) -> Result<(), String> {
    let i = found(list, key)?;
    let p = list.remove(i);
    list.insert(index.min(list.len()), p);
    Ok(())
}

/// The file's projects, by order. A file that is not ours (bad JSON, no
/// list) reads as empty; a row with no path is skipped.
pub fn parse(text: &str) -> Vec<Project> {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let mut out: Vec<Project> = v
        .get("projects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let path = PathBuf::from(r.get("path")?.as_str().filter(|s| !s.is_empty())?);
            let name = r.get("name").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| folder_name(&path));
            let order = r.get("order").and_then(Value::as_u64).unwrap_or(u64::from(u32::MAX)) as u32;
            let added_ms = r.get("added_ms").and_then(Value::as_u64).unwrap_or(0);
            Some(Project { path, name, order, added_ms })
        })
        .collect();
    out.sort_by_key(|p| (p.order, p.added_ms));
    out
}

/// The file's text for `list`.
pub fn render(list: &[Project]) -> String {
    let rows: Vec<Value> = list
        .iter()
        .map(|p| json!({"path": p.path.to_string_lossy(), "name": p.name, "order": p.order, "added_ms": p.added_ms}))
        .collect();
    serde_json::to_string_pretty(&json!({"v": 1, "projects": rows})).unwrap_or_default() + "\n"
}

/// The registered projects (the file's; none in the legacy layout).
pub fn read(home: &Home) -> Vec<Project> {
    match home.projects_file() {
        Some(f) => std::fs::read_to_string(f).map(|t| parse(&t)).unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Every row: the home workspace first, then the projects by order.
pub fn list(home: &Home, home_ws: &Path) -> Vec<Row> {
    rows(&read(home), home_ws)
}

/// The part of [`list`] after the read. The home row is the canonical
/// home workspace (its hub runs there: `/var/...` and `/private/var/...`
/// are one folder and one hub id).
pub fn rows(projects: &[Project], home_ws: &Path) -> Vec<Row> {
    let home_ws = canonical(home_ws);
    let home_row = Row { id: crate::hub_id(&home_ws), path: home_ws, name: "bise".into(), home: true, added_ms: 0 };
    std::iter::once(home_row)
        .chain(projects.iter().map(|p| Row { path: p.path.clone(), name: p.name.clone(), id: crate::hub_id(&p.path), home: false, added_ms: p.added_ms }))
        .collect()
}

/// The hub id of the row named `name` (its name or its id), for a
/// message from the hub `own` (bise desktop S2): never `own` itself, so a
/// delivery never goes to the sender's own socket.
pub fn target(rows: &[Row], name: &str, own: &str) -> Result<String, String> {
    let name = name.trim().trim_start_matches('@');
    let row = rows
        .iter()
        .find(|r| r.name == name)
        .or_else(|| rows.iter().find(|r| r.id == name))
        .ok_or_else(|| format!("no project named {name} (sb project list)"))?;
    if row.id == own {
        return Err(format!("{} is this hub itself", row.name));
    }
    Ok(row.id.clone())
}

/// The one writer: under an exclusive lock, read the list, let `f`
/// change it, renumber the orders, write it whole (tmp + rename). `f`'s
/// error writes nothing. A file a newer bise wrote (`v` > 1) is never
/// written over.
pub fn update<T>(home: &Home, f: impl FnOnce(&mut Vec<Project>) -> Result<T, String>) -> Result<T, String> {
    let file = home.projects_file().ok_or_else(|| LEGACY.to_string())?;
    std::fs::create_dir_all(home.root()).map_err(|e| format!("{}: {e}", home.root().display()))?;
    let _lock = lock(&file.with_extension("json.lock"))?;
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    if let Some(v) = serde_json::from_str::<Value>(&text).ok().and_then(|v| v.get("v").and_then(Value::as_u64)) {
        if v > 1 {
            return Err(format!("{} was written by a newer bise (v{v}): not changed", file.display()));
        }
    }
    let mut list = parse(&text);
    let out = f(&mut list)?;
    for (i, p) in list.iter_mut().enumerate() {
        p.order = i as u32;
    }
    crate::prefs::write_atomic(&file, &render(&list)).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(out)
}

/// An exclusive flock on `path`, held until the file is dropped.
fn lock(path: &Path) -> Result<std::fs::File, String> {
    use std::os::unix::io::AsRawFd;
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    // SAFETY: flock on a descriptor this function owns; blocks until free
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(format!("{}: {}", path.display(), std::io::Error::last_os_error()));
    }
    Ok(f)
}

/// Add the workspace at `path` (any form: canonicalized here), the
/// shell around [`add`]: the CLI's `bise project add` and the hub's
/// first start.
pub fn add_path(home: &Home, home_ws: &Path, path: &Path, name: Option<&str>, now_ms: u64) -> Result<Added, String> {
    if !path.is_dir() {
        return Err(format!("{}: no such folder", path.display()));
    }
    let path = canonical(path);
    let places = Places::of(home, home_ws);
    let main_repo = worktree_main(&path);
    update(home, |l| add(l, &path, name, now_ms, &places, main_repo.as_deref()))
}

impl Home {
    /// `projects.json` (bise layout only).
    pub fn projects_file(&self) -> Option<PathBuf> {
        (self.layout == Layout::Bise).then(|| self.root.join(FILE))
    }
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;
