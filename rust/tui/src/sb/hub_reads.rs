//! The hub's notifications the terminal reads typed (client-protocol
//! step 4, P4c; plan v2 signed by architect m_13977): its hello lists
//! them in `reads` ([`READS`]) and the hub sends them instead of their
//! older events (`bise_proto::rpc::OLDER`, one kind at a time, never
//! twice). Each reader takes the typed `HubEv` (`rpc::ev` of the
//! notification), so the terminal and the window read one shape; the
//! other kinds still come the older way (sb.rs's dispatch) until their
//! chunk moves them.
//!
//! - `hub/approvals`: the mode for the key bar, the checker and the rules
//!   (`/approvals`' read, approvals/set without a mode, opens the screen
//!   on the same fields: sb/rpc.rs's `Then::Approvals`);
//! - `confirm/ask`: the hub's yes/no question (`y`/`n`, esc: no);
//! - `hub/artifacts`: the list, its new ones and when he last looked
//!   (`artifacts.rs`'s store, the header's `↗ N new`, /artifacts);
//! - `hub/agents`, `hub/cards`, `hub/scheduled`, `hub/flow` (P4c-4b, the
//!   older `state` event split by kind; state_rows.rs maps the rows): the
//!   agents and the worktrees they sit in (a rename is the same `dir`
//!   with a new name, architect Q3), the inbox (his cards and bise's
//!   own), the scheduled tasks (live, then the week's ended ones), the
//!   repo's flow.

use super::*;
use bise_proto::hub::HubEv;
use bise_proto::rows::{ApprovalMode, ApprovalRule, CheckerKind};
use bise_proto::rpc::{self, Message};

/// The notifications this terminal reads typed: whole rows of
/// `bise_proto::rpc::OLDER` (a half-listed row comes the older way).
// TODO(client-protocol step 4's end, P4e): the hello's `reads` goes when
// the terminal connects with `initialize`
pub(crate) const READS: &[&str] = &["hub/approvals", "confirm/ask", "hub/artifacts", "hub/agents", "hub/cards", "hub/scheduled", "hub/flow"];

/// The terminal's first line on the hub's socket: `hello` with [`READS`].
pub fn hello_line() -> String {
    format!("{}\n", json!({"op": "hello", "reads": READS}))
}

/// A JSON-RPC notification from the hub (a line with `jsonrpc` and a
/// `method`): its typed reader. A method this terminal doesn't read
/// (a newer hub's): nothing.
pub(super) fn read(app: &mut App, v: Value) {
    let Ok(Message::Notification(n)) = Message::from_value(v) else { return };
    let Ok((ev, _watermark)) = rpc::ev(&n) else { return };
    match &ev {
        HubEv::Approvals { .. } => approvals(app, &ev, false),
        HubEv::Confirm { id, text, .. } => confirm(app, *id, text),
        HubEv::Artifacts { items, seen_ms, .. } => artifacts(app, items, *seen_ms),
        HubEv::Agents { agents: rows, places, .. } => agents(app, rows, places),
        HubEv::Cards { cards: his, others, .. } => cards(app, his, others),
        HubEv::Scheduled { items, ended, .. } => app.sb.timers = items.iter().chain(ended).map(super::state_rows::task_of_row).collect(),
        HubEv::Flow { flow, .. } => app.sb.flow = super::state_rows::flow_word(*flow),
        _ => {}
    }
}

