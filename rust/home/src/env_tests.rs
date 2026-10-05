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
            if ["SB_", "BEND_", "BISE_"].iter().any(|p| rest.starts_with(p)) {
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
    for child in [Child::Hub, Child::Core, Child::Repl] {
        let env = env_for(child, parent(), [("SB_AGENT", "t1")]);
        let get = |k: &str| env.get(k).map(|v| v.to_string_lossy().into_owned());
        for junk in ["SB_CORE_BIN", "SB_SOCKET", "SB_TASK", "BISE_ROLE", "BISE_SESSION_CHOICE", "BISE_APP_ROOT", "BEND_WORKDIR", "BEND_WIRE_LOG", "BISE_OWNERS", "BISE_HOME_WORKSPACE"] {
            assert_eq!(get(junk), None, "{child:?} inherited {junk}");
        }
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
