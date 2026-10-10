//! Archived artifacts (docs/artifacts.md, "archive"): an artifact put
//! away leaves `/artifacts`' list (a quiet `N archived` row opens them)
//! and its stored copies go, except the newest version's, so the space
//! comes back. The row is reversible (`unarchive`); the old copies are
//! not.
//!
//! The archived set is one file, `<state>/artifacts/archived.json`
//! (`{"<id>": <archived at ms>}`), so a bise page (read from the page
//! store, never written here) archives the same way as a stored one.

use super::{read_json, write_atomic, Meta, Store};
use std::collections::BTreeMap;
use std::path::Path;

/// What `sb artifact archive` picks: named ids, or every artifact that
/// fits all the filters given (an agent, before a time, a kind).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pick {
    pub ids: Vec<String>,
    pub agent: Option<String>,
    /// its current version came before this (ms)
    pub before_ms: Option<u64>,
    pub kind: Option<String>,
}

impl Pick {
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty() && self.agent.is_none() && self.before_ms.is_none() && self.kind.is_none()
    }

    /// Does this artifact fit? Named ids: one of them; else every filter.
    pub fn fits(&self, m: &Meta) -> bool {
        if !self.ids.is_empty() {
            return self.ids.contains(&m.id);
        }
        let at = m.current().map_or(m.created_ms, |v| v.at_ms);
        !self.is_empty()
            && self.agent.as_ref().is_none_or(|a| &m.agent == a)
            && self.before_ms.is_none_or(|b| at < b)
            && self.kind.as_ref().is_none_or(|k| &m.kind == k)
    }
}

/// `--before`: `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM` (UTC), as ms.
pub fn parse_before(s: &str) -> Option<u64> {
    let (day, time) = match s.trim().split_once(['T', ' ']) {
        Some((d, t)) => (d, Some(t)),
        None => (s.trim(), None),
    };
    let days = crate::every::parse_day(day)?;
    let mins = match time {
        Some(t) => {
            let (h, m) = t.split_once(':')?;
            let (h, m): (u64, u64) = (h.parse().ok()?, m.parse().ok()?);
            (h < 24 && m < 60).then_some(h * 60 + m)?
        }
        None => 0,
    };
    u64::try_from(days).ok().map(|d| d * 86_400_000 + mins * 60_000)
}

/// What an archive did: the ids put away, the ids it did not know, and
/// the bytes of the copies deleted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Archived {
    pub ids: Vec<String>,
    pub unknown: Vec<String>,
    pub freed: u64,
}

fn size_of(p: &Path) -> u64 {
    match std::fs::symlink_metadata(p) {
        Ok(md) if md.is_dir() => std::fs::read_dir(p).map_or(0, |rd| rd.flatten().map(|e| size_of(&e.path())).sum()),
        Ok(md) => md.len(),
        Err(_) => 0,
    }
}

/// `1.4 MB`, `640 KB`.
pub fn mb(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else {
        format!("{} KB", bytes.div_ceil(1000))
    }
}

impl Store {
    fn archived_file(&self) -> std::path::PathBuf {
        self.dir().join("archived.json")
    }

    /// The archived ids and when each was archived.
    pub fn archived(&self) -> BTreeMap<String, u64> {
        read_json(&self.archived_file()).unwrap_or_default()
    }

    fn save_archived(&self, set: &BTreeMap<String, u64>) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(set).map_err(|e| e.to_string())?;
        write_atomic(&self.archived_file(), &bytes)
    }

    /// The ids a pick names, newest first (archived or not).
    pub fn picked(&self, pick: &Pick) -> Vec<Meta> {
        self.all().into_iter().filter(|m| pick.fits(m)).collect()
    }

    /// Put away what the pick names: out of the list, and every stored
    /// copy but the newest version's deleted (that version keeps
    /// opening; an older one says `no copy: archived`). Archived again:
    /// it keeps its first time.
    pub fn archive(&self, pick: &Pick, now: u64) -> Result<Archived, String> {
        let hits = self.picked(pick);
        let mut out = Archived {
            unknown: pick.ids.iter().filter(|id| !hits.iter().any(|m| &m.id == *id)).cloned().collect(),
            ..Default::default()
        };
        let mut set = self.archived();
        for mut m in hits {
            if m.by != "page" {
                let last = m.versions.len().saturating_sub(1);
                let mut changed = false;
                for v in m.versions.iter_mut().take(last) {
                    let Some(copy) = v.copy.take() else { continue };
                    // the version's folder (`v2/plan.xlsx` -> `v2`)
                    let top = copy.split('/').next().unwrap_or(&copy).to_string();
                    let p = self.dir().join(&m.id).join(&top);
                    out.freed += size_of(&p);
                    std::fs::remove_dir_all(&p).or_else(|_| std::fs::remove_file(&p)).ok();
                    v.no_copy = Some("archived".into());
                    changed = true;
                }
                if changed {
                    self.save(&m)?;
                }
            }
            set.entry(m.id.clone()).or_insert(now);
            out.ids.push(m.id);
        }
        self.save_archived(&set)?;
        Ok(out)
    }

    /// Back in the list (the copies deleted stay gone): (ids back, ids
    /// that were not archived).
    pub fn unarchive(&self, pick: &Pick) -> Result<(Vec<String>, Vec<String>), String> {
        let mut set = self.archived();
        let mut back = Vec::new();
        let mut not = Vec::new();
        let hits: Vec<String> = if pick.ids.is_empty() { self.picked(pick).into_iter().map(|m| m.id).collect() } else { pick.ids.clone() };
        for id in hits {
            match set.remove(&id) {
                Some(_) => back.push(id),
                None if !pick.ids.is_empty() => not.push(id),
                None => {}
            }
        }
        self.save_archived(&set)?;
        Ok((back, not))
    }
}

/// What `sb artifact archive` answers.
pub fn archived_text(a: &Archived) -> String {
    let mut out = match a.ids.len() {
        0 => "nothing to archive.".to_string(),
        1 => format!("archived {} · {} freed · sb artifact unarchive {} brings it back", a.ids[0], mb(a.freed), a.ids[0]),
        n => format!("archived {} artifacts · {} freed · sb artifact list --archived shows them", n, mb(a.freed)),
    };
    if !a.unknown.is_empty() {
        out.push_str(&format!("\nno artifact {}.", a.unknown.join(", ")));
    }
    out
}

/// What `sb artifact unarchive` answers.
pub fn unarchived_text(back: &[String], not: &[String]) -> String {
    let mut out = match back {
        [] => "nothing was archived.".to_string(),
        [one] => format!("{} is back in the list", one),
        _ => format!("{} artifacts are back in the list", back.len()),
    };
    if !not.is_empty() && !back.is_empty() {
        out.push_str(&format!("\nnot archived: {}.", not.join(", ")));
    }
    out
}
