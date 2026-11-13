//! The one scanner of the loose skill folders (docs/plugins.md, "Skills"):
//! which folders an agent reads ([`roots`]), what is in them ([`scan`]),
//! which of their skills are on ([`on`]), the session's skills index
//! ([`index`], [`loose_on`]) and the fingerprint that tells the hub a
//! REPL's skills moved ([`fingerprint`]). The hub, the terminal's `$`
//! popup and the session bridge all call it; nobody else lists a skill
//! folder.
//!
//! Each folder is a plugin of its own, `skills:<scope>` ([`SkillsScope`]),
//! and each of its skills a part, `skills:<scope>#skill:<name>`;
//! `skills:bise` (bise's own, bise-pages) is locked on
//! ([`StateV2::locked`]).
//!
//! The roots reach the bridge from the hub through `$BISE_SKILL_ROOTS`
//! (one `<scope>=<dir>` per line), written and read by
//! [`Root::encode`] and [`Root::decode`] only.
//!
//! No read of a skill folder blocks its caller: a SKILL.md that links
//! into ~/Documents waits on macOS's privacy check (release-1007: main
//! never made its first model call). Every listing, stat and read runs
//! on a thread of its own, waited at most `limit`; a path still stuck
//! is skipped, named once ([`Scan::stuck`]), and never gets a second
//! thread while its first one hangs.
//!
//! No scan follows a link into a folder macOS guards (Documents, Desktop,
//! Downloads, iCloud Drive: `bise_home::guarded`) unless he said so
//! (skills-scan-tcc, architect m_64243): listing is not his act, and a
//! stat there makes macOS ask him at launch. [`mode_of`] reads his
//! switch (`skills.read_guarded`, off by default); in [`Mode::List`] a
//! SKILL.md reached by such a link is found by reading links only, never
//! stat'd or read: [`Scan::guarded`] names it, [`Scan::listed`] gives
//! it to the agents and the `$` popup as 'in Documents: read when used'.
//! Loading a skill's text when an agent uses it, or he picks it, reads it.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::id::{PartKind, PluginId, SkillsScope};
use crate::model::{SkillEntry, StateV2};

/// The environment variable that carries an agent's roots, from the hub
/// to its REPL's bridge.
pub const ENV: &str = "BISE_SKILL_ROOTS";

/// How long a scan waits for a folder's listing, then for its files.
pub const SCAN_LIMIT: Duration = Duration::from_secs(3);

/// How much of a SKILL.md the scan reads (its frontmatter is at the top).
const READ_MAX: u64 = 64 * 1024;

/// How a scan treats a SKILL.md it reaches by a link into a folder
/// macOS guards ([`mode_of`] picks it; every scan takes it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// read it like any other (he turned `skills.read_guarded` on)
    Read,
    /// never stat or read it: name it from its folder only. `home` is
    /// his home, whose Documents, Desktop... are guarded
    List { home: PathBuf },
}

/// The mode his prefs (`prefs.json`'s object) give, `user_home` his home:
/// [`Mode::Read`] once he turned `skills.read_guarded` on, else
/// [`Mode::List`]. Pure.
pub fn mode_for(prefs: Option<&serde_json::Value>, user_home: &Path) -> Mode {
    if bise_home::prefs::read_guarded(prefs) {
        Mode::Read
    } else {
        Mode::List { home: user_home.to_path_buf() }
    }
}

/// The mode of `home`'s prefs: the one reader of `skills.read_guarded`,
/// every scan's caller asks it.
pub fn mode_of(home: &bise_home::Home) -> Mode {
    let prefs = std::fs::read_to_string(home.prefs_file()).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    mode_for(prefs.as_ref(), home.user_home())
}

/// What the agents and the `$` popup read as a guarded skill's
/// description in [`Mode::List`] (`folder`: `Documents`): its text is
/// read when used.
pub fn unread_description(folder: &str) -> String {
    format!("in {folder}: read when used. Its SKILL.md links into {folder}, a folder macOS guards; its text is read when you load it.")
}

