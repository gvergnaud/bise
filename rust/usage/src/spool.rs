//! The spool: one folder per UTC day (`Home::usage_day_dir`), one file per
//! counting process, `<kind>[-<hub hash>]-<pid>.json` (architect m_53623 c:
//! an opaque id, the 8 hex of a hub id, never the project's name). A
//! process keeps its day's [`Tally`] in memory and writes it whole every
//! 5 minutes and at a clean exit (tmp then rename: a reader never sees
//! half); a process that starts again with the same name reads its file
//! first and goes on from it. The sender sums a closed day's files
//! ([`read_day`], then [`Tally::sum`]).
//!
//! The edge of this crate that touches files, with the paths as
//! arguments; writing asks for a [`Permit`]. Not here: when to write, the
//! 30 days' pruning, deleting everything when he turns sharing off
//! (steps 2 to 5).

use std::io;
use std::path::Path;

use crate::sharing::Permit;
use crate::tally::Tally;

closed! {
    /// Which process counted.
    pub enum SourceKind {
        Hub = "hub",
        Core = "core",
        Tui = "tui",
    }
}

/// A counting process: its kind, the hash of its hub's id (none for the
/// app's core, which serves every project) and its pid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub kind: SourceKind,
    scope: Option<String>,
    pub pid: u32,
}

/// The most bytes a source's file holds; a bigger tally is not written.
pub const MAX_FILE: usize = 16 * 1024;

/// The 8 hex digits of a hub id (`<name>-<8 hex>`), never its name.
fn hub_hash(hub_id: &str) -> Option<String> {
    bise_home::hub_id::is_hub_id(hub_id).then(|| hub_id[hub_id.len() - 8..].to_string())
}

impl Source {
    /// A hub, by its hub id.
    pub fn hub(hub_id: &str, pid: u32) -> Option<Source> {
        Some(Source { kind: SourceKind::Hub, scope: Some(hub_hash(hub_id)?), pid })
    }

    /// The app's core.
    pub fn core(pid: u32) -> Source {
        Source { kind: SourceKind::Core, scope: None, pid }
    }

    /// A TUI, by the hub id it is attached to.
    pub fn tui(hub_id: &str, pid: u32) -> Option<Source> {
        Some(Source { kind: SourceKind::Tui, scope: Some(hub_hash(hub_id)?), pid })
    }

    /// Its file's name in a day's folder.
    pub fn file_name(&self) -> String {
        match &self.scope {
            Some(h) => format!("{}-{h}-{}.json", self.kind.name(), self.pid),
            None => format!("{}-{}.json", self.kind.name(), self.pid),
        }
    }

    /// The kind a spool file's name says, None for another file.
    pub fn kind_of_file(name: &str) -> Option<SourceKind> {
        let stem = name.strip_suffix(".json")?;
        let (kind, rest) = stem.split_once('-')?;
        let parts: Vec<&str> = rest.split('-').collect();
        let hex8 = |s: &str| s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        let pid = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        let kind = SourceKind::from_name(kind)?;
        let ok = match (kind, parts.as_slice()) {
            (SourceKind::Core, [p]) => pid(p),
            (SourceKind::Hub | SourceKind::Tui, [h, p]) => hex8(h) && pid(p),
            _ => false,
        };
        ok.then_some(kind)
    }
}

/// Write a source's tally for its day, whole and atomically.
pub fn write_source(_: &Permit, day_dir: &Path, source: &Source, tally: &Tally) -> io::Result<()> {
    let text = serde_json::to_string(tally)?;
    if text.len() > MAX_FILE {
        return Err(io::Error::other(format!("a usage source of {} bytes", text.len())));
    }
    std::fs::create_dir_all(day_dir)?;
    let path = day_dir.join(source.file_name());
    let tmp = day_dir.join(format!(".{}.tmp", source.file_name()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// A source's tally of that day, if it wrote one (a restart goes on from it).
pub fn read_source(day_dir: &Path, source: &Source) -> Option<Tally> {
    Tally::from_json(&std::fs::read_to_string(day_dir.join(source.file_name())).ok()?)
}

/// Every source of a day, with its kind; files of another shape, too big
/// or unreadable are skipped.
pub fn read_day(day_dir: &Path) -> Vec<(SourceKind, Tally)> {
    let Ok(rd) = std::fs::read_dir(day_dir) else { return Vec::new() };
    let mut out: Vec<(String, SourceKind, Tally)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let kind = Source::kind_of_file(&name)?;
            let small = e.metadata().ok().is_some_and(|m| m.len() as usize <= MAX_FILE);
            let tally = small.then(|| std::fs::read_to_string(e.path()).ok().and_then(|t| Tally::from_json(&t))).flatten()?;
            Some((name, kind, tally))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|(_, k, t)| (k, t)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::CountKey;

    #[test]
    fn a_source_file_names_no_project() {
        let hub = Source::hub("camille-taxes-3f9a2b1c", 4242).unwrap();
        assert_eq!(hub.file_name(), "hub-3f9a2b1c-4242.json");
        assert_eq!(Source::tui("camille-taxes-3f9a2b1c", 7).unwrap().file_name(), "tui-3f9a2b1c-7.json");
        assert_eq!(Source::core(9).file_name(), "core-9.json");
        assert_eq!(Source::hub("not a hub id", 1), None);
        for (name, kind) in [("hub-3f9a2b1c-4242.json", Some(SourceKind::Hub)), ("core-9.json", Some(SourceKind::Core)), ("core-3f9a2b1c-9.json", None), ("hub-camille-1.json", None), ("x.json", None), (".hub-3f9a2b1c-1.json.tmp", None)] {
            assert_eq!(Source::kind_of_file(name), kind, "{name}");
        }
    }

    #[test]
    fn a_day_reads_back_its_sources() {
        let dir = std::env::temp_dir().join(format!("bise-usage-spool-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // a Permit is sharing_allowed's only: the tests make one through decide's twin
        let permit = crate::sharing::test_permit();
        let mut t = Tally::default();
        t.count(CountKey::Land);
        let hub = Source::hub("x-0000000a", 1).unwrap();
        write_source(&permit, &dir, &hub, &t).unwrap();
        write_source(&permit, &dir, &Source::core(2), &t).unwrap();
        std::fs::write(dir.join("notes.txt"), "/Users/camille").unwrap();
        let day = read_day(&dir);
        assert_eq!(day.iter().map(|(k, _)| *k).collect::<Vec<_>>(), [SourceKind::Core, SourceKind::Hub]);
        assert_eq!(read_source(&dir, &hub), Some(t));
        assert!(!dir.join(format!(".{}.tmp", hub.file_name())).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
