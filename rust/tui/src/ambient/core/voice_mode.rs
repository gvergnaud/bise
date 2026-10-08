//! Voice mode with the agent in view (bar V12/W24/L21/N13, the TUI's
//! ctrl+r twice; plan approved by architect m_11164). The window sends
//! `voice_mode {project, agent, on}`; the core runs the TUI's own
//! controller (voicemode::turn::VoiceMode) with its own ports (the mic,
//! the speaker, the voice role's listener, the TTS: voicemode/live.rs
//! `for_core`), so the mic and the speaker never leave the core.
//!
//! - His turn goes to the agent he talks to, on THAT project's hub (a
//!   typed `send`), never to main unless main is the one in view; first
//!   it is tried as an answer to that agent's open card with the TUI's
//!   rule (core/voice_answer.rs `voice_answer_agent`, `voice_answer`).
//!   A cut-in by voice while the agent's turn runs stops it (`stop`).
//! - The agent's new messages (live `entry` of kind agent, never a page
//!   of old ones, never one twice) are said aloud as speakable sentences,
//!   but not while quiet (a call, a meeting) or with voice answers off.
//! - One mic owner: voice mode is refused while fn's talk or a dictation
//!   runs (`the mic is busy.`); while it is on, a dictation is refused the
//!   same way, and fn is refused with `voice mode is on with <agent> ·
//!   ⌃R to leave` (ambient-lead m_11168). Never two listeners.
//! - Its thread: the core subscribes only when the window doesn't, and
//!   unsubscribes when voice mode ends or moves (core/hubs.rs voice_sub).
//! - It belongs to the window that turned it on: the app's main ends it
//!   (`on: false`) when that window goes; the core ends it when it stops.
//!   It is never left listening.
//! - His words (`heard`) and the said sentence (`said`) go out only in
//!   `voice_mode` events, to that window; never in a log line.

use super::*;
use crate::voicemode::config::VoiceModeConfig;
use crate::voicemode::turn::{self, Act, Jobs, SpaceKey, VoiceMode};
use crate::voicemode::{Phase as Pane, Who};
use bise_proto::draft::AppCmd;
use bise_proto::hub::HubEv;
use bise_proto::thread::EntryKind;

/// Opens voice mode's ports, jobs and settings (live: the default mic and
/// speaker; tests: voicemode's fakes).
pub type OpenVoiceMode = Box<dyn FnMut() -> Result<(turn::Ports, Jobs, VoiceModeConfig), String>>;

/// An agent message older than voice mode's start by more than this is
/// not said (a page the window or the core asked for).
const OLD_MS: u64 = 2_000;

/// Voice mode, on.
pub(super) struct VoiceOn {
    project: String,
    vm: VoiceMode,
    started_ms: u64,
    /// the agent entries said already (an entry may come again, changed)
    said: std::collections::BTreeSet<(String, u64)>,
    /// the last `voice_mode` event (sent on change only)
    last: Option<Value>,
    /// a note to carry on the next event
    note: Option<String>,
    levels: (f32, f32),
}

impl Core {
    /// Why fn or a dictation can't have the mic: voice mode has it.
    pub(super) fn voice_mode_busy(&self) -> Option<String> {
        self.voice_on.as_ref().map(|v| format!("voice mode is on with {} · ⌃R to leave", v.vm.agent()))
    }

    /// The window's voice mode commands (one dispatch arm in core.rs).
    pub(super) fn voice_mode_cmd(&mut self, c: AppCmd, now: Instant) {
        match c {
            AppCmd::VoiceMode { project, agent, on: true } => self.voice_mode_on(project, agent, now),
            AppCmd::VoiceMode { on: false, .. } => self.voice_mode_off(now, None),
            AppCmd::VoiceMute { on } => {
                if let Some(v) = self.voice_on.as_mut() {
                    if v.vm.view(now).muted != on {
                        v.vm.toggle_mute();
                    }
                }
            }
            AppCmd::VoiceType { on } => {
                if let Some(v) = self.voice_on.as_mut() {
                    v.vm.set_typing(on);
                }
            }
            // the TUI's space tap: what he said goes now
            AppCmd::VoiceSend => {
                if let Some(v) = self.voice_on.as_mut() {
                    v.vm.space(SpaceKey::Press, now);
                    v.vm.space(SpaceKey::Release, now);
                }
            }
            // its voice stops (a space press over its voice is a cut), and
            // its running turn is stopped
            AppCmd::VoiceCut => {
                let Some(v) = self.voice_on.as_mut() else { return };
                v.vm.space(SpaceKey::Press, now);
                v.vm.space(SpaceKey::Release, now);
                let (project, agent) = (v.project.clone(), v.vm.agent().to_string());
                if self.agent_working(&project, &agent) {
                    self.typed_cmd(json!({"cmd": "stop", "project": project, "agent": agent}));
                }
            }
            _ => {}
        }
        self.voice_mode_emit(now);
    }

