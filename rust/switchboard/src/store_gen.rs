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
//! `write_atomic` and `copy_whole`, artifacts_archive.rs's removal), and so
//! does each change of what a page row reads from memory (pages/mod.rs
//! `set_busy`, `done_at`). What changes outside the hub (an artifact's
//! target file the user deletes: its row's `gone`) is caught by the
//! artifacts memo's backstop: its key also holds a 10 s time bucket.
//!
//! A memo only derives; it never decides (architect m_35024).

use std::sync::atomic::{AtomicU64, Ordering};

static GEN: AtomicU64 = AtomicU64::new(1);

/// A write to a store (or to what a row reads) happened.
pub fn bump() {
    GEN.fetch_add(1, Ordering::AcqRel);
}

/// The stores' generation now.
pub fn now() -> u64 {
    GEN.load(Ordering::Acquire)
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
        let mut builds = 0;
        let mut get = |m: &mut Memo<u64, u32>, k: u64| {
            m.get(k, || {
                builds += 1;
                k as u32 * 10
            })
        };
        assert_eq!(get(&mut m, 1), 10);
        assert_eq!(get(&mut m, 1), 10);
        assert_eq!(get(&mut m, 2), 20);
        assert_eq!(get(&mut m, 1), 10);
        drop(get);
        assert_eq!(builds, 3);
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
            ("artifacts.rs", include_str!("artifacts.rs"), 6, 2),
            ("artifacts_archive.rs", include_str!("artifacts_archive.rs"), 2, 1),
        ];
        for (name, src, writes, bumps) in files {
            let code = src.split("#[cfg(test)]").next().unwrap_or(src);
            let n: usize = MUTATIONS.iter().map(|m| code.matches(m).count()).sum();
            assert_eq!(n, writes, "{name}: a store write was added or removed: call crate::store_gen::bump() in it (the page and artifact rows are memoized on it), then update this count");
            assert_eq!(code.matches("crate::store_gen::bump()").count(), bumps, "{name}: its write doors bump the generation");
        }
        let pages = include_str!("pages/mod.rs");
        assert_eq!(pages.split("#[cfg(test)]").next().unwrap_or(pages).matches("crate::store_gen::bump()").count(), 2, "pages/mod.rs: set_busy and mark_done (a row's `held`) bump");
    }

    #[test]
    fn a_bump_moves_the_generation() {
        let g = now();
        bump();
        assert!(now() > g);
    }
}
