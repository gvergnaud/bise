//! The workspace file index behind the `@` popup: every file and folder
//! under the working directory that `.gitignore` keeps, walked in a
//! background thread, ranked per keystroke (file name before path).
//! Design: docs/at-mentions.md.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Entries past this many are dropped (a home folder, a huge monorepo).
const MAX_ENTRIES: usize = 200_000;
/// An index older than this is walked again when the popup opens.
const STALE: Duration = Duration::from_secs(3);
/// Recent picks kept for the boost.
const RECENT: usize = 32;

/// One file or folder, relative to the root, `/`-separated.
#[derive(Debug)]
pub(crate) struct Entry {
    pub(crate) path: String,
    pub(crate) dir: bool,
    lower: Vec<u8>,
    /// byte offset of the name in `path`
    name: usize,
    depth: u16,
    /// bit `b % 64` set for every byte `b` of `lower`
    mask: u64,
}

impl Entry {
    pub(crate) fn new(path: String, dir: bool) -> Entry {
        let lower = path.to_lowercase().into_bytes();
        let name = lower.iter().rposition(|&b| b == b'/').map(|i| i + 1).unwrap_or(0);
        let depth = path.matches('/').count().min(u16::MAX as usize) as u16;
        Entry { mask: mask(&lower), path, dir, lower, name, depth }
    }

    fn lname(&self) -> &[u8] {
        &self.lower[self.name..]
    }
}

fn mask(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0, |m, &b| m | 1u64 << (b % 64))
}

/// `q` is a subsequence of `hay` (memchr per byte).
fn subseq(mut hay: &[u8], q: &[u8]) -> bool {
    for &c in q {
        match memchr::memchr(c, hay) {
            Some(i) => hay = &hay[i + 1..],
            None => return false,
        }
    }
    true
}

/// Walk `root` as git sees it: `.gitignore`, `.ignore`, `.git/info/exclude`
/// and the global excludes apply (only inside a git repo, like git);
/// dot files are kept, `.git` is not. Files and folders, sorted.
pub(crate) fn walk(root: &Path, cap: usize) -> Vec<Entry> {
    let out = Mutex::new(Vec::new());
    ignore::WalkBuilder::new(root)
        .hidden(false)
        .require_git(true)
        .filter_entry(|e| e.file_name() != ".git")
        .threads(4)
        .build_parallel()
        .run(|| {
            let out = &out;
            Box::new(move |e| {
                let Ok(e) = e else { return ignore::WalkState::Continue };
                let Some(rel) = e.path().strip_prefix(root).ok().and_then(|p| p.to_str()) else {
                    return ignore::WalkState::Continue;
                };
                if rel.is_empty() {
                    return ignore::WalkState::Continue;
                }
                let dir = e.file_type().is_some_and(|t| t.is_dir());
                let mut v = out.lock().unwrap_or_else(|e| e.into_inner());
                if v.len() >= cap {
                    return ignore::WalkState::Quit;
                }
                v.push((rel.replace('\\', "/"), dir));
                ignore::WalkState::Continue
            })
        });
    let mut v = out.into_inner().unwrap_or_else(|e| e.into_inner());
    v.sort();
    v.into_iter().map(|(p, d)| Entry::new(p, d)).collect()
}

