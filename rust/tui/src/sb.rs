//! Switchboard mode of the TUI (docs/, RFC 0001 §6 and
//! ux-notes.md): one feed per agent, main in focus by default, a task
//! panel on the right, checkout / Esc, preview, attention cards.
//!
//! The feeds are the wire lines of each agent's REPL (the hub relays
//! them), so every event renders with the code of lib.rs.
//! The focused feed lives in the `App` fields; the other feeds wait in
//! `Sb::views` and are swapped in on focus change.

use super::*;
use bise_proto::thread::lines::{self, GateStep, Hub};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::os::unix::net::UnixStream;

mod versions;
pub(super) use versions::{version_choices, versions_dev};
use versions::{parse_versions, VersionItem};
mod mention;
pub(crate) mod palette;
pub(super) use mention::mentions;
mod cards;
pub(super) use cards::{card_choices, card_mouse, proves_ctrl_digits};
pub(crate) use cards::{answer_by_voice, show_heard, voice_question};
pub(crate) use cards::{ambient_local, ambient_options, ambient_reply, split_choices};
use cards::{Card, CardView};
mod card_draw;
pub(super) use card_draw::{box_height, card_frame, BoxFit, card_view_open, divider_label as card_divider_label, draw_box, draw_view as draw_card_view, fit_pairs as fit_card_pairs, key_pairs as card_key_pairs};
mod panel;
pub(crate) mod places;
#[cfg(test)]
mod tour_tests;
#[cfg(test)]
mod places_tests;
pub(super) use panel::PANEL_TITLE;
pub(super) use panel::{archived_refusal, archived_warn, draw_panel, focus_model, key_mode, panel_mouse, placeholder, split, status_state, viewed_model, viewed_who, viewed_working, workspace};
#[cfg(test)]
pub(super) use panel::status_text;
pub(crate) use panel::{demo_ready, fill_demo, prefill_demo, short_age, DEMO, DEMO_READY};
use panel::glyph;
mod feed;
pub(super) use feed::FeedWindow;
mod client;
pub(crate) mod setup;
mod tune;
pub(crate) mod drafts;
mod keep;
#[cfg(test)]
mod keep_tests;
pub(crate) mod reload_wait;
mod keys;
pub(crate) mod rpc;
pub(crate) mod hub_reads;
pub(crate) mod release;
pub(super) use keys::key;
pub(crate) use keys::{scene, Scene};
#[cfg(test)]
use keys::{nav_key, Nav};
pub use client::{run_switchboard, take_reexec, take_refused};
pub use hub_reads::hello_line;
pub use tune::setup_main;
use client::{follow_hub_exe, follow_reload, HUB_DOWN, HUB_UP};
#[cfg(test)]
use client::{new_sb, sb_app};

/// What the ctrl hints read of the switchboard (ctrlhint.rs): the
/// agents, the open cards, the card view open or not.
pub(crate) struct CtrlView {
    pub(crate) agents: usize,
    pub(crate) cards: usize,
    pub(crate) card_open: bool,
}

/// The inbox's rows (BISE-302: ctrl+1 to ctrl+N open them).
pub(crate) fn strip_rows(app: &App) -> usize {
    card_draw::strip_ids(&app.sb).len()
}

/// What the terminal's tab title says (termtitle.rs): the workspace's
/// folder, the inbox's cards, the new artifacts, the agents at work
/// (working or waiting on another agent; main and archived left out).
pub(crate) fn title_status(app: &App) -> crate::termtitle::Status {
    let sb = &app.sb;
    crate::termtitle::Status {
        repo: crate::termtitle::repo_name(&sb.workspace),
        inbox: sb.sorted_cards().len(),
        running: sb.agents.iter().filter(|a| !a.main && a.busy()).count(),
    }
}

pub(crate) fn ctrl_view(app: &App) -> CtrlView {
    let sb = &app.sb;
    CtrlView {
        agents: sb.nav().len(),
        cards: sb.sorted_cards().len(),
        card_open: sb.card.open,
    }
}
use feed::{
    clear_feed, ingest_at, prepend_page, swap_draft, swap_feed, trim_window, want_older, with_feed, View,
};

#[derive(Clone, Default)]
pub(super) struct Agent {
    name: String,
    main: bool,
    status: String,
    objective: String,
    mode: String,
    branch: Option<String>,
    path: String,
    note: String,
    queued: u64,
    /// BISE-299: main's inbox, the agents' questions waiting for main (a
    /// quiet count on main's row: the user is not asked)
    inbox: u64,
    turn_ms: Option<u64>,
    /// When `turn_ms` came: the turn's age moves on between two states.
    turn_seen: Option<std::time::Instant>,
    /// The last report, one line, and when it came (for an archived
    /// task: what it did, and about when it stopped).
    report: String,
    report_ms: Option<u64>,
    /// What it is doing now, one line (BISE-126): the hub's role line, ""
    /// for main.
    role: String,
    created_ms: u64,
    /// Who it waits on (`sb wait` / `sb ask`), "" when no one.
    waiting_on: String,
    /// Its folder under the hub's `agents/` (its name but after a rename):
    /// its computer-use key's second half (docs/issues/18)
    dir: String,
    /// BISE-136: the private git worktree it works in (`gate.sh new`),
    /// "" when it works in its own workspace.
    place: String,
    /// dev-flow §3.1: the id of its place (`shared`, `wt:<dir>`), the
    /// box it sits in when `places` has it ("" from an older hub)
    place_id: String,
    /// The model it runs (the full `provider/id`) and its reasoning
    /// effort ("" = the model takes none), as the hub resolves them
    /// (BISE-135: its `/model`, `/reasoning` choice first); the efforts
    /// its model takes (the `/reasoning` list).
    model: String,
    effort: String,
    efforts: Vec<String>,
    /// site/m/artifacts D: what it changed against main (files, +, −),
    /// None when the hub does not know (the `± 9 files` door, the live
    /// diff panel)
    changes: Option<(u64, u64, u64)>,
}

impl Agent {
    fn archived(&self) -> bool {
        self.status == "archived"
    }

    /// When it was last heard of: its last report, else its creation.
    fn last_ms(&self) -> u64 {
        self.report_ms.unwrap_or(self.created_ms)
    }

    /// The current turn's age now: the hub's `turn_ms` plus the time
    /// since it came.
    fn turn_age_ms(&self) -> Option<u64> {
        let since = self.turn_seen.map_or(0, |t| t.elapsed().as_millis() as u64);
        self.turn_ms.map(|ms| ms.saturating_add(since))
    }

    /// In a turn (its feed shows the spinner).
    fn busy(&self) -> bool {
        self.status == "working" || self.status == "waiting"
    }
}

pub(super) struct Sb {
    /// Shared with the reader thread, which swaps in a fresh stream when
    /// it reconnects after the hub went away (a hub restart, a switch
    /// of version).
    writer: std::sync::Arc<std::sync::Mutex<UnixStream>>,
    workspace: String,
    pub(super) focus: String,
    views: HashMap<String, View>,
    agents: Vec<Agent>,
    /// The worktrees (pr-design §4.1: a section in the panel when agents
    /// share one, else a solo row's mark), in their first agent's order.
    places: Vec<places::Place>,
    /// The repo's flow, `pr` or `trunk` ("" until the hub says): the
    /// header's held `lands via PRs` (dev-flow §7).
    flow: String,
    cards: Vec<Card>,
    /// dev-flow §5.1: the feature merge item asking `drop it?` once more
    /// (its `3 drop the branch` was picked), by card id.
    feature_drop_ask: Option<u64>,
    /// Index in `nav()` of the highlighted entry of the panel.
    selected: Option<usize>,
    preview: bool,
    confirm: Option<(u64, String)>,
    /// `/release-bise`'s plan waiting for your y/n (BISE-235).
    release_ask: Option<release::Ask>,
    /// The release running: its tag, since when (the header's item).
    release: Option<(String, std::time::Instant)>,
    /// `/update` building HEAD in the dev build (dev-update): its short
    /// hash, since when (the header's item when no release runs).
    updating: Option<(String, std::time::Instant)>,
    /// `D` on an agent asks first (book §16): the agent to drop on `y`.
    drop_ask: Option<String>,
    /// The feeds out of view where lines arrived since their last visit.
    activity: std::collections::HashSet<String>,
    ready: bool,
    /// The version the hub runs (its VERSION id), for the status row.
    version: String,
    /// The `/version` picker (the hub's `versions` event), and when it
    /// was last asked for.
    versions: Vec<VersionItem>,
    /// The hub's workspace is bise's source tree (its `versions` event;
    /// None before the first one, or from an older hub).
    versions_dev: Option<bool>,
    versions_asked: std::cell::Cell<Option<std::time::Instant>>,
    /// The reload id of the first hub this TUI met (None before its
    /// first hello): a hub with another one was started by a reload
    /// (BISE-131), which this TUI follows by re-executing itself.
    reload_seen: Option<String>,
    /// A reload asked by the hub, waiting for the keys to stop
    /// (keep-state, sb/reload_wait.rs).
    pub(crate) reload_wait: reload_wait::Wait,
    /// The strip and the card view (ctrl+1-9, a click), never opened by the hub.
    card: CardView,
    /// Set by the last draw: the panel rows and their agents (clicks).
    panel_hits: std::cell::RefCell<panel::PanelHits>,
    /// The archived section of the panel is expanded (`A`, a click on
    /// its header, `/archived`).
    archived_open: bool,
    /// Things that asked for you since the start (a new card, a message
    /// to you, a confirm): a change leaves zen (BISE-121).
    calls: u64,
    /// The setup card and its offers (BISE-245), the TUI's own cards.
    setup: setup::Setup,
    /// The approvals mode, its checker and saved rules (the hub's
    /// `approvals` event, approvals-design.md §8).
    pub(crate) approvals: Approvals,
    /// The scheduled tasks (sb every), active then a week of ended ones
    /// (the state's `timers`): /scheduled and the panel's ◷ next run.
    pub(crate) timers: Vec<crate::scheduled::Task>,
    /// The JSON-RPC requests waiting for their answer (sb/rpc.rs).
    rpc: rpc::Calls,
}

/// What the hub says of approvals: the global mode (`yolo` / `auto`),
/// whether `BISE_APPROVALS` set it, which checker, the saved rules.
#[derive(Clone, Debug, Default)]
pub(crate) struct Approvals {
    pub(crate) mode: String,
    pub(crate) env: bool,
    /// `jev`, `model` or `off`
    pub(crate) checker: String,
    /// who checks: `TypeSafe`, `OpenRouter`, a chat model's id
    pub(crate) checker_who: String,
    /// Jev's model id (`jev-1.13`), "" for the others
    pub(crate) checker_model: String,
    /// the workspace's git common root: the rules below are its own
    pub(crate) repo: String,
    /// the saved rules of this repo and of every project (`/approvals`)
    pub(crate) rules: Vec<Rule>,
    /// a switch: the key bar's 3-second flash since then
    pub(crate) flash: Option<std::time::Instant>,
}

