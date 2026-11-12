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
    t.walk_apply(&[Cover::FULL], &|_| {}, &mut |_| {});
    let mut paths: Vec<&str> = t.snap.iter().map(|(p, _, _)| p).collect();
    paths.sort();
    assert_eq!(paths, vec!["notes", "notes/a.md", "notes/deep", "notes/deep/b.md"], "hidden, backup and symlink skipped");
    let (a, b) = (id(&t, "notes/a.md").unwrap(), id(&t, "notes/deep/b.md").unwrap());

    // a rename, seen through its folder only
    fs::rename(root.join("notes/a.md"), root.join("notes/c.md")).unwrap();
    t.walk_apply(&[Cover::Dir("notes".into())], &|_| {}, &mut |_| {});
    assert_eq!((id(&t, "notes/c.md"), id(&t, "notes/a.md")), (Some(a.clone()), None));

    // a safe save: a temp file beside it, renamed over it (a new ino)
    fs::write(root.join("notes/.c.md.bise-1-2"), "c2").unwrap();
    fs::rename(root.join("notes/.c.md.bise-1-2"), root.join("notes/c.md")).unwrap();
    let changes = t.walk_apply(&[Cover::Dir("notes".into())], &|_| {}, &mut |_| {});
    assert_eq!(id(&t, "notes/c.md"), Some(a.clone()), "{changes:?}");
    let d = t.changed(&changes, &|_| {}).expect("a change");
    assert_eq!((d.seq, d.entries.len(), d.entries[0].id.as_str(), d.entries[0].size), (1, 1, a.as_str(), 2));

    // a folder moved: its rows keep their ids
    fs::rename(root.join("notes/deep"), root.join("deep")).unwrap();
    t.walk_apply(&[Cover::Dir("".into()), Cover::Dir("notes".into()), Cover::Tree("deep".into())], &|_| {}, &mut |_| {});
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
    t.walk_apply(&[Cover::FULL], &|_| {}, &mut |_| {});
    assert!(id(&t, "x/y/a.md").is_some());
    fs::remove_dir_all(root.join("x")).unwrap();
    t.walk_apply(&[Cover::Dir("x/y".into()), Cover::Tree("x".into())], &|_| {}, &mut |_| {});
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
    t.walk_apply(&[Cover::FULL], &|_| {}, &mut |_| {});
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

fn listed(t: &mut Table, folder: Option<&str>, links: &Links) -> Result<FolderList, RpcError> {
    t.list(folder, links, &|_| {}).0.map(|(l, _)| l)
}

fn reason(e: RpcError) -> String {
    e.data.and_then(|d| d.reason).unwrap_or_default()
}

