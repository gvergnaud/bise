//! The rows a hub lists: its agents and its open cards (the `agents` and
//! `cards` events; S1's `view.json` reuses them), and the split of a
//! question's trailing numbered options. One file per type family, all
//! re-exported here (`rows::Agent`, `rows::Card`, ...).

mod agents;
mod approvals;
mod artifacts;
mod cards;
mod models;
mod pages;
mod places;
mod scheduled;

pub use agents::*;
pub use approvals::*;
pub use artifacts::*;
pub use cards::*;
pub use models::*;
pub use pages::*;
pub use places::*;
pub use scheduled::*;

fn is_zero_u64(n: &u64) -> bool {
    *n == 0
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_words_map_to_statuses() {
        // the released meanings (the wire rule); the exact word is the phase
        assert_eq!(Status::of_hub("starting"), (Status::Working, false));
        assert_eq!(Status::of_hub("stopped"), (Status::Done, false));
        assert_eq!((Phase::of_hub("starting"), Phase::of_hub("stopped"), Phase::of_hub("working")), (Some(Phase::Starting), Some(Phase::Stopped), None));
        assert_eq!(Status::of_hub("archived"), (Status::Done, true));
        assert_eq!(Status::of_hub("idle"), (Status::Idle, false));
    }

    #[test]
    fn a_question_loses_its_options() {
        let (q, o) = question("keep the banner?\n1. yes\n2. no");
        assert_eq!((q.as_str(), o.len(), o[1].label.as_str(), o[1].n), ("keep the banner?", 2, "no", 2));
        let (q, o) = question("Que fais-tu ? 1. regarde le diff 2. arrête-le 3. laisse-le finir");
        assert_eq!((q.as_str(), o[2].label.as_str()), ("Que fais-tu ?", "laisse-le finir"));
        let (q, o) = question("how should agents ship?\n1 a PR per task\n2 straight to main");
        assert_eq!((q.as_str(), o[0].label.as_str()), ("how should agents ship?", "a PR per task"));
        assert_eq!(question("plain"), ("plain".to_string(), vec![]));
    }

    /// R9/S3: the hub model's waiting_on word, typed: 'you', an agent's
    /// name, none; a newer kind reads as Unknown.
    #[test]
    fn whom_an_agent_waits_on_is_typed() {
        assert_eq!(WaitingOn::of_word("you"), Some(WaitingOn::You));
        assert_eq!(WaitingOn::of_word("docs"), Some(WaitingOn::Agent { name: "docs".into() }));
        assert_eq!(WaitingOn::of_word(""), None);
        assert_eq!(serde_json::to_value(WaitingOn::You).unwrap(), serde_json::json!({"who": "you"}));
        assert_eq!(serde_json::to_value(WaitingOn::Agent { name: "docs".into() }).unwrap(), serde_json::json!({"who": "agent", "name": "docs"}));
        assert_eq!(serde_json::from_value::<WaitingOn>(serde_json::json!({"who": "a_review"})).unwrap(), WaitingOn::Unknown);
    }

    #[test]
    fn what_blocks_an_agent_ranks_first() {
        let order = ["confirm", "question", "merge", "signin", "blocked", "failed", "drop", "overlap", "done", "newer_kind", "setup"];
        let ranks: Vec<u8> = order.iter().map(|k| card_rank(k)).collect();
        assert_eq!(ranks, [0, 1, 1, 1, 2, 3, 4, 5, 6, 6, 7]);
        assert_eq!(card_rank("approval"), card_rank("confirm"));
        assert_eq!(card_rank("restart"), card_rank("failed"));
    }
}
