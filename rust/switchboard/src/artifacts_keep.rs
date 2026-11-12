//! The store's side of crate::artifacts_marks: `<state>/artifacts/
//! marks.json` read and written (tmp + rename, the daemon's thread its
//! one writer), his keep and an agent's, his opens, and the one-shot
//! backfill of a store from before the fields.

use super::{read_json, write_atomic, Meta, Store};
use crate::artifacts_marks::{self as marks, Marks, Outcome, Who};

/// What the backfill did: how many it found for him, between agents,
/// and kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Backfilled {
    pub you: usize,
    pub agents: usize,
    pub kept: usize,
}

impl Store {
    fn marks_file(&self) -> std::path::PathBuf {
        self.dir().join("marks.json")
    }

    /// marks.json as it is (none yet: empty).
    pub fn marks(&self) -> Marks {
        read_json(&self.marks_file()).unwrap_or_default()
    }

    fn save_marks(&self, m: &Marks) -> Result<(), String> {
        if !m.writable() {
            return Err(format!("marks.json is from a newer bise (v{}): not written", m.v));
        }
        let bytes = serde_json::to_vec_pretty(m).map_err(|e| e.to_string())?;
        write_atomic(&self.marks_file(), &bytes)
    }

    /// Keep (or let go, `keep` false) each of `ids`, all known or none
    /// (`title`: an id's title, None for one the list does not hold; the
    /// daemon reads it from the rows it built, never the store on its
    /// loop): each one's id, title and outcome, marks.json written once
    /// when one changed.
    pub fn keep(&self, ids: &[String], title: &dyn Fn(&str) -> Option<String>, who: Who, keep: bool, now: u64) -> Result<Vec<(String, String, Outcome)>, String> {
        let named = ids
            .iter()
            .map(|id| title(id).map(|t| (id.clone(), t)).ok_or_else(|| format!("no artifact {id} (sb artifact list)")))
            .collect::<Result<Vec<_>, _>>()?;
        let mut m = self.marks();
        let out: Vec<(String, String, Outcome)> = named
            .into_iter()
            .map(|(id, t)| {
                let o = if keep { marks::keep(&mut m, &id, who, now) } else { marks::unkeep(&mut m, &id, who) };
                (id, t, o)
            })
            .collect();
        if out.iter().any(|(_, _, o)| *o == Outcome::Changed) {
            self.save_marks(&m)?;
        }
        Ok(out)
    }

    /// His opens, `(id, at, version seen)` (the daemon gathers them, at
    /// most one write a second): each moves only forward. True when one
    /// moved.
    pub fn opened(&self, opens: &[(String, u64, Option<u64>)]) -> Result<bool, String> {
        let mut m = self.marks();
        let moved = opens.iter().fold(false, |moved, (id, at, v)| marks::opened(&mut m, id, *at, *v) | moved);
        if moved {
            self.save_marks(&m)?;
        }
        Ok(moved)
    }

    /// The one-shot backfill (TODO(artifacts-backfill): goes in a later
    /// release, tests/shims.txt), run off the hub's loop on its own thread
    /// (hub-loop m_61273): each stored artifact without an audience gets
    /// artifacts-audit's rules', written in its meta.json; what is for him
    /// and outlives its task is kept by its own agent ("kept by
    /// learnings"), at its first time. Each write is guarded
    /// ([`write_unless_moved`]): a meta.json or marks.json the loop changed
    /// meanwhile is left as the loop wrote it (the next start retries what
    /// was skipped). Runs once per store (`backfilled` in marks.json):
    /// never again, even when he lets every kept one go. Never touches an
    /// artifact that has an audience or a mark already. None: it ran
    /// before.
    pub fn backfill(&self) -> Result<Option<Backfilled>, String> {
        if self.marks().backfilled.is_some() || !self.marks().writable() {
            return Ok(None);
        }
        let mut n = Backfilled::default();
        let mut keeps: Vec<(String, String, u64)> = Vec::new();
        let mut skipped = false;
        for meta in self.all() {
            let fresh = meta.by != "page" && meta.audience.is_none();
            let audience = meta.audience();
            if fresh {
                let path = self.dir().join(&meta.id).join("meta.json");
                let Ok(before) = std::fs::read(&path) else { continue };
                let Ok(on_disk) = serde_json::from_slice::<Meta>(&before) else { continue };
                let bytes = serde_json::to_vec_pretty(&Meta { audience: Some(audience), ..on_disk }).map_err(|e| e.to_string())?;
                skipped |= !write_unless_moved(&path, &before, &bytes)?;
            }
            match audience {
                marks::Audience::You => n.you += 1,
                marks::Audience::Agents => n.agents += 1,
            }
            if (fresh || meta.by == "page") && audience == marks::Audience::You && marks::evergreen(&meta.title, &meta.id) {
                keeps.push((meta.id.clone(), meta.agent.clone(), meta.created_ms));
            }
        }
        // marks.json last, read again: his keep or open written meanwhile stays
        let path = self.marks_file();
        let before = std::fs::read(&path).unwrap_or_default();
        let mut m: Marks = serde_json::from_slice(&before).unwrap_or_default();
        for (id, agent, at) in &keeps {
            if m.get(id).is_none() {
                marks::keep(&mut m, id, Who::Agent(agent), *at);
                n.kept += 1;
            }
        }
        // a skipped meta.json: no marker, the next start finishes it
        if !skipped {
            m.backfilled = Some(marks::BACKFILL);
        }
        let bytes = serde_json::to_vec_pretty(&m).map_err(|e| e.to_string())?;
        if !write_unless_moved(&path, &before, &bytes)? {
            return Ok(None);
        }
        Ok(Some(n))
    }
}

/// Write `bytes` at `p` (tmp + rename, like the store's writes) unless the
/// file is no longer `before` (empty: it was not there): the loop wrote it
/// meanwhile, its write stays. False: not written.
fn write_unless_moved(p: &std::path::Path, before: &[u8], bytes: &[u8]) -> Result<bool, String> {
    let tmp = p.with_extension("backfill.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    if std::fs::read(p).unwrap_or_default() != before {
        let _ = std::fs::remove_file(&tmp);
        return Ok(false);
    }
    crate::store_gen::bump();
    std::fs::rename(&tmp, p).map_err(|e| e.to_string())?;
    Ok(true)
}
