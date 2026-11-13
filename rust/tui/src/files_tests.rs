use super::*;

/// Every folder `list_dir` read (the protected-folder tests).
pub(super) static LISTED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// An index with no mtimes (all as old): the order falls to the path.
fn index(paths: &[&str]) -> Vec<Entry> {
    index_at(&paths.iter().map(|p| (*p, 0)).collect::<Vec<_>>())
}

/// An index with mtimes (a bigger number: modified later).
fn index_at(paths: &[(&str, u64)]) -> Vec<Entry> {
    paths
        .iter()
        .map(|&(p, m)| match p.strip_suffix('/') {
            Some(d) => Entry::new(d.to_string(), true, m),
            None => Entry::new(p.to_string(), false, m),
        })
        .collect()
}

fn top<'a>(e: &'a [Entry], q: &str, recent: &[&str]) -> Vec<&'a str> {
    let recent: Vec<String> = recent.iter().map(|s| s.to_string()).collect();
    rank(e, q, &recent, 10).into_iter().map(|i| e[i].path.as_str()).collect()
}

const TREE: &[&str] = &[
    "Makefile",
    "README.md",
    ".gitignore",
    "rust/",
    "rust/tui/",
    "rust/tui/src/",
    "rust/tui/src/app.rs",
    "rust/tui/src/sb/",
    "rust/tui/src/sb/mention.rs",
    "rust/tui/src/main.rs",
    "hub/main.bend",
    "docs/main-notes/readme.txt",
    "projects/switchboard/docs/at-mentions.md",
];

#[test]
fn file_name_beats_path() {
    let e = index(TREE);
    // exact name, then name prefix, then the path-only match
    assert_eq!(top(&e, "main.rs", &[]), ["rust/tui/src/main.rs"]);
    assert_eq!(top(&e, "main", &[])[..2], ["hub/main.bend", "rust/tui/src/main.rs"]);
    assert_eq!(top(&e, "mention", &[]), ["rust/tui/src/sb/mention.rs", "projects/switchboard/docs/at-mentions.md"]);
    // "sbmen" only matches along a path: the tighter match first
    assert_eq!(top(&e, "sbmen", &[])[0], "rust/tui/src/sb/mention.rs");
    // case-insensitive; same tier: the shallower first
    assert_eq!(top(&e, "readme", &[])[..2], ["README.md", "docs/main-notes/readme.txt"]);
    assert!(top(&e, "zzz", &[]).is_empty());
}

#[test]
fn exact_beats_prefix_beats_fuzzy() {
    let e = index(&["a/src.rs", "b/src/", "c/sourcery.rs", "d/xsrc.rs"]);
    assert_eq!(top(&e, "src", &[]), ["b/src", "a/src.rs", "c/sourcery.rs", "d/xsrc.rs"]);
}

#[test]
fn folder_part_filters_the_parent_path() {
    let e = index(TREE);
    // the folder part is fuzzy too ("sb/" in "projects/switchboard/docs/")
    assert_eq!(top(&e, "sb/me", &[]), ["rust/tui/src/sb/mention.rs", "projects/switchboard/docs/at-mentions.md"]);
    assert_eq!(top(&e, "docs/me", &[])[0], "projects/switchboard/docs/at-mentions.md");
    assert_eq!(top(&e, "tui/src/m", &[]), ["rust/tui/src/main.rs", "rust/tui/src/sb/mention.rs"]);
    // folder then empty name: the children of that folder (no mtimes: by path)
    assert_eq!(top(&e, "rust/tui/src/", &[]), ["rust/tui/src/app.rs", "rust/tui/src/main.rs", "rust/tui/src/sb"]);
    assert_eq!(top(&e, "src/", &[]), ["rust/tui/src/app.rs", "rust/tui/src/main.rs", "rust/tui/src/sb"]);
}

#[test]
fn empty_query_lists_the_root_dot_files_hidden() {
    let e = index(TREE);
    assert_eq!(top(&e, "", &[]), ["Makefile", "README.md", "rust"]);
    assert_eq!(top(&e, ".git", &[]), [".gitignore"]);
    assert!(!top(&e, "gitig", &[]).contains(&".gitignore"));
}

