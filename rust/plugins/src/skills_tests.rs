use super::*;

/// A temp folder of the test's own, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Tmp {
        let d = std::env::temp_dir().join(format!("bp-skills-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Tmp(d.canonicalize().unwrap())
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn skill(root: &Path, dir: &str, name: &str, desc: &str) -> PathBuf {
    let f = root.join(dir).join("SKILL.md");
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, format!("---\nname: {name}\ndescription: {desc}\n---\nbody\n")).unwrap();
    f
}

fn names(v: &[SkillEntry]) -> Vec<&str> {
    v.iter().map(|s| s.name.as_str()).collect()
}

fn off(ids: &[&str]) -> StateV2 {
    let mut st = StateV2::default();
    for i in ids {
        let id = PluginId::parse(i).unwrap();
        if id.is_part() {
            st.parts_off.push(id);
        } else {
            st.disabled.push(i.to_string());
        }
    }
    st
}

const LIMIT: Duration = Duration::from_secs(3);

/// Every path the scans listed or stat'd (a read stats first), in this
/// test binary: the guarded-link tests look for theirs.
static TOUCHED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub(super) fn touched(p: &Path) {
    TOUCHED.lock().unwrap_or_else(|e| e.into_inner()).push(p.to_path_buf());
}

fn was_touched(under: &Path) -> Vec<PathBuf> {
    TOUCHED.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|p| p.starts_with(under)).cloned().collect()
}

/// A fake home whose ~/.vibe/skills/x/SKILL.md links into
/// <home>/Documents (his grill-me), next to a plain skill.
fn linked_home(tag: &str) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let home = t.0.join("home");
    let target = skill(&home.join("Documents/ai"), "x", "x", "in his documents");
    std::fs::create_dir_all(home.join(".vibe/skills/x")).unwrap();
    let link = home.join(".vibe/skills/x/SKILL.md");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    // a whole skill folder linked in too (relative)
    std::os::unix::fs::symlink("../../Documents/ai/x", home.join(".agents/skills_y")).ok();
    skill(&home.join(".agents/skills"), "plain", "plain", "his own");
    (t, home, link)
}

/// Which folders each agent reads: the workspace and the user's two
/// always; main bise's prompts/skills; prompts/skills-all only where the
/// desktop is on (architect m_12576), main or task; a bare REPL none of
/// bise's.
#[test]
fn roots_per_role_and_desktop_flag() {
    let (home, ws, app) = (Path::new("/h"), Path::new("/w"), Path::new("/app"));
    let dirs = |b: BiseSkills| -> Vec<(SkillsScope, PathBuf)> {
        roots(Some(home), ws, Some(app), b).into_iter().map(|r| (r.scope, r.dir)).collect()
    };
    let common = |mid: Vec<&str>| -> Vec<(SkillsScope, PathBuf)> {
        let mut v = vec![(SkillsScope::Workspace, PathBuf::from("/w/.agents/skills"))];
        v.extend(mid.into_iter().map(|d| (SkillsScope::Bise, PathBuf::from(d))));
        v.push((SkillsScope::Personal, PathBuf::from("/h/.agents/skills")));
        v.push((SkillsScope::Vibe, PathBuf::from("/h/.vibe/skills")));
        v
    };
    assert_eq!(dirs(BiseSkills::of(false, false)), common(vec![]));
    assert_eq!(dirs(BiseSkills::of(true, false)), common(vec!["/app/prompts/skills"]));
    assert_eq!(dirs(BiseSkills::of(false, true)), common(vec!["/app/prompts/skills-all"]));
    assert_eq!(dirs(BiseSkills::of(true, true)), common(vec!["/app/prompts/skills-all", "/app/prompts/skills"]));
    // no app root (a bare REPL): no bise folder whatever the role
    assert_eq!(roots(Some(home), ws, None, BiseSkills::MainAndAll).len(), 3);
    // no HOME: the workspace only
    assert_eq!(roots(None, ws, None, BiseSkills::None).len(), 1);
}

/// `$BISE_SKILL_ROOTS` round-trips, splits on the first `=`, and refuses
/// what it cannot read (never a guess).
#[test]
fn the_env_roots_round_trip_and_refuse_junk() {
    let rs = roots(Some(Path::new("/h")), Path::new("/w=x"), Some(Path::new("/app")), BiseSkills::MainAndAll);
    let (k, v) = Root::env(&rs);
    assert_eq!(k, ENV);
    assert_eq!(Root::decode(&v).unwrap(), rs);
    assert!(v.contains("workspace=/w=x/.agents/skills\n"), "{v}");
    assert!(Root::decode("").unwrap().is_empty());
    assert!(Root::decode("nope=/x").unwrap_err().contains("unknown skills folder"));
    assert!(Root::decode("/just/a/path").unwrap_err().contains("no `=`"));
    assert!(Root::decode("vibe=").unwrap_err().contains("no path"));
    // a path with a newline cannot cross: it is left out, the rest kept
    let bad = vec![Root { scope: SkillsScope::Vibe, dir: PathBuf::from("/a\nb") }, rs[0].clone()];
    assert_eq!(Root::decode(&Root::env(&bad).1).unwrap(), vec![rs[0].clone()]);
}

