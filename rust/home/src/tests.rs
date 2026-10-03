use super::*;
use std::collections::HashMap;
use serde_json::Value;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-home-test-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn home_of(pairs: &[(&str, &str)]) -> Home {
    let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    Home::from_lookup(&move |k: &str| m.get(k).cloned())
}

#[test]
fn the_old_layout_keeps_todays_paths() {
    let h = home_of(&[("HOME", "/h"), ("XDG_STATE_HOME", "/xdg")]);
    assert_eq!(h.layout(), Layout::Legacy);
    let p = |s: &str| PathBuf::from(s);
    assert_eq!(h.root(), p("/h/.bend-harness"));
    assert_eq!(h.config_file(), p("/h/.bend-harness/config.toml"));
    assert_eq!(h.key_file(), p("/h/.bend-harness/.env"));
    assert_eq!(h.env_files(), vec![p("/h/.bend-harness/.env"), p("/h/.vibe/.env")]);
    assert_eq!(h.sessions_dir(), p("/h/.bend-harness/sessions"));
    // XDG_STATE_HOME is not read any more
    assert_eq!(h.hub_dir("ws-12345678"), p("/h/.local/state/switchboard/ws-12345678"));
    assert_eq!(h.worktrees_dir(), p("/h/.local/state/switchboard/worktrees"));
    assert_eq!(h.images_dir(), p("/h/.bend-harness/images"));
    assert_eq!(h.crashes_dir(), p("/h/.bend-harness/crashes"));
    assert_eq!(h.mcp_index(), p("/h/.bend-harness/mcp-index.txt"));
    assert_eq!(h.skills_index(), p("/h/.bend-harness/skills-index.txt"));
    assert_eq!(h.plugins_state(), p("/h/.bend-harness/plugins.json"));
    assert_eq!(h.plugin_data_dir(), p("/h/.bend-harness/plugin-data"));
    assert_eq!(h.run_dir(), p("/h/.bend-harness/run"));
    assert_eq!(h.drafts_dir(), p("/h/.local/state/switchboard/drafts"));
    assert_eq!(h.versions_dir(), p("/h/.local/state/switchboard/versions"));
    assert_eq!(h.build_dir(), p("/h/.local/state/switchboard/build"));
    assert_eq!(h.pref(Pref::Theme), Slot::key("/h/.bend-harness/tui.json", "theme"));
    assert_eq!(h.pref(Pref::Voice), Slot::key("/h/.bend-harness/tui.json", "voice_mode_enabled"));
    assert_eq!(h.pref(Pref::Hints), Slot::file("/h/.local/state/switchboard/hints.json"));
    assert_eq!(h.pref(Pref::Tip), Slot::file("/h/.local/state/switchboard/tip"));
    assert_eq!(h.pref(Pref::Onboarded), Slot::file("/h/.local/state/switchboard/onboarded"));
}

#[test]
fn bise_home_moves_everything() {
    let h = home_of(&[("HOME", "/h"), ("BISE_HOME", "/b")]);
    assert_eq!(h.layout(), Layout::Bise);
    let p = |s: &str| PathBuf::from(s);
    assert_eq!(h.root(), p("/b"));
    assert_eq!(h.user_home(), p("/h"));
    assert_eq!(h.config_file(), p("/b/config.toml"));
    assert_eq!(h.auth_file(), p("/b/auth.json"));
    assert_eq!(h.env_files(), vec![p("/b/.env"), p("/h/.bend-harness/.env"), p("/h/.vibe/.env")]);
    assert_eq!(h.sessions_dir(), p("/b/sessions"));
    assert_eq!(h.hub_dir("ws-1"), p("/b/hubs/ws-1"));
    assert_eq!(h.worktrees_dir(), p("/b/worktrees"));
    assert_eq!(h.mcp_index(), p("/b/cache/mcp-index.txt"));
    assert_eq!(h.skills_index(), p("/b/cache/skills-index.txt"));
    assert_eq!(h.run_dir(), p("/b/run"));
    assert_eq!(h.drafts_dir(), p("/b/drafts"));
    assert_eq!(h.versions_dir(), p("/b/dev/versions"));
    assert_eq!(h.build_dir(), p("/b/dev/build"));
    for pref in [Pref::Voice, Pref::Theme, Pref::Hints, Pref::Tip, Pref::Onboarded, Pref::Setup] {
        assert_eq!(h.pref(pref), Slot::key("/b/prefs.json", pref.key()));
    }
}

