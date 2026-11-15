//! The land record (architect m_29755, item D; land-revert-guard m_42458):
//! one typed line per land that moved a ref, in the hub's `lands.jsonl`
//! ([`crate::paths::Paths::lands`]). The land's own thread appends it
//! (`daemon/land.rs`), after the swap, under one lock: the hub is its
//! only writer. Read by the revert guard ([`crate::land_revert`]): a
//! commit is an agent's own when one of its records holds it.
//! `daemon/merged.rs`'s `by` still reads main's feed; it moves onto this
//! record next (its TODO).

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

/// One land: who, from which place, onto which ref, the range it brought
/// (`base..tip`, full shas) and its commits (newest first, `rev-list`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LandRecord {
    pub agent: String,
    pub place: String,
    pub target: String,
    pub base: String,
    pub tip: String,
    pub commits: Vec<String>,
    pub at_ms: u64,
    /// tasks-roma (architect m_65317): the works the lander's turn served
    /// (`land::Job.works`), the task Log's typed land fact; none on an
    /// older record
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub works: Vec<String>,
}

/// The record of `job`'s land `o` (its commits: `git rev-list base..tip`).
pub fn of(job: &crate::land::Job, o: &crate::land::Outcome, at_ms: u64) -> LandRecord {
    let range = format!("{}..{}", o.base, o.tip);
    let commits = crate::land::git(&job.shared, &["rev-list", &range]).unwrap_or_default().lines().map(str::to_string).collect();
    LandRecord {
        agent: job.agent.clone(),
        place: job.place.clone(),
        target: o.target.clone(),
        base: o.base.clone(),
        tip: o.tip.clone(),
        commits,
        at_ms,
        works: job.works.clone(),
    }
}

static WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Append `rec` as one line to `path` (made when missing).
pub fn append(path: &Path, rec: &LandRecord) -> std::io::Result<()> {
    let _held = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut line = serde_json::to_string(rec).map_err(std::io::Error::other)?;
    line.push('\n');
    std::fs::OpenOptions::new().create(true).append(true).open(path)?.write_all(line.as_bytes())
}

/// Every record of `path` (none: no file); a line that is not one (cut by
/// a crash) is skipped.
pub fn read(path: &Path) -> Vec<LandRecord> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// The agent whose land brought `sha` (a full sha), the newest record first.
pub fn landed_by<'a>(records: &'a [LandRecord], sha: &str) -> Option<&'a str> {
    records.iter().rev().find(|r| r.commits.iter().any(|c| c == sha)).map(|r| r.agent.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(agent: &str, commits: &[&str]) -> LandRecord {
        LandRecord {
            agent: agent.into(),
            place: format!("wt:{}", agent),
            target: "main".into(),
            base: "b".into(),
            tip: commits.first().unwrap_or(&"b").to_string(),
            commits: commits.iter().map(|s| s.to_string()).collect(),
            at_ms: 1,
            works: Vec::new(),
        }
    }

    #[test]
    fn records_append_read_back_and_name_who_landed_a_commit() {
        let d = std::env::temp_dir().join(format!("sb-land-record-{}-{}", std::process::id(), crate::util::now_ms()));
        let p = d.join("lands.jsonl");
        assert!(read(&p).is_empty(), "no file: no record");
        append(&p, &rec("y", &["c2", "c1"])).unwrap();
        std::fs::OpenOptions::new().append(true).open(&p).unwrap().write_all(b"{\"cut").unwrap();
        std::fs::OpenOptions::new().append(true).open(&p).unwrap().write_all(b"\n").unwrap();
        append(&p, &rec("x", &["c3"])).unwrap();
        let all = read(&p);
        assert_eq!(all, vec![rec("y", &["c2", "c1"]), rec("x", &["c3"])], "the cut line is skipped");
        assert_eq!(landed_by(&all, "c1"), Some("y"));
        assert_eq!(landed_by(&all, "c3"), Some("x"));
        assert_eq!(landed_by(&all, "c9"), None);
        let _ = std::fs::remove_dir_all(d);
    }
}
