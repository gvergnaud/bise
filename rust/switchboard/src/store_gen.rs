//! One number that moves at each write to the pages or the artifacts
//! stores (send-fast), and [`Memo`]: a value built from those stores,
//! built again only when its key moves.
//!
//! Why: the state push (≤ 10 a second) and the Tick rebuilt the pages'
//! and the artifacts' rows from disk each time. On his hub (137 pages,
//! 741 artifacts) that was 60% of the loop's time in a 15 s sample at
//! load ~45, so his sends waited behind it.
//!
//! Every write of both stores goes through the hub process (the loop,
//! and the page server's thread for a page's drafts): each store's write
//! doors call [`bump`] (pages/store.rs `write_atomic`, artifacts.rs
//! `write_atomic` and `copy_whole`, artifacts_archive.rs's removal); each
//! change of what a page row reads from memory (pages/mod.rs `set_busy`,
//! `done_at`) calls [`bump_mem`], which moves the pages' key ([`now`])
//! and never the artifacts' ([`disk_now`]: hub-loop, a set_busy at each
//! turn rebuilt 1300 artifact rows). What changes outside the hub is in
//! the keys too: [`dir_stamp`], the mtimes of the pages' folder and of
//! each page's folder (a page written by another process: artifacts_e2e,
//! release-45 m_35739), and of the artifacts' folder; an artifact's
//! target file the user deletes (its row's `gone`) by the artifact rows'
//! 5 min backstop (daemon/art_rows.rs, built off the loop).
//!
//! A memo only derives; it never decides (architect m_35024).

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static GEN: AtomicU64 = AtomicU64::new(1);
static MEM: AtomicU64 = AtomicU64::new(0);

/// A write to a store on disk happened (the pages' or the artifacts').
pub fn bump() {
    GEN.fetch_add(1, Ordering::AcqRel);
}

/// What a page row reads from the hub's memory changed (pages/mod.rs
/// `set_busy`, `mark_done`): the pages' rows move, never the artifacts'
/// (hub-loop: a set_busy at each turn rebuilt 1300 artifact rows).
pub fn bump_mem() {
    MEM.fetch_add(1, Ordering::AcqRel);
}

/// The generation of what the pages' rows read: the stores and the
/// hub's memory.
pub fn now() -> u64 {
    GEN.load(Ordering::Acquire) + MEM.load(Ordering::Acquire)
}

/// The generation of the stores on disk only (what the artifacts' rows
/// read).
pub fn disk_now() -> u64 {
    GEN.load(Ordering::Acquire)
}

/// One number for the state of `dir` on disk: its mtime and each of its
/// folders' (name, mtime). A folder or file added, removed or renamed in
/// it, or in one of its folders, moves it (a write_atomic renames into
/// the folder). One stat per folder: cheap next to reading the files.
pub fn dir_stamp(dir: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mtime = |p: &Path| crate::store_io::metadata(p).and_then(|m| m.modified()).ok();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    mtime(dir).hash(&mut h);
    let mut kids: Vec<_> = crate::store_io::read_dir(dir).map(|rd| rd.flatten().map(|e| (e.file_name(), mtime(&e.path()))).collect()).unwrap_or_default();
    kids.sort();
    kids.hash(&mut h);
    h.finish()
}

/// How often the stamps' thread reads each watched folder again.
pub const STAMP_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

type Watched = std::sync::Mutex<Vec<(std::path::PathBuf, std::sync::Arc<AtomicU64>)>>;

/// [`dir_stamp`] of `dir` as the `store-stamps` thread last read it
/// (every [`STAMP_EVERY`]): the loop reads an atomic, never the folder
/// (hub-loop, architect m_61480: the artifacts' key listed 1550 folders
/// with a stat each on the loop, a disk wait at load). The first ask for
/// a folder reads it once there. A write by another process is seen
/// within STAMP_EVERY; the hub's own writes move the generation at once.
pub fn stamp(dir: &Path) -> u64 {
    static WATCHED: std::sync::OnceLock<std::sync::Arc<Watched>> = std::sync::OnceLock::new();
    let watched = WATCHED.get_or_init(|| {
        let w: std::sync::Arc<Watched> = Default::default();
        let mine = w.clone();
        let _ = std::thread::Builder::new().name("store-stamps".into()).spawn(move || loop {
            std::thread::sleep(STAMP_EVERY);
            let dirs = mine.lock().unwrap_or_else(|e| e.into_inner()).clone();
            for (d, v) in dirs {
                v.store(dir_stamp(&d), Ordering::Release);
            }
        });
        w
    });
    let mut list = watched.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, v)) = list.iter().find(|(d, _)| d == dir) {
        return v.load(Ordering::Acquire);
    }
    let s = dir_stamp(dir);
    list.push((dir.to_path_buf(), std::sync::Arc::new(AtomicU64::new(s))));
    s
}

