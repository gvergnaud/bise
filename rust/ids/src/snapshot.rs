//! [`Snapshot`]: the id table, `path -> (id, entry)` with its reverse
//! index, what a walk saw ([`Scan`], [`Cover`]) and what changed
//! ([`Change`]). The diff itself is in [`crate::diff`].
//!
//! Persisted as `{next, entries: [{path, id, ino, kind, size, mtime_ms}]}`
//! (serde): the reverse index is rebuilt on load, and a load that breaks a
//! rule (an untracked path, an id twice, an id not below `next`) is
//! refused, never repaired.

use crate::id::Id;
use crate::path::{parent, tracked};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// A file or a folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Folder,
}

/// What a walk read of one path (its `lstat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub ino: u64,
    pub kind: Kind,
    pub size: u64,
    pub mtime_ms: u64,
}

/// A path that gets no id ([`crate::tracked`] said no): what came instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Untracked(pub String);

impl fmt::Display for Untracked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a tracked path under the bise root: {:?}", self.0)
    }
}

impl std::error::Error for Untracked {}

/// What one walk saw: tracked paths and their entries. Paths outside the
/// walk's [`Cover`] are allowed (they are seen, so they count), but only
/// the cover says what is gone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan(pub(crate) BTreeMap<String, Entry>);

impl Scan {
    pub fn new() -> Scan {
        Scan::default()
    }

    /// Records `rel`; refuses a path [`crate::tracked`] refuses.
    pub fn insert(&mut self, rel: &str, entry: Entry) -> Result<(), Untracked> {
        if !tracked(rel) {
            return Err(Untracked(rel.to_string()));
        }
        self.0.insert(rel.to_string(), entry);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// What a walk listed, so what it would have seen: an old entry under a
/// cover that the scan lacks is gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cover {
    /// The direct children of this folder (`""`: the root's): one
    /// incremental rescan of a changed folder.
    Dir(String),
    /// Everything under this folder, at any depth: a new folder's walk,
    /// or MustScanSubDirs.
    Tree(String),
}

impl Cover {
    /// The whole root: a full walk (at start, after dropped events).
    pub const FULL: Cover = Cover::Tree(String::new());

    pub(crate) fn covers(&self, rel: &str) -> bool {
        match self {
            Cover::Dir(dir) => parent(rel) == dir,
            Cover::Tree(dir) if dir.is_empty() => !rel.is_empty(),
            Cover::Tree(dir) => {
                rel.len() > dir.len() + 1
                    && rel.starts_with(dir.as_str())
                    && rel.as_bytes()[dir.len()] == b'/'
            }
        }
    }
}

/// One change to the table, at most one per id per apply. The entry's
/// fresh size, mtime and ino are in the snapshot: read them there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// A path seen for the first time: a new id.
    Added { id: Id, path: String },
    /// Its path changed (a rename, a move, its folder's move); its content
    /// may have changed too.
    Moved { id: Id, from: String, to: String },
    /// A file's content changed at the same path: its size or mtime moved
    /// in place, or a save replaced it (a new ino).
    Changed { id: Id, path: String },
    /// Gone; the id is never handed out again.
    Removed { id: Id, path: String },
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Change::Added { id, path } => write!(f, "added {id} {path}"),
            Change::Moved { id, from, to } => write!(f, "moved {id} {from} -> {to}"),
            Change::Changed { id, path } => write!(f, "changed {id} {path}"),
            Change::Removed { id, path } => write!(f, "removed {id} {path}"),
        }
    }
}

/// One row of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub id: Id,
    pub entry: Entry,
}

/// The id table: every tracked path with its id and entry, both ways.
/// Changed only by [`Snapshot::apply`] and [`Snapshot::mint`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Persisted", into = "Persisted")]
pub struct Snapshot {
    pub(crate) by_path: BTreeMap<String, Row>,
    pub(crate) by_id: BTreeMap<Id, String>,
    /// The number of the next id minted.
    pub(crate) next: u64,
}

impl Default for Snapshot {
    fn default() -> Snapshot {
        Snapshot {
            by_path: BTreeMap::new(),
            by_id: BTreeMap::new(),
            next: 1,
        }
    }
}

impl Snapshot {
    /// An empty table: the first apply is a full walk.
    pub fn new() -> Snapshot {
        Snapshot::default()
    }

    pub fn len(&self) -> usize {
        self.by_path.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    pub fn id_of(&self, rel: &str) -> Option<Id> {
        self.by_path.get(rel).map(|row| row.id)
    }

    pub fn path_of(&self, id: Id) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    pub fn get(&self, rel: &str) -> Option<(Id, Entry)> {
        self.by_path.get(rel).map(|row| (row.id, row.entry))
    }

    /// Every row, in path order (bytes): the full table at connect.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Id, Entry)> {
        self.by_path
            .iter()
            .map(|(path, row)| (path.as_str(), row.id, row.entry))
    }

    /// The direct children of folder `dir` (`""`: the root), in path
    /// order.
    pub fn children<'a>(&'a self, dir: &'a str) -> impl Iterator<Item = (&'a str, Id, Entry)> + 'a {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        self.by_path
            .range(prefix.clone()..)
            .take_while(move |(path, _)| path.starts_with(&prefix))
            .filter(move |(path, _)| parent(path) == dir)
            .map(|(path, row)| (path.as_str(), row.id, row.entry))
    }

    pub(crate) fn from_rows(by_path: BTreeMap<String, Row>, next: u64) -> Snapshot {
        let by_id = by_path
            .iter()
            .map(|(path, row)| (row.id, path.clone()))
            .collect();
        Snapshot {
            by_path,
            by_id,
            next,
        }
    }
}

/// The persisted form: the rows once, no index.
#[derive(Serialize, Deserialize)]
struct Persisted {
    next: u64,
    entries: Vec<PersistedRow>,
}

#[derive(Serialize, Deserialize)]
struct PersistedRow {
    path: String,
    id: Id,
    #[serde(flatten)]
    entry: Entry,
}

impl From<Snapshot> for Persisted {
    fn from(s: Snapshot) -> Persisted {
        let entries = s
            .by_path
            .into_iter()
            .map(|(path, row)| PersistedRow {
                path,
                id: row.id,
                entry: row.entry,
            })
            .collect();
        Persisted {
            next: s.next,
            entries,
        }
    }
}

/// A persisted table that breaks a rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Corrupt(pub String);

impl fmt::Display for Corrupt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "corrupt id table: {}", self.0)
    }
}

impl std::error::Error for Corrupt {}

impl TryFrom<Persisted> for Snapshot {
    type Error = Corrupt;

    fn try_from(p: Persisted) -> Result<Snapshot, Corrupt> {
        if p.next == 0 {
            return Err(Corrupt("next is 0".into()));
        }
        let mut by_path = BTreeMap::new();
        let mut ids = std::collections::BTreeSet::new();
        for row in p.entries {
            if !tracked(&row.path) {
                return Err(Corrupt(format!("untracked path {:?}", row.path)));
            }
            if row.id.number() >= p.next {
                return Err(Corrupt(format!("{} not below next {}", row.id, p.next)));
            }
            if !ids.insert(row.id) {
                return Err(Corrupt(format!("{} twice", row.id)));
            }
            if by_path
                .insert(
                    row.path.clone(),
                    Row {
                        id: row.id,
                        entry: row.entry,
                    },
                )
                .is_some()
            {
                return Err(Corrupt(format!("path {:?} twice", row.path)));
            }
        }
        Ok(Snapshot::from_rows(by_path, p.next))
    }
}