/// folder/list (A4b): folders first then by name, dotfiles out, crumbs
/// from the root, a folder's names, a plugin's documents for the matcher,
/// the folder read again from disk, and the wrong ids refused.
#[test]
fn folder_list_reads_its_folder_again_and_gives_rows_crumbs_and_names() {
    let (root, mut t) = table("list");
    fs::create_dir_all(root.join("notes/deep")).unwrap();
    fs::create_dir_all(root.join("Plugins/sheets")).unwrap();
    fs::write(root.join("b.md"), "bb").unwrap();
    fs::write(root.join("A.txt"), "a").unwrap();
    fs::write(root.join(".hidden"), "h").unwrap();
    fs::write(root.join("notes/x.md"), "x").unwrap();
    fs::write(root.join("Plugins/sheets/q3.xlsx"), "q").unwrap();
    t.walk_apply(&[Cover::FULL], &|_| {}, &mut |_| {});
    let none = Links::default();

    let top = listed(&mut t, None, &none).unwrap();
    let names: Vec<&str> = top.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["notes", "Plugins", "A.txt", "b.md"], "folders first, then by name whatever the case; no dotfile");
    assert_eq!((top.folder.id.as_str(), top.crumb.len()), (ROOT_CRUMB, 0));
    assert_eq!(top.rows[0].names.names, vec!["deep", "x.md"]);
    assert_eq!((top.rows[3].kind, top.rows[3].size, top.rows[0].size), (FileKind::File, 2, 0));
    assert_eq!(listed(&mut t, Some(ROOT_CRUMB), &none).unwrap(), top, "the root's crumb sent back lists the root");

    // a file made after the walk, before any event: the list reads it
    fs::write(root.join("notes/deep/new.md"), "n").unwrap();
    let deep = id(&t, "notes/deep").unwrap();
    let (r, changes) = t.list(Some(&deep), &none, &|_| {});
    let (l, _) = r.unwrap();
    assert_eq!((l.rows.len(), l.rows[0].name.as_str(), changes.len()), (1, "new.md", 1));
    assert_eq!(l.folder, Crumb { id: deep, name: "deep".into() });
    let crumb: Vec<(&str, &str)> = l.crumb.iter().map(|c| (c.id.as_str(), c.name.as_str())).collect();
    let notes = id(&t, "notes").unwrap();
    assert_eq!(crumb, vec![(ROOT_CRUMB, ""), (notes.as_str(), "notes")]);

    // a plugin's document: that plugin's, for the opener's pick
    let plug = id(&t, "Plugins/sheets").unwrap();
    let (_, targets) = t.list(Some(&plug), &none, &|_| {}).0.unwrap();
    assert_eq!(targets[0].documents, Some(("sheets".to_string(), "q3.xlsx".to_string())));

    let b = id(&t, "b.md").unwrap();
    assert_eq!(reason(listed(&mut t, Some(&b), &none).unwrap_err()), "bad_request", "a file is no folder");
    assert_eq!(reason(listed(&mut t, Some("e:999999"), &none).unwrap_err()), "bad_request");
    assert_eq!(reason(listed(&mut t, Some("nope"), &none).unwrap_err()), "bad_request");
    fs::remove_dir_all(root.join("notes")).unwrap();
    assert_eq!(reason(listed(&mut t, Some(&notes), &none).unwrap_err()), "not_found", "gone since the last walk");
}

/// One document, one write path (A4): a `~/bise` row that is an
/// artifact's current file says so, and opening it by its entry id binds
/// the same target as opening its card (architect m_63160's test).
#[test]
fn an_artifacts_row_opens_as_its_card_does() {
    let (root, mut t) = table("art-link");
    let state = root.parent().unwrap().join("state");
    fs::create_dir_all(root.join("work")).unwrap();
    let f = root.join("work/q3.csv");
    fs::write(&f, "a,b\n").unwrap();
    fs::write(root.join("work/plain.md"), "p").unwrap();
    let store = crate::artifacts::Store::new(&state);
    let add = crate::artifacts::Add { target: f.to_string_lossy().to_string(), agent: "perf".into(), by: "perf".into(), cwd: root.to_string_lossy().to_string(), ..Default::default() };
    let art = store.add(&add, 10).unwrap().meta.id;
    let links = super::super::art_rows::links_of(&store.all());
    t.walk_apply(&[Cover::FULL], &|_| {}, &mut |_| {});

    let work = id(&t, "work").unwrap();
    let l = listed(&mut t, Some(&work), &links).unwrap();
    let row = |n: &str| l.rows.iter().find(|r| r.name == n).unwrap().clone();
    assert_eq!(row("q3.csv").artifact.map(|a| a.id), Some(art.clone()));
    assert_eq!(row("plain.md").artifact, None);

    let by_entry = crate::view_files_disk::resolve_entry(&store, &root, &t.path(&row("q3.csv").id).unwrap(), &links).unwrap();
    let by_card = crate::view_files_disk::resolve_artifact(&store, &art, None).unwrap();
    assert_eq!(by_entry.target, by_card.target);
    let plain = crate::view_files_disk::resolve_entry(&store, &root, &t.path(&row("plain.md").id).unwrap(), &links).unwrap();
    assert_eq!(plain.target, crate::view_files::FileTarget::RootFile { path: root.join("work/plain.md") });
    assert_eq!(t.path("e:999999"), None);
}