/// The scan reads each root's skills with their owner and real path,
/// skips what is not a skill, and the precedence keeps the first name:
/// workspace, then plugins, then the rest.
#[test]
fn scan_owners_and_precedence() {
    let t = Tmp::new("scan");
    let ws = t.0.join("ws/.agents/skills");
    let home = t.0.join("home");
    skill(&ws, "beta", "beta", "the workspace's");
    skill(&home.join(".agents/skills"), "alpha", "alpha", "\"quoted\"\tpersonal");
    skill(&home.join(".vibe/skills"), "beta", "beta", "vibe's, shadowed");
    // not skills: no description, a name with a space, a folder without SKILL.md
    std::fs::create_dir_all(home.join(".vibe/skills/empty")).unwrap();
    std::fs::create_dir_all(home.join(".vibe/skills/nodesc")).unwrap();
    std::fs::write(home.join(".vibe/skills/nodesc/SKILL.md"), "name: nodesc\n").unwrap();
    std::fs::create_dir_all(home.join(".vibe/skills/spaced")).unwrap();
    std::fs::write(home.join(".vibe/skills/spaced/SKILL.md"), "name: a b\ndescription: d\n").unwrap();
    let rs = roots(Some(&home), &t.0.join("ws"), None, BiseSkills::None);
    let s = scan(&rs, LIMIT, &Mode::Read, None);
    assert!(s.stuck.is_empty());
    assert_eq!(names(&s.skills), ["beta", "alpha", "beta"]);
    assert_eq!(s.skills[1].owner.to_string(), "skills:personal");
    assert_eq!(s.skills[1].description, "quotedpersonal");
    assert_eq!(s.skills[1].path, home.join(".agents/skills/alpha/SKILL.md"));
    let plugin = SkillEntry { name: "alpha".into(), description: "plugin's".into(), path: "/p/SKILL.md".into(), owner: PluginId::plugin("docs") };
    let all = ordered(s.skills, vec![plugin]);
    assert_eq!(names(&all), ["beta", "alpha"]);
    assert_eq!(all[0].owner.to_string(), "skills:workspace");
    assert_eq!(all[1].owner.to_string(), "docs", "a plugin's skill wins over a personal one");
    let idx = index(&all);
    assert_eq!(idx, format!("beta\tthe workspace's\t{}\nalpha\tplugin's\t/p/SKILL.md\n", ws.join("beta/SKILL.md").display()));
}

/// An off skill or an off folder is left out; skills:bise cannot be
/// turned off, by its folder or by one skill.
#[test]
fn off_skills_are_left_out_and_bise_is_locked() {
    let t = Tmp::new("off");
    let app = t.0.join("app");
    skill(&app.join("prompts/skills-all"), "bise-pages", "bise-pages", "pages");
    skill(&t.0.join("home/.agents/skills"), "a", "a", "one");
    skill(&t.0.join("home/.agents/skills"), "b", "b", "two");
    skill(&t.0.join("home/.vibe/skills"), "v", "v", "three");
    let rs = roots(Some(&t.0.join("home")), &t.0.join("ws"), Some(&app), BiseSkills::All);
    let found = scan(&rs, LIMIT, &Mode::Read, None).skills;
    assert_eq!(names(&on(found.clone(), &StateV2::default())), ["bise-pages", "a", "b", "v"]);
    assert_eq!(names(&on(found.clone(), &off(&["skills:personal#skill:a"]))), ["bise-pages", "b", "v"]);
    assert_eq!(names(&on(found.clone(), &off(&["skills:vibe"]))), ["bise-pages", "a", "b"]);
    let st = off(&["skills:bise", "skills:bise#skill:bise-pages"]);
    assert_eq!(names(&on(found, &st)), ["bise-pages", "a", "b", "v"]);
}

