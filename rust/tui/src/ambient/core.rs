//! The core's state machine (docs/ambient-app.md §3-§4): the app's
//! commands and the hub's lines in, compact UI events out. Driven by the
//! run loop ([`super::run`]) or a test: [`Core::cmd`], [`Core::hub`],
//! [`Core::tick`] (~30 Hz), then [`Core::take_out`]. Its ports (mic,
//! listener, synthesizer, speaker, the voice jobs) are fakes in tests.

mod agents_view;
mod app_update;
mod away;
mod cmd;
mod dictate;
mod hubs;
mod index;
mod picks;
mod voice_answer;
mod setup;
mod voice;
mod voice_mode;

pub use self::cmd::Cmd;
pub use self::hubs::{branch_of_head, ProjectFacts, ProjectPorts, Spawn};
pub use self::voice_mode::OpenVoiceMode;
#[cfg(test)]
pub use self::hubs::{HUB_OLDER, LINGER, START};
#[cfg(test)]
pub use self::dictate::DICTATE_MAX;
use super::hub::{Hub, HubIn};
use super::speech::{Speech, Step};
use crate::voicemode::{Heard, ListenJob, ListenMsg, Listener, Mic, MicBlock, MicStream, SayJob, Speaker, Synthesizer, UttId};
use crate::wire::{parse_line, Ev};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// After fn up, the listener's last words wait at most this long; then
/// what was heard goes.
pub const FINAL_WAIT: Duration = Duration::from_millis(1500);

/// The hub's marker on what reaches main from a page (switchboard's
/// daemon PAGE_HINT): that turn's words stay on the page.
const PAGE_MARK: &str = "[from the page:";

/// A note talk's words that send the page's notes (§4.1): the word
/// alone, any case, its punctuation dropped.
fn send_word(words: &str) -> bool {
    let w = words.trim().trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    matches!(w.as_str(), "send" | "send it" | "envoie" | "envoyer" | "envoie-le" | "envoie le")
}

/// The composer label of the shot's marker (the terminal shows
/// `[Screen <app> · <title>]` in main's thread).
const SHOT_NAME: &str = "[Screen]";

/// The listener for the voice role's job (`listen::listener_for`).
pub type ListenerFor = Box<dyn Fn(&ListenJob) -> Box<dyn Listener>>;
/// The output device, opened at the first spoken answer.
pub type OpenSpeaker = Box<dyn FnMut() -> Result<Box<dyn Speaker>, String>>;
/// A file into the image store (`bend_images::store_file`).
pub type StoreImage = Box<dyn Fn(&Path) -> Result<bend_images::Stored, String>>;

/// Everything the core reaches outside: real in `bise ambient-core`,
/// fakes in tests. The mic opens only between talk_start and talk_end;
/// the speaker opens at main's first spoken answer.
pub struct Ports {
    pub mic: Box<dyn Mic>,
    pub listener: ListenerFor,
    pub synth: Box<dyn Synthesizer>,
    pub open_speaker: OpenSpeaker,
    /// the voice role's listener job and the TTS job, read at each use
    /// (a key added while the app runs counts)
    pub listen_job: Box<dyn Fn() -> Result<ListenJob, String>>,
    pub say_job: Box<dyn Fn() -> Result<SayJob, String>>,
    /// the configured voice language (`[voice] language`), if any
    pub language: Option<String>,
    /// a file into the image store (`bend_images::store_file`)
    pub store_image: StoreImage,
    /// the hub's `switchboard::model::user_kind` (the user's inbox),
    /// handed in by the binary: bend-tui does not link switchboard
    pub user_kind: fn(&str) -> bool,
    /// harness A's fake voice (fake.rs): the next talk's words, set by
    /// the `fake_words` cmd; None in his normal run (the cmd is refused)
    pub fake_words: Option<super::fake::Words>,
    /// voice mode's controller ports (core/voice_mode.rs)
    pub voice_mode: OpenVoiceMode,
}

/// One phase of main's turn, for the orb (§4 `phase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Listening,
    Sending,
    Working,
    Speaking,
    Done,
    Failed,
}

impl Phase {
    fn word(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Listening => "listening",
            Phase::Sending => "sending",
            Phase::Working => "working",
            Phase::Speaking => "speaking",
            Phase::Done => "done",
            Phase::Failed => "failed",
        }
    }
}

