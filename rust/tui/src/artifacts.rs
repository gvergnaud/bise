//! Artifacts (site/m/artifacts, B with C's doors and E): what the
//! agents made for you to look at, as the hub lists them (its
//! `artifacts` event, docs/artifacts.md). This module is the data side
//! of the TUI: the list the hub sent, the words of a row (kind, age,
//! group), the search, what a link or a path names, and how an artifact
//! opens. The full screen is `artifacts_screen.rs`; the chips are drawn
//! by markdown.rs and render.rs from [`chip_title`] and [`resolve`].
//!
//! The list lives in a thread-local store (like file_links' folders): the
//! feed's rows are built without the app, and a chip needs the title of
//! the artifact a link names.

use serde_json::Value;
use std::cell::RefCell;

/// One version of an artifact: a bise page's own, or a file added again.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Version {
    pub(crate) v: u32,
    pub(crate) ts_ms: u64,
    /// the path or link as it was added
    pub(crate) target: String,
    /// bise's copy of that version (a file), when it kept one
    pub(crate) copy: Option<String>,
    /// `3 notes done`, `2 notes open`, or ""
    pub(crate) note: String,
}

/// A PR artifact: ⏎ opens its diff, `o` GitHub.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Pr {
    pub(crate) repo: String,
    pub(crate) number: u64,
    pub(crate) branch: String,
}

/// One artifact, as the hub sent it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Artifact {
    pub(crate) id: String,
    pub(crate) title: String,
    /// page, site, doc, sheet, slides, code, image, video, sound, pr,
    /// release, link
    pub(crate) kind: String,
    /// the agent it belongs to ("" when you added it with no agent)
    pub(crate) agent: String,
    /// who added it: `you` or the agent
    pub(crate) by: String,
    pub(crate) archived: bool,
    /// its last version's time
    pub(crate) ts_ms: u64,
    /// the current version
    pub(crate) v: u32,
    /// the current version's path or link
    pub(crate) target: String,
    pub(crate) copy: Option<String>,
    /// the file is gone from disk (never a link)
    pub(crate) gone: bool,
    /// the short last column: `2 notes open`, `127.0.0.1:4747`, `open`
    pub(crate) detail: String,
    pub(crate) pr: Option<Pr>,
    /// every link or path that names it (E: a plain link in a reply)
    pub(crate) keys: Vec<String>,
    /// oldest first
    pub(crate) versions: Vec<Version>,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn n(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(|x| x.as_u64()).unwrap_or(0)
}

fn opt(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(String::from)
}

