//! The window's copy of each project's main thread, its last page
//! (boot-speed, Gabriel's pick A; architect m_54045): when the window
//! subscribes main before its hub sent the page (a hub still booting, a
//! restart of his Mac), the core gives the copy at once as a `thread`
//! event marked `cached`. The store shows it with the catching-up cue and
//! replaces it whole by the hub's first page; nothing reads it for a
//! decision or a count. Display only: one small file per project
//! (`bise_home::Home::thread_cache`), main's last [`KEEP`] entries, at most
//! [`MAX_BYTES`], written at most every [`FLUSH_EVERY`], deleted with the
//! project. Pure: the core reads and writes the files ([`read`], [`write`]).

use bise_proto::hub::HubEv;
use bise_proto::thread::Entry;
use bise_proto::Pos;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

/// The one thread kept: the project's main.
pub const AGENT: &str = "main";
/// Entries kept: a page as the window's thread asks for it.
pub const KEEP: usize = 40;
/// A copy's file at most (its oldest entries go first).
pub const MAX_BYTES: usize = 256 * 1024;
/// A changed copy is written at most this often.
pub const FLUSH_EVERY: Duration = Duration::from_secs(5);

/// The copies of this core: what its hubs sent of main, by project.
#[derive(Default)]
pub struct Threads {
    kept: BTreeMap<String, BTreeMap<Pos, Entry>>,
    /// the projects whose hub sent main's page to this core: the window
    /// holds the real thread, never the copy again
    live: BTreeSet<String>,
    /// changed since written
    dirty: BTreeSet<String>,
    flushed: Option<Instant>,
}

impl Threads {
    /// A hub's event: main's page or entry goes into its project's copy.
    pub fn seen(&mut self, project: &str, ev: &HubEv) {
        let entries: Vec<&Entry> = match ev {
            HubEv::Thread { agent, entries, cached: false, .. } if agent == AGENT => {
                self.live.insert(project.to_string());
                entries.iter().collect()
            }
            HubEv::Entry { agent, entry, .. } if agent == AGENT => vec![entry.as_ref()],
            _ => return,
        };
        let kept = self.kept.entry(project.to_string()).or_default();
        for e in entries {
            kept.insert(e.pos, e.clone());
        }
        while kept.len() > KEEP {
            kept.pop_first();
        }
        self.dirty.insert(project.to_string());
    }

    /// The copy to show for the window's subscribe of `agent` in
    /// `project`: main's, read by `file` (its text), when this core's hub
    /// sent no page of it yet. Given once per core.
    pub fn copy_for(&mut self, project: &str, agent: &str, file: impl FnOnce() -> Option<String>) -> Option<HubEv> {
        if agent != AGENT || self.live.contains(project) {
            return None;
        }
        // once: a second subscribe before the page shows what the window holds
        self.live.insert(project.to_string());
        let entries = decode(&file()?)?;
        (!entries.is_empty()).then(|| HubEv::Thread { project: project.to_string(), agent: AGENT.into(), entries, before: None, more: false, cached: true })
    }

    /// The copies to write now, (project, its file's text or None to
    /// delete it): the changed ones, at most every [`FLUSH_EVERY`]
    /// (`all`: whatever the time, at the core's end).
    pub fn take_due(&mut self, now: Instant, all: bool) -> Vec<(String, Option<String>)> {
        if self.dirty.is_empty() || (!all && self.flushed.is_some_and(|t| now.duration_since(t) < FLUSH_EVERY)) {
            return Vec::new();
        }
        self.flushed = Some(now);
        let dirty = std::mem::take(&mut self.dirty);
        dirty.into_iter().map(|p| {
            let text = self.kept.get(&p).and_then(|k| encode(k.values()));
            (p, text)
        }).collect()
    }

    /// A project removed: its copy goes (the core deletes its file).
    pub fn forget(&mut self, project: &str) {
        self.kept.remove(project);
        self.dirty.remove(project);
    }
}

/// A copy's text: `{"entries": [...]}`, the newest kept under
/// [`MAX_BYTES`]; none when nothing fits.
pub fn encode<'a>(entries: impl DoubleEndedIterator<Item = &'a Entry>) -> Option<String> {
    let mut kept: Vec<&Entry> = Vec::new();
    let mut bytes: usize = 16;
    for e in entries.rev() {
        let n = serde_json::to_string(e).map_or(usize::MAX, |s| s.len() + 1);
        if bytes.saturating_add(n) > MAX_BYTES {
            break;
        }
        bytes += n;
        kept.push(e);
    }
    kept.reverse();
    (!kept.is_empty()).then(|| serde_json::json!({ "entries": kept }).to_string())
}

/// A copy's entries, oldest first; none when the file doesn't read.
pub fn decode(text: &str) -> Option<Vec<Entry>> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut entries: Vec<Entry> = serde_json::from_value(v.get("entries")?.clone()).ok()?;
    entries.sort_by_key(|e| e.pos);
    Some(entries)
}