/// A SKILL.md reached by a link into a folder macOS guards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guarded {
    /// its folder's name (its frontmatter is not read in [`Mode::List`])
    pub name: String,
    /// the SKILL.md as listed (`~/.vibe/skills/x/SKILL.md`), not its target
    pub path: PathBuf,
    pub owner: PluginId,
    /// the guarded folder's name for him (`Documents`, `iCloud Drive`)
    pub folder: String,
    /// read by this scan ([`Mode::Read`] and the read worked)
    pub read: bool,
}

/// A loose skill folder and the plugin it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    pub scope: SkillsScope,
    pub dir: PathBuf,
}

/// Which of bise's own skill folders (in the app root) an agent reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiseSkills {
    /// none: a bare REPL with no hub
    None,
    /// `prompts/skills` (main's own, bise-demo)
    Main,
    /// `prompts/skills-all` (every agent's, bise-pages), where the
    /// desktop is on (architect m_12576)
    All,
    /// both: main where the desktop is on
    MainAndAll,
}

impl BiseSkills {
    /// An agent's: main or a task, the desktop on or off.
    pub fn of(is_main: bool, desktop: bool) -> BiseSkills {
        match (is_main, desktop) {
            (true, true) => BiseSkills::MainAndAll,
            (true, false) => BiseSkills::Main,
            (false, true) => BiseSkills::All,
            (false, false) => BiseSkills::None,
        }
    }
}

/// The skill folders an agent reads, in precedence order (the first of
/// a name wins, [`ordered`]): `<ws>/.agents/skills`, the app root's
/// `prompts/skills-all` then `prompts/skills` (by `bise`), then
/// `~/.agents/skills` and `~/.vibe/skills`.
pub fn roots(home: Option<&Path>, ws: &Path, app_root: Option<&Path>, bise: BiseSkills) -> Vec<Root> {
    let mut out = vec![Root { scope: SkillsScope::Workspace, dir: ws.join(".agents/skills") }];
    if let Some(app) = app_root {
        if matches!(bise, BiseSkills::All | BiseSkills::MainAndAll) {
            out.push(Root { scope: SkillsScope::Bise, dir: app.join("prompts/skills-all") });
        }
        if matches!(bise, BiseSkills::Main | BiseSkills::MainAndAll) {
            out.push(Root { scope: SkillsScope::Bise, dir: app.join("prompts/skills") });
        }
    }
    if let Some(home) = home {
        out.push(Root { scope: SkillsScope::Personal, dir: home.join(".agents/skills") });
        out.push(Root { scope: SkillsScope::Vibe, dir: home.join(".vibe/skills") });
    }
    out
}

/// The home whose `~/.agents/skills` and `~/.vibe/skills` the scans
/// read: his, unless `$BISE_HOME` chose another root (a throwaway home
/// never reads his real skills, architect m_64235).
pub fn user_home(home: &bise_home::Home) -> Option<&Path> {
    (!home.explicit()).then(|| home.user_home())
}

impl Root {
    /// The plugin this folder is (`skills:personal`).
    pub fn id(&self) -> PluginId {
        PluginId::Skills { scope: self.scope }
    }

    /// `$BISE_SKILL_ROOTS`'s value: one `<scope>=<dir>` per line.
    pub fn encode(roots: &[Root]) -> String {
        roots.iter().map(|r| format!("{}={}\n", r.scope.as_str(), r.dir.display())).collect()
    }

    /// Read [`Root::encode`]'s text: split on the first `=`; a line with
    /// an unknown scope or an empty path is an error (a path never holds
    /// a newline: [`Root::encode`]'s lines are split on it).
    pub fn decode(text: &str) -> Result<Vec<Root>, String> {
        let mut out = Vec::new();
        for line in text.lines().filter(|l| !l.is_empty()) {
            let Some((scope, dir)) = line.split_once('=') else {
                return Err(format!("{ENV}: no `=` in {line:?}"));
            };
            let Some(scope) = SkillsScope::ALL.into_iter().find(|s| s.as_str() == scope) else {
                return Err(format!("{ENV}: unknown skills folder {scope:?}"));
            };
            if dir.is_empty() {
                return Err(format!("{ENV}: no path for {scope:?}", scope = scope.as_str()));
            }
            out.push(Root { scope, dir: PathBuf::from(dir) });
        }
        Ok(out)
    }

