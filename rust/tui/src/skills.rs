//! The `$` popup of the composer: pick a skill by name, as in Codex.
//!
//! The list is the TUI's own scan of the skill folders the agents read
//! (runtime/skills.bend: the workspace's `.agents/skills`, the loaded
//! plugins' skills, bise's `prompts/skills`, `~/.agents/skills`,
//! `~/.vibe/skills`; first name kept), rebuilt when a SKILL.md comes,
//! goes or changes ([`index`]). Picking inserts `$name ` in the text; the
//! model reads the mention (nothing loads the skill on the client side).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Skill {
    pub(crate) name: String,
    pub(crate) desc: String,
}

/// The index lines, first occurrence of a name kept (the scan visits
/// several skill folders), in file order (the tests' own index).
#[cfg(test)]
pub(crate) fn parse_index(text: &str) -> Vec<Skill> {
    let mut out: Vec<Skill> = Vec::new();
    for line in text.lines() {
        let mut f = line.split('\t');
        let (Some(name), Some(desc)) = (f.next(), f.next()) else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            continue;
        }
        if out.iter().any(|s| s.name == name) {
            continue;
        }
        out.push(Skill {
            name: name.to_string(),
            desc: short(desc.trim()),
        });
    }
    out
}

/// The first sentence of a description.
fn short(desc: &str) -> String {
    match desc.find(". ") {
        Some(i) => desc[..=i].to_string(),
        None => desc.to_string(),
    }
}

/// Where the scan looks besides the workspace: the user's home, bise's
/// built-in `prompts/skills` and the plugin roots. The real ones come
/// from the environment ([`Roots::standard`]); a test passes temp folders
/// (never the user's HOME: a skill there may link into ~/Documents, whose
/// read waits on a macOS privacy prompt).
struct Places {
    home: Option<PathBuf>,
    prompts: Option<PathBuf>,
    plugins: bend_plugins::resolve::Roots,
}

impl Places {
    fn standard(workspace: &Path) -> Places {
        Places {
            home: std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from),
            prompts: builtin_prompts(),
            plugins: bend_plugins::resolve::Roots::standard(Some(workspace)),
        }
    }
}

/// The skill folders of `workspace`, in the agents' order (runtime/
/// skills.bend: the session's index, then the shared one): the
/// workspace's `.agents/skills`, (then the loaded plugins' skills, `scan`)
/// bise's built-in `prompts/skills`, then `~/.agents/skills` and
/// `~/.vibe/skills`.
fn skill_dirs(workspace: &Path, places: &Places) -> Vec<PathBuf> {
    let mut dirs = vec![workspace.join(".agents/skills")];
    dirs.extend(places.prompts.clone());
    if let Some(home) = &places.home {
        dirs.push(home.join(".agents/skills"));
        dirs.push(home.join(".vibe/skills"));
    }
    dirs
}

/// The app root's `prompts/skills` (main's built-in skills, bise-demo).
fn builtin_prompts() -> Option<PathBuf> {
    let root = bend_plugins::resolve::builtin_root()?;
    Some(root.parent()?.join("prompts/skills")).filter(|p| p.is_dir())
}

/// A SKILL.md's `name:` and `description:` lines, as the scan reads them
/// (the first of each, tabs, CRs and quotes dropped, both non-empty).
pub(crate) fn parse_skill(text: &str) -> Option<Skill> {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .map(|v| v.trim_start().chars().filter(|c| !matches!(c, '\t' | '\r' | '"')).collect::<String>())
            .filter(|v| !v.is_empty())
    };
    let name = field("name:")?;
    let desc = field("description:")?;
    (!name.contains(char::is_whitespace)).then(|| Skill { name, desc: short(desc.trim()) })
}

/// Stats only: each folder's `<skill>/SKILL.md` with its size and mtime,
/// and the plugins' fingerprint (a plugin enabled, added or edited).
fn fingerprint(workspace: &Path, places: &Places) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bend_plugins::resolve::fingerprint(&places.plugins).hash(&mut h);
    for d in skill_dirs(workspace, places) {
        d.hash(&mut h);
        for f in skill_files(&d) {
            let Ok(m) = std::fs::metadata(&f) else { continue };
            f.hash(&mut h);
            m.len().hash(&mut h);
            m.modified().ok().hash(&mut h);
        }
    }
    h.finish()
}

fn skill_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("SKILL.md"))
        .filter(|f| f.is_file())
        .collect();
    v.sort();
    v
}

/// Every skill of `workspace`'s folders, read now, first name kept.
fn scan(workspace: &Path, places: &Places) -> Vec<Skill> {
    let mut out: Vec<Skill> = Vec::new();
    let mut push = |s: Skill| {
        if !out.iter().any(|o| o.name == s.name) {
            out.push(s);
        }
    };
    let dirs = skill_dirs(workspace, places);
    for f in skill_files(&dirs[0]) {
        if let Some(s) = std::fs::read_to_string(&f).ok().as_deref().and_then(parse_skill) {
            push(s);
        }
    }
    let res = bend_plugins::resolve::resolve(&places.plugins);
    for p in res.loaded() {
        for s in &p.skills {
            push(Skill { name: s.name.clone(), desc: short(s.description.trim()) });
        }
    }
    for d in &dirs[1..] {
        for f in skill_files(d) {
            if let Some(s) = std::fs::read_to_string(&f).ok().as_deref().and_then(parse_skill) {
                push(s);
            }
        }
    }
    out
}

