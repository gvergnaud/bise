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
    throwaway();
    let kc = Keychains::here().unwrap().bise;
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
    assert_eq!(place(&f).unwrap(), Place::Keychain(Stub { gen: Stub::parse(&std::fs::read_to_string(&f).unwrap()).unwrap().gen, parts: 1, at: At::Bise }));
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
    throwaway();
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
    Keychains::here().unwrap().bise.delete(&security::account(&f, 0));
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

/// Law (main m_13693): no test can make macOS show a keychain dialog on
/// his screen. A test keychain is made unlocked and never locks by itself
/// (`set-keychain-settings` with no -l/-t/-u: no sleep lock, no timeout),
/// and no test locks one: the locked cases use BISE_TEST_KEYCHAIN_LOCKED
/// (security never runs). Scans every Rust and Python source of the repo.
#[test]
fn law_no_test_can_prompt_for_a_keychain() {
    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if !["target", "vendor", "node_modules"].contains(&n.as_str()) && !n.starts_with('.') {
                    files(&p, out);
                }
            } else if n.ends_with(".rs") || n.ends_with(".py") || n.ends_with(".sh") {
                out.push(p);
            }
        }
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut all = vec![];
    files(&repo.join("rust"), &mut all);
    files(&repo.join("tests"), &mut all);
    assert!(all.len() > 100, "the scan sees the repo: {}", all.len());
    let me = Path::new(file!()).file_name().unwrap();
    let mut bad = vec![];
    for f in &all {
        if f.file_name() == Some(me) && f.to_string_lossy().contains("secrets/src") {
            continue;
        }
        let text = std::fs::read_to_string(f).unwrap_or_default();
        for (i, l) in text.lines().enumerate() {
            let locks = l.replace("unlock-keychain", "").contains("lock-keychain");
            let settings = l.contains("set-keychain-settings") && ["\"-l\"", "\"-t\"", "\"-u\"", " -l ", " -t ", " -u "].iter().any(|x| l.contains(x));
            if locks || settings {
                bad.push(format!("{}:{}: {}", f.display(), i + 1, l.trim()));
            }
        }
    }
    assert!(bad.is_empty(), "a test that locks a keychain makes macOS prompt on his screen: use BISE_TEST_KEYCHAIN_LOCKED\n{}", bad.join("\n"));
}

/// The simulated lock answers "locked" without running security.
#[test]
fn a_simulated_lock_never_runs_security() {
    let kc = Keychain { file: Some(PathBuf::from("/nonexistent/never.keychain-db")), locked: true };
    assert!(matches!(kc.find("x"), Err(ReadError::Locked)));
    assert!(matches!(kc.run(&["add-generic-password".into()]), Err(ReadError::Locked)));
}

/// Step B (issue 19): the items go to bise's own keychain file in
/// `secrets/`, made on the first write with its password in the login
/// keychain (here the throwaway); the throwaway gets no item of the
/// secret itself, and the file is unlocked (probed with no window).
#[cfg(target_os = "macos")]
#[test]
fn the_items_go_to_bises_own_keychain_file() {
    let login = throwaway();
    let kcs = Keychains::here().unwrap();
    let file = kcs.bise.file().unwrap().to_path_buf();
    assert!(file.ends_with("secrets/bise.keychain-db"), "{}", file.display());
    let d = tmp("kc-own");
    let f = d.join("auth.json");
    write_to(Store::Keychain, &f, "mine", &|| panic!()).unwrap();
    assert!(file.exists());
    assert_eq!(status::status(Some(&file)), status::Status::Unlocked);
    assert_eq!(status::status(Some(login)), status::Status::Unlocked);
    assert_eq!(status::status(Some(&d.join("none.keychain-db"))), status::Status::Missing);
    let Place::Keychain(stub) = place(&f).unwrap() else { panic!("a stub") };
    assert_eq!(stub.at, At::Bise);
    assert!(kcs.bise.find(&security::account(&f, 0)).unwrap().is_some());
    assert_eq!(kcs.login.find(&security::account(&f, 0)).unwrap(), None, "nothing of the secret in the login keychain");
    assert!(kcs.login.find(keychain::PASSWORD_ACCOUNT).unwrap().is_some_and(|p| p.len() == 64));
    forget_cache();
    assert_eq!(read(&f).unwrap().as_deref(), Some("mine"));
    remove(&f).unwrap();
}