    fn voice_mode_error(&mut self, text: &str) {
        self.emit(json!({"ev": "error", "cmd": "voice_mode", "text": text}));
    }

    fn voice_mode_on(&mut self, project: String, agent: String, now: Instant) {
        if let Some(v) = self.voice_on.as_mut() {
            if v.project == project {
                // another agent of the same project: it follows him there
                if v.vm.agent() != agent {
                    v.vm.set_agent(&agent);
                    v.last = None;
                    self.voice_sub(Some((project.clone(), agent.clone())));
                    let running = self.agent_working(&project, &agent);
                    if let Some(v) = self.voice_on.as_mut() {
                        v.vm.on_turn(&agent, running);
                    }
                }
                return;
            }
            // another project: this one ends first
            self.voice_mode_off(now, None);
        }
        if self.talk.is_some() || self.dictation.is_some() {
            return self.voice_mode_error("the mic is busy.");
        }
        // main's voice stops: the mic would hear it
        if self.stop_voice() {
            self.set_phase(Phase::Done, None);
        }
        let started = (self.ports.voice_mode)().and_then(|(ports, jobs, cfg)| VoiceMode::start(&agent, ports, jobs, cfg, true, now));
        let mut vm = match started {
            Ok(vm) => vm,
            Err(e) => return self.voice_mode_error(&format!("voice mode: {e}")),
        };
        vm.on_turn(&agent, self.agent_working(&project, &agent));
        self.voice_on = Some(VoiceOn {
            project: project.clone(),
            vm,
            started_ms: now_ms(),
            said: Default::default(),
            last: None,
            note: None,
            levels: (0.0, 0.0),
        });
        self.voice_sub(Some((project, agent)));
    }

    /// Voice mode ends (he left, his window went, the core stops): the mic
    /// and the speaker close, its last event says how it went.
    pub(super) fn voice_mode_off(&mut self, now: Instant, why: Option<&str>) {
        let Some(mut v) = self.voice_on.take() else { return };
        let acts = v.vm.leave(now);
        let mut note = acts.into_iter().find_map(|a| match a {
            Act::Note(n) => Some(n),
            _ => None,
        });
        if let Some(w) = why {
            note = Some(match note {
                Some(n) => format!("{n} · {w}"),
                None => w.to_string(),
            });
        }
        let ev = json!({"ev": "voice_mode", "project": v.project, "agent": v.vm.agent(), "on": false, "state": "listening", "note": note});
        drop(v);
        self.voice_sub(None);
        self.emit(json!({"ev": "level", "who": "you", "v": 0.0}));
        self.emit(json!({"ev": "level", "who": "agent", "v": 0.0}));
        self.emit(strip_nulls(ev));
    }

    /// His typed send to the agent voice mode waits on: its answer is said.
    pub(super) fn voice_mode_typed(&mut self, project: &str, agent: &str) {
        let Some(v) = self.voice_on.as_mut() else { return };
        if v.project == project && v.vm.agent() == agent && v.vm.typing() {
            v.vm.typed_sent();
        }
    }

