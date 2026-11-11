//! Stable ids for the files and folders under `~/bise` (architect m_60046,
//! m_60052): a file keeps its id across a save, a rename and a move, so a
//! tab, a folder row and a Finder item keep pointing at it.
//!
//! Pure: no file, no clock, no watcher. The home hub owns the one table
//! (its FSEvents watcher walks, calls [`Snapshot::apply`], persists the
//! table through a `Home` fn and sends the [`Change`]s); the desktop
//! core's Finder edge keeps a copy from those changes and never mints.
//!
//! - [`path`]: the one rule for the paths that get ids ([`tracked`],
//!   [`skipped`], [`valid_rel`]);
//! - [`Id`]: `e:<n>` on the wire, never reused;
//! - [`Snapshot`]: the table, both ways ([`Snapshot::id_of`],
//!   [`Snapshot::path_of`], [`Snapshot::children`]), persisted with serde;
//! - [`Snapshot::apply`] (in `diff`): a [`Scan`] and its [`Cover`]s in,
//!   the [`Change`]s out; [`Snapshot::mint`] for `files/mint`.

mod diff;
mod id;
pub mod path;
mod snapshot;

pub use id::{BadId, Id};
pub use path::{skipped, tracked, valid_rel};
pub use snapshot::{Change, Corrupt, Cover, Entry, Kind, Scan, Snapshot, Untracked};

#[cfg(test)]
#[path = "diff_tests.rs"]
mod diff_tests;