/// A FULL walk answers the jobs that can't wait between its folders: a
/// `view/open` asks a path in the middle of it and has it then; a
/// subscribe waits for the walk's end (architect m_63160).
#[test]
fn a_path_asked_in_the_middle_of_a_full_walk_is_answered_there() {
    let (root, mut t) = table("between");
    fs::write(root.join("known.md"), "k").unwrap();
    t.walk_apply(&[Cover::FULL], &|_| {}, &mut |_| {});
    let known = id(&t, "known.md").unwrap();
    for i in 0..5 {
        fs::create_dir_all(root.join(format!("d{i}/e"))).unwrap();
    }
    let (jobs_tx, jobs) = channel::<Job>();
    let (msg_tx, msgs) = channel::<Msg>();
    let o = Out { tx: &msg_tx, log: &|_| {} };
    let mut later = VecDeque::new();
    let (reply_tx, reply) = channel();
    let (mut calls, mut answered_at) = (0, None);
    t.walk_apply(&[Cover::FULL], &|_| {}, &mut |t: &mut Table| {
        calls += 1;
        if calls == 3 {
            jobs_tx.send(Job::Path(known.clone(), reply_tx.clone())).unwrap();
            jobs_tx.send(Job::Subscribe(ClientId::default(), Id::Num(1))).unwrap();
        }
        between(t, &jobs, &o, &mut later);
        if answered_at.is_none() && reply.try_recv().is_ok_and(|p| p == Some(root.join("known.md"))) {
            answered_at = Some(calls);
        }
    });
    assert!(calls > 4, "the walk read {calls} folders");
    assert_eq!(answered_at, Some(3), "answered between folders, not after the walk");
    assert_eq!(later.len(), 1, "the subscribe waits for the walk");
    assert!(msgs.try_recv().is_err(), "nothing sent to the loop for it yet");
}

/// The newest listing wins (architect m_63320): a FULL walk reads A,
/// then a file is made in A and another deleted, then a `List(A)`
/// between its folders applies them; when the walk ends, its older read
/// of A changes nothing: the new file keeps its id (no remove, no add)
/// and the deleted one stays gone.
#[test]
fn a_list_between_folders_wins_over_the_walks_older_read() {
    let (root, mut t) = table("fresher");
    fs::create_dir_all(root.join("A/B/C")).unwrap();
    fs::write(root.join("A/g.md"), "g").unwrap();
    t.walk_apply(&[Cover::FULL], &|_| {}, &mut |_| {});
    let a = id(&t, "A").unwrap();
    let (mut calls, mut made) = (0, None);
    // one folder per level: the walk reads "", A, A/B, A/B/C in that order
    let changes = t.walk_apply(&[Cover::FULL], &|_| {}, &mut |t: &mut Table| {
        calls += 1;
        if calls == 3 {
            fs::write(root.join("A/f.md"), "f").unwrap();
            fs::remove_file(root.join("A/g.md")).unwrap();
            let (r, inner) = t.list(Some(&a), &Links::default(), &|_| {});
            assert!(r.is_ok() && inner.len() == 2, "{inner:?}");
            made = t.snap.id_of("A/f.md");
        }
    });
    assert_eq!(calls, 4);
    let f = made.expect("the List gave f an id");
    assert_eq!(t.snap.id_of("A/f.md"), Some(f), "f keeps its id");
    assert!(t.snap.id_of("A/g.md").is_none(), "g stays gone");
    assert!(changes.is_empty(), "the walk's older read of A changed nothing: {changes:?}");
    assert!(t.fresh.is_empty(), "emptied when the outer walk ends");
}

