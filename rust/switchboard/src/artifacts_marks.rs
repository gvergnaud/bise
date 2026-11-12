//! Who an artifact is for, who kept it, when he opened it, and so the
//! section of the library it reads in (his pick D+C+A+B on page
//! artifacts-signal v3; design signed by architect m_60685, words by
//! designer m_60689). Pure: the store reads and writes the file
//! (crate::artifacts, `artifacts_keep.rs`), the daemon calls them.
//!
//! - **audience** (D): `you` or `agents`, a stored artifact's own fact
//!   (its meta.json), set at its add by [`audience_at_add`]; a bise page
//!   is always `you`.
//! - **marks** (A, B): `<state>/artifacts/marks.json`, [`Marks`], one file
//!   for stored artifacts and pages alike (a page's meta.json is the page
//!   store's): each id's `kept` (by `you` or an agent, [`keep`] and
//!   [`unkeep`] with his word first), `opened_ms` (his last open) and
//!   `seen_v` (the version he last saw), both only forward ([`opened`]).
//! - **done** (C), read at send time from other facts, never stored
//!   ([`done`]): its agent archived, the artifact archived (`sb artifact
//!   archive`, or the hub's archive at its PR's end), its PR merged or
//!   closed; a page with something waiting on him stays open.
//! - **section** ([`section`]): `kept`, `open`, `done` or `agents`, the
//!   hub's one decision; clients read it, never re-decide it. Inside a
//!   section a client orders by `touched_ms` ([`touched`]), newest first.
//!
//! What moves a row's section, so the hub sends the rows again
//! (daemon/art.rs): a keep or an unkeep (sb, the window), an add with
//! `--for-you`, an agent archived or unarchived (`art_who_key`), an
//! artifact archived or unarchived, a PR's state (`join_pr_status`), a
//! page's waiting items (the page store's stamp), and his open
//! (`opened_ms` moves `touched_ms`).
//!
//! The backfill ([`classify`], [`evergreen`]): artifacts-audit's rules
//! (title words, kind, who added it), run once per store and never
//! again (`backfilled` in marks.json).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Who an artifact is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    /// for him: a bise page, a draft that leaves, a deliverable he asked
    /// for, his own add, a PR, or `sb artifact add --for-you`
    You,
    /// what agents trade: shots, gates, QA, briefs
    Agents,
}

impl Audience {
    pub fn word(self) -> &'static str {
        match self {
            Audience::You => "you",
            Audience::Agents => "agents",
        }
    }
}

/// The section of the library a row reads in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Kept,
    Open,
    Done,
    Agents,
}

impl Section {
    pub fn word(self) -> &'static str {
        match self {
            Section::Kept => "kept",
            Section::Open => "open",
            Section::Done => "done",
            Section::Agents => "agents",
        }
    }
}

/// Who kept it and when: `you`, or the agent (its name when it kept it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub by: String,
    pub at_ms: u64,
}

impl Kept {
    pub fn his(&self) -> bool {
        self.by == YOU
    }
}

/// One artifact's marks. Unknown keys are kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<Kept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_ms: Option<u64>,
    /// the version he last saw (his open says it; plugin-apps' view/open
    /// reads what changed since; architect m_60735): only forward
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen_v: Option<u64>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

impl Mark {
    fn is_empty(&self) -> bool {
        self.kept.is_none() && self.opened_ms.is_none() && self.seen_v.is_none() && self.rest.is_empty()
    }
}

/// The version of marks.json this hub writes; a newer one is never
/// written over.
pub const VERSION: u64 = 1;
/// The backfill's version, in `backfilled` once it ran.
pub const BACKFILL: u64 = 1;
/// `by` for his own keep.
pub const YOU: &str = "you";

/// `<state>/artifacts/marks.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marks {
    #[serde(default = "version")]
    pub v: u64,
    /// the backfill ran (its version): it never runs again, even when he
    /// lets every kept one go
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backfilled: Option<u64>,
    #[serde(default)]
    pub marks: BTreeMap<String, Mark>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

fn version() -> u64 {
    VERSION
}

