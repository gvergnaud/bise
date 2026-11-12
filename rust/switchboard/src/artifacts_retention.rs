//! How long bise keeps an agent's file (his q-library pick on page
//! artifacts-signal v13, architect m_62392): his files, anything kept (by
//! him or an agent), pages and what is for him never expire; an agent's
//! file for the agents keeps its copies until [`KEEP_MS`] after its task
//! ended (its agent archived), then the sweep deletes them and the row
//! stays, `file expired` with a link to the agent's thread. Pure: the
//! sweep (daemon/art.rs) reads the facts and acts on the answer.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::artifacts::{Meta, Store};
use crate::artifacts_marks::{Audience, Marks};

/// How long an agent's file outlives its task: 3 months (a const for
/// now, architect m_62392; his Settings line says it).
pub const KEEP_MS: u64 = 90 * 86_400_000;

/// What the rule reads of an artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts<'a> {
    /// who added it: `you`, `page`, or the agent
    pub by: &'a str,
    pub audience: Audience,
    /// kept by him or an agent
    pub kept: bool,
}

/// Whether its copies go now: an agent's file for the agents, kept by
/// none, whose agent the sweep first saw archived at `seen_archived_ms`
/// (None: live, or not known: never) more than [`KEEP_MS`] before `now`.
pub fn expires(f: Facts, seen_archived_ms: Option<u64>, now: u64) -> bool {
    if f.by == "you" || f.by == "page" || f.kept || f.audience == Audience::You {
        return false;
    }
    seen_archived_ms.is_some_and(|a| now > a.saturating_add(KEEP_MS))
}

/// `<state>/artifacts/ended.json`: each archived agent (by every name it
/// goes by) -> seen_archived_ms, when the retention sweep first saw it
/// archived. Retention's own record of a fact sb-core owns (when an agent
/// was archived): the first sweep starts every agent archived already at
/// its own now, which is safe (later, never earlier). It goes away when
/// sb-core puts an archive time on the agent row (architect m_62433,
/// tech debt). The sweep thread is its one writer; an agent back from
/// the archive leaves it.
fn ended_file(store: &Store) -> PathBuf {
    store.dir().join("ended.json")
}

/// Each archived agent's seen_archived_ms, with `archived` (each archived
/// agent's names, read on the loop) seen at `now`: a new one from now,
/// one no longer archived out. Written only when it changed.
pub fn note_ended(store: &Store, archived: &BTreeSet<String>, now: u64) -> Result<BTreeMap<String, u64>, String> {
    let p = ended_file(store);
    let was: BTreeMap<String, u64> =
        crate::store_io::read(&p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let is: BTreeMap<String, u64> = archived.iter().map(|n| (n.clone(), was.get(n).copied().unwrap_or(now))).collect();
    if is != was {
        let bytes = serde_json::to_vec_pretty(&is).map_err(|e| e.to_string())?;
        let tmp = p.with_extension("tmp");
        std::fs::create_dir_all(store.dir()).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    }
    Ok(is)
}

/// What a sweep did: rows expired, bytes freed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    pub rows: usize,
    pub bytes: u64,
}

/// The bytes under `p` (a file, or a folder copy's files).
fn size_of(p: &Path) -> u64 {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => std::fs::read_dir(p).map_or(0, |rd| rd.flatten().map(|e| size_of(&e.path())).sum()),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

/// The copies the store keeps of agents' files for the agents: its bytes
/// (Settings › Storage, 'Agents' files'). Expired ones keep none.
pub fn agents_bytes(store: &Store, metas: &[Meta], marks: &Marks) -> u64 {
    metas
        .iter()
        .filter(|m| m.expired_ms.is_none() && facts(m, marks).audience == Audience::Agents && !facts(m, marks).kept)
        .filter(|m| m.by != "you" && m.by != "page")
        .flat_map(|m| m.versions.iter().filter_map(|v| store.copy_path(m, v)))
        .map(|p| size_of(&p))
        .sum()
}

fn facts<'a>(m: &'a Meta, marks: &Marks) -> Facts<'a> {
    Facts { by: &m.by, audience: m.audience(), kept: marks.kept(&m.id).is_some() }
}