/// One saved rule of `~/.bise/approvals.toml`, as the hub sent it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Rule {
    /// the hub's fields as they came: `remove_rule` sends them back
    pub(crate) raw: Value,
    pub(crate) tool: String,
    pub(crate) pattern: String,
    pub(crate) path: String,
    /// no project: it applies in every repo
    pub(crate) every: bool,
    /// when "always" saved it (ms), when the file says
    pub(crate) added: Option<u64>,
    /// `card #12, api-v2`
    pub(crate) from: String,
    /// `sandbox = false`: its commands run outside the sandbox
    pub(crate) outside: bool,
}

impl Rule {
    /// Its facts for the words (bise_proto::approvals).
    pub(crate) fn facts(&self) -> bise_proto::approvals::RuleFacts<'_> {
        bise_proto::approvals::RuleFacts {
            tool: &self.tool,
            pattern: &self.pattern,
            path: &self.path,
            from: &self.from,
            every: self.every,
            outside: self.outside,
        }
    }
}

impl Approvals {
    /// The mode word, `yolo` until the hub said.
    pub(crate) fn word(&self) -> &str {
        if self.mode.is_empty() { "yolo" } else { &self.mode }
    }
}

/// The string field `k` of `v` ("" when absent).
fn str_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

impl Sb {
    /// The feed in view is main's (its replies carry `:*`, BISE-15).
    /// The agent in view (whose feed is drawn).
    pub(crate) fn focus_name(&self) -> &str {
        &self.focus
    }

    pub(crate) fn is_main_focus(&self) -> bool {
        match self.agents.iter().find(|a| a.name == self.focus) {
            Some(a) => a.main,
            None => self.focus == "main",
        }
    }

    /// How many things asked for you so far (zen, BISE-121).
    pub(crate) fn calls(&self) -> u64 {
        self.calls
    }

    /// [`Sb::send`] from a shared borrow (a popup asking the hub for
    /// its list while it draws).
    pub(crate) fn send_shared(&self, v: Value) {
        let mut s = v.to_string();
        s.push('\n');
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(s.as_bytes());
        }
    }

    pub(crate) fn send(&mut self, v: Value) {
        let mut s = v.to_string();
        s.push('\n');
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(s.as_bytes());
        }
    }

    /// What the panel navigates, in its order (pr-design §4.1): the
    /// blocks' agents ([`Sb::blocks`]), then (the section expanded) the
    /// archived ones, newest first.
    fn nav(&self) -> Vec<&Agent> {
        let mut out: Vec<&Agent> = self.blocks().into_iter().flat_map(|(_, a)| a).collect();
        if self.archived_open {
            out.extend(self.archived());
        }
        out
    }

    /// The live agents by block (pr-design §4.1, option B of
    /// sidebar-wt): the `agents` section first (None): every agent not
    /// in a shared worktree, your folder's and the ones alone in theirs
    /// alike, at their number (main is 0); then one section per worktree
    /// that 2 or more live agents share, ordered by its lowest number. In
    /// each, the order of their numbers (QA M: a newcomer that takes a
    /// dropped agent's number sits at that number's row, not last).
    /// Numbers never change; the order follows the sections.
    fn blocks(&self) -> Vec<(Option<&places::Place>, Vec<&Agent>)> {
        let numbers = self.numbers();
        let num = |name: &str| numbers.iter().find(|(n, _)| n == name).map_or(usize::MAX, |(_, k)| *k);
        let live: Vec<&Agent> = self.agents.iter().filter(|a| !a.archived()).collect();
        let mut rows: Vec<&Agent> = live.iter().copied().filter(|a| self.box_of(a).is_none()).collect();
        rows.sort_by_key(|a| num(&a.name));
        let mut boxes: Vec<(Option<&places::Place>, Vec<&Agent>)> = self
            .places
            .iter()
            .filter(|p| self.shared(p))
            .map(|p| {
                let mut v = self.live_in(p);
                v.sort_by_key(|a| num(&a.name));
                (Some(p), v)
            })
            .collect();
        boxes.sort_by_key(|(_, v)| v.iter().map(|a| num(&a.name)).min().unwrap_or(usize::MAX));
        let mut out = vec![(None, rows)];
        out.extend(boxes);
        out
    }

    /// The worktree `a` works in, when the hub sent it (main never).
    fn place_of(&self, a: &Agent) -> Option<&places::Place> {
        self.places.iter().find(|p| !a.main && (p.agents.contains(&a.name) || (!a.place_id.is_empty() && p.id == a.place_id)))
    }

    /// The live agents working in worktree `p`.
    fn live_in(&self, p: &places::Place) -> Vec<&Agent> {
        self.agents.iter().filter(|a| !a.archived() && self.place_of(a).is_some_and(|q| std::ptr::eq(p, q))).collect()
    }

    /// Option B (sidebar-wt): a worktree is a section only when 2 or
    /// more live agents share it.
    fn shared(&self, p: &places::Place) -> bool {
        self.live_in(p).len() >= 2
    }

    /// The section `a` sits in: its worktree when another live agent
    /// shares it.
    fn box_of(&self, a: &Agent) -> Option<&places::Place> {
        self.place_of(a).filter(|p| self.shared(p))
    }

    /// The worktree `a` is alone in: its row carries the mark.
    fn solo_of(&self, a: &Agent) -> Option<&places::Place> {
        self.place_of(a).filter(|p| !self.shared(p))
    }

    /// The worktrees with no live agent but an open PR: a numberless row
    /// each, after the solo rows. One with no agent and no open PR (a
    /// `gate.sh new` scratch worktree) is never shown.
    fn orphans(&self) -> Vec<&places::Place> {
        self.places.iter().filter(|p| p.live_pr().is_some() && self.live_in(p).is_empty()).collect()
    }

    /// An inbox item asks you about `p`'s PR (ready to merge, pr-design
    /// §6.3): its `↑` takes the accent. The item names its place
    /// (pr-merge); one without (an older hub): a `merge` item of one of
    /// its agents, or naming its number.
    fn asks_merge(&self, p: &places::Place) -> bool {
        let Some(pr) = p.live_pr() else { return false };
        let tag = format!("#{}", pr.number);
        self.cards.iter().filter(|c| c.kind == "merge").any(|c| match &c.place {
            Some(id) => *id == p.id,
            None => {
                p.agents.contains(&c.agent)
                    || c.text.match_indices(&tag).any(|(i, _)| !c.text[i + tag.len()..].starts_with(|ch: char| ch.is_ascii_digit()))
            }
        })
    }

    /// The archived tasks, the most recently active first.
    fn archived(&self) -> Vec<&Agent> {
        let mut out: Vec<&Agent> = self.agents.iter().filter(|a| a.archived()).collect();
        out.sort_by_key(|a| std::cmp::Reverse(a.last_ms()));
        out
    }

    /// Expand or collapse the archived section; the selection stays on
    /// the same entry (or leaves a row that is folded away).
    fn toggle_archived(&mut self) {
        let sel = self.selected_agent().map(|a| a.name.clone());
        self.archived_open = !self.archived_open;
        self.selected = sel.and_then(|n| self.nav().iter().position(|a| a.name == n));
        if self.selected.is_none() {
            self.preview = false;
        }
    }

    /// The agent in focus is archived: its feed is read-only.
    fn focus_archived(&self) -> bool {
        self.agent(&self.focus).is_some_and(|a| a.archived())
    }

    /// The context usage of an agent's feed (the focused one lives in
    /// the `App` fields).
    fn usage_of<'a>(&'a self, app: &'a App, name: &str) -> Option<&'a crate::usage::Usage> {
        if self.focus == name {
            return crate::usage::current(&app.events);
        }
        crate::usage::current(&self.views.get(name)?.events)
    }

    /// The model of the agent in focus (BISE-150): the one its last
    /// usage line names, else the one its role starts with (main's or
    /// the sub-agents', from the catalog's setup).
    fn focus_model(&self, app: &App) -> String {
        // the hub says what it runs now (BISE-135: a /model switch)
        if let Some(m) = self.agent(&self.focus).map(|a| a.model.clone()).filter(|m| !m.is_empty()) {
            return m;
        }
        let used = app.events.iter().rev().find_map(|e| match e {
            crate::Ev::Usage(u) if !u.model.is_empty() => Some(u.model.clone()),
            _ => None,
        });
        used.unwrap_or_else(|| {
            let main = self.agent(&self.focus).is_none_or(|a| a.main);
            crate::models::model_for(main)
        })
    }

    fn agent(&self, name: &str) -> Option<&Agent> {
        self.agents.iter().find(|a| a.name == name)
    }

    /// `name`'s folder (its name but after a rename; its name when the
    /// hub didn't say): its computer-use key's second half.
    pub(super) fn dir_of(&self, name: &str) -> String {
        self.agent(name).map(|a| a.dir.clone()).filter(|d| !d.is_empty()).unwrap_or_else(|| name.to_string())
    }

    /// `name` works in the shared folder (no branch, no worktree of its
    /// own): its diff is `your folder vs main` (designer m_7354).
    pub(crate) fn in_shared_folder(&self, name: &str) -> bool {
        self.agent(name).is_some_and(|a| a.branch.is_none() && a.mode != "worktree")
    }

    /// The agent working on `branch`, if one is.
    pub(crate) fn agent_of_branch(&self, branch: &str) -> Option<String> {
        self.agents.iter().find(|a| a.branch.as_deref() == Some(branch)).map(|a| a.name.clone())
    }

    /// The entry of the panel highlighted by ⌥↑↓.
    fn selected_agent(&self) -> Option<&Agent> {
        self.nav().get(self.selected?).copied()
    }

    /// Tell the hub which feed is in focus.
    fn send_focus(&mut self) {
        let focus = self.focus.clone();
        self.call("client/focus", json!({"focus": focus}), rpc::Then::Shown);
    }

    /// A line typed to the agent in focus (the hub interprets it).
    fn send_input(&mut self, text: String) {
        let focus = self.focus.clone();
        self.send_input_to(&focus, text);
    }

    /// What the user typed, for `agent` (in view or not).
    fn send_input_to(&mut self, agent: &str, text: String) {
        // computer use (C6, m_3893): a message to an agent you stopped is
        // its go-ahead; `@name …` is for that agent
        if !text.starts_with('/') {
            let to = text.split_whitespace().next().and_then(|w| w.strip_prefix('@')).filter(|n| self.agent(n).is_some());
            crate::computer_use::resume_if_stopped(to.unwrap_or(agent));
        }
        self.call("command/run", json!({"agent": agent, "line": text}), rpc::Then::Line);
    }
}