#[test]
fn the_migration_marker_turns_dot_bise_on() {
    let d = tmp("marker");
    let hs = d.to_string_lossy().to_string();
    assert_eq!(home_of(&[("HOME", &hs)]).layout(), Layout::Legacy);
    std::fs::create_dir_all(d.join(".bise")).unwrap();
    assert_eq!(home_of(&[("HOME", &hs)]).layout(), Layout::Legacy, "an empty ~/.bise is not enough");
    std::fs::write(d.join(".bise").join(MIGRATED), "{}").unwrap();
    let h = home_of(&[("HOME", &hs)]);
    assert_eq!((h.layout(), h.root()), (Layout::Bise, d.join(".bise").as_path()));
    assert_eq!(h.hubs_dir(), d.join(".bise/hubs"));
}

#[test]
fn no_home_falls_back_to_tmp() {
    let h = home_of(&[]);
    assert_eq!(h.config_file(), PathBuf::from("/tmp/.bend-harness/config.toml"));
    assert_eq!(home_of(&[("HOME", "")]).root(), Path::new("/tmp/.bend-harness"));
}

#[test]
fn overrides_win_and_are_exported() {
    let h = home_of(&[("HOME", "/h"), ("BEND_CONFIG", "/c.toml"), ("SB_VERSIONS_DIR", "/v")]);
    assert_eq!(h.config_file(), PathBuf::from("/c.toml"));
    assert_eq!(h.versions_dir(), PathBuf::from("/v"));
    let ex: HashMap<_, _> = h.exports().into_iter().collect();
    assert_eq!(ex["BEND_CONFIG"], "/c.toml");
    assert_eq!(ex["BEND_SESSIONS_DIR"], "/h/.bend-harness/sessions");
    for k in PATH_VARS {
        assert!(ex.contains_key(k), "{k} not exported");
    }
    assert!(!ex.contains_key("BISE_HOME"), "legacy: BISE_HOME would turn the new layout on");
    let b: HashMap<_, _> = home_of(&[("HOME", "/h"), ("BISE_HOME", "/b")]).exports().into_iter().collect();
    assert_eq!(b["BISE_HOME"], "/b");
}

#[test]
fn exported_paths_follow_the_same_home_and_only_it() {
    let parent = home_of(&[("HOME", "/h")]);
    let mut env: HashMap<String, String> = parent.exports().into_iter().map(|(k, v)| (k.into(), v)).collect();
    // a child with the same HOME: the exports are read back unchanged
    env.insert("HOME".into(), "/h".into());
    let child = Home::from_lookup(&|k: &str| env.get(k).cloned());
    assert_eq!(child.exports(), parent.exports());
    // a test with a temp HOME started from that shell: the inherited paths are ignored
    env.insert("HOME".into(), "/t".into());
    let test = Home::from_lookup(&|k: &str| env.get(k).cloned());
    assert_eq!(test.sessions_dir(), PathBuf::from("/t/.bend-harness/sessions"));
    assert_eq!(test.config_file(), PathBuf::from("/t/.bend-harness/config.toml"));
    // same HOME, a BISE_HOME of its own: ignored too
    env.insert("HOME".into(), "/h".into());
    env.insert("BISE_HOME".into(), "/b".into());
    let moved = Home::from_lookup(&|k: &str| env.get(k).cloned());
    assert_eq!(moved.config_file(), PathBuf::from("/b/config.toml"));
    // a hand-set override without a stamp is honoured
    let own = home_of(&[("HOME", "/t"), ("BEND_CONFIG", "/mine.toml")]);
    assert_eq!(own.config_file(), PathBuf::from("/mine.toml"));
}