/// fn held: the mic, the listener, the words so far.
struct Talk {
    /// None once fn is up (the mic is off)
    stream: Option<Box<dyn MicStream>>,
    blocks: Receiver<MicBlock>,
    audio: Sender<ListenMsg>,
    heard: Receiver<Heard>,
    cancel: Arc<AtomicBool>,
    text: String,
    /// fn up at
    ending: Option<Instant>,
    /// a note talk: the page its words go to (never main)
    page: Option<String>,
}

impl Drop for Talk {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// A front-window shot in the image store.
struct Shot {
    stored: bend_images::Stored,
    app: String,
    title: String,
    /// main's turn has it (started after it was sent, or steered into it)
    reached: bool,
}

impl Shot {
    fn marker(&self) -> String {
        let source = match (self.app.trim(), self.title.trim()) {
            ("", t) => t.to_string(),
            (a, "") => a.to_string(),
            (a, t) => format!("{a} · {t}"),
        };
        bend_images::marker(SHOT_NAME, &source, &self.stored)
    }

    fn files(&self) -> [&Path; 2] {
        [&self.stored.file, &self.stored.b64]
    }
}

/// A page card's numbered options, one or more (`sign in\n1. done`): the
/// TUI's split wants two or more, and a step's card has one.
fn page_options(text: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for l in text.trim_end().lines().rev() {
        let l = l.trim();
        let digits: String = l.chars().take_while(char::is_ascii_digit).collect();
        let Some(label) = l[digits.len()..].strip_prefix(". ").map(str::trim).filter(|x| !x.is_empty()) else { break };
        let Ok(n) = digits.parse::<usize>() else { break };
        out.push((n, label.to_string()));
    }
    out.reverse();
    let numbered = out.iter().enumerate().all(|(i, (n, _))| *n == i + 1);
    if numbered && out.len() <= 9 { out } else { Vec::new() }
}

/// A card of the user's inbox, as the hub's snapshot has it.
#[derive(Clone, Debug)]
struct CardInfo {
    id: u64,
    kind: String,
    agent: String,
    text: String,
    /// a page question's card: {id, block, url} (docs/ambient-pages.md §4.2)
    page: Option<Value>,
    /// a batch of drafts' card: {count, title, what, names} (amb-web
    /// m_6061: the capsule's words)
    batch: Option<Value>,
}

/// Milliseconds since the epoch.
fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub struct Core {
    workspace: String,
    hub: Hub,
    ports: Ports,
    out: Vec<Value>,
    /// the last `hub` said (None: nothing said yet)
    hub_up: Option<bool>,
    /// the hello replay is over (`ready`): main's lines are live
    ready: bool,
    cards: Vec<CardInfo>,
    phase: Phase,
    /// main's turns seen (the `turn` of `main` and `word`)
    turn: u64,
    in_turn: bool,
    /// the turn running answers a user message (main's words are sent)
    for_user: bool,
    /// a user message main has not taken into a turn yet
    user_waiting: bool,
    /// every agent of the last state (main included)
    agent_names: Vec<String>,
    /// the agents there were when the user's last turn started: a page
    /// from main in that turn, or from an agent born since, is one the
    /// user waits for (docs/ambient-pages.md §2.8)
    user_turn_agents: Option<Vec<String>>,
    /// main's last message of this turn, as sent in `main`
    main_text: String,
    /// the last message sent was said: Some(reached main's turn yet)
    voice_out: Option<bool>,
    talk: Option<Talk>,
    /// the composer's mic (bar I, core/dictate.rs)
    dictation: Option<dictate::Dictation>,
    /// voice mode with the agent in view (core/voice_mode.rs)
    voice_on: Option<voice_mode::VoiceOn>,
    /// the composer's `@` answers on their way (core/picks.rs)
    picks: picks::Picks,
    /// ⌘K's index of the projects not held (core/index.rs)
    index: index::Index,
    /// bar N14: the card in front of him (core/voice_answer.rs)
    in_view: Option<(String, u64)>,
    speaker: Option<Box<dyn Speaker>>,
    speech: Option<Speech>,
    next_utt: UttId,
    /// the shot for the next message
    shot: Option<Shot>,
    /// the fn context for the next message (S9): its `shot` filled from
    /// the shot at send
    fn_ctx: Option<bise_proto::context::FnContext>,
    /// shots sent, forgotten (deleted) at the end of main's turn
    sent_shots: Vec<Shot>,
    /// the pages of the last state: (id, title)
    page_titles: Vec<(String, String)>,
    /// batch cards seen (page.drafts): an answer on one after it closed
    /// still goes to the hub, which acts on the page's current drafts or
    /// says what changed (pm's C fail 43)
    batches_seen: Vec<u64>,
    /// each agent's row status and since when (round 10's rows: the hub
    /// does not say when a status changed; the core sees it change)
    since: std::collections::HashMap<String, (String, u64)>,
    /// the agents he follows (round 10: follow {agent, on})
    followed: Vec<String>,
    /// the hub's agents as last seen (round 10's preview: status,
    /// objective, report)
    hub_agents: Vec<Value>,
    /// the hub's pages as last seen
    hub_pages: Vec<Value>,
    /// every project's typed connection, for the window (core/hubs.rs)
    hubs: hubs::Hubs,
    /// the window's setup commands (S11, core/setup.rs)
    setup: Option<setup::Setup>,
    /// the agents shown (round 10): a history panel open, a preview
    feeds: Vec<agents_view::Feed>,
    /// the typed `subscribe`/`page` sent, answered in order
    pending: std::collections::VecDeque<agents_view::Pending>,
    /// the voice target's project id (its hub's `welcome`): the hub the
    /// capsule talks to and the agents view reads, `--workspace`'s until
    /// S2 makes it bise's home hub (one value, architect m_8433)
    target: Option<String>,
    /// the hub's last state, for a row change of the core's (follow)
    last_state: Option<Value>,
    /// each agent's current step (its last tool intent) for `agent_typing`
    typing: std::collections::HashMap<String, String>,
    /// the page server's base URL, from the hub's state
    pages_url: Option<String>,
    /// on a call or in a meeting (the app's `quiet`): no voice
    quiet: bool,
    /// the settings' voice answers are off: no voice
    voice_off: bool,
}

impl Core {
    pub fn new(workspace: String, hub: Hub, ports: Ports) -> Core {
        Core {
            workspace,
            hub,
            ports,
            out: Vec::new(),
            hub_up: None,
            ready: false,
            cards: Vec::new(),
            phase: Phase::Idle,
            turn: 0,
            in_turn: false,
            for_user: false,
            user_waiting: false,
            agent_names: Vec::new(),
            user_turn_agents: None,
            main_text: String::new(),
            voice_out: None,
            talk: None,
            dictation: None,
            voice_on: None,
            picks: picks::Picks::default(),
            index: index::Index::default(),
            in_view: None,
            speaker: None,
            speech: None,
            next_utt: 1,
            shot: None,
            fn_ctx: None,
            sent_shots: Vec::new(),
            page_titles: Vec::new(),
            batches_seen: Vec::new(),
            since: std::collections::HashMap::new(),
            followed: Vec::new(),
            hub_agents: Vec::new(),
            hub_pages: Vec::new(),
            feeds: Vec::new(),
            hubs: Default::default(),
            setup: None,
            pending: std::collections::VecDeque::new(),
            target: None,
            last_state: None,
            typing: std::collections::HashMap::new(),
            pages_url: None,
            quiet: false,
            voice_off: false,
        }
    }