#[test]
fn recent_picks_come_first_in_their_tier() {
    let e = index(TREE);
    assert_eq!(top(&e, "main", &["rust/tui/src/main.rs"])[..2], ["rust/tui/src/main.rs", "hub/main.bend"]);
    assert_eq!(top(&e, "", &["README.md"])[0], "README.md");
    // a recent path-only match does not jump over a name match
    assert_eq!(top(&e, "mention", &["projects/switchboard/docs/at-mentions.md"])[0], "rust/tui/src/sb/mention.rs");
}

/// Gabriel (at-recent): browsing a folder lists the last modified
/// first, folders and files mixed (a folder by its own mtime, designer
/// m_24034); recent picks stay on top; the folder's subfolders
/// elsewhere (`src/` matching `a/src/`) still come after its own.
#[test]
fn browsing_lists_the_newest_first_folders_and_files_mixed() {
    let e = index_at(&[
        ("2025/", 50),
        ("2026/", 10),
        ("2026/old-folder/", 1),
        ("2026/report.pdf", 9),
        ("2026/new-folder/", 8),
        ("2026/notes.md", 5),
        ("2026/.hidden", 99),
        ("2026/unknown.bin", 0),
        ("a/2026/", 100),
        ("a/2026/deep.txt", 100),
    ]);
    assert_eq!(top(&e, "", &[]), ["2025", "2026"]);
    assert_eq!(
        top(&e, "2026/", &[]),
        ["2026/report.pdf", "2026/new-folder", "2026/notes.md", "2026/old-folder", "2026/unknown.bin"]
    );
    // a recent pick first, then the newest
    assert_eq!(top(&e, "2026/", &["2026/old-folder"])[..2], ["2026/old-folder", "2026/report.pdf"]);
    // not an index folder (`026/`): the shallowest parents first, newest inside
    let e2 = index_at(&[("x/026/", 1), ("x/026/a", 1), ("x/026/b", 2), ("y/z/026/c", 9)]);
    assert_eq!(top(&e2, "026/", &[]), ["x/026/b", "x/026/a", "y/z/026/c"]);
}

/// With a typed name, the match comes first; the newest only breaks a
/// tie (same tier, same score), before the shallower / shorter path.
#[test]
fn a_typed_name_ranks_by_match_then_the_newest() {
    let e = index_at(&[
        ("old/main.rs", 1),
        ("a/b/c/main.rs", 7),
        ("main.rs.bak", 9),
        ("mainly.md", 3),
        ("x/mxaxixn.txt", 100),
    ]);
    // exact name tier: the newer deep one beats the older shallow one
    assert_eq!(top(&e, "main.rs", &[])[..2], ["a/b/c/main.rs", "old/main.rs"]);
    // name prefix tier, newest first; the newest of all only fuzzy: last
    assert_eq!(top(&e, "main", &[]), ["main.rs.bak", "a/b/c/main.rs", "mainly.md", "old/main.rs", "x/mxaxixn.txt"]);
    // all equal mtimes: shallower first, as before
    let flat = index(&["old/main.rs", "a/b/c/main.rs"]);
    assert_eq!(top(&flat, "main.rs", &[]), ["old/main.rs", "a/b/c/main.rs"]);
}

/// R14 (architect m_12214): a window's pick lands on its own root's
/// index, never the root searched last; a re-pick moves to the front;
/// a root with no index (evicted) is a no-op.
#[test]
fn a_pick_in_a_root_is_remembered_on_that_root_only() {
    let (a, b) = (PathBuf::from("/r14/project-a"), PathBuf::from("/r14/project-b"));
    let recent = |r: &Path| state().iter().find(|s| s.root == r).map(|s| s.recent.clone());
    {
        let mut g = state();
        for r in [&a, &b] {
            g.retain(|s| s.root != *r);
            g.insert(0, State { root: r.clone(), entries: Arc::new(Vec::new()), built: None, walking: false, recent: Vec::new() });
        }
    }
    // b was searched last (first in the list): a's pick still goes to a
    picked_in(&a, "src/b.rs");
    picked_in(&a, "src/c.rs");
    picked_in(&a, "src/b.rs");
    assert_eq!(recent(&a).unwrap(), ["src/b.rs", "src/c.rs"]);
    assert_eq!(recent(&b).unwrap(), Vec::<String>::new());
    // an evicted (or never searched) root: nothing, no index made
    picked_in(Path::new("/r14/gone"), "x.rs");
    assert!(recent(Path::new("/r14/gone")).is_none());
    state().retain(|s| s.root != a && s.root != b);
}