#[test]
fn an_inherited_legacy_default_is_not_an_override_in_the_bise_layout() {
    // an older version exported BEND_MCP_INDEX=~/.bend-harness/mcp-index.txt
    // (no stamp); the bise hub took it as an override and exported it again
    // with its own stamp: the agents read an empty connector index
    let d = tmp("legacy-default");
    std::fs::create_dir_all(d.join(".bise")).unwrap();
    std::fs::write(d.join(".bise").join(MIGRATED), "{}").unwrap();
    let hs = d.to_string_lossy().to_string();
    let old_index = d.join(".bend-harness/mcp-index.txt").to_string_lossy().to_string();
    let old_skills = d.join(".bend-harness/skills-index.txt").to_string_lossy().to_string();
    let new_index = d.join(".bise/cache/mcp-index.txt");
    let no_stamp = home_of(&[("HOME", &hs), ("BEND_MCP_INDEX", &old_index), ("BEND_SKILLS_INDEX", &old_skills)]);
    assert_eq!(no_stamp.mcp_index(), new_index);
    assert_eq!(no_stamp.skills_index(), d.join(".bise/cache/skills-index.txt"));
    // the poisoned export (a valid stamp) heals too
    let mut env: HashMap<String, String> = no_stamp.exports().into_iter().map(|(k, v)| (k.into(), v)).collect();
    env.insert("HOME".into(), hs.clone());
    env.insert("BEND_MCP_INDEX".into(), old_index.clone());
    let stamped = Home::from_lookup(&|k: &str| env.get(k).cloned());
    assert_eq!(stamped.mcp_index(), new_index);
    let ex: HashMap<_, _> = stamped.exports().into_iter().collect();
    assert_eq!(ex["BEND_MCP_INDEX"], new_index.to_string_lossy());
    // any other path is still a real override
    let mine = home_of(&[("HOME", &hs), ("BEND_MCP_INDEX", "/elsewhere/idx.txt")]);
    assert_eq!(mine.mcp_index(), PathBuf::from("/elsewhere/idx.txt"));
    // the legacy layout keeps its own defaults as overrides (same value)
    let legacy = home_of(&[("HOME", "/h"), ("BEND_MCP_INDEX", "/h/.bend-harness/mcp-index.txt")]);
    assert_eq!(legacy.mcp_index(), PathBuf::from("/h/.bend-harness/mcp-index.txt"));
}