    /// The events since the last call, in order.
    pub fn take_out(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.out)
    }

    fn emit(&mut self, v: Value) {
        self.out.push(v);
    }

    fn error(&mut self, text: &str) {
        self.emit(json!({"ev": "error", "text": text}));
    }

    fn set_phase(&mut self, p: Phase, line: Option<String>) {
        self.phase = p;
        let mut v = json!({"ev": "phase", "phase": p.word()});
        if let Some(l) = line {
            v["line"] = json!(l);
        }
        self.emit(v);
    }

    // ---- the app's commands ----

    pub fn cmd(&mut self, c: Cmd, now: Instant) {
        match c {
            Cmd::TalkStart => self.talk_start(None),
            Cmd::PageTalk { page } => self.talk_start(Some(page)),
            Cmd::TalkEnd => self.talk_end(now),
            Cmd::TalkCancel => self.talk_cancel(),
            Cmd::DictateStart { id } => self.dictate_start(id, now),
            Cmd::DictateStop { id } => self.dictate_stop(&id, now),
            Cmd::DictateCancel { id } => self.dictate_cancel(&id),
            Cmd::VoiceMode(c) => self.voice_mode_cmd(c, now),
            Cmd::HubRetry { project } => self.hub_retry(&project),
            Cmd::CardInView { project, card } => self.in_view = project.zip(card),
            Cmd::Commands => self.pick_commands(),
            Cmd::Index => self.index_cmd(now),
            Cmd::Skills { project } => self.pick_skills(project),
            Cmd::Files { project, q, rid, limit } => self.pick_files(project, q, rid, limit),
            Cmd::FilePicked { project, path } => self.file_picked(project, path),
            Cmd::Shot { path, app, title } => self.take_shot(&path, app, title),
            // the latest fn wins; an excluded app's `{}` clears
            Cmd::AwayBack { away_ms } => self.away_back(away_ms),
            Cmd::FnContext(c) => self.fn_ctx = Some(c).filter(|c| *c != bise_proto::context::FnContext::default()),
            Cmd::Send { text } => {
                let text = text.trim().to_string();
                if !text.is_empty() || self.shot.is_some() {
                    self.send_to_main(text, false);
                }
            }
            // his dropped files go as the input's files field: the hub
            // renders them once after his words (switchboard attached.rs,
            // item H: an image as a marker the model sees, others listed)
            Cmd::SendFiles { text, files } => self.send_input(text.trim().to_string(), false, "capsule", files),
            Cmd::Start { text } => {
                let text = text.trim().to_string();
                if !text.is_empty() {
                    self.send_via(format!("start an agent for: {text}"), false, "capsule-start");
                }
            }
            Cmd::Answer { card, reply } => self.answer(card, reply.trim()),
            Cmd::Hush => {
                if self.stop_voice() {
                    self.set_phase(Phase::Done, None);
                }
            }
            Cmd::CutIn => {
                if self.stop_voice() {
                    self.set_phase(Phase::Idle, None);
                }
            }
            Cmd::Quiet { on } => {
                self.quiet = on;
                // a call starts while main speaks: the voice stops, the
                // text stays
                if on && self.stop_voice() {
                    self.set_phase(Phase::Done, None);
                }
            }
            Cmd::EveryStop { id } => {
                if !self.hub.send(&json!({"op": "every_stop", "id": id})) {
                    self.error("bise isn't reachable: it keeps watching. try again in a moment.");
                }
            }
            Cmd::Voice { on } => {
                self.voice_off = !on;
                if !on && self.stop_voice() {
                    self.set_phase(Phase::Done, None);
                }
            }
            Cmd::AgentPreview { agent } => self.agent_preview(agent),
            Cmd::AgentHistory { agent, before, limit } => self.agent_history(agent, before, limit),
            Cmd::AgentUnwatch { agent } => self.agent_unwatch(&agent),
            Cmd::AgentSend { agent, text, queued } => self.agent_send(agent, text, queued),
            Cmd::Follow { agent, on } => {
                self.followed.retain(|a| *a != agent);
                if on {
                    self.followed.push(agent);
                }
                if let Some(st) = self.last_state.clone() {
                    self.state(&st);
                }
            }
            Cmd::Stop { agent } => {
                if !self.hub.send(&json!({"op": "interrupt", "agent": agent})) {
                    self.error("bise isn't reachable: it keeps going. try again in a moment.");
                }
            }
            Cmd::Archive { agent, stop_first } => {
                let force = if stop_first { " --force" } else { "" };
                self.slash(&format!("/archive {agent}{force}"));
            }
            Cmd::Unarchive { agent } => self.slash(&format!("/restore {agent}")),
            Cmd::Shown { projects } => {
                self.shown(projects);
                self.app_start();
            }
            Cmd::Typed(v) => self.typed_cmd(v),
            Cmd::App(c) => self.app_cmd(c),
            Cmd::FakeWords { text } => match &self.ports.fake_words {
                Some(w) => w.set(&text),
                None => self.error("fake_words: the fake voice is off (harness A only)."),
            },
        }
    }

