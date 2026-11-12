//! The `artifacts` event's rows, built off the hub's loop (hub-loop
//! m_61105, architect m_61130).
//!
//! Why: on his hub (1298 artifacts, 251 pages, load 50-85) one build of
//! the rows read every meta.json three times, a realpath per version and
//! a stat per target: ~5 s, run by the state push on the loop about every
//! 10 s, so his messages waited minutes behind it.
//!
//! Now the loop only asks ([`ArtRows::ask`]): it gets the last view built
//! at once, and a key that moved starts one build on the `art-rows`
//! thread ([`ArtJob::build`], what it reads copied in). Its view comes
//! back as `Msg::ArtBuilt` ([`ArtRows::built`]), and the list goes to the
//! clients then. One build at a time: a key that moves during a build
//! starts one more when it lands, never a queue. Before the first build
//! lands there is no view: no `artifacts` event yet (the clients keep
//! their loading state).
//!
//! The key ([`Shell::art_key`]) holds what the rows read: the stores'
//! disk generation (crate::store_gen::disk_now, not the pages' busy set),
//! the agents' names, the folders' stamps, and a 5 min backstop for what
//! changes outside the hub with no stamp (a target file deleted: its
//! `gone`); hub.log says when the backstop is the one that found a
//! change.

use crate::artifacts::{Meta, Store};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

/// The backstop: the rows are built again at least this often.
pub(super) const BACKSTOP_MS: u64 = 5 * 60_000;

/// (disk generation, agents' names, backstop bucket, folders' stamps).
pub(super) type ArtKey = (u64, u64, u64, u64);

/// What the artifacts event is built from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ArtView {
    pub(super) rows: Vec<Value>,
    pub(super) stored: BTreeMap<String, Meta>,
    pub(super) new: usize,
    pub(super) seen_ms: u64,
    /// The pages with something waiting on him (the page rows'
    /// `waiting`/`asking`): they stay open work (artifacts::join_sections).
    pub(super) waiting: std::collections::BTreeSet<String>,
    /// The bytes of the agents' files' copies (artifacts_retention): the
    /// From agents place's foot line.
    pub(super) agents_bytes: u64,
    /// Each artifact's current file, its path as the store keeps it (made
    /// canonical at its add, `Store::add`) -> its id: a `~/bise` row that
    /// is an artifact's current target opens as that artifact (A4, one
    /// document one write path). This hub's store only: on the home hub,
    /// the home store's artifacts (a project's artifact under `~/bise`
    /// is not linked, for now). Shared: the files thread reads it
    /// (architect m_63160: built once per build, no realpath per list).
    pub(super) links: Links,
}

/// [`ArtView::links`]: canonical path -> artifact id.
pub(super) type Links = std::sync::Arc<BTreeMap<PathBuf, String>>;

/// The links of the artifacts `all`: a file or folder (never a link),
/// at its current version's target.
pub(super) fn links_of(all: &[Meta]) -> Links {
    let mut out = BTreeMap::new();
    for m in all {
        let Some(v) = m.current() else { continue };
        let target = if v.target.is_empty() { &m.source } else { &v.target };
        if target.is_empty() || crate::artifacts::is_link(target) || !std::path::Path::new(target).is_absolute() {
            continue;
        }
        out.insert(PathBuf::from(target.trim_end_matches('/')), m.id.clone());
    }
    std::sync::Arc::new(out)
}

/// An agent as `art_who` reads it, copied for the builder's thread.
#[derive(Clone, Debug)]
pub(super) struct WhoRow {
    pub(super) name: String,
    pub(super) dir: String,
    pub(super) aliases: Vec<String>,
    pub(super) archived: bool,
}

/// One build's inputs, owned (it runs on the `art-rows` thread).
pub(super) struct ArtJob {
    pub(super) state: PathBuf,
    pub(super) workspace: String,
    pub(super) who: Vec<WhoRow>,
    pub(super) now: u64,
    /// The page store, for which pages wait on him (its own memo; read
    /// here, never on the loop).
    pub(super) pages: Option<std::sync::Arc<crate::pages::Pages>>,
}

impl ArtJob {
    /// The view: the store read once (rows, metas and the new count from
    /// the same list).
    pub(super) fn build(self) -> ArtView {
        let store = Store::new(&self.state);
        let who = |name: &str| {
            self.who
                .iter()
                .find(|a| a.name == name || a.dir == name || a.aliases.iter().any(|x| x == name))
                .map(|a| (a.name.clone(), a.archived))
        };
        let stored = store.stored();
        let all = store.all_with(stored.clone());
        ArtView {
            rows: store.rows_from(&all, &who, &self.workspace),
            new: store.new_count_from(&all, self.now),
            seen_ms: store.seen_ms(self.now),
            agents_bytes: crate::artifacts_retention::agents_bytes(&store, &stored, &store.marks()),
            links: links_of(&all),
            stored: stored.into_iter().map(|m| (m.id.clone(), m)).collect(),
            waiting: self.pages.map(|p| p.list_rows().into_iter().filter(|r| r.waiting > 0 || r.asking).map(|r| r.id).collect()).unwrap_or_default(),
        }
    }
}

/// The last view built and the builder's state. Pure: the caller runs
/// the build and gives its result back.
#[derive(Debug, Default)]
pub(super) struct ArtRows {
    /// Shared: an ask is a pointer copy, never a copy of 1300 rows (a
    /// clone of them took 380 ms at load 90).
    last: Option<(ArtKey, Rc<ArtView>)>,
    /// A build runs off the loop.
    building: bool,
}

impl ArtRows {
    /// The view for `key`: the last one built (None before the first),
    /// and true when the caller must start a build for `key` now (the
    /// key moved and none runs; it runs until [`ArtRows::built`]).
    pub(super) fn ask(&mut self, key: ArtKey) -> (Option<Rc<ArtView>>, bool) {
        let start = !self.building && self.last.as_ref().is_none_or(|(k, _)| *k != key);
        if start {
            self.building = true;
        }
        (self.last.as_ref().map(|(_, v)| Rc::clone(v)), start)
    }

    /// A build for `key` landed: its view is the one shown (even when a
    /// newer key waits: it is newer than what the clients have; the next
    /// ask starts that one). True when only the backstop moved the key
    /// and the rows changed: a change no stamp saw (hub.log says it).
    pub(super) fn built(&mut self, key: ArtKey, view: ArtView) -> bool {
        let by_backstop = self.last.as_ref().is_some_and(|(k, v)| (k.0, k.1, k.3) == (key.0, key.1, key.3) && k.2 != key.2 && v.rows != view.rows);
        self.last = Some((key, Rc::new(view)));
        self.building = false;
        by_backstop
    }

    /// A build runs off the loop now.
    #[cfg(test)]
    pub(super) fn building(&self) -> bool {
        self.building
    }

    /// The build could not start (no thread): the next ask tries again.
    pub(super) fn failed(&mut self) {
        self.building = false;
    }
}

#[cfg(test)]
#[path = "art_rows_tests.rs"]
mod tests;