/// The entries matching `query`, best first, at most `limit`.
///
/// The query splits at its last `/` into a folder part (a subsequence
/// of the parent path) and a name part. Tiers: exact name, name prefix,
/// name fuzzy, path fuzzy (no folder part only). In a tier: recent
/// picks first, then the fuzzy score, then shallower, shorter, path.
/// Empty name part (`""`, `src/`): the children of that folder only,
/// folders first. Dot files only when the name part starts with `.`.
pub(crate) fn rank(entries: &[Entry], query: &str, recent: &[String], limit: usize) -> Vec<usize> {
    let q = query.to_lowercase();
    let q = q.as_bytes();
    let (qdir, qname) = match q.iter().rposition(|&b| b == b'/') {
        Some(i) => (&q[..=i], &q[i + 1..]),
        None => (&q[..0], q),
    };
    let dots = qname.first() == Some(&b'.');
    let visible = |e: &Entry| dots || e.lname().first() != Some(&b'.');
    let is_recent = |e: &Entry| recent.contains(&e.path);
    // `rust/tui/` names a folder of the index: the search stays inside it
    let scoped = !qdir.is_empty() && is_folder(entries, &qdir[..qdir.len() - 1]);
    if qname.is_empty() {
        return children(entries, qdir, scoped, &visible, &is_recent, limit);
    }
    let qm = mask(q) & !(1u64 << b'/');
    let mut tiers: [Vec<usize>; 4] = Default::default();
    for (i, e) in entries.iter().enumerate() {
        if qm & !e.mask != 0 || !visible(e) {
            continue;
        }
        if scoped && !e.lower.starts_with(qdir) {
            continue;
        }
        let parent = &e.lower[..e.name];
        let dir_ok = qdir.is_empty() || scoped || subseq(parent, qdir);
        let n = e.lname();
        let tier = if dir_ok && n == qname {
            0
        } else if dir_ok && n.starts_with(qname) {
            1
        } else if dir_ok && subseq(n, qname) {
            2
        } else if scoped && subseq(&e.lower[qdir.len()..], qname) {
            3
        } else if !scoped && subseq(&e.lower, if qdir.is_empty() { qname } else { q }) {
            // the whole query along the path (`src/tu` finds `src/tui/`)
            3
        } else {
            continue;
        };
        tiers[tier].push(i);
    }
    let name_query = std::str::from_utf8(qname).unwrap_or("");
    let pat = Pattern::new(name_query, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let mut buf = Vec::new();
    let mut out = Vec::new();
    for (t, ids) in tiers.iter().enumerate() {
        let want = limit.saturating_sub(out.len());
        if want == 0 {
            break;
        }
        // (recent, score, -depth, -len): bigger is better
        let mut v: Vec<((bool, u32, i32, i32), usize)> = ids
            .iter()
            .map(|&i| {
                let e = &entries[i];
                let score = match t {
                    2 => pat.score(Utf32Str::new(&e.path[e.name..], &mut buf), &mut matcher),
                    3 => pat.score(Utf32Str::new(&e.path, &mut buf), &mut matcher),
                    _ => Some(0),
                };
                ((is_recent(e), score.unwrap_or(0), -(e.depth as i32), -(e.path.len() as i32)), i)
            })
            .collect();
        let cmp = |a: &((bool, u32, i32, i32), usize), b: &((bool, u32, i32, i32), usize)| {
            b.0.cmp(&a.0).then(a.1.cmp(&b.1))
        };
        if v.len() > want {
            v.select_nth_unstable_by(want - 1, cmp);
            v.truncate(want);
        }
        v.sort_by(cmp);
        out.extend(v.into_iter().map(|(_, i)| i));
    }
    out
}

/// `lower` (lowercase, no trailing `/`) is a folder of the index.
fn is_folder(entries: &[Entry], lower: &[u8]) -> bool {
    entries.iter().any(|e| e.dir && e.lower == lower)
}

/// The children of folder `dir` (`""`: the root; a folder query like
/// `src/` also matches `rust/tui/src/` unless `scoped`: `dir` is a folder
/// of the index, only its own children), recent then folders first.
fn children(
    entries: &[Entry],
    dir: &[u8],
    scoped: bool,
    visible: &dyn Fn(&Entry) -> bool,
    is_recent: &dyn Fn(&Entry) -> bool,
    limit: usize,
) -> Vec<usize> {
    fn parent(e: &Entry) -> &[u8] {
        &e.lower[..e.name]
    }
    let mut v: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| visible(e))
        .filter(|(_, e)| {
            let p = parent(e);
            if dir.is_empty() || scoped {
                p == dir
            } else {
                p.ends_with(dir) && (p.len() == dir.len() || p[p.len() - dir.len() - 1] == b'/')
            }
        })
        .map(|(i, _)| i)
        .collect();
    // exact folder path first, then the shallowest folders named so
    v.sort_by(|&a, &b| {
        let (x, y) = (&entries[a], &entries[b]);
        (!is_recent(x), parent(x).len() != dir.len(), x.depth, !x.dir, &x.lower)
            .cmp(&(!is_recent(y), parent(y).len() != dir.len(), y.depth, !y.dir, &y.lower))
    });
    v.truncate(limit);
    v
}

/// The `@word` that ends at the cursor: its start (char index of the
/// `@`) and the query typed so far. The `@` opens a word (line start or
/// after a space) and the word has no space yet, or it is an open quote
/// (`@"docs/my notes/`, a folder with a space) not closed yet. None when
/// the cursor is on or before the `@`.
pub(crate) fn token(input: &str, cursor: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    let before = &chars[..cursor.min(chars.len())];
    if let Some(q) = before.iter().rposition(|&c| c == '"') {
        let opens = q >= 1 && before[q - 1] == '@' && (q == 1 || before[q - 2].is_whitespace());
        if opens && !before[q + 1..].contains(&'\n') {
            return Some((q - 1, before[q + 1..].iter().collect()));
        }
    }
    let start = before
        .iter()
        .rposition(|c| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    match before.get(start..) {
        Some(['@', rest @ ..]) => Some((start, rest.iter().collect())),
        _ => None,
    }
}

/// The token text that browses folder `path`: `@path/`, `@"path/` when
/// the path holds a space (the popup lists the folder's entries).
pub(crate) fn browse(path: &str) -> String {
    let slash = if path.ends_with('/') { "" } else { "/" }; // `/`: the root
    if path.is_empty() {
        "@".to_string()
    } else if path.contains(char::is_whitespace) {
        format!("@\"{path}{slash}")
    } else {
        format!("@{path}{slash}")
    }
}

/// One folder up from a query that browses a folder (`rust/tui/` →
/// `rust`, `rust/` → the root `""`, `/usr/` → `/`, `~/` and `/` → the
/// workspace `""`); None when the query does not end with `/`.
pub(crate) fn parent_query(query: &str) -> Option<&str> {
    let q = query.strip_suffix('/')?;
    Some(match q.rfind('/') {
        Some(0) => "/", // `/usr/` → the root
        Some(i) => &q[..i],
        None => "",
    })
}

/// What a picked entry inserts: the relative path (a folder with a
/// trailing `/`), quoted when it holds a space.
pub(crate) fn reference(path: &str, dir: bool) -> String {
    let p = if dir { format!("{path}/") } else { path.to_string() };
    if p.contains(char::is_whitespace) && !p.contains('"') {
        format!("\"{p}\"")
    } else {
        p
    }
}

/// The composer once `ins` replaces the token at `start..cursor`: the
/// new text and the cursor (after `ins` and one space).
pub(crate) fn complete(input: &str, start: usize, cursor: usize, ins: &str) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = start.min(cursor);
    let head: String = chars[..start].iter().collect();
    let mut tail: String = chars[cursor..].iter().collect();
    if tail.starts_with(' ') {
        tail.remove(0);
    }
    let at = start + ins.chars().count() + 1;
    (format!("{head}{ins} {tail}"), at)
}