impl Default for Marks {
    fn default() -> Marks {
        Marks { v: VERSION, backfilled: None, marks: BTreeMap::new(), rest: Map::new() }
    }
}

impl Marks {
    pub fn get(&self, id: &str) -> Option<&Mark> {
        self.marks.get(id)
    }

    pub fn kept(&self, id: &str) -> Option<&Kept> {
        self.get(id)?.kept.as_ref()
    }

    pub fn opened_ms(&self, id: &str) -> Option<u64> {
        self.get(id)?.opened_ms
    }

    pub fn seen_v(&self, id: &str) -> Option<u64> {
        self.get(id)?.seen_v
    }

    /// A newer hub's file: read, never written over.
    pub fn writable(&self) -> bool {
        self.v <= VERSION
    }

    fn tidy(&mut self, id: &str) {
        if self.marks.get(id).is_some_and(Mark::is_empty) {
            self.marks.remove(id);
        }
    }
}

/// Who keeps or lets go: him, or an agent by its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who<'a> {
    You,
    Agent(&'a str),
}

impl Who<'_> {
    fn by(&self) -> String {
        match self {
            Who::You => YOU.into(),
            Who::Agent(a) => (*a).to_string(),
        }
    }
}

/// What a keep or an unkeep did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// changed
    Changed,
    /// already so: nothing written
    Same,
    /// an agent's unkeep of his keep, or an agent's keep over his: his
    /// word stands
    His,
}

/// Keep `id`: his keep takes it whoever kept it before; an agent's keep
/// never writes over his (`His`), and keeping it again is `Same`.
pub fn keep(m: &mut Marks, id: &str, who: Who, now: u64) -> Outcome {
    let mark = m.marks.entry(id.to_string()).or_default();
    let out = match (&mark.kept, who) {
        (Some(k), Who::You) if k.his() => Outcome::Same,
        (Some(k), Who::Agent(_)) if k.his() => Outcome::His,
        (Some(_), Who::Agent(_)) => Outcome::Same,
        _ => {
            mark.kept = Some(Kept { by: who.by(), at_ms: now });
            Outcome::Changed
        }
    };
    m.tidy(id);
    out
}

/// Let `id` go: he lets any keep go; an agent only its own or another
/// agent's, never his (`His`); not kept: `Same`.
pub fn unkeep(m: &mut Marks, id: &str, who: Who) -> Outcome {
    let Some(mark) = m.marks.get_mut(id) else { return Outcome::Same };
    let out = match (&mark.kept, who) {
        (None, _) => Outcome::Same,
        (Some(k), Who::Agent(_)) if k.his() => Outcome::His,
        _ => {
            mark.kept = None;
            Outcome::Changed
        }
    };
    m.tidy(id);
    out
}

/// He opened `id` at `at`, seeing version `v` (none: it did not say):
/// each only forward (an older open or version never moves one back).
/// True when one moved.
pub fn opened(m: &mut Marks, id: &str, at: u64, v: Option<u64>) -> bool {
    let mark = m.marks.entry(id.to_string()).or_default();
    let at_moved = mark.opened_ms.is_none_or(|o| o < at);
    if at_moved {
        mark.opened_ms = Some(at);
    }
    let v_moved = v.is_some_and(|v| mark.seen_v.is_none_or(|s| s < v));
    if v_moved {
        mark.seen_v = v;
    }
    at_moved || v_moved
}

/// What `sb artifact keep|unkeep` says for one artifact (designer m_60689:
/// to agents, the user is "the user"): `agent` the one asking, `by` who
/// keeps it now.
pub fn keep_text(title: &str, agent: &str, keep: bool, o: &Outcome, by: Option<&str>) -> String {
    match (keep, o) {
        (true, Outcome::Changed) => format!("kept: {title} · its card says \"kept by {agent}\"; the user can let it go"),
        (true, Outcome::Same) => format!("{title} is kept already (by {})", by.unwrap_or(agent)),
        (_, Outcome::His) => format!("{title} is kept by the user: only they can let it go"),
        (false, Outcome::Changed) => format!("let go: {title} · it goes to Done when its task ends"),
        (false, Outcome::Same) => format!("{title} is not kept"),
    }
}