/// One sweep at `now` (its own thread, never the hub loop): each stored
/// artifact that [`expires`] loses its copies first, then gets
/// `expired_ms` through the one meta writer. An expired one is skipped.
pub fn sweep(store: &Store, ended: &BTreeMap<String, u64>, now: u64) -> Result<Swept, String> {
    let marks = store.marks();
    let mut out = Swept::default();
    for m in store.stored() {
        if m.expired_ms.is_some() || !expires(facts(&m, &marks), ended.get(&m.agent).copied(), now) {
            continue;
        }
        for v in &m.versions {
            let Some(p) = store.copy_path(&m, v) else { continue };
            let n = size_of(&p);
            let gone = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
            match gone {
                Ok(()) => out.bytes += n,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("{}: {e}", p.display())),
            }
        }
        store.save(&Meta { expired_ms: Some(now), ..m })?;
        out.rows += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 86_400_000;
    const ENDED: u64 = 1_000 * DAY;
    const LATER: u64 = ENDED + KEEP_MS + DAY;

    fn agents(by: &str) -> Facts<'_> {
        Facts { by, audience: Audience::Agents, kept: false }
    }

    #[test]
    fn his_kept_pages_and_for_him_never_expire() {
        assert!(!expires(Facts { by: "you", audience: Audience::Agents, kept: false }, Some(ENDED), LATER));
        assert!(!expires(Facts { by: "page", audience: Audience::Agents, kept: false }, Some(ENDED), LATER));
        assert!(!expires(Facts { kept: true, ..agents("perf") }, Some(ENDED), LATER));
        assert!(!expires(Facts { audience: Audience::You, ..agents("perf") }, Some(ENDED), LATER));
    }

    #[test]
    fn an_agent_still_working_or_unknown_never_expires() {
        assert!(!expires(agents("perf"), None, LATER));
    }

    #[test]
    fn three_months_and_a_day_after_its_task_ended_it_goes() {
        assert!(expires(agents("perf"), Some(ENDED), LATER));
        assert!(!expires(agents("perf"), Some(ENDED), ENDED + KEEP_MS), "on the day itself it stays");
        assert!(!expires(agents("perf"), Some(ENDED), ENDED + KEEP_MS - DAY));
    }

    fn store(name: &str) -> (Store, PathBuf) {
        let d = std::env::temp_dir().join(format!("sb-retention-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let d = d.canonicalize().unwrap();
        (Store::new(&d.join("state")), d)
    }

    fn add(s: &Store, d: &Path, file: &str, by: &str, audience: Audience) -> Meta {
        std::fs::write(d.join(file), "0123456789").unwrap();
        let a = crate::artifacts::Add {
            target: d.join(file).to_string_lossy().to_string(),
            agent: by.into(),
            by: by.into(),
            cwd: d.to_string_lossy().to_string(),
            ..Default::default()
        };
        let m = s.add(&a, ENDED - DAY).unwrap().meta;
        s.save(&Meta { audience: Some(audience), ..m.clone() }).unwrap();
        Meta { audience: Some(audience), ..m }
    }

    /// architect m_62392: the copies go first, then `expired_ms` through
    /// the one writer; his file, a file for him, a live agent's stay; a
    /// second sweep finds nothing.
    #[test]
    fn a_sweep_deletes_an_ended_agents_copies_and_keeps_the_row() {
        let (s, d) = store("sweep");
        let gone = add(&s, &d, "log.txt", "perf", Audience::Agents);
        let his = add(&s, &d, "plan.md", "you", Audience::Agents);
        let for_him = add(&s, &d, "report.md", "perf", Audience::You);
        let live = add(&s, &d, "notes.txt", "helper", Audience::Agents);
        let copy = s.copy_path(&gone, gone.current().unwrap()).unwrap();
        assert!(copy.exists());
        let ended = BTreeMap::from([("perf".to_string(), ENDED)]);
        assert_eq!(agents_bytes(&s, &s.stored(), &s.marks()), 20, "perf's log and helper's notes");

        assert_eq!(sweep(&s, &ended, LATER).unwrap(), Swept { rows: 1, bytes: 10 });
        assert!(!copy.exists());
        let after = |id: &str| s.stored().into_iter().find(|m| m.id == id).unwrap();
        assert_eq!(after(&gone.id).expired_ms, Some(LATER));
        for m in [&his, &for_him, &live] {
            assert_eq!(after(&m.id).expired_ms, None, "{}", m.id);
            assert!(s.copy_path(m, m.current().unwrap()).unwrap().exists(), "{}", m.id);
        }
        assert_eq!(agents_bytes(&s, &s.stored(), &s.marks()), 10, "helper's notes only");
        assert_eq!(sweep(&s, &ended, LATER + DAY).unwrap(), Swept::default());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn an_agent_is_ended_from_when_the_sweep_first_saw_it_archived() {
        let (s, d) = store("ended");
        let set = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
        assert_eq!(note_ended(&s, &set(&["perf"]), 5).unwrap(), BTreeMap::from([("perf".into(), 5)]));
        let both = note_ended(&s, &set(&["perf", "helper"]), 9).unwrap();
        assert_eq!(both, BTreeMap::from([("perf".into(), 5), ("helper".into(), 9)]), "perf keeps its first sight");
        assert_eq!(note_ended(&s, &set(&["helper"]), 12).unwrap(), BTreeMap::from([("helper".into(), 9)]), "perf came back: out");
        assert_eq!(note_ended(&s, &set(&["perf"]), 20).unwrap()["perf"], 20, "archived again: from then");
        let _ = std::fs::remove_dir_all(d);
    }
}
