//! The diff: an old [`Snapshot`], a [`Scan`] and its [`Cover`]s in, the new
//! table and its [`Change`]s out (architect m_60046).
//!
//! Each seen path takes an old row, in four passes, each over the paths
//! the passes before left:
//!
//! 1. the same path and the same ino: unchanged, or [`Change::Changed`]
//!    when a file's size or mtime moved in place;
//! 2. the same ino at another path, looked up in the whole table (a move
//!    seen only from its destination still keeps its id): a rename or a
//!    move, [`Change::Moved`];
//! 3. the same path with a new ino (a safe save replaced the file, or a
//!    folder came back): the id is kept, [`Change::Changed`] for a file;
//! 4. else a new id, [`Change::Added`].
//!
//! So a rename and a replace in one window keep the ids by ino first, then
//! by path. Two paths of one ino (a hard link): pass 1 keeps both; a new
//! link finds the ino taken and gets a new id, so a move never takes
//! another path's id. Kinds must match in every pass, and "the same ino"
//! means the same `(dev, ino)`: a mount inside the root can repeat an ino
//! of another device (architect m_60095).
//!
//! An old row nothing took stays where its folder went: a folder that
//! moved carries the rows under it ([`Change::Moved`] for each, ids kept),
//! even when the walk listed only the folder's parent. It is removed
//! ([`Change::Removed`]) when the cover lists where it would be, when its
//! folder is removed, or when a seen path holds that place. A removed id is
//! never minted again: a path that comes back after its removal gets a
//! new one.

use crate::id::Id;
use crate::path::{parent, rebase, tracked};
use crate::snapshot::{Change, Cover, Entry, Kind, Row, Scan, Snapshot, Untracked};
use std::collections::{BTreeMap, BTreeSet, HashMap};