impl Artifact {
    /// One row of the hub's `artifacts` event; None without an id.
    pub(crate) fn of(v: &Value) -> Option<Artifact> {
        let id = s(v, "id");
        if id.is_empty() {
            return None;
        }
        let mut versions: Vec<Version> = v
            .get("versions")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .map(|x| Version {
                        v: n(x, "v") as u32,
                        ts_ms: n(x, "ts_ms"),
                        target: s(x, "target"),
                        copy: opt(x, "copy"),
                        note: s(x, "note"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        versions.sort_by_key(|x| x.v);
        let pr = v.get("pr").filter(|p| p.is_object()).map(|p| Pr {
            repo: s(p, "repo"),
            number: n(p, "number"),
            branch: s(p, "branch"),
        });
        let title = s(v, "title");
        Some(Artifact {
            title: if title.is_empty() { id.clone() } else { title },
            id,
            kind: s(v, "kind"),
            agent: s(v, "agent"),
            by: s(v, "by"),
            archived: v.get("archived").and_then(|x| x.as_bool()).unwrap_or(false),
            ts_ms: n(v, "ts_ms"),
            v: (n(v, "v") as u32).max(1),
            target: s(v, "target"),
            copy: opt(v, "copy"),
            gone: v.get("gone").and_then(|x| x.as_bool()).unwrap_or(false),
            detail: s(v, "detail"),
            pr,
            keys: v
                .get("keys")
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|k| k.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            versions,
        })
    }

    /// The kind as a row says it: `PR` in capitals, the rest as is.
    pub(crate) fn kind_word(&self) -> String {
        kind_word(&self.kind)
    }

    /// Has more than one version: the row shows `v3`.
    pub(crate) fn versioned(&self) -> bool {
        self.v > 1 || self.versions.len() > 1
    }

    /// The agent's column: `subs-lead · archived`.
    pub(crate) fn agent_words(&self) -> String {
        let who = if self.agent.is_empty() { self.by.clone() } else { self.agent.clone() };
        if self.archived {
            format!("{} · archived", who)
        } else {
            who
        }
    }

    /// The last column at 150: `v3 · 2 notes open`, `127.0.0.1:4747`,
    /// `▲ gone from disk · bise kept a copy`.
    pub(crate) fn last_words(&self) -> String {
        if self.gone {
            return gone_words(self.copy.is_some());
        }
        let v = self.versioned().then(|| format!("v{}", self.v));
        match (v, self.detail.is_empty()) {
            (Some(v), true) => v,
            (Some(v), false) => format!("{} · {}", v, self.detail),
            (None, _) => self.detail.clone(),
        }
    }

    /// A link (http, https), not a file.
    pub(crate) fn is_link(&self) -> bool {
        is_url(&self.target)
    }

    /// What opens: the version `v` (None: the current one), its copy
    /// when the file is gone.
    pub(crate) fn open_target(&self, v: Option<u32>) -> String {
        let ver = v.and_then(|v| self.versions.iter().find(|x| x.v == v));
        let (target, copy) = match ver {
            Some(x) => (x.target.clone(), x.copy.clone()),
            None => (self.target.clone(), self.copy.clone()),
        };
        if is_url(&target) {
            return target;
        }
        let here = !target.is_empty() && std::path::Path::new(&target).exists();
        match (here, copy) {
            (false, Some(c)) => c,
            _ => target,
        }
    }

    /// Where it lives, short, for the line under the list (designer,
    /// m_7220): a link without its scheme `127.0.0.1:47438/p/pricing-page`;
    /// a file in the workspace `ws` from it `docs/q3-plan.md`, elsewhere
    /// with `~`.
    pub(crate) fn where_words(&self, ws: &str) -> String {
        let ws = ws.trim_end_matches('/');
        match self.target.strip_prefix(ws).and_then(|r| r.strip_prefix('/')) {
            Some(rel) if !ws.is_empty() && !is_url(&self.target) => rel.to_string(),
            _ => short_target(&self.target),
        }
    }
}

/// `▲ gone from disk · bise kept a copy`, or without a copy `▲ gone from disk`.
pub(crate) fn gone_words(copy: bool) -> String {
    let g = crate::theme::glyph(crate::theme::G_INTERRUPTED);
    if copy {
        format!("{} gone from disk · bise kept a copy", g)
    } else {
        format!("{} gone from disk", g)
    }
}

pub(crate) fn kind_word(kind: &str) -> String {
    bise_proto::thread::words::kind_word(kind)
}

pub(crate) fn is_url(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

/// A target without its scheme, `~` for the home folder.
pub(crate) fn short_target(t: &str) -> String {
    let l = t.to_ascii_lowercase();
    if l.starts_with("https://") {
        return t[8..].trim_end_matches('/').to_string();
    }
    if l.starts_with("http://") {
        return t[7..].trim_end_matches('/').to_string();
    }
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && t.starts_with(&h) => format!("~{}", &t[h.len()..]),
        _ => t.to_string(),
    }
}

// ---- the store ----

#[derive(Default)]
struct Store {
    rows: Vec<Artifact>,
    /// added since you last looked (the header's `↗ 3 new`)
    new: u64,
    /// when you last looked (the hub's `seen_ms`; an older hub: none)
    seen_ms: Option<u64>,
}

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(Store::default());
    /// the hub's workspace: a file in it shows from it (`docs/plan.md`)
    static WORKSPACE: RefCell<String> = const { RefCell::new(String::new()) };
}

pub(crate) fn set_workspace(ws: &str) {
    WORKSPACE.with(|w| *w.borrow_mut() = ws.to_string());
}

pub(crate) fn workspace() -> String {
    WORKSPACE.with(|w| w.borrow().clone())
}

/// The hub's `artifacts` event: the whole list, newest first.
pub(crate) fn set_from(v: &Value) {
    let mut rows: Vec<Artifact> =
        v.get("rows").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(Artifact::of).collect()).unwrap_or_default();
    rows.sort_by_key(|a| std::cmp::Reverse(a.ts_ms));
    let new = n(v, "new");
    let seen_ms = v.get("seen_ms").and_then(|x| x.as_u64());
    STORE.with(|st| {
        let mut st = st.borrow_mut();
        st.rows = rows;
        st.new = new;
        st.seen_ms = seen_ms;
    });
}

/// When you last looked, as the hub said (none from an older hub).
pub(crate) fn seen_ms() -> Option<u64> {
    STORE.with(|st| st.borrow().seen_ms)
}

/// The list, newest first.
pub(crate) fn all() -> Vec<Artifact> {
    STORE.with(|st| st.borrow().rows.clone())
}


pub(crate) fn get(id: &str) -> Option<Artifact> {
    STORE.with(|st| st.borrow().rows.iter().find(|a| a.id == id).cloned())
}

pub(crate) fn new_count() -> u64 {
    STORE.with(|st| st.borrow().new)
}

/// The new rows, newest first (the header's `↗ designer · pricing
/// page`): the hub counts the rows whose current version came after you
/// last looked, your own adds left out, so they are the first `new` of
/// the list (newest first) that you did not add.
pub(crate) fn new_rows() -> Vec<Artifact> {
    STORE.with(|st| {
        let st = st.borrow();
        st.rows.iter().filter(|a| a.by != "you").take(st.new as usize).cloned().collect()
    })
}

/// You looked: the header's count goes (the hub is told too).
pub(crate) fn mark_seen() {
    STORE.with(|st| st.borrow_mut().new = 0);
}

/// The hub's `seen` op, with when you looked: a request that waits in
/// the socket (a hub still booting) never swallows what came after.
pub(crate) fn seen_op() -> serde_json::Value {
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    serde_json::json!({"op": "artifacts", "do": "seen", "at_ms": at})
}



#[cfg(test)]
pub(crate) fn set_for_test(rows: Vec<Artifact>, new: u64) {
    STORE.with(|st| {
        let mut st = st.borrow_mut();
        st.rows = rows;
        st.new = new;
    });
}

// ---- what a link names (E) ----

/// The artifact (and version) an `artifact:` url names:
/// `artifact:pricing-page`, `artifact:pricing-page@v3`.
pub(crate) fn parse_url(url: &str) -> Option<(String, Option<u32>)> {
    let rest = url.strip_prefix("artifact:")?;
    let rest = rest.trim_start_matches("//");
    let (id, v) = match rest.rsplit_once('@') {
        Some((id, v)) => (id, v.trim_start_matches('v').parse::<u32>().ok()),
        None => (rest, None),
    };
    (!id.is_empty()).then(|| (id.to_string(), v))
}

/// The url of an artifact (a version of it): what a chip links to.
pub(crate) fn url_of(id: &str, v: Option<u32>) -> String {
    match v {
        Some(v) => format!("artifact:{}@v{}", id, v),
        None => format!("artifact:{}", id),
    }
}

/// A link or a path met in a reply: the artifact registered under it
/// (its keys, its targets), compared without the scheme, a trailing
/// slash, `./`, or a line number.
pub(crate) fn resolve(text: &str) -> Option<String> {
    let want = norm(text);
    if want.is_empty() {
        return None;
    }
    STORE.with(|st| {
        st.borrow()
            .rows
            .iter()
            .find(|a| {
                a.keys.iter().chain(std::iter::once(&a.target)).chain(a.versions.iter().map(|v| &v.target)).any(|k| {
                    let k = norm(k);
                    !k.is_empty() && (k == want || (k.starts_with('/') && !want.starts_with('/') && want.contains('/') && k.ends_with(&format!("/{}", want))))
                })
            })
            .map(|a| a.id.clone())
    })
}

fn norm(s: &str) -> String {
    let t = s.trim();
    let l = t.to_ascii_lowercase();
    let t = if l.starts_with("https://") {
        &t[8..]
    } else if l.starts_with("http://") || l.starts_with("file://") {
        &t[7..]
    } else {
        t
    };
    let t = t.strip_prefix("./").unwrap_or(t);
    let t = t.strip_prefix("www.").unwrap_or(t);
    t.trim_end_matches('/').to_string()
}

/// What the key bar says of a chip under the mouse (E, hovered): `click
/// opens it: artifacts mock · page · v4 · by designer, 4 min ago ·
/// bise.dev/m/artifacts`.
pub(crate) fn hover_words(url: &str, now: u64) -> Option<String> {
    let (id, v) = parse_url(url)?;
    let a = get(&id)?;
    let mut parts = vec![a.title.clone(), a.kind_word()];
    let shown = v.unwrap_or(a.v);
    if a.versioned() || v.is_some() {
        parts.push(format!("v{}", shown));
    }
    let who = if a.agent.is_empty() { a.by.clone() } else { a.agent.clone() };
    parts.push(format!("by {}, {}", who, ago_words(a.ts_ms, now)));
    if a.gone {
        parts.push(gone_words(a.copy.is_some()));
    } else {
        parts.push(a.where_words(&workspace()));
    }
    Some(format!("click opens it: {}", parts.join(" · ")))
}

/// Where a chip copied out of the feed points, after its title:
/// `artifacts mock (bise.dev/m/artifacts)`.
pub(crate) fn copy_link(url: &str) -> Option<String> {
    let (id, _) = parse_url(url)?;
    let a = get(&id)?;
    Some(short_target(&a.target))
}

// ---- time words ----

const MINUTE: u64 = 60_000;
const HOUR: u64 = 60 * MINUTE;
const DAYS: [&str; 7] = ["thu", "fri", "sat", "sun", "mon", "tue", "wed"];
const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// The groups of the list (designer's words).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Group {
    Today,
    Yesterday,
    Week,
    Earlier,
}