/// The rule alone, on rows: a folder applied by an inner walk after this
/// walk read it is dropped (entries and cover), one read after it is
/// kept; a folder under a dropped one the table lost goes too; a Tree
/// over a dropped folder becomes its other folders' Dirs.
#[test]
fn fresher_wins_drops_the_folders_read_before_a_newer_listing() {
    let e = |ino| Entry { dev: 1, ino, kind: Kind::File, size: 1, mtime_ms: 1 };
    let read: Read = [
        ("", 0, vec![("A", 1), ("K", 2)]),
        ("A", 0, vec![("A/x", 3), ("A/gone", 4)]),
        ("A/lost", 0, vec![("A/lost/y", 5)]),
        ("A/kept", 0, vec![("A/kept/z", 6)]),
        ("K", 2, vec![("K/w", 7)]),
    ]
    .into_iter()
    .map(|(d, at, s)| (d.to_string(), (at, s.into_iter().map(|(p, i)| (p.to_string(), e(i))).collect())))
    .collect();
    // A applied at 1 (after this walk read it at 0); K applied at 1, read at 2
    let fresh: BTreeMap<String, u64> = [("A".to_string(), 1), ("K".to_string(), 1)].into();
    let known = |d: &str| d != "A/lost";
    let (scan, covers, folders) = fresher_wins(vec![Cover::FULL], read, &fresh, &known);
    assert_eq!(covers, vec![Cover::Dir("".into()), Cover::Dir("A/kept".into()), Cover::Dir("K".into())]);
    assert_eq!(folders, vec!["", "A/kept", "K"]);
    let mut expect = Scan::new();
    for (p, i) in [("A", 1), ("K", 2), ("A/kept/z", 6), ("K/w", 7)] {
        expect.insert(p, e(i)).unwrap();
    }
    assert_eq!(scan, expect);
    // nothing fresher: the cover and the scan as read
    let (_, covers, _) = fresher_wins(vec![Cover::FULL, Cover::Dir("A".into())], Read::new(), &BTreeMap::new(), &known);
    assert_eq!(covers, vec![Cover::FULL, Cover::Dir("A".into())]);
}

/// file/new's file (A4b): the make's title, then `Sheet 2` when taken
/// (the words' pick, never over a file), in the folder asked, with its
/// id at once; a file or an unknown id as the folder: refused.
#[test]
fn a_new_file_takes_the_first_free_name_in_its_folder_and_its_id() {
    let (root, mut t) = table("new");
    fs::create_dir_all(root.join("q3")).unwrap();
    fs::write(root.join("Sheet.xlsx"), "his").unwrap();
    fs::write(root.join("q3/a.md"), "a").unwrap();
    t.walk_apply(&[Cover::FULL], &|_| {}, &mut |_| {});
    let (p, e, changes) = t.new_file(None, "Sheet", ".xlsx", b"xl", &|_| {}).unwrap();
    assert_eq!(p, root.join(format!("Sheet {}.xlsx", bise_proto::words::views::new_nth(2))));
    assert_eq!((fs::read(&p).unwrap(), fs::read(root.join("Sheet.xlsx")).unwrap()), (b"xl".to_vec(), b"his".to_vec()), "never over his file");
    assert_eq!((id(&t, "Sheet 2.xlsx"), changes.len()), (Some(e), 1));
    let q3 = id(&t, "q3").unwrap();
    let (p, e, _) = t.new_file(Some(&q3), "Sheet", ".xlsx", b"", &|_| {}).unwrap();
    assert_eq!((p, id(&t, "q3/Sheet.xlsx")), (root.join("q3/Sheet.xlsx"), Some(e)));
    let a = id(&t, "q3/a.md").unwrap();
    assert_eq!(reason(t.new_file(Some(&a), "Sheet", ".xlsx", b"", &|_| {}).unwrap_err()), "bad_request");
    assert_eq!(reason(t.new_file(Some("e:999999"), "Sheet", ".xlsx", b"", &|_| {}).unwrap_err()), "bad_request");
}
