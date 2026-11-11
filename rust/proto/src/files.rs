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
}