    /// A slash command of his, through main's input (the TUI's way).
    fn slash(&mut self, text: &str) {
        if !self.hub.send(&json!({"op": "input", "focus": "main", "text": text})) {
            self.error("bise isn't reachable: nothing changed. try again in a moment.");
        }
    }

    /// The message to main's thread: the words, then the pending shot's
    /// marker.
    fn send_to_main(&mut self, words: String, voice: bool) {
        self.send_via(words, voice, "capsule");
    }

    /// [`send_to_main`] with its `via`: "capsule" (one short line back),
    /// "capsule-start" (start an agent; the hub's hint says so).
    fn send_via(&mut self, words: String, voice: bool, via: &str) {
        self.send_input(words, voice, via, Vec::new());
    }

    /// [`send_via`] with his files (absolute paths) as the input's `files`.
    fn send_input(&mut self, words: String, voice: bool, via: &str, files: Vec<String>) {
        let shot = self.shot.take();
        let text = match &shot {
            Some(s) if words.is_empty() => s.marker(),
            Some(s) => format!("{words}\n\n{}", s.marker()),
            None => words.clone(),
        };
        // S9: the fn context goes as a field (the hub frames it after his
        // words, its one render); its shot is the stored file, a path in
        // the image store (the window's thumbnail), never the app's PNG
        let ctx = self.fn_ctx.take().map(|mut c| {
            c.shot = shot.as_ref().map(|s| s.stored.file.display().to_string());
            c
        });
        // via: the hub tells main it came from the capsule (one short line back)
        let mut input = json!({"op": "input", "focus": "main", "text": text, "via": via});
        if let Some(c) = ctx {
            input["context"] = json!(c);
        }
        if !files.is_empty() {
            input["files"] = json!(files);
        }
        if !self.hub.send(&input) {
            if let Some(s) = shot {
                self.forget(vec![s]);
            }
            self.error("main didn't get it: bise isn't reachable. try again in a moment.");
            return self.set_phase(Phase::Failed, None);
        }
        let had_shot = shot.is_some();
        self.sent_shots.extend(shot);
        self.voice_out = voice.then_some(false);
        // the hub echoes it as `sb you` too; this one counts if it does not
        self.user_waiting = true;
        self.emit(json!({"ev": "sent", "text": words, "voice": voice, "shot": had_shot}));
        self.set_phase(Phase::Sending, None);
    }

