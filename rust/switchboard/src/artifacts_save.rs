//! His save of an artifact from a document view, the rule (page
//! xlsx-univer, architect m_37322, m_37328, m_37387; docs/plugin-apps.md
//! A4, architect m_59962): whether a save writes a new version, answers the
//! one it made already (a retry), or conflicts. Pure; the hub's loop calls
//! it before it renames the file and adds the version (daemon/view_files.rs,
//! the artifact store's only writer). One version per sitting: his sitting
//! is the document view's session (its doc), `seq` its save's number.

use crate::artifacts::Version;

/// His save, as the hub reads it from a write.
#[derive(Clone, Debug, PartialEq)]
pub struct Save {
    /// The version his edits started from (the write's `ifMatch`), and its rev.
    pub base: u64,
    pub rev: u32,
    /// The sitting (the view's doc session), and this save's number in it (from 1).
    pub session: String,
    pub seq: u64,
    /// write over a file changed on disk
    pub force: bool,
}

/// What the hub does with a save.
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    /// write the file, add the version
    Write,
    /// this save (or a later one of the sitting) is in already: its version
    Already { v: u64, rev: u32 },
    /// a version came in since his base
    Stale { v: u64, rev: u32 },
    /// the file on disk is not the newest version's copy (an agent wrote it)
    Disk { v: u64, rev: u32 },
}

/// The rule, pure. His own sitting's newest version is never a conflict
/// (a retry, or a save after a slow one); another version since his base
/// is; and unless `force`, so is a file on disk that is not the newest
/// version's (`disk_same`, asked only when needed).
pub fn decide(latest: Option<&Version>, s: &Save, disk_same: impl FnOnce() -> bool) -> Decision {
    let Some(l) = latest else { return Decision::Write };
    let (v, rev) = (l.v, l.rev());
    match &l.sitting {
        Some(sit) if sit.by == "you" && sit.session == s.session => {
            if s.seq <= sit.seq {
                return Decision::Already { v, rev };
            }
        }
        _ => {
            if s.base != v || s.rev != rev {
                return Decision::Stale { v, rev };
            }
        }
    }
    if !s.force && !disk_same() {
        return Decision::Disk { v, rev };
    }
    Decision::Write
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts_turns::Sitting;

    fn save(base: u64, rev: u32, session: &str, seq: u64) -> Save {
        Save { base, rev, session: session.into(), seq, force: false }
    }

    fn version(v: u64, revs: u32, sitting: Option<(&str, u64)>) -> Version {
        Version {
            v,
            revs: (1..=revs).map(|r| crate::artifacts_turns::Rev { rev: r, at_ms: 0, changed: None }).collect(),
            sitting: sitting.map(|(s, seq)| Sitting { by: "you".into(), session: s.into(), seq }),
            ..Default::default()
        }
    }

    /// The law: his base is the newest version, or the newest is his own
    /// sitting; a save of the sitting already in answers its version.
    #[test]
    fn a_save_writes_only_on_the_newest_version_or_his_own_sitting() {
        let same = || true;
        let agents = version(3, 2, None);
        assert_eq!(decide(Some(&agents), &save(3, 2, "s1", 1), same), Decision::Write);
        assert_eq!(decide(Some(&agents), &save(2, 1, "s1", 1), same), Decision::Stale { v: 3, rev: 2 }, "a version came in");
        assert_eq!(decide(Some(&agents), &save(3, 1, "s1", 1), same), Decision::Stale { v: 3, rev: 2 }, "the agent saved again in its turn");
        let his = version(4, 3, Some(("s1", 3)));
        assert_eq!(decide(Some(&his), &save(4, 2, "s1", 4), same), Decision::Write, "his own sitting: a stale rev is no conflict (a retry)");
        assert_eq!(decide(Some(&his), &save(4, 2, "s1", 3), same), Decision::Already { v: 4, rev: 3 }, "a retry of a save that landed");
        assert_eq!(decide(Some(&his), &save(4, 3, "s2", 1), same), Decision::Write, "another sitting on the newest version");
        assert_eq!(decide(Some(&his), &save(3, 1, "s0", 9), same), Decision::Stale { v: 4, rev: 3 }, "an older sitting after another saved");
        assert_eq!(decide(None, &save(1, 1, "s1", 1), same), Decision::Write);
    }

    #[test]
    fn a_file_changed_on_disk_is_a_conflict_unless_forced() {
        let v = version(2, 1, None);
        assert_eq!(decide(Some(&v), &save(2, 1, "s1", 1), || false), Decision::Disk { v: 2, rev: 1 });
        assert_eq!(decide(Some(&v), &Save { force: true, ..save(2, 1, "s1", 1) }, || false), Decision::Write);
        assert_eq!(decide(Some(&v), &Save { force: true, ..save(1, 1, "s1", 1) }, || false), Decision::Stale { v: 2, rev: 1 }, "force never skips the base");
    }

    /// A retry (the host's, the same seq) after the save landed: one
    /// version, no conflict (architect m_37387, m_59962 #1).
    #[test]
    fn a_retried_save_makes_one_version_and_no_conflict() {
        let d = std::env::temp_dir().join(format!("sb-save-retry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let work = d.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let f = work.join("q3.xlsx");
        std::fs::write(&f, b"PK agent").unwrap();
        let store = crate::artifacts::Store::new(&d.join("state"));
        let add = |sitting: Option<Sitting>, staged: Option<std::path::PathBuf>| crate::artifacts::Add { target: f.to_string_lossy().to_string(), agent: "deck".into(), by: "deck".into(), cwd: work.to_string_lossy().to_string(), sitting, staged, ..Default::default() };
        let id = store.add(&add(None, None), 10).unwrap().meta.id;
        // what daemon/view_files.rs does with a decision to write
        let write = |s: &Save, bytes: &[u8]| -> Decision {
            let m = store.get(&id).unwrap();
            let latest = m.current().cloned();
            let d = decide(latest.as_ref(), s, || crate::artifacts::file_sig(&f).as_deref() == latest.as_ref().map(|l| l.sig.as_str()));
            if d == Decision::Write {
                let staged = store.stage_copy(&id, bytes).unwrap();
                let p = bise_peer::fsx::prepare(&f, &mut &bytes[..]).unwrap();
                bise_peer::fsx::commit(p, None, None).unwrap();
                store.add(&add(Some(Sitting { by: "you".into(), session: s.session.clone(), seq: s.seq }), Some(staged)), 20).unwrap();
            }
            d
        };
        let first = save(1, 1, "s1", 1);
        assert_eq!(write(&first, b"PK his"), Decision::Write);
        assert_eq!(write(&first, b"PK his"), Decision::Already { v: 2, rev: 1 }, "the retry answers it");
        assert_eq!(write(&save(1, 1, "s1", 2), b"PK his, more"), Decision::Write, "his next save, on a base he never got: his sitting, no conflict");
        let m = store.get(&id).unwrap();
        assert_eq!((m.versions.len(), m.current().unwrap().rev()), (2, 2), "one version for the sitting");
        let copy = store.copy_bytes(&m, m.current().unwrap()).unwrap();
        assert_eq!(copy, b"PK his, more", "the staged copy is the version's");
        assert!(std::fs::read_dir(d.join("state/artifacts").join(&id)).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().starts_with(".staged-")), "no staged file left");
        let _ = std::fs::remove_dir_all(&d);
    }
}
