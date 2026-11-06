//! The core's voice and clock: main's words spoken, the ~30 Hz tick
//! (the mic to the listener, its words, the speaker's words and levels).

use super::*;
use crate::voice::TRANSCRIBE_TIMEOUT;

/// What reaches a fn talk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TalkEv {
    /// talk_start: fn down
    Start,
    /// talk_end: fn up
    FnUp,
    /// his press on the window's mic
    DictateStart,
    /// the clock
    Tick,
    /// talk_cancel: fn let go without words, escaped, the helper gone
    Cancel,
}

/// Why a listening talk ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TalkEnd {
    FnUp,
    /// the window's mic took it: a lost fn up (amb-tools H3)
    Dictation,
    /// no fn up in DICTATE_MAX
    Limit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TalkRule {
    /// the listening talk ends as at fn up: its words go
    End(TalkEnd),
    /// nothing changes (or, with no talk, a talk_start starts one)
    Keep,
    /// a talk_start while a talk runs: ignored, said in core.log
    Refuse,
    /// the listening talk goes, its words with it
    Cancel,
}

/// The fn talk's one end rule (architect m_22291): `talk` is (listening,
/// fn down at), None with no talk. A lost fn up reads as a fn up: a
/// press on the window's mic or DICTATE_MAX ends a listening talk, its
/// words to main, never a mic held forever. A cancel takes only a
/// listening talk: one past its end (fn up, a take, the limit) was
/// promised to main, its flush finishes and its words go (amb-tools
/// m_22412: the helper's exit cancel came during a take's flush).
pub(super) fn talk_rule(talk: Option<(bool, Instant)>, ev: TalkEv, now: Instant) -> TalkRule {
    let Some((listening, started)) = talk else { return TalkRule::Keep };
    match ev {
        TalkEv::Start => TalkRule::Refuse,
        _ if !listening => TalkRule::Keep,
        TalkEv::Cancel => TalkRule::Cancel,
        TalkEv::FnUp => TalkRule::End(TalkEnd::FnUp),
        TalkEv::DictateStart => TalkRule::End(TalkEnd::Dictation),
        TalkEv::Tick if now.duration_since(started) >= DICTATE_MAX => TalkRule::End(TalkEnd::Limit),
        TalkEv::Tick => TalkRule::Keep,
    }
}

/// The window's one notice when a talk ended without his fn up (never
/// silent, architect m_22291); `sent`: its words went to main.
pub(super) fn talk_end_words(end: TalkEnd, sent: bool) -> Option<String> {
    let head = if sent { "fn talk sent to main" } else { "fn talk ended" };
    match end {
        TalkEnd::FnUp => None,
        TalkEnd::Dictation => Some(format!("{head}: the mic went to dictation")),
        TalkEnd::Limit => Some(format!("{head} after {} min with no fn up", DICTATE_MAX.as_secs() / 60)),
    }
}

impl Core {
    /// Main's message aloud. False: no voice (no TTS, no speaker,
    /// nothing to say); the text shows anyway.
    pub(super) fn say_main(&mut self) -> bool {
        if self.main_text.is_empty() {
            return false;
        }
        let job = match (self.ports.say_job)() {
            Ok(j) => j,
            Err(_) => {
                self.error("main can't speak: no voice model is set up.");
                return false;
            }
        };
        if self.speaker.is_none() {
            match (self.ports.open_speaker)() {
                Ok(s) => self.speaker = Some(s),
                Err(e) => {
                    self.error(&e);
                    return false;
                }
            }
        }
        self.stop_voice();
        let lang = self.ports.language.clone();
        let Some(sp) = Speech::new(self.turn, &self.main_text, lang.as_deref(), job, self.next_utt) else {
            return false;
        };
        self.next_utt += sp.utts();
        self.speech = Some(sp);
        true
    }

    // ---- the clock ----

    /// ~30 Hz: the mic to the listener, its words, the voice and its
    /// words, the levels.
    pub fn tick(&mut self, now: Instant) {
        self.tick_talk(now);
        self.tick_dictation(now);
        self.tick_voice_mode(now);
        self.tick_picks();
        self.tick_speech();
        self.tick_projects(now);
        self.poll_projects(false);
        self.tick_index(now);
        self.setup_tick();
        self.tick_computer(now);
    }

