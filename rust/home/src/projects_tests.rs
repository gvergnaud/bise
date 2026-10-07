use super::*;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-projects-test-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    canonical(&d)
}

fn places() -> Places {
    Places { home_ws: "/u/bise".into(), hubs: "/u/.bise/hubs".into(), worktrees: "/u/.bise/worktrees".into() }
}

fn added(r: Result<Added, String>) -> Project {
    match r {
        Ok(Added::New(p)) => p,
        other => panic!("not added: {other:?}"),
    }
}

#[test]
fn add_names_by_folder_and_dedupes_by_path() {
    let mut l = Vec::new();
    let a = added(add(&mut l, Path::new("/code/api"), None, 5, &places(), None));
    assert_eq!((a.name.as_str(), a.order, a.added_ms), ("api", 0, 5));
    // same path again: unchanged
    assert_eq!(add(&mut l, Path::new("/code/api"), Some("x"), 9, &places(), None), Ok(Added::Already(a.clone())));
    // another folder with the same name: api-2, then api-3
    assert_eq!(added(add(&mut l, Path::new("/other/api"), None, 6, &places(), None)).name, "api-2");
    assert_eq!(added(add(&mut l, Path::new("/third/api"), None, 7, &places(), None)).name, "api-3");
    // a given name is kept, a clash with it is refused
    assert_eq!(added(add(&mut l, Path::new("/w/telemetry"), Some("tel"), 8, &places(), None)).name, "tel");
    assert!(add(&mut l, Path::new("/w/x"), Some("tel"), 8, &places(), None).unwrap_err().contains("already named tel"));
    assert!(add(&mut l, Path::new("/w/y"), Some("  "), 8, &places(), None).is_err());
    assert_eq!(l.len(), 4);
}

#[test]
fn bise_own_places_are_never_projects() {
    let mut l = Vec::new();
    for p in ["/u/bise", "/u/bise/notes", "/u/.bise/hubs/api-12345678", "/u/.bise/worktrees/api-12345678/task"] {
        assert!(add(&mut l, Path::new(p), None, 1, &places(), None).is_err(), "{p} was added");
    }
    // a git worktree of a registered project (an agent's task folder
    // elsewhere): refused; of an unregistered repo: a project
    added(add(&mut l, Path::new("/code/api"), None, 1, &places(), None));
    let e = add(&mut l, Path::new("/elsewhere/api-wt"), None, 2, &places(), Some(Path::new("/code/api"))).unwrap_err();
    assert!(e.contains("git worktree of the project api"), "{e}");
    added(add(&mut l, Path::new("/elsewhere/web-wt"), None, 2, &places(), Some(Path::new("/code/web"))));
    assert!(l.iter().all(|p| !p.path.starts_with("/u/")));
}

#[test]
fn a_git_file_names_the_main_repo() {
    let d = tmp("gitfile");
    std::fs::create_dir_all(d.join("repo/.git/worktrees/t1")).unwrap();
    let text = format!("gitdir: {}\n", d.join("repo/.git/worktrees/t1").display());
    assert_eq!(main_of_gitdir(&text), Some(d.join("repo")));
    assert_eq!(main_of_gitdir("gitdir: /x/modules/sub\n"), None);
    assert_eq!(main_of_gitdir("not a git file"), None);
    // a real folder: .git is a dir (a repo) or absent
    std::fs::create_dir_all(d.join("plain/.git")).unwrap();
    assert_eq!(worktree_main(&d.join("plain")), None);
    std::fs::create_dir_all(d.join("wt")).unwrap();
    std::fs::write(d.join("wt/.git"), &text).unwrap();
    assert_eq!(worktree_main(&d.join("wt")), Some(d.join("repo")));
}

