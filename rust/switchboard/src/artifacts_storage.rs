//! A site artifact's localStorage, kept by the hub while sites ran opaque
//! (web-storage, architect m_46151): kit/storage.js gave a site copy an
//! in-memory storage, which bise's viewer posted here. A site now opens in
//! Sites on its own origin with a real storage (docs/plugin-apps.md A5,
//! architect m_60421): nothing writes this any more.
//!
//! - `<state>/artifacts/<id>/storage.json` ([`crate::artifacts::Store::storage_path`]):
//!   `{"<key>": "<value>"}`, shared by every version of `<id>`.
//! - [`read`]: the map, `{}` when none (or unreadable): the page server
//!   still seeds each html page of the copy with it (site.rs `tagged`).
//! - [`seed`]: the map, once, for a site opened in Sites (its own origin,
//!   a real localStorage): then moved aside; the shim `site-storage-seed`
//!   (tests/shims.txt) goes with this file.

use crate::artifacts::Store;
use std::collections::BTreeMap;

/// A site's kept map.
pub type Kept = BTreeMap<String, String>;

/// The kept map of `id` (empty: none yet, or unreadable).
pub fn read(store: &Store, id: &str) -> Kept {
    std::fs::read(store.storage_path(id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Where [`seed`] puts a map it gave away (kept, never read again by bise:
/// his data is not lost if the view failed to write it).
// TODO(site-storage-seed): goes in v2026.12.15 with this file
pub fn seeded_path(store: &Store, id: &str) -> std::path::PathBuf {
    store.storage_path(id).with_file_name("storage.seeded.json")
}

/// The kept map of `id` for the one seed of its site's real localStorage
/// (docs/plugin-apps.md A5, architect m_60421): the map, once, then
/// moved aside ([`seeded_path`]); None when there is none (or it went
/// already). The page server's lock is the caller's.
// TODO(site-storage-seed): goes in v2026.12.15, once every site opened
// since moved its kept map into its own origin's storage
pub fn seed(store: &Store, id: &str) -> Option<Kept> {
    let p = store.storage_path(id);
    let kept = read(store, id);
    std::fs::rename(&p, seeded_path(store, id)).ok()?;
    Some(kept).filter(|k| !k.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keep(s: &Store, id: &str, m: &Kept) {
        let p = s.storage_path(id);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, serde_json::to_vec(m).unwrap()).unwrap();
    }

    /// A site opened in Sites gets its kept map once (A5, architect
    /// m_60421), then it is moved aside, never lost (TODO(site-storage-seed)).
    #[test]
    fn a_kept_map_seeds_once_and_stays_aside() {
        let d = std::env::temp_dir().join(format!("sb-art-seed-{}", std::process::id()));
        let s = Store::new(&d);
        assert!(read(&s, "tracker").is_empty());
        assert_eq!(seed(&s, "tracker"), None, "nothing kept");
        let m = Kept::from([("done".to_string(), "</script>3".to_string())]);
        keep(&s, "tracker", &m);
        assert_eq!(read(&s, "tracker"), m);
        assert_eq!(seed(&s, "tracker"), Some(m.clone()));
        assert_eq!(seed(&s, "tracker"), None, "once");
        assert!(!s.storage_path("tracker").exists() && seeded_path(&s, "tracker").exists(), "aside, not deleted");
        let _ = std::fs::remove_dir_all(&d);
    }
}
