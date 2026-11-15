//! The hub's client snapshot, typed (issue 30): what `core::Hub::snapshot`
//! returns (built in core_snapshot.rs, its agent rows in snapshot_rows.rs)
//! and what the views read by field: the typed rows (proto_view), the
//! growth rows, view.json.
//!
//! The hub's view, not wire, and never sent: the wire rows stay
//! bise_proto's (`rows::Agent`, `rows::Card`...), mapped from these by
//! proto_view. The hub builds the core of it (agents, cards, places,
//! flow); the shell fills the rest, one writer per area (daemon.rs
//! `snapshot`: the pages, the cards' page links, the agents' models).

use crate::card_link::PageLink;
use crate::flow::FlowMode;
use crate::model::{Declared, Mode, Status};
use crate::pages::questions::Pick;
use crate::place::PlaceView;
use bise_proto::draft::Queued;
use bise_proto::rows::{AgentWatch, ArtLink, CardBatch, Changes, Doing, OptAct, Page};
use std::collections::BTreeMap;

/// The hub's state for the views: its agents, its open cards, its
/// places, the repo's flow.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// every agent, archived ones included, in the hub's order
    pub agents: Vec<AgentRow>,
    /// the open cards, oldest first
    pub cards: Vec<CardRow>,
    /// the worktrees the views draw (`place::views`)
    pub places: Vec<PlaceView>,
    pub flow: Option<FlowMode>,
    /// the shell's: the pages (docs/ambient-pages.md §2.3), newest first
    pub pages: Vec<Page>,
    /// the shell's: his late promises (checklist rows of his past
    /// data-due), a count for the morning page and the menu bar
    pub overdue: u32,
    /// tasks: every task sb-core keeps with its status, oldest first, and
    /// the folded ones' tombs (task_list::rows; the `works` event)
    pub works: Vec<bise_proto::rows::WorkRow>,
    pub tombs: Vec<bise_proto::rows::WorkTomb>,
}

/// One agent as the views see it (snapshot_rows.rs builds it).
#[derive(Clone, Debug, PartialEq)]
pub struct AgentRow {
    pub name: String,
    pub main: bool,
    pub status: Status,
    pub objective: String,
    /// what it is doing now, one line (BISE-126); "" for main
    pub role: String,
    pub parent: Option<String>,
    pub mode: Mode,
    pub path: String,
    pub branch: Option<String>,
    /// BISE-136: a private worktree (gate.sh new), else null
    pub place: Option<String>,
    /// dev-flow §3.1: the id of the place it is in (`places`) (BISE-136:
    /// its private worktree's, `pt:<path>`; a feature's agent: its
    /// feature's, dev-flow §5.1)
    pub place_id: String,
    pub created_ms: u64,
    /// its folder under agents/ and its former names (S2 C: view.json
    /// carries them for sb history --project)
    pub dir: String,
    pub aliases: Vec<String>,
    pub note: String,
    /// the status it declared (done, blocked, working; null: none): les
    /// bims' growth counts the done ones, archived too (the archive keeps
    /// it; an auto turn report replaces report_kind)
    pub declared: Option<Declared>,
    pub report: Option<String>,
    /// when it last reported (an archived task: about when it stopped)
    pub report_ms: Option<u64>,
    /// S10: followed, and its progress step (`--step n/m`)
    pub follow: bool,
    pub report_kind: Option<String>,
    pub step: Option<u32>,
    pub of: Option<u32>,
    /// messages waiting for its delivery
    pub queued: usize,
    /// the user's queued inputs waiting for the turn's end (the window's
    /// "send queued")
    pub queued_inputs: Vec<Queued>,
    /// BISE-299: main's inbox, the agents' questions waiting for main
    /// (the user sees a quiet count, never asked); 0 for a task
    pub inbox: usize,
    pub turn_ms: Option<u64>,
    /// issue 22: its ended turns, from the same sb-core view as its
    /// status
    pub turns: u64,
    /// who it waits on (`sb wait` / `sb ask`), for `waits {name}`
    pub waiting_on: Option<String>,
    /// event-wake: the events it waits for (wake-ui)
    pub watching: Vec<AgentWatch>,
    /// les bims: what it is doing now, only while its turn runs (or
    /// paused on a backgrounded command: a build)
    pub doing: Option<Doing>,
    /// its running processes and their lowest port (`Hub.procs`)
    pub procs: (u16, Option<u16>),
    /// the shell's (issue #4): its model, for sb list / sb tasks too
    pub model: Option<Model>,
}