    fn tick_talk(&mut self, now: Instant) {
        // a talk still listening with no talk_end (fn up missed, the
        // capsule hidden): its mic would refuse every dictation after it
        // (ambient-lead m_22196). Past the mic's time limit it ends like
        // fn up: his words go, the mic is free.
        if talk_rule(self.talk.as_ref().map(Talk::state), TalkEv::Tick, now) == TalkRule::End(TalkEnd::Limit) {
            eprintln!("ambient-core: talk: the time limit, no talk_end in {} s", DICTATE_MAX.as_secs());
            self.talk_end_by(now, TalkEnd::Limit);
        }
        let Some(t) = self.talk.as_mut() else { return };
        while let Ok(b) = t.blocks.try_recv() {
            let _ = t.audio.send(ListenMsg::Audio(b.pcm));
        }
        let mut evs = Vec::new();
        let mut flushed = false;
        let mut failed = None;
        let mut words = false;
        loop {
            match t.heard.try_recv() {
                Ok(Heard::Text(w)) => {
                    t.text.push_str(&w);
                    words = true;
                    if t.ending.is_none() {
                        evs.push(heard(t.text.trim(), false));
                    }
                }
                Ok(Heard::Flushed) => {
                    flushed = true;
                    break;
                }
                // fn's talk: the line goes to core.log, the words go on
                Ok(Heard::Note(line)) => eprintln!("ambient-core: talk: {line}"),
                Ok(Heard::Failed(e)) => {
                    failed = Some(e);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    flushed = t.ending.is_some();
                    break;
                }
            }
        }
        if let Some(s) = &t.stream {
            evs.push(json!({"ev": "level", "who": "you", "v": s.level()}));
        }
        // fn up: his words come at the listener's flush, however long a
        // batch request takes (the TUI's wait; it was 1.5 s and a slower
        // request lost them, m_22120); past it, what was heard goes, and
        // nothing heard is said as a failure, never a silent idle
        let late = t.ending.is_some_and(|e| now.duration_since(e) >= TRANSCRIBE_TIMEOUT);
        let timed_out = late && !flushed && failed.is_none() && t.text.trim().is_empty();
        let ending = t.ending.is_some();
        let page = t.page.clone();
        // a note talk's words so far, live on the page
        if let (Some(p), true, None) = (&page, words, &failed) {
            let text = t.text.trim().to_string();
            self.home_call("page/voice", json!({"page": p, "phase": "heard", "text": text}));
        }
        self.out.extend(evs);
        let failed = failed.or_else(|| timed_out.then(|| "the transcription timed out.".to_string()));
        if let Some(e) = failed {
            eprintln!("ambient-core: talk: the listener: {e}");
            self.talk = None;
            if let Some(p) = page {
                self.error(&e);
                return self.page_voice_over(&p, "cancel", "");
            }
            self.drop_shot();
            self.error(&e);
            return self.set_phase(Phase::Failed, None);
        }
        if ending && (flushed || late) {
            self.finish_talk();
        }
    }

    fn tick_speech(&mut self) {
        let (Some(sp), Some(spk)) = (self.speech.as_mut(), self.speaker.as_mut()) else { return };
        let step = sp.step(self.ports.synth.as_ref(), spk.as_mut());
        let (turn, level) = (sp.turn, spk.level());
        match step {
            Step::Playing { word } => {
                if let Some(i) = word {
                    self.ev(CoreEv::Word { turn, i: i as u64 });
                }
                self.emit(json!({"ev": "level", "who": "main", "v": level}));
            }
            Step::Over { word } => {
                self.speech = None;
                if let Some(i) = word {
                    self.ev(CoreEv::Word { turn, i: i as u64 });
                }
                self.emit(json!({"ev": "level", "who": "main", "v": 0.0}));
                if self.talk.is_none() {
                    self.set_phase(Phase::Done, None);
                }
            }
            Step::Failed(e) => {
                self.speech = None;
                self.emit(json!({"ev": "level", "who": "main", "v": 0.0}));
                self.error(&e);
                self.set_phase(Phase::Done, None);
            }
        }
    }
}