    fn answer(&mut self, id: u64, reply: &str) {
        let card = self.cards.iter().find(|c| c.id == id).cloned();
        let digit = reply.parse::<usize>().ok().filter(|d| (1..=9).contains(d));
        let reply = match (card, digit) {
            // a page's card (a question, a step: "1. done"): the digit
            // goes as is, the hub maps it on the page's path. The TUI's
            // option split wants 2+ options, so a step's one option got
            // "opens in the terminal" and its card never closed (pm's D
            // fail 31)
            (Some(c), Some(d)) if c.page.is_some() => d.to_string(),
            (Some(c), Some(d)) => match crate::sb::ambient_reply(&c.kind, &c.agent, &c.text, d) {
                Some(r) => r,
                // a PR or release page to open, the feature drop's second
                // ask: the box acts on those itself
                None if crate::sb::ambient_local(&c.kind, d) => return self.error("that option opens in the terminal."),
                // options this split can't read: the digit as is, the
                // hub's question path resolves it; never a refusal (pm's
                // C fail 41)
                None => d.to_string(),
            },
            // a batch card replaced or closed: the hub acts on the page's
            // current drafts, or says in one line what the card is now
            (None, Some(d)) if self.batches_seen.contains(&id) => d.to_string(),
            (None, None) if self.batches_seen.contains(&id) && !reply.trim().is_empty() => reply.to_string(),
            (None, _) => return self.error("that question is already closed."),
            (Some(_), None) if reply.is_empty() => return,
            (Some(_), None) => reply.to_string(),
        };
        if !self.hub.send(&json!({"op": "input", "focus": "main", "text": format!("/answer {id} {reply}")})) {
            self.error("main didn't get it: bise isn't reachable. try again in a moment.");
        }
    }

    /// Hush, cut in: main's voice stops (the text stays). False: it was
    /// not speaking.
    fn stop_voice(&mut self) -> bool {
        let Some(mut sp) = self.speech.take() else { return false };
        if let Some(spk) = self.speaker.as_mut() {
            sp.stop(spk.as_mut());
        }
        self.emit(json!({"ev": "level", "who": "main", "v": 0.0}));
        true
    }

    /// Delete the shots' stored files (SPEC §5.1: not kept after main's
    /// turn), but a file another live shot still uses (two shots of an
    /// unchanged window are the same bytes, the same store file).
    fn forget(&mut self, shots: Vec<Shot>) {
        let live: Vec<PathBuf> = self
            .shot
            .iter()
            .chain(&self.sent_shots)
            .flat_map(|s| s.files().map(Path::to_path_buf))
            .collect();
        for s in shots {
            for f in s.files() {
                if !live.iter().any(|l| l == f) {
                    let _ = std::fs::remove_file(f);
                }
            }
        }
    }

    /// Stdin closed: the app is gone. Nothing of the shots stays.
    pub fn shutdown(&mut self) {
        self.voice_mode_off(Instant::now(), Some("bise stopped"));
        self.close_projects();
        self.stop_voice();
        self.talk = None;
        let mut all: Vec<Shot> = self.sent_shots.drain(..).collect();
        all.extend(self.shot.take());
        self.forget(all);
    }