/// An agent's model as the shell knows it (the catalog's choice).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub name: String,
    /// "" when the model takes no effort
    pub effort: String,
    pub efforts: Vec<String>,
    /// the "± 9 files so far" door (docs/artifacts.md): its changes
    /// against its base, none when not measured
    pub changes: Option<Changes>,
}

/// One open card as the views see it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CardRow {
    pub id: u64,
    pub kind: String,
    pub agent: String,
    pub text: String,
    pub for_msg: Option<u64>,
    pub age_ms: u64,
    /// what the view adds under it (a merge's, an update's, a sign-in's,
    /// a question maybe answered another way)
    pub note: Option<String>,
    /// expired-ux: the agents that wait for the sign-in (⏎ in their
    /// thread signs in); a sign-in card only
    pub waiting: Option<Vec<String>>,
    /// the update card's release page (its changelog's full notes link)
    pub link: Option<String>,
    /// the update card's release and changelog (release_line.rs)
    pub release: Option<bise_proto::rows::ReleaseNotes>,
    /// a hub item's place and PR (pr-merge): the TUI ties it to the
    /// place's box and opens its link
    pub place: Option<String>,
    pub pr: Option<u64>,
    /// the shell's (daemon/page_cards.rs): the page it asks on, what it
    /// is about, the options that open an artifact, a drafts batch's
    /// fields, a drawn pick's why lines
    pub page: Option<PageLink>,
    pub artifact: Option<ArtLink>,
    pub opens: BTreeMap<u32, OptAct>,
    pub batch: Option<CardBatch>,
    pub pick: Option<Pick>,
    /// the hub carries out his answer to it now (`Hub::running_cards`)
    pub running: bool,
    /// an agent's blocked report (tasks): his card whatever its kind
    /// (model::user_card)
    pub from_report: bool,
}

#[cfg(test)]
impl Snapshot {
    /// The row of `name` (a test's lookup).
    pub fn agent(&self, name: &str) -> &AgentRow {
        self.agents.iter().find(|a| a.name == name).unwrap_or_else(|| panic!("no row {name}"))
    }

    /// The first open card of `kind`.
    pub fn card_of(&self, kind: &str) -> Option<&CardRow> {
        self.cards.iter().find(|c| c.kind == kind)
    }

    /// The place `id` as its view's JSON (`null`: none).
    pub fn place_json(&self, id: &str) -> serde_json::Value {
        self.places.iter().find(|p| p.id == id).map_or(serde_json::Value::Null, |p| serde_json::to_value(p).unwrap())
    }
}

#[cfg(test)]
impl AgentRow {
    /// A task's row with nothing in it but its name (tests build the
    /// rest with `..AgentRow::test(name)`).
    pub fn test(name: &str) -> AgentRow {
        AgentRow {
            name: name.into(),
            main: false,
            status: Status::Idle,
            objective: String::new(),
            role: String::new(),
            parent: None,
            mode: Mode::Shared,
            path: String::new(),
            branch: None,
            place: None,
            place_id: String::new(),
            created_ms: 0,
            dir: name.into(),
            aliases: Vec::new(),
            note: String::new(),
            declared: None,
            report: None,
            report_ms: None,
            follow: false,
            report_kind: None,
            step: None,
            of: None,
            queued: 0,
            queued_inputs: Vec::new(),
            inbox: 0,
            turn_ms: None,
            turns: 0,
            waiting_on: None,
            watching: Vec::new(),
            doing: None,
            procs: (0, None),
            model: None,
        }
    }
}