/// (workspace, last stat check, fingerprint, skills)
type Cache = Option<(PathBuf, Instant, u64, Vec<Skill>)>;
static CACHE: Mutex<Cache> = Mutex::new(None);

#[cfg(test)]
thread_local! {
    /// A test's own index (this thread only): the real one is the
    /// machine's, maybe empty.
    pub(crate) static TEST_INDEX: std::cell::RefCell<Option<Vec<Skill>>> = const { std::cell::RefCell::new(None) };
}

/// The skills of `workspace`'s skill folders and plugins, what its
/// agents list (no index file: the shared one the REPLs write holds only
/// the user folders, and only as of the last REPL start). Rebuilt when
/// the folders' stats move (a SKILL.md added, edited or removed), checked
/// when the `$` popup asks after a second without asking (it asks at
/// every frame while open): no timer, no watcher.
pub(crate) fn index(workspace: &Path) -> Vec<Skill> {
    #[cfg(test)]
    if let Some(list) = TEST_INDEX.with(|t| t.borrow().clone()) {
        return list;
    }
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((w, t, _, list)) = cache.as_ref() {
        if w == workspace && t.elapsed() < Duration::from_secs(1) {
            return list.clone();
        }
    }
    let places = Places::standard(workspace);
    let fp = fingerprint(workspace, &places);
    if let Some((w, t, f, list)) = cache.as_mut() {
        if w == workspace && *f == fp {
            *t = Instant::now();
            return list.clone();
        }
    }
    let list = scan(workspace, &places);
    *cache = Some((workspace.to_path_buf(), Instant::now(), fp, list.clone()));
    list
}

/// The `$word` that ends at the cursor: its start (char index of the
/// `$`) and the name typed so far. The `$` opens a word (line start or
/// after a space) and the word has no space yet.
pub(crate) fn token(input: &str, cursor: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = chars[..cursor].iter().rposition(|c| c.is_whitespace()).map(|i| i + 1).unwrap_or(0);
    // the cursor on or before the `$`: no token (and no slice past it)
    match chars[..cursor].get(start..) {
        Some(['$', rest @ ..]) => Some((start, rest.iter().collect())),
        _ => None,
    }
}

/// Case-insensitive prefix matches first, then substring matches.
pub(crate) fn filter<'a>(skills: &'a [Skill], query: &str) -> Vec<&'a Skill> {
    prefix_first(skills, query, |s| &s.name)
}

/// The items whose name matches `query`, case-insensitive: prefix
/// matches first, then substring matches, each group in input order
/// (the `$` skill popup, the `@` agent popup).
pub(crate) fn prefix_first<'a, T>(
    items: impl IntoIterator<Item = &'a T>,
    query: &str,
    name: impl Fn(&T) -> &str,
) -> Vec<&'a T> {
    let q = query.to_lowercase();
    // (is a prefix match, item) of every match
    let hits: Vec<(bool, &T)> = items
        .into_iter()
        .filter_map(|x| {
            let n = name(x).to_lowercase();
            n.contains(&q).then(|| (n.starts_with(&q), x))
        })
        .collect();
    let group = |prefix: bool| hits.iter().filter(move |h| h.0 == prefix).map(|h| h.1);
    group(true).chain(group(false)).collect()
}

/// The composer once `name` is picked for the token at `start..cursor`:
/// the new text and the new cursor (after `$name `).
pub(crate) fn complete(input: &str, start: usize, cursor: usize, name: &str) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let cursor = cursor.min(chars.len());
    // a stale start (past the cursor) never slices past the text
    let start = start.min(cursor);
    let head: String = chars[..start].iter().collect();
    let ins = format!("${} ", name);
    let mut tail: String = chars[cursor..].iter().collect();
    if tail.starts_with(' ') {
        tail.remove(0);
    }
    let at = start + ins.chars().count();
    (format!("{}{}{}", head, ins, tail), at)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = "bend\tUse for Bend code. Loads the guide.\t/a/bend/SKILL.md