/// The composer once `tok` replaces the token at `start..cursor`, the
/// popup still open: the new text and the cursor right after `tok`.
pub(crate) fn replace_token(input: &str, start: usize, cursor: usize, tok: &str) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = start.min(cursor);
    let head: String = chars[..start].iter().collect();
    let tail: String = chars[cursor..].iter().collect();
    (format!("{head}{tok}{tail}"), start + tok.chars().count())
}

/// The indexes of the process, one per workspace root (the TUI's own;
/// the window core's projects, desktop C/A/B), each walked in the
/// background, re-walked when stale, plus its recent picks.
struct State {
    root: PathBuf,
    entries: Arc<Vec<Entry>>,
    built: Option<Instant>,
    walking: bool,
    recent: Vec<String>,
}

/// At most this many roots are indexed; the least recently used goes.
const ROOTS: usize = 4;

/// The indexes, the most recently used first.
static STATE: Mutex<Vec<State>> = Mutex::new(Vec::new());

fn state() -> std::sync::MutexGuard<'static, Vec<State>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Index `root` in the background (the TUI calls it at startup): first
/// in the list, a new root walked, the oldest dropped past [`ROOTS`].
pub(crate) fn start(root: PathBuf) {
    {
        let mut g = state();
        match g.iter().position(|s| s.root == root) {
            Some(i) => {
                let s = g.remove(i);
                g.insert(0, s);
            }
            None => {
                g.insert(0, State { root: root.clone(), entries: Arc::new(Vec::new()), built: None, walking: false, recent: Vec::new() });
                g.truncate(ROOTS);
            }
        }
    }
    refresh(&root);
}

/// Walk `root` again in the background when its index is stale and no
/// walk runs.
fn refresh(root: &Path) {
    {
        let mut g = state();
        let Some(s) = g.iter_mut().find(|s| s.root == root) else { return };
        if s.walking || s.built.is_some_and(|t| t.elapsed() < STALE) {
            return;
        }
        s.walking = true;
    }
    let root = root.to_path_buf();
    let _ = std::thread::Builder::new().name("file-index".into()).spawn(move || {
        let entries = Arc::new(walk(&root, MAX_ENTRIES));
        if let Some(s) = state().iter_mut().find(|s| s.root == root) {
            s.entries = entries;
            s.built = Some(Instant::now());
            s.walking = false;
        }
    });
}

/// `root`'s first walk is not over yet (its searches miss files).
pub(crate) fn first_walk(root: &Path) -> bool {
    state().iter().find(|s| s.root == root).is_none_or(|s| s.built.is_none())
}

/// A search hit: the relative path and whether it is a folder
/// (`protected`: a folder macOS guards, see [`protected`]).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hit {
    pub(crate) path: String,
    pub(crate) dir: bool,
    pub(crate) protected: bool,
}

/// The best `limit` entries of `root` for `query` (refreshes a stale
/// index in the background; this call never waits for a walk). A new
/// root starts a new index: its popup shows the files a frame later.
pub(crate) fn search(root: &Path, query: &str, limit: usize) -> Vec<Hit> {
    start(root.to_path_buf());
    let (entries, recent) = match state().iter().find(|s| s.root == root) {
        Some(s) => (s.entries.clone(), s.recent.clone()),
        None => return Vec::new(),
    };
    rank(&entries, query, &recent, limit)
        .into_iter()
        .map(|i| Hit { path: entries[i].path.clone(), dir: entries[i].dir, protected: false })
        .collect()
}

// ---- outside the workspace: `@../`, `@~/`, `@/` (BISE-206) ----
//
// No index: the popup lists the one folder typed up to the last `/`
// (read_dir, never recursive, never ahead of the user), so macOS asks
// for a guarded folder (Desktop, Documents...) only once the user
// enters it. The listings are cached while the popup stays open.

/// Entries read from one folder, at most.
const MAX_DIR_ENTRIES: usize = 500;
/// How long a keystroke waits for a folder being read (a slow volume
/// goes on in the background; the popup fills a frame later).
const LIST_WAIT: Duration = Duration::from_millis(20);