    /// `(name, value)` of `$BISE_SKILL_ROOTS` for these roots. A root
    /// whose path holds a newline cannot be written: it is left out.
    pub fn env(roots: &[Root]) -> (&'static str, String) {
        let ok: Vec<Root> = roots.iter().filter(|r| !r.dir.to_string_lossy().contains('\n')).cloned().collect();
        (ENV, Root::encode(&ok))
    }

    /// The roots of this process: `$BISE_SKILL_ROOTS` as the hub set it;
    /// without it (a bare REPL, no hub) the user's (none under a
    /// throwaway home: [`user_home`]) and `ws`'s folders, none of bise's.
    /// A bad value is an error, never a guess.
    pub fn from_env(ws: &Path) -> Result<Vec<Root>, String> {
        match std::env::var(ENV) {
            Ok(v) => Root::decode(&v),
            Err(_) => Ok(roots(user_home(&bise_home::Home::from_env()), ws, None, BiseSkills::None)),
        }
    }
}

/// What [`scan`] found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// every skill of the roots read, in root order then name order
    pub skills: Vec<SkillEntry>,
    /// the SKILL.md files reached by a link into a guarded folder, in
    /// the same order: in [`Mode::List`] never read (`read` false); in
    /// [`Mode::Read`] read too, so in `skills` as well when it worked
    pub guarded: Vec<Guarded>,
    /// what the agents and the `$` popup list: `skills` with the unread
    /// guarded ones as entries named by their folder, described by
    /// [`unread_description`], in root order
    pub listed: Vec<SkillEntry>,
    /// the paths this scan found stuck for the first time (a later scan
    /// skips them without a word while they still hang): name each once
    pub stuck: Vec<PathBuf>,
}

/// The paths whose thread still hangs (a listing, a stat or a read), in
/// this process: skipped at once until their thread ends.
static STUCK: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

fn stuck_set<T>(f: impl FnOnce(&mut HashSet<PathBuf>) -> T) -> T {
    let mut g = STUCK.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashSet::new))
}

/// Run `job(path)` for each path, each on a thread of its own, and wait
/// until `deadline`. A path already stuck is skipped (no new thread); a
/// path whose job is not done at `deadline` becomes stuck (its thread
/// clears it when it ends) and is returned in the second list.
fn timed<T: Send + 'static, F: Fn(&Path) -> T + Send + Sync + 'static>(
    paths: Vec<PathBuf>,
    deadline: Instant,
    job: F,
) -> (Vec<(PathBuf, T)>, Vec<PathBuf>) {
    let job = std::sync::Arc::new(job);
    let (tx, rx) = channel();
    let mut waiting: Vec<PathBuf> = Vec::new();
    for p in paths {
        if stuck_set(|s| s.contains(&p)) {
            continue;
        }
        let tx = tx.clone();
        let q = p.clone();
        let job = job.clone();
        let spawned = std::thread::Builder::new().name("skills-scan".into()).spawn(move || {
            let out = job(&q);
            stuck_set(|s| s.remove(&q));
            let _ = tx.send((q, out));
        });
        if spawned.is_ok() {
            waiting.push(p);
        }
    }
    drop(tx);
    let mut done = Vec::new();
    while !waiting.is_empty() {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok((p, out)) => {
                waiting.retain(|w| *w != p);
                done.push((p, out));
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
        }
    }
    // still running at the deadline: stuck until its thread ends (it
    // may end between the timeout and here: then it is not stuck)
    let mut fresh = Vec::new();
    for (p, out) in rx.try_iter() {
        waiting.retain(|w| *w != p);
        done.push((p, out));
    }
    for p in waiting {
        stuck_set(|s| s.insert(p.clone()));
        fresh.push(p);
    }
    done.sort_by(|a, b| a.0.cmp(&b.0));
    (done, fresh)
}