// ---- his talk: fn held, a note talk on a page, a window shot ----

impl Core {
    /// fn let go without words (or escaped): nothing goes to main. A talk
    /// already ending ([`talk_rule`]) is left to its flush.
    pub(super) fn talk_cancel(&mut self, now: Instant) {
        match talk_rule(self.talk.as_ref().map(Talk::state), TalkEv::Cancel, now) {
            TalkRule::Cancel => {}
            _ => {
                if self.talk.is_some() {
                    eprintln!("ambient-core: talk: talk_cancel while the talk ends: its words still go");
                }
                return;
            }
        }
        if let Some(page) = self.talk.as_ref().and_then(|t| t.page.clone()) {
            self.talk = None;
            return self.page_voice_over(&page, "cancel", "");
        }
        if self.talk.take().is_some() {
            // the shot of fn down was for these words
            self.drop_shot();
            self.set_phase(Phase::Idle, None);
        }
    }

    pub(super) fn talk_start(&mut self, page: Option<String>, now: Instant) {
        // each fn talk on receipt, in core.log (ambient-lead m_21884: did it reach the core?)
        eprintln!("ambient-core: talk_start{}", page.as_deref().map(|p| format!(" (note on page {p})")).unwrap_or_default());
        // a home hub the connector gave up restarting is tried again now
        super::super::talked();
        if let Some(t) = self.talk.as_ref().filter(|t| talk_rule(Some(t.state()), TalkEv::Start, now) == TalkRule::Refuse) {
            // never silent in core.log (his fn stuck on 'thinking…', main m_21882)
            // its age says a double fn down (ms) from a lost talk_end (s)
            // (ambient-lead m_22266); the sender is only known in the app
            eprintln!(
                "ambient-core: talk: talk_start while a talk runs ({}, started {} ms ago): ignored",
                if t.ending.is_some() { "ending" } else { "listening" },
                now.duration_since(t.started).as_millis()
            );
            return;
        }
        // one mic owner: voice mode has it (ambient-lead m_11168)
        if let Some(why) = self.voice_mode_busy() {
            eprintln!("ambient-core: talk: {why}");
            return self.emit(json!({"ev": "error", "cmd": "talk_start", "text": why}));
        }
        // fn wins over the composer's mic: the dictation ends first
        self.dictate_yield();
        // fn down while main speaks: the voice stops (the app sends
        // cut_in first; a talk_start alone does the same)
        self.stop_voice();
        let job = match (self.ports.listen_job)() {
            Ok(j) => j,
            Err(e) => {
                self.error(&e);
                return self.set_phase(Phase::Failed, None);
            }
        };
        let (btx, blocks) = mpsc::channel();
        let stream = match self.ports.mic.open(btx) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("ambient-core: talk: the mic: {e}");
                self.error(&e);
                return self.set_phase(Phase::Failed, None);
            }
        };
        let (audio, arx) = mpsc::channel();
        let (htx, heard) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        (self.ports.listener)(&job).start(job, arx, htx, cancel.clone());
        self.talk = Some(Talk { stream: Some(stream), blocks, audio, heard, cancel, text: String::new(), started: now, ending: None, ended_by: TalkEnd::FnUp, page: page.clone() });
        let Some(id) = page else { return self.set_phase(Phase::Listening, None) };
        // a note talk: the page shows the words at its mouse, the
        // capsule says where they go
        self.home_call("page/voice", json!({"page": id, "phase": "start", "text": ""}));
        let title = match self.page_titles.iter().find(|(i, _)| *i == id) {
            Some((_, t)) if !t.trim().is_empty() => t.clone(),
            _ => id.replace('-', " "),
        };
        self.phase = Phase::Listening;
        self.ev(CoreEv::Phase { phase: "listening".into(), line: None, page: Some(bise_proto::app_rows::PhasePage { id: id.to_string(), title }) });
    }

    /// A note talk is over: its last words to the page, the capsule idle
    /// (no `sent`, no main phases: main never hears a note talk).
    pub(super) fn page_voice_over(&mut self, page: &str, phase: &str, text: &str) {
        self.home_call("page/voice", json!({"page": page, "phase": phase, "text": text}));
        self.emit(json!({"ev": "level", "who": "you", "v": 0.0}));
        self.set_phase(Phase::Idle, None);
    }

    pub(super) fn talk_end(&mut self, now: Instant) {
        eprintln!("ambient-core: talk_end");
        if self.talk.is_none() {
            eprintln!("ambient-core: talk: talk_end with no talk: ignored");
        }
        if let TalkRule::End(by) = talk_rule(self.talk.as_ref().map(Talk::state), TalkEv::FnUp, now) {
            self.talk_end_by(now, by);
        }
    }

    /// The listening talk ends as at fn up ([`talk_rule`]'s End): the mic
    /// off, its words to come; `by`: what ended it (fn up, the window's
    /// mic, the time limit).
    pub(super) fn talk_end_by(&mut self, now: Instant, by: TalkEnd) {
        let Some(t) = self.talk.as_mut() else { return };
        if t.ending.is_some() {
            return;
        }
        t.ended_by = by;
        while let Ok(b) = t.blocks.try_recv() {
            let _ = t.audio.send(ListenMsg::Audio(b.pcm));
        }
        // the mic goes off now (the macOS mic indicator too)
        t.stream = None;
        let _ = t.audio.send(ListenMsg::Flush);
        t.ending = Some(now);
        let note = t.page.is_some();
        self.emit(json!({"ev": "level", "who": "you", "v": 0.0}));
        // a note talk never shows main's phases
        if !note {
            self.set_phase(Phase::Sending, None);
        }
    }

    /// The talk's words are all in (or waited for long enough): to main.
    pub(super) fn finish_talk(&mut self) {
        let Some(t) = self.talk.take() else { return };
        let words = t.text.trim().to_string();
        self.emit(heard(&words, true));
        // ended without his fn up: one notice in the window, its cause
        // (architect m_22291); "sent to main" only when they went there
        let to_main = t.page.is_none() && !words.is_empty();
        let by = t.ended_by;
        let notice = |core: &mut Core, sent: bool| {
            if let Some(text) = talk_end_words(by, sent) {
                eprintln!("ambient-core: talk: notice: {text}");
                let project = core.home_project();
                core.emit(json!({"ev": "notice", "project": project, "text": text}));
            }
        };
        if !to_main {
            notice(self, false);
        }
        if let Some(page) = &t.page {
            // nothing heard: no note; the word "send" alone sends the
            // page's notes (§4.1)
            let phase = match send_word(&words) {
                _ if words.is_empty() => "cancel",
                true => "send",
                false => "end",
            };
            return self.page_voice_over(page, phase, &words);
        }
        if words.is_empty() {
            self.drop_shot();
            return self.set_phase(Phase::Idle, None);
        }
        // bar N14: words that answer the card in view don't go to main
        if self.voice_answer(&words) {
            notice(self, false);
            return self.set_phase(Phase::Idle, None);
        }
        notice(self, true);
        self.send_to_main(words, true);
    }

    pub(super) fn take_shot(&mut self, path: &Path, app: String, title: String) {
        let stored = (self.ports.store_image)(path);
        // the app's PNG goes at once, stored or not
        let _ = std::fs::remove_file(path);
        match stored {
            Ok(stored) => {
                self.shot_n += 1;
                if let Some(old) = self.shot.replace(Shot { id: self.shot_n, stored, app, title }) {
                    self.forget(vec![old]);
                }
                let ev = shot_ev(self.shot.as_ref());
                self.emit(ev);
            }
            Err(_) => self.error("main can't see this window: the screenshot didn't save."),
        }
    }

    /// The talk's still goes unsent (his ✕ on it: `shot_drop`, a cancel,
    /// a talk with no words, a failed talk): forgotten, and the app told
    /// (`shot {file: null}`); the talk itself goes on.
    pub(super) fn drop_shot(&mut self) {
        if let Some(s) = self.shot.take() {
            self.forget(vec![s]);
            self.emit(shot_ev(None));
        }
    }

    /// `shot_drop {id}`: only that still (the spotlight closed on it, a
    /// newer one stays); no id: whatever still is pending.
    pub(super) fn drop_shot_id(&mut self, id: Option<u64>) {
        if id.is_none_or(|id| self.shot.as_ref().is_some_and(|s| s.id == id)) {
            self.drop_shot();
        }
    }
}