/// `query` names a path outside the workspace index: `../`, `~/`, `/`.
pub(crate) fn outside(query: &str) -> bool {
    query.starts_with('/') || query.starts_with("~/") || query.starts_with("../")
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// The file an outside path names: `~/` from `home`, `/` as is, the
/// rest from `root` (the workspace, the agent's working directory).
fn resolve_in(root: &Path, home: Option<&Path>, path: &str) -> PathBuf {
    match (path.strip_prefix("~/").or((path == "~").then_some("")), home) {
        (Some(rest), Some(h)) => h.join(rest),
        _ => root.join(path),
    }
}

/// What a picked outside path inserts, so the tools read it with no
/// shell: `~/` becomes the home folder; `../` stays relative to the
/// workspace (the agent's working directory), `/` stays absolute.
pub(crate) fn sent_path(path: &str) -> String {
    sent_path_in(home().as_deref(), path)
}

fn sent_path_in(home: Option<&Path>, path: &str) -> String {
    match (path.strip_prefix("~/"), home.and_then(|h| h.to_str())) {
        (Some(rest), Some(h)) => format!("{}/{rest}", h.trim_end_matches('/')),
        _ => path.to_string(),
    }
}

/// A folder macOS guards (TCC: « would like to access files in your
/// Desktop folder »): shown as an entry, read only once entered.
pub(crate) fn protected(home: Option<&Path>, path: &Path) -> bool {
    let guarded = ["Desktop", "Documents", "Downloads", "Library/Mobile Documents", "Library/CloudStorage"];
    let in_home = home.is_some_and(|h| guarded.iter().any(|g| path == h.join(g)));
    let volume = path.parent() == Some(Path::new("/Volumes"));
    in_home || volume
}

/// One folder read: its entries (not sorted), or None when it cannot
/// be read (EPERM, gone, not a folder). A link is a folder when its
/// target is, unless the target is guarded (no stat inside it).
fn list_dir(dir: &Path, home: Option<&Path>, cap: usize) -> Option<Vec<Entry>> {
    #[cfg(test)]
    tests::LISTED.lock().unwrap_or_else(|e| e.into_inner()).push(dir.to_path_buf());
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).ok()?.flatten().take(cap) {
        let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
        let Ok(t) = e.file_type() else { continue };
        let dir = if t.is_symlink() {
            let target = std::fs::read_link(e.path()).map(|t| dir.join(t));
            let guarded = target.as_ref().map_or(true, |t| {
                protected(home, t) || t.ancestors().any(|a| protected(home, a))
            });
            !guarded && std::fs::metadata(e.path()).is_ok_and(|m| m.is_dir())
        } else {
            t.is_dir()
        };
        out.push(Entry::new(name, dir));
    }
    Some(out)
}

/// A folder's listing: its entries, or None (cannot be read: locked).
type Listing = Option<Arc<Vec<Entry>>>;

/// The folders read while the popup is open (None: being read).
static DIRS: Mutex<Option<std::collections::HashMap<PathBuf, Option<Listing>>>> = Mutex::new(None);

fn dirs() -> std::sync::MutexGuard<'static, Option<std::collections::HashMap<PathBuf, Option<Listing>>>> {
    DIRS.lock().unwrap_or_else(|e| e.into_inner())
}

/// The popup closed: the next one reads its folders again.
pub(crate) fn forget_dirs() {
    if let Some(m) = dirs().as_mut() {
        m.retain(|_, l| l.is_none()); // a read in flight fills in later
    }
}

/// The listing of `dir`, read in the background once per popup; waits
/// at most [`LIST_WAIT`] for it. None: not read yet.
fn listing(dir: &Path, home: Option<&Path>) -> Option<Listing> {
    if let Some(l) = dirs().get_or_insert_with(Default::default).get(dir) {
        return l.clone();
    }
    dirs().get_or_insert_with(Default::default).insert(dir.to_path_buf(), None);
    let (tx, rx) = std::sync::mpsc::channel();
    let (d, h) = (dir.to_path_buf(), home.map(Path::to_path_buf));
    let spawned = std::thread::Builder::new().name("at-list".into()).spawn(move || {
        let l: Listing = list_dir(&d, h.as_deref(), MAX_DIR_ENTRIES).map(Arc::new);
        dirs().get_or_insert_with(Default::default).insert(d, Some(l.clone()));
        let _ = tx.send(l);
    });
    if spawned.is_err() {
        dirs().get_or_insert_with(Default::default).remove(dir);
        return None;
    }
    rx.recv_timeout(LIST_WAIT).ok()
}

/// The popup rows for an outside query.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Outside {
    /// the entries of the typed folder matching the name part, as typed
    /// (`~/Documents`, `../other/a.md`), folders first when browsing
    pub(crate) hits: Vec<Hit>,
    /// the typed folder cannot be read (EPERM...)
    pub(crate) locked: bool,
    /// the typed folder is still being read
    pub(crate) loading: bool,
}

/// The entries of the folder `query` names up to its last `/` that
/// match the rest (ranked like the index; dot files only when the rest
/// starts with `.`). Reads that one folder, nothing else.
pub(crate) fn search_outside(root: &Path, query: &str, limit: usize) -> Outside {
    search_outside_in(root, home().as_deref(), query, limit)
}

