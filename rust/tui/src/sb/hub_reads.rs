//! The hub's notifications the terminal reads typed (client-protocol
//! step 4; plan v2 signed by architect m_13977), on a connection opened
//! with `initialize` (P4e-1: [`init_line`]): its answer says who the hub
//! is (the workspace, the version it runs, a reload) and holds the
//! hub-wide state at a watermark ([`initialized`]); then each hub-wide
//! notification goes through the watermark (`rpc::Watermark::take`: the
//! next one applies, a seen one is skipped, a gap or a new epoch reads
//! `hub/read` again, whose state replaces it). The terminal is ready then
//! ([`ready`]); what keep-state restores waits for the first page of the
//! thread it shows (sb/feed_entries.rs `first_page_in`). Each
//! reader takes the typed `HubEv` (`rpc::ev` of the notification), so the
//! terminal and the window read one shape.
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
//!   repo's flow;
//! - `hub/notice`: the hub's words, info lines (the queue moves on);
//! - `card/open`: update-card's item opens (`/update` found a release);
//! - `client/focused`: the hub moved his focus (its agent is gone);
//! - `hub/versions`: `/version`'s picker and the dev build (sb/versions.rs);
//! - `release/progress`, `update/progress`: a `/release-bise` run's steps
//!   and `/update`'s build in bise's source tree (sb/release.rs).

use super::*;
use bise_proto::hub::HubEv;
use bise_proto::rows::{ApprovalMode, ApprovalRule, CheckerKind};
use bise_proto::rpc::{self, HubState, Id, Init, InitializeParams, InitializeResult, Message, Notification, Request, Take};

/// `initialize`'s id (the terminal's requests start at 1, sb/rpc.rs).
const INIT: u64 = 0;

/// The terminal's first line on the hub's socket (at start and on every
/// reconnection): `initialize`, this client's name and version.
pub fn init_line() -> String {
    let params = serde_json::to_value(InitializeParams::new("bise-tui", env!("CARGO_PKG_VERSION"))).unwrap_or_default();
    format!("{}\n", Message::Request(Request::new(Id::Num(INIT), rpc::INITIALIZE, params)).to_value())
}

/// `initialize`'s answer (until it came on this connection): the hub's
/// facts and state; the hub refused this client (it runs in an agent's
/// process, docs/issues/16): why, and the terminal ends; an older hub
/// that doesn't serve it: the same, with what to do. True: the line was
/// that answer.
pub(super) fn init(app: &mut App, v: &Value) -> bool {
    if app.sb.rpc.initialized {
        return false;
    }
    match rpc::init_answer(v, &Id::Num(INIT)) {
        Init::Ready(res) => initialized(app, *res),
        Init::Refused(why) => {
            client::set_refused(why);
            app.should_quit = true;
        }
        Init::Older => {
            client::set_refused("the hub runs an older bise, which this terminal can't read: `bise stop`, then `bise` again".into());
            app.should_quit = true;
        }
        Init::Other => return false,
    }
    true
}

/// `initialize` answered: who the hub is (the terminal follows its
/// version: another binary, or a reload), its state at its watermark,
/// then the thread in view subscribed (its first page makes the terminal
/// ready).
fn initialized(app: &mut App, res: InitializeResult) {
    let sb = &mut app.sb;
    sb.rpc.initialized = true;
    sb.workspace = res.workspace;
    crate::artifacts::set_workspace(&sb.workspace);
    sb.version = res.version.pointer("/id").and_then(Value::as_str).unwrap_or("").to_string();
    let first = sb.reload_seen.is_none();
    let reloaded = !first && !res.reload.is_empty() && sb.reload_seen.as_deref() != Some(res.reload.as_str());
    if first {
        sb.reload_seen = Some(res.reload);
    }
    // a reload (BISE-131) starts the same binary again; both wait for the
    // keys to stop (sb/reload_wait.rs, run.rs quits); the drafts, queues
    // and view are written when the UI ends
    if (!res.exe.is_empty() && follow_hub_exe(&res.exe)) || (reloaded && follow_reload()) {
        app.sb.reload_wait.ask(std::time::Instant::now());
    }
    // the threads first (the older hello's place, before its burst): the
    // agents rows then subscribe the other live agents, never the focus
    // twice (a second first page would replace the restored scroll)
    feed_entries::on_connect(app);
    apply_state(app, res.hub);
    ready(app);
}

