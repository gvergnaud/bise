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
fn the_default_home_is_dot_bise() {
    let h = home_of(&[("HOME", "/h"), ("XDG_STATE_HOME", "/xdg")]);
    let p = |s: &str| PathBuf::from(s);
    assert_eq!(h.root(), p("/h/.bise"));
    assert_eq!(h.config_file(), p("/h/.bise/config.toml"));
    assert_eq!(h.key_file(), p("/h/.bise/.env"));
    assert_eq!(h.env_files(), vec![p("/h/.bise/.env"), p("/h/.vibe/.env")]);
    assert_eq!(h.sessions_dir(), p("/h/.bise/sessions"));
    // XDG_STATE_HOME is not read
    assert_eq!(h.hub_dir("ws-12345678"), p("/h/.bise/hubs/ws-12345678"));
    assert_eq!(h.worktrees_dir(), p("/h/.bise/worktrees"));
    assert_eq!(h.drafts_dir(), p("/h/.bise/drafts"));
    assert_eq!(h.versions_dir(), p("/h/.bise/dev/versions"));
    assert_eq!(h.build_dir(), p("/h/.bise/dev/build"));
    assert_eq!(h.pref(Pref::Onboarded), Slot::key("/h/.bise/prefs.json", "onboarded"));
    assert_eq!(h.projects_file(), p("/h/.bise/projects.json"));
    assert_eq!(h.library_folders(), p("/h/.bise/library/folders.json"));
}

#[test]
fn bise_home_moves_everything() {
    let h = home_of(&[("HOME", "/h"), ("BISE_HOME", "/b")]);
    let p = |s: &str| PathBuf::from(s);
    assert_eq!(h.root(), p("/b"));
    assert_eq!(h.user_home(), p("/h"));
    assert_eq!(h.config_file(), p("/b/config.toml"));
    assert_eq!(h.auth_file(), p("/b/auth.json"));
    assert_eq!(h.secrets_dir(), p("/b/secrets"));
    assert_eq!(h.env_files(), vec![p("/b/.env"), p("/h/.vibe/.env")]);
    assert_eq!(h.sessions_dir(), p("/b/sessions"));
    assert_eq!(h.hub_dir("ws-1"), p("/b/hubs/ws-1"));
    assert_eq!(h.worktrees_dir(), p("/b/worktrees"));
    assert_eq!(h.marketplaces(), p("/b/marketplaces.json"));
    assert_eq!(h.marketplace_cache(), p("/b/cache/marketplaces"));
    assert_eq!(h.mcp_tools_cache(), p("/b/cache/mcp-tools"));
    assert_eq!(h.mcp_tools_salt(), p("/b/cache/mcp-tools.salt"));
    assert_eq!(h.mcp_secrets_dir(), p("/b/secrets/mcp-env"));
    assert_eq!(h.run_dir(), p("/b/run"));
    assert_eq!(h.drafts_dir(), p("/b/drafts"));
    assert_eq!(h.versions_dir(), p("/b/dev/versions"));
    assert_eq!(h.build_dir(), p("/b/dev/build"));
    for pref in [Pref::Voice, Pref::Theme, Pref::Hints, Pref::Tip, Pref::Onboarded, Pref::Setup, Pref::ExcludedApps] {
        assert_eq!(h.pref(pref), Slot::key("/b/prefs.json", pref.key()));
    }
}

#[test]
fn no_home_falls_back_to_tmp() {
    let h = home_of(&[]);
    assert_eq!(h.config_file(), PathBuf::from("/tmp/.bise/config.toml"));
    assert_eq!(home_of(&[("HOME", "")]).root(), Path::new("/tmp/.bise"));
}