impl Group {
    pub(crate) fn words(self) -> &'static str {
        match self {
            Group::Today => "today",
            Group::Yesterday => "yesterday",
            Group::Week => "this week",
            Group::Earlier => "earlier",
        }
    }
}

/// Local calendar day number of `ms` at offset `off` (seconds east).
fn day_of(ms: u64, off: i32) -> i64 {
    ((ms / 1000) as i64 + off as i64).div_euclid(86_400)
}

/// Which group `ms` falls in, seen at `now` (local offsets given).
pub(crate) fn group(ms: u64, off: i32, now: u64, now_off: i32) -> Group {
    match day_of(now, now_off) - day_of(ms, off) {
        d if d <= 0 => Group::Today,
        1 => Group::Yesterday,
        d if d < 7 => Group::Week,
        _ => Group::Earlier,
    }
}

/// The age column: today `12 min` / `1 h` (`12m` / `1h` when `short`),
/// `now` under a minute; yesterday its time `18:40`; this week its day
/// `tue`; earlier `sep 28`.
pub(crate) fn age(ms: u64, off: i32, now: u64, now_off: i32, short: bool) -> String {
    let ago = now.saturating_sub(ms);
    let local = (ms / 1000) as i64 + off as i64;
    match group(ms, off, now, now_off) {
        Group::Today if ago < MINUTE => "now".to_string(),
        Group::Today if ago < HOUR && short => format!("{}m", ago / MINUTE),
        Group::Today if ago < HOUR => format!("{} min", ago / MINUTE),
        Group::Today if short => format!("{}h", ago / HOUR),
        Group::Today => format!("{} h", ago / HOUR),
        Group::Yesterday => {
            let secs = local.rem_euclid(86_400);
            format!("{:02}:{:02}", secs / 3600, (secs % 3600) / 60)
        }
        Group::Week => DAYS[local.div_euclid(86_400).rem_euclid(7) as usize].to_string(),
        Group::Earlier => {
            let (m, d) = month_day(local.div_euclid(86_400));
            format!("{} {}", MONTHS[(m - 1) as usize], d)
        }
    }
}