    // ---- the hub ----

    pub fn hub(&mut self, h: HubIn) {
        match h {
            HubIn::Up => {
                self.ready = false;
                // the typed protocol on the same connection (S3b): its
                // `welcome` names the target; the feed lines stay the
                // capsule's until S8
                self.target = None;
                self.pending.clear();
                self.hub.send(&json!({"cmd": "hello", "proto": bise_proto::PROTO}));
                self.hub_up = Some(true);
                let ws = self.workspace.clone();
                self.emit(json!({"ev": "hub", "up": true, "workspace": ws}));
            }
            HubIn::Down => {
                if self.hub_up != Some(false) {
                    self.hub_up = Some(false);
                    let ws = self.workspace.clone();
                    self.emit(json!({"ev": "hub", "up": false, "workspace": ws}));
                }
            }
            HubIn::Line(l) => self.hub_line(&l),
            // the hub refused this core (docs/issues/16): never again until
            // a new core starts; the window says why
            HubIn::Refused(why) => {
                self.hub_up = Some(false);
                let project = bise_home::hub_id(Path::new(&self.workspace));
                self.emit(json!({"ev": "hub_refused", "project": project, "error": why}));
            }
        }
    }

    fn hub_line(&mut self, raw: &str) {
        let Ok(v) = serde_json::from_str::<Value>(raw) else { return };
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let ev = s("ev");
        if bise_proto::hub::HubEv::TAGS.contains(&ev.as_str()) {
            // J: a job's end once, from whichever hub said it first
            if !self.end_once(&v) {
                return;
            }
            return self.typed_ev(v);
        }
        match ev.as_str() {
            "state" => self.state(&v),
            "ready" => {
                self.ready = true;
                // the replay told where main's turn is (working only on
                // a turn that answers the user)
                let p = if self.in_turn && self.for_user { Phase::Working } else { Phase::Idle };
                if self.talk.is_none() && self.speech.is_none() {
                    self.set_phase(p, None);
                }
            }
            "page" if self.ready => self.page(v),
            "line" => {
                let line = s("line");
                if self.ready && line.starts_with("sb msg-you : ") {
                    self.msg_you(&line);
                } else if s("agent") == "main" {
                    self.main_line(&line);
                }
            }
            _ => {}
        }
    }

    /// The hub's `page` event, relayed with `front`: the user is waiting
    /// for it (main's page in the user's turn, or a page of an agent born
    /// since the user's last turn started), so the app opens it without
    /// fn + o (docs/ambient-pages.md §2.8).
    fn page(&mut self, mut v: Value) {
        let agent = v.get("agent").and_then(Value::as_str).unwrap_or("").to_string();
        let ready = v.get("state").and_then(Value::as_str) == Some("ready");
        let waited = match &self.user_turn_agents {
            None => false,
            Some(_) if agent == "main" => self.for_user && self.in_turn,
            Some(known) => !known.contains(&agent),
        };
        v["front"] = json!(ready && waited);
        self.emit(v);
    }

