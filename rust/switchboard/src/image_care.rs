//! The hub's side of the image store's retention (D14, docs/images.md
//! "Retention"; the rule is `bend_images::prune`'s). On its tick, a hub:
//!
//! - touches the images its hot messages and open cards name, at its
//!   start and every `TOUCH_EVERY_MS` (the store is shared by every hub
//!   of the home: the age is what keeps another hub's images);
//! - prunes the store at most once a day, on a thread, with the user's
//!   `shots_keep_days` (`bise_home::prefs::keep_days`), its own named
//!   images kept;
//! - never before `keep_days` after its first tick with the rule (the
//!   grace, architect m_33081: the update deletes nothing at once, he
//!   has the time to read the release note or change the setting;
//!   [`prune_allowed`]).
//!
//! - prunes its agents' MCP Apps views files (`<agent>/views/<call>.json`,
//!   `Paths::agent_views_dir`, docs/mcp-apps.md C2) with the same days on
//!   the same thread: the one prune owner (architect m_50047); a view
//!   whose file is gone shows "expired".
//!
//! When each was last done is the mtime of a stamp in the hub's folder
//! (`Paths::images_touched`, `Paths::images_pruned`, and
//! `Paths::images_since` for the grace's start): one stat each per tick,
//! and a restart neither prunes twice in a day nor waits an hour to touch
//! (a hub down for more than an hour finds its stamp old).

use crate::paths::Paths;
use bend_images::prune::{self, DAY_MS, TOUCH_EVERY_MS};
use std::collections::BTreeSet;
use std::path::Path;

/// Due when the stamp is missing, unreadable, older than `every` or in
/// the future (a clock moved back). Pure.
pub fn due(stamp_ms: Option<u64>, now_ms: u64, every_ms: u64) -> bool {
    match stamp_ms {
        None => true,
        Some(t) => t > now_ms || now_ms - t >= every_ms,
    }
}

/// The grace: a prune may run only once `keep_days` have passed since the
/// rule started on this hub (`since_ms`, the `images_since` stamp); no
/// stamp yet (the first tick after the update): never. Pure.
pub fn prune_allowed(since_ms: Option<u64>, now_ms: u64, keep_days: u64) -> bool {
    since_ms.is_some_and(|s| now_ms >= s.saturating_add(keep_days.max(1).saturating_mul(DAY_MS)))
}

fn stamp_ms(p: &Path) -> Option<u64> {
    let t = std::fs::metadata(p).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64)
}

fn stamp(p: &Path) {
    // a rewrite moves the mtime (the content says what it is)
    let _ = std::fs::write(p, b"bend_images::prune stamp\x0a");
}

/// A views file last written at `mtime_ms` is past `keep_days` at
/// `now_ms` (the images' rule: older than the days kept). Pure.
pub fn view_expired(mtime_ms: u64, now_ms: u64, keep_days: u64) -> bool {
    now_ms.saturating_sub(mtime_ms) > keep_days.max(1).saturating_mul(DAY_MS)
}

/// Delete every `<agents>/<agent>/views/*.json` past `keep_days` (and a
/// writer's leftover temp file); the count deleted.
pub fn prune_views(agents: &Path, now_ms: u64, keep_days: u64) -> usize {
    let Ok(rd) = std::fs::read_dir(agents) else { return 0 };
    let mut n = 0;
    for a in rd.flatten() {
        let views = bise_home::hub_state::views_of_agent_dir(&a.path());
        let Ok(files) = std::fs::read_dir(&views) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) != Some("json") && p.extension().and_then(|e| e.to_str()) != Some("tmp") {
                continue;
            }
            if stamp_ms(&p).is_some_and(|t| view_expired(t, now_ms, keep_days)) && std::fs::remove_file(&p).is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// What one tick did (the log's words).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Care {
    pub touched: Option<usize>,
    /// The prune started on a thread, with this many days.
    pub pruning: Option<u64>,
}

