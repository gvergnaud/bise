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
mod hub_rpc;
mod home;
mod home_switch;
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
#[cfg(test)]
pub use self::home::{MAIN_PAGE, QUIET_PAGE};
use super::hub::{Hub, HubIn};
use super::speech::{Speech, Step};
use crate::voicemode::{Heard, ListenJob, ListenMsg, Listener, Mic, MicBlock, MicStream, SayJob, Speaker, Synthesizer, UttId};
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
    /// the move of an older home hub to this core's version (core/home_switch.rs)
    home_move: Option<home_switch::HomeMoveState>,
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
    hub_agents: Vec<bise_proto::rows::Agent>,
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
    /// the home hub's connection (core/home.rs)
    home: home::Home,
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
            home_move: None,
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
            home: home::Home::default(),
            typing: std::collections::HashMap::new(),
            pages_url: None,
            quiet: false,
            voice_off: false,
        }
    }

    /// The events since the last call, in order.
    pub fn take_out(&mut self) -> Vec<Value> {
        // the hub's state kinds that came since: one `state`
        if std::mem::take(&mut self.home.state_due) {
            self.emit_state();
        }
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
            // JSON-RPC's scheduled/stop on the home connection (the
            // untyped every_stop op is gone); its answer is not read
            Cmd::EveryStop { id } => {
                let project = bise_home::hub_id(Path::new(&self.workspace));
                let req = json!({"jsonrpc": "2.0", "id": "every_stop", "method": "scheduled/stop", "params": {"project": project, "id": id}});
                if !self.hub.send(&req) {
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
                self.emit_state();
            }
            // JSON-RPC's turn/interrupt on the home connection (P1c: the
            // untyped interrupt op is gone); its answer is not read
            Cmd::Stop { agent } => {
                if !self.home_call("turn/interrupt", json!({"agent": agent})) {
                    self.error("bise isn't reachable: it keeps going. try again in a moment.");
                }
            }
            Cmd::Archive { agent, stop_first } => self.home_act("agent/archive", json!({"agent": agent, "force": stop_first})),
            Cmd::Unarchive { agent } => self.home_act("agent/unarchive", json!({"agent": agent})),
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

    /// An action of his on the home connection: unreachable, he hears it.
    fn home_act(&mut self, method: &str, params: Value) {
        if !self.home_call(method, params) {
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
        let mut input = json!({"agent": "main", "text": text, "via": via});
        if let Some(c) = ctx {
            input["context"] = json!(c);
        }
        if !files.is_empty() {
            input["files"] = json!(files);
        }
        if !self.home_call("turn/send", input) {
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
        if !self.home_call("card/answer", json!({"card": id, "reply": reply})) {
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
            // JSON-RPC's `initialize`, then typed events only (core/home.rs)
            HubIn::Up => self.home_up(),
            HubIn::Down => {
                if self.hub_up != Some(false) {
                    self.hub_up = Some(false);
                    let ws = self.workspace.clone();
                    self.emit(json!({"ev": "hub", "up": false, "workspace": ws}));
                }
            }
            HubIn::Line(l) => self.home_line(&l),
            // the hub refused this core (docs/issues/16): never again until
            // a new core starts; the window says why
            HubIn::Refused(why) => {
                self.hub_up = Some(false);
                let project = bise_home::hub_id(Path::new(&self.workspace));
                self.emit(json!({"ev": "hub_refused", "project": project, "error": why}));
            }
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

    /// Main's turn ended (`fail`: why, when it didn't complete).
    fn turn_done(&mut self, fail: Option<String>, for_user: bool) {
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
        if let Some(why) = fail {
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