/// A folder's entries, sorted (a missing folder: none).
fn list(dir: &Path) -> Vec<PathBuf> {
    #[cfg(test)]
    tests::touched(dir);
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    v.sort();
    v
}

/// A SKILL.md's size and mtime, only when it is a regular file (a FIFO,
/// a socket, a folder named SKILL.md: no skill, never opened).
fn stat(file: &Path) -> Option<(u64, Option<u128>)> {
    #[cfg(test)]
    tests::touched(file);
    let m = std::fs::metadata(file).ok().filter(std::fs::Metadata::is_file)?;
    let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos());
    Some((m.len(), t))
}

/// A link's own size and mtime (its lstat, never through it), its
/// target's length folded in so a relinked SKILL.md moves the number.
fn link_stat(link: &Path) -> Option<(u64, Option<u128>)> {
    let m = std::fs::symlink_metadata(link).ok()?;
    let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos());
    let target = std::fs::read_link(link).map(|t| t.as_os_str().len() as u64).unwrap_or(0);
    Some((m.len() ^ (target << 32), t))
}

/// A regular SKILL.md's head and its real path.
fn read(file: &Path) -> Option<(String, PathBuf)> {
    stat(file)?;
    let mut buf = Vec::new();
    std::fs::File::open(file).ok()?.take(READ_MAX).read_to_end(&mut buf).ok()?;
    let real = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    Some((String::from_utf8_lossy(&buf).into_owned(), real))
}

/// The `<dir>/SKILL.md` of each root's entries, with the root's index.
/// In [`Mode::List`], a root reached by a link into a guarded folder is
/// not listed (nothing of it is shown: a listing there is a stat).
fn skill_files(roots: &[Root], deadline: Instant, mode: &Mode) -> (Vec<(usize, PathBuf)>, Vec<PathBuf>) {
    let dirs: Vec<PathBuf> = roots.iter().map(|r| r.dir.clone()).collect();
    let m = mode.clone();
    let (listed, stuck) = timed(dirs, deadline, move |d| if guard(&m, d).is_some() { Vec::new() } else { list(d) });
    let mut files = Vec::new();
    for (i, r) in roots.iter().enumerate() {
        for (_, entries) in listed.iter().filter(|(d, _)| *d == r.dir) {
            files.extend(entries.iter().map(|e| (i, e.join("SKILL.md"))));
        }
    }
    (files, stuck)
}

/// Where `file`'s link leads into a guarded folder, in [`Mode::List`]
/// only (links read, nothing inside the folder touched).
fn guard(mode: &Mode, file: &Path) -> Option<bise_home::guarded::LinkInto> {
    match mode {
        Mode::Read => None,
        Mode::List { home } => bise_home::guarded::link_into(Some(home), file),
    }
}

/// What a scan learns of one SKILL.md.
enum Found {
    /// read (none: not a regular file, or the read failed)
    Read(Option<(String, PathBuf)>),
    /// reached by a link into this guarded folder: not read
    Guarded(String),
}

/// One SKILL.md, by `mode`: a link into a guarded folder in
/// [`Mode::List`] is named, never stat'd or read; in [`Mode::Read`] it
/// is read and still named as linked (his switch's count).
fn look(mode: &Mode, home: Option<&Path>, file: &Path) -> (Option<String>, Found) {
    if let Some(into) = guard(mode, file) {
        let Mode::List { home } = mode else { unreachable!() };
        return (None, Found::Guarded(bise_home::guarded::label(Some(home), &into.folder)));
    }
    let linked = home.and_then(|h| bise_home::guarded::link_into(Some(h), file)).map(|i| bise_home::guarded::label(home, &i.folder));
    (linked, Found::Read(read(file)))
}