impl Snapshot {
    /// Takes in what a walk saw: `scan` holds the paths it read, `covers`
    /// what it listed (an old row under a cover that `scan` lacks is gone;
    /// `&[Cover::FULL]` after a full walk). All the folders of one event
    /// batch go in one apply, so a move between two of them is one move.
    ///
    /// The changes come removals first (old path order), then the rest
    /// (new path order).
    pub fn apply(&mut self, scan: &Scan, covers: &[Cover]) -> Vec<Change> {
        let old = std::mem::take(&mut self.by_path);
        let mut next = self.next;
        let mut taken: BTreeMap<&str, String> = BTreeMap::new(); // old path -> new path
        let mut placed: BTreeMap<String, (Row, Origin)> = BTreeMap::new();
        let mut left: Vec<(&str, Entry)> = Vec::new();

        // 1. same path, same ino
        for (path, entry) in &scan.0 {
            match old.get(path) {
                Some(row) if same_file(&row.entry, entry) => {
                    taken.insert(path, path.clone());
                    placed.insert(
                        path.clone(),
                        (
                            Row {
                                id: row.id,
                                entry: *entry,
                            },
                            Origin::Same(row.entry),
                        ),
                    );
                }
                _ => left.push((path, *entry)),
            }
        }

        // 2. same ino, another path
        let mut by_ino: HashMap<(u64, u64, Kind), Vec<&str>> = HashMap::new();
        for (path, row) in &old {
            if !taken.contains_key(path.as_str()) {
                by_ino.entry(file_key(&row.entry)).or_default().push(path);
            }
        }
        left.retain(|(path, entry)| {
            let Some(from) = by_ino
                .get_mut(&file_key(entry))
                .and_then(|paths| (!paths.is_empty()).then(|| paths.remove(0)))
            else {
                return true;
            };
            taken.insert(from, path.to_string());
            placed.insert(
                path.to_string(),
                (
                    Row {
                        id: old[from].id,
                        entry: *entry,
                    },
                    Origin::From(from.to_string()),
                ),
            );
            false
        });

        // where each old row not taken now is: under its folder's new place
        let mut now: BTreeMap<&str, String> = BTreeMap::new(); // old path -> its place now
        for path in old.keys() {
            let place = match taken.get(path.as_str()) {
                Some(to) => to.clone(),
                None => match now.get(parent(path)) {
                    Some(dir_now) => rebase(path, parent(path), dir_now),
                    None => path.clone(),
                },
            };
            now.insert(path, place);
        }
        let mut at_place: HashMap<&str, &str> = HashMap::new(); // place now -> old path, rows not taken
        for (path, place) in &now {
            if !taken.contains_key(path) {
                at_place.entry(place.as_str()).or_insert(path);
            }
        }

        // 3. same path, a new ino
        // 4. a new id
        for (path, entry) in left {
            let same_place = at_place
                .get(path)
                .copied()
                .filter(|from| old[*from].entry.kind == entry.kind && !taken.contains_key(from));
            let (id, origin) = match same_place {
                Some(from) => {
                    taken.insert(from, path.to_string());
                    (old[from].id, Origin::From(from.to_string()))
                }
                None => {
                    let id = Id::new(next);
                    next += 1;
                    (id, Origin::New)
                }
            };
            placed.insert(path.to_string(), (Row { id, entry }, origin));
        }

        // the rows nothing took: kept at their place, or removed
        let mut changes = Vec::new();
        let mut removed: BTreeSet<&str> = BTreeSet::new();
        let mut kept: BTreeMap<String, Row> = BTreeMap::new();
        let mut moved_along: Vec<(String, Change)> = Vec::new();
        for (path, row) in &old {
            if taken.contains_key(path.as_str()) {
                continue;
            }
            let place = &now[path.as_str()];
            let gone = removed.contains(parent(path))
                || covers.iter().any(|c| c.covers(place))
                || placed.contains_key(place)
                || kept.contains_key(place);
            if gone {
                removed.insert(path);
                changes.push(Change::Removed {
                    id: row.id,
                    path: path.clone(),
                });
                continue;
            }
            if place != path {
                moved_along.push((
                    place.clone(),
                    Change::Moved {
                        id: row.id,
                        from: path.clone(),
                        to: place.clone(),
                    },
                ));
            }
            kept.insert(place.clone(), *row);
        }

        let mut rest: Vec<(String, Change)> = moved_along;
        for (path, (row, origin)) in &placed {
            let change = match origin {
                Origin::New => Some(Change::Added {
                    id: row.id,
                    path: path.clone(),
                }),
                Origin::From(from) if from != path => Some(Change::Moved {
                    id: row.id,
                    from: from.clone(),
                    to: path.clone(),
                }),
                Origin::From(_) => file_changed(path, row, None),
                Origin::Same(before) => file_changed(path, row, Some(before)),
            };
            rest.extend(change.map(|c| (path.clone(), c)));
        }
        rest.sort_by(|a, b| a.0.cmp(&b.0));
        changes.extend(rest.into_iter().map(|(_, c)| c));

        let mut by_path = kept;
        by_path.extend(placed.into_iter().map(|(path, (row, _))| (path, row)));
        *self = Snapshot::from_rows(by_path, next);
        changes
    }

    /// The id of `rel`, minted now if the table has none (`files/mint`,
    /// right after a create the watcher may not have seen yet): an apply
    /// of that one entry with no cover, so it is idempotent, and a path
    /// that took a known ino keeps that ino's id. The changes go out like
    /// an apply's.
    pub fn mint(&mut self, rel: &str, entry: Entry) -> Result<(Id, Vec<Change>), Untracked> {
        if !tracked(rel) {
            return Err(Untracked(rel.to_string()));
        }
        let mut scan = Scan::new();
        scan.insert(rel, entry)?;
        let changes = self.apply(&scan, &[]);
        let id = self.id_of(rel).expect("an applied path has an id");
        Ok((id, changes))
    }
}

/// Where a seen path's row came from.
enum Origin {
    /// Pass 1: this path, this ino; its entry before.
    Same(Entry),
    /// Pass 2 or 3: the old row at that path.
    From(String),
    /// Pass 4.
    New,
}

/// One file on disk: its device, its ino, its kind.
fn file_key(e: &Entry) -> (u64, u64, Kind) {
    (e.dev, e.ino, e.kind)
}

fn same_file(a: &Entry, b: &Entry) -> bool {
    file_key(a) == file_key(b)
}

/// [`Change::Changed`] for a file whose content moved: a new ino
/// (`before` is `None`), or its size or mtime.
fn file_changed(path: &str, row: &Row, before: Option<&Entry>) -> Option<Change> {
    let moved = match before {
        None => true,
        Some(b) => b.size != row.entry.size || b.mtime_ms != row.entry.mtime_ms,
    };
    (row.entry.kind == Kind::File && moved).then(|| Change::Changed {
        id: row.id,
        path: path.to_string(),
    })
}
