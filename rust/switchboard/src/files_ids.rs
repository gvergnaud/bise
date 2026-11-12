//! The home hub's id table of `~/bise`, its pure parts (docs/plugin-apps.md
//! A6 ids; architect m_60046, m_60075, m_60095): what a batch of watcher
//! events makes the table walk ([`covers`]), the wire of the table
//! ([`full`], [`delta`]), a watched path as a table path ([`rel_of`]) and
//! the load of the persisted table ([`load`]). The table itself and its
//! diff are `bise_ids`; the thread, the walk and the watcher are the
//! shell's (`daemon/files_ids.rs`).
//!
//! Paths are exact bytes relative to `Home::bise_root()`, `/`-joined,
//! never Unicode-normalised (architect m_60074).

use bise_ids::{tracked, Change, Cover, Entry, Id, Kind, Snapshot};
use bise_proto::files::{FileKind, FilesIds, IdEntry};
use std::path::{Component, Path};

/// The most entries one walk reads: past it the walk stops, and only the
/// folders it listed whole say what is gone.
pub const WALK_MAX: usize = 20_000;

/// `path` (under the canonical root `root`) as a table path: `""` for the
/// root, else its names joined by `/`. Outside the root, a `..`, or a name
/// that is not UTF-8: none.
pub fn rel_of(root: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    let mut names = Vec::new();
    for c in rest.components() {
        match c {
            Component::Normal(n) => names.push(n.to_str()?),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(names.join("/"))
}

/// The folder that holds `rel` (`""` for a name at the root).
pub fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// What one batch of watcher events makes the table read again: each
/// changed path's folder ([`Cover::Dir`]), and the path's whole tree when
/// it is a folder now ([`Cover::Tree`]: a folder moved in, or
/// MustScanSubDirs). `seen` holds `(path, folder now)`; `rescan` (dropped
/// events, the watcher's own error) is the whole root. A path no id is
/// kept for (hidden, an editor's temp, under one) changes nothing, but its
/// folder is read: its rename to a tracked name comes as its own event.
/// A cover inside another is dropped; the order is stable (sorted).
pub fn covers(seen: &[(String, bool)], rescan: bool) -> Vec<Cover> {
    if rescan {
        return vec![Cover::FULL];
    }
    let mut dirs: Vec<String> = Vec::new();
    let mut trees: Vec<String> = Vec::new();
    for (rel, folder) in seen {
        if rel.is_empty() {
            dirs.push(String::new());
            continue;
        }
        let dir = parent(rel);
        if !(dir.is_empty() || tracked(dir)) {
            continue;
        }
        dirs.push(dir.to_string());
        if *folder && tracked(rel) {
            trees.push(rel.clone());
        }
    }
    trees.sort();
    trees.dedup();
    let trees: Vec<String> = trees.iter().filter(|t| !trees.iter().any(|o| under(t, o))).cloned().collect();
    dirs.sort();
    dirs.dedup();
    dirs.retain(|d| !trees.iter().any(|t| d == t || under(d, t)));
    dirs.into_iter().map(Cover::Dir).chain(trees.into_iter().map(Cover::Tree)).collect()
}

/// `rel` is strictly below folder `dir`.
fn under(rel: &str, dir: &str) -> bool {
    rel.len() > dir.len() + 1 && rel.starts_with(dir) && rel.as_bytes()[dir.len()] == b'/'
}

/// One table row on the wire.
pub fn id_entry(path: &str, id: Id, e: Entry) -> IdEntry {
    let kind = match e.kind {
        Kind::File => FileKind::File,
        Kind::Folder => FileKind::Folder,
    };
    IdEntry { id: id.to_string(), path: path.to_string(), kind, size: e.size, mtime_ms: e.mtime_ms, ino: e.ino }
}

/// `files/ids/subscribe`'s answer: the whole table at `seq`, with the
/// canonical root. A client drops a later delta whose `seq` is not past
/// this one's.
pub fn full(snap: &Snapshot, seq: u64, root: &str) -> FilesIds {
    let entries = snap.iter().map(|(p, id, e)| id_entry(p, id, e)).collect();
    FilesIds { full: true, seq, root: root.to_string(), entries, deleted: Vec::new() }
}

/// The `files_ids` event of one apply or mint, read from the table after
/// it: the added, moved and changed rows as upserts, the removed ids.
/// Nothing changed: none (no event, `seq` stays).
pub fn delta(snap: &Snapshot, changes: &[Change], seq: u64) -> Option<FilesIds> {
    if changes.is_empty() {
        return None;
    }
    let mut entries = Vec::new();
    let mut deleted = Vec::new();
    for c in changes {
        let path = match c {
            Change::Added { path, .. } | Change::Changed { path, .. } => path,
            Change::Moved { to, .. } => to,
            Change::Removed { id, .. } => {
                deleted.push(id.to_string());
                continue;
            }
        };
        if let Some((id, e)) = snap.get(path) {
            entries.push(id_entry(path, id, e));
        }
    }
    Some(FilesIds { full: false, seq, root: String::new(), entries, deleted })
}

/// The persisted table (`Home::files_ids_file`'s text; none: no file
/// yet). A table that does not parse or breaks a rule is refused with its
/// reason: the shell moves the file aside and starts empty, never repairs.
pub fn load(text: Option<&str>) -> Result<Snapshot, String> {
    match text {
        None => Ok(Snapshot::new()),
        Some(t) => serde_json::from_str(t).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bise_ids::Scan;
    use std::path::PathBuf;

    fn file(ino: u64, size: u64) -> Entry {
        Entry { dev: 1, ino, kind: Kind::File, size, mtime_ms: 5 }
    }

    #[test]
    fn a_watched_path_is_its_exact_names_under_the_root() {
        let root = PathBuf::from("/Users/g/bise");
        assert_eq!(rel_of(&root, &root).as_deref(), Some(""));
        assert_eq!(rel_of(&root, &root.join("notes/a.md")).as_deref(), Some("notes/a.md"));
        // never normalised: e + combining accent stays two chars
        assert_eq!(rel_of(&root, &root.join("cafe\u{301}.md")).as_deref(), Some("cafe\u{301}.md"));
        assert_eq!(rel_of(&root, Path::new("/Users/g/bisex/a.md")), None);
        assert_eq!(rel_of(&root, Path::new("/Users/g/other")), None);
    }

    #[test]
    fn a_batch_reads_each_changed_folder_once_and_a_new_folder_whole() {
        let seen = |v: &[(&str, bool)]| v.iter().map(|(p, f)| (p.to_string(), *f)).collect::<Vec<_>>();
        let dir = |d: &str| Cover::Dir(d.into());
        let tree = |d: &str| Cover::Tree(d.into());
        assert_eq!(covers(&seen(&[("a.md", false), ("b.md", false)]), false), vec![dir("")]);
        assert_eq!(covers(&seen(&[("x/a.md", false), ("y/b.md", false)]), false), vec![dir("x"), dir("y")]);
        // a folder moved in: its parent listed, itself walked; a change
        // inside it adds nothing more
        assert_eq!(covers(&seen(&[("x", true), ("x/a.md", false), ("x/d/b.md", false)]), false), vec![dir(""), tree("x")]);
        // hidden and temp names: their folder is read, nothing below them
        assert_eq!(covers(&seen(&[("x/.a.md.bise-1-2", false), (".git/config", false), (".cache", true)]), false), vec![dir(""), dir("x")]);
        assert_eq!(covers(&seen(&[("", true)]), false), vec![dir("")]);
        assert_eq!(covers(&seen(&[("x/a.md", false)]), true), vec![Cover::FULL]);
        assert_eq!(covers(&[], false), Vec::<Cover>::new());
    }

    #[test]
    fn the_wire_is_upserts_and_removed_ids_read_from_the_table() {
        let mut snap = Snapshot::new();
        let mut scan = Scan::new();
        scan.insert("a.md", file(10, 1)).unwrap();
        scan.insert("b.md", file(11, 1)).unwrap();
        let changes = snap.apply(&scan, &[Cover::FULL]);
        let d = delta(&snap, &changes, 1).unwrap();
        assert_eq!((d.full, d.seq, d.entries.len(), d.deleted.len()), (false, 1, 2, 0));
        assert_eq!((d.entries[0].id.as_str(), d.entries[0].path.as_str(), d.entries[0].ino), ("e:1", "a.md", 10));

        // a.md renamed to c.md (same ino), b.md gone
        let mut scan = Scan::new();
        scan.insert("c.md", file(10, 2)).unwrap();
        let changes = snap.apply(&scan, &[Cover::FULL]);
        let d = delta(&snap, &changes, 2).unwrap();
        assert_eq!(d.deleted, vec!["e:2".to_string()]);
        assert_eq!(d.entries.iter().map(|e| (e.id.as_str(), e.path.as_str(), e.size)).collect::<Vec<_>>(), vec![("e:1", "c.md", 2)]);
        assert_eq!(delta(&snap, &[], 3), None, "nothing changed: no event");

        let f = full(&snap, 2, "/Users/g/bise");
        assert_eq!((f.full, f.seq, f.root.as_str(), f.entries.len()), (true, 2, "/Users/g/bise", 1));
    }

    #[test]
    fn a_table_that_breaks_a_rule_is_refused_never_repaired() {
        assert!(load(None).unwrap().is_empty());
        let mut snap = Snapshot::new();
        snap.mint("a.md", file(1, 1)).unwrap();
        let text = serde_json::to_string(&snap).unwrap();
        assert_eq!(load(Some(&text)).unwrap(), snap);
        assert!(load(Some("{")).is_err());
        assert!(load(Some(r#"{"next":1,"entries":[{"path":".x","id":"e:0","dev":1,"ino":1,"kind":"file","size":0,"mtime_ms":0}]}"#)).is_err());
    }
}
