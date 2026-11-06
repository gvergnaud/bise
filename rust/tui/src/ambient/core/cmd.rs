//! The app's commands (stdin, docs/ambient-app.md §4) and their parse.

use bise_proto::app_cmd::AppCmd;
use bise_proto::hub::Mode;
use serde_json::Value;
use std::path::PathBuf;

/// A command of the app (stdin), §4.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    TalkStart,
    /// `talk_start` with `page: {id, url}`: a note talk on that bise page
    /// (docs/ambient-pages.md §4.1), its words to the page, never to main
    PageTalk { page: String },
    TalkEnd,
    TalkCancel,
    /// his ✕ on the talk's still (ambient-lead m_21912): it is forgotten,
    /// the talk goes on; `id`: only that still (the spotlight closed on it)
    ShotDrop { id: Option<u64> },
    /// The composer's mic (bar I, core/dictate.rs): `id` is the window's,
    /// echoed on each `dictation`.
    DictateStart { id: String },
    DictateStop { id: String },
    DictateCancel { id: String },
    /// a kept take's sound transcribed again (core/dictate.rs)
    DictateRetry { id: String },
    /// bar N14 (core/voice_answer.rs): the card in front of him in the
    /// focused window, or none.
    CardInView { project: Option<String>, card: Option<u64> },
    /// The composer's lists (desktop C/A/B, core/picks.rs): the `/`
    /// commands, a workspace's `$` skills, an `@` query's files.
    Commands,
    /// ⌘K opened (core/index.rs): the index of the projects not held
    Index,
    Skills { project: String },
    Files { project: String, q: String, rid: u64, limit: Option<u32> },
    /// R14: a path he took from a workspace's `@` list
    FilePicked { project: String, path: String },
    Shot { path: PathBuf, app: String, title: String },
    /// He's back after `away_ms` (amb-mac m_9038): `away_summary` answers.
    AwayBack { away_ms: u64 },
    Send { text: String },
    /// fn space's typed text sent with tab: main starts an agent on it
    /// (only main starts agents; ambient-lead m_6513)
    Start { text: String },
    Answer { card: u64, reply: String },
    Hush,
    CutIn,
    /// The user is on a call or in a meeting (the app's read): text only,
    /// no voice (docs/ambient-app.md §4 `quiet`).
    Quiet { on: bool },
    /// The settings' voice answers (amb-mac m_5327): off, main's answers
    /// are text only; on (the default), as usual.
    Voice { on: bool },
    /// The menu bar's "stop watching" (roadmap B, amb-mac m_5475): the
    /// standing order `id` stops (the hub's scheduled/stop).
    EveryStop { id: u64 },
    /// Round 10 (identity10 #data): an agent's preview (now, its last
    /// actions, what waits on him, its last report, its pages), re-sent
    /// on its events while it stays selected.
    AgentPreview { agent: String },
    /// An agent's history panel: the newest entries (before: None, the
    /// panel opens and its live entries follow) or the ones before a pos.
    AgentHistory { agent: String, before: Option<usize>, limit: usize },
    /// The panel closed: no more live entries.
    AgentUnwatch { agent: String },
    /// His words to an agent: now (steered into its turn) or queued
    /// (held by the core until the agent is free).
    AgentSend { agent: String, text: String, queued: bool },
    Follow { agent: String, on: bool },
    /// Stop an agent's turn (the hub's interrupt; the web asks first).
    Stop { agent: String },
    /// `/archive <agent>` (stop_first: `--force`, a working agent).
    Archive { agent: String, stop_first: bool },
    /// `/restore <agent>`: back in his list.
    Unarchive { agent: String },
    /// The window: the projects it shows now (replaces the set; the core
    /// decides what it holds, core/hubs.rs).
    Shown { projects: Vec<String> },
    /// his change of the artifacts library's folders (core/library.rs)
    Folders { op: bise_proto::rows::FolderOp },
    /// his move of artifacts across projects (core/library.rs)
    FoldersMove { items: Vec<bise_proto::app_cmd::FolderItem>, folder: Option<String> },
    /// The window's running screen shown or left (core/hubs.rs `processes_all`).
    ProcessesAll { on: bool, by: Option<String>, project: Option<String> },
    /// The window: a command for a project's hub (bise-proto's `HubCmd`
    /// with its `project`), routed as is.
    Typed(Value),
    /// The window's setup commands (S11, core/setup.rs).
    App(AppCmd),
    /// voice mode's commands (core/voice_mode.rs)
    VoiceMode(AppCmd),
    /// try a refused project's hub again (his action)
    HubRetry { project: String },
    /// S29: his act on an agent's computer use (core/computer.rs)
    Computer { act: String, key: String, max_width: Option<u32> },
    /// Harness A: the words the fake voice hears at the next talk (only
    /// with the fake voice on; an error otherwise).
    FakeWords { text: String },
}