fn search_outside_in(root: &Path, home: Option<&Path>, query: &str, limit: usize) -> Outside {
    let Some(cut) = query.rfind('/') else { return Outside::default() };
    let (typed, name) = query.split_at(cut + 1);
    let dir = resolve_in(root, home, typed);
    let Some(l) = listing(&dir, home) else { return Outside { loading: true, ..Outside::default() } };
    let Some(entries) = l else { return Outside { locked: true, ..Outside::default() } };
    let hits = rank(&entries, name, &[], limit)
        .into_iter()
        .map(|i| {
            let e = &entries[i];
            let protected = e.dir && protected(home, &dir.join(&e.path));
            Hit { path: format!("{typed}{}", e.path), dir: e.dir, protected }
        })
        .collect();
    Outside { hits, locked: false, loading: false }
}

/// An `@` query's rows (the TUI's popup and the window core's `files`,
/// desktop C/A/B: one ranking, no copy): the hits, the browsed folder's
/// own row (`@src/`: ⏎ still inserts a reference to the folder),
/// whether the query is outside the workspace (`@../`, `@~/`, `@/`: its
/// paths go out in a form the tools read, [`sent_path`]) and whether
/// that folder can't be read.
pub(crate) struct Pick {
    pub(crate) hits: Vec<Hit>,
    pub(crate) this: Option<String>,
    pub(crate) outside: bool,
    pub(crate) locked: bool,
}

/// The rows for `q` in `root`, at most `limit` with the folder's row.
pub(crate) fn pick(root: &Path, q: &str, limit: usize) -> Pick {
    let outside = outside(q);
    let (mut hits, locked) = if outside {
        let o = search_outside(root, q, limit);
        (o.hits, o.locked)
    } else {
        (search(root, q, limit), false)
    };
    // browsing a folder: the folder itself last (↑ from the first row)
    let this = if outside {
        // even an empty or locked one: it is what the user typed
        parent_query(q).map(|_| q.strip_suffix('/').filter(|p| !p.is_empty()).unwrap_or("/"))
    } else {
        parent_query(q)
            .and_then(|_| hits.first())
            .and_then(|h| h.path.rsplit_once('/'))
            .map(|(parent, _)| parent)
            .filter(|parent| parent.to_lowercase() == q.trim_end_matches('/').to_lowercase())
    }
    .map(|parent| parent.to_string());
    if this.is_some() {
        hits.truncate(limit.saturating_sub(1));
    }
    Pick { hits, this, outside, locked }
}

/// Remember a picked path (boosted in the next searches).
pub(crate) fn picked(path: &str) {
    if let Some(s) = state().first_mut() {
        remember(s, path);
    }
}

/// Remember a path picked in `root`'s list (the window core's projects,
/// desktop R14): on that root's own index, never the last one searched.
/// A root with no index (never searched, or evicted past [`ROOTS`])
/// remembers nothing: a no-op, as its picks went with it.
pub(crate) fn picked_in(root: &Path, path: &str) {
    if let Some(s) = state().iter_mut().find(|s| s.root == root) {
        remember(s, path);
    }
}

