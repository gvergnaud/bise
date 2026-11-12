//! The home hub's id table on a throwaway root (daemon/files_ids.rs): the
//! walk, a rename and a safe save keep their ids, files/mint's checks, a
//! broken table moved aside.

use super::*;
use crate::view_bridge::tests::tmp;
use std::fs;

/// A fresh root and its table file beside it (never under the root).
fn table(name: &str) -> (PathBuf, Table) {
    let d = tmp(&format!("ids-{name}"));
    let root = d.join("bise");
    fs::create_dir_all(&root).unwrap();
    let (t, said) = Table::open(&root, d.join("view-ids.json"));
    assert!(said.is_empty(), "{said:?}");
    (fs::canonicalize(&root).unwrap(), t)
}

fn id(t: &Table, rel: &str) -> Option<String> {
    t.snap.id_of(rel).map(|i| i.to_string())
}

fn mint(t: &mut Table, rel: &str) -> Result<(String, Vec<bise_ids::Change>), RpcError> {
    t.mint(rel, &|_| {})
}

/// A file minted before its folders are known: they get their ids in the
/// same job, never waiting on an event (architect m_63071).
#[test]
fn a_mint_reads_the_folders_the_table_lacks() {
    let (root, mut t) = table("mint-parents");
    fs::create_dir_all(root.join("a/b")).unwrap();
    fs::write(root.join("a/b/c.md"), "c").unwrap();
    fs::write(root.join("a/side.md"), "s").unwrap();
    let (c, changes) = mint(&mut t, "a/b/c.md").unwrap();
    assert!(id(&t, "a").is_some() && id(&t, "a/b").is_some(), "{changes:?}");
    assert!(id(&t, "a/side.md").is_some(), "its folder was read whole");
    assert_eq!(id(&t, "a/b/c.md"), Some(c.clone()));
    let d = t.changed(&changes, &|_| {}).expect("one event");
    assert_eq!(d.entries.iter().filter(|e| e.id == c).count(), 1, "{:?}", d.entries);
    assert_eq!(folders_missing(&t.snap, "a/b/c.md"), Vec::<Cover>::new());
}

#[test]
fn a_walk_keeps_ids_across_a_rename_and_a_safe_save_and_skips_what_it_must() {
    let (root, mut t) = table("walk");
    fs::create_dir_all(root.join("notes/deep")).unwrap();
    fs::write(root.join("notes/a.md"), "a").unwrap();
    fs::write(root.join("notes/deep/b.md"), "b").unwrap();
    fs::write(root.join(".DS_Store"), "x").unwrap();
    fs::write(root.join("notes/a.md~"), "x").unwrap();
    std::os::unix::fs::symlink(root.join("notes"), root.join("link")).unwrap();
    t.walk_apply(&[Cover::FULL], &|_| {});
    let mut paths: Vec<&str> = t.snap.iter().map(|(p, _, _)| p).collect();
    paths.sort();
    assert_eq!(paths, vec!["notes", "notes/a.md", "notes/deep", "notes/deep/b.md"], "hidden, backup and symlink skipped");
    let (a, b) = (id(&t, "notes/a.md").unwrap(), id(&t, "notes/deep/b.md").unwrap());

    // a rename, seen through its folder only
    fs::rename(root.join("notes/a.md"), root.join("notes/c.md")).unwrap();
    t.walk_apply(&[Cover::Dir("notes".into())], &|_| {});
    assert_eq!((id(&t, "notes/c.md"), id(&t, "notes/a.md")), (Some(a.clone()), None));

    // a safe save: a temp file beside it, renamed over it (a new ino)
    fs::write(root.join("notes/.c.md.bise-1-2"), "c2").unwrap();
    fs::rename(root.join("notes/.c.md.bise-1-2"), root.join("notes/c.md")).unwrap();
    let changes = t.walk_apply(&[Cover::Dir("notes".into())], &|_| {});
    assert_eq!(id(&t, "notes/c.md"), Some(a.clone()), "{changes:?}");
    let d = t.changed(&changes, &|_| {}).expect("a change");
    assert_eq!((d.seq, d.entries.len(), d.entries[0].id.as_str(), d.entries[0].size), (1, 1, a.as_str(), 2));

    // a folder moved: its rows keep their ids
    fs::rename(root.join("notes/deep"), root.join("deep")).unwrap();
    t.walk_apply(&[Cover::Dir("".into()), Cover::Dir("notes".into()), Cover::Tree("deep".into())], &|_| {});
    assert_eq!(id(&t, "deep/b.md"), Some(b));

    // saved, and loaded again as it was
    t.save().unwrap();
    let (again, said) = Table::open(&root, t.file.clone());
    assert!(said.is_empty());
    assert_eq!(again.snap, t.snap);
}

