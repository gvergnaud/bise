//! The app's commands (stdin, docs/ambient-app.md §4) and their parse.

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
    /// The composer's mic (bar I, core/dictate.rs): `id` is the window's,
    /// echoed on each `dictation`.
    DictateStart { id: String },
    DictateStop { id: String },
    DictateCancel { id: String },
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
    /// S9 (amb-mac m_9035): what the app read of his front app at fn
    /// (talk or spotlight), sent apart from talk_start; `{}` (an excluded
    /// app) clears it. The next turn to main carries it, then it's gone.
    FnContext(bise_proto::context::FnContext),
    /// He's back after `away_ms` (amb-mac m_9038): `away_summary` answers.
    AwayBack { away_ms: u64 },
    Send { text: String },
    /// A drop on the pearl sent from the spotlight (amb-web m_9242):
    /// `send` with `files` (absolute paths): his words and the files' paths
    /// to bise's main.
    SendFiles { text: String, files: Vec<String> },
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
    /// The window: a command for a project's hub (bise-proto's `HubCmd`
    /// with its `project`), routed as is.
    Typed(Value),
    /// The window's setup commands (S11, core/setup.rs).
    App(bise_proto::draft::AppCmd),
    /// voice mode's commands (core/voice_mode.rs)
    VoiceMode(bise_proto::draft::AppCmd),
    /// try a refused project's hub again (his action)
    HubRetry { project: String },
    /// Harness A: the words the fake voice hears at the next talk (only
    /// with the fake voice on; an error otherwise).
    FakeWords { text: String },
}