/// The file's text, if it reads.
pub fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Writes the copy (a temp file, then a rename: never a half file), or
/// deletes it (`None`). A display cache: a failure is only logged.
pub fn write(path: &Path, text: Option<&str>) {
    let res = match text {
        None => match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
        Some(t) => (|| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, t)?;
            std::fs::rename(&tmp, path)
        })(),
    };
    if let Err(e) = res {
        eprintln!("ambient-core: thread copy {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(pos: Pos, text: &str) -> Entry {
        serde_json::from_value(serde_json::json!({"pos": pos, "at_ms": pos, "kind": "agent", "text": text})).unwrap()
    }

    fn page(project: &str, agent: &str, pos: &[Pos]) -> HubEv {
        HubEv::Thread { project: project.into(), agent: agent.into(), entries: pos.iter().map(|p| e(*p, "x")).collect(), before: None, more: false, cached: false }
    }

    fn entry(agent: &str, pos: Pos, text: &str) -> HubEv {
        HubEv::Entry { project: "p".into(), agent: agent.into(), entry: Box::new(e(pos, text)) }
    }

    #[test]
    fn main_s_last_entries_are_kept_and_written_back_as_a_cached_page() {
        let mut t = Threads::default();
        let all: Vec<Pos> = (1..=60).collect();
        t.seen("p", &page("p", AGENT, &all));
        t.seen("p", &entry(AGENT, 61, "new"));
        t.seen("p", &entry(AGENT, 61, "grown"));
        // another agent's thread is never kept
        t.seen("p", &page("p", "perf", &[100]));
        let now = Instant::now();
        let w = t.take_due(now, false);
        assert_eq!(w.len(), 1);
        let entries = decode(w[0].1.as_deref().unwrap()).unwrap();
        assert_eq!(entries.len(), KEEP);
        assert_eq!(entries.first().unwrap().pos, 61 - KEEP as Pos + 1);
        assert_eq!(entries.last().unwrap().text, "grown");
        // a fresh core: the copy at its first subscribe, marked cached, once
        let text = w[0].1.clone();
        let mut fresh = Threads::default();
        assert_eq!(fresh.copy_for("p", "perf", || text.clone()), None, "only main's");
        let Some(HubEv::Thread { entries, cached, more, .. }) = fresh.copy_for("p", AGENT, || text.clone()) else { panic!("no copy") };
        assert!(cached && !more);
        assert_eq!(entries.len(), KEEP);
        assert_eq!(fresh.copy_for("p", AGENT, || text.clone()), None, "once per core");
    }

    #[test]
    fn no_copy_once_the_hub_sent_main_s_page_and_none_from_a_bad_file() {
        let mut t = Threads::default();
        t.seen("p", &page("p", AGENT, &[1]));
        assert_eq!(t.copy_for("p", AGENT, || Some("{\"entries\":[]}".into())), None);
        let mut t = Threads::default();
        assert_eq!(t.copy_for("p", AGENT, || Some("not json".into())), None);
        let mut t = Threads::default();
        assert_eq!(t.copy_for("p", AGENT, || None), None);
        // a cached page passing through (the core's own) is not the hub's
        let mut t = Threads::default();
        t.seen("p", &HubEv::Thread { project: "p".into(), agent: AGENT.into(), entries: vec![e(1, "x")], before: None, more: false, cached: true });
        assert!(t.take_due(Instant::now(), true).is_empty());
    }

    #[test]
    fn writes_wait_for_the_flush_pause_but_not_at_the_end() {
        let mut t = Threads::default();
        let now = Instant::now();
        t.seen("p", &page("p", AGENT, &[1]));
        assert_eq!(t.take_due(now, false).len(), 1);
        t.seen("p", &entry(AGENT, 2, "y"));
        assert!(t.take_due(now + Duration::from_secs(1), false).is_empty(), "too soon");
        assert_eq!(t.take_due(now + Duration::from_secs(1), true).len(), 1, "at the end");
        t.seen("p", &entry(AGENT, 3, "z"));
        assert_eq!(t.take_due(now + FLUSH_EVERY + Duration::from_secs(2), false).len(), 1);
    }

    #[test]
    fn a_copy_stays_under_its_byte_cap_newest_first() {
        let big = "é".repeat(MAX_BYTES / 6);
        let entries: Vec<Entry> = (1..=5).map(|p| e(p, &big)).collect();
        let text = encode(entries.iter()).unwrap();
        assert!(text.len() <= MAX_BYTES, "{}", text.len());
        let back = decode(&text).unwrap();
        assert_eq!(back.last().unwrap().pos, 5, "the newest kept");
        assert!(back.len() < 5);
        let huge = [e(1, &"x".repeat(MAX_BYTES))];
        assert_eq!(encode(huge.iter()), None);
    }

    #[test]
    fn a_removed_project_s_copy_goes() {
        let mut t = Threads::default();
        t.seen("p", &page("p", AGENT, &[1]));
        t.forget("p");
        assert!(t.take_due(Instant::now(), true).is_empty());
    }
}