build-mcp-app\tBuild an MCP app\t/a/b/SKILL.md
grill-me\tInterview the user\t/a/g/SKILL.md
bend\tduplicate from another folder\t/b/bend/SKILL.md
broken line without tabs
mcp-builder\tGuide for MCP servers\t/a/m/SKILL.md
";

    fn names(v: Vec<&Skill>) -> Vec<&str> {
        v.into_iter().map(|s| s.name.as_str()).collect()
    }

    /// The workspace's skills: a SKILL.md added shows, edited shows its
    /// new text, removed goes; the stats move each time and stay put
    /// while nothing changes.
    #[test]
    fn the_scan_follows_the_workspace_skills() {
        let root = std::env::temp_dir().join(format!("tui-skills-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let ws = root.join("ws");
        let dir = ws.join(".agents/skills/gamma");
        std::fs::create_dir_all(&dir).unwrap();
        // a temp HOME, no built-in folder, no real plugin root: the user's
        // own folders are never read
        let home = root.join("home");
        let other = home.join(".vibe/skills/other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("SKILL.md"), "name: other\ndescription: from the home\n").unwrap();
        let places = Places {
            home: Some(home.clone()),
            prompts: None,
            plugins: bend_plugins::resolve::Roots {
                builtin: None,
                user: Some(home.join(".agents/plugins")),
                workspace: Some(ws.join(".agents/plugins")),
                data: root.join("plugin-data"),
                disabled: vec![],
                enabled: vec![],
            },
        };
        let find = |ws: &Path| scan(ws, &places).into_iter().find(|s| s.name == "gamma");
        let fingerprint = |ws: &Path| fingerprint(ws, &places);
        let names: Vec<String> = scan(&ws, &places).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["other"], "the temp home's skill, nothing else");
        let empty = fingerprint(&ws);
        assert_eq!(find(&ws), None);
        std::fs::write(dir.join("SKILL.md"), "---\nname: gamma\ndescription: \"First\" text\n---\nbody\n").unwrap();
        let added = fingerprint(&ws);
        assert_ne!(added, empty);
        assert_eq!(fingerprint(&ws), added);
        assert_eq!(find(&ws).map(|s| s.desc), Some("First text".into()));
        std::fs::write(dir.join("SKILL.md"), "---\nname: gamma\ndescription: Second, longer\n---\n").unwrap();
        assert_ne!(fingerprint(&ws), added);
        assert_eq!(find(&ws).map(|s| s.desc), Some("Second, longer".into()));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(fingerprint(&ws), empty);
        assert_eq!(find(&ws), None);
        // no description, or a name with a space: not a skill
        assert_eq!(parse_skill("name: a\n"), None);
        assert_eq!(parse_skill("name: a b\ndescription: d\n"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The guard: this test binary runs on a temp HOME (lib.rs,
    /// bise_home::test_home), so even the real places read no user file.
    #[test]
    fn the_tests_never_see_the_real_home() {
        assert!(bise_home::test_home::active(), "HOME = {:?}", std::env::var_os("HOME"));
        let ws = std::env::temp_dir().join(format!("tui-skills-home-{}", std::process::id()));
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        for d in Places::standard(&ws).home.iter().map(|h| h.join(".vibe/skills")) {
            assert!(d.starts_with(&home), "{d:?}");
        }
        for k in ["BISE_HOME", "BEND_CONFIG", "BEND_PLUGINS_HOME", "SB_SOCKET"] {
            assert_eq!(std::env::var_os(k), None, "{k}");
        }
    }

    #[test]
    fn parses_the_index_first_name_wins() {
        let s = parse_index(INDEX);
        assert_eq!(s.len(), 4);
        assert_eq!(s[0].name, "bend");
        assert_eq!(s[0].desc, "Use for Bend code."); // first sentence
        assert_eq!(s[1].desc, "Build an MCP app");
    }

    #[test]
    fn token_is_the_dollar_word_at_the_cursor() {
        // the cursor on the `$` (← after typing it): none, no panic
        assert_eq!(token("$", 0), None);
        assert_eq!(token("run $be", 4), None);
        assert_eq!(token("$", 1), Some((0, String::new())));
        assert_eq!(token("$be", 3), Some((0, "be".into())));
        assert_eq!(token("use $gr", 7), Some((4, "gr".into())));
        assert_eq!(token("use $gr please", 7), Some((4, "gr".into())));
        assert_eq!(token("use $gr please", 14), None); // cursor after the word
        assert_eq!(token("cost 5$", 7), None); // not a word start
        assert_eq!(token("$bend ", 6), None); // done: a space follows
        assert_eq!(token("plain", 5), None);
    }

    #[test]
    fn filters_prefix_first() {
        let s = parse_index(INDEX);
        assert_eq!(names(filter(&s, "")).len(), 4);
        assert_eq!(names(filter(&s, "B")), ["bend", "build-mcp-app", "mcp-builder"]);
        assert_eq!(names(filter(&s, "mcp")), ["mcp-builder", "build-mcp-app"]);
        assert!(filter(&s, "zzz").is_empty());
    }

    #[test]
    fn completion_replaces_the_token() {
        assert_eq!(complete("$be", 0, 3, "bend"), ("$bend ".into(), 6));
        assert_eq!(complete("use $gr", 4, 7, "grill-me"), ("use $grill-me ".into(), 14));
        assert_eq!(
            complete("use $gr please", 4, 7, "grill-me"),
            ("use $grill-me please".into(), 14)
        );
        // the popup closes once picked
        let (t, c) = complete("$be", 0, 3, "bend");
        assert_eq!(token(&t, c), None);
    }
}