/// The fingerprint moves when a SKILL.md comes, changes or goes, and when
/// a skill or a folder is turned off; it stays put otherwise, and for a
/// part of another folder or a state change on skills:bise (locked).
#[test]
fn the_fingerprint_moves_on_a_change_or_an_off_skill() {
    let t = Tmp::new("fp");
    let home = t.0.join("home");
    let rs = roots(Some(&home), &t.0.join("ws"), Some(&t.0.join("app")), BiseSkills::All);
    let fp = |st: &StateV2| fingerprint(&rs, st, LIMIT, &Mode::Read).0;
    let none = StateV2::default();
    let empty = fp(&none);
    std::fs::create_dir_all(home.join(".agents/skills/notes")).unwrap();
    assert_eq!(fp(&none), empty, "a folder without SKILL.md is no skill");
    let f = skill(&home.join(".agents/skills"), "a", "a", "one");
    let added = fp(&none);
    assert_ne!(added, empty);
    assert_eq!(fp(&none), added, "stable while nothing changes");
    std::fs::write(&f, "---\nname: a\ndescription: two, longer\n---\n").unwrap();
    let edited = fp(&none);
    assert_ne!(edited, added);
    assert_ne!(fp(&off(&["skills:personal#skill:a"])), edited, "a skill turned off");
    assert_ne!(fp(&off(&["skills:personal"])), edited, "its folder turned off");
    assert_eq!(fp(&off(&["docs#skill:a"])), edited, "another plugin's part");
    assert_eq!(fp(&off(&["skills:bise#skill:bise-pages"])), edited, "skills:bise is locked");
    std::fs::remove_dir_all(f.parent().unwrap()).unwrap();
    assert_eq!(fp(&none), empty);
}

/// A SKILL.md that is a FIFO is no regular file: never opened, no thread
/// left on it, no skip to name.
#[test]
fn a_fifo_skill_is_skipped_without_a_wait() {
    let t = Tmp::new("fifo");
    let dir = t.0.join("ws/.agents/skills");
    skill(&dir, "ok", "ok", "fine");
    std::fs::create_dir_all(dir.join("fifo")).unwrap();
    let fifo = dir.join("fifo/SKILL.md");
    let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: mkfifo on a path this test owns
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    let rs = roots(None, &t.0.join("ws"), None, BiseSkills::None);
    let at = Instant::now();
    let s = scan(&rs, LIMIT, &Mode::Read, None);
    assert!(at.elapsed() < Duration::from_secs(1), "{:?}", at.elapsed());
    assert_eq!(names(&s.skills), ["ok"]);
    assert!(s.stuck.is_empty());
    assert!(!stuck_set(|s| s.contains(&fifo)));
}

/// A read that never answers (macOS's privacy wait): skipped at the
/// deadline and named once; the next scan neither waits on it nor starts
/// a second thread for it; once it answers it is read again.
#[test]
fn a_stuck_path_is_named_once_and_never_rescanned_while_stuck() {
    static GATE: Mutex<()> = Mutex::new(());
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn hang(p: &Path) -> bool {
        RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if p.ends_with("slow") {
            drop(GATE.lock());
        }
        true
    }
    let paths = vec![PathBuf::from("/x/fast"), PathBuf::from("/x/slow")];
    let held = GATE.lock().unwrap();
    let short = Duration::from_millis(100);
    let (done, fresh) = timed(paths.clone(), Instant::now() + short, hang);
    assert_eq!(done.iter().map(|d| d.0.clone()).collect::<Vec<_>>(), [PathBuf::from("/x/fast")]);
    assert_eq!(fresh, [PathBuf::from("/x/slow")]);
    let at = Instant::now();
    let (done, fresh) = timed(paths.clone(), Instant::now() + Duration::from_secs(3), hang);
    assert!(at.elapsed() < Duration::from_secs(1), "no wait on a stuck path");
    assert_eq!(done.len(), 1);
    assert!(fresh.is_empty(), "named once");
    assert_eq!(RUNS.load(std::sync::atomic::Ordering::SeqCst), 3, "no second thread on the stuck one");
    drop(held);
    let end = Instant::now() + Duration::from_secs(3);
    while stuck_set(|s| s.contains(Path::new("/x/slow"))) && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(5));
    }
    let (done, _) = timed(paths, Instant::now() + Duration::from_secs(3), hang);
    assert_eq!(done.len(), 2, "answered: read again");
}

#[test]
fn parse_reads_the_first_name_and_description() {
    assert_eq!(parse("---\nname: a\ndescription: \"First\" text\r\n---\n"), Some(("a".into(), "First text".into())));
    assert_eq!(parse("name: a\n"), None);
    assert_eq!(parse("name: a b\ndescription: d\n"), None);
    assert_eq!(parse("name: a\ndescription: one\nname: b\ndescription: two\n"), Some(("a".into(), "one".into())));
}