/// Every skill of `roots`: the listings within `limit`, then the files
/// within `limit` again, by `mode` ([`mode_of`]). A root listed twice
/// is read once. `home`: his home, to name the skills linked into a
/// guarded folder in [`Mode::Read`] too (none: not named).
pub fn scan(roots: &[Root], limit: Duration, mode: &Mode, home: Option<&Path>) -> Scan {
    let (files, mut stuck) = skill_files(roots, Instant::now() + limit, mode);
    let paths: Vec<PathBuf> = files.iter().map(|(_, f)| f.clone()).collect();
    let (m, h) = (mode.clone(), home.map(Path::to_path_buf));
    let (found, more) = timed(paths, Instant::now() + limit, move |f| look(&m, h.as_deref(), f));
    stuck.extend(more);
    let mut out = Scan::default();
    let mut seen = HashSet::new();
    for (i, f) in &files {
        if !seen.insert(f) {
            continue;
        }
        let Some((_, (linked, found))) = found.iter().find(|(p, _)| p == f) else { continue };
        let owner = roots[*i].id();
        let dir_name = || f.parent().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match found {
            Found::Guarded(folder) => {
                let name = dir_name();
                if name.is_empty() || name.contains(char::is_whitespace) {
                    continue;
                }
                out.listed.push(SkillEntry { name: name.clone(), description: unread_description(folder), path: f.clone(), owner: owner.clone() });
                out.guarded.push(Guarded { name, path: f.clone(), owner, folder: folder.clone(), read: false });
            }
            Found::Read(read) => {
                let skill = read.as_ref().and_then(|(text, real)| parse(text).map(|(name, description)| SkillEntry { name, description, path: real.clone(), owner: owner.clone() }));
                if let Some(folder) = linked {
                    out.guarded.push(Guarded { name: skill.as_ref().map_or_else(dir_name, |s| s.name.clone()), path: f.clone(), owner, folder: folder.clone(), read: skill.is_some() });
                }
                if let Some(s) = skill {
                    out.listed.push(s.clone());
                    out.skills.push(s);
                }
            }
        }
    }
    out.stuck = stuck;
    out
}

/// A SKILL.md's `name:` and `description:`: the first line that starts
/// with each, its value with tabs, CRs and quotes dropped; both non-empty
/// and the name without a space, else no skill. (The Agent Skills format
/// keeps the description on one line; a folded one is out of scope.)
pub fn parse(text: &str) -> Option<(String, String)> {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .map(|v| v.trim().chars().filter(|c| !matches!(c, '\t' | '\r' | '"')).collect::<String>())
            .filter(|v| !v.is_empty())
    };
    let name = field("name:")?;
    let desc = field("description:")?;
    (!name.contains(char::is_whitespace)).then_some((name, desc))
}

/// The skill's own id: `<owner>#skill:<name>`.
pub fn id(s: &SkillEntry) -> PluginId {
    s.owner.part(PartKind::Skill, None, s.name.clone())
}

/// Whether a skill is on: locked (`skills:bise`), or on in the state (its
/// folder on and itself not turned off).
pub fn is_on(s: &SkillEntry, st: &StateV2) -> bool {
    let id = id(s);
    StateV2::locked(&id) || st.is_on(&id, false)
}

/// The skills that are on.
pub fn on(skills: Vec<SkillEntry>, st: &StateV2) -> Vec<SkillEntry> {
    skills.into_iter().filter(|s| is_on(s, st)).collect()
}

/// The precedence of a name the agents read (the first one wins): the
/// workspace's loose skills, then the plugins' skills, then the other
/// loose folders in [`roots`] order.
pub fn ordered(loose: Vec<SkillEntry>, plugins: Vec<SkillEntry>) -> Vec<SkillEntry> {
    let ws = PluginId::Skills { scope: SkillsScope::Workspace };
    let (first, rest): (Vec<_>, Vec<_>) = loose.into_iter().partition(|s| s.owner == ws);
    let mut out: Vec<SkillEntry> = Vec::new();
    for s in first.into_iter().chain(plugins).chain(rest) {
        if !out.iter().any(|o| o.name == s.name) {
            out.push(s);
        }
    }
    out
}