/// The tick's care: `texts` are the hub's live texts (hot messages, open
/// cards); `store` the image store, `prefs` the user's `prefs.json`.
/// Answers what it started; the prune's count goes to `done` on its
/// thread.
pub fn tick<'a>(
    paths: &Paths,
    store: &Path,
    prefs: &Path,
    texts: impl Iterator<Item = &'a str>,
    now_ms: u64,
    done: impl FnOnce(std::io::Result<usize>) + Send + 'static,
) -> Care {
    let touch_due = due(stamp_ms(&paths.images_touched()), now_ms, TOUCH_EVERY_MS);
    let mut prune_due = due(stamp_ms(&paths.images_pruned()), now_ms, DAY_MS);
    // the grace (architect m_33081): the first tick only writes when the
    // rule started; nothing goes before keep_days after it
    let since = stamp_ms(&paths.images_since());
    if since.is_none() {
        stamp(&paths.images_since());
    }
    let mut care = Care::default();
    if !touch_due && !prune_due {
        return care;
    }
    let days = if prune_due {
        let p = std::fs::read_to_string(prefs).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        bise_home::prefs::keep_days(p.as_ref())
    } else {
        0
    };
    prune_due = prune_due && prune_allowed(since, now_ms, days);
    if !touch_due && !prune_due {
        return care;
    }
    let named: BTreeSet<String> = texts.flat_map(prune::named).collect();
    if touch_due {
        prune::touch(store, &named);
        stamp(&paths.images_touched());
        care.touched = Some(named.len());
    }
    if prune_due {
        // stamped first: a crash mid-prune never makes it run at each tick
        stamp(&paths.images_pruned());
        let store = store.to_path_buf();
        let agents = paths.state.join("agents");
        std::thread::spawn(move || {
            let r = prune::prune(&store, now_ms, days, &named);
            prune_views(&agents, now_ms, days);
            done(r)
        });
        care.pruning = Some(days);
    }
    care
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_once_per_period() {
        assert!(due(None, 10, DAY_MS), "never done: now");
        assert!(!due(Some(10), 10 + DAY_MS - 1, DAY_MS));
        assert!(due(Some(10), 10 + DAY_MS, DAY_MS));
        assert!(due(Some(500), 10, DAY_MS), "a stamp in the future: the clock moved back");
    }

    #[test]
    fn a_tick_touches_hourly_and_prunes_daily() {
        let root = std::env::temp_dir().join(format!("sb-image-care-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths { workspace: root.join("ws"), state: root.join("hub"), worktrees: root.join("wt") };
        let store = root.join("images");
        std::fs::create_dir_all(&paths.state).expect("state");
        std::fs::create_dir_all(&store).expect("store");
        let prefs = root.join("prefs.json");
        std::fs::write(&prefs, r#"{"shots_keep_days": 30}"#).expect("prefs");
        let h = "0123456789abcdef0123456789abcdef";
        std::fs::write(store.join(format!("{h}.png")), "x").expect("img");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_millis() as u64;
        let text = format!("<image name=\"[Image #1]\" path=\"a.png\" mime=\"image/png\" b64=\"{}/{h}.b64\">", store.display());
        // the first tick after the update: it touches, writes when the rule
        // started and deletes nothing (the grace)
        let first = tick(&paths, &store, &prefs, [text.as_str()].into_iter(), now, |_| panic!("no prune in the grace"));
        assert_eq!(first, Care { touched: Some(1), pruning: None });
        assert!(paths.images_since().exists(), "the grace's start is written");
        // keep_days later (the stamp moved back by 30 days): it prunes
        prune::touch_file(&paths.images_since(), prune::at_ms(now - 30 * DAY_MS));
        let _ = std::fs::remove_file(paths.images_pruned());
        // a second later (a stamp written after `now` reads as a clock moved back)
        let (tx, rx) = std::sync::mpsc::channel();
        let care = tick(&paths, &store, &prefs, [text.as_str()].into_iter(), now + 1000, move |r| {
            let _ = tx.send(r.ok());
        });
        assert_eq!(care, Care { touched: None, pruning: Some(30) });
        assert_eq!(rx.recv().expect("done"), Some(0));
        // the next tick: both stamps are fresh, nothing to do
        let again = tick(&paths, &store, &prefs, [text.as_str()].into_iter(), now + 2000, |_| {});
        assert_eq!(again, Care::default());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// docs/mcp-apps.md C2: the daily prune takes the agents' views
    /// files past keep_days, keeps the fresh ones and anything else.
    #[test]
    fn the_prune_takes_old_views_files() {
        assert!(!view_expired(0, DAY_MS, 1));
        assert!(view_expired(0, DAY_MS + 1, 1));
        assert!(!view_expired(5, 1, 30), "a clock moved back keeps it");
        let root = std::env::temp_dir().join(format!("sb-image-care-views-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let views = bise_home::hub_state::agent_views(&root, "a");
        std::fs::create_dir_all(&views).expect("views");
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("clock").as_millis() as u64;
        for (name, age_days) in [("1-1.json", 31u64), ("1-2.json", 1), (".1-3.json.tmp", 40), ("notes.txt", 40)] {
            let p = views.join(name);
            std::fs::write(&p, "{}").expect("file");
            prune::touch_file(&p, prune::at_ms(now - age_days * DAY_MS));
        }
        assert_eq!(prune_views(&root.join("agents"), now, 30), 2);
        let mut left: Vec<String> = std::fs::read_dir(&views).expect("dir").flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, vec!["1-2.json", "notes.txt"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The grace's law (architect m_33081): no stamp, never; one ms before
    /// since + keep_days, never; from since + keep_days on, yes.
    #[test]
    fn the_first_prune_waits_keep_days_after_the_update() {
        for keep in [1u64, 7, 30] {
            let since = 1_800_000_000_000u64;
            let at = since + keep * DAY_MS;
            assert!(!prune_allowed(None, at * 2, keep), "no stamp: never");
            assert!(!prune_allowed(Some(since), at - 1, keep), "keep {keep}: one ms early");
            assert!(prune_allowed(Some(since), at, keep), "keep {keep}: on time");
            assert!(prune_allowed(Some(since), at + DAY_MS, keep));
        }
    }
}