/// What `ctrl+z` and a typed `/cancel` say (book §13, §17).
pub(super) const NO_UNDO: &str =
    "no undo: an agent may already have acted. say the change to main instead (\"no, v1 for docs\").";

/// `/theme [auto|light|dark]`: switch the palette now and keep it for
/// the next launches (`choose`: `theme_detect::choose`, a fake in tests
/// so they never write the real home). No argument: say which one is in
/// use.
fn theme_command(
    arg: Option<&str>,
    choose: impl Fn(crate::theme_detect::Choice) -> (crate::theme::Mode, Result<(), String>),
) -> Ev {
    use crate::theme_detect::Choice;
    let name = |m| if m == crate::theme::Mode::Light { "light" } else { "dark" };
    match arg.map(Choice::parse) {
        None => Ev::Info(format!("theme: {}. /theme auto, light or dark to change it.", name(crate::theme::mode()))),
        Some(Some(c)) => match choose(c) {
            (m, Ok(())) => Ev::Info(format!("theme: {}.", name(m))),
            (m, Err(e)) => Ev::Warn(format!("theme: {}, for now: i couldn't save it ({}).", name(m), e)),
        },
        Some(None) => Ev::Warn("/theme takes auto, light or dark.".into()),
    }
}

/// Ctrl+L in the switchboard: the feed in focus is cleared, its lines
/// stay reachable by scrolling up (`feed::clear_feed`).
pub(super) fn clear_display(app: &mut App) {
    clear_feed(app);
}

/// The agents matching `q` (name or objective): the live tasks (not
/// main), or the archived ones, newest first (BISE-117). The agent in
/// view comes first, `· in view` (dim) after its name: from its view,
/// `/archive` + ⏎ archives it, `/restore` + ⏎ restores it.
pub(super) fn agent_choices(app: &App, archived: bool, q: &str) -> Vec<Choice> {
    let sb = &app.sb;
    let list: Vec<&Agent> = if archived {
        sb.archived()
    } else {
        sb.agents.iter().filter(|a| !a.main && !a.archived()).collect()
    };
    let (here, others): (Vec<&Agent>, Vec<&Agent>) = list.into_iter().partition(|a| a.name == sb.focus);
    here.into_iter()
        .chain(others)
        .filter(|a| crate::commands::matches(q, &[&a.name, &a.objective]))
        .map(|a| {
            let in_view = if a.name == sb.focus { " in view ·" } else { "" };
            Choice {
                value: a.name.clone(),
                label: a.name.clone(),
                desc: format!("·{in_view} {} · {}", a.status, truncate_chars(&a.objective, 80)),
                mark: None,
            }
        })
        .collect()
}

/// Startup timing: the hub's `ready` arrived (its replay is taken in).
pub(super) fn is_ready(app: &App) -> bool {
    app.sb.ready
}

/// Startup timing: the events of the feeds not in focus.
pub(super) fn background_events(app: &App) -> usize {
    app.sb.views.values().map(|v| v.events.len()).sum()
}

/// The hub is back (a new connection, `hello` sent): it replays every
/// feed, so the feeds start empty again. The focus and the drafts stay.
fn hub_reconnected(app: &mut App) {
    app.connected = true;
    // the scroll comes back at the replay's `ready` (keep-state)
    keep::before_reconnect(app);
    feed::empty_feed(app);
    app.win = FeedWindow::default();
    app.pending = false;
    app.interrupt_requested = false;
    let sb = &mut app.sb;
    for v in sb.views.values_mut() {
        let draft = std::mem::take(&mut v.ed);
        *v = View::new();
        v.ed = draft;
    }
    sb.activity.clear();
    sb.ready = false;
    sb.confirm = None;
    sb.release_ask = None;
    sb.release = None;
    sb.rpc.forget();
    sb.send_focus();
}