#[test]
fn remove_rename_and_move_by_name_or_path() {
    let mut l = Vec::new();
    for p in ["/c/a", "/c/b", "/c/c"] {
        added(add(&mut l, Path::new(p), None, 1, &places(), None));
    }
    move_to(&mut l, "c", 0).unwrap();
    assert_eq!(l.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["c", "a", "b"]);
    move_to(&mut l, "/c/c", 99).unwrap();
    assert_eq!(l.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    rename(&mut l, "/c/b", "bee").unwrap();
    assert!(rename(&mut l, "a", "bee").unwrap_err().contains("already named"));
    assert!(rename(&mut l, "a", "").is_err());
    rename(&mut l, "a", "a").unwrap();
    assert_eq!(remove(&mut l, "bee").unwrap().path, PathBuf::from("/c/b"));
    assert!(remove(&mut l, "bee").unwrap_err().contains("no project bee"));
    assert!(move_to(&mut l, "zz", 0).is_err());
    assert_eq!(l.len(), 2);
}

#[test]
fn the_file_round_trips_and_a_bad_one_reads_empty() {
    let l = vec![
        Project { path: "/c/b".into(), name: "b".into(), order: 1, added_ms: 2 },
        Project { path: "/c/a".into(), name: "a".into(), order: 0, added_ms: 1 },
    ];
    let back = parse(&render(&l));
    assert_eq!(back.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(back[1], l[0]);
    assert!(parse("").is_empty());
    assert!(parse("{not json").is_empty());
    assert!(parse(r#"{"v":1,"projects":"x"}"#).is_empty());
    // a row with no path is skipped; no name: the folder's
    let p = parse(r#"{"v":1,"projects":[{"name":"x"},{"path":"/c/z"}]}"#);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].name, "z");
}

#[test]
fn home_is_row_zero_and_never_in_the_file() {
    let l = vec![Project { path: "/c/a".into(), name: "a".into(), order: 0, added_ms: 1 }];
    let r = rows(&l, Path::new("/u/bise"));
    assert_eq!(r.len(), 2);
    assert!(r[0].home && r[0].name == "bise" && r[0].id == crate::hub_id(Path::new("/u/bise")));
    assert!(!r[1].home && r[1].id == crate::hub_id(Path::new("/c/a")));
    let mut l2 = l.clone();
    assert!(remove(&mut l2, "bise").is_err());
}

// desktop S2: a cross-hub message goes to a registered row, by name or
// id, and never to the sender's own hub (architect m_8524 3.)
#[test]
fn target_is_a_row_and_never_the_sender() {
    let l = vec![Project { path: "/c/shop".into(), name: "shop".into(), order: 0, added_ms: 1 }];
    let r = rows(&l, Path::new("/u/bise"));
    let (home, shop) = (crate::hub_id(Path::new("/u/bise")), crate::hub_id(Path::new("/c/shop")));
    assert_eq!(target(&r, "shop", &home), Ok(shop.clone()));
    assert_eq!(target(&r, "@shop", &home), Ok(shop.clone()));
    assert_eq!(target(&r, &shop, &home), Ok(shop.clone()));
    assert!(target(&r, "shop", &shop).unwrap_err().contains("this hub itself"));
    assert!(target(&r, "bise", &home).unwrap_err().contains("this hub itself"));
    assert!(target(&r, "nope", &home).unwrap_err().contains("no project named nope"));
}

// amb-core m_8673: the home workspace named through a symlink
// (/var/folders vs /private/var/folders) is one row, its hub's id
#[test]
fn home_row_is_the_canonical_home_workspace() {
    let root = tmp("home-link");
    let real = root.join("real-ws");
    std::fs::create_dir_all(&real).unwrap();
    let link = root.join("link-ws");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let r = rows(&[], &link);
    assert_eq!(r[0].path, real);
    assert_eq!(r[0].id, crate::hub_id(&real));
    // and the hub's boot at the real path is refused as home
    let home = Home::at(root.join(".bise"));
    assert!(add_path(&home, &link, &real, None, 1).is_err());
    assert_eq!(list(&home, &link).len(), 1);
}

#[test]
fn update_writes_whole_and_an_error_writes_nothing() {
    let root = tmp("update");
    let home = Home::at(root.join(".bise"));
    let ws = root.join("bise");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(root.join("code/api")).unwrap();
    let p = add_path(&home, &ws, &root.join("code/../code/api"), None, 3).unwrap();
    assert!(matches!(p, Added::New(ref p) if p.path == root.join("code/api")));
    assert!(add_path(&home, &ws, &root.join("nope"), None, 3).unwrap_err().contains("no such folder"));
    assert!(add_path(&home, &ws, &ws, None, 3).is_err());
    let before = std::fs::read_to_string(home.projects_file().unwrap()).unwrap();
    assert!(update(&home, |l| remove(l, "zz")).is_err());
    assert_eq!(std::fs::read_to_string(home.projects_file().unwrap()).unwrap(), before);
    let rows = list(&home, &ws);
    assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["bise", "api"]);
    // a newer bise's file is never written over
    std::fs::write(home.projects_file().unwrap(), r#"{"v":2,"projects":[]}"#).unwrap();
    assert!(update(&home, |_| Ok(())).unwrap_err().contains("newer bise"));
}

#[test]
fn racing_writers_lose_no_add() {
    let root = tmp("race");
    let home = Home::at(root.join(".bise"));
    let ws = root.join("bise");
    let dirs: Vec<PathBuf> = (0..16).map(|i| root.join(format!("p{i}"))).collect();
    for d in &dirs {
        std::fs::create_dir_all(d).unwrap();
    }
    let threads: Vec<_> = dirs
        .into_iter()
        .map(|d| {
            let (home, ws) = (home.clone(), ws.clone());
            std::thread::spawn(move || add_path(&home, &ws, &d, None, 1).unwrap())
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let l = read(&home);
    assert_eq!(l.len(), 16);
    assert_eq!(l.iter().map(|p| p.order).collect::<Vec<_>>(), (0..16).collect::<Vec<u32>>());
}

#[test]
fn the_legacy_layout_has_home_only_and_refuses_writes() {
    let m: std::collections::HashMap<String, String> = [("HOME".to_string(), "/h".to_string())].into();
    let home = Home::from_lookup(&move |k: &str| m.get(k).cloned());
    assert_eq!(home.layout(), Layout::Legacy);
    assert_eq!(home.projects_file(), None);
    assert_eq!(list(&home, Path::new("/h/bise")).len(), 1);
    assert_eq!(update(&home, |_| Ok(())).unwrap_err(), LEGACY);
}
