//! What an upload may send (files.rs), on a temp home.

use super::*;
use crate::proto::code;
use serde_json::json;

/// A temp home with bise's home in it; removed on drop.
struct Home {
    root: PathBuf,
    homes: Homes,
}

impl Home {
    fn new(tag: &str) -> Home {
        let root = std::env::temp_dir().join(format!("cu-files-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("h");
        std::fs::create_dir_all(&home).unwrap();
        Home {
            homes: Homes {
                bise: home.join(".bise"),
                home,
            },
            root,
        }
    }
    /// A file of `len` bytes at `rel` under the home.
    fn file(&self, rel: &str, len: u64) -> String {
        let p = self.homes.home.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let f = std::fs::File::create(&p).unwrap();
        f.set_len(len).unwrap();
        p.to_string_lossy().into_owned()
    }
    fn check(&self, files: Value) -> Result<Vec<File>, Value> {
        check(&files, &self.homes)
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn code_of(r: Result<Vec<File>, Value>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(e) => code(&e).to_string(),
    }
}

/// Law (architect m_52652): every path the agents' sandbox denies
/// (`Secrets::of`, the one list) is refused as an upload, a key named
/// oddly under ~/.ssh included, and through a link from Documents.
#[test]
fn law_an_upload_refuses_every_secret_the_sandbox_denies() {
    let h = Home::new("law");
    let real_home = std::fs::canonicalize(&h.homes.home).unwrap();
    let s = Secrets::of(&real_home.join(".bise"), &real_home);
    let mut paths: Vec<PathBuf> = s.files.clone();
    paths.extend(s.folders.iter().map(|d| d.join("mcp-oauth/x.json")));
    for k in ["id_ed25519", "work_key", "keys/deploy", "config"] {
        paths.push(s.ssh.join(k));
    }
    for p in &paths {
        let rel = p
            .strip_prefix(&real_home)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let written = h.file(&rel, 10);
        let r = h.check(json!([written]));
        assert_eq!(
            code_of(r.clone()),
            "refused",
            "{} went through: {:?}",
            rel,
            r
        );
        // through a link from Documents too: judged where it leads
        let link = h
            .homes
            .home
            .join("Documents")
            .join(format!("link-{}", rel.replace('/', "_")));
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&written, &link).unwrap();
        assert_eq!(
            code_of(h.check(json!([link.to_string_lossy()]))),
            "refused",
            "a link to {}",
            rel
        );
    }
    // the sandbox's words say why
    let key = h
        .homes
        .home
        .join(".ssh/work_key")
        .to_string_lossy()
        .into_owned();
    let e = h.check(json!([key])).unwrap_err();
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("an ssh private key: agents can't send it"),
        "{}",
        e
    );
}

#[test]
fn documents_go_with_their_real_path_name_and_size() {
    let h = Home::new("ok");
    let a = h.file("Documents/CAFPI dossier/01 identité.pdf", 1234);
    let b = h.file(
        "Library/Mobile Documents/com~apple~CloudDocs/bulletin.pdf",
        10,
    );
    let c = h.file("Library/CloudStorage/Dropbox/avis.pdf", 10);
    let got = h.check(json!([a, b, c])).unwrap();
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].name, "01 identité.pdf");
    assert_eq!(got[0].size, 1234);
    assert_eq!(got[0].path, std::fs::canonicalize(&a).unwrap());
    // one string is one file
    assert_eq!(h.check(json!(a)).unwrap().len(), 1);
}

#[test]
fn the_upload_only_refusals() {
    let h = Home::new("extra");
    for rel in [
        ".ssh/config",
        ".ssh/id_rsa.pub",
        ".aws/credentials",
        ".config/gh/hosts.yml",
        ".gnupg/x",
        ".bise/hubs/x/journal.jsonl",
        "Library/Keychains/login.keychain-db",
        "Library/Cookies/Cookies.binarycookies",
        "Library/Application Support/x.db",
        "Documents/backup/id_rsa",
        "Documents/old.keychain",
        "Documents/project/.env",
    ] {
        let p = h.file(rel, 1);
        assert_eq!(code_of(h.check(json!([p]))), "refused", "{}", rel);
    }
    // `..` can't climb out of an allowed folder into a refused one
    h.file(".aws/credentials", 1);
    std::fs::create_dir_all(h.homes.home.join("Documents")).unwrap();
    let climb = format!("{}/Documents/../.aws/credentials", h.homes.home.display());
    assert_eq!(code_of(h.check(json!([climb]))), "refused");
    // system folders, by their real path
    for p in ["/etc/hosts", "/private/etc/hosts"] {
        if Path::new(p).exists() {
            assert_eq!(code_of(h.check(json!([p]))), "refused", "{}", p);
        }
    }
}

#[test]
fn wrong_paths_sizes_and_counts() {
    let h = Home::new("bad");
    let ok = h.file("Documents/a.pdf", 1);
    assert_eq!(
        code_of(h.check(json!(["Documents/a.pdf"]))),
        "bad_args",
        "relative"
    );
    assert_eq!(
        code_of(h.check(json!(["~/Documents/a.pdf"]))),
        "bad_args",
        "~ isn't expanded"
    );
    assert_eq!(
        code_of(h.check(json!([format!(
            "{}/Documents/none.pdf",
            h.homes.home.display()
        )]))),
        "file_not_found"
    );
    assert_eq!(
        code_of(h.check(json!([h.homes.home.join("Documents").to_string_lossy()]))),
        "bad_args",
        "a folder"
    );
    assert_eq!(code_of(h.check(json!([]))), "bad_args");
    assert_eq!(code_of(h.check(json!({"path": ok}))), "bad_args");
    assert_eq!(code_of(h.check(json!(vec![ok.clone(); MAX_FILES]))), "ok");
    assert_eq!(
        code_of(h.check(json!(vec![ok.clone(); MAX_FILES + 1]))),
        "bad_args"
    );
    let big = h.file("Documents/big.pdf", MAX_BYTES + 1);
    let e = h.check(json!([big])).unwrap_err();
    assert_eq!(code(&e), "file_too_large");
    assert_eq!(
        code_of(h.check(json!([h.file("Documents/edge.pdf", MAX_BYTES)]))),
        "ok"
    );
}