/// Route one hub event.
pub(super) fn dispatch(app: &mut App, raw: &str) {
    if raw == HUB_DOWN {
        app.connected = false;
        return;
    }
    if raw == HUB_UP {
        hub_reconnected(app);
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    // a JSON-RPC response (client-protocol step 3): its request's answer;
    // a notification (step 4): its typed reader
    if v.get("jsonrpc").is_some() {
        return if v.get("method").is_none() { rpc::answered(app, v) } else { hub_reads::read(app, v) };
    }
    let s = |k: &str| str_of(&v, k);
    match s("ev").as_str() {
        "line" => {
            let pos = v.get("pos").and_then(|x| x.as_u64()).map(|p| p as usize);
            let ts = v.get("ts").and_then(|x| x.as_u64());
            ingest_for(app, &s("agent"), s("line"), pos, ts)
        }
        "history" => {
            let before = v.get("before").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            let lines = crate::wire::parse_history(&v);
            with_feed(app, &s("agent"), |app| prepend_page(app, before, lines));
        }
        "state" => apply_state(app, &v),
        // the hub refused this client (it runs in an agent's process,
        // docs/issues/16): say why once the terminal is back, and end
        "refused" => {
            client::set_refused(s("error"));
            app.should_quit = true;
        }
        // update-card: `/update` with a newer release opens its item here
        "open_card" => {
            if let Some(id) = v.get("id").and_then(|x| x.as_u64()) {
                cards::open_view(app, Some(id));
            }
        }
        "notice" => {
            // the hub refused an input (it says so in a notice): a queued
            // message that went will start no turn, so the queue moves on
            crate::queue::seen(app);
            for l in s("text").lines() {
                push_event(&mut app.events, &mut app.cache, Ev::Info(l.to_string()));
            }
        }
        // `/prs` (pr-news, designer): one dim head row, then each PR like
        // a PR line, its URL dim under it
        "prs" => {
            for e in prs_events(&v) {
                push_event(&mut app.events, &mut app.cache, e);
            }
        }
        "release" => release::event(app, &v),
        "update" => release::update_event(app, &v),
        "focus" => focus(app, &s("focus")),
        "renamed" => {
            let (old, new) = (s("old"), s("new"));
            let sb = &mut app.sb;
            if let Some(view) = sb.views.remove(&old) {
                sb.views.insert(new.clone(), view);
            }
            if sb.focus == old {
                sb.focus = new;
            }
        }
        "versions" => {
            let sb = &mut app.sb;
            sb.versions = parse_versions(&v);
            sb.versions_dev = v.get("dev").and_then(|x| x.as_bool());
        }
        "hello" => {
            let sb = &mut app.sb;
            sb.workspace = s("workspace");
            crate::artifacts::set_workspace(&sb.workspace);
            sb.version = v
                .pointer("/version/id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            // the hub runs another version: this TUI follows it
            let exe = s("exe");
            let reload = s("reload");
            let first = sb.reload_seen.is_none();
            let reloaded = !first && !reload.is_empty() && sb.reload_seen.as_deref() != Some(reload.as_str());
            if first {
                sb.reload_seen = Some(reload);
            }
            // a reload (BISE-131) starts the same binary again; both wait
            // for the keys to stop (sb/reload_wait.rs, run.rs quits); the
            // drafts, queues and view are written when the UI ends
            if (!exe.is_empty() && follow_hub_exe(&exe)) || (reloaded && follow_reload()) {
                app.sb.reload_wait.ask(std::time::Instant::now());
            }
        }
        "ready" => {
            let sb = &mut app.sb;
            sb.ready = true;
            // the queues saved by the TUI before this one (a reload, a
            // restart): back now that the feeds say who is busy
            drafts::requeue(app);
            // the inbox answers and what was open (a reload)
            keep::apply(app);
        }
        _ => {}
    }
}

fn ingest_for(app: &mut App, agent: &str, line: String, pos: Option<usize>, ts: Option<u64>) {
    let sb = &mut app.sb;
    // BISE-61: the first live message between agents in view
    let level3 = sb.ready
        && sb.focus == agent
        && (line.starts_with("sb msg : ") || line.starts_with("sb msg-in : "));
    // BISE-15: the first steering the model read (its line turns ✓✓)
    let steered = sb.ready && sb.focus == agent && line.contains("obs: steered: ");
    // zen (BISE-121): a live card or message to you, in any feed
    if sb.ready && (line.starts_with("sb card : ") || line.starts_with("sb msg-you : ") || line.starts_with("sb msg-in : @")) {
        sb.calls += 1;
    }
    if sb.focus != agent {
        let visible = line.contains("obs: assistant:") || line.starts_with("sb ");
        if visible && sb.ready {
            sb.activity.insert(agent.to_string());
        }
    }
    // an answer given here: its fold line is in this feed already
    let answer_id = line.strip_prefix("sb route : ").and_then(|r| lines::answer_route(&unescape_md(r)).map(|(_, id, _)| id));
    let folded = answer_id.is_some_and(|id| sb.folded_in(id, agent));
    // BISE-307: what the item asked, for its line to open on (the
    // inbox's card while the hub still holds it)
    let asked = answer_id.and_then(|id| sb.card_by_id(id)).map(|c| c.text.trim().to_string());
    let mut queued = None;
    // voice mode: the live messages and turns of this agent (never a replay)
    let voice = app.voice_mode.is_some() && app.sb.ready;
    let mut said = Vec::new();
    with_feed(app, agent, |app| {
        if folded {
            let n0 = app.events.len();
            feed::seen_at(app, pos, n0);
        } else {
            let n0 = app.events.len();
            ingest_at(app, line, pos, ts);
            if voice {
                said.extend(
                    app.events[n0.min(app.events.len())..]
                        .iter()
                        .filter(|e| matches!(e, Ev::Assistant(_) | Ev::Turn | Ev::TurnDone | Ev::Idle))
                        .cloned(),
                );
            }
            if let Some(id) = answer_id {
                ask_of(app, n0, id, asked);
            }
        }
        trim_window(app);
        // BISE-89: its turn ended, the oldest queued message goes (and
        // the next one waits for the turn it starts)
        queued = crate::queue::next(app);
        if queued.is_some() {
            app.pending = true;
        }
    });
    if !said.is_empty() {
        crate::voicemode::live::on_events(app, agent, &said);
    }
    if let Some(m) = queued {
        app.sb.send_input_to(agent, m);
    }
    if level3 {
        crate::hints::once(app, crate::hints::Hint::FirstLevel3);
    }
    if steered {
        crate::hints::once(app, crate::hints::Hint::FirstSteer);
    }
}

/// The answer to card `id` just read (events from `n0`): what the item
/// asked, from the inbox (`asked`), else from the card's own line in
/// this feed (`#12 question @main : …`); unknown, the line opens on the
/// answer alone (BISE-307).
fn ask_of(app: &mut App, n0: usize, id: u64, asked: Option<String>) {
    let head = format!("#{id} ");
    let found = asked.or_else(|| {
        app.events[..n0.min(app.events.len())].iter().rev().find_map(|e| match e {
            Ev::Card { text, .. } if text.starts_with(&head) => Some(crate::render::card_parts(text).map_or("", |p| p.2).trim().to_string()),
            _ => None,
        })
    });
    let Some(q) = found.filter(|q| !q.is_empty()) else { return };
    for i in n0..app.events.len() {
        if let Ev::Approval { asked, .. } = &mut app.events[i] {
            *asked = q.clone();
            if let Some(c) = app.cache.get_mut(i) {
                *c = None;
            }
        }
    }
}

/// shift+tab (approvals-design.md §8): the hub switches the mode for
/// every agent and says so to every TUI (its `approvals` event flashes).
pub(crate) fn toggle_approvals(app: &mut App) {
    // an older hub never said a mode: it has no switch
    if !app.sb.approvals.mode.is_empty() {
        app.sb.call("approvals/set", json!({"mode": "toggle"}), rpc::Then::Shown);
    }
}

fn apply_state(app: &mut App, v: &Value) {
    let sb = &mut app.sb;
    let s = str_of;
    let known: Vec<u64> = sb.cards.iter().map(|c| c.id).collect();
    sb.agents = v
        .get("agents")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|x| Agent {
                    name: s(x, "name"),
                    main: x.get("main").and_then(|m| m.as_bool()).unwrap_or(false),
                    status: s(x, "status"),
                    objective: s(x, "objective"),
                    mode: s(x, "mode"),
                    branch: x.get("branch").and_then(|b| b.as_str()).map(String::from),
                    path: s(x, "path"),
                    note: s(x, "note"),
                    queued: x.get("queued").and_then(|q| q.as_u64()).unwrap_or(0),
                    inbox: x.get("inbox").and_then(|q| q.as_u64()).unwrap_or(0),
                    turn_ms: x.get("turn_ms").and_then(|q| q.as_u64()),
                    turn_seen: Some(std::time::Instant::now()),
                    report: s(x, "report"),
                    role: s(x, "role"),
                    report_ms: x.get("report_ms").and_then(|q| q.as_u64()),
                    created_ms: x.get("created_ms").and_then(|q| q.as_u64()).unwrap_or(0),
                    waiting_on: s(x, "waiting_on"),
                    dir: s(x, "dir"),
                    place: s(x, "place"),
                    place_id: s(x, "place_id"),
                    model: s(x, "model"),
                    effort: s(x, "effort"),
                    efforts: x
                        .get("efforts")
                        .and_then(|e| e.as_array())
                        .map(|e| e.iter().filter_map(|w| w.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                    changes: x.get("changes").filter(|c| c.is_object()).map(|c| {
                        let n = |k: &str| c.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                        (n("files"), n("add"), n("del"))
                    }),
                })
                .collect()
        })
        .unwrap_or_default();
    sb.places = places::parse(v);
    // an open diff panel on an agent follows its changes (site/m/artifacts D)
    if let Some(crate::diffview::Ask::Agent(name)) = app.diff.as_ref().map(|p| p.ask.clone()) {
        let changes = app.sb.agents.iter().find(|a| a.name == name).and_then(|a| a.changes);
        crate::diffview::on_changes(app, &name, changes);
    }
    let sb = &mut app.sb;
    // an older hub has no `timers`: keep none
    sb.timers = crate::scheduled::from_state(v);
    sb.flow = s(v, "flow");
    sb.cards = v
        .get("cards")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|x| Card {
                    id: x.get("id").and_then(|i| i.as_u64()).unwrap_or(0),
                    kind: s(x, "kind"),
                    agent: s(x, "agent"),
                    text: s(x, "text"),
                    age_ms: x.get("age_ms").and_then(|i| i.as_u64()).unwrap_or(0),
                    seen_at: std::time::Instant::now(),
                    note: s(x, "note"),
                    look: None,
                    place: x.get("place").and_then(|p| p.as_str()).map(String::from),
                    pr: x.get("pr").and_then(|n| n.as_u64()),
                    link: x.get("link").and_then(|p| p.as_str()).map(String::from),
                    // the drop's second ask stays across snapshots
                    asking: sb.feature_drop_ask == x.get("id").and_then(|i| i.as_u64()),
                    waiting: x
                        .get("waiting")
                        .and_then(|w| w.as_array())
                        .map(|w| w.iter().filter_map(|a| a.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    // the setup cards are the TUI's own: not in the hub's snapshot
    setup::put_back(sb);
    // zen (BISE-121): a card that was not there
    if sb.cards.iter().any(|c| !known.contains(&c.id)) {
        sb.calls += 1;
    }
    cards::sync(app);
    let sb = &mut app.sb;
    // the spinner of every feed follows the agent, whoever started the turn
    let busy: HashMap<String, bool> = sb.agents.iter().map(|a| (a.name.clone(), a.busy())).collect();
    for (name, view) in sb.views.iter_mut() {
        view.pending = busy.get(name).copied().unwrap_or(false);
    }
    let focus_busy = busy.get(&sb.focus).copied().unwrap_or(false);
    if let Some(sel) = sb.selected {
        if sel >= sb.nav().len() {
            sb.selected = None;
            sb.preview = false;
        }
    }
    app.pending = focus_busy;
    if !focus_busy {
        app.interrupt_requested = false;
    }
    // BISE-61: the first agent, the first card (one-time hints)
    let sb = &app.sb;
    let (agent, card) = (sb.agents.iter().any(|a| !a.main && !a.archived()), !sb.cards.is_empty());
    if agent {
        crate::hints::once(app, crate::hints::Hint::FirstAgent);
    }
    if card {
        crate::hints::once(app, crate::hints::Hint::FirstCard);
    } else {
        crate::hints::used(crate::hints::Hint::FirstCard);
    }
    // the demo's guided tips (tour.rs)
    crate::tour::on_state(app);
}

/// What the demo's tour looks at (tour.rs): the demo agents with their
/// numbers, the feed in view, a hub card waiting, the palette open.
pub(crate) fn tour_snap(app: &App) -> crate::tour::Snap {
    let sb = &app.sb;
    let numbers = sb.numbers();
    let members = sb
        .agents
        .iter()
        .filter(|a| !a.main && crate::tour::is_demo(&a.objective))
        .map(|a| crate::tour::Member {
            name: a.name.clone(),
            number: numbers.iter().find(|(n, _)| *n == a.name).map(|(_, k)| *k).filter(|k| *k <= 9),
            archived: a.archived(),
        })
        .collect();
    let card = sb.cards.iter().find(|c| !setup::is_local(c.id)).map(|c| c.agent.clone());
    crate::tour::Snap { members, focus: sb.focus.clone(), card, palette: app.palette.is_some(), no_ctrl_digits: !app.ctrl_digits }
}

/// Change the feed in focus (checkout / return).
pub(super) fn focus(app: &mut App, name: &str) {
    // the card view goes: the thread you go to takes its place
    cards::close_view(app);
    let sb = &mut app.sb;
    // BISE-61: looking inside an agent is what the first-agent hint asks
    if sb.agents.iter().any(|a| a.name == name && !a.main) {
        crate::hints::used(crate::hints::Hint::FirstAgent);
    }
    sb.selected = None;
    sb.preview = false;
    if sb.focus == name {
        return;
    }
    // find (BISE-237): what it opened closes with the feed we leave
    crate::find::close(app);
    let sb = &mut app.sb;
    let old = std::mem::replace(&mut sb.focus, name.to_string());
    sb.activity.remove(name);
    let mut incoming = sb.views.remove(name).unwrap_or_else(View::new);
    sb.send_focus();
    swap_feed(app, &mut incoming);
    swap_draft(app, &mut incoming);
    // a feed selection belongs to the feed we left
    app.feed_sel = None;
    // `incoming` now holds the feed we left
    let sb = &mut app.sb;
    sb.views.insert(old, incoming);
    app.follow = true;
    app.unseen = 0;
}

/// The agents that run their own model (`/model` in that agent), for
/// `/models`' line under agents (BISE-298): (agent, model).
fn model_overrides(app: &App) -> Vec<(String, String)> {
    let (main_m, agent_m) = (crate::models::model_for(true), crate::models::model_for(false));
    app.sb
        .agents
        .iter()
        // main's own choice shows on main's row, not under agents
        .filter(|a| a.name != "main" && !a.model.is_empty() && !a.archived())
        .filter(|a| a.model != if a.name == "main" { main_m.as_str() } else { agent_m.as_str() })
        .map(|a| (a.name.clone(), a.model.clone()))
        .collect()
}

/// One line typed by the user: the client's own commands (/voice,
/// /quit, /clear, /help, /theme…) here, the rest goes to the hub.
pub(crate) fn handle_input(app: &mut App, v: &str) -> Vec<Ev> {
    // BISE-298: /voice turns voice on (its setup first when it has
    // none that works) or off; /voice setup opens the voice picker
    // voice mode: /voice is its settings screen (dictation's on/off is a
    // row there)
    if v.trim() == "/voice" {
        crate::voicemode::settings::request(crate::voicemode::settings::Open::Settings);
        return Vec::new();
    }
    // round 2: /voice setup is the same screen, on speech to text
    // (voice-menu: no longer offered by the menu, still taken typed)
    if v.trim() == "/voice setup" {
        crate::voicemode::settings::request_stt();
        return Vec::new();
    }
    let typed = v
        .strip_prefix("steer ")
        .or_else(|| v.strip_prefix("say "))
        .unwrap_or(v)
        .trim()
        .to_string();
    app.history.insert(0, typed.clone());
    app.history.truncate(drafts::HISTORY_MAX);
    // BISE-120a: the sent draft leaves the file at once
    drafts::save_now(app);
    app.popup_sel = 0;
    let mut out: Vec<Ev> = Vec::new();
    let sb = &mut app.sb;
    if let Some((id, _)) = sb.confirm.clone() {
        let t = typed.to_lowercase();
        let yes = matches!(t.as_str(), "y" | "yes" | "o" | "oui");
        let no = matches!(t.as_str(), "n" | "no" | "non");
        if yes || no {
            sb.confirm = None;
            sb.call("confirm/answer", json!({"id": id, "yes": yes}), rpc::Then::Shown);
            return out;
        }
    }
    if release::answer(app, &typed) {
        return out;
    }
    // the raw session of the agent in view (logview.rs)
    if typed.split_whitespace().next() == Some("/log") && crate::logview::enabled(app) {
        if let Err(e) = crate::logview::open(app, &typed) {
            out.push(Ev::Warn(format!("/log: {e}")));
        }
        return out;
    }
    // site/m/artifacts: `/artifacts` (the full screen), `/artifacts add
    // <path or link>`, `/diff [<branch>]`
    match typed.split_whitespace().next() {
        // site/m/timers: the scheduled tasks, full screen
        Some("/scheduled") => {
            crate::scheduled_screen::open(app);
            return out;
        }
        // bise_proto::slash reads it, as the hub reads the window's line
        Some("/artifacts") => {
            use bise_proto::slash::{artifacts, Artifacts, ARTIFACTS_ADD};
            match artifacts(&typed) {
                Some(Artifacts::Usage) => out.push(Ev::Warn(ARTIFACTS_ADD.into())),
                Some(Artifacts::Add(target)) => {
                    let agent = app.sb.focus.clone();
                    app.sb.call("artifacts/add", json!({"target": target, "agent": agent}), rpc::Then::Said);
                }
                Some(Artifacts::List) | None => crate::artifacts_screen::open(app),
            }
            return out;
        }
        Some("/diff") => {
            let branch = typed.trim_start_matches("/diff").trim();
            if branch.is_empty() {
                let a = app.sb.focus.clone();
                crate::diffview::request(app, crate::diffview::Ask::Agent(a), crate::diffview::By::Key);
            } else {
                crate::diffview::request(app, crate::diffview::Ask::Branch(branch.to_string()), crate::diffview::By::Key);
            }
            return out;
        }
        _ => {}
    }
    let sb = &mut app.sb;
    let first = typed.split_whitespace().next().unwrap_or("");
    let recolor = first == "/theme";
    match first {
        "/quit" | "/exit" => app.should_quit = true,
        "/release-bise" => release::command(sb, &typed),
        // the hub's version/* methods; one parse with the hub's slash
        // (bise_proto::slash::version). /update: the release channel now
        // (update-card's new-release item)
        "/restart" | "/update" | "/version" => {
            if let Some((method, params, then)) = bise_proto::slash::version(&typed).as_ref().map(versions::method) {
                sb.call(method, params, then);
            }
        }
        "/plugins" => {
            let ws = std::path::PathBuf::from(&sb.workspace);
            // one line each: an Info line loses its newlines (designer:
            // the listing read as one paragraph)
            for l in crate::plugins::command(&typed, &ws).lines() {
                out.push(Ev::Info(l.to_string()));
            }
        }
        "/clear" => {
            clear_feed(app);
            out.push(Ev::Info("display cleared — scroll up to see the earlier lines again".into()));
        }
        "/help" | "/shortcuts" | "/shortcut" | "/keys" => {
            app.help = crate::help::page_of(first).map(crate::help::Overlay::new);
        }
        "/archived" => sb.toggle_archived(),
        // the agent palette (BISE-265), on the rest of the line
        "/switch" => palette::open(app, typed.strip_prefix("/switch").unwrap_or("")),
        // the inbox: its first item in the view (`/cards`: its old name)
        "/inbox" | "/cards" => {
            if sb.sorted_cards().is_empty() {
                out.push(Ev::Info("the inbox is empty: nothing waits for you".into()));
            } else {
                cards::open_view(app, None);
            }
        }
        "/theme" => out.push(theme_command(typed.split_whitespace().nth(1), crate::theme_detect::choose)),
        "/keychain" => out.extend(crate::keychain::command(&typed)),
        // approvals-design.md §8: the mode, the checker, the rules; or a switch
        // one parse with the hub's slash (bise_proto::slash::approvals)
        "/approvals" => match bise_proto::slash::approvals(&typed) {
            Some(Ok(Some(bise_proto::rows::ApprovalMode::Yolo))) => sb.call("approvals/set", json!({"mode": "yolo"}), rpc::Then::Shown),
            Some(Ok(Some(bise_proto::rows::ApprovalMode::Auto))) => sb.call("approvals/set", json!({"mode": "auto"}), rpc::Then::Shown),
            Some(Ok(_)) => sb.call("approvals/set", json!({}), rpc::Then::Approvals),
            Some(Err(words)) => out.push(Ev::Warn(words)),
            None => {}
        },
        "/welcome" => crate::onboarding::run(app),
        // computer-use-design.md §8: the setup steps, polled live
        // opt-in (computer-use-ship.md §1): opening it turns the plugin on,
        // off/uninstall turn it off (the agents follow at their next idle)
        "/computer-use" => match typed.split_whitespace().nth(1) {
            None => {
                crate::computer_use::set_on(true);
                app.computer_use = Some(crate::computer_use::Screen::open());
            }
            Some(w @ ("off" | "uninstall")) => out.push(Ev::Info(crate::computer_use::turn_off(w == "uninstall"))),
            Some(other) => out.push(Ev::Warn(format!("/computer-use {other}: off or uninstall"))),
        },
        // §7.3: the turn stops and the agent lets go of Chrome and its apps
        // until you write to it again
        // one parse with the hub's slash (bise_proto::slash::stop)
        "/stop" => match bise_proto::slash::stop(&typed).unwrap_or_else(|| Err(bise_proto::slash::STOP_USAGE.into())) {
            Ok(name) if sb.agent(&name).is_some() => {
                if sb.agent(&name).is_some_and(|a| a.status == "working") {
                    sb.call("turn/interrupt", json!({"agent": name}), rpc::Then::Shown);
                }
                // main's feed says it once, from the hub (m_3904)
                crate::computer_use::stop(&sb.dir_of(&name));
            }
            Ok(name) => out.push(Ev::Warn(format!("/stop: no agent named {name}"))),
            Err(usage) => out.push(Ev::Warn(usage)),
        },
        // BISE-298: which model does what
        "/models" | "/roles" => {
            crate::onboarding::provider_request(crate::onboarding::Ask {
                open: crate::onboarding::Open::Roles,
                overrides: model_overrides(app),
                ..Default::default()
            });
        }
        // BISE-294: set up a provider's key, or change it
        "/provider" | "/providers" => {
            let id = typed.split_whitespace().nth(1).map(|s| s.trim().to_lowercase());
            crate::onboarding::provider_request(crate::onboarding::Ask { provider: id, ..Default::default() });
        }
        // a model whose provider has no key: set it up first, then the
        // line runs (never saved blindly, BISE-294)
        "/model" if crate::models::model_needs_key(&typed).is_some() => {
            let (id, model) = crate::models::model_needs_key(&typed).unwrap_or_default();
            crate::onboarding::provider_request(crate::onboarding::Ask {
                provider: Some(id),
                model: Some(model),
                line: Some(typed.clone()),
                ..Default::default()
            });
        }
        "/setup" => setup::command(app),
        "/cancel" => out.push(Ev::Info(NO_UNDO.into())),
        // an archived task reads nothing: its feed is history only
        _ if sb.focus_archived() && !typed.starts_with('/') => {
            out.push(Ev::Warn(archived_warn(&sb.focus)));
        }
        _ => {
            sb.send_input(typed);
            // BISE-61: a hint goes away after the next user message
            crate::hints::user_message();
            crate::tour::on_sent(app);
        }
    }
    if recolor {
        // the feed's rows carry their colors: build them again
        app.cache.clear();
    }
    for ev in &out {
        push_event(&mut app.events, &mut app.cache, ev.clone());
    }
    out
}

/// Draw, with the previewed feed swapped in when a preview is open.
pub(super) fn draw_sb(app: &mut App, frame: &mut Frame) {
    let sb = &app.sb;
    let target = sb
        .selected_agent()
        .filter(|_| sb.preview)
        .map(|a| a.name.clone())
        .filter(|n| *n != sb.focus);
    // a path in the feed resolves against its agent's folders (BISE-264)
    crate::file_links::set_dirs(panel::feed_dirs(app, target.as_deref().unwrap_or(&app.sb.focus)));
    match target {
        Some(name) => with_feed(app, &name, |app| draw(app, frame)),
        None => {
            draw(app, frame);
            want_older(app);
        }
    }
}


/// The feed rows of the hub's `prs` event, its answer to `/prs`
/// (`HubEv::Prs`: `{head, items: [bise-proto rows::Pr]}`); each row's tone is the TUI's
/// look of what it means ([`pr_tone`]), never a color from the wire.
pub(super) fn prs_events(v: &serde_json::Value) -> Vec<Ev> {
    // the head at col 1, like every feed row's glyph column (designer)
    let head = format!(" {}", v["head"].as_str().unwrap_or_default());
    let mut out = vec![Ev::Fold { head, text: String::new(), open: false }];
    for r in v["items"].as_array().into_iter().flatten() {
        let Ok(pr) = serde_json::from_value::<bise_proto::rows::Pr>(r.clone()) else { continue };
        out.push(Ev::Pr { tone: pr_tone(&pr).into(), number: pr.number, url: pr.url, text: pr.text, url_row: true });
    }
    out
}

/// A `/prs` row's tone: red when its checks fail, dim for a draft, else
/// plain (the PR line's look, designer).
pub(super) fn pr_tone(pr: &bise_proto::rows::Pr) -> &'static str {
    use bise_proto::rows::{PrChecks, PrState};
    match (pr.checks, pr.state) {
        (PrChecks::Fail, _) => "red",
        (_, PrState::Draft) => "dim",
        _ => "plain",
    }
}

/// A PR news line's state (bise_proto: what it means, never a color, the
/// window's too) as the TUI's look: news plain, done dim, failing red.
pub(crate) fn pr_look(state: bise_proto::thread::PrNewsState) -> &'static str {
    use bise_proto::thread::PrNewsState;
    match state {
        PrNewsState::Done => "dim",
        PrNewsState::Failing => "red",
        PrNewsState::News | PrNewsState::Unknown => "plain",
    }
}

/// A sender or receiver as the user reads it: the hub's own id
/// `switchboard` (bise's old name, still its id in the journals and the
/// routing) shows as `bise`, like the agents read it (prompts.rs
/// `shown_sender`); any other name stays.
pub(crate) fn shown_name(name: &str) -> String {
    bise_proto::thread::lines::shown_name(name)
}

/// A hub line `sb <kind> : <text>` (hub line protocol, contract C2) as a
/// feed event; the line is read by bise-proto's `thread::lines::hub`
/// (the tests' entry: `wire::parse_line` calls [`hub_ev`] itself).
#[cfg(test)]
pub(super) fn parse_hub_line(rest: &str) -> Option<Ev> {
    hub_ev(lines::hub(rest))
}

/// The hub's own lines in a feed as feed events. v1 kinds keep working
/// (an old transcript still renders); v2 adds `msg` (between agents,
/// level 3), `msg-you` (an agent writing to the user, level 2) and
/// `answered` (main answered an agent for the user, level 2).
pub(crate) fn hub_ev(h: Hub) -> Option<Ev> {
    let msg = |from: String, to: String, text: String, level: u8, id: String| Ev::AgentMsg { from, to, text, level, id, open: false, fold: false };
    Some(match h {
        Hub::You(text) => Ev::You(text, Mark::Sent, false),
        // S9: the fn context of the 'you' line before it (the window's
        // thread shows it on his message; the TUI doesn't)
        Hub::Context(_) => return None,
        // BISE-86
        Hub::Undelivered { to, text } => Ev::Undelivered { name: to, text, open: true },
        // v1: what this feed's owner received (`@{from}`: an old direct
        // reply to the user)
        Hub::MsgIn { from, id, body } => {
            // a scheduled task's run reads as its ◷ line; the note of a
            // stop is for the agent only (site/m/timers)
            if crate::scheduled::is_hub(&from) {
                if let Some(ev) = crate::scheduled::run_line(&body) {
                    return Some(ev);
                }
                if crate::scheduled::is_stop_note(&body) {
                    return None;
                }
                // main's note of an answer to its own card: the route
                // line's row says it (bise_proto lines, the window's rule)
                if bise_proto::thread::lines::is_main_answer_note(&body) {
                    return None;
                }
            }
            match from.strip_prefix('@') {
                Some(f) => msg(f.to_string(), "you".into(), body, 2, id),
                None => msg(shown_name(&from), String::new(), body, 3, id),
            }
        }
        Hub::Msg { from, to, id, body } => {
            // main's thread never shows another agent's runs, nor the
            // note of a stop (site/m/timers): its ◷ lines say enough
            if crate::scheduled::is_hub(&from) && (crate::scheduled::run_line(&body).is_some() || crate::scheduled::is_stop_note(&body)) {
                return None;
            }
            msg(shown_name(&from), shown_name(&to), body, 3, id)
        }
        // what this feed's owner (a task) sent another agent, sb-core's
        // line (architect m_10203); `from` empty: the owner
        // (render::l3_sender). Its `sb send` box goes quiet as for main's
        // `msg` lines (BISE-110: same id)
        Hub::Sent { to, id, body, .. } => msg(String::new(), shown_name(&to), body, 3, id),
        Hub::MsgYou { from, body } => msg(shown_name(&from), "you".into(), body, 2, String::new()),
        Hub::Answered { agent, question, answer, why } => Ev::Answered { agent, question, answer, why, open: false },
        // a gate's card (approvals-design.md §9): the tool row says it
        // waits and the inbox holds it; its fold comes with the answer
        Hub::Card { id: Some(_), kind, .. } if kind == "confirm" => return None,
        Hub::Card { text, .. } => Ev::Card { text, closed: String::new() },
        // the approvals gate (approvals-design.md §3.1, §10)
        Hub::Gate(GateStep::Check) => Ev::Gate(crate::wire::Gate::Check),
        Hub::Gate(GateStep::Card) => Ev::Gate(crate::wire::Gate::Card),
        Hub::Gate(GateStep::Done) => Ev::Gate(crate::wire::Gate::Done),
        // a gate's card answered, folded (§9); BISE-307: never cut here,
        // the line cuts at the width and opens whole
        // a sandbox card's fold says the sandbox was off for it (designer);
        // the sentences are bise_proto's (the hub's fold says the same)
        Hub::Approval { how, who, what, note } => {
            let (ok, text, note) = bise_proto::thread::words::approval(&how, &who, &what, &note);
            Ev::Approval { ok, text, note, asked: String::new(), open: false }
        }
        Hub::CardClosed { id, res } => Ev::CardClosed { id, res },
        // an answer to an item: the box's fold line (BISE-305, designer:
        // one sentence), `✓ you answered flow-prompts: oui`
        Hub::Route { who, said, .. } => {
            let (text, note) = cards::answered(&who, &said);
            Ev::Approval { ok: true, text, note, asked: String::new(), open: false }
        }
        // pr-news (pr-design §4)
        Hub::Pr { state, number, url, text } => Ev::Pr { tone: pr_look(state).into(), number, url, text, url_row: false },
        // site/m/artifacts C
        Hub::Artifact { id, agent, title, kind, v } => Ev::Made { id, agent, title, kind, v },
        // site/m/artifacts D
        Hub::Landed { agent, from, sha, files, add, del, .. } => Ev::Landed { agent, from, sha, files, add, del },
        // a spawn (✚), computer use (design §7.3: `↖ api-v2 stopped
        // driving Chrome · you stopped it`), a direct message (⇄), a
        // warning: the notices the hub's fold shows too
        h @ (Hub::Spawn(_) | Hub::Computer(_) | Hub::Direct(_) | Hub::Warn(_)) => {
            return bise_proto::thread::words::hub_notice(&h).map(crate::wire::notice_ev)
        }
        // site/m/timers: a scheduled task set or ended
        Hub::Scheduled(json) => return crate::scheduled::hub_line(&json),
        // sb-core's `stopped` line (an interrupted turn): the TUI already
        // says it in its own lines ('interrupted — …', '▲ turn
        // interrupted', 'in-flight response dropped'), so it draws nothing
        // more and an interrupt reads as on main (architect m_12576); the
        // desktop thread keeps its Stopped entry (bise-proto's fold)
        Hub::Stopped(_) => return None,
        // a known kind's line that doesn't parse, as before
        Hub::Other { kind, text } => match kind.as_str() {
            "card-closed" => Ev::Info(format!("card {} ", text)),
            "route" => Ev::Info(format!("→ {}", text)),
            "landed" => return None,
            _ => Ev::Info(text),
        },
    })
}

/// BISE-04: the hub line protocol, v1 and v2 kinds (contract C2).
#[cfg(test)]
mod hub_line_tests {
    use super::*;

    fn msg(from: &str, to: &str, text: &str, level: u8, id: &str) -> String {
        format!("msg {from}|{to}|{text}|{level}|{id}")
    }

    /// The feed rows of `evs`, as text.
    fn draw(evs: &[Ev]) -> String {
        (0..evs.len())
            .flat_map(|i| crate::feed::build_rows(evs, i, false, 60, 0))
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>() + "\n")
            .collect()
    }

    /// A comparable form of the events these tests look at (`Ev` has no
    /// `PartialEq`).
    fn p(line: &str) -> Option<String> {
        Some(match parse_hub_line(line)? {
            Ev::AgentMsg { from, to, text, level, id, .. } => format!("msg {from}|{to}|{text}|{level}|{id}"),
            Ev::Answered { agent, question, answer, why, .. } => format!("answered {agent}|{question}|{answer}|{why}"),
            Ev::You(t, ..) => format!("you {t}"),
            Ev::Card { text, .. } => format!("card {text}"),
            Ev::CardClosed { id, res } => format!("card-closed {id}|{res}"),
            Ev::Info(t) => format!("info {t}"),
            Ev::Warn(t) => format!("warn {t}"),
            _ => "other".into(),
        })
    }

    #[test]
    fn pr_lines() {
        // pr-news: `pr : tone : number : url : text`, a ` : ` in the text escaped
        let ev = parse_hub_line("pr : red : 412 : https://github.com/o/r/pull/412 : checks fail \\: e2e · dark is on it");
        assert!(
            matches!(&ev, Some(Ev::Pr { tone, number: 412, url, text, .. }) if tone == "red" && url.ends_with("/412") && text == "checks fail : e2e · dark is on it"),
        );
        assert!(draw(&[ev.unwrap()]).contains("#412 checks fail : e2e · dark is on it"));
        // no number: an info line, never lost
        assert_eq!(p("pr : plain : x : u : hi"), Some("info hi".into()));
        // `/prs`: a dim head at col 1, each row a PR line with its URL under it
        let evs = prs_events(&serde_json::json!({"head": "1 PR open", "items": [
            {"number": 415, "url": "https://github.com/o/r/pull/415", "branch": "sb/x", "agents": ["x"], "state": "open", "checks": "fail",
             "failing": ["e2e"], "review": "none", "words": "checks fail: e2e", "text": "sb/x · x · checks fail: e2e"}]}));
        assert!(matches!(&evs[1], Ev::Pr { tone, number: 415, .. } if tone == "red"), "failing checks: red");
        let rows = draw(&evs);
        assert!(rows.starts_with(" 1 PR open\n"), "{rows}");
        assert!(rows.contains("#415 sb/x · x · checks fail: e2e\n   https://github.com/o/r/pull/415"), "{rows}");
    }

    /// Law (architect m_10314): a `/prs` row's tone, from what it means,
    /// is the one the hub sent before (forge::news: Checks::Fail red,
    /// else a draft dim, else plain), for every state and checks.
    #[test]
    fn a_pr_rows_tone_is_its_meanings_look() {
        use bise_proto::rows::{Pr, PrChecks, PrReview, PrState};
        let row = |state, checks| Pr {
            number: 1, url: String::new(), branch: String::new(), agents: vec![], state, checks,
            failing: vec![], review: PrReview::None, words: String::new(), text: String::new(),
            in_review: false, stale_ms: None,
        };
        for state in [PrState::Open, PrState::Draft, PrState::Unknown] {
            for checks in [PrChecks::Pass, PrChecks::Fail, PrChecks::Running, PrChecks::None, PrChecks::Unknown] {
                let before = if checks == PrChecks::Fail { "red" } else if state == PrState::Draft { "dim" } else { "plain" };
                assert_eq!(pr_tone(&row(state, checks)), before, "{state:?} {checks:?}");
            }
        }
    }

    #[test]
    fn v2_kinds() {
        assert_eq!(p("msg : a → b : hi : there"), Some(msg("a", "b", "hi : there", 3, "")));
        // BISE-110: the id after the receiver
        assert_eq!(p("msg : main → docs m_12 : use v2"), Some(msg("main", "docs", "use v2", 3, "m_12")));
        assert_eq!(p("msg : a → b m_x : hi"), Some(msg("a", "b m_x", "hi", 3, "")));
        assert_eq!(p("msg : a → b : one\\ntwo"), Some(msg("a", "b", "one\ntwo", 3, "")));
        assert_eq!(p("msg-you : docs : la v2"), Some(msg("docs", "you", "la v2", 2, "")));
        assert_eq!(
            p("answered : docs : v1 \\: v2? : v2 : the brief says v2").as_deref(),
            Some("answered docs|v1 : v2?|v2|the brief says v2")
        );
        assert_eq!(p("answered : docs : q : a : ").as_deref(), Some("answered docs|q|a|"));
    }

    /// The hub's own id `switchboard` (bise's old name) never reaches the
    /// user: its messages read as from `bise`, in main's feed (`msg`), in
    /// an agent's (`msg-in`) and to the user (`msg-you`); the chip says so.
    #[test]
    fn the_hub_reads_bise() {
        assert_eq!(p("msg : switchboard → main m_7 : card #3 closed"), Some(msg("bise", "main", "card #3 closed", 3, "m_7")));
        assert_eq!(p("msg-in : switchboard m_8 : timer #48"), Some(msg("bise", "", "timer #48", 3, "m_8")));
        assert_eq!(p("msg-you : switchboard : hi"), Some(msg("bise", "you", "hi", 2, "")));
        // a name that only contains it stays
        assert_eq!(p("msg : switchboard-ui → main : hi"), Some(msg("switchboard-ui", "main", "hi", 3, "")));
        let rows = draw(&[parse_hub_line("msg : switchboard → answer-line m_9 : timer #48").unwrap()]);
        assert!(rows.contains("bise → answer-line") && !rows.contains("switchboard"), "{rows}");
    }

    /// An old (v1) transcript still parses to what it drew before.
    #[test]
    fn v1_kinds_still_parse() {
        assert_eq!(p("msg-in : docs m_3 : done : ok"), Some(msg("docs", "", "done : ok", 3, "m_3")));
        assert_eq!(p("msg-in : @docs : la v2"), Some(msg("docs", "you", "la v2", 2, "")));
        assert_eq!(p("you : bonjour"), Some("you bonjour".into()));
        assert_eq!(p("card : #1 question @docs"), Some("card #1 question @docs".into()));
        assert_eq!(p("card-closed : #1 answered"), Some("card-closed 1|answered".into()));
        assert_eq!(p("card-closed : #12 answered via @main"), Some("card-closed 12|answered via @main".into()));
        assert_eq!(p("card-closed : weird"), Some("info card weird ".into()));
        assert_eq!(p("spawn : main → new task @t : x"), Some("info ✚ main → new task @t : x".into()));
        assert_eq!(p("warn : w"), Some("warn w".into()));
        // and draws: an old main feed with every v1 kind
        let evs: Vec<Ev> = [
            "you : bonjour",
            "msg-in : docs m_3 : done",
            "msg-in : @docs : la v2",
            "card : #1 question @docs",
            "card-closed : #1 answered",
            "route : you → @docs : v2",
            "direct : x",
            "warn : w",
        ]
        .iter()
        .filter_map(|l| parse_hub_line(l))
        .collect();
        let text = draw(&evs);
        // level 3: a chip, the id as the receiver (BISE-106)
        let (m3, to_you) = (format!("{} docs → m_3", crate::render::G_ENVELOPE), format!("{G_MSG} docs to you"));
        for want in [m3.as_str(), "done", to_you.as_str(), "la v2", "docs needs you", "→ you → @docs"] {
            assert!(text.contains(want), "{want:?} missing in:\n{text}");
        }
    }

    /// sb-core's `sent` line (architect m_10203): what this feed's owner
    /// sent, a level-3 message from the owner, with its id (so its
    /// `sb send` box goes quiet) and its fields unescaped.
    #[test]
    fn a_sent_line_is_the_owners_message() {
        assert_eq!(p("sent : main : m_9 : 1 : cart \\: or checkout?"), Some(msg("", "main", "cart : or checkout?", 3, "m_9")));
        assert_eq!(p("sent : switchboard : m_4 : 0 : hi"), Some(msg("", "bise", "hi", 3, "m_4")));
        assert_eq!(p("sent : main"), Some("info main".into()), "a short line is never lost");
        crate::render::set_feed_owner("gift-ui");
        let text = draw(&[parse_hub_line("sent : main : m_9 : 1 : cart or checkout?").unwrap()]);
        crate::render::set_feed_owner("");
        let head = format!("{} gift-ui → main", crate::render::G_ENVELOPE);
        for want in [head.as_str(), "cart or checkout?"] {
            assert!(text.contains(want), "{want:?} missing in:\n{text}");
        }
    }

    #[test]
    fn peer_and_answered_draw() {
        let evs = vec![
            parse_hub_line("msg : a → b : hello b").unwrap(),
            parse_hub_line("answered : docs : v1 or v2? : v2 : the brief").unwrap(),
        ];
        let text = draw(&evs);
        let ab = format!("{} a → b", crate::render::G_ENVELOPE);
        let why = format!("{} why", crate::theme::G_CLOSED);
        for want in [ab.as_str(), "hello b", "docs asked: v1 or v2? · i answered: v2", why.as_str()] {
            assert!(text.contains(want), "{want:?} missing in:\n{text}");
        }
    }
}

#[cfg(test)]
pub(crate) mod bench;

/// The inbox for the tests outside `sb` (textlayer_tests.rs): cards
/// (id, kind, agent, text), and whether the card view is open.
#[cfg(test)]
pub(crate) fn set_cards_for_tests(app: &mut App, cards: &[(u64, &str, &str, &str)]) {
    app.sb.cards = cards
        .iter()
        .map(|(id, kind, agent, text)| cards::Card { id: *id, kind: kind.to_string(), agent: agent.to_string(), text: text.to_string(), age_ms: 60_000, ..Default::default() })
        .collect();
}
#[cfg(test)]
pub(crate) fn card_open_for_tests(app: &App) -> bool {
    app.sb.card.open
}
#[cfg(test)]
mod when_tests;
#[cfg(test)]
mod codeblock_tests;

#[cfg(test)]
mod nav_key_tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn nav(code: KeyCode, m: KeyModifiers) -> Option<Nav> {
        nav_key(&KeyEvent::new(code, m))
    }

    /// BISE-302: ctrl+k / ctrl+j no longer move between agents.
    #[test]
    fn alt_down_next_alt_up_previous() {
        assert_eq!(nav(KeyCode::Char('k'), KeyModifiers::CONTROL), None);
        assert_eq!(nav(KeyCode::Char('j'), KeyModifiers::CONTROL), None);
        assert_eq!(nav(KeyCode::Down, KeyModifiers::ALT), Some(Nav::Next));
        assert_eq!(nav(KeyCode::Up, KeyModifiers::ALT), Some(Nav::Prev));
    }

    fn agent(name: &str) -> Agent {
        Agent {
            name: name.into(),
            main: name == "main",
            status: "idle".into(),
            objective: String::new(),
            mode: String::new(),
            branch: None,
            path: String::new(),
            note: String::new(),
            queued: 0,
            turn_ms: None,
            ..Agent::default()
        }
    }

    /// /stop's computer-use key (architect m_13415): an agent's folder from
    /// the snapshot's `dir` (a renamed agent keeps its old folder), its name
    /// when the hub didn't say, and the key is `<hub>.<dir>`.
    #[test]
    fn stop_keys_an_agent_by_its_folder() {
        let mut app = bench::test_app();
        app.sb.agents = vec![Agent { dir: "t1".into(), ..agent("api") }, agent("main")];
        assert_eq!(app.sb.dir_of("api"), "t1");
        assert_eq!(app.sb.dir_of("main"), "main");
        assert_eq!(app.sb.dir_of("gone"), "gone");
        let key = bise_computer_use::who::key("0123456789abcdef", &app.sb.dir_of("api"));
        assert_eq!(bise_computer_use::who::split(&key), (Some("0123456789abcdef"), "t1"));
    }

    fn press(app: &mut App, code: KeyCode, m: KeyModifiers) -> bool {
        key(app, &KeyEvent::new(code, m), false)
    }

    /// BISE-150: a message with an image to a model the catalog lists
    /// without vision is not sent: the no-vision line, the text stays.
    #[test]
    fn images_to_a_model_without_vision_are_refused_before_sending() {
        let mut app = bench::test_app();
        app.sb.agents = vec![agent("main")];
        let usage = |m: &str| Ev::Usage(crate::usage::Usage { model: m.into(), input: 10, ..Default::default() });
        app.events.push(usage("mistral/codestral-latest"));
        app.cache.push(None);
        app.attachments.push(crate::attach::Attachment {
            label: "[Image #1]".into(),
            marker: "<image name=\"[Image #1]\" b64=\"/x.b64\">".into(),
            info: crate::attach::Info { width: 2, height: 3, ..Default::default() },
        });
        app.ed.insert("look at [Image #1]");
        let n = app.events.len();
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.ed.text, "look at [Image #1]", "kept in the composer");
        assert_eq!(app.attachments.len(), 1);
        match app.events.get(n) {
            Some(Ev::Err(e)) => {
                assert!(crate::attach::is_no_vision(e), "{e}");
                assert!(e.contains("mistral/codestral-latest"), "{e}");
            }
            _ => panic!("no no-vision line"),
        }
        // a model that reads images: sent as before
        app.events.push(usage("mistral/mistral-medium-latest"));
        app.cache.push(None);
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.ed.text, "");
        assert!(app.attachments.is_empty());
    }

    /// The panel path of tui_tmux.py: from no selection, ⌥↓ selects
    /// main then the first task; Enter (empty composer) enters its view.
    #[test]
    fn alt_down_then_enter_enters_the_selected_task() {
        let mut app = bench::test_app();
        app.sb.agents = vec![agent("main"), agent("t1")];
        assert!(!press(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL), "ctrl+k is the editor's");
        assert_eq!(app.sb.selected, None);
        assert!(press(&mut app, KeyCode::Down, KeyModifiers::ALT));
        assert_eq!(app.sb.selected, Some(0));
        assert!(press(&mut app, KeyCode::Down, KeyModifiers::ALT));
        assert_eq!(app.sb.selected, Some(1));
        assert!(press(&mut app, KeyCode::Enter, KeyModifiers::NONE));
        let sb = &app.sb;
        assert_eq!(sb.focus, "t1");
        assert_eq!(sb.selected, None);
    }

    /// ⌥↑ goes backwards: from no selection, the last agent first.
    #[test]
    fn alt_up_from_nothing_selects_the_last_agent() {
        let mut app = bench::test_app();
        app.sb.agents = vec![agent("main"), agent("t1"), agent("t2")];
        press(&mut app, KeyCode::Up, KeyModifiers::ALT);
        assert_eq!(app.sb.selected, Some(2));
        press(&mut app, KeyCode::Up, KeyModifiers::ALT);
        assert_eq!(app.sb.selected, Some(1));
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.sb.focus, "t1");
    }