/// The hub-wide state (`initialize`'s, `hub/read`'s): its watermark, and
/// each kind's notification in the order the older hello burst had them
/// (the agents, cards, scheduled tasks and flow first, the approvals,
/// the artifacts last), so the readers' side effects come as before.
fn apply_state(app: &mut App, st: HubState) {
    app.sb.rpc.wm = st.watermark;
    let mut notes = st.state;
    notes.sort_by_key(|n| burst_rank(&n.method));
    for n in &notes {
        apply(app, n);
    }
}

/// A kind's place in the state's order (stable: the hub's order inside
/// one rank).
fn burst_rank(method: &str) -> u8 {
    match method {
        "hub/agents" => 0,
        "hub/cards" => 1,
        "hub/scheduled" => 2,
        "hub/flow" => 3,
        "hub/approvals" => 5,
        "hub/artifacts" => 6,
        _ => 4,
    }
}

/// `hub/read`'s answer (a gap or a new epoch): the state again.
pub(super) fn reread(app: &mut App, v: Value) {
    app.sb.rpc.reading = false;
    if let Ok(st) = serde_json::from_value::<HubState>(v) {
        apply_state(app, st);
    }
}

/// The terminal is ready (what the older `ready` event did, once
/// `initialize` answered): the queues saved by the terminal before this
/// one (a reload, a restart) come back now that the agents rows say who
/// is busy; what was open and the scroll (keep-state) once the focus's
/// first page is in (feed_entries::on_ready, first_page_in).
pub(crate) fn ready(app: &mut App) {
    app.sb.ready = true;
    drafts::requeue(app);
    feed_entries::on_ready(app);
}

/// A JSON-RPC notification from the hub (a line with `jsonrpc` and a
/// `method`): a hub-wide one through the watermark, then its typed
/// reader. A method this terminal doesn't read (a newer hub's): nothing.
pub(super) fn read(app: &mut App, v: Value) {
    let Ok(Message::Notification(n)) = Message::from_value(v) else { return };
    let Ok((_, Some(w))) = rpc::ev(&n) else { return apply(app, &n) };
    let calls = &mut app.sb.rpc;
    // no watermark before initialize's answer (its state replaces what
    // came before it)
    if !calls.initialized {
        return apply(app, &n);
    }
    // a `hub/read` on its way: its state covers what comes before it
    if calls.reading {
        return;
    }
    match calls.wm.take(w) {
        Take::Apply => apply(app, &n),
        Take::Skip => {}
        Take::Resync => {
            app.sb.rpc.reading = true;
            app.sb.call(rpc::HUB_READ, json!({}), super::rpc::Then::HubRead);
        }
    }
}

/// One notification's typed reader.
fn apply(app: &mut App, n: &Notification) {
    let Ok((ev, _)) = rpc::ev(n) else { return };
    match &ev {
        HubEv::Approvals { .. } => approvals(app, &ev, false),
        HubEv::Confirm { id, text, .. } => confirm(app, *id, text),
        HubEv::Artifacts { items, seen_ms, .. } => artifacts(app, items, *seen_ms),
        HubEv::Agents { agents: rows, places, .. } => agents(app, rows, places),
        HubEv::Cards { cards: his, others, .. } => cards(app, his, others),
        HubEv::Scheduled { items, ended, .. } => app.sb.timers = items.iter().chain(ended).map(super::state_rows::task_of_row).collect(),
        HubEv::Flow { flow, .. } => app.sb.flow = super::state_rows::flow_word(*flow),
        HubEv::Notice { text, .. } => notice(app, text),
        // update-card: `/update` with a newer release opens its item here
        HubEv::CardOpen { id, .. } => cards::open_view(app, Some(*id)),
        // the hub moved his focus (the agent he was on is gone)
        HubEv::Focused { focus, .. } => super::focus(app, focus),
        HubEv::Entry { agent, entry, .. } => feed_entries::entry(app, agent, entry),
        // P4c-5: `/version`'s picker, a release run's steps, `/update`'s build
        HubEv::Versions { dev, items, .. } => versions::set(app, *dev, items),
        HubEv::Release(r) => release::event(app, r),
        HubEv::Update(u) => release::update_event(app, u),
        _ => {}
    }
}

