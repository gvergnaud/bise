use super::*;
use std::path::{Path, PathBuf};

/// The bise variable names a Rust source spells as a whole string literal
/// (`"SB_SOCKET"`): a read through `env::var`, a const, an `env(k)`
/// closure or a `.env(..)` all spell it so.
fn bise_names(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let rest = &src[i + 1..];
            if ["SB_", "BEND_", "BISE_", "PLUGIN_"].iter().any(|p| rest.starts_with(p)) {
                let n = rest.bytes().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == b'_').count();
                let name = &rest[..n];
                if rest[n..].starts_with('"') && !name.ends_with('_') {
                    out.push(name.to_string());
                    i += n + 2;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        if p.is_dir() {
            if name != "target" && name != "vendor" && !name.to_string_lossy().starts_with('.') {
                rust_sources(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("home/src/env_tests.rs") {
            out.push(p);
        }
    }
}

fn unregistered(src: &str) -> Vec<String> {
    bise_names(src).into_iter().filter(|n| kind(n).is_none()).collect()
}

#[test]
fn the_table_is_sorted_and_each_name_is_once() {
    for w in VARS.windows(2) {
        assert!(w[0].name < w[1].name, "{} then {}: keep VARS sorted, each name once", w[0].name, w[1].name);
    }
    for v in VARS {
        assert!(!v.what.is_empty(), "{}: say what it is", v.name);
    }
}

#[test]
fn a_read_of_an_unregistered_name_is_found() {
    let planted = "fn f() { let _ = std::env::var(\"BISE_NEW_THING\"); let _ = std::env::var(\"SB_SOCKET\"); }";
    assert_eq!(unregistered(planted), ["BISE_NEW_THING"]);
    assert_eq!(bise_names("const X: &str = \"BEND_WORKDIR\"; f(\"SB_\"); g(\"BISE_x\");"), ["BEND_WORKDIR"]);
}

#[test]
fn every_bise_name_in_the_rust_sources_is_registered() {
    let rust = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    rust_sources(&rust, &mut files);
    assert!(files.len() > 100, "found only {} sources under {}", files.len(), rust.display());
    let mut missing = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap_or_default();
        for n in unregistered(&src) {
            missing.push(format!("{} ({})", n, f.strip_prefix(&rust).unwrap_or(f).display()));
        }
    }
    missing.dedup();
    assert!(
        missing.is_empty(),
        "bise variables not in the registry: {}. Add each to VARS in rust/home/src/env.rs with its kind (user, internal or test) and one line of what it is",
        missing.join(", ")
    );
}

/// tests/bise_env.py's copy of the internal and test names: the tests
/// build a throwaway hub's environment with the same rule.
#[test]
fn the_python_copy_matches() {
    let py = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/bise_env.py");
    let src = std::fs::read_to_string(&py).unwrap();
    let list = |var: &str| -> Vec<String> {
        let start = src.find(&format!("{} = (", var)).unwrap_or_else(|| panic!("{var} in {}", py.display()));
        let body = &src[start..src[start..].find(')').map(|e| start + e).unwrap()];
        bise_names(body)
    };
    let of = |k: Kind| -> Vec<String> { VARS.iter().filter(|v| v.kind == k).map(|v| v.name.to_string()).collect() };
    assert_eq!(list("INTERNAL"), of(Kind::Internal), "tests/bise_env.py INTERNAL");
    assert_eq!(list("TEST"), of(Kind::Test), "tests/bise_env.py TEST");
}

/// The desktop app's copy (apps/desktop/src/main/env_vars.gen.ts): the
/// internal and test names, generated from [`VARS`] so the app's
/// `childEnv` (src/main/childenv.ts) drops what `env_for` drops. A stale
/// or hand-edited file is red; `BISE_PROTO_BLESS=1` rewrites it (one bless
/// for every generated TS file, like bise-proto's).
fn desktop_ts() -> String {
    let mut s = String::new();
    s.push_str("// generated from bise_home::env::VARS (rust/home/src/env_tests.rs), never hand-edited:\n");
    s.push_str("// BISE_PROTO_BLESS=1 cargo test -p bise-home --lib env::tests\n");
    for (k, what, kind) in [
        ("INTERNAL_VARS", "set by bise for one child: no child ever inherits one", Kind::Internal),
        ("TEST_VARS", "a knob of the tests: a child gets one only in test mode", Kind::Test),
    ] {
        s.push_str(&format!("\n/** {what} */\nexport const {k} = [\n"));
        for v in VARS.iter().filter(|v| v.kind == kind) {
            s.push_str(&format!("  '{}',\n", v.name));
        }
        s.push_str("] as const;\n");
    }
    s.push_str("\n/** a voice-fake test knob the core also gets in an off-screen run, never the terminal shells (Var::offscreen_core) */\nexport const OFFSCREEN_CORE_VARS = [\n");
    for v in VARS.iter().filter(|v| v.offscreen_core) {
        s.push_str(&format!("  '{}',\n", v.name));
    }
    s.push_str("] as const;\n");
    s
}

#[test]
fn the_desktop_copy_is_generated() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src/main/env_vars.gen.ts");
    let want = desktop_ts();
    if test_setting("BISE_PROTO_BLESS").is_some() {
        std::fs::write(&path, &want).unwrap();
        return;
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(have == want, "{} is stale: BISE_PROTO_BLESS=1 cargo test -p bise-home --lib env::tests", path.display());
}

/// A parent full of junk internal variables and the user's settings.
fn parent() -> Vec<(OsString, OsString)> {
    [
        ("PATH", "/usr/bin:/bin"),
        ("HOME", "/h"),
        ("LANG", "fr_FR.UTF-8"),
        ("HTTPS_PROXY", "http://proxy:3128"),
        ("MISTRAL_API_KEY", "k"),
        ("BISE_HOME", "/h/.bise-x"),
        ("BISE_ASCII", "1"),
        ("BEND_MODEL", "mistral-small-latest"),
        ("SB_CORE_BIN", "/nonexistent"),
        ("SB_AGENT", "x"),
        ("SB_SOCKET", "/nope"),
        ("SB_TASK", "x"),
        ("BISE_ROLE", "agent"),
        ("BISE_SESSION_CHOICE", "/junk/choice.toml"),
        ("BISE_APP_ROOT", "/nope"),
        ("BEND_WORKDIR", "/junk"),
        ("BEND_WIRE_LOG", "/junk/wire.log"),
        ("BISE_OWNERS", "junk.x.1"),
        ("BISE_EXPORTS_FOR", "/other\n\n/other/.bend-harness"),
        ("BISE_HOME_WORKSPACE", "/junk"),
        ("SB_STATE_DIR", "/test-state"),
        ("SB_EVERY_MIN_MS", "10"),
    ]
    .iter()
    .map(|(k, v)| (OsString::from(k), OsString::from(v)))
    .collect()
}

#[test]
fn no_child_inherits_an_internal_variable() {
    for child in [Child::Hub, Child::Core, Child::Repl, Child::Shell, Child::Script] {
        let env = env_for(child, parent(), [("SB_AGENT", "t1")]);
        let get = |k: &str| env.get(k).map(|v| v.to_string_lossy().into_owned());
        for junk in ["SB_CORE_BIN", "SB_SOCKET", "SB_TASK", "BISE_ROLE", "BISE_SESSION_CHOICE", "BISE_APP_ROOT", "BEND_WORKDIR", "BEND_WIRE_LOG", "BISE_OWNERS"] {
            assert_eq!(get(junk), None, "{child:?} inherited {junk}");
        }
        // a test's home workspace: a hub keeps it (its home check), no
        // other child (the hub sets it for its agents)
        let ws = (child == Child::Hub).then(|| "/junk".to_string());
        assert_eq!(get("BISE_HOME_WORKSPACE"), ws, "{child:?}");
        // what the parent set for it
        assert_eq!(get("SB_AGENT").as_deref(), Some("t1"), "{child:?}");
        // the user's own environment and settings
        for (k, v) in [("PATH", "/usr/bin:/bin"), ("HOME", "/h"), ("LANG", "fr_FR.UTF-8"), ("HTTPS_PROXY", "http://proxy:3128"), ("MISTRAL_API_KEY", "k"), ("BISE_HOME", "/h/.bise-x"), ("BISE_ASCII", "1"), ("BEND_MODEL", "mistral-small-latest")] {
            assert_eq!(get(k).as_deref(), Some(v), "{child:?} lost {k}");
        }
        // the paths of the parent's Home, with a stamp for this Home (not
        // the junk one inherited)
        assert_eq!(get("BEND_CONFIG").as_deref(), Some("/h/.bise-x/config.toml"), "{child:?}");
        assert_eq!(get("BISE_EXPORTS_FOR").as_deref(), Some("/h\n/h/.bise-x\n/h/.bise-x"), "{child:?}");
        // test settings: to a hub only
        let tests = get("SB_STATE_DIR").is_some() && get("SB_EVERY_MIN_MS").is_some();
        assert_eq!(tests, child == Child::Hub, "{child:?}: test settings");
    }
}

#[test]
fn a_set_value_wins_and_unset_removes() {
    let mut env = env_for(Child::Repl, parent(), [("BEND_MODEL", "other")]);
    assert_eq!(env.get("BEND_MODEL"), Some(OsStr::new("other")));
    env.unset("MISTRAL_API_KEY");
    assert_eq!(env.get("MISTRAL_API_KEY"), None);
}

/// hub-fifo: the test home keeps a test run's own inputs (a bench's
/// journal, the gate's sb-core) and still unsets every other place
/// variable; only test settings carry the flag.
#[test]
fn the_test_home_keeps_only_the_test_runs_inputs() {
    for n in ["SB_BENCH_JOURNAL", "SB_CORE_BIN", "SB_SEARCH_BENCH"] {
        assert!(kept_in_tests(n), "{n}");
    }
    for n in ["SB_STATE_DIR", "SB_SOCKET", "BISE_HOME", "SB_EVERY_MIN_MS", "NOT_A_BISE_VAR"] {
        assert!(!kept_in_tests(n), "{n}");
    }
    assert!(VARS.iter().filter(|v| v.kept_in_tests).all(|v| v.kind == Kind::Test));
}

/// architect m_22561: an off-screen desktop app hands its core exactly the
/// three voice-fake knobs outside test mode (childenv.ts reads the flag from
/// env_vars.gen.ts), all three test settings; the core's voice_of stays the
/// guard against the real mic.
#[test]
fn only_the_voice_fake_knobs_reach_an_off_screen_core() {
    let flagged: Vec<&str> = VARS.iter().filter(|v| v.offscreen_core).map(|v| v.name).collect();
    assert_eq!(flagged, ["BISE_AMBIENT_FAKE_VOICE", "BISE_VOICE_FAKE_HEARD", "BISE_VOICE_FAKE_HEARD_MS"]);
    assert!(VARS.iter().filter(|v| v.offscreen_core).all(|v| v.kind == Kind::Test && !v.kept_in_tests));
}