#[test]
fn another_homes_default_is_not_an_override() {
    // an agent's shell (HOME=/u, bise layout, exports stamped for /u) ran a
    // test with a temp HOME and BISE_EXPORTS_FOR unset: the test's REPL took
    // BEND_SKILLS_INDEX=/u/.bise/cache/skills-index.txt as an override,
    // scanned the temp HOME (no skills) and wrote an empty index over the
    // user's one: the skill tool found no skill in main's session
    let t = tmp("foreign-default");
    let ts = t.to_string_lossy().to_string();
    let inherited = [
        ("BEND_CONFIG", "/u/.bise/config.toml"),
        ("BEND_SESSIONS_DIR", "/u/.bise/sessions"),
        ("BEND_IMAGE_DIR", "/u/.bise/images"),
        ("BEND_MCP_INDEX", "/u/.bend-harness/mcp-index.txt"),
        ("BEND_SKILLS_INDEX", "/u/.bise/cache/skills-index.txt"),
        ("BEND_PLUGINS_STATE", "/u/.bise/plugins.json"),
        ("BEND_PLUGINS_DATA", "/u/.bise/plugin-data"),
        ("BEND_RUN_DIR", "/u/.bise/run"),
        ("SB_VERSIONS_DIR", "/u/.bise/dev/versions"),
        ("SB_BUILD_DIR", "/u/.local/state/switchboard/build"),
    ];
    let mut pairs = vec![("HOME", ts.as_str())];
    pairs.extend(inherited);
    let h = home_of(&pairs);
    assert_eq!(h, home_of(&[("HOME", &ts)]), "every inherited path was /u's default");
    assert_eq!(h.skills_index(), t.join(".bend-harness/skills-index.txt"));
    // every exported variable is covered by the rule
    assert_eq!(inherited.len(), PATH_VARS.len());
    // this HOME's own defaults and any other path stay overrides
    let own = t.join(".bend-harness/skills-index.txt").to_string_lossy().to_string();
    let kept = home_of(&[("HOME", &ts), ("BEND_SKILLS_INDEX", &own), ("BEND_CONFIG", "/u/cfg/config.toml")]);
    assert_eq!(kept.skills_index(), PathBuf::from(&own));
    assert_eq!(kept.config_file(), PathBuf::from("/u/cfg/config.toml"));
    // same in the bise layout: /u's defaults dropped, its own kept
    std::fs::create_dir_all(t.join(".bise")).unwrap();
    std::fs::write(t.join(".bise").join(MIGRATED), "{}").unwrap();
    let bise_own = t.join(".bise/cache/skills-index.txt").to_string_lossy().to_string();
    let b = home_of(&[("HOME", &ts), ("BEND_SKILLS_INDEX", "/u/.bise/cache/skills-index.txt")]);
    assert_eq!(b.skills_index(), PathBuf::from(&bise_own));
    let b_own = home_of(&[("HOME", &ts), ("BEND_SKILLS_INDEX", &bise_own), ("BEND_SESSIONS_DIR", "/u/.bise/sessions")]);
    assert_eq!(b_own.skills_index(), PathBuf::from(&bise_own));
    assert_eq!(b_own.sessions_dir(), t.join(".bise/sessions"));
}