/// The agents and their places: renames move their feed and the focus,
/// an open diff panel on one follows its changes, every feed's spinner
/// follows its agent, the first agent's hint, the tour.
fn agents(app: &mut App, rows: &[bise_proto::rows::Agent], places: &[bise_proto::rows::Place]) {
    let new: Vec<Agent> = rows.iter().map(Agent::of_row).collect();
    // TODO(proto-zone-b, m_14782: its P4d-feed f-a sha): each row's turn
    // edges, `for a in rows { feed_entries::agent_row(app, &a.name,
    // a.status.working(), a.turns) }`, once feed_entries.rs is in
    for (old, name) in super::state_rows::renamed(&app.sb.agents, &new) {
        let sb = &mut app.sb;
        if let Some(view) = sb.views.remove(&old) {
            sb.views.insert(name.clone(), view);
        }
        if sb.focus == old {
            sb.focus = name;
        }
    }
    app.sb.agents = new;
    app.sb.places = places.iter().map(super::places::Place::of_row).collect();
    // an open diff panel on an agent follows its changes (site/m/artifacts D)
    if let Some(crate::diffview::Ask::Agent(name)) = app.diff.as_ref().map(|p| p.ask.clone()) {
        let changes = app.sb.agents.iter().find(|a| a.name == name).and_then(|a| a.changes);
        crate::diffview::on_changes(app, &name, changes);
    }
    // the spinner of every feed follows the agent, whoever started the turn
    let sb = &mut app.sb;
    let busy: HashMap<String, bool> = sb.agents.iter().map(|a| (a.name.clone(), a.busy())).collect();
    for (name, view) in sb.views.iter_mut() {
        view.pending = busy.get(name).copied().unwrap_or(false);
    }
    let focus_busy = busy.get(&sb.focus).copied().unwrap_or(false);
    app.pending = focus_busy;
    if !focus_busy {
        app.interrupt_requested = false;
    }
    keep_selection(app);
    // BISE-61: the first agent (a one-time hint)
    if app.sb.agents.iter().any(|a| !a.main && !a.archived()) {
        crate::hints::once(app, crate::hints::Hint::FirstAgent);
    }
    crate::tour::on_state(app);
}

/// The inbox: his cards and bise's own in one list (the drop asking once
/// more stays), the TUI's setup items put back, a new card wakes zen, the
/// cards view follows, the first card's hint, the tour.
fn cards(app: &mut App, his: &[bise_proto::rows::Card], others: &[bise_proto::rows::Card]) {
    let sb = &mut app.sb;
    let known: Vec<u64> = sb.cards.iter().map(|c| c.id).collect();
    sb.cards = super::state_rows::cards_of_rows(his, others, sb.feature_drop_ask, crate::when::now_ms());
    // the setup cards are the TUI's own: not in the hub's rows
    super::setup::put_back(sb);
    // zen (BISE-121): a card that was not there
    if sb.cards.iter().any(|c| !known.contains(&c.id)) {
        sb.calls += 1;
    }
    super::cards::sync(app);
    keep_selection(app);
    // BISE-61: the first card (a one-time hint)
    if app.sb.cards.is_empty() {
        crate::hints::used(crate::hints::Hint::FirstCard);
    } else {
        crate::hints::once(app, crate::hints::Hint::FirstCard);
    }
    crate::tour::on_state(app);
}

/// A panel row selected past the end of the list (an agent or a card
/// went): none.
fn keep_selection(app: &mut App) {
    let sb = &mut app.sb;
    if sb.selected.is_some_and(|sel| sel >= sb.nav().len()) {
        sb.selected = None;
        sb.preview = false;
    }
}

/// The word the key bar and the screen use; an older or unknown mode is
/// "" (no switch, as an older hub that never said one).
fn mode_word(m: ApprovalMode) -> String {
    match m {
        ApprovalMode::Yolo => "yolo".into(),
        ApprovalMode::Auto => "auto".into(),
        ApprovalMode::Unknown => String::new(),
    }
}

fn checker_word(c: CheckerKind) -> String {
    match c {
        CheckerKind::Jev => "jev".into(),
        CheckerKind::Model => "model".into(),
        CheckerKind::Off => "off".into(),
        CheckerKind::Unknown => String::new(),
    }
}