/// fn's talk's still as the app reads it: bise-proto's CoreEv::Shot
/// (ambient-lead m_21912): its stored file, front app and window, or
/// `file: null` once it went.
pub(super) fn shot_ev(shot: Option<&Shot>) -> Value {
    let some = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
    let ev = match shot {
        Some(s) => bise_proto::core_ev::CoreEv::Shot { file: Some(s.stored.file.display().to_string()), id: Some(s.id), app: some(&s.app), title: some(&s.title) },
        None => bise_proto::core_ev::CoreEv::Shot { file: None, id: None, app: None, title: None },
    };
    serde_json::to_value(ev).unwrap_or_default()
}

/// fn's talk words as the app reads them: bise-proto's CoreEv::Heard, its
/// one owner (architect m_11629), the whole text so far or, `done`, all of it.
fn heard(text: &str, done: bool) -> Value {
    serde_json::to_value(bise_proto::core_ev::CoreEv::Heard { text: text.to_string(), is_final: done }).unwrap_or_default()
}

#[cfg(test)]
mod rule_tests {
    use super::*;

    /// The talk's end rule on its three ways (architect m_22291): fn up,
    /// the window's mic, the time limit; a talk_start while one runs is
    /// refused; a talk past fn up is left to its flush.
    #[test]
    fn a_listening_talk_ends_at_fn_up_a_dictation_or_the_limit() {
        let now = Instant::now();
        let Some(long_ago) = now.checked_sub(DICTATE_MAX) else { return };
        let fresh = Some((true, now));
        assert_eq!(talk_rule(fresh, TalkEv::FnUp, now), TalkRule::End(TalkEnd::FnUp));
        assert_eq!(talk_rule(fresh, TalkEv::DictateStart, now), TalkRule::End(TalkEnd::Dictation));
        assert_eq!(talk_rule(fresh, TalkEv::Tick, now), TalkRule::Keep);
        assert_eq!(talk_rule(Some((true, long_ago)), TalkEv::Tick, now), TalkRule::End(TalkEnd::Limit));
        assert_eq!(talk_rule(fresh, TalkEv::Start, now), TalkRule::Refuse);
        assert_eq!(talk_rule(fresh, TalkEv::Cancel, now), TalkRule::Cancel);
        // past fn up: its words are on their way, nothing ends it again,
        // a cancel neither (amb-tools m_22412)
        for ev in [TalkEv::FnUp, TalkEv::DictateStart, TalkEv::Tick, TalkEv::Cancel] {
            assert_eq!(talk_rule(Some((false, long_ago)), ev, now), TalkRule::Keep, "{ev:?}");
        }
        assert_eq!(talk_rule(Some((false, now)), TalkEv::Start, now), TalkRule::Refuse);
        // no talk: a talk_start starts one, the rest is nothing
        for ev in [TalkEv::Start, TalkEv::FnUp, TalkEv::DictateStart, TalkEv::Tick, TalkEv::Cancel] {
            assert_eq!(talk_rule(None, ev, now), TalkRule::Keep, "{ev:?}");
        }
    }

    #[test]
    fn only_a_lost_fn_up_gets_a_notice() {
        assert_eq!(talk_end_words(TalkEnd::FnUp, true), None);
        assert_eq!(talk_end_words(TalkEnd::Dictation, true).as_deref(), Some("fn talk sent to main: the mic went to dictation"));
        assert_eq!(talk_end_words(TalkEnd::Dictation, false).as_deref(), Some("fn talk ended: the mic went to dictation"));
        assert_eq!(talk_end_words(TalkEnd::Limit, true).as_deref(), Some("fn talk sent to main after 2 min with no fn up"));
    }
}