/// The skills index the Bend catalog and the `skill` tool read: one
/// `name\tdescription\tpath` line per skill, tabs, newlines and quotes
/// dropped from the fields so the split is unambiguous.
pub fn index(skills: &[SkillEntry]) -> String {
    let clean = |s: &str| -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ").chars().filter(|c| *c != '"').collect()
    };
    skills.iter().map(|s| format!("{}\t{}\t{}\n", clean(&s.name), clean(&s.description), s.path.display())).collect()
}

/// A plugin's skills as entries, owned by the plugin.
pub fn of_plugins<'a>(plugins: impl IntoIterator<Item = &'a crate::resolve::Plugin>) -> Vec<SkillEntry> {
    plugins
        .into_iter()
        .flat_map(|p| {
            p.skills.iter().map(|s| SkillEntry {
                name: s.name.clone(),
                description: s.description.clone(),
                path: s.path.clone(),
                owner: PluginId::plugin(p.name.clone()),
            })
        })
        .collect()
}

/// This session's loose skills that are on, for the bridge's
/// `D/skills-index.txt` (with the plugins' skills, [`ordered`]): the
/// roots of `$BISE_SKILL_ROOTS` ([`Root::from_env`]; the workspace is
/// `$BEND_WORKDIR`, else the cwd, as the bridge's), the state of
/// `plugins.json`, his switch ([`mode_of`]: a skill linked into a guarded
/// folder is listed, not read, while it is off). A stuck path or a bad
/// `$BISE_SKILL_ROOTS` goes to stderr (the bridge's log): no loose skill
/// then, never a guess.
pub fn loose_on() -> Vec<SkillEntry> {
    let roots = match Root::from_env(&crate::cli::workspace(&[])) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skills: {e}");
            return Vec::new();
        }
    };
    let home = bise_home::Home::from_env();
    let found = scan(&roots, SCAN_LIMIT, &mode_of(&home), Some(home.user_home()));
    for p in &found.stuck {
        eprintln!("skills scan: skipped {} (no answer in {} s)", p.display(), SCAN_LIMIT.as_secs());
    }
    on(found.listed, &crate::state::read(&crate::state::state_path()))
}

/// What an agent's skills depend on, as one number, stats only: each
/// root and the size and mtime of each `<root>/<skill>/SKILL.md`, and
/// the on/off of each root and of the skills turned off or on in it (a
/// skill toggle changes the prompt), and the mode. A stuck path counts
/// as absent; the second value names the paths stuck for the first time.
/// In [`Mode::List`] a SKILL.md reached by a link into a guarded folder
/// counts by the link's own lstat and target (architect m_64240): an
/// edit inside Documents to a linked SKILL.md is not seen here, only
/// when the skill is next loaded.
pub fn fingerprint(roots: &[Root], st: &StateV2, limit: Duration, mode: &Mode) -> (u64, Vec<PathBuf>) {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    matches!(mode, Mode::Read).hash(&mut h);
    let (files, mut stuck) = skill_files(roots, Instant::now() + limit, mode);
    let paths: Vec<PathBuf> = files.iter().map(|(_, f)| f.clone()).collect();
    let m = mode.clone();
    let (stats, more) = timed(paths, Instant::now() + limit, move |f| match guard(&m, f) {
        Some(into) => link_stat(&into.link),
        None => stat(f),
    });
    stuck.extend(more);
    for (i, r) in roots.iter().enumerate() {
        r.dir.hash(&mut h);
        let id = r.id();
        if !StateV2::locked(&id) {
            st.is_on(&id, false).hash(&mut h);
            let mine = |l: &[PluginId]| -> Vec<String> {
                let mut v: Vec<String> = l.iter().filter(|p| p.owner() == &id).map(ToString::to_string).collect();
                v.sort();
                v
            };
            mine(&st.parts_off).hash(&mut h);
            mine(&st.parts_on).hash(&mut h);
        }
        for (_, f) in files.iter().filter(|(j, _)| *j == i) {
            if let Some((_, Some(s))) = stats.iter().find(|(p, _)| p == f) {
                f.hash(&mut h);
                s.hash(&mut h);
            }
        }
    }
    (h.finish(), stuck)
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