/// Under the agents' sandbox (a read deny on bise's keychain file, as
/// step A's profile has on `secrets/`), `security` can't read its items;
/// outside, it can.
#[cfg(target_os = "macos")]
#[test]
fn the_sandbox_cant_read_bises_keychain() {
    throwaway();
    let kcs = Keychains::here().unwrap();
    let d = tmp("kc-sbx");
    let f = d.join("x.json");
    write_to(Store::Keychain, &f, "hidden-1234", &|| panic!()).unwrap();
    let file = std::fs::canonicalize(kcs.bise.file().unwrap()).unwrap();
    let applies = std::process::Command::new("/usr/bin/sandbox-exec").args(["-p", "(version 1)(allow default)", "/usr/bin/true"]).status().is_ok_and(|s| s.success());
    if !applies {
        eprintln!("in a sandbox already: skipped");
        return;
    }
    let deny = format!("(version 1)(allow default)(deny file-read-data (literal \"{}\"))", file.display());
    let acct = security::account(&f, 0);
    let args = ["find-generic-password", "-s", security::SERVICE, "-a", &acct, "-w", file.to_str().unwrap()];
    let inside = std::process::Command::new("/usr/bin/sandbox-exec").arg("-p").arg(&deny).arg(security::PROGRAM).args(args).output().unwrap();
    assert!(!inside.status.success() && !String::from_utf8_lossy(&inside.stdout).contains("hidden"), "{inside:?}");
    let outside = std::process::Command::new(security::PROGRAM).args(args).output().unwrap();
    assert!(outside.status.success(), "{outside:?}");
    // and the no-window probe says it can't be read there
    let probe = std::process::Command::new("/usr/bin/sandbox-exec").arg("-p").arg(&deny).arg("/bin/test").arg("-r").arg(&file).status().unwrap();
    assert!(!probe.success());
    remove(&f).unwrap();
}

/// A stub of v2026.10.2-28 (its items in the login keychain) is still
/// read there, and its next write, or a move (`on` again), takes it to
/// bise's keychain and deletes the login items (architect m_13735 Q2).
#[cfg(target_os = "macos")]
#[test]
fn a_stub_of_before_is_read_then_moved_on_write() {
    throwaway();
    let kcs = Keychains::here().unwrap();
    let d = tmp("kc-old");
    for (name, by_move) in [("a.json", false), ("b.json", true)] {
        let f = d.join(name);
        // as -28 wrote it: items in the login keychain, the old head
        to_keychain(&kcs.login, &f, "old", 0).unwrap();
        let s = Stub::parse(&std::fs::read_to_string(&f).unwrap()).unwrap();
        std::fs::write(&f, Stub { at: At::Login, ..s }.to_text()).unwrap();
        assert!(std::fs::read_to_string(&f).unwrap().starts_with("bise-secret keychain gen="));
        forget_cache();
        assert_eq!(read(&f).unwrap().as_deref(), Some("old"));
        if by_move {
            assert!(move_to(Store::Keychain, &f).unwrap());
        } else {
            write_to(Store::Keychain, &f, "new", &|| panic!()).unwrap();
        }
        let Place::Keychain(now) = place(&f).unwrap() else { panic!("a stub") };
        assert_eq!(now.at, At::Bise);
        assert_eq!(kcs.login.find(&security::account(&f, 0)).unwrap(), None, "the login item went");
        forget_cache();
        assert_eq!(read(&f).unwrap().as_deref(), Some(if by_move { "old" } else { "new" }));
        // moved already: a second move does nothing
        assert!(!move_to(Store::Keychain, &f).unwrap());
        remove(&f).unwrap();
    }
}
