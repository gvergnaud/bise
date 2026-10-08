//! The store on a throwaway keychain (`security create-keychain` in a
//! temp folder, never the user's): macOS only.

use super::*;
use std::sync::OnceLock;

/// A temp folder of this test binary.
fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-secrets-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The throwaway keychain, made once, unlocked, never locking by
/// itself; every call of this binary goes to it (BISE_TEST_KEYCHAIN).
#[cfg(target_os = "macos")]
fn throwaway() -> &'static Path {
    static KC: OnceLock<PathBuf> = OnceLock::new();
    KC.get_or_init(|| {
        let d = tmp("kc");
        let kc = d.join("t.keychain-db");
        let run = |args: &[&str]| assert!(std::process::Command::new(security::PROGRAM).args(args).output().unwrap().status.success(), "{args:?}");
        let k = kc.to_str().unwrap();
        run(&["create-keychain", "-p", "pw", k]);
        run(&["set-keychain-settings", k]);
        run(&["unlock-keychain", "-p", "pw", k]);
        std::env::set_var(keychain::TEST_KEYCHAIN, &kc);
        kc
    })
}

fn forget_cache() {
    CACHE.lock().unwrap().clear();
}

fn writer(path: &Path, text: &str) -> impl Fn() -> std::io::Result<()> {
    let (p, t) = (path.to_path_buf(), text.to_string());
    move || std::fs::write(&p, &t)
}

#[test]
fn files_stay_files() {
    let d = tmp("files");
    let f = d.join("auth.json");
    assert!(matches!(read(&f), Ok(None)));
    assert_eq!(place(&f).unwrap(), Place::Missing);
    write_to(Store::File, &f, "{\"a\": 1}\n", &writer(&f, "{\"a\": 1}\n")).unwrap();
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "{\"a\": 1}\n", "the caller's own write, byte for byte");
    assert_eq!(read(&f).unwrap().as_deref(), Some("{\"a\": 1}\n"));
    assert_eq!(place(&f).unwrap(), Place::File);
    remove(&f).unwrap();
    assert!(!f.exists());
}

#[test]
fn a_test_home_never_reaches_the_users_keychain() {
    // test_home! gave this binary a temp HOME; BISE_TEST_KEYCHAIN aside
    // (other tests set it), here() refuses
    if bise_home::env::test_setting(keychain::TEST_KEYCHAIN).is_none() {
        assert!(Keychain::here().is_err());
    }
}

#[cfg(target_os = "macos")]
#[test]
fn the_keychain_holds_a_big_secret_and_the_file_none() {
    let kc = Keychain { file: Some(throwaway().to_path_buf()) };
    let d = tmp("kc-big");
    let f = d.join("auth.json");
    let secret = format!("{{\"chatgpt\": {{\"type\": \"oauth\", \"access\": \"{}\", \"email\": \"ana@exemple.fr é\"}}}}\n", "tok".repeat(2000));
    write_to(Store::Keychain, &f, &secret, &|| panic!("not a file")).unwrap();
    let on_disk = std::fs::read_to_string(&f).unwrap();
    assert!(!on_disk.contains("tok"), "no secret in the stub: {on_disk}");
    let Place::Keychain(stub) = place(&f).unwrap() else { panic!("a stub") };
    assert!(stub.parts >= 4, "{stub:?}");
    forget_cache();
    let t = std::time::Instant::now();
    assert_eq!(read(&f).unwrap().as_deref(), Some(secret.as_str()));
    eprintln!("read of {} parts from the keychain: {:?}", stub.parts, t.elapsed());
    // the cache: the same generation, no keychain call
    let t = std::time::Instant::now();
    assert_eq!(read(&f).unwrap().as_deref(), Some(secret.as_str()));
    assert!(t.elapsed() < Duration::from_millis(15), "{:?}", t.elapsed());
    // a smaller secret: the old parts beyond it are deleted
    write_to(Store::Keychain, &f, "{}\n", &|| panic!("not a file")).unwrap();
    assert_eq!(place(&f).unwrap(), Place::Keychain(Stub { gen: Stub::parse(&std::fs::read_to_string(&f).unwrap()).unwrap().gen, parts: 1 }));
    assert_eq!(kc.find(&security::account(&f, 1)).unwrap(), None);
    forget_cache();
    assert_eq!(read(&f).unwrap().as_deref(), Some("{}\n"));
    // back to a file: the items go
    write_to(Store::File, &f, "{}\n", &writer(&f, "{}\n")).unwrap();
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "{}\n");
    assert_eq!(kc.find(&security::account(&f, 0)).unwrap(), None);
}

#[cfg(target_os = "macos")]
#[test]
fn another_processs_write_is_seen_and_a_deleted_item_is_no_secret() {
    let kc = Keychain { file: Some(throwaway().to_path_buf()) };
    let d = tmp("kc-other");
    let f = d.join("x.json");
    write_to(Store::Keychain, &f, "one", &|| panic!()).unwrap();
    assert_eq!(read(&f).unwrap().as_deref(), Some("one"));
    // another process writes: a new generation in the stub
    forget_cache();
    write_to(Store::Keychain, &f, "two", &|| panic!()).unwrap();
    CACHE.lock().unwrap().insert(f.clone(), ("0000".into(), "one".into()));
    assert_eq!(read(&f).unwrap().as_deref(), Some("two"));
    // the item deleted by hand (Keychain Access): no secret, not an error
    kc.delete(&security::account(&f, 0));
    forget_cache();
    assert_eq!(read(&f).unwrap(), None);
    remove(&f).unwrap();
    assert!(!f.exists());
}

/// Law (architect, m_13198): only bise_secrets opens auth.json and the
/// MCP logins (their locks aside), so no reader forgets the keychain: a
/// line of a Rust source outside this crate that opens a file (or the next
/// line) never names one of them. Tests may (they make the fixtures).
#[test]
fn law_only_bise_secrets_opens_the_secrets() {
    fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if !["target", "vendor", "tests", "secrets"].contains(&n.as_str()) && !n.starts_with('.') {
                    sources(&p, out);
                }
            } else if n.ends_with(".rs") && !n.ends_with("tests.rs") && !n.ends_with("_tests.rs") {
                out.push(p);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = vec![];
    sources(root, &mut files);
    assert!(files.len() > 50, "the scan sees the workspace: {}", files.len());
    let opens = ["read_to_string(", "fs::read(", "File::open(", "fs::write(", "OpenOptions::new()"];
    let secret = ["auth_file", "auth.json\"", "file_of(", "mcp-oauth"];
    let mut bad = vec![];
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        // a file's own unit tests (after `#[cfg(test)]`) make fixtures
        let text = text.split("#[cfg(test)]").next().unwrap_or("");
        let lines: Vec<&str> = text.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            let near = format!("{l} {}", lines.get(i + 1).unwrap_or(&""));
            if opens.iter().any(|o| l.contains(o)) && secret.iter().any(|s| near.contains(s)) && !near.contains(".lock") && !near.contains("\"lock\"") {
                bad.push(format!("{}:{}: {}", f.strip_prefix(root).unwrap_or(f).display(), i + 1, l.trim()));
            }
        }
    }
    assert!(bad.is_empty(), "open these through bise_secrets::read / write:\n{}", bad.join("\n"));
}