/// A new artifact's audience, or a next version's: `--for-you`, his own
/// add (`by` = `you`), main's (the agent he talks to: what it adds is for
/// him; architect m_60955) and a PR (it waits on his merge) are for him;
/// the rest is between agents. A next version keeps what it was, and
/// `--for-you` only raises it, never lowers it.
pub fn audience_at_add(before: Option<Audience>, by: &str, kind: &str, for_you: bool) -> Audience {
    if for_you || by == YOU || by == crate::model::MAIN || kind == "pr" || before == Some(Audience::You) {
        Audience::You
    } else {
        Audience::Agents
    }
}

/// What a row's `done` reads: the facts of its end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ending {
    /// its agent (task) is archived
    pub agent_archived: bool,
    /// the artifact itself is archived (his `sb artifact archive`, or the
    /// hub's at its PR's end)
    pub archived: bool,
    /// its PR is merged or closed (the hub's PR row)
    pub pr_ended: bool,
    /// a page with something waiting on him (a question, a draft, a step
    /// of his): it stays open work
    pub waiting: bool,
}

/// Done when its task or its PR ended, unless something on it waits on him.
pub fn done(e: Ending) -> bool {
    !e.waiting && (e.agent_archived || e.archived || e.pr_ended)
}

/// The section: kept first (whatever its audience: an agent's keep shows
/// in his Kept, and he can let it go), then between agents, then done,
/// else open work.
pub fn section(audience: Audience, kept: bool, done: bool) -> Section {
    if kept {
        Section::Kept
    } else if audience == Audience::Agents {
        Section::Agents
    } else if done {
        Section::Done
    } else {
        Section::Open
    }
}

/// When it was last touched: its newest version, his last open, a note
/// on it, whichever came last.
pub fn touched(version_ms: u64, opened_ms: Option<u64>, note_ms: Option<u64>) -> u64 {
    version_ms.max(opened_ms.unwrap_or(0)).max(note_ms.unwrap_or(0))
}

// The backfill's words (artifacts-audit's cls2.py, page artifacts-signal
// v3): a stored artifact is for him when its title says a draft, a film,
// a proto, a deck or a doc he asked for, and not a shot or a check.
const FOR_YOU_WORDS: &[&str] = &[
    "draft", "brouillon", "prototype", "preview", "film", "pitch", "readme", "note for", "plan q3", "projections",
    "learnings", "carnet", "démo", "landing", "bise.dev", "maquettes", "deck", ".pptx", ".xlsx", "easter eggs",
    "scénarios", "hero loop",
];
// matched as written (case kept): 'QA' in a title is a check, 'qa' inside a word is not
const AGENTS_WORDS: &[&str] = &["shot", "check", "QA"];
// what outlives its task (kept by itself at the backfill)
const EVERGREEN_WORDS: &[&str] = &[
    "learnings", "carnet", "changelog", "architecture", "prd", "pitch", "projections", "readme", "easter", "scénarios",
    "dashboard", "tableau de bord", "about", "contract", "design system", "tokens", "prototype", "hero loop",
    "notebook", "docs de bise",
];

/// The backfill's audience for a stored artifact from before the field:
/// his own add and a PR are for him; else its title's words.
pub fn classify(title: &str, kind: &str, by: &str) -> Audience {
    let low = title.to_lowercase();
    let for_you = FOR_YOU_WORDS.iter().any(|w| low.contains(w)) && !AGENTS_WORDS.iter().any(|w| title.contains(w));
    audience_at_add(None, by, kind, for_you)
}

/// The backfill keeps what outlives its task: a learnings notebook, the
/// changelog, a PRD, the architecture, the design system, a pitch, a film
/// (its title or id; `ds` as a word).
pub fn evergreen(title: &str, id: &str) -> bool {
    let low = format!("{} {}", title, id).to_lowercase();
    EVERGREEN_WORDS.iter().any(|w| low.contains(w)) || low.split(|c: char| !c.is_alphanumeric()).any(|w| w == "ds")
}

#[cfg(test)]
#[path = "artifacts_marks_tests.rs"]
mod tests;