impl Cmd {
    /// One stdin line; Err: the line to say back as `error` (a bad line
    /// from the app is a bug, never the user's).
    pub fn parse(line: &str) -> Result<Cmd, String> {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("not JSON: {e}"))?;
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let cmd = s("cmd");
        // a hub's command with its project: the window's (bise-proto)
        if v.get("project").is_some() && cmd != "hello" && bise_proto::hub::HubCmd::TAGS.contains(&cmd.as_str()) {
            return Ok(Cmd::Typed(v));
        }
        if super::setup::TAGS.contains(&cmd.as_str()) {
            return bise_proto::draft::AppCmd::decode(line).map(Cmd::App).map_err(|e| format!("{cmd}: {e}"));
        }
        Ok(match cmd.as_str() {
            // bise-proto's AppCmd talk_* (qa-native m_9726): the proto's shapes are the ones parsed
            "talk_start" | "talk_end" | "talk_cancel" => match bise_proto::draft::AppCmd::decode(line).map_err(|e| format!("{cmd}: {e}"))? {
                bise_proto::draft::AppCmd::TalkStart { page } => match page.as_ref().map(|p| p.id.trim()) {
                    Some(id) if !id.is_empty() => Cmd::PageTalk { page: id.to_string() },
                    _ => Cmd::TalkStart,
                },
                bise_proto::draft::AppCmd::TalkEnd => Cmd::TalkEnd,
                bise_proto::draft::AppCmd::TalkCancel => Cmd::TalkCancel,
                other => return Err(format!("{cmd}: decoded as {other:?}")),
            },
            "dictate_start" | "dictate_stop" | "dictate_cancel" => match bise_proto::draft::AppCmd::decode(line).map_err(|e| format!("{cmd}: {e}"))? {
                bise_proto::draft::AppCmd::DictateStart { id } if !id.trim().is_empty() => Cmd::DictateStart { id },
                bise_proto::draft::AppCmd::DictateStop { id } if !id.trim().is_empty() => Cmd::DictateStop { id },
                bise_proto::draft::AppCmd::DictateCancel { id } if !id.trim().is_empty() => Cmd::DictateCancel { id },
                _ => return Err(format!("{cmd}: no id")),
            },
            "voice_mode" | "voice_mute" | "voice_type" | "voice_send" | "voice_cut" => {
                Cmd::VoiceMode(bise_proto::draft::AppCmd::decode(line).map_err(|e| format!("{cmd}: {e}"))?)
            }
            "hub_retry" => match bise_proto::draft::AppCmd::decode(line).map_err(|e| format!("{cmd}: {e}"))? {
                bise_proto::draft::AppCmd::HubRetry { project } => Cmd::HubRetry { project },
                other => return Err(format!("{cmd}: decoded as {other:?}")),
            },
            "card_in_view" => match bise_proto::draft::AppCmd::decode(line).map_err(|e| format!("{cmd}: {e}"))? {
                bise_proto::draft::AppCmd::CardInView { project, card } => Cmd::CardInView { project, card },
                other => return Err(format!("{cmd}: decoded as {other:?}")),
            },
            "commands" | "index" | "skills" | "files" | "file_picked" => match bise_proto::draft::AppCmd::decode(line).map_err(|e| format!("{cmd}: {e}"))? {
                bise_proto::draft::AppCmd::Commands => Cmd::Commands,
                bise_proto::draft::AppCmd::Index => Cmd::Index,
                bise_proto::draft::AppCmd::Skills { project } => Cmd::Skills { project },
                bise_proto::draft::AppCmd::Files { project, q, rid, limit } => Cmd::Files { project, q, rid, limit },
                bise_proto::draft::AppCmd::FilePicked { project, path } => Cmd::FilePicked { project, path },
                other => return Err(format!("{cmd}: decoded as {other:?}")),
            },
            "away_back" => Cmd::AwayBack { away_ms: v.get("away_ms").and_then(Value::as_u64).ok_or("away_back: no away_ms")? },
            "fn_context" => match v.get("context") {
                Some(c) if c.is_object() => Cmd::FnContext(serde_json::from_value(c.clone()).map_err(|e| format!("fn_context: {e}"))?),
                _ => return Err("fn_context: no context object".into()),
            },
            "shot" => Cmd::Shot { path: PathBuf::from(s("path")), app: s("app"), title: s("title") },
            "send" => {
                let files: Vec<String> = v.get("files").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::trim).filter(|f| !f.is_empty()).map(str::to_string).collect();
                if let Some(f) = files.iter().find(|f| !f.starts_with('/')) {
                    return Err(format!("send: a file must be an absolute path: {f:?}"));
                }
                match files.is_empty() {
                    true => Cmd::Send { text: s("text") },
                    false => Cmd::SendFiles { text: s("text"), files },
                }
            }
            "start" => Cmd::Start { text: s("text") },
            "answer" => {
                let card = match v.get("card") {
                    Some(Value::Number(n)) => n.as_u64(),
                    Some(Value::String(t)) => t.trim().trim_start_matches('#').parse().ok(),
                    _ => None,
                };
                Cmd::Answer { card: card.ok_or("answer: no card id")?, reply: s("reply") }
            }
            "hush" => Cmd::Hush,
            "cut_in" => Cmd::CutIn,
            "quiet" => Cmd::Quiet { on: v.get("on").and_then(Value::as_bool).unwrap_or(false) },
            "voice" => Cmd::Voice { on: v.get("on").and_then(Value::as_bool).unwrap_or(true) },
            "every_stop" => {
                let id = match v.get("id") {
                    Some(Value::Number(n)) => n.as_u64(),
                    Some(Value::String(t)) => t.trim().parse().ok(),
                    _ => None,
                };
                Cmd::EveryStop { id: id.ok_or("every_stop: no id")? }
            }
            "fake_words" => Cmd::FakeWords { text: s("text") },
            // bise-proto's AppCmd shown: its shape is the one parsed
            "shown" => match bise_proto::draft::AppCmd::decode(line).map_err(|e| format!("shown: {e}"))? {
                bise_proto::draft::AppCmd::Shown { projects } => Cmd::Shown { projects },
                other => return Err(format!("shown: decoded as {other:?}")),
            },
            c @ ("agent_preview" | "agent_history" | "agent_unwatch" | "agent_send" | "follow" | "stop" | "archive" | "unarchive") => {
                let agent = s("agent").trim().trim_start_matches('@').to_string();
                if agent.is_empty() {
                    return Err(format!("{c}: no agent"));
                }
                let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
                match c {
                    "agent_preview" => Cmd::AgentPreview { agent },
                    "agent_history" => Cmd::AgentHistory {
                        agent,
                        before: v.get("before").and_then(Value::as_u64).map(|b| b as usize),
                        limit: v.get("limit").and_then(Value::as_u64).map_or(60, |l| (l as usize).clamp(1, 200)),
                    },
                    "agent_unwatch" => Cmd::AgentUnwatch { agent },
                    "agent_send" => Cmd::AgentSend { agent, text: s("text"), queued: s("mode") == "queued" },
                    "follow" => Cmd::Follow { agent, on: v.get("on").and_then(Value::as_bool).unwrap_or(true) },
                    "stop" => Cmd::Stop { agent },
                    "archive" => Cmd::Archive { agent, stop_first: flag("stop_first") },
                    _ => Cmd::Unarchive { agent },
                }
            }
            other => return Err(format!("unknown cmd {other:?}")),
        })
    }
}