    fn state(&mut self, v: &Value) {
        let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let list = |k: &str| v.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
        self.last_state = Some(v.clone());
        self.agent_names = list("agents").iter().map(|a| s(a, "name")).collect();
        self.hub_agents = list("agents");
        self.hub_pages = list("pages");
        let now = now_ms();
        let user_kind = self.ports.user_kind;
        let waits = |name: &str| {
            list("cards")
                .iter()
                .filter(|c| s(c, "agent") == name && user_kind(&s(c, "kind")) && s(c, "kind") != "drop")
                .count()
        };
        let mut rows = Vec::new();
        for a in list("agents").iter().filter(|a| !a.get("main").and_then(Value::as_bool).unwrap_or(false)) {
            let (name, hub_status) = (s(a, "name"), s(a, "status"));
            let (status, archived) = super::agents::status(&hub_status);
            // since: when the core saw the status change; at first sight
            // its last report's time, else its birth, else now
            let first = || {
                let ms = |k: &str| a.get(k).and_then(Value::as_u64).filter(|m| *m > 0);
                ms("report_ms").filter(|_| status != "working").or(ms("created_ms")).unwrap_or(now)
            };
            let since = match self.since.get(&name) {
                Some((st, ms)) if st == status => *ms,
                Some(_) => now,
                None => first(),
            };
            self.since.insert(name.clone(), (status.to_string(), since));
            rows.push(json!({
                "name": name,
                "status": status,
                "title": super::agents::title(a),
                "purpose": super::agents::one_line(&s(a, "objective"), 200),
                "since_ms": since,
                "waits": waits(&name),
                "followed": self.followed.contains(&name),
                "archived": archived,
                "note": s(a, "note"),
                "report": s(a, "report"),
                "working": !archived && matches!(hub_status.as_str(), "working" | "waiting" | "starting"),
                "turn_ms": a.get("turn_ms").and_then(Value::as_u64),
            }));
        }
        let agents = rows;
        self.cards = list("cards")
            .iter()
            // bise's own bookkeeping (main's archive suggestion, a
            // `drop` card) stays in the TUI: never on the glass, in the
            // count or in needs-you (pm's C fail 36)
            .filter(|c| (self.ports.user_kind)(&s(c, "kind")) && s(c, "kind") != "drop")
            .map(|c| CardInfo {
                id: c.get("id").and_then(Value::as_u64).unwrap_or(0),
                kind: s(c, "kind"),
                agent: s(c, "agent"),
                text: s(c, "text"),
                page: c.get("page").filter(|p| p.is_object()).cloned(),
                batch: c.get("batch").filter(|b| b.is_object()).cloned(),
            })
            .collect();
        for c in self.cards.iter().filter(|c| c.page.as_ref().is_some_and(|p| p["drafts"] == true)) {
            if !self.batches_seen.contains(&c.id) {
                self.batches_seen.push(c.id);
            }
        }
        let cards: Vec<Value> = self
            .cards
            .iter()
            .map(|c| {
                let mut options = crate::sb::ambient_options(&c.kind, &c.agent, &c.text);
                if options.is_empty() && c.page.is_some() {
                    options = page_options(&c.text);
                }
                // the body does not repeat the options (ambient m_6476):
                // a numbered list at the end of the text that is the
                // card's options goes, the question stays
                let (body, listed) = crate::sb::split_choices(&c.text);
                let text = if !listed.is_empty() && listed.iter().eq(options.iter().map(|(_, l)| l)) { body } else { c.text.clone() };
                let options: Vec<Value> = options.into_iter().map(|(n, label)| json!({"n": n, "label": label})).collect();
                // urgent: it asks even on a call or in a meeting. A
                // tool call waiting for a yes holds an agent's work now
                // (best guess, told to ambient-lead)
                let urgent = c.kind == "confirm";
                let mut v = json!({"id": c.id, "kind": c.kind, "agent": c.agent, "text": text, "options": options, "urgent": urgent});
                // main's own question (no page): its label (ambient m_6476)
                if c.agent == "main" && c.kind == "question" && c.page.is_none() {
                    v["label"] = json!("? main needs you");
                }
                if let Some(p) = &c.page {
                    v["page"] = p.clone();
                }
                if let Some(b) = &c.batch {
                    v["batch"] = b.clone();
                }
                v
            })
            .collect();
        let pages = list("pages");
        self.page_titles = pages.iter().map(|p| (s(p, "id"), s(p, "title"))).collect();
        if let Some(u) = v.get("pages_url").and_then(Value::as_str).filter(|u| !u.is_empty()) {
            self.pages_url = Some(u.to_string());
        }
        let mut st = json!({"ev": "state", "agents": agents, "cards": cards, "pages": pages});
        if let Some(u) = &self.pages_url {
            st["pages_url"] = json!(u);
        }
        // the standing orders (roadmap B), as the hub has them
        if let Some(t) = v.get("timers").filter(|t| !t.is_null()) {
            st["timers"] = t.clone();
        }
        self.emit(st);
        // round 10: what is shown of an agent follows its status, its
        // cards, its pages
        self.refresh_shown();
    }