impl Rule {
    /// The hub's row; `raw` is what approvals/removeRule sends back.
    pub(crate) fn of_row(r: &ApprovalRule) -> Rule {
        Rule {
            raw: serde_json::to_value(r).unwrap_or(Value::Null),
            tool: r.tool.clone(),
            pattern: r.pattern.clone().unwrap_or_default(),
            path: r.path.clone().unwrap_or_default(),
            every: r.project.as_deref().is_none_or(str::is_empty),
            // the hub writes ms; a hand-written date is not read
            added: r.added_ms,
            from: r.from.clone().unwrap_or_default(),
            outside: r.sandbox == Some(false),
        }
    }
}

/// The hub's approvals (approvals-design.md §8): the mode for the key
/// bar; `flash`: a switch (the 3-second flash, the first switch to auto's
/// tip); `show`: `/approvals` asked (its screen opens).
pub(super) fn approvals(app: &mut App, ev: &HubEv, show: bool) {
    let HubEv::Approvals { mode, env, checker, checker_who, checker_model, repo, rules, flash, .. } = ev else { return };
    let mode = mode_word(*mode);
    let was = app.sb.approvals.mode.clone();
    if was.is_empty() && mode == "yolo" {
        crate::hints::once(app, crate::hints::Hint::FirstYolo);
    }
    let a = &mut app.sb.approvals;
    a.mode = mode;
    a.env = *env;
    a.checker = checker_word(*checker);
    a.checker_who = checker_who.clone();
    a.checker_model = checker_model.clone().unwrap_or_default();
    a.repo = repo.clone();
    a.rules = rules.iter().map(Rule::of_row).collect();
    if *flash {
        a.flash = Some(std::time::Instant::now());
        if a.mode == "auto" && was != "auto" && a.checker != "off" {
            crate::hints::set_auto_text(&a.checker, checker_who);
            crate::hints::once(app, crate::hints::Hint::FirstAuto);
        }
    }
    if show {
        app.approvals = Some(crate::approvals_screen::Screen::default());
    }
}

/// The artifacts (site/m/artifacts, docs/artifacts.md): the whole list;
/// an open /artifacts marks what came new; the feed's chips are built
/// again with the new titles.
fn artifacts(app: &mut App, items: &[bise_proto::rows::Artifact], seen_ms: Option<u64>) {
    crate::artifacts::set_rows(items, seen_ms);
    crate::artifacts_screen::on_list(app);
    app.cache.iter_mut().for_each(|c| *c = None);
}

/// The hub asks yes or no (`/archive` of a task with unpushed work...):
/// its words, how to answer, and the question kept for `y`/`n`.
fn confirm(app: &mut App, id: u64, text: &str) {
    push_event(&mut app.events, &mut app.cache, Ev::Warn(text.to_string()));
    push_event(&mut app.events, &mut app.cache, Ev::Info("answer y (yes) or n (no), then ⏎".into()));
    let sb = &mut app.sb;
    sb.confirm = Some((id, text.to_string()));
    sb.calls += 1;
}

/// The hub's typed state rows for tests (what the hub sends after a
/// hello that lists [`READS`]): an agent by its status word, a card by
/// its words, their notifications as the lines `dispatch` reads.
#[cfg(test)]
pub(crate) mod rows_for_tests {
    use bise_proto::hub::HubEv;
    use bise_proto::rows::{Agent, Card, Phase, Status};
    use bise_proto::rpc::{self, Message};

    pub(crate) fn agent(name: &str, status: &str, objective: &str) -> Agent {
        let (st, archived) = Status::of_hub(status);
        let mut a: Agent = serde_json::from_value(serde_json::json!({
            "name": name, "main": name == "main", "status": st, "archived": archived,
            "title": "", "purpose": "", "since_ms": 0, "waits": 0, "dir": name, "objective": objective,
        }))
        .unwrap();
        a.phase = Phase::of_hub(status);
        a
    }

    pub(crate) fn card(id: u64, kind: &str, agent: &str, text: &str) -> Card {
        let (question, options) = bise_proto::rows::question(text);
        Card {
            id,
            project: "p".into(),
            kind: kind.into(),
            agent: agent.into(),
            question,
            options,
            urgent: false,
            since_ms: crate::when::now_ms(),
            page: None,
            approval: false,
            rank: None,
            waiting: None,
            text: text.into(),
            note: None,
            place: None,
            pr: None,
            link: None,
            for_msg: None,
        }
    }