/// (month, day) of a day number since 1970-01-01 (H. Hinnant).
fn month_day(days: i64) -> (u32, u32) {
    let z = days + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (m, d)
}

/// `12 min ago`, `3 h ago`, `yesterday 18:40`, `tue 14:02`, `sep 28`:
/// the line under the list and the hover.
pub(crate) fn ago_at(ms: u64, off: i32, now: u64, now_off: i32) -> String {
    let local = (ms / 1000) as i64 + off as i64;
    let secs = local.rem_euclid(86_400);
    let hm = format!("{:02}:{:02}", secs / 3600, (secs % 3600) / 60);
    match group(ms, off, now, now_off) {
        Group::Today => match age(ms, off, now, now_off, false).as_str() {
            "now" => "just now".to_string(),
            a => format!("{} ago", a),
        },
        Group::Yesterday => format!("yesterday {}", hm),
        Group::Week => format!("{} {}", age(ms, off, now, now_off, false), hm),
        Group::Earlier => age(ms, off, now, now_off, false),
    }
}

/// [`ago_at`] on the real clock's offsets.
pub(crate) fn ago_words(ms: u64, now: u64) -> String {
    ago_at(ms, crate::when::offset_at(ms), now, crate::when::offset_at(now))
}

// ---- the search ----

/// Where `q` is found in `a`: None when it is not; else the char
/// indexes of the title that match (the accent). Every word of `q` must
/// be in the title, the agent or the kind (any case).
pub(crate) fn find(a: &Artifact, q: &str) -> Option<Vec<usize>> {
    let words: Vec<String> = q.split_whitespace().map(|w| w.to_lowercase()).collect();
    if words.is_empty() {
        return Some(Vec::new());
    }
    let title: Vec<char> = a.title.chars().collect();
    let low: Vec<char> = title.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let hay = format!("{} {} {}", a.agent, a.kind_word(), a.kind).to_lowercase();
    let mut hits = Vec::new();
    for w in &words {
        let wc: Vec<char> = w.chars().collect();
        let at = (0..low.len()).find(|&i| low[i..].starts_with(&wc));
        match at {
            Some(i) => hits.extend(i..i + wc.len()),
            None if hay.contains(w.as_str()) => {}
            None => return None,
        }
    }
    hits.sort_unstable();
    hits.dedup();
    Some(hits)
}