#[test]
fn overrides_win_and_are_exported() {
    let h = home_of(&[("HOME", "/h"), ("BEND_CONFIG", "/c.toml"), ("SB_VERSIONS_DIR", "/v")]);
    assert_eq!(h.config_file(), PathBuf::from("/c.toml"));
    assert_eq!(h.versions_dir(), PathBuf::from("/v"));
    let ex: HashMap<_, _> = h.exports().into_iter().collect();
    assert_eq!(ex["BEND_CONFIG"], "/c.toml");
    assert_eq!(ex["BEND_SESSIONS_DIR"], "/h/.bise/sessions");
    for k in PATH_VARS {
        assert!(ex.contains_key(k), "{k} not exported");
    }
    assert!(!ex.contains_key("BISE_HOME"), "~/.bise is the default: no BISE_HOME");
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
    assert_eq!(test.sessions_dir(), PathBuf::from("/t/.bise/sessions"));
    assert_eq!(test.config_file(), PathBuf::from("/t/.bise/config.toml"));
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
fn another_homes_default_is_not_an_override() {
    // an agent's shell (HOME=/u, bise layout, exports stamped for /u) ran a
    // test with a temp HOME and BISE_EXPORTS_FOR unset: the test's REPL took
    // BEND_SKILLS_INDEX=/u/.bise/cache/skills-index.txt (a shared index,
    // gone since) as an override and wrote an empty index over the user's
    // one: the skill tool found no skill in main's session. The same rule
    // holds for every path it exports
    let t = tmp("foreign-default");
    let ts = t.to_string_lossy().to_string();
    let inherited = [
        ("BEND_CONFIG", "/u/.bise/config.toml"),
        ("BEND_SESSIONS_DIR", "/u/.bise/sessions"),
        ("BEND_IMAGE_DIR", "/u/.bise/images"),
        ("BEND_PLUGINS_STATE", "/u/.bise/plugins.json"),
        ("BEND_PLUGINS_DATA", "/u/.bise/plugin-data"),
        ("BEND_RUN_DIR", "/u/.bise/run"),
        ("SB_VERSIONS_DIR", "/u/.bise/dev/versions"),
        ("SB_BUILD_DIR", "/u/.bise/dev/build"),
    ];
    let mut pairs = vec![("HOME", ts.as_str())];
    pairs.extend(inherited);
    let h = home_of(&pairs);
    assert_eq!(h, home_of(&[("HOME", &ts)]), "every inherited path was /u's default");
    assert_eq!(h.plugins_state(), t.join(".bise/plugins.json"));
    // every exported variable is covered by the rule
    assert_eq!(inherited.len(), PATH_VARS.len());
    // this HOME's own defaults and any other path stay overrides
    let own = t.join(".bise/plugins.json").to_string_lossy().to_string();
    let kept = home_of(&[("HOME", &ts), ("BEND_PLUGINS_STATE", &own), ("BEND_CONFIG", "/u/cfg/config.toml")]);
    assert_eq!(kept.plugins_state(), PathBuf::from(&own));
    assert_eq!(kept.config_file(), PathBuf::from("/u/cfg/config.toml"));
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
    assert!(crate::test_home::is_place_var("BEND_RUN_DIR") && !crate::test_home::is_place_var("PATH"));
}

#[test]
fn excluded_apps_start_from_the_seed_and_then_are_his() {
    let d = tmp("excluded");
    let h = Home::at(&d);
    let slot = h.pref(Pref::ExcludedApps);
    assert_eq!(slot, Slot::key(h.prefs_file(), "excluded_apps"));
    let seed = prefs::excluded_apps(slot.get());
    assert!(seed.contains(&"com.1password.1password".to_string()) && seed.contains(&"com.apple.MobileSMS".to_string()));
    slot.set(serde_json::json!(["com.apple.MobileSMS", "com.example.bank"])).unwrap();
    assert_eq!(prefs::excluded_apps(slot.get()), vec!["com.apple.MobileSMS", "com.example.bank"]);
    slot.set(serde_json::json!([])).unwrap();
    assert!(prefs::excluded_apps(slot.get()).is_empty(), "he cleared it: nothing excluded, not the seed again");
}

#[test]
fn the_window_marks_a_project_seen_by_its_hub_id_in_ms() {
    use serde_json::json;
    // the desktop's app/seen.ts markSeen (proj-unread's dot; his m_48087: it never went away, the
    // core refused every mark): `seen.<hub id>` = ms, the key bise_proto::rows::seen_key builds
    let o = json!({"theme": "dark"});
    let v = prefs::set_dotted(&o, "seen.ai-docs-1a2b3c4d", json!(1_793_134_500_000u64)).unwrap();
    assert_eq!(v["seen"], json!({"ai-docs-1a2b3c4d": 1_793_134_500_000u64}));
    assert_eq!(prefs::set_dotted(&v, "seen.ai-docs-1a2b3c4d", json!(1_793_134_600_000u64)).unwrap()["seen"]["ai-docs-1a2b3c4d"], json!(1_793_134_600_000u64), "a later look replaces it");
    assert_eq!(prefs::set_dotted(&v, "seen.ai-docs-1a2b3c4d", Value::Null).unwrap()["seen"], json!({}), "null takes it out");
    assert!(prefs::set_dotted(&o, "seen", json!({})).is_err(), "never seen whole");
    assert!(prefs::set_dotted(&o, "seen.not a hub id", json!(1)).is_err(), "a hub id");
    assert!(prefs::set_dotted(&o, "seen.ai-docs-1a2b3c4d", json!("now")).is_err(), "ms");
    assert!(prefs::set_dotted(&o, "seen.ai-docs-1a2b3c4d.x", json!(1)).is_err(), "nothing under it");
}

#[test]
fn the_window_pins_each_kind_by_its_hub_id() {
    use serde_json::json;
    // the desktop's app/pins.ts pinThing (his m_50763: 'couldn't pin', the core refused every pin):
    // `<root>.<hub id>` = the pinned ids in pin order, one root per kind
    let o = json!({"theme": "dark"});
    for (root, id) in [("pins", "main"), ("pins_art", "visite-webgl"), ("pins_sched", "watch:12"), ("pins_proc", "perf/bg3.1791100100")] {
        let key = format!("{root}.ai-docs-1a2b3c4d");
        let v = prefs::set_dotted(&o, &key, json!([id])).unwrap();
        assert_eq!(v[root], json!({"ai-docs-1a2b3c4d": [id]}), "{root}");
        assert_eq!(prefs::set_dotted(&v, &key, json!([])).unwrap()[root], json!({"ai-docs-1a2b3c4d": []}), "{root}: unpinned the last");
        assert_eq!(prefs::set_dotted(&v, &key, Value::Null).unwrap()[root], json!({}), "{root}: null takes it out");
        assert!(prefs::set_dotted(&o, root, json!({})).is_err(), "{root}: never whole");
        assert!(prefs::set_dotted(&o, &format!("{root}.not a hub id"), json!([id])).is_err(), "{root}: a hub id");
        assert!(prefs::set_dotted(&o, &key, json!(id)).is_err(), "{root}: a list");
        assert!(prefs::set_dotted(&o, &key, json!([1])).is_err(), "{root}: of strings");
        assert!(prefs::set_dotted(&o, &key, json!([""])).is_err(), "{root}: not empty");
        assert!(prefs::set_dotted(&o, &format!("{key}.x"), json!([id])).is_err(), "{root}: nothing under it");
    }
}

#[test]
fn the_window_sets_known_prefs_by_dotted_key_and_keeps_their_types() {
    use serde_json::json;
    let o = json!({"theme": "dark", "quiet": {"call": true, "batch_min": 20}});
    let v = prefs::set_dotted(&o, "quiet.call", json!(false)).unwrap();
    assert_eq!(v, json!({"theme": "dark", "quiet": {"call": false, "batch_min": 20}}));
    assert_eq!(prefs::set_dotted(&o, "context.url", json!(true)).unwrap()["context"], json!({"url": true}));
    assert_eq!(prefs::set_dotted(&json!({"voice": "old"}), "voice.fn_hold", json!(true)).unwrap()["voice"], json!({"fn_hold": true}));
    assert_eq!(prefs::set_dotted(&o, "quiet.call", Value::Null).unwrap()["quiet"], json!({"batch_min": 20}), "null takes it out");
    assert!(prefs::set_dotted(&o, "hints", json!({})).is_err(), "never hints whole");
    // the desktop app's update channel: stable or beta, nothing else
    assert_eq!(prefs::set_dotted(&o, "desktop_channel", json!("beta")).unwrap()["desktop_channel"], json!("beta"));
    assert!(prefs::set_dotted(&o, "desktop_channel", json!("nightly")).is_err());
    assert!(prefs::set_dotted(&o, "desktop_channel.x", json!(true)).is_err());
    // his own bim's look (page bim-pick): set whole, its four fields in their shape, nothing else
    let look = json!({"skin": 3, "hair": "pigtails", "color": 1, "wear": "none"});
    assert_eq!(prefs::set_dotted(&o, "bim", look.clone()).unwrap()["bim"], look);
    assert!(prefs::set_dotted(&o, "bim", json!({"skin": 3, "hair": "Pigtails!", "color": 1, "wear": "none"})).is_err(), "hair a slug");
    assert!(prefs::set_dotted(&o, "bim", json!({"skin": 3, "hair": "short", "color": 1})).is_err(), "all four fields");
    assert!(prefs::set_dotted(&o, "bim", json!({"skin": 99, "hair": "short", "color": 1, "wear": "cap"})).is_err(), "a small index");
    assert!(prefs::set_dotted(&o, "bim.skin", json!(2)).is_err(), "never a part");
    // the accessory's colour (bim-hat-colour): a fifth small index; a look without it still sets
    let hat = json!({"skin": 3, "hair": "short", "color": 1, "wear": "cap", "hat": 4});
    assert_eq!(prefs::set_dotted(&o, "bim", hat.clone()).unwrap()["bim"], hat);
    assert!(prefs::set_dotted(&o, "bim", json!({"skin": 3, "hair": "short", "color": 1, "wear": "cap", "hat": 99})).is_err(), "hat a small index");
    assert!(prefs::set_dotted(&o, "bim", json!({"skin": 3, "hair": "short", "color": 1, "wear": "cap", "hat": "red"})).is_err(), "hat an index");
    assert!(prefs::set_dotted(&o, "bim", json!({"skin": 3, "hair": "short", "color": 1, "wear": "cap", "tie": 2})).is_err(), "nothing else");
    // the office's map per project (the desktop's /bims): bims.map.<hub id> = a slug, nothing else
    let m = prefs::set_dotted(&o, "bims.map.harness-3abb2bd8", json!("kyoto")).unwrap();
    assert_eq!(m["bims"], json!({"map": {"harness-3abb2bd8": "kyoto"}}));
    assert_eq!(prefs::set_dotted(&m, "bims.map.harness-3abb2bd8", Value::Null).unwrap()["bims"], json!({"map": {}}), "null: back to the loft");
    for (key, v) in [
        ("bims", json!({})),
        ("bims.map", json!({})),
        ("bims.other.harness-3abb2bd8", json!("kyoto")),
        ("bims.map.harness", json!("kyoto")),
        ("bims.map.harness-3abb2bd8.x", json!("kyoto")),
        ("bims.map.harness-3abb2bd8", json!("Kyoto")),
        ("bims.map.harness-3abb2bd8", json!("a-very-long-map-name-past-32-chars")),
        ("bims.map.harness-3abb2bd8", json!(3)),
    ] {
        assert!(prefs::set_dotted(&o, key, v.clone()).is_err(), "{key} = {v}");
    }
    // S36: a one-time hint marked seen, the TUI's key in the same object
    let h = prefs::set_dotted(&json!({"hints": {"first_agent": true}}), "hints.first_card", json!(true)).unwrap();
    assert_eq!(h["hints"], json!({"first_agent": true, "first_card": true}));
    for name in prefs::WINDOW_HINTS {
        assert!(prefs::set_dotted(&o, &format!("hints.{name}"), json!(true)).is_ok(), "{name}");
    }
    assert!(prefs::set_dotted(&o, "hints.first_card", json!(false)).is_err(), "never unseen by the window");
    assert!(prefs::set_dotted(&o, "hints.first_card", Value::Null).is_err(), "never taken out");
    assert!(prefs::set_dotted(&o, "hints.first_card", json!("yes")).is_err());
    // the desktop's own: the capsule's permissions sheet, shown once (only true, never unseen)
    let f = prefs::set_dotted(&o, "hints.fn_perms", json!(true)).unwrap();
    assert_eq!(f["hints"]["fn_perms"], json!(true));
    assert!(prefs::set_dotted(&o, "hints.fn_perms", json!(false)).is_err(), "fn_perms only takes true");
    assert!(prefs::set_dotted(&o, "hints.fn_perms", Value::Null).is_err());
    assert!(prefs::set_dotted(&o, "hints.other", json!(true)).is_err(), "only the TUI's hints");
    assert!(prefs::set_dotted(&o, "hints.first_card.x", json!(true)).is_err());
    assert!(prefs::set_dotted(&o, "setup.asked", json!(true)).is_err());
    assert!(prefs::set_dotted(&o, "quiet..call", json!(true)).is_err());
    assert!(prefs::set_dotted(&o, "shots_keep_days", json!("7")).is_err(), "a number stays a number");
    assert!(prefs::set_dotted(&o, "excluded_apps", json!(["a", 1])).is_err());
    assert!(prefs::set_dotted(&o, "onboarded", json!("yes")).is_err());
    assert!(prefs::set_dotted(&o, "theme", json!("purple")).is_err());
    assert!(prefs::set_dotted(&o, "shots_keep_days.x", json!(1)).is_err());
    assert_eq!(prefs::set_dotted(&o, "shots_keep_days", json!(14)).unwrap()["shots_keep_days"], json!(14));
    // the image store's retention: his number, else what settings shows
    assert_eq!(prefs::keep_days(Some(&json!({"shots_keep_days": 30}))), 30);
    assert_eq!(prefs::keep_days(Some(&json!({}))), prefs::KEEP_DAYS_DEFAULT);
    assert_eq!(prefs::keep_days(None), 7);
    assert_eq!(prefs::keep_days(Some(&json!({"shots_keep_days": 0}))), 7, "0 is not 'delete at once'");
    assert_eq!(prefs::keep_days(Some(&json!({"shots_keep_days": "30"}))), 7);
    let w = prefs::for_window(Some(json!({"theme": "dark"})));
    assert_eq!(w["theme"], json!("dark"));
    assert!(w["excluded_apps"].as_array().is_some_and(|a| a.contains(&json!("com.apple.MobileSMS"))), "the seed, resolved");
    assert_eq!(prefs::for_window(Some(json!({"excluded_apps": []})))["excluded_apps"], json!([]));
    assert_eq!(prefs::for_window(None)["onboarded"], json!(false), "a fresh home: not onboarded");
    assert_eq!(prefs::for_window(Some(json!({"onboarded": true})))["onboarded"], json!(true));
    // the seed's names ride along, one table for the window; never a key it sets
    assert_eq!(w["app_names"]["com.apple.MobileSMS"], json!("Messages"));
    assert_eq!(w["app_names"].as_object().map(|m| m.len()), Some(prefs::EXCLUDED_APPS_SEED.len()));
    assert!(prefs::set_dotted(&o, "app_names", json!({})).is_err());
}

#[test]
fn an_executable_in_a_version_bundle_finds_the_version_dir() {
    let d = tmp("exe-home");
    let macos = d.join("v1/bise.app/Contents/MacOS");
    std::fs::create_dir_all(&macos).unwrap();
    assert_eq!(exe_home_dir(&macos), macos, "no VERSION above the bundle: the folder itself");
    std::fs::write(d.join("v1/VERSION"), "id=v1\n").unwrap();
    assert_eq!(exe_home_dir(&macos), d.join("v1"), "<root>/bise.app/Contents/MacOS -> <root>");
    assert_eq!(exe_home_dir(&d.join("v1")), d.join("v1"), "a plain version dir: itself");
    let other = d.join("v1/bise.app/Contents/Resources");
    assert_eq!(exe_home_dir(&other), other, "only Contents/MacOS");
    let _ = std::fs::remove_dir_all(&d);
}