#[test]
fn walk_respects_gitignore() {
    let root = std::env::temp_dir().join(format!("at-files-walk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["src", "target/debug", "node_modules/x", ".git"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    for (f, body) in [
        (".gitignore", "target/\nnode_modules\n*.log\n"),
        ("src/lib.rs", ""),
        ("src/app.log", ""),
        ("target/debug/bin", ""),
        ("node_modules/x/i.js", ""),
        (".env.example", ""),
        (".git/HEAD", "ref: refs/heads/main\n"),
    ] {
        std::fs::write(root.join(f), body).unwrap();
    }
    let got: Vec<(String, bool)> = walk(&root, 100).into_iter().map(|e| (e.path, e.dir)).collect();
    let _ = std::fs::remove_dir_all(&root);
    let want = [(".env.example", false), (".gitignore", false), ("src", true), ("src/lib.rs", false)];
    assert_eq!(got, want.map(|(p, d)| (p.to_string(), d)));
}

#[test]
fn token_is_the_at_word_at_the_cursor() {
    assert_eq!(token("@", 1), Some((0, String::new())));
    assert_eq!(token("@ma", 3), Some((0, "ma".into())));
    assert_eq!(token("look at @src/ma", 15), Some((8, "src/ma".into())));
    assert_eq!(token("look at @src/ma now", 15), Some((8, "src/ma".into())));
    assert_eq!(token("look at @src/ma now", 19), None); // cursor past the word
    assert_eq!(token("mail a@b.c", 10), None); // mid-word @
    assert_eq!(token("@main ", 6), None); // done: a space follows
    assert_eq!(token("line\n@ap", 8), Some((5, "ap".into())));
}

#[test]
fn token_is_total_and_reads_an_open_quote() {
    // the cursor on the `@` (← after typing it): no token, no panic
    assert_eq!(token("@", 0), None);
    assert_eq!(token("see @ru", 4), None);
    assert_eq!(token("", 5), None);
    // an open quote: a folder with a space being browsed
    assert_eq!(token("@\"docs/my notes/", 16), Some((0, "docs/my notes/".into())));
    assert_eq!(token("see @\"docs/my notes/a", 21), Some((4, "docs/my notes/a".into())));
    // a closed quote ends it; a quote mid-word is not an opener
    assert_eq!(token("@\"docs/my notes/a.md\" ", 22), None);
    assert_eq!(token("say \"@x", 7), None);
}

#[test]
fn browse_and_parent_query() {
    assert_eq!(browse("rust/tui"), "@rust/tui/");
    assert_eq!(browse("docs/my notes"), "@\"docs/my notes/");
    assert_eq!(browse(""), "@");
    assert_eq!(parent_query("rust/tui/"), Some("rust"));
    assert_eq!(parent_query("rust/"), Some(""));
    assert_eq!(parent_query("rust/tu"), None);
    assert_eq!(parent_query(""), None);
    assert_eq!(replace_token("see @ru now", 4, 7, "@rust/"), ("see @rust/ now".into(), 10));
    assert_eq!(replace_token("@", 5, 9, "@x/"), ("@@x/".into(), 4)); // out of range: clamped, no panic
}

#[test]
fn a_folder_path_scopes_the_search() {
    let e = index(&[
        "rust/",
        "rust/tui/",
        "rust/tui/src/",
        "rust/tui/src/files.rs",
        "rust/tui/src/main.rs",
        "rust/tui/Cargo.toml",
        "rust/other/",
        "rust/other/tui/",
        "rust/other/tui/files.rs",
        "vendor/rust/tui/x.rs",
    ]);
    // an exact folder: its own children only (not vendor/rust/tui/)
    assert_eq!(top(&e, "rust/tui/", &[]), ["rust/tui/Cargo.toml", "rust/tui/src"]);
    // a name inside it: its descendants only
    assert_eq!(top(&e, "rust/tui/fi", &[]), ["rust/tui/src/files.rs"]);
    // not a folder path: fuzzy as before, and the whole query along
    // the path finds folders (`src/fi`, `tu/sr`)
    assert_eq!(top(&e, "tui/", &[]), ["rust/tui/Cargo.toml", "rust/tui/src", "rust/other/tui/files.rs", "vendor/rust/tui/x.rs"]);
    assert_eq!(top(&e, "rust/tu", &[])[..2], ["rust/tui", "rust/other/tui"]);
    assert!(top(&e, "tu/sr", &[]).contains(&"rust/tui/src"));
}

#[test]
fn completion_inserts_the_reference() {
    assert_eq!(reference("src/app.rs", false), "src/app.rs");
    assert_eq!(reference("rust/tui", true), "rust/tui/");
    assert_eq!(reference("docs/my notes.md", false), "\"docs/my notes.md\"");
    assert_eq!(complete("see @ap", 4, 7, "src/app.rs"), ("see src/app.rs ".into(), 15));
    assert_eq!(complete("see @ap now", 4, 7, "src/app.rs"), ("see src/app.rs now".into(), 15));
    let (t, c) = complete("@ma", 0, 3, "hub/main.bend");
    assert_eq!((t.as_str(), c), ("hub/main.bend ", 14));
    assert_eq!(token(&t, c), None); // popup closes
}

/// Per-keystroke latency on a big repo (release):
/// `AT_FILES_BENCH=~/mistral/dashboard cargo test --release -p bend-tui files::tests::bench -- --ignored --nocapture`
#[test]
#[ignore]
fn bench() {
    let root = std::env::var("AT_FILES_BENCH").unwrap_or_else(|_| ".".into());
    let t = Instant::now();
    let e = walk(Path::new(&root), MAX_ENTRIES);
    println!("walk {root}: {} entries in {:.1?}", e.len(), t.elapsed());
    let mut worst = Duration::ZERO;
    for q in ["", "m", "ma", "mai", "main", "main.rs", "src/", "src/comp", "README", "c", "co", "com", "comp", "compose", "composer", "fidx", "zzzq"] {
        let t = Instant::now();
        for _ in 0..10 {
            rank(&e, q, &[], 50);
        }
        let d = t.elapsed() / 10;
        worst = worst.max(d);
        println!("{:>12} {:>9.2?}", format!("{q:?}"), d);
    }
    println!("worst {worst:.2?}");
}

// ---- outside the workspace (BISE-206) ----

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("at-out-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Set `p`'s last modified time to `secs` after the epoch (a folder too).
fn touch(p: &Path, secs: u64) {
    let t = std::time::UNIX_EPOCH + Duration::from_secs(secs);
    std::fs::File::open(p).and_then(|f| f.set_modified(t)).unwrap();
}

/// The outside search once its folder is read (the first keystroke
/// may return before the background read ends).
fn outside_rows(root: &Path, home: Option<&Path>, q: &str) -> Outside {
    for _ in 0..500 {
        let o = search_outside_in(root, home, q, 50);
        if !o.loading {
            return o;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("{q} never read")
}

fn paths(o: &Outside) -> Vec<&str> {
    o.hits.iter().map(|h| h.path.as_str()).collect()
}

fn listed(dir: &Path) -> bool {
    LISTED.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|d| d == dir)
}

#[test]
fn outside_lists_the_typed_folder_only() {
    let up = tmp("up");
    for d in ["ws/src", "other/deep", "zeta"] {
        std::fs::create_dir_all(up.join(d)).unwrap();
    }
    for f in ["b.md", "a.txt", ".env", "other/o.rs"] {
        std::fs::write(up.join(f), "").unwrap();
    }
    for (p, s) in [("zeta", 5), ("b.md", 4), ("other", 3), ("a.txt", 2), ("ws", 1), ("other/deep", 2), ("other/o.rs", 1)] {
        touch(&up.join(p), s);
    }
    let ws = up.join("ws");
    assert!(outside("../") && outside("~/") && outside("/") && !outside("src/") && !outside(".."));
    // `../`: the parent's entries, the newest first (folders and files
    // mixed), dot files hidden
    let o = outside_rows(&ws, None, "../");
    assert_eq!(paths(&o), ["../zeta", "../b.md", "../other", "../a.txt", "../ws"]);
    let dirs: Vec<bool> = o.hits.iter().map(|h| h.dir).collect();
    assert_eq!(dirs, [true, false, true, false, true]);
    assert!(o.hits.iter().all(|h| !h.protected));
    // the name part narrows; dot files once it starts with `.`
    assert_eq!(paths(&outside_rows(&ws, None, "../b")), ["../b.md"]);
    assert_eq!(paths(&outside_rows(&ws, None, "../.e")), ["../.env"]);
    // a folder not entered is not read; entered, only it
    assert!(!listed(&up.join("other")) && !listed(&up.join("other/deep")));
    assert_eq!(paths(&outside_rows(&ws, None, "../other/")), ["../other/deep", "../other/o.rs"]);
    assert!(listed(&ws.join("../other/")) && !listed(&up.join("other/deep")));
    // `/`: absolute, typed as is
    let abs = format!("{}/", up.join("other").display());
    assert_eq!(paths(&outside_rows(&ws, None, &abs)), [format!("{abs}deep"), format!("{abs}o.rs")]);
    let _ = std::fs::remove_dir_all(&up);
}

#[test]
fn protected_folders_are_read_only_once_entered() {
    let home = tmp("home");
    for d in ["Desktop", "Documents", "Downloads", "Library/Mobile Documents", "code"] {
        std::fs::create_dir_all(home.join(d)).unwrap();
    }
    std::fs::write(home.join("Desktop/secret.txt"), "").unwrap();
    let ws = home.join("code");
    let h = Some(home.as_path());
    let o = outside_rows(&ws, h, "~/");
    let mut prot: Vec<&str> = o.hits.iter().filter(|x| x.protected).map(|x| x.path.as_str()).collect();
    prot.sort(); // listed newest first: made in the same instant, any order
    assert_eq!(prot, ["~/Desktop", "~/Documents", "~/Downloads"]);
    assert!(!o.hits.iter().any(|x| x.path == "~/code" && x.protected));
    // typing its name, even whole, reads nothing inside
    for q in ["~/Desk", "~/Desktop", "~/Library/", "~/Library/Mob"] {
        let o = outside_rows(&ws, h, q);
        assert!(!listed(&home.join("Desktop")), "{q} read Desktop");
        if q == "~/Library/" {
            assert!(o.hits[0].protected && o.hits[0].path == "~/Library/Mobile Documents");
        }
    }
    assert!(!listed(&home.join("Library/Mobile Documents")));
    // the `/` after it enters it: read now
    assert_eq!(paths(&outside_rows(&ws, h, "~/Desktop/")), ["~/Desktop/secret.txt"]);
    assert!(listed(&home.join("Desktop/")));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_folder_that_cannot_be_read_is_locked_and_empty() {
    use std::os::unix::fs::PermissionsExt;
    let up = tmp("eperm");
    let lock = up.join("Mail");
    std::fs::create_dir_all(lock.join("inbox")).unwrap();
    std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&lock).is_ok() {
        // root reads anything: nothing to test
        let _ = std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o755));
        return;
    }
    let q = format!("{}/", lock.display());
    let o = outside_rows(&up, None, &q);
    assert!(o.locked && o.hits.is_empty());
    assert!(outside_rows(&up, None, "/no/such/folder/").locked);
    let _ = std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o755));
    let _ = std::fs::remove_dir_all(&up);
}