// ---- opening ----

/// How an artifact opens (the page's table): a link in the browser, a
/// .md or code file in your editor, the rest in its app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum How {
    Browser(String),
    Editor(std::path::PathBuf),
    App(std::path::PathBuf),
    /// a PR: its diff in the panel
    Diff(u64),
    Nothing,
}

const EDITOR_EXT: &[&str] = &[
    "md", "markdown", "txt", "rs", "ts", "tsx", "js", "jsx", "py", "go", "rb", "sh", "toml", "yaml", "yml", "json", "css", "scss", "c",
    "h", "cpp", "java", "kt", "swift", "sql", "lua", "zig", "bend", "ex", "exs", "hs", "ml",
];

/// How `a` (its version `v`) opens. A PR opens its diff when `diff`.
pub(crate) fn how(a: &Artifact, v: Option<u32>, diff: bool) -> How {
    if diff {
        if let Some(pr) = a.pr.as_ref().filter(|p| p.number > 0) {
            return How::Diff(pr.number);
        }
    }
    let t = a.open_target(v);
    if t.is_empty() {
        return How::Nothing;
    }
    if is_url(&t) {
        return How::Browser(t);
    }
    let p = std::path::PathBuf::from(&t);
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if a.kind == "code" || EDITOR_EXT.contains(&ext.as_str()) {
        How::Editor(p)
    } else {
        How::App(p)
    }
}

/// Opens `a` (version `v`): the note for the status row. A PR's diff is
/// the caller's (it opens the panel).
pub(crate) fn open(app: &mut crate::App, a: &Artifact, v: Option<u32>) -> String {
    match how(a, v, false) {
        How::Browser(url) => {
            if crate::links::open(&url) {
                format!("opening {}", short_target(&url))
            } else {
                format!("could not open {}", short_target(&url))
            }
        }
        How::Editor(path) => {
            let t = crate::file_links::Target { path, line: None, col: None };
            crate::file_links::open(app, &t)
        }
        How::App(path) => {
            let url = crate::file_links::url_of(&crate::file_links::Target { path: path.clone(), line: None, col: None });
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if crate::links::open(&url) {
                format!("opening {}", name)
            } else {
                format!("could not open {}", name)
            }
        }
        How::Diff(_) | How::Nothing => format!("{} has nothing to open", a.title),
    }
}

#[cfg(test)]
#[path = "artifacts_tests.rs"]
mod tests;