/// D asks first (book §16, BISE-43): the status row says
    /// `archive {name}? …`; n and esc keep the agent; y sends the /archive.
    #[test]
    fn d_asks_before_dropping() {
        use std::io::Read;
        let (a, mut hub) = UnixStream::pair().unwrap();
        hub.set_nonblocking(true).unwrap();
        let (_tx, rx) = mpsc::channel::<String>();
        let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "ws".into());
        let mut app = sb_app(sb, rx, false, 100, crate::voice::Voice::live(false));
        let mut sent = move || {
            let mut buf = vec![0u8; 4096];
            match hub.read(&mut buf) {
                Ok(n) => String::from_utf8_lossy(&buf[..n]).to_string(),
                Err(_) => String::new(),
            }
        };
        app.sb.agents = vec![agent("main"), agent("docs")];
        app.sb.selected = Some(1);
        let status = |app: &App| status_text(app);
        // D: the question, nothing sent
        assert!(press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT));
        assert_eq!(status(&app).trim(), "archive docs? /restore brings it back. y / n");
        assert_eq!(key_mode(&app), crate::keybar::Mode::DropAsk);
        assert_eq!(sent(), "");
        // n keeps it, the composer stays empty
        assert!(press(&mut app, KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(!status(&app).contains("archive docs?"));
        assert_eq!(sent(), "");
        // esc keeps it too
        press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT);
        assert!(press(&mut app, KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.sb.drop_ask, None);
        assert_eq!(sent(), "");
        // y archives it
        press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT);
        assert!(press(&mut app, KeyCode::Char('y'), KeyModifiers::NONE));
        let out = sent();
        assert!(out.contains(r#""method":"agent/archive""#) && out.contains(r#""agent":"docs""#), "{out}");
        assert_eq!(app.ed.text, "");
        // main is never asked about
        app.sb.selected = Some(0);
        press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT);
        assert_eq!(app.sb.drop_ask, None);
    }

    /// BISE-86 (C2 `undelivered`, book §13, §17): the hub could not
    /// deliver your message: your line ends with `✗`, a line says
    /// `✗ not delivered: {name} stopped. ⏎ send again · esc drop`; ⏎ on an
    /// empty composer sends it again (`@name` from another view), esc
    /// drops it; the question then goes away.
    #[test]
    fn a_message_not_delivered_is_marked_and_asks() {
        use std::io::Read;
        let (a, mut hub) = UnixStream::pair().unwrap();
        hub.set_nonblocking(true).unwrap();
        let (_tx, rx) = mpsc::channel::<String>();
        let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "ws".into());
        let mut app = sb_app(sb, rx, false, 100, crate::voice::Voice::live(false));
        let mut sent = move || {
            let mut buf = vec![0u8; 4096];
            match hub.read(&mut buf) {
                Ok(n) => String::from_utf8_lossy(&buf[..n]).to_string(),
                Err(_) => String::new(),
            }
        };
        let ev = parse_hub_line("undelivered : fix : d'abord \\: les tests").unwrap();
        assert!(matches!(&ev, Ev::Undelivered { name, text, open: true } if name == "fix" && text == "d'abord : les tests"));
        push_event(&mut app.events, &mut app.cache, Ev::You("d'abord : les tests".into(), Mark::Sent, false));
        push_event(&mut app.events, &mut app.cache, ev.clone());
        assert!(matches!(&app.events[0], Ev::You(_, Mark::Failed, ..)));
        let rows = |app: &App| -> Vec<String> {
            app.events
                .iter()
                .flat_map(|e| crate::render::ev_lines(e, 80))
                .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
                .collect()
        };
        let r = rows(&app);
        assert!(r[0].trim_end().ends_with("d'abord : les tests ✗"), "{:?}", r);
        assert_eq!(r[1].trim(), "✗ not delivered: fix stopped. ⏎ send again · esc drop");
        // esc drops it: nothing sent, the question goes
        assert!(press(&mut app, KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(sent(), "");
        assert_eq!(rows(&app)[1].trim(), "✗ not delivered: fix stopped.");
        // a second one, from main's view: ⏎ sends it again to @fix
        push_event(&mut app.events, &mut app.cache, Ev::You("encore".into(), Mark::Sent, false));
        push_event(&mut app.events, &mut app.cache, parse_hub_line("undelivered : fix : encore").unwrap());
        assert!(press(&mut app, KeyCode::Enter, KeyModifiers::NONE));
        let out = sent();
        assert!(out.contains(r#""method":"command/run""#) && out.contains("@fix encore"), "{out}");
        assert!(matches!(app.events.last(), Some(Ev::You(t, Mark::Sent, ..)) if t == "@fix encore"));
        // `@fix encore` is the line the user wrote in main's view: marked
        push_event(&mut app.events, &mut app.cache, parse_hub_line("undelivered : fix : encore").unwrap());
        assert!(matches!(app.events.iter().rev().nth(1), Some(Ev::You(t, Mark::Failed, ..)) if t == "@fix encore"));
        // a message the feed does not show (an `@fix` line from another
        // view): it comes back, marked, before the question
        let n = app.events.len();
        push_event(&mut app.events, &mut app.cache, parse_hub_line("undelivered : fix : où ?").unwrap());
        assert!(matches!(&app.events[n], Ev::You(t, Mark::Failed, ..) if t == "@fix où ?"));
        assert!(matches!(&app.events[n + 1], Ev::Undelivered { open: true, .. }));
        // not with a draft: ⏎ sends the draft as usual
        push_event(&mut app.events, &mut app.cache, parse_hub_line("undelivered : fix : x").unwrap());
        app.ed.text = "draft".into();
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(!sent().contains(r#""text":"@fix x""#));
        assert!(app.events.iter().any(|e| matches!(e, Ev::Undelivered { text, open: true, .. } if text == "x")));
    }

    fn infos(app: &App) -> Vec<String> {
        app.events.iter().filter_map(|e| match e { Ev::Info(t) => Some(t.clone()), _ => None }).collect()
    }

    /// No undo (book §13, §17): a typed /cancel, and ctrl+z once the
    /// composer has nothing left to undo, only say so; the draft stays,
    /// nothing goes to the hub.
    #[test]
    fn ctrl_z_and_cancel_say_no_undo() {
        let mut app = bench::test_app();
        app.ed.text = "draft".into();
        assert!(press(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL));
        assert_eq!(app.ed.text, "draft");
        assert_eq!(infos(&app), vec![NO_UNDO.to_string()]);
        assert!(NO_UNDO.starts_with("no undo: an agent may already have acted."));
        let out = handle_input(&mut app, "/cancel");
        assert!(matches!(&out[..], [Ev::Info(t)] if t == NO_UNDO));
        assert!(!crate::commands::COMMANDS.iter().any(|c| c.name == "/cancel"));
    }

    /// /theme switches the palette; /welcome and /theme are listed; the
    /// descriptions are lowercase and say "agent" (book §4).
    #[test]
    fn theme_and_welcome_commands() {
        use crate::theme_detect::{apply, Choice};
        let saved = |c: Choice| (apply(c), Ok(()));
        let ev = theme_command(Some("light"), saved);
        assert!(matches!(&ev, Ev::Info(t) if t == "theme: light."));
        assert_eq!(crate::theme::mode(), crate::theme::Mode::Light);
        let ev = theme_command(Some("dark"), saved);
        assert!(matches!(&ev, Ev::Info(t) if t == "theme: dark."));
        let ev = theme_command(Some("light"), |c| (apply(c), Err("disk full".into())));
        assert!(matches!(&ev, Ev::Warn(t) if t.contains("couldn't save it (disk full)")));
        assert!(matches!(theme_command(Some("blue"), saved), Ev::Warn(_)));
        assert!(matches!(theme_command(None, saved), Ev::Info(t) if t.starts_with("theme: light.")));
        for name in ["/theme", "/welcome"] {
            assert!(crate::commands::COMMANDS.iter().any(|c| c.name == name), "{name}");
        }
        for c in crate::commands::COMMANDS {
            assert!(!c.desc.chars().next().unwrap().is_uppercase(), "{}", c.desc);
            // "scheduled task" is the user's word (card #411, site/m/timers)
            assert!(!c.desc.replace("scheduled task", "").contains("task"), "{}", c.desc);
        }
    }

    /// Ctrl+R belongs to voice input; alt+r is gone (cards v2: typing
    /// in the card view is answering): neither touches a card.
    #[test]
    fn alt_r_and_ctrl_r_are_not_the_cards() {
        let mut app = bench::test_app();
        app.sb.cards = vec![Card { id: 7, kind: "question".into(), agent: "t1".into(), text: "which one?".into(), ..Card::default() }];
        app.ed.text = "the first".into();
        assert!(!press(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(!press(&mut app, KeyCode::Char('r'), KeyModifiers::ALT));
        assert_eq!(app.ed.text, "the first");
        assert!(!app.sb.card.open);
    }

    /// A non-empty composer keeps Enter for sending: no view change.
    #[test]
    fn enter_with_text_does_not_enter_the_selection() {
        let mut app = bench::test_app();
        app.sb.agents = vec![agent("main"), agent("t1")];
        app.sb.selected = Some(1);
        app.ed.text = "hello".into();
        assert!(!press(&mut app, KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.sb.focus, "main");
    }

    #[test]
    fn alt_digits_go_to_agent_n() {
        for d in 0..=9u32 {
            let c = char::from_digit(d, 10).unwrap();
            assert_eq!(nav(KeyCode::Char(c), KeyModifiers::ALT), Some(Nav::Goto(d as usize)));
            // Ctrl+digit is not bound (most terminals cannot send it)
            assert_eq!(nav(KeyCode::Char(c), KeyModifiers::CONTROL), None);
        }
    }

    #[test]
    fn plain_keys_and_card_keys_are_not_navigation() {
        assert_eq!(nav(KeyCode::Char('1'), KeyModifiers::NONE), None);
        assert_eq!(nav(KeyCode::Char('k'), KeyModifiers::NONE), None);
        assert_eq!(nav(KeyCode::Enter, KeyModifiers::NONE), None);
        assert_eq!(nav(KeyCode::Esc, KeyModifiers::NONE), None);
        for c in ['g', 'f', 'n', 'p', 'r', 'x', 'a', 'c', 'o', 'z'] {
            assert_eq!(nav(KeyCode::Char(c), KeyModifiers::CONTROL), None);
        }
    }
}