    /// A project hub's typed event: the agent's turn and its new messages.
    pub(super) fn voice_mode_hub(&mut self, ev: &HubEv) {
        let Some(project) = self.voice_on.as_ref().map(|v| v.project.clone()) else { return };
        match ev {
            HubEv::Agents { project: p, agents } if *p == project => {
                let Some(v) = self.voice_on.as_mut() else { return };
                let agent = v.vm.agent().to_string();
                let running = agents.iter().any(|a| a.name == agent && a.status.working());
                v.vm.on_turn(&agent, running);
            }
            HubEv::Entry { project: p, agent, entry } if *p == project && entry.kind == EntryKind::Agent => {
                let quiet = self.quiet || self.voice_off;
                let lang = self.ports.language.clone();
                let Some(v) = self.voice_on.as_mut() else { return };
                if *agent != v.vm.agent() || entry.at_ms + OLD_MS < v.started_ms || !v.said.insert((agent.clone(), entry.pos)) {
                    return;
                }
                // a call or a meeting: nothing aloud, the window shows the text
                if quiet {
                    return;
                }
                let spoken = crate::voicemode::speak::speakable(&entry.text, lang.as_deref());
                v.vm.on_message(agent, &entry.text, spoken);
            }
            _ => {}
        }
    }

    fn agent_working(&self, project: &str, agent: &str) -> bool {
        self.project_rows().into_iter().filter(|(p, _, _)| p == project).flat_map(|(_, agents, _)| agents).any(|a| a.name == agent && a.status.working())
    }

    /// The clock: the controller's step, its acts, its state on change.
    pub(super) fn tick_voice_mode(&mut self, now: Instant) {
        let Some(v) = self.voice_on.as_mut() else { return };
        let acts = v.vm.tick(now);
        let project = v.project.clone();
        for a in acts {
            match a {
                Act::Send { agent, text } => {
                    if let Some(line) = self.voice_answer_agent(&project, &agent, &text) {
                        if let Some(v) = self.voice_on.as_mut() {
                            v.vm.show_heard(line, now);
                        }
                        continue;
                    }
                    self.typed_cmd(json!({"cmd": "send", "project": project, "agent": agent, "text": text, "mode": "now"}));
                }
                Act::Interrupt { agent } => self.typed_cmd(json!({"cmd": "stop", "project": project, "agent": agent})),
                Act::Note(n) => {
                    if let Some(v) = self.voice_on.as_mut() {
                        v.note = Some(n);
                    }
                }
            }
        }
        self.voice_mode_emit(now);
    }

    /// `voice_mode` when its state changed, the levels while they move.
    fn voice_mode_emit(&mut self, now: Instant) {
        let Some(v) = self.voice_on.as_mut() else { return };
        let view = v.vm.view(now);
        let words = view.words.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>().join(" ");
        let (state, fail) = match &view.phase {
            Pane::Listening | Pane::CutIn => ("listening", None),
            Pane::Hearing | Pane::Holding => ("hearing", None),
            Pane::AboutToAnswer { .. } => ("sending", None),
            Pane::Working => ("thinking", None),
            Pane::Speaking | Pane::HoldToTalk => ("speaking", None),
            Pane::Muted => ("muted", None),
            Pane::Typing => ("typing", None),
            Pane::Failed(r) => ("failed", Some(r.clone())),
        };
        let heard = (view.who == Who::You && !words.is_empty() && matches!(state, "hearing" | "sending")).then(|| words.clone());
        let said = (matches!(view.who, Who::Agent(_)) && !words.is_empty() && state == "speaking").then_some(words);
        let ev = strip_nulls(json!({
            "ev": "voice_mode", "project": v.project, "agent": view.agent, "on": true, "state": state,
            "heard": heard, "said": said, "muted": view.muted, "note": v.note.clone(), "fail": fail,
        }));
        let mut out = Vec::new();
        if v.last.as_ref() != Some(&ev) {
            v.last = Some(ev.clone());
            v.note = None;
            out.push(ev);
        }
        let (you, agent) = (view.you_level, view.agent_level);
        if (you - v.levels.0).abs() > 0.01 {
            v.levels.0 = you;
            out.push(json!({"ev": "level", "who": "you", "v": you}));
        }
        if (agent - v.levels.1).abs() > 0.01 {
            v.levels.1 = agent;
            out.push(json!({"ev": "level", "who": "agent", "v": agent}));
        }
        self.out.extend(out);
    }
}

/// The event without its absent fields (and `muted` only when true).
fn strip_nulls(mut v: Value) -> Value {
    if let Some(m) = v.as_object_mut() {
        m.retain(|k, x| !x.is_null() && !(k == "muted" && *x == Value::Bool(false)));
    }
    v
}