/// The agents and their places: renames move their feed and the focus,
/// an open diff panel on one follows its changes, every feed's spinner
/// follows its agent, the first agent's hint, the tour.
fn agents(app: &mut App, rows: &[bise_proto::rows::Agent], places: &[bise_proto::rows::Place]) {
    let new: Vec<Agent> = rows.iter().map(Agent::of_row).collect();
    // each row's turn edges (P4d-feed, proto-lead m_14731): a run flip
    // or an ended turn, never missed; both halves from sb-core's one
    // view (issue 22)
    for a in rows {
        feed_entries::agent_row(app, &a.name, a.turn_running(), a.turns);
        // every live agent's feed is kept as the lines kept it (its
        // preview, its cards, its dot by kind): subscribed once per
        // connection; an archived one lights by its newest entry
        // (plan v2 read 4)
        if !a.archived {
            feed_entries::subscribe(app, &a.name);
        } else if !app.sb.subscribed.contains(&a.name) {
            crate::entry_reads::on_head(app, &a.name, a.last_pos);
        }
    }
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

/// The hub's words (`hub/notice`): one info line per line. The hub
/// refused an input (it says so in a notice): a queued message that went
/// starts no turn, so the queue moves on.
fn notice(app: &mut App, text: &str) {
    crate::queue::seen(app);
    for l in text.lines() {
        push_event(&mut app.events, &mut app.cache, Ev::Info(l.to_string()));
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

/// The hub's typed state rows for tests (what an initialized connection
/// gets): an agent by its status word, a card by its words, their
/// notifications as the lines `dispatch` reads.
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
            batch: None,
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
    use bise_proto::rpc::Watermark;

    fn note(ev: &HubEv) -> Value {
        Message::Notification(rpc::note(ev, None).unwrap()).to_value()
    }

    /// `initialize`'s answer with the hub-wide state `notes` at `wm`.
    fn init_answer(notes: Vec<Notification>, wm: Watermark) -> String {
        let res = json!({"project": "p", "proto": bise_proto::PROTO, "workspace": "/ws", "name": "ws", "exe": "",
            "version": {"id": "v9"}, "reload": "", "methods": [], "notifications": [],
            "hub": {"watermark": wm, "state": notes}});
        Message::Response(rpc::Response::ok(Id::Num(INIT), res)).to_value().to_string()
    }

    fn numbered(ev: &HubEv, seq: u64) -> String {
        Message::Notification(rpc::note(ev, Some(Watermark { epoch: 3, seq })).unwrap()).to_value().to_string()
    }

    fn flow(f: bise_proto::rows::FlowMode) -> HubEv {
        HubEv::Flow { project: "p".into(), flow: Some(f) }
    }

    #[test]
    fn the_first_line_is_initialize() {
        let v: Value = serde_json::from_str(init_line().trim()).unwrap();
        assert_eq!((v["method"].as_str(), v["id"].as_u64(), v["params"]["client"]["name"].as_str()), (Some("initialize"), Some(INIT), Some("bise-tui")));
        assert_eq!(v["params"]["proto"], bise_proto::PROTO);
    }

    #[test]
    fn initialize_answered_gives_the_facts_and_the_state_in_the_bursts_order() {
        let mut app = crate::sb::bench::test_app();
        let agents = rows_for_tests::lines(vec![rows_for_tests::agent("main", "idle", ""), rows_for_tests::agent("docs", "working", "the docs")], vec![rows_for_tests::card(4, "question", "docs", "v1?")]);
        let notes: Vec<Notification> = agents.iter().rev().map(|l| match Message::from_value(serde_json::from_str(l).unwrap()) {
            Ok(Message::Notification(n)) => n,
            _ => unreachable!(),
        }).chain(rpc::note(&flow(bise_proto::rows::FlowMode::Trunk), None)).collect();
        crate::sb::dispatch(&mut app, &init_answer(notes, Watermark { epoch: 3, seq: 10 }));
        let sb = &app.sb;
        assert!(sb.rpc.initialized);
        assert_eq!((sb.workspace.as_str(), sb.version.as_str(), sb.flow.as_str()), ("/ws", "v9", "trunk"));
        assert_eq!((sb.agents.len(), sb.cards.len()), (2, 1), "cards after agents, as the burst had them");
        assert_eq!(sb.rpc.wm, Watermark { epoch: 3, seq: 10 });
        assert!(sb.ready, "ready once initialize answered");
        assert_eq!(sb.ready_page.as_deref(), Some("main"), "the focus is subscribed: keep-state waits for its first page");
        // the answer only once per connection
        assert!(!init(&mut app, &serde_json::from_str(&init_answer(vec![], Watermark::default())).unwrap()));
    }

    #[test]
    fn the_watermark_applies_the_next_skips_the_seen_and_reads_again_on_a_gap() {
        let mut app = crate::sb::bench::test_app();
        crate::sb::dispatch(&mut app, &init_answer(vec![], Watermark { epoch: 3, seq: 10 }));
        crate::sb::dispatch(&mut app, &numbered(&flow(bise_proto::rows::FlowMode::Pr), 11));
        assert_eq!(app.sb.flow, "pr", "the next one applies");
        crate::sb::dispatch(&mut app, &numbered(&flow(bise_proto::rows::FlowMode::Trunk), 11));
        assert_eq!(app.sb.flow, "pr", "a seen one is skipped");
        crate::sb::dispatch(&mut app, &numbered(&flow(bise_proto::rows::FlowMode::Trunk), 13));
        assert!(app.sb.rpc.reading && app.sb.flow == "pr", "a gap: hub/read, nothing applied");
        crate::sb::dispatch(&mut app, &numbered(&flow(bise_proto::rows::FlowMode::Trunk), 14));
        assert_eq!(app.sb.flow, "pr", "while reading, its state covers what comes");
        // hub/read's answer replaces the state and the watermark
        let st = json!({"watermark": {"epoch": 4, "seq": 2}, "state": [rpc::note(&flow(bise_proto::rows::FlowMode::Trunk), None)]});
        reread(&mut app, st);
        assert_eq!((app.sb.flow.as_str(), app.sb.rpc.wm, app.sb.rpc.reading), ("trunk", Watermark { epoch: 4, seq: 2 }, false));
        crate::sb::dispatch(&mut app, &numbered(&flow(bise_proto::rows::FlowMode::Pr), 3));
        assert_eq!(app.sb.flow, "trunk", "another epoch: read again");
    }

    #[test]
    fn a_refusal_ends_the_terminal_and_ready_comes_once() {
        let mut app = crate::sb::bench::test_app();
        let refused = json!({"jsonrpc": "2.0", "id": INIT, "error": {"code": rpc::code::REFUSED, "message": "runs in an agent"}});
        crate::sb::dispatch(&mut app, &refused.to_string());
        assert!(app.should_quit);
        assert_eq!(client::take_refused().as_deref(), Some("runs in an agent"));
        let mut app = crate::sb::bench::test_app();
        app.sb.ready_page = Some("main".into());
        ready(&mut app);
        assert!(app.sb.ready && app.sb.ready_page.is_some(), "keep-state waits for the focus's page");
        feed_entries::first_page_in(&mut app, "main");
        assert!(app.sb.ready_page.is_none());
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