/// A value derived from a key: built again only when the key differs
/// from the last build's.
#[derive(Debug)]
pub struct Memo<K, V> {
    last: Option<(K, V)>,
}

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Memo { last: None }
    }
}

impl<K: PartialEq, V: Clone> Memo<K, V> {
    /// The value for `key`: the last one when its key is the same, else
    /// `build()`'s, kept.
    pub fn get(&mut self, key: K, build: impl FnOnce() -> V) -> V {
        match &self.last {
            Some((k, v)) if *k == key => v.clone(),
            _ => {
                let v = build();
                self.last = Some((key, v.clone()));
                v
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_memo_builds_again_only_when_its_key_moves() {
        let mut m: Memo<u64, u32> = Memo::default();
        let builds = std::cell::Cell::new(0);
        let get = |m: &mut Memo<u64, u32>, k: u64| {
            m.get(k, || {
                builds.set(builds.get() + 1);
                k as u32 * 10
            })
        };
        assert_eq!(get(&mut m, 1), 10);
        assert_eq!(get(&mut m, 1), 10);
        assert_eq!(get(&mut m, 2), 20);
        assert_eq!(get(&mut m, 1), 10);
        assert_eq!(builds.get(), 3);
    }

    /// Law (architect m_35024): every write of the two stores moves the
    /// memos' key. Each store's writes, counted in its code (tests left
    /// out); a new one turns this red: call `crate::store_gen::bump()` in
    /// it, then count it here.
    #[test]
    fn every_store_write_is_counted_and_bumps() {
        const MUTATIONS: &[&str] = &["fs::write(", "fs::rename(", "fs::remove_dir_all(", "fs::remove_file(", "fs::copy(", "File::create(", "OpenOptions"];
        let files: [(&str, &str, usize, usize); 3] = [
            // (file, its code, its write calls, its bumps)
            ("pages/store.rs", include_str!("pages/store.rs"), 4, 1),
            // + write_source: his edits in the viewer over the file (xlsx-univer, architect m_37387)
            ("artifacts.rs", include_str!("artifacts.rs"), 8, 3),
            ("artifacts_archive.rs", include_str!("artifacts_archive.rs"), 2, 1),
        ];
        for (name, src, writes, bumps) in files {
            let code = src.split("#[cfg(test)]").next().unwrap_or(src);
            let n: usize = MUTATIONS.iter().map(|m| code.matches(m).count()).sum();
            assert_eq!(n, writes, "{name}: a store write was added or removed: call crate::store_gen::bump() in it (the page and artifact rows are memoized on it), then update this count");
            assert_eq!(code.matches("crate::store_gen::bump()").count(), bumps, "{name}: its write doors bump the generation");
        }
        let pages = include_str!("pages/mod.rs");
        let code = pages.split("#[cfg(test)]").next().unwrap_or(pages);
        assert_eq!(code.matches("crate::store_gen::bump_mem()").count(), 2, "pages/mod.rs: set_busy and mark_done (a row's `held`) bump the memory's generation");
        assert_eq!(code.matches("crate::store_gen::bump()").count(), 0, "pages/mod.rs: an in-memory change never moves the disk generation (the artifacts' rows)");
    }

    /// hub-loop: what only the pages' rows read (a set_busy) moves the
    /// pages' key, never the artifacts' (their rows took 5 s to build).
    #[test]
    fn a_memory_bump_moves_the_pages_key_not_the_disks() {
        let (d, p) = (disk_now(), now());
        bump_mem();
        assert_eq!(disk_now(), d);
        assert!(now() > p);
    }

    /// A page written by another process (no bump) moves the stamp:
    /// its folder made, then its meta.json written in it.
    #[test]
    fn a_write_from_outside_the_hub_moves_the_stamp() {
        let d = std::env::temp_dir().join(format!("sb-stamp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let s0 = dir_stamp(&d);
        std::fs::create_dir_all(d.join("weekly")).unwrap();
        let s1 = dir_stamp(&d);
        assert_ne!(s0, s1, "a page's folder made");
        std::fs::write(d.join("weekly").join("meta.json"), "{}").unwrap();
        assert_ne!(s1, dir_stamp(&d), "a file written in a page's folder");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_bump_moves_the_generation() {
        let g = now();
        bump();
        assert!(now() > g);
    }
}