/// His switch off (the default): a SKILL.md that links into Documents is
/// never stat'd or read, by the scan or the fingerprint; the scan names
/// it from its folder for the page, the agents and the `$` popup.
/// Mutation: `guard` returning None turns this red (x/SKILL.md stat'd).
#[test]
fn list_mode_never_stats_or_reads_through_a_link_into_documents() {
    let (t, home, link) = linked_home("list");
    let mode = mode_for(None, &home);
    assert_eq!(mode, Mode::List { home: home.clone() }, "off unless he turned it on");
    let rs = roots(Some(&home), &t.0.join("ws"), None, BiseSkills::None);
    let s = scan(&rs, LIMIT, &mode, Some(&home));
    assert_eq!(names(&s.skills), ["plain"], "x is not read");
    assert_eq!(s.guarded, [Guarded { name: "x".into(), path: link.clone(), owner: PluginId::Skills { scope: SkillsScope::Vibe }, folder: "Documents".into(), read: false }]);
    assert_eq!(names(&s.listed), ["plain", "x"]);
    assert_eq!(s.listed[1].description, unread_description("Documents"));
    assert_eq!(s.listed[1].path, link);
    let st = StateV2::default();
    let fp = |m: &Mode| fingerprint(&rs, &st, LIMIT, m).0;
    let before = fp(&mode);
    std::fs::write(home.join("Documents/ai/x/SKILL.md"), "name: x\ndescription: edited, longer\n").unwrap();
    assert_eq!(fp(&mode), before, "an edit inside Documents is not seen while off");
    assert_eq!(was_touched(&link), Vec::<PathBuf>::new(), "no stat or read through the link");
    assert_eq!(was_touched(&home.join("Documents")), Vec::<PathBuf>::new());
}

/// His switch on: read as before, and still named as linked (the
/// switch's count line).
#[test]
fn read_mode_reads_through_the_link_and_names_it() {
    let (t, home, link) = linked_home("read");
    let on = serde_json::json!({"skills": {"read_guarded": true}});
    let mode = mode_for(Some(&on), &home);
    assert_eq!(mode, Mode::Read);
    let rs = roots(Some(&home), &t.0.join("ws"), None, BiseSkills::None);
    let s = scan(&rs, LIMIT, &mode, Some(&home));
    assert_eq!(names(&s.skills), ["plain", "x"]);
    assert_eq!(s.skills[1].description, "in his documents");
    assert_eq!(s.listed, s.skills);
    assert_eq!(s.guarded.len(), 1);
    assert!(s.guarded[0].read && s.guarded[0].folder == "Documents" && s.guarded[0].path == link);
    let st = StateV2::default();
    let before = fingerprint(&rs, &st, LIMIT, &mode).0;
    std::fs::write(home.join("Documents/ai/x/SKILL.md"), "name: x\ndescription: edited, longer\n").unwrap();
    assert_ne!(fingerprint(&rs, &st, LIMIT, &mode).0, before, "on: an edit is seen");
    assert_ne!(fingerprint(&rs, &st, LIMIT, &mode_for(None, &home)).0, before, "the switch itself moves it");
}

/// A whole skills folder linked into Documents is not listed while off.
#[test]
fn a_root_linked_into_documents_is_not_listed() {
    let t = Tmp::new("root");
    let home = t.0.join("home");
    skill(&home.join("Documents/skills"), "z", "z", "zed");
    std::fs::create_dir_all(home.join(".agents")).unwrap();
    std::os::unix::fs::symlink(home.join("Documents/skills"), home.join(".agents/skills")).unwrap();
    let rs = roots(Some(&home), &t.0.join("ws"), None, BiseSkills::None);
    let s = scan(&rs, LIMIT, &Mode::List { home: home.clone() }, Some(&home));
    assert!(s.skills.is_empty() && s.listed.is_empty());
    assert_eq!(was_touched(&home.join(".agents/skills")), Vec::<PathBuf>::new());
    assert_eq!(names(&scan(&rs, LIMIT, &Mode::Read, Some(&home)).skills), ["z"]);
}

/// A throwaway home (`$BISE_HOME` elsewhere) never reads his own folders.
#[test]
fn an_explicit_home_scans_no_user_folder() {
    let env = |vars: &'static [(&'static str, &'static str)]| move |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
    let real = bise_home::Home::from_lookup(&env(&[("HOME", "/Users/g")]));
    assert_eq!(user_home(&real), Some(Path::new("/Users/g")));
    let throwaway = bise_home::Home::from_lookup(&env(&[("HOME", "/Users/g"), ("BISE_HOME", "/tmp/x")]));
    assert_eq!(user_home(&throwaway), None);
    assert_eq!(roots(user_home(&throwaway), Path::new("/w"), None, BiseSkills::None).len(), 1, "the workspace's only");
}
