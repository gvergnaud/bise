//! The diff's table: each case walks a few steps from an empty table (the
//! first step a full walk, ids minted in path order: e:1, e:2, ...), then
//! checks the last step's ids and changes.

use crate::*;

fn f(ino: u64) -> Entry {
    Entry {
        ino,
        kind: Kind::File,
        size: 10,
        mtime_ms: 1000,
    }
}

fn f_at(ino: u64, size: u64, mtime_ms: u64) -> Entry {
    Entry {
        ino,
        kind: Kind::File,
        size,
        mtime_ms,
    }
}

fn d(ino: u64) -> Entry {
    Entry {
        ino,
        kind: Kind::Folder,
        size: 0,
        mtime_ms: 1000,
    }
}

fn dir(rel: &str) -> Cover {
    Cover::Dir(rel.to_string())
}

fn tree(rel: &str) -> Cover {
    Cover::Tree(rel.to_string())
}

struct Step {
    seen: Vec<(&'static str, Entry)>,
    covers: Vec<Cover>,
}

fn full(seen: Vec<(&'static str, Entry)>) -> Step {
    Step {
        seen,
        covers: vec![Cover::FULL],
    }
}

fn part(seen: Vec<(&'static str, Entry)>, covers: Vec<Cover>) -> Step {
    Step { seen, covers }
}

struct Case {
    name: &'static str,
    steps: Vec<Step>,
    /// The table after the last step: path, id.
    ids: Vec<(&'static str, &'static str)>,
    /// The last step's changes, as `Change`'s Display.
    changes: Vec<&'static str>,
}

fn scan_of(seen: &[(&'static str, Entry)]) -> Scan {
    let mut scan = Scan::new();
    for (path, entry) in seen {
        scan.insert(path, *entry).unwrap();
    }
    scan
}

fn run(case: &Case) {
    let mut table = Snapshot::new();
    let mut last = Vec::new();
    for step in &case.steps {
        last = table.apply(&scan_of(&step.seen), &step.covers);
    }
    let ids: Vec<(String, String)> = table
        .iter()
        .map(|(p, id, _)| (p.to_string(), id.to_string()))
        .collect();
    let want: Vec<(String, String)> = case
        .ids
        .iter()
        .map(|(p, id)| (p.to_string(), id.to_string()))
        .collect();
    assert_eq!(ids, want, "{}: ids", case.name);
    let changes: Vec<String> = last.iter().map(Change::to_string).collect();
    assert_eq!(changes, case.changes, "{}: changes", case.name);
    for (path, id, _) in table.iter() {
        assert_eq!(
            table.path_of(id),
            Some(path),
            "{}: path_of({id})",
            case.name
        );
        assert_eq!(table.id_of(path), Some(id), "{}: id_of({path})", case.name);
    }
}

#[test]
fn diff_table() {
    let cases = vec![
        Case {
            name: "a first full walk mints in path order",
            steps: vec![full(vec![("b.md", f(2)), ("a.md", f(1)), ("x", d(3))])],
            ids: vec![("a.md", "e:1"), ("b.md", "e:2"), ("x", "e:3")],
            changes: vec!["added e:1 a.md", "added e:2 b.md", "added e:3 x"],
        },
        Case {
            name: "nothing moved: no change",
            steps: vec![full(vec![("a.md", f(1))]), full(vec![("a.md", f(1))])],
            ids: vec![("a.md", "e:1")],
            changes: vec![],
        },
        Case {
            name: "an in-place write (same ino, size or mtime) is a change",
            steps: vec![full(vec![("a.md", f(1))]), full(vec![("a.md", f_at(1, 11, 2000))])],
            ids: vec![("a.md", "e:1")],
            changes: vec!["changed e:1 a.md"],
        },
        Case {
            name: "a folder's mtime is no change",
            steps: vec![full(vec![("x", d(1))]), full(vec![("x", Entry { mtime_ms: 5000, ..d(1) })])],
            ids: vec![("x", "e:1")],
            changes: vec![],
        },
        Case {
            name: "a rename keeps the id",
            steps: vec![full(vec![("a.md", f(1))]), full(vec![("b.md", f(1))])],
            ids: vec![("b.md", "e:1")],
            changes: vec!["moved e:1 a.md -> b.md"],
        },
        Case {
            name: "a safe save (same path, new ino) keeps the id",
            steps: vec![full(vec![("a.md", f(1))]), full(vec![("a.md", f_at(2, 12, 3000))])],
            ids: vec![("a.md", "e:1")],
            changes: vec!["changed e:1 a.md"],
        },
        Case {
            name: "two saves in one ms (same size and mtime, new ino) still a change",
            steps: vec![full(vec![("a.md", f(1))]), full(vec![("a.md", f(2))])],
            ids: vec![("a.md", "e:1")],
            changes: vec!["changed e:1 a.md"],
        },
        Case {
            name: "rename + new file at the old path in one window: by ino first",
            steps: vec![full(vec![("a.md", f(1))]), full(vec![("a.md", f(2)), ("b.md", f(1))])],
            ids: vec![("a.md", "e:2"), ("b.md", "e:1")],
            changes: vec!["added e:2 a.md", "moved e:1 a.md -> b.md"],
        },
        Case {
            name: "rename + replace in one window: each keeps its id",
            steps: vec![full(vec![("a.md", f(1)), ("c.md", f(3))]), full(vec![("b.md", f(1)), ("c.md", f(4))])],
            ids: vec![("b.md", "e:1"), ("c.md", "e:2")],
            changes: vec!["moved e:1 a.md -> b.md", "changed e:2 c.md"],
        },
        Case {
            name: "a file moved over another: the mover keeps its id, the other is gone",
            steps: vec![full(vec![("a.md", f(1)), ("b.md", f(2))]), full(vec![("b.md", f(1))])],
            ids: vec![("b.md", "e:1")],
            changes: vec!["removed e:2 b.md", "moved e:1 a.md -> b.md"],
        },
        Case {
            name: "a new file gets a new id",
            steps: vec![full(vec![("a.md", f(1))]), full(vec![("a.md", f(1)), ("b.md", f(2))])],
            ids: vec![("a.md", "e:1"), ("b.md", "e:2")],
            changes: vec!["added e:2 b.md"],
        },
        Case {
            name: "a deleted path that comes back after a full walk gets a new id",
            steps: vec![full(vec![("a.md", f(1))]), full(vec![]), full(vec![("a.md", f(1))])],
            ids: vec![("a.md", "e:2")],
            changes: vec!["added e:2 a.md"],
        },
        Case {
            name: "a removed id is never minted again",
            steps: vec![full(vec![("a.md", f(1)), ("b.md", f(2))]), full(vec![("a.md", f(1))]), full(vec![("a.md", f(1)), ("c.md", f(3))])],
            ids: vec![("a.md", "e:1"), ("c.md", "e:3")],
            changes: vec!["added e:3 c.md"],
        },
        Case {
            name: "a kind change at one path: a new id",
            steps: vec![full(vec![("a", f(1))]), full(vec![("a", d(2))])],
            ids: vec![("a", "e:2")],
            changes: vec!["removed e:1 a", "added e:2 a"],
        },
        Case {
            name: "a shallow rescan leaves the folders it did not list",
            steps: vec![
                full(vec![("x", d(1)), ("x/a.md", f(2)), ("y", d(3)), ("y/b.md", f(4))]),
                part(vec![], vec![dir("x")]),
            ],
            ids: vec![("x", "e:1"), ("y", "e:3"), ("y/b.md", "e:4")],
            changes: vec!["removed e:2 x/a.md"],
        },
        Case {
            name: "a shallow rescan leaves the deeper rows",
            steps: vec![
                full(vec![("x", d(1)), ("x/a.md", f(2)), ("x/s", d(3)), ("x/s/c.md", f(4))]),
                part(vec![("x/s", d(3))], vec![dir("x")]),
            ],
            ids: vec![("x", "e:1"), ("x/s", "e:3"), ("x/s/c.md", "e:4")],
            changes: vec!["removed e:2 x/a.md"],
        },
        Case {
            name: "a tree rescan removes at any depth",
            steps: vec![
                full(vec![("x", d(1)), ("x/s", d(2)), ("x/s/c.md", f(3)), ("y.md", f(4))]),
                part(vec![("x/s", d(2))], vec![tree("x")]),
            ],
            ids: vec![("x", "e:1"), ("x/s", "e:2"), ("y.md", "e:4")],
            changes: vec!["removed e:3 x/s/c.md"],
        },
        Case {
            name: "a renamed folder keeps its id and carries its rows, from a shallow rescan of its parent",
            steps: vec![
                full(vec![("x", d(1)), ("x/a.md", f(2)), ("x/s", d(3)), ("x/s/c.md", f(4))]),
                part(vec![("z", d(1))], vec![dir("")]),
            ],
            ids: vec![("z", "e:1"), ("z/a.md", "e:2"), ("z/s", "e:3"), ("z/s/c.md", "e:4")],
            changes: vec!["moved e:1 x -> z", "moved e:2 x/a.md -> z/a.md", "moved e:3 x/s -> z/s", "moved e:4 x/s/c.md -> z/s/c.md"],
        },
        Case {
            name: "a moved folder's rows under a cover that lacks them are gone",
            steps: vec![
                full(vec![("x", d(1)), ("x/a.md", f(2)), ("x/b.md", f(3))]),
                part(vec![("z", d(1)), ("z/a.md", f(2))], vec![dir(""), dir("z")]),
            ],
            ids: vec![("z", "e:1"), ("z/a.md", "e:2")],
            changes: vec!["removed e:3 x/b.md", "moved e:1 x -> z", "moved e:2 x/a.md -> z/a.md"],
        },
        Case {
            name: "a deleted folder takes its rows, from a shallow rescan of its parent",
            steps: vec![
                full(vec![("x", d(1)), ("x/a.md", f(2)), ("x/s", d(3)), ("x/s/c.md", f(4)), ("y.md", f(5))]),
                part(vec![("y.md", f(5))], vec![dir("")]),
            ],
            ids: vec![("y.md", "e:5")],
            changes: vec!["removed e:1 x", "removed e:2 x/a.md", "removed e:3 x/s", "removed e:4 x/s/c.md"],
        },
        Case {
            name: "a folder moved over an unlisted one: the old one and its rows are gone",
            steps: vec![
                full(vec![("x", d(1)), ("x/a.md", f(2)), ("z", d(3)), ("z/c.md", f(4))]),
                part(vec![("z", d(1))], vec![dir("")]),
            ],
            ids: vec![("z", "e:1"), ("z/a.md", "e:2")],
            changes: vec!["removed e:3 z", "removed e:4 z/c.md", "moved e:1 x -> z", "moved e:2 x/a.md -> z/a.md"],
        },
        Case {
            name: "a move between two listed folders",
            steps: vec![
                full(vec![("x", d(1)), ("x/a.md", f(2)), ("y", d(3))]),
                part(vec![("y/a.md", f(2))], vec![dir("x"), dir("y")]),
            ],
            ids: vec![("x", "e:1"), ("y", "e:3"), ("y/a.md", "e:2")],
            changes: vec!["moved e:2 x/a.md -> y/a.md"],
        },
        Case {
            name: "a move seen only from its destination keeps its id",
            steps: vec![
                full(vec![("x", d(1)), ("x/a.md", f(2)), ("y", d(3))]),
                part(vec![("y/a.md", f(2))], vec![dir("y")]),
            ],
            ids: vec![("x", "e:1"), ("y", "e:3"), ("y/a.md", "e:2")],
            changes: vec!["moved e:2 x/a.md -> y/a.md"],
        },
        Case {
            name: "a hard link: both paths keep their ids",
            steps: vec![full(vec![("a.md", f(1)), ("b.md", f(1))]), full(vec![("a.md", f(1)), ("b.md", f(1))])],
            ids: vec![("a.md", "e:1"), ("b.md", "e:2")],
            changes: vec![],
        },
        Case {
            name: "a new hard link gets a new id, never its source's",
            steps: vec![full(vec![("a.md", f(1))]), full(vec![("a.md", f(1)), ("b.md", f(1))])],
            ids: vec![("a.md", "e:1"), ("b.md", "e:2")],
            changes: vec!["added e:2 b.md"],
        },
        Case {
            name: "a hard link moved: it keeps its own id, the other path its own",
            steps: vec![full(vec![("a.md", f(1)), ("b.md", f(1))]), full(vec![("b.md", f(1)), ("c.md", f(1))])],
            ids: vec![("b.md", "e:2"), ("c.md", "e:1")],
            changes: vec!["moved e:1 a.md -> c.md"],
        },
        Case {
            name: "paths are bytes: NFC and NFD of one name are two names",
            steps: vec![
                full(vec![("caf\u{e9}.md", f(1))]),
                full(vec![("caf\u{e9}.md", f(1)), ("cafe\u{301}.md", f(2))]),
            ],
            ids: vec![("cafe\u{301}.md", "e:2"), ("caf\u{e9}.md", "e:1")],
            changes: vec!["added e:2 cafe\u{301}.md"],
        },
    ];
    for case in &cases {
        run(case);
    }
}

#[test]
fn scan_refuses_untracked_paths() {
    let mut scan = Scan::new();
    for rel in [
        "",
        ".DS_Store",
        "x/.a.md.bise-1-2",
        "a.md~",
        "x/../a.md",
        "/a.md",
    ] {
        assert_eq!(
            scan.insert(rel, f(1)),
            Err(Untracked(rel.to_string())),
            "{rel:?}"
        );
    }
    assert!(scan.is_empty());
}

#[test]
fn mint_table() {
    let mut table = Snapshot::new();
    table.apply(&scan_of(&[("x", d(1)), ("x/a.md", f(2))]), &[Cover::FULL]);

    // already seen: its id, no change
    assert_eq!(table.mint("x/a.md", f(2)), Ok((Id::new(2), vec![])));
    // a create the watcher has not seen: a new id, once
    let (id, changes) = table.mint("x/b.md", f(3)).unwrap();
    assert_eq!((id.to_string(), changes.len()), ("e:3".to_string(), 1));
    assert_eq!(table.mint("x/b.md", f(3)), Ok((id, vec![])));
    // a path that took a known ino: that ino's id, moved
    let (id, changes) = table.mint("x/c.md", f(2)).unwrap();
    assert_eq!(id, Id::new(2));
    assert_eq!(
        changes.iter().map(Change::to_string).collect::<Vec<_>>(),
        ["moved e:2 x/a.md -> x/c.md"]
    );
    // mint removes nothing it was not told about
    assert_eq!(table.len(), 3);
    // refused
    for rel in ["", "x/.tmp", "x/a.md~", "../a.md", "x//a.md"] {
        assert_eq!(
            table.mint(rel, f(9)),
            Err(Untracked(rel.to_string())),
            "{rel:?}"
        );
    }
    assert_eq!(table.len(), 3);
}

#[test]
fn children_lists_one_level() {
    let mut table = Snapshot::new();
    table.apply(
        &scan_of(&[
            ("a.md", f(1)),
            ("x", d(2)),
            ("x/b.md", f(3)),
            ("x/s", d(4)),
            ("x/s/c.md", f(5)),
            ("x b.md", f(6)),
            ("xy", d(7)),
        ]),
        &[Cover::FULL],
    );
    let names = |dir: &str| {
        table
            .children(dir)
            .map(|(p, _, _)| p.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(""), ["a.md", "x", "x b.md", "xy"]);
    assert_eq!(names("x"), ["x/b.md", "x/s"]);
    assert_eq!(names("x/s"), ["x/s/c.md"]);
    assert!(names("a.md").is_empty());
    assert!(names("nope").is_empty());
}

#[test]
fn persisted_round_trip() {
    let mut table = Snapshot::new();
    table.apply(
        &scan_of(&[("x", d(1)), ("x/a.md", f(2)), ("b.md", f(3))]),
        &[Cover::FULL],
    );
    table.apply(&scan_of(&[("x", d(1)), ("x/a.md", f(2))]), &[Cover::FULL]);
    let json = serde_json::to_string(&table).unwrap();
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back, table);
    // the next id survives a restart: e:3 (b.md) is never handed out again
    let mut back = back;
    let (id, _) = back.mint("c.md", f(4)).unwrap();
    assert_eq!(id.to_string(), "e:4");
}

#[test]
fn persisted_load_refuses_a_broken_table() {
    let row = |path: &str, id: &str| {
        format!(r#"{{"path":"{path}","id":"{id}","ino":1,"kind":"file","size":0,"mtime_ms":0}}"#)
    };
    let rows: &[(&str, String)] = &[
        (
            "an id twice",
            format!(
                r#"{{"next":3,"entries":[{},{}]}}"#,
                row("a.md", "e:1"),
                row("b.md", "e:1")
            ),
        ),
        (
            "a path twice",
            format!(
                r#"{{"next":3,"entries":[{},{}]}}"#,
                row("a.md", "e:1"),
                row("a.md", "e:2")
            ),
        ),
        (
            "an id not below next",
            format!(r#"{{"next":2,"entries":[{}]}}"#, row("a.md", "e:2")),
        ),
        (
            "an untracked path",
            format!(r#"{{"next":2,"entries":[{}]}}"#, row(".a.md", "e:1")),
        ),
        ("next 0", r#"{"next":0,"entries":[]}"#.to_string()),
        (
            "a bad id",
            format!(r#"{{"next":2,"entries":[{}]}}"#, row("a.md", "d:1")),
        ),
    ];
    for (name, json) in rows {
        assert!(serde_json::from_str::<Snapshot>(json).is_err(), "{name}");
    }
}

#[test]
fn cover_table() {
    let rows: &[(Cover, &str, bool)] = &[
        (Cover::FULL, "a.md", true),
        (Cover::FULL, "x/s/c.md", true),
        (dir(""), "a.md", true),
        (dir(""), "x/a.md", false),
        (dir("x"), "x/a.md", true),
        (dir("x"), "x/s/c.md", false),
        (dir("x"), "x", false),
        (dir("x"), "xy/a.md", false),
        (tree("x"), "x", false),
        (tree("x"), "x/a.md", true),
        (tree("x"), "x/s/c.md", true),
        (tree("x"), "xy/a.md", false),
        (tree("x"), "x b/a.md", false),
    ];
    for (cover, rel, want) in rows {
        assert_eq!(cover.covers(rel), *want, "{cover:?} covers {rel:?}");
    }
}
