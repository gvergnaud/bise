//! The `$` popup of the composer: pick a skill by name, as in Codex.
//!
//! The list is what main reads: `bend_plugins::skills`'s scan of the
//! skill folders and the loaded plugins' skills, the off ones left out,
//! first name kept, rebuilt when a SKILL.md comes, goes or changes or a
//! skill is turned on or off ([`index`]). Picking inserts `$name ` in the text; the
//! model reads the mention (nothing loads the skill on the client side).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use bend_plugins::model::StateV2;
use bend_plugins::skills;
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

/// How long the popup's scan waits on a folder or a file: short, the
/// popup must not freeze (a stuck path is skipped, then never waited on
/// again while it hangs: `bend_plugins::skills`).
const LIMIT: Duration = Duration::from_millis(300);

/// Where the scan looks: the skill folders (`bend_plugins::skills::roots`,
/// main's: the user talks to main) and the plugin roots. The real ones
/// come from the environment ([`Places::standard`]); a test passes temp
/// folders (never the user's HOME: a skill there may link into
/// ~/Documents, whose read waits on a macOS privacy prompt).
struct Places {
    roots: Vec<skills::Root>,
    plugins: bend_plugins::resolve::Roots,
    /// his switch (`skills::mode_of`): a link into Documents is listed,
    /// never read, while it is off; picking it is his act
    mode: skills::Mode,
    /// his home, to name a skill linked into a guarded folder
    home: Option<PathBuf>,
}

impl Places {
    fn standard(workspace: &Path) -> Places {
        let h = bise_home::Home::from_env();
        let home = skills::user_home(&h).map(Path::to_path_buf);
        let app = bend_plugins::resolve::builtin_root().and_then(|r| r.parent().map(Path::to_path_buf));
        // bise-pages only where the desktop's rules are on (the hub's
        // agents get it under the same flag, architect m_12576)
        let bise = skills::BiseSkills::of(true, bise_home::projects::desktop_for(workspace));
        Places {
            roots: skills::roots(home.as_deref(), workspace, app.as_deref(), bise),
            plugins: bend_plugins::resolve::Roots::standard(Some(workspace)),
            mode: skills::mode_of(&h),
            home: Some(h.user_home().to_path_buf()),
        }
    }
}

/// Stats only: the skill folders' fingerprint with the on/off state, and
/// the plugins' (a plugin enabled, added or edited).
fn fingerprint(places: &Places, st: &StateV2) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bend_plugins::resolve::fingerprint(&places.plugins).hash(&mut h);
    skills::fingerprint(&places.roots, st, LIMIT, &places.mode).0.hash(&mut h);
    h.finish()
}

/// Every skill the agents read, on now, in their order (first name kept).
fn scan(places: &Places, st: &StateV2) -> Vec<Skill> {
    let found = skills::scan(&places.roots, LIMIT, &places.mode, places.home.as_deref());
    let res = bend_plugins::resolve::resolve(&places.plugins);
    skills::ordered(skills::on(found.listed, st), skills::of_plugins(res.loaded()))
        .into_iter()
        .map(|s| Skill { name: s.name, desc: short(s.description.trim()) })
        .collect()
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

/// The skills of `workspace`'s skill folders and plugins that are on,
/// what main lists. Rebuilt when the folders' stats or the on/off state
/// move (a SKILL.md added, edited or removed, a skill turned off), checked
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
    let st = bend_plugins::state::read(&bend_plugins::state::state_path());
    let fp = fingerprint(&places, &st);
    if let Some((w, t, f, list)) = cache.as_mut() {
        if w == workspace && *f == fp {
            *t = Instant::now();
            return list.clone();
        }
    }
    let list = scan(&places, &st);
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

    /// The popup's list: the workspace's skills follow their SKILL.md
    /// (added, edited, removed: the stats move each time and stay put
    /// while nothing changes), and a skill turned off leaves it.
    #[test]
    fn the_scan_follows_the_workspace_skills_and_the_state() {
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
            roots: skills::roots(Some(&home), &ws, None, skills::BiseSkills::None),
            plugins: bend_plugins::resolve::Roots {
                user: Some(home.join(".agents/plugins")),
                workspace: Some(ws.join(".agents/plugins")),
                data: root.join("plugin-data"),
                ..bend_plugins::resolve::Roots::for_test(&root)
            },
            mode: skills::Mode::List { home: home.clone() },
            home: Some(home.clone()),
        };
        let on = StateV2::default();
        let find = |st: &StateV2| scan(&places, st).into_iter().find(|s| s.name == "gamma");
        let fingerprint = |st: &StateV2| fingerprint(&places, st);
        let names: Vec<String> = scan(&places, &on).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["other"], "the temp home's skill, nothing else");
        let empty = fingerprint(&on);
        assert_eq!(find(&on), None);
        std::fs::write(dir.join("SKILL.md"), "---\nname: gamma\ndescription: \"First\" text. More.\n---\nbody\n").unwrap();
        let added = fingerprint(&on);
        assert_ne!(added, empty);
        assert_eq!(fingerprint(&on), added);
        assert_eq!(find(&on).map(|s| s.desc), Some("First text.".into()), "the first sentence");
        let gamma_off = StateV2 {
            parts_off: vec![bend_plugins::id::PluginId::parse("skills:workspace#skill:gamma").unwrap()],
            ..StateV2::default()
        };
        assert_eq!(find(&gamma_off), None, "turned off: not offered");
        assert_ne!(fingerprint(&gamma_off), added);
        std::fs::write(dir.join("SKILL.md"), "---\nname: gamma\ndescription: Second, longer\n---\n").unwrap();
        assert_ne!(fingerprint(&on), added);
        assert_eq!(find(&on).map(|s| s.desc), Some("Second, longer".into()));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(fingerprint(&on), empty);
        assert_eq!(find(&on), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The guard: this test binary runs on a temp HOME (lib.rs,
    /// bise_home::test_home), so even the real places read no user file.
    #[test]
    fn the_tests_never_see_the_real_home() {
        assert!(bise_home::test_home::active(), "HOME = {:?}", bise_home::Home::from_env().user_home());
        let ws = std::env::temp_dir().join(format!("tui-skills-home-{}", std::process::id()));
        let home = bise_home::Home::from_env().user_home().to_path_buf();
        for r in Places::standard(&ws).roots.iter().filter(|r| r.scope != bend_plugins::id::SkillsScope::Workspace && r.scope != bend_plugins::id::SkillsScope::Bise) {
            assert!(r.dir.starts_with(&home), "{r:?}");
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