    /// One line of main's feed (ambient's review, m_4672: only main's
    /// words FOR THE USER reach the capsule). A turn answers the user when
    /// a user message reached it: `sb you` (the TUI's, or this core's
    /// echoed) before it started, or steered into it. Main's other turns
    /// (an agent's report, a message between agents) send nothing: no
    /// `main`, no phase. Before `ready` (the hello replay) the lines only
    /// say where main is; nothing of them is sent.
    fn main_line(&mut self, line: &str) {
        let obs = line.strip_prefix("  obs: ");
        if let Some(t) = line.strip_prefix("sb you : ") {
            // a slash command (`/answer`) is the hub's, never a turn; what
            // comes from a page (notes, starts, picks, ticks: the hub's
            // marker) is answered on the page, never in the capsule
            // (ambient-lead m_5724, pm's 27)
            if !t.trim_start().starts_with('/') && !t.contains(PAGE_MARK) {
                self.user_waiting = true;
            }
            return;
        }
        if obs == Some("turn_started") {
            self.in_turn = true;
            self.turn += 1;
            self.main_text.clear();
            self.for_user = std::mem::take(&mut self.user_waiting);
            if self.for_user {
                self.user_turn_agents = Some(self.agent_names.clone());
            }
            if self.ready {
                self.reached();
                if self.for_user && self.speech.is_none() {
                    self.set_phase(Phase::Working, None);
                }
            }
            return;
        }
        if let Some(how) = obs.and_then(|o| o.strip_prefix("turn_done: ")) {
            self.in_turn = false;
            let for_user = std::mem::take(&mut self.for_user);
            if self.ready {
                self.turn_done(how, line, for_user);
            }
            return;
        }
        if obs.is_some_and(|o| o.starts_with("steering_received: ") || o.starts_with("steered: ")) {
            // the user's message went into the running turn
            if self.in_turn && self.user_waiting {
                self.user_waiting = false;
                if !self.for_user {
                    self.for_user = true;
                    self.user_turn_agents = Some(self.agent_names.clone());
                    if self.ready && self.speech.is_none() {
                        self.set_phase(Phase::Working, None);
                    }
                }
            }
            if self.ready {
                self.reached();
            }
            return;
        }
        if !self.ready || !self.for_user {
            return;
        }
        match parse_line(line) {
            Some(Ev::Assistant(t)) => {
                // kept until the turn ends: words followed by a tool call
                // are main planning, not its answer (pm's 22)
                let vis = crate::wire::split_thinking(&t).map(|(_, v)| v).unwrap_or(t);
                let text = super::plain::plain(&crate::markdown::unescape_md(&vis));
                if !text.is_empty() {
                    self.main_text = text;
                }
            }
            Some(Ev::ToolIntent { text, .. }) => {
                self.main_text.clear();
                if self.speech.is_none() {
                    self.set_phase(Phase::Working, Some(crate::render::truncate_chars(&text, 80)));
                }
            }
            Some(Ev::ToolInfo { name, .. }) => {
                self.main_text.clear();
                if self.speech.is_none() {
                    self.set_phase(Phase::Working, Some(name));
                }
            }
            _ => {}
        }
    }

    /// `sb msg-you : {from} : {text}` in any feed: an agent (or main)
    /// writing to the user, shown as main's words, with who wrote it.
    fn msg_you(&mut self, line: &str) {
        let Some(Ev::AgentMsg { from, text, level: 2, .. }) = parse_line(line) else { return };
        let text = super::plain::plain(&text);
        if !text.is_empty() {
            let turn = self.turn;
            self.emit(json!({"ev": "main", "text": text, "turn": turn, "from": from}));
        }
    }

    /// Main's turn has the message sent last: its shots and its voice.
    fn reached(&mut self) {
        for s in &mut self.sent_shots {
            s.reached = true;
        }
        if self.voice_out == Some(false) {
            self.voice_out = Some(true);
        }
    }

    fn turn_done(&mut self, how: &str, line: &str, for_user: bool) {
        // the shots main's turn had are forgotten now
        let (done, keep): (Vec<Shot>, Vec<Shot>) = self.sent_shots.drain(..).partition(|s| s.reached);
        self.sent_shots = keep;
        self.forget(done);
        if !for_user {
            // a turn for an agent: the capsule keeps its status line
            if self.voice_out == Some(true) {
                self.voice_out = None;
            }
            return;
        }
        // main's answer: its words after its last tool call
        if !self.main_text.is_empty() {
            let (text, turn) = (self.main_text.clone(), self.turn);
            self.emit(json!({"ev": "main", "text": text, "turn": turn}));
        }
        if how != "completed" {
            let why = match parse_line(line) {
                Some(Ev::Err(t) | Ev::Warn(t)) => t,
                _ => how.to_string(),
            };
            if self.voice_out == Some(true) {
                self.voice_out = None;
            }
            return self.set_phase(Phase::Failed, Some(why));
        }
        if self.voice_out == Some(true) {
            self.voice_out = None;
            // on a call or in a meeting: text only
            if !self.quiet && !self.voice_off && self.say_main() {
                return self.set_phase(Phase::Speaking, None);
            }
        }
        self.set_phase(Phase::Done, None);
    }
}
