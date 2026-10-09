//! While you were away (S9, amb-mac m_9038, shape agreed m_9043): the
//! app's `away_back {away_ms}` gets `away_summary`, counted from the rows
//! the core has (a held hub's own, else its `view.json`). Done and failed:
//! agents (not a main, not archived) whose status turned so at or after
//! `since_ms` (their `since_ms`); questions: his open cards that came
//! since (approvals included).
//!
//! Its limit: only an agent's current status is known, so one that went
//! done then working again while he was away isn't counted. When S10's
//! `job_end` lands, a followed agent's end is journaled: count those from
//! the jobs then, exactly, instead of from the current status.

use super::Core;
use bise_proto::draft::{AwayProject, CoreEv};
use bise_proto::rows::{Agent, Card, Status};

/// The summary since `since_ms` of each project's (id, agents, cards), in
/// his projects' order; a project where nothing happened is left out.
pub fn summary(since_ms: u64, projects: &[(String, Vec<Agent>, Vec<Card>)]) -> CoreEv {
    let turned = |agents: &[Agent], s: &[Status]| {
        agents.iter().filter(|a| !a.main && !a.archived && s.contains(&a.status) && a.since_ms >= since_ms).count() as u32
    };
    let rows: Vec<AwayProject> = projects
        .iter()
        .map(|(id, agents, cards)| AwayProject {
            project: id.clone(),
            // stopped by him counts as done (Status says done, its phase
            // stopped)
            done: turned(agents, &[Status::Done]),
            questions: cards.iter().filter(|c| c.since_ms >= since_ms).count() as u32,
            failed: turned(agents, &[Status::Failed]),
        })
        .filter(|p| p.done + p.questions + p.failed > 0)
        .collect();
    CoreEv::AwaySummary {
        since_ms,
        done: rows.iter().map(|p| p.done).sum(),
        questions: rows.iter().map(|p| p.questions).sum(),
        failed: rows.iter().map(|p| p.failed).sum(),
        projects: rows,
    }
}

impl Core {
    /// `away_back {away_ms}`: `away_summary` since then.
    pub(super) fn away_back(&mut self, away_ms: u64) {
        let since = super::now_ms().saturating_sub(away_ms);
        let ev = summary(since, &self.project_rows());
        self.emit(serde_json::to_value(&ev).unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn agent(name: &str, status: &str, since_ms: u64) -> Agent {
        serde_json::from_value(json!({"name": name, "main": name == "main", "status": status, "archived": false, "title": "", "purpose": "", "since_ms": since_ms, "waits": 0})).unwrap()
    }

    fn card(id: u64, kind: &str, since_ms: u64) -> Card {
        serde_json::from_value(json!({"id": id, "project": "p", "kind": kind, "agent": "a", "question": "?", "options": [], "urgent": false, "since_ms": since_ms})).unwrap()
    }

    /// Law: only what turned done or failed since, and the cards that came
    /// since, count; a main, an archived agent and anything older don't;
    /// a project where nothing happened is left out; the totals are the
    /// rows' sums.
    #[test]
    fn away_counts_what_happened_since() {
        let mut archived = agent("old", "done", 2_000);
        archived.archived = true;
        let acme = (
            "acme-1a2b3c4d".to_string(),
            vec![agent("main", "done", 2_000), agent("perf", "done", 2_000), agent("ci", "failed", 1_500), agent("older", "done", 900), archived, agent("busy", "working", 2_000)],
            vec![card(1, "question", 1_200), card(2, "merge", 1_000), card(3, "question", 500)],
        );
        let quiet = ("docs-00000000".to_string(), vec![agent("d", "done", 10)], vec![card(4, "question", 20)]);
        let shop = ("shop-0badc0de".to_string(), vec![agent("checkout", "done", 1_000)], vec![]);
        let ev = summary(1_000, &[acme, quiet, shop]);
        let want = json!({"ev": "away_summary", "since_ms": 1000, "done": 2, "questions": 2, "failed": 1, "projects": [
            {"project": "acme-1a2b3c4d", "done": 1, "questions": 2, "failed": 1},
            {"project": "shop-0badc0de", "done": 1, "questions": 0, "failed": 0},
        ]});
        assert_eq!(serde_json::to_value(&ev).unwrap(), want);
        let none = summary(5_000, &[("a".into(), vec![agent("x", "done", 10)], vec![])]);
        assert_eq!(serde_json::to_value(&none).unwrap(), json!({"ev": "away_summary", "since_ms": 5000, "done": 0, "questions": 0, "failed": 0, "projects": []}));
    }
}
