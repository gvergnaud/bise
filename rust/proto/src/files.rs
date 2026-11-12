//! The window's file methods on `hub.sock` (docs/plugin-apps.md A4b, A6;
//! architect m_59962 (b)): `folder/list` (a folder of `~/bise`, its rows)
//! and `file/new` (New ›: a document made from a plugin's `makes`). They
//! are the window's own methods, never a view's: a view reads and writes
//! its one file through `view/call` (`resources/read`,
//! `ai.mistral.vibe/resources/write`, [`crate::view_rpc`]).
//!
//! A row's `id` is the home hub's stable id of that file (`e:<n>`, the
//! `bise_ids` map, architect m_60046): kept across a rename, a safe save,
//! a restart. The window keeps it and sends it back as is; it never
//! builds, splits or reads one, and never sends a path. An unknown id is
//! refused.

use serde::{Deserialize, Serialize};

/// A row of `~/bise`: a file or a folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    #[default]
    File,
    Folder,
}

/// The artifact a row is the current target of (contract v4, one document
/// one write path): the window opens it as that artifact, one tab for the
/// card and the row.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EntryArtifact {
    pub project: String,
    pub id: String,
}

/// One row of `folder/list`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FolderRow {
    /// the hub's id of this entry (opaque to the window)
    pub id: String,
    pub name: String,
    pub kind: FileKind,
    /// bytes (a folder: 0)
    pub size: u64,
    /// its last change, ms since the epoch
    pub mtime: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub artifact: Option<EntryArtifact>,
}

/// A folder on the way from the root to the listed one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Crumb {
    pub id: String,
    pub name: String,
}

/// `folder/list`'s answer: the folder listed (the root: its id, an empty
/// name, the window says `~/bise`), its ancestors (the root first, the
/// folder's parent last; none for the root), and its rows (folders first,
/// then by name).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FolderList {
    pub folder: Crumb,
    pub crumb: Vec<Crumb>,
    pub rows: Vec<FolderRow>,
}

/// What `file/new` makes: one of a plugin's `makes` (A1), by its ending.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Make {
    pub plugin: String,
    pub ending: String,
}

/// `file/new`'s answer: the artifact it added (by `you`) and its row's id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FileNewResult {
    pub artifact: String,
    pub entry: String,
}

/// What `view/open` opens (A4; architect m_60316): an artifact (its newest
/// version when `version` is none) or a row `folder/list` gave. The one
/// wire type: a path string, a missing `kind` or an unknown field is
/// refused; a kind this version doesn't know reads as `Unknown`, which the
/// hub refuses as a bad request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OpenTarget {
    Artifact {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        version: Option<u64>,
    },
    Entry {
        id: String,
    },
    #[serde(other)]
    #[cfg_attr(feature = "ts", ts(skip))]
    Unknown,
}

/// One row of the home hub's id table (`files/ids`, architect m_60046,
/// m_60075; kb-fileprovider m_60060): a tracked path of `~/bise`, the
/// exact bytes on disk relative to the root, `/`-joined, never
/// Unicode-normalised.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IdEntry {
    /// `e:<n>`
    pub id: String,
    pub path: String,
    pub kind: FileKind,
    pub size: u64,
    pub mtime_ms: u64,
    pub ino: u64,
}

/// `files/ids/subscribe`'s answer and every `files_ids` event after it,
/// from the home hub only: at subscribe the whole table (`full`), then its
/// changes. `seq` grows by one per event: a gap means subscribe again.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FilesIds {
    pub full: bool,
    pub seq: u64,
    /// the canonical `Home::bise_root()` (in the full answer only)
    #[serde(default)]
    pub root: String,
    /// `full`: every entry; else the new, moved and changed ones
    #[serde(default)]
    pub entries: Vec<IdEntry>,
    /// the ids gone (never in a `full` one)
    #[serde(default)]
    pub deleted: Vec<String>,
}

/// `files/mint`'s answer: the path's id (the one it has, else a new one,
/// its `files_ids` delta sent to every subscriber).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FilesMintResult {
    pub id: String,
}

/// `view/copy`'s answer (A6b Keep mine as a copy, architect m_60222): the
/// artifact it added.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ViewCopyResult {
    pub artifact: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_row_reads_as_the_contract_says() {
        let r = FolderRow { id: "e:7".into(), name: "a.xlsx".into(), kind: FileKind::File, size: 3, mtime: 9, artifact: Some(EntryArtifact { project: "p".into(), id: "q3".into() }) };
        assert_eq!(serde_json::to_value(&r).unwrap(), json!({"id": "e:7", "name": "a.xlsx", "kind": "file", "size": 3, "mtime": 9, "artifact": {"project": "p", "id": "q3"}}));
        let folder: FolderRow = serde_json::from_value(json!({"id": "e:1", "name": "x", "kind": "folder", "size": 0, "mtime": 0})).unwrap();
        assert_eq!((folder.kind, folder.artifact), (FileKind::Folder, None));
    }

    /// architect m_60316: one tagged type; a path is never a target.
    #[test]
    fn an_open_target_is_an_artifact_or_an_entry_never_a_path() {
        let read = |v: serde_json::Value| serde_json::from_value::<OpenTarget>(v);
        assert_eq!(read(json!({"kind": "artifact", "id": "q3"})).unwrap(), OpenTarget::Artifact { id: "q3".into(), version: None });
        assert_eq!(read(json!({"kind": "artifact", "id": "q3", "version": 2})).unwrap(), OpenTarget::Artifact { id: "q3".into(), version: Some(2) });
        assert_eq!(read(json!({"kind": "entry", "id": "e:7"})).unwrap(), OpenTarget::Entry { id: "e:7".into() });
        assert_eq!(read(json!({"kind": "shared", "mount": "m"})).unwrap(), OpenTarget::Unknown, "a newer kind: refused by the hub");
        for bad in [json!("/Users/x/bise/a.md"), json!({"id": "e:7"}), json!({"kind": "entry", "id": "e:7", "path": "/etc/passwd"}), json!({"kind": "artifact"})] {
            assert!(read(bad.clone()).is_err(), "{bad}");
        }
        assert_eq!(serde_json::to_value(OpenTarget::Entry { id: "e:7".into() }).unwrap(), json!({"kind": "entry", "id": "e:7"}));
    }

    #[test]
    fn the_id_table_says_its_changes() {
        let delta = FilesIds { full: false, seq: 4, deleted: vec!["e:2".into()], ..Default::default() };
        assert_eq!(serde_json::to_value(&delta).unwrap(), json!({"full": false, "seq": 4, "root": "", "entries": [], "deleted": ["e:2"]}));
        let e = IdEntry { id: "e:9".into(), path: "Boards/été.board.json".into(), kind: FileKind::File, size: 12, mtime_ms: 5, ino: 77 };
        assert_eq!(serde_json::to_value(&e).unwrap(), json!({"id": "e:9", "path": "Boards/été.board.json", "kind": "file", "size": 12, "mtime_ms": 5, "ino": 77}));
    }
}