#[test]
fn the_run_dir_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp("run");
    let h = Home::at(&d);
    std::fs::create_dir_all(h.run_dir()).unwrap();
    std::fs::set_permissions(h.run_dir(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let r = h.ensure_run_dir().unwrap();
    assert_eq!(std::fs::metadata(r).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
fn prefs_share_one_file_and_keep_the_other_keys() {
    let d = tmp("prefs");
    let h = Home::at(&d);
    h.pref(Pref::Theme).set("dark".into()).unwrap();
    h.pref(Pref::Hints).set(serde_json::json!({"first_card": true})).unwrap();
    h.pref(Pref::Tip).set(3.into()).unwrap();
    assert_eq!(h.pref(Pref::Theme).get(), Some("dark".into()));
    assert_eq!(h.pref(Pref::Tip).get(), Some(3.into()));
    assert_eq!(h.pref(Pref::Onboarded).get(), None);
    let all: Value = serde_json::from_str(&std::fs::read_to_string(h.prefs_file()).unwrap()).unwrap();
    assert_eq!(all["hints"]["first_card"], true);
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1, "no tmp file left");
}

#[test]
fn legacy_prefs_read_the_old_files() {
    let d = tmp("legacy-prefs");
    let hs = d.to_string_lossy().to_string();
    let st = d.join(".local/state/switchboard");
    std::fs::create_dir_all(&st).unwrap();
    std::fs::create_dir_all(d.join(".bend-harness")).unwrap();
    std::fs::write(st.join("onboarded"), "1\n").unwrap();
    std::fs::write(st.join("tip"), "4\n").unwrap();
    std::fs::write(d.join(".bend-harness/tui.json"), r#"{"voice_mode_enabled": true}"#).unwrap();
    let h = home_of(&[("HOME", &hs)]);
    assert_eq!(h.pref(Pref::Onboarded).get(), Some(1.into()));
    assert_eq!(h.pref(Pref::Tip).get(), Some(4.into()));
    assert_eq!(h.pref(Pref::Voice).get(), Some(true.into()));
    h.pref(Pref::Theme).set("light".into()).unwrap();
    let tui: Value = serde_json::from_str(&std::fs::read_to_string(d.join(".bend-harness/tui.json")).unwrap()).unwrap();
    assert_eq!((tui["voice_mode_enabled"].clone(), tui["theme"].clone()), (true.into(), "light".into()));
    // a flag file that is not JSON still reads as set
    std::fs::write(st.join("onboarded"), "yes\n").unwrap();
    assert_eq!(h.pref(Pref::Onboarded).get(), Some("yes".into()));
}

// ---- the migration (BISE-161) ----

fn sh(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new(args[0]).args(&args[1..]).current_dir(dir).output().unwrap();
    assert!(ok.status.success(), "{args:?}: {}", String::from_utf8_lossy(&ok.stderr));
}

/// A HOME with today's layout: user files, TUI state, two hubs (one with
/// a git worktree), dev versions, an old `.moved-` leftover.
fn old_layout(name: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp(name);
    let root = d.join(".bend-harness");
    let st = d.join(".local/state/switchboard");
    for p in ["sessions", "images", "run/123", "cache"] {
        std::fs::create_dir_all(root.join(p)).unwrap();
    }
    std::fs::write(root.join("config.toml"), "model = \"zai-glm-5-3\"\n").unwrap();
    std::fs::write(root.join(".env"), "MISTRAL_API_KEY=k\n").unwrap();
    std::fs::set_permissions(root.join(".env"), std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(root.join("tui.json"), r#"{"voice_mode_enabled": true, "theme": "light"}"#).unwrap();
    std::fs::write(root.join("sessions/s1.txt"), "session").unwrap();
    std::fs::write(root.join("images/a.png"), "png").unwrap();
    std::fs::write(root.join("run/123/x"), "side channel").unwrap();
    std::fs::write(root.join("mcp-index.txt"), "idx").unwrap();
    std::fs::write(root.join("cache/models.toml"), "m").unwrap();
    for p in ["drafts", "versions/abc", "build/cache", "idle-0000000a/agents/main", "busy-0000000b", "x-1234abcd.moved-20260928"] {
        std::fs::create_dir_all(st.join(p)).unwrap();
    }
    std::fs::write(st.join("onboarded"), "1\n").unwrap();
    std::fs::write(st.join("hints.json"), r#"{"first_card": true}"#).unwrap();
    std::fs::write(st.join("tip"), "4\n").unwrap();
    std::fs::write(st.join("drafts/ws.json"), "{}").unwrap();
    std::fs::write(st.join("idle-0000000a/journal.jsonl"), "{\"t\":\"idle\"}\n").unwrap();
    std::fs::write(st.join("busy-0000000b/journal.jsonl"), "{\"t\":\"busy\"}\n").unwrap();
    // a repository whose task worktree lives in the idle hub
    let repo = d.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    sh(&repo, &["git", "init", "-q"]);
    std::fs::write(repo.join("f"), "x").unwrap();
    sh(&repo, &["git", "add", "f"]);
    // never the user's signing (an ssh agent that refuses in the background)
    sh(&repo, &["git", "-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "commit", "-qm", "i"]);
    let wt = st.join("idle-0000000a/worktrees/t1");
    sh(&repo, &["git", "worktree", "add", "-q", "--detach", wt.to_str().unwrap()]);
    d
}

fn home_env(d: &Path) -> Home {
    home_of(&[("HOME", &d.to_string_lossy())])
}

fn read(p: PathBuf) -> String {
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn the_migration_copies_the_user_files_and_moves_the_idle_hubs() {
    use std::os::unix::fs::PermissionsExt;
    let d = old_layout("migrate");
    let st = d.join(".local/state/switchboard");
    let bise = d.join(".bise");
    assert_eq!(home_env(&d).layout(), Layout::Legacy);
    let running = std::cell::Cell::new(true);
    let busy = |p: &Path| running.get() && p.ends_with("busy-0000000b");
    let r = migrate(&d, &busy).unwrap();
    assert!(r.first && r.errors.is_empty(), "{r:?}");
    assert_eq!((r.hubs_moved.clone(), r.hubs_kept.clone()), (vec!["idle-0000000a".to_string()], vec!["busy-0000000b".to_string()]));

    // user files: copied (the old ones stay), modes kept, run/ left out
    let h = home_env(&d);
    assert_eq!((h.layout(), h.root()), (Layout::Bise, bise.as_path()));
    assert_eq!(std::fs::metadata(&bise).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(read(h.config_file()), "model = \"zai-glm-5-3\"\n");
    assert_eq!(std::fs::metadata(h.key_file()).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(read(h.sessions_dir().join("s1.txt")), "session");
    assert_eq!(read(h.images_dir().join("a.png")), "png");
    assert_eq!(read(h.mcp_index()), "idx");
    assert_eq!(read(h.cache_dir().join("models.toml")), "m");
    assert_eq!(read(h.drafts_dir().join("ws.json")), "{}");
    assert!(!bise.join("run").exists());
    assert!(d.join(".bend-harness/config.toml").exists() && st.join("hints.json").exists(), "old files stay");
    // prefs: the four old files in one
    assert_eq!(h.pref(Pref::Voice).get(), Some(true.into()));
    assert_eq!(h.pref(Pref::Theme).get(), Some("light".into()));
    assert_eq!(h.pref(Pref::Hints).get(), Some(serde_json::json!({"first_card": true})));
    assert_eq!(h.pref(Pref::Tip).get(), Some(4.into()));
    assert_eq!(h.pref(Pref::Onboarded).get(), Some(true.into()));
    // dev: links to the versions and build cache, nothing moved
    assert_eq!(std::fs::read_link(bise.join("dev/versions")).unwrap(), st.join("versions"));
    assert!(h.versions_dir().join("abc").is_dir() && st.join("versions/abc").is_dir());

    // the idle hub moved; its old path is a link (an older binary opens the same hub)
    let idle = bise.join("hubs/idle-0000000a");
    assert_eq!(h.hub_dir("idle-0000000a"), idle);
    assert_eq!(read(idle.join("journal.jsonl")), "{\"t\":\"idle\"}\n");
    assert_eq!(std::fs::read_link(st.join("idle-0000000a")).unwrap(), idle);
    let old_home = h.legacy();
    assert_eq!(read(old_home.hub_dir("idle-0000000a").join("journal.jsonl")), "{\"t\":\"idle\"}\n");
    // its worktree: the repository points at the new place
    let list = std::process::Command::new("git").args(["worktree", "list", "--porcelain"]).current_dir(d.join("repo")).output().unwrap();
    let list = String::from_utf8_lossy(&list.stdout).to_string();
    let wt = idle.join("worktrees/t1").canonicalize().unwrap();
    assert!(list.contains(&format!("worktree {}", wt.display())), "{list}");
    sh(&wt, &["git", "status", "--short"]);

    // the running hub stays in the old place and is used there
    assert!(std::fs::symlink_metadata(st.join("busy-0000000b")).unwrap().is_dir());
    assert_eq!(h.hub_dir("busy-0000000b"), st.join("busy-0000000b"));
    // a new workspace goes to hubs/; the leftover is not a hub
    assert_eq!(h.hub_dir("new-00000001"), bise.join("hubs/new-00000001"));
    assert!(st.join("x-1234abcd.moved-20260928").is_dir());
    assert!(st.join("MOVED").exists());

    // once it stopped, the next start moves it
    running.set(false);
    let r2 = migrate(&d, &busy).unwrap();
    assert!(!r2.first && r2.copied.is_empty(), "{r2:?}");
    assert_eq!(r2.hubs_moved, vec!["busy-0000000b".to_string()]);
    assert_eq!(h.hub_dir("busy-0000000b"), bise.join("hubs/busy-0000000b"));
    assert_eq!(read(old_home.hub_dir("busy-0000000b").join("journal.jsonl")), "{\"t\":\"busy\"}\n");
    let m: serde_json::Value = serde_json::from_str(&read(bise.join(MIGRATED))).unwrap();
    assert_eq!(m["hubs_moved"].as_array().unwrap().len(), 2, "{m}");
    assert_eq!(m["hubs_waiting"], serde_json::json!([]));
    // nothing left to do: nothing written
    let before = read(bise.join(MIGRATED));
    assert_eq!(migrate(&d, &busy).unwrap(), migrate::Report::default());
    assert_eq!(read(bise.join(MIGRATED)), before);
}

#[test]
fn a_rollback_exports_the_new_paths_to_an_older_version() {
    // an older binary reads BEND_CONFIG & co. (BISE-160 exports them) and
    // computes the old hub path, which is a link after the move
    let d = old_layout("rollback");
    migrate(&d, &|_: &Path| false).unwrap();
    let ex: HashMap<_, _> = home_env(&d).exports().into_iter().collect();
    assert_eq!(ex["BEND_CONFIG"], d.join(".bise/config.toml").to_string_lossy());
    assert_eq!(ex["BEND_SESSIONS_DIR"], d.join(".bise/sessions").to_string_lossy());
    assert!(!ex.contains_key("BISE_HOME"));
    let old = d.join(".local/state/switchboard/idle-0000000a");
    assert_eq!(old.canonicalize().unwrap(), d.join(".bise/hubs/idle-0000000a").canonicalize().unwrap());
    // a process that inherited the pre-migration exports recomputes them
    let pre: HashMap<String, String> = {
        let mut m: HashMap<String, String> =
            home_env(&d).legacy().exports().into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        m.insert("HOME".into(), d.to_string_lossy().into());
        m
    };
    let after = Home::from_lookup(&|k: &str| pre.get(k).cloned());
    assert_eq!(after.config_file(), d.join(".bise/config.toml"));
}

#[test]
fn a_fresh_home_starts_in_dot_bise_and_explicit_homes_never_migrate() {
    let d = tmp("fresh");
    let r = migrate(&d, &|_: &Path| false).unwrap();
    assert!(r.first && r.copied.is_empty() && r.hubs_moved.is_empty(), "{r:?}");
    assert_eq!(home_env(&d).root(), d.join(".bise"));
    let e = |pairs: &'static [(&'static str, &'static str)]| {
        move |k: &str| pairs.iter().find(|(a, _)| *a == k).map(|(_, v)| v.to_string())
    };
    assert!(migrate::wanted(&e(&[("HOME", "/h")])));
    assert!(!migrate::wanted(&e(&[("HOME", "/h"), ("BISE_HOME", "/b")])));
    assert!(!migrate::wanted(&e(&[("HOME", "/h"), ("BISE_NO_MIGRATE", "1")])));
    assert!(!migrate::wanted(&e(&[])));
    assert!(migrate::is_hub_id("harness-3abb2bd8") && migrate::is_hub_id("a-b-00000000"));
    assert!(!migrate::is_hub_id("versions") && !migrate::is_hub_id("x-1234abcd.moved-2026") && !migrate::is_hub_id("-12345678"));
    assert!(!migrate::is_hub_id("x-1234ABCD"));
}

/// The guard (test_home): this binary runs on a temp HOME, the place
/// variables unset; a Home from the env never names the user's files.
#[test]
fn the_tests_run_on_a_temp_home() {
    assert!(crate::test_home::active(), "HOME = {:?}", std::env::var_os("HOME"));
    for k in [BISE_HOME, EXPORTS_FOR, "BEND_CONFIG", "SB_SOCKET", "XDG_STATE_HOME"] {
        assert_eq!(std::env::var_os(k), None, "{k}");
    }
    let h = Home::from_env();
    assert!(h.config_file().starts_with(std::env::temp_dir()), "{:?}", h.config_file());
    assert!(crate::test_home::is_place_var("BEND_SKILLS_INDEX") && !crate::test_home::is_place_var("PATH"));
}