    /// hub/agents then hub/cards (a question is his, any other kind
    /// bise's own), as JSON-RPC lines.
    pub(crate) fn lines(agents: Vec<Agent>, cards: Vec<Card>) -> [String; 2] {
        let (his, others) = cards.into_iter().partition(|c| c.kind == "question" || c.kind == "confirm");
        let line = |ev: &HubEv| Message::Notification(rpc::note(ev, None).unwrap()).to_value().to_string();
        [
            line(&HubEv::Agents { project: "p".into(), agents, places: vec![] }),
            line(&HubEv::Cards { project: "p".into(), cards: his, others }),
        ]
    }

    /// Both lines through `dispatch`, as the hub's socket gives them.
    pub(crate) fn apply(app: &mut crate::app::App, agents: Vec<Agent>, cards: Vec<Card>) {
        for l in lines(agents, cards) {
            crate::sb::dispatch(app, &l);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(ev: &HubEv) -> Value {
        Message::Notification(rpc::note(ev, None).unwrap()).to_value()
    }

    #[test]
    fn the_hello_lists_whole_older_rows_only() {
        let listed: Vec<String> = READS.iter().map(|s| s.to_string()).collect();
        let reads = rpc::reads_of(&listed);
        assert_eq!(reads.len(), READS.len(), "every method listed is part of a whole OLDER row");
        let hello: Value = serde_json::from_str(hello_line().trim()).unwrap();
        assert_eq!((hello["op"].as_str(), hello["reads"].as_array().map(Vec::len)), (Some("hello"), Some(READS.len())));
    }

    #[test]
    fn hub_approvals_sets_the_mode_rules_and_flash_as_the_older_event_did() {
        let mut app = crate::sb::bench::test_app();
        let rule = ApprovalRule {
            tool: "bash".into(),
            pattern: Some("cargo test *".into()),
            path: None,
            project: None,
            added: Some("1791100000000".into()),
            from: Some("card #3, api".into()),
            sandbox: Some(false),
            added_ms: Some(1791100000000),
            what: "cargo test *".into(),
            note: String::new(),
        };
        let ev = HubEv::Approvals {
            project: "p".into(),
            mode: ApprovalMode::Auto,
            env: false,
            checker: CheckerKind::Off,
            checker_who: String::new(),
            checker_model: None,
            repo: "/r".into(),
            rules: vec![rule],
            flash: true,
        };
        read(&mut app, note(&ev));
        let a = &app.sb.approvals;
        assert_eq!((a.mode.as_str(), a.checker.as_str(), a.repo.as_str()), ("auto", "off", "/r"));
        assert!(a.flash.is_some(), "a switch flashes");
        let r = &a.rules[0];
        assert_eq!((r.tool.as_str(), r.pattern.as_str(), r.every, r.added, r.outside), ("bash", "cargo test *", true, Some(1791100000000), true));
        assert_eq!(r.raw["added"], "1791100000000", "removeRule sends the hub's fields back");
        assert!(app.approvals.is_none(), "a notification never opens the screen");
    }

    #[test]
    fn confirm_ask_asks_and_keeps_the_question() {
        let mut app = crate::sb::bench::test_app();
        let n = app.events.len();
        read(&mut app, note(&HubEv::Confirm { project: "p".into(), id: 7, text: "t4 has 1 unpushed commit: archive it?".into() }));
        assert_eq!(app.sb.confirm, Some((7, "t4 has 1 unpushed commit: archive it?".into())));
        assert_eq!(app.events.len(), n + 2);
        // a method this terminal doesn't read: nothing
        read(&mut app, json!({"jsonrpc": "2.0", "method": "hub/newer", "params": {}}));
        assert_eq!(app.events.len(), n + 2);
    }
}