#[test]
fn a_folder_gone_before_its_walk_is_removed_through_its_parent() {
    let (root, mut t) = table("gone");
    fs::create_dir_all(root.join("x/y")).unwrap();
    fs::write(root.join("x/y/a.md"), "a").unwrap();
    t.walk_apply(&[Cover::FULL], &|_| {});
    assert!(id(&t, "x/y/a.md").is_some());
    fs::remove_dir_all(root.join("x")).unwrap();
    t.walk_apply(&[Cover::Dir("x/y".into()), Cover::Tree("x".into())], &|_| {});
    assert!(t.snap.is_empty(), "{:?}", t.snap.iter().map(|(p, _, _)| p).collect::<Vec<_>>());
}

/// An accented name has one id whichever side saw it first: the walk
/// reads the disk's bytes, files/mint takes only the disk's bytes (APFS
/// finds `é` spelled either way; the table never does).
#[test]
fn an_accented_name_gets_one_id_through_the_walk_and_through_mint() {
    let (root, mut t) = table("accent");
    let nfd = "cafe\u{301}.md";
    let nfc = "caf\u{e9}.md";
    fs::write(root.join(nfd), "x").unwrap();
    let (minted, changes) = mint(&mut t, nfd).unwrap();
    assert_eq!(changes.len(), 1, "minted before the watcher saw it");
    let (again, changes) = mint(&mut t, nfd).unwrap();
    assert_eq!((again.as_str(), changes.len()), (minted.as_str(), 0), "idempotent");
    let other = mint(&mut t, nfc).unwrap_err();
    assert_eq!(other.data.and_then(|d| d.reason).as_deref(), Some("not_found"), "the other spelling is not the disk's");
    t.walk_apply(&[Cover::FULL], &|_| {});
    assert_eq!(id(&t, nfd), Some(minted));
    assert_eq!(t.snap.len(), 1);
}

#[test]
fn mint_refuses_an_untracked_a_missing_and_a_symlinked_path() {
    let (root, mut t) = table("mint");
    fs::create_dir_all(root.join("x")).unwrap();
    fs::write(root.join("x/a.md"), "a").unwrap();
    std::os::unix::fs::symlink(root.join("x"), root.join("l")).unwrap();
    let reason = |r: Result<(String, Vec<bise_ids::Change>), RpcError>| r.unwrap_err().data.and_then(|d| d.reason).unwrap_or_default();
    assert_eq!(reason(mint(&mut t, ".hidden")), "bad_request");
    assert_eq!(reason(mint(&mut t, "x/../x/a.md")), "bad_request");
    assert_eq!(reason(mint(&mut t, "/x/a.md")), "bad_request");
    assert_eq!(reason(mint(&mut t, "x/b.md")), "not_found");
    assert_eq!(reason(mint(&mut t, "x/a.md/b")), "not_found");
    assert_eq!(reason(mint(&mut t, "l")), "bad_request");
    assert_eq!(reason(mint(&mut t, "l/a.md")), "bad_request");
    let (id_a, _) = mint(&mut t, "x/a.md").unwrap();
    assert!(id_a.starts_with("e:"));
    assert!(t.snap.id_of("l/a.md").is_none());
}

#[test]
fn a_broken_table_moves_aside_and_the_walk_starts_empty() {
    let d = tmp("ids-broken");
    let root = d.join("bise");
    fs::create_dir_all(&root).unwrap();
    let file = d.join("view-ids.json");
    fs::write(&file, "{not json").unwrap();
    let (t, said) = Table::open(&root, file.clone());
    assert!(t.snap.is_empty());
    assert_eq!(said.len(), 1, "one log line");
    assert!(!file.exists());
    let aside: Vec<_> = fs::read_dir(&d).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("view-ids.broken-")).collect();
    assert_eq!(aside.len(), 1);
}