fn remember(s: &mut State, path: &str) {
    s.recent.retain(|p| p != path);
    s.recent.insert(0, path.to_string());
    s.recent.truncate(RECENT);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every folder `list_dir` read (the protected-folder tests).
    pub(super) static LISTED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

    fn index(paths: &[&str]) -> Vec<Entry> {
        paths
            .iter()
            .map(|p| match p.strip_suffix('/') {
                Some(d) => Entry::new(d.to_string(), true),
                None => Entry::new(p.to_string(), false),
            })
            .collect()
    }

    fn top<'a>(e: &'a [Entry], q: &str, recent: &[&str]) -> Vec<&'a str> {
        let recent: Vec<String> = recent.iter().map(|s| s.to_string()).collect();
        rank(e, q, &recent, 10).into_iter().map(|i| e[i].path.as_str()).collect()
    }

    const TREE: &[&str] = &[
        "Makefile",
        "README.md",
        ".gitignore",
        "rust/",
        "rust/tui/",
        "rust/tui/src/",
        "rust/tui/src/app.rs",
        "rust/tui/src/sb/",
        "rust/tui/src/sb/mention.rs",
        "rust/tui/src/main.rs",
        "hub/main.bend",
        "docs/main-notes/readme.txt",
        "projects/switchboard/docs/at-mentions.md",
    ];

    #[test]
    fn file_name_beats_path() {
        let e = index(TREE);
        // exact name, then name prefix, then the path-only match
        assert_eq!(top(&e, "main.rs", &[]), ["rust/tui/src/main.rs"]);
        assert_eq!(top(&e, "main", &[])[..2], ["hub/main.bend", "rust/tui/src/main.rs"]);
        assert_eq!(top(&e, "mention", &[]), ["rust/tui/src/sb/mention.rs", "projects/switchboard/docs/at-mentions.md"]);
        // "sbmen" only matches along a path: the tighter match first
        assert_eq!(top(&e, "sbmen", &[])[0], "rust/tui/src/sb/mention.rs");
        // case-insensitive; same tier: the shallower first
        assert_eq!(top(&e, "readme", &[])[..2], ["README.md", "docs/main-notes/readme.txt"]);
        assert!(top(&e, "zzz", &[]).is_empty());
    }

    #[test]
    fn exact_beats_prefix_beats_fuzzy() {
        let e = index(&["a/src.rs", "b/src/", "c/sourcery.rs", "d/xsrc.rs"]);
        assert_eq!(top(&e, "src", &[]), ["b/src", "a/src.rs", "c/sourcery.rs", "d/xsrc.rs"]);
    }

    #[test]
    fn folder_part_filters_the_parent_path() {
        let e = index(TREE);
        // the folder part is fuzzy too ("sb/" in "projects/switchboard/docs/")
        assert_eq!(top(&e, "sb/me", &[]), ["rust/tui/src/sb/mention.rs", "projects/switchboard/docs/at-mentions.md"]);
        assert_eq!(top(&e, "docs/me", &[])[0], "projects/switchboard/docs/at-mentions.md");
        assert_eq!(top(&e, "tui/src/m", &[]), ["rust/tui/src/main.rs", "rust/tui/src/sb/mention.rs"]);
        // folder then empty name: the children of that folder, folders first
        assert_eq!(top(&e, "rust/tui/src/", &[]), ["rust/tui/src/sb", "rust/tui/src/app.rs", "rust/tui/src/main.rs"]);
        assert_eq!(top(&e, "src/", &[]), ["rust/tui/src/sb", "rust/tui/src/app.rs", "rust/tui/src/main.rs"]);
    }

    #[test]
    fn empty_query_lists_the_root_dot_files_hidden() {
        let e = index(TREE);
        assert_eq!(top(&e, "", &[]), ["rust", "Makefile", "README.md"]);
        assert_eq!(top(&e, ".git", &[]), [".gitignore"]);
        assert!(!top(&e, "gitig", &[]).contains(&".gitignore"));
    }

    #[test]
    fn recent_picks_come_first_in_their_tier() {
        let e = index(TREE);
        assert_eq!(top(&e, "main", &["rust/tui/src/main.rs"])[..2], ["rust/tui/src/main.rs", "hub/main.bend"]);
        assert_eq!(top(&e, "", &["README.md"])[0], "README.md");
        // a recent path-only match does not jump over a name match
        assert_eq!(top(&e, "mention", &["projects/switchboard/docs/at-mentions.md"])[0], "rust/tui/src/sb/mention.rs");
    }

    /// R14 (architect m_12214): a window's pick lands on its own root's
    /// index, never the root searched last; a re-pick moves to the front;
    /// a root with no index (evicted) is a no-op.
    #[test]
    fn a_pick_in_a_root_is_remembered_on_that_root_only() {
        let (a, b) = (PathBuf::from("/r14/project-a"), PathBuf::from("/r14/project-b"));
        let recent = |r: &Path| state().iter().find(|s| s.root == r).map(|s| s.recent.clone());
        {
            let mut g = state();
            for r in [&a, &b] {
                g.retain(|s| s.root != *r);
                g.insert(0, State { root: r.clone(), entries: Arc::new(Vec::new()), built: None, walking: false, recent: Vec::new() });
            }
        }
        // b was searched last (first in the list): a's pick still goes to a
        picked_in(&a, "src/b.rs");
        picked_in(&a, "src/c.rs");
        picked_in(&a, "src/b.rs");
        assert_eq!(recent(&a).unwrap(), ["src/b.rs", "src/c.rs"]);
        assert_eq!(recent(&b).unwrap(), Vec::<String>::new());
        // an evicted (or never searched) root: nothing, no index made
        picked_in(Path::new("/r14/gone"), "x.rs");
        assert!(recent(Path::new("/r14/gone")).is_none());
        state().retain(|s| s.root != a && s.root != b);
    }

    #[test]
    fn walk_respects_gitignore() {
        let root = std::env::temp_dir().join(format!("at-files-walk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["src", "target/debug", "node_modules/x", ".git"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        for (f, body) in [
            (".gitignore", "target/\nnode_modules\n*.log\n"),
            ("src/lib.rs", ""),
            ("src/app.log", ""),
            ("target/debug/bin", ""),
            ("node_modules/x/i.js", ""),
            (".env.example", ""),
            (".git/HEAD", "ref: refs/heads/main\n"),
        ] {
            std::fs::write(root.join(f), body).unwrap();
        }
        let got: Vec<(String, bool)> = walk(&root, 100).into_iter().map(|e| (e.path, e.dir)).collect();
        let _ = std::fs::remove_dir_all(&root);
        let want = [(".env.example", false), (".gitignore", false), ("src", true), ("src/lib.rs", false)];
        assert_eq!(got, want.map(|(p, d)| (p.to_string(), d)));
    }

    #[test]
    fn token_is_the_at_word_at_the_cursor() {
        assert_eq!(token("@", 1), Some((0, String::new())));
        assert_eq!(token("@ma", 3), Some((0, "ma".into())));
        assert_eq!(token("look at @src/ma", 15), Some((8, "src/ma".into())));
        assert_eq!(token("look at @src/ma now", 15), Some((8, "src/ma".into())));
        assert_eq!(token("look at @src/ma now", 19), None); // cursor past the word
        assert_eq!(token("mail a@b.c", 10), None); // mid-word @
        assert_eq!(token("@main ", 6), None); // done: a space follows
        assert_eq!(token("line\n@ap", 8), Some((5, "ap".into())));
    }

    #[test]
    fn token_is_total_and_reads_an_open_quote() {
        // the cursor on the `@` (← after typing it): no token, no panic
        assert_eq!(token("@", 0), None);
        assert_eq!(token("see @ru", 4), None);
        assert_eq!(token("", 5), None);
        // an open quote: a folder with a space being browsed
        assert_eq!(token("@\"docs/my notes/", 16), Some((0, "docs/my notes/".into())));
        assert_eq!(token("see @\"docs/my notes/a", 21), Some((4, "docs/my notes/a".into())));
        // a closed quote ends it; a quote mid-word is not an opener
        assert_eq!(token("@\"docs/my notes/a.md\" ", 22), None);
        assert_eq!(token("say \"@x", 7), None);
    }

    #[test]
    fn browse_and_parent_query() {
        assert_eq!(browse("rust/tui"), "@rust/tui/");
        assert_eq!(browse("docs/my notes"), "@\"docs/my notes/");
        assert_eq!(browse(""), "@");
        assert_eq!(parent_query("rust/tui/"), Some("rust"));
        assert_eq!(parent_query("rust/"), Some(""));
        assert_eq!(parent_query("rust/tu"), None);
        assert_eq!(parent_query(""), None);
        assert_eq!(replace_token("see @ru now", 4, 7, "@rust/"), ("see @rust/ now".into(), 10));
        assert_eq!(replace_token("@", 5, 9, "@x/"), ("@@x/".into(), 4)); // out of range: clamped, no panic
    }

    #[test]
    fn a_folder_path_scopes_the_search() {
        let e = index(&[
            "rust/",
            "rust/tui/",
            "rust/tui/src/",
            "rust/tui/src/files.rs",
            "rust/tui/src/main.rs",
            "rust/tui/Cargo.toml",
            "rust/other/",
            "rust/other/tui/",
            "rust/other/tui/files.rs",
            "vendor/rust/tui/x.rs",
        ]);
        // an exact folder: its own children only (not vendor/rust/tui/)
        assert_eq!(top(&e, "rust/tui/", &[]), ["rust/tui/src", "rust/tui/Cargo.toml"]);
        // a name inside it: its descendants only
        assert_eq!(top(&e, "rust/tui/fi", &[]), ["rust/tui/src/files.rs"]);
        // not a folder path: fuzzy as before, and the whole query along
        // the path finds folders (`src/fi`, `tu/sr`)
        assert_eq!(top(&e, "tui/", &[]), ["rust/tui/src", "rust/tui/Cargo.toml", "rust/other/tui/files.rs", "vendor/rust/tui/x.rs"]);
        assert_eq!(top(&e, "rust/tu", &[])[..2], ["rust/tui", "rust/other/tui"]);
        assert!(top(&e, "tu/sr", &[]).contains(&"rust/tui/src"));
    }

    #[test]
    fn completion_inserts_the_reference() {
        assert_eq!(reference("src/app.rs", false), "src/app.rs");
        assert_eq!(reference("rust/tui", true), "rust/tui/");
        assert_eq!(reference("docs/my notes.md", false), "\"docs/my notes.md\"");
        assert_eq!(complete("see @ap", 4, 7, "src/app.rs"), ("see src/app.rs ".into(), 15));
        assert_eq!(complete("see @ap now", 4, 7, "src/app.rs"), ("see src/app.rs now".into(), 15));
        let (t, c) = complete("@ma", 0, 3, "hub/main.bend");
        assert_eq!((t.as_str(), c), ("hub/main.bend ", 14));
        assert_eq!(token(&t, c), None); // popup closes
    }

    /// Per-keystroke latency on a big repo (release):
    /// `AT_FILES_BENCH=~/mistral/dashboard cargo test --release -p bend-tui files::tests::bench -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench() {
        let root = std::env::var("AT_FILES_BENCH").unwrap_or_else(|_| ".".into());
        let t = Instant::now();
        let e = walk(Path::new(&root), MAX_ENTRIES);
        println!("walk {root}: {} entries in {:.1?}", e.len(), t.elapsed());
        let mut worst = Duration::ZERO;
        for q in ["", "m", "ma", "mai", "main", "main.rs", "src/", "src/comp", "README", "c", "co", "com", "comp", "compose", "composer", "fidx", "zzzq"] {
            let t = Instant::now();
            for _ in 0..10 {
                rank(&e, q, &[], 50);
            }
            let d = t.elapsed() / 10;
            worst = worst.max(d);
            println!("{:>12} {:>9.2?}", format!("{q:?}"), d);
        }
        println!("worst {worst:.2?}");
    }

    // ---- outside the workspace (BISE-206) ----

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("at-out-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The outside search once its folder is read (the first keystroke
    /// may return before the background read ends).
    fn outside_rows(root: &Path, home: Option<&Path>, q: &str) -> Outside {
        for _ in 0..500 {
            let o = search_outside_in(root, home, q, 50);
            if !o.loading {
                return o;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("{q} never read")
    }

    fn paths(o: &Outside) -> Vec<&str> {
        o.hits.iter().map(|h| h.path.as_str()).collect()
    }

    fn listed(dir: &Path) -> bool {
        LISTED.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|d| d == dir)
    }

    #[test]
    fn outside_lists_the_typed_folder_only() {
        let up = tmp("up");
        for d in ["ws/src", "other/deep", "zeta"] {
            std::fs::create_dir_all(up.join(d)).unwrap();
        }
        for f in ["b.md", "a.txt", ".env", "other/o.rs"] {
            std::fs::write(up.join(f), "").unwrap();
        }
        let ws = up.join("ws");
        assert!(outside("../") && outside("~/") && outside("/") && !outside("src/") && !outside(".."));
        // `../`: the parent's entries, folders first, dot files hidden
        let o = outside_rows(&ws, None, "../");
        assert_eq!(paths(&o), ["../other", "../ws", "../zeta", "../a.txt", "../b.md"]);
        assert!(o.hits[..3].iter().all(|h| h.dir && !h.protected));
        // the name part narrows; dot files once it starts with `.`
        assert_eq!(paths(&outside_rows(&ws, None, "../b")), ["../b.md"]);
        assert_eq!(paths(&outside_rows(&ws, None, "../.e")), ["../.env"]);
        // a folder not entered is not read; entered, only it
        assert!(!listed(&up.join("other")) && !listed(&up.join("other/deep")));
        assert_eq!(paths(&outside_rows(&ws, None, "../other/")), ["../other/deep", "../other/o.rs"]);
        assert!(listed(&ws.join("../other/")) && !listed(&up.join("other/deep")));
        // `/`: absolute, typed as is
        let abs = format!("{}/", up.join("other").display());
        assert_eq!(paths(&outside_rows(&ws, None, &abs)), [format!("{abs}deep"), format!("{abs}o.rs")]);
        let _ = std::fs::remove_dir_all(&up);
    }

    #[test]
    fn protected_folders_are_read_only_once_entered() {
        let home = tmp("home");
        for d in ["Desktop", "Documents", "Downloads", "Library/Mobile Documents", "code"] {
            std::fs::create_dir_all(home.join(d)).unwrap();
        }
        std::fs::write(home.join("Desktop/secret.txt"), "").unwrap();
        let ws = home.join("code");
        let h = Some(home.as_path());
        let o = outside_rows(&ws, h, "~/");
        let prot: Vec<&str> = o.hits.iter().filter(|x| x.protected).map(|x| x.path.as_str()).collect();
        assert_eq!(prot, ["~/Desktop", "~/Documents", "~/Downloads"]);
        assert!(!o.hits.iter().any(|x| x.path == "~/code" && x.protected));
        // typing its name, even whole, reads nothing inside
        for q in ["~/Desk", "~/Desktop", "~/Library/", "~/Library/Mob"] {
            let o = outside_rows(&ws, h, q);
            assert!(!listed(&home.join("Desktop")), "{q} read Desktop");
            if q == "~/Library/" {
                assert!(o.hits[0].protected && o.hits[0].path == "~/Library/Mobile Documents");
            }
        }
        assert!(!listed(&home.join("Library/Mobile Documents")));
        // the `/` after it enters it: read now
        assert_eq!(paths(&outside_rows(&ws, h, "~/Desktop/")), ["~/Desktop/secret.txt"]);
        assert!(listed(&home.join("Desktop/")));
        assert!(protected(None, Path::new("/Volumes/Backup")));
        assert!(!protected(None, Path::new("/Volumes")) && !protected(h, &home.join("code")));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_folder_that_cannot_be_read_is_locked_and_empty() {
        use std::os::unix::fs::PermissionsExt;
        let up = tmp("eperm");
        let lock = up.join("Mail");
        std::fs::create_dir_all(lock.join("inbox")).unwrap();
        std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read_dir(&lock).is_ok() {
            // root reads anything: nothing to test
            let _ = std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o755));
            return;
        }
        let q = format!("{}/", lock.display());
        let o = outside_rows(&up, None, &q);
        assert!(o.locked && o.hits.is_empty());
        assert!(outside_rows(&up, None, "/no/such/folder/").locked);
        let _ = std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o755));
        let _ = std::fs::remove_dir_all(&up);
    }

    #[test]
    fn the_sent_path_expands_home_and_keeps_the_rest() {
        let h = Some(Path::new("/Users/me"));
        assert_eq!(sent_path_in(h, "~/Documents/a b.md"), "/Users/me/Documents/a b.md");
        assert_eq!(sent_path_in(Some(Path::new("/Users/me/")), "~/x"), "/Users/me/x");
        assert_eq!(sent_path_in(h, "../other/o.rs"), "../other/o.rs");
        assert_eq!(sent_path_in(h, "/etc/hosts"), "/etc/hosts");
        assert_eq!(sent_path_in(None, "~/x"), "~/x");
        assert_eq!(resolve_in(Path::new("/ws"), h, "~/Desktop/"), Path::new("/Users/me/Desktop/"));
        assert_eq!(resolve_in(Path::new("/ws"), h, "../o/"), Path::new("/ws/../o/"));
        assert_eq!(resolve_in(Path::new("/ws"), h, "/usr/"), Path::new("/usr/"));
        // browsing: `/` is the root, one folder up from `/usr/` is `/`
        assert_eq!(browse("/"), "@/");
        assert_eq!(browse("~/Documents"), "@~/Documents/");
        assert_eq!(parent_query("/usr/"), Some("/"));
        assert_eq!(parent_query("/usr/lib/"), Some("/usr"));
        assert_eq!(parent_query("~/Documents/"), Some("~"));
        assert_eq!(parent_query("../../"), Some(".."));
    }
}