impl Cmd {
    /// One stdin line; Err: the line to say back as `error` (a bad line
    /// from the app is a bug, never the user's). One parse: the line is
    /// bise-proto's `AppCmd` (or a project hub's `HubCmd`), then the
    /// rules that are not its shape (an agent's name, absolute files).
    pub fn parse(line: &str) -> Result<Cmd, String> {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("not JSON: {e}"))?;
        let cmd = v.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
        // a hub's command with its project: the window's (bise-proto)
        if v.get("project").is_some() && bise_proto::hub::HubCmd::TAGS.contains(&cmd.as_str()) {
            return Ok(Cmd::Typed(v));
        }
        if !AppCmd::TAGS.contains(&cmd.as_str()) || bise_proto::app_rpc::NOT_SERVED.contains(&cmd.as_str()) {
            return Err(format!("unknown cmd {cmd:?}"));
        }
        let typed = AppCmd::decode(line).map_err(|e| format!("{cmd}: {e}"))?;
        if super::setup::TAGS.contains(&cmd.as_str()) {
            return Ok(Cmd::App(typed));
        }
        let no_id = || Err(format!("{cmd}: no id"));
        Ok(match typed {
            AppCmd::TalkStart { page } => match page.as_ref().map(|p| p.id.trim()) {
                Some(id) if !id.is_empty() => Cmd::PageTalk { page: id.to_string() },
                _ => Cmd::TalkStart,
            },
            AppCmd::TalkEnd => Cmd::TalkEnd,
            AppCmd::TalkCancel => Cmd::TalkCancel,
            AppCmd::ShotDrop { id } => Cmd::ShotDrop { id },
            AppCmd::DictateStart { id } | AppCmd::DictateStop { id } | AppCmd::DictateCancel { id } | AppCmd::DictateRetry { id } if id.trim().is_empty() => return no_id(),
            AppCmd::DictateStart { id } => Cmd::DictateStart { id },
            AppCmd::DictateStop { id } => Cmd::DictateStop { id },
            AppCmd::DictateCancel { id } => Cmd::DictateCancel { id },
            AppCmd::DictateRetry { id } => Cmd::DictateRetry { id },
            AppCmd::VoiceMode { .. } | AppCmd::VoiceMute { .. } | AppCmd::VoiceType { .. } | AppCmd::VoiceSend | AppCmd::VoiceCut => Cmd::VoiceMode(typed),
            AppCmd::HubRetry { project } => Cmd::HubRetry { project },
            AppCmd::CardInView { project, card } => Cmd::CardInView { project, card },
            AppCmd::Commands => Cmd::Commands,
            AppCmd::Index => Cmd::Index,
            AppCmd::Skills { project } => Cmd::Skills { project },
            AppCmd::Files { project, q, rid, limit } => Cmd::Files { project, q, rid, limit },
            AppCmd::FilePicked { project, path } => Cmd::FilePicked { project, path },
            AppCmd::Computer { act, key, max_width } => Cmd::Computer { act, key, max_width },
            AppCmd::AwayBack { away_ms } => Cmd::AwayBack { away_ms },
            AppCmd::Shot { path, app, title } => Cmd::Shot { path: PathBuf::from(path), app, title },
            AppCmd::Send { text } => Cmd::Send { text },
            AppCmd::Start { text } => Cmd::Start { text },
            AppCmd::Answer { card, reply } => Cmd::Answer { card, reply },
            AppCmd::Hush => Cmd::Hush,
            AppCmd::CutIn => Cmd::CutIn,
            AppCmd::Quiet { on } => Cmd::Quiet { on },
            AppCmd::Voice { on } => Cmd::Voice { on },
            AppCmd::EveryStop { id } => Cmd::EveryStop { id },
            AppCmd::FakeWords { text } => Cmd::FakeWords { text },
            AppCmd::Shown { projects } => Cmd::Shown { projects },
            AppCmd::Folders { op } => Cmd::Folders { op },
            AppCmd::FoldersMove { items, folder } => Cmd::FoldersMove { items, folder },
            AppCmd::ProcessesAll { on, by, project } => Cmd::ProcessesAll { on, by, project },
            AppCmd::AgentPreview { agent } => Cmd::AgentPreview { agent: agent_name(&cmd, &agent)? },
            AppCmd::AgentHistory { agent, before, limit } => Cmd::AgentHistory {
                agent: agent_name(&cmd, &agent)?,
                before: before.map(|b| b as usize),
                limit: limit.map_or(60, |l| (l as usize).clamp(1, 200)),
            },
            AppCmd::AgentUnwatch { agent } => Cmd::AgentUnwatch { agent: agent_name(&cmd, &agent)? },
            AppCmd::AgentSend { agent, text, mode } => Cmd::AgentSend { agent: agent_name(&cmd, &agent)?, text, queued: mode == Some(Mode::Queued) },
            AppCmd::Follow { agent, on } => Cmd::Follow { agent: agent_name(&cmd, &agent)?, on },
            AppCmd::Stop { agent } => Cmd::Stop { agent: agent_name(&cmd, &agent)? },
            AppCmd::Archive { agent, stop_first } => Cmd::Archive { agent: agent_name(&cmd, &agent)?, stop_first },
            AppCmd::Unarchive { agent } => Cmd::Unarchive { agent: agent_name(&cmd, &agent)? },
            other => return Err(format!("{cmd}: decoded as {other:?}")),
        })
    }
}

/// An agent's name as the window wrote it (`@perf`, ` perf `); Err: none.
fn agent_name(cmd: &str, agent: &str) -> Result<String, String> {
    let a = agent.trim().trim_start_matches('@');
    match a.is_empty() {
        true => Err(format!("{cmd}: no agent")),
        false => Ok(a.to_string()),
    }
}