#[test]
fn the_sent_path_expands_home_and_keeps_the_rest() {
    let h = Some(Path::new("/Users/me"));
    assert_eq!(sent_path_in(h, "~/Documents/a b.md"), "/Users/me/Documents/a b.md");
    assert_eq!(sent_path_in(Some(Path::new("/Users/me/")), "~/x"), "/Users/me/x");
    assert_eq!(sent_path_in(h, "../other/o.rs"), "../other/o.rs");
    assert_eq!(sent_path_in(h, "/etc/hosts"), "/etc/hosts");
    assert_eq!(sent_path_in(None, "~/x"), "~/x");
    assert_eq!(resolve_in(Path::new("/ws"), h, "~/Desktop/"), Path::new("/Users/me/Desktop/"));
    assert_eq!(resolve_in(Path::new("/ws"), h, "../o/"), Path::new("/ws/../o/"));
    assert_eq!(resolve_in(Path::new("/ws"), h, "/usr/"), Path::new("/usr/"));
    // browsing: `/` is the root, one folder up from `/usr/` is `/`
    assert_eq!(browse("/"), "@/");
    assert_eq!(browse("~/Documents"), "@~/Documents/");
    assert_eq!(parent_query("/usr/"), Some("/"));
    assert_eq!(parent_query("/usr/lib/"), Some("/usr"));
    assert_eq!(parent_query("~/Documents/"), Some("~"));
    assert_eq!(parent_query("../../"), Some(".."));
}
