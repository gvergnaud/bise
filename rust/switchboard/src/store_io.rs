//! The one door of the pages' and artifacts' stores' file reads (and
//! their stamps' stats), so a test can count them per thread
//! (hub-loop, architect m_61480): the hub's loop must never read a store
//! file at a state push. Its 5 s artifact rebuild on 2026-11-11 was disk
//! wait at load 60-85, which the loop's CPU time never shows; the count
//! does (daemon/loop_budget_tests.rs).
//!
//! Every read of a store file goes through here: its JSON and text, its
//! folders' listings, a stat (a target's `gone`, a stamp's mtime), a
//! realpath. A new read path that skips it is a read the guard misses.
//! `sb history` and `sb show`'s transcript reads (search.rs, a message's
//! position in daemon/history.rs) count here too: the loop reads none
//! (history-loop, architect m_61891; daemon/history_work.rs).

use std::path::{Path, PathBuf};

#[cfg(test)]
thread_local! {
    static READS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn counted() {
    #[cfg(test)]
    READS.with(|r| r.set(r.get() + 1));
}

/// How many store reads this thread made (tests).
#[cfg(test)]
pub fn reads() -> u64 {
    READS.with(|r| r.get())
}

pub fn read(p: &Path) -> std::io::Result<Vec<u8>> {
    counted();
    std::fs::read(p)
}

pub fn open(p: &Path) -> std::io::Result<std::fs::File> {
    counted();
    std::fs::File::open(p)
}

pub fn read_to_string(p: &Path) -> std::io::Result<String> {
    counted();
    std::fs::read_to_string(p)
}

pub fn read_dir(p: &Path) -> std::io::Result<std::fs::ReadDir> {
    counted();
    std::fs::read_dir(p)
}

pub fn metadata(p: &Path) -> std::io::Result<std::fs::Metadata> {
    counted();
    std::fs::metadata(p)
}

pub fn exists(p: &Path) -> bool {
    metadata(p).is_ok()
}

pub fn canonicalize(p: &Path) -> std::io::Result<PathBuf> {
    counted();
    p.canonicalize()
}
