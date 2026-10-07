//! Speech to text for voice mode (owner: voice-models; plan §4.3, §8):
//! the voice role's batch model (Voxtral Transcribe 3 by default), one
//! request per turn. The controller's VAD ends the turn
//! ([`ListenMsg::Flush`]); the turn's audio goes to
//! `voice::transcribe_clip`, its words come back as one [`Heard::Text`],
//! then [`Heard::Flushed`]. [`ListenMsg::Clear`] drops the half turn.
//!
//! Round 2 (the user's notes on 699f710): no realtime listener. The only
//! realtime model was Voxtral Mini's, and it got too many words wrong;
//! Transcribe 3 has no streaming endpoint, so `ListenJob::realtime` is
//! always None now (bise_catalog::voice::realtime_model).
//!
//! Failures: [`Heard::Failed`] with `voice::fail_lines`' wording ends the
//! turn (no `Flushed` for it).

use super::{Heard, ListenJob, ListenMsg, Listener, MIC_RATE};
use crate::voice::{self, http, Failure, VoiceJob};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

/// One batch request.
const BATCH_TIMEOUT: Duration = Duration::from_secs(60);
/// The batch clip keeps the last this many samples (5 min, ~9.6 MB):
/// the mic sends audio while nobody talks too.
const MAX_CLIP: usize = 5 * 60 * MIC_RATE as usize;

/// The listener for `job`: the batch one (no provider has a realtime
/// model we offer).
pub fn listener_for(_job: &ListenJob) -> Box<dyn Listener> {
    Box::new(BatchListener)
}

/// One batch request per turn (voice::transcribe_clip), at Flush.
pub struct BatchListener;

/// Your languages across talks (the capsule opens a listener per talk):
/// the last you spoke stays known for the next talk.
static LANGS: std::sync::Mutex<Option<Langs>> = std::sync::Mutex::new(None);

impl Listener for BatchListener {
    fn start(&self, job: ListenJob, audio: Receiver<ListenMsg>, events: Sender<Heard>, cancel: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            let mut langs = LANGS.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_else(|| Langs::new(system_languages()));
            run_batch(&job.batch, &audio, &events, &cancel, &mut langs, &|req| http::send(req, BATCH_TIMEOUT));
            *LANGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(langs);
        });
    }
}

// ---- the language guard (voice-echo3) ----
//
// With `[voice] language` unset (auto), Transcribe 3 detects the language
// per turn, and once heard two quick French words as Russian ("Да,
// наверное."). A turn whose words are in a script none of your languages
// write (Cyrillic for a French and English speaker) is transcribed again
// with your language: the last one you spoke, else your Mac's first.

/// A clip shorter than this is short: auto-detect is unsure on it.
const SHORT_CLIP_SECS: f32 = 4.0;

/// A writing system, enough to tell a misdetection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Latin,
    Cyrillic,
    Greek,
    Arabic,
    Hebrew,
    Cjk,
    Other,
}

/// The script a language (ISO 639-1, `fr`, `ru`) is written in.
pub fn script_of_lang(lang: &str) -> Script {
    match base(lang).as_str() {
        "ru" | "uk" | "bg" | "sr" | "mk" | "be" | "kk" => Script::Cyrillic,
        "el" => Script::Greek,
        "ar" | "fa" | "ur" => Script::Arabic,
        "he" | "yi" => Script::Hebrew,
        "zh" | "ja" | "ko" => Script::Cjk,
        "hi" | "th" | "bn" | "ta" | "te" | "ka" | "hy" => Script::Other,
        _ => Script::Latin,
    }
}

fn script_of_char(c: char) -> Option<Script> {
    if !c.is_alphabetic() {
        return None;
    }
    Some(match c as u32 {
        0x0041..=0x024F | 0x1E00..=0x1EFF => Script::Latin,
        0x0400..=0x052F => Script::Cyrillic,
        0x0370..=0x03FF => Script::Greek,
        0x0600..=0x06FF | 0x0750..=0x077F => Script::Arabic,
        0x0590..=0x05FF => Script::Hebrew,
        0x3040..=0x30FF | 0x3400..=0x9FFF | 0xAC00..=0xD7AF => Script::Cjk,
        _ => Script::Other,
    })
}

/// The script most of `text`'s letters are in (None: no letters).
pub fn script_of_text(text: &str) -> Option<Script> {
    let mut counts: Vec<(Script, usize)> = Vec::new();
    for s in text.chars().filter_map(script_of_char) {
        match counts.iter_mut().find(|(k, _)| *k == s) {
            Some((_, n)) => *n += 1,
            None => counts.push((s, 1)),
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(s, _)| s)
}

/// `fr-FR`, `fr_FR.UTF-8` → `fr`.
fn base(lang: &str) -> String {
    lang.trim().split(['-', '_', '.']).next().unwrap_or("").to_ascii_lowercase()
}

/// Your languages: the ones you spoke in this voice mode (the last
/// first), then your Mac's.
#[derive(Clone, Debug, Default)]
pub struct Langs {
    spoken: Vec<String>,
    system: Vec<String>,
}

impl Langs {
    pub fn new(system: Vec<String>) -> Langs {
        let mut sys: Vec<String> = Vec::new();
        for l in system.iter().map(|l| base(l)).filter(|l| l.len() == 2 || l.len() == 3) {
            if !sys.contains(&l) {
                sys.push(l);
            }
        }
        Langs { spoken: Vec::new(), system: sys }
    }

    /// The language to transcribe `text`'s audio again with: `text` is
    /// in a script none of your languages write. None: keep it (no
    /// language known, or the script is one of yours).
    pub fn redo(&self, text: &str) -> Option<String> {
        let script = script_of_text(text)?;
        let mut known = self.spoken.iter().chain(&self.system);
        if known.clone().any(|l| script_of_lang(l) == script) {
            return None;
        }
        known.next().cloned()
    }

    /// The language to check an English transcript of `secs` of audio
    /// in (ambient-lead m_6275: his first French ask came back as
    /// English): auto-detect is unsure on the first words of a session
    /// and on short clips, so when you have not spoken yet here, or the
    /// clip is short, and your language (the last you spoke, else your
    /// Mac's first) is not English, the clip is heard again in it. None:
    /// keep the transcript.
    pub fn recheck(&self, text: &str, secs: f32) -> Option<String> {
        if super::speak::language(text) != Some("en") || !(self.spoken.is_empty() || secs < SHORT_CLIP_SECS) {
            return None;
        }
        let mine = self.spoken.first().or(self.system.first())?;
        (mine != "en" && script_of_lang(mine) == Script::Latin).then(|| mine.clone())
    }

    /// A turn was kept: its language (when it tells) is the last you spoke.
    pub fn heard(&mut self, text: &str) {
        if let Some(l) = super::speak::language(text) {
            self.spoken.retain(|x| x != l);
            self.spoken.insert(0, l.to_string());
        }
    }
}

/// Your Mac's languages, preferred first (`defaults read -g
/// AppleLanguages`, then LC_ALL / LANG). Reads, never writes.
pub fn system_languages() -> Vec<String> {
    let mut out = Vec::new();
    #[cfg(target_os = "macos")]
    if let Ok(o) = std::process::Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output() {
        out.extend(parse_apple_languages(&String::from_utf8_lossy(&o.stdout)));
    }
    for k in ["LC_ALL", "LANG"] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() && v != "C" && v != "POSIX" && !v.starts_with("C.") {
                out.push(v);
            }
        }
    }
    out
}

/// `(\n    "fr-FR",\n    "en-FR"\n)` → ["fr-FR", "en-FR"].
pub fn parse_apple_languages(text: &str) -> Vec<String> {
    text.split([',', '\n'])
        .map(|l| l.trim().trim_matches(|c| c == '"' || c == '(' || c == ')').trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// A failure as voice mode says it: `voice::fail_lines`' first line.
fn fail_line(f: &Failure, provider: &str, model: &str, billing: &str) -> String {
    voice::fail_lines(f, provider, model, billing, false).head
}

/// One `voice::transcribe_clip` per Flush over the audio since the last
/// one, until the controller goes or `cancel`. `send`: the HTTP call.
fn run_batch(
    job: &VoiceJob,
    audio: &Receiver<ListenMsg>,
    events: &Sender<Heard>,
    cancel: &AtomicBool,
    langs: &mut Langs,
    send: &dyn Fn(&http::Request) -> Result<http::Response, String>,
) {
    let mut clip: Vec<i16> = Vec::new();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        match audio.recv_timeout(Duration::from_millis(50)) {
            Ok(ListenMsg::Audio(pcm)) => {
                clip.extend(pcm);
                if clip.len() > MAX_CLIP + MAX_CLIP / 4 {
                    clip.drain(..clip.len() - MAX_CLIP);
                }
            }
            Ok(ListenMsg::Clear) => clip.clear(),
            Ok(ListenMsg::Flush) => {
                let mut result = voice::transcribe_clip(job, &clip, cancel, send);
                // auto language, and the words came in a script you never
                // use: again, with your language
                let auto = job.language.as_deref().is_none_or(|l| l.trim().is_empty());
                if let (true, Ok(text)) = (auto, &result) {
                    if let Some(lang) = langs.redo(text) {
                        let again = VoiceJob { language: Some(lang.clone()), ..job.clone() };
                        let first = text.clone();
                        if let Ok(t) = voice::transcribe_clip(&again, &clip, cancel, send) {
                            super::debug::log(|| format!("language guard · \"{}\" redone in {} · \"{}\"", first, lang, t));
                            result = Ok(t);
                        }
                    }
                }
                // auto language, and the words came back English where you
                // likely spoke yours (the session's first words, a short
                // clip): heard again in yours, kept when it reads as not
                // English (an English ask stays English)
                let secs = clip.len() as f32 / MIC_RATE as f32;
                if let (true, Ok(text)) = (auto, &result) {
                    if let Some(lang) = langs.recheck(text, secs) {
                        let again = VoiceJob { language: Some(lang.clone()), ..job.clone() };
                        if let Ok(t) = voice::transcribe_clip(&again, &clip, cancel, send) {
                            let keep = !t.is_empty() && super::speak::language(&t) != Some("en");
                            super::debug::log(|| format!("language recheck · \"{}\" heard in {} · \"{}\" · {}", text, lang, t, if keep { "kept" } else { "dropped" }));
                            if keep {
                                result = Ok(t);
                            }
                        }
                    }
                }
                if let Ok(text) = &result {
                    langs.heard(text);
                }
                clip.clear();
                if cancel.load(Ordering::SeqCst) {
                    return;
                }
                let _ = match result {
                    Ok(text) => {
                        if !text.is_empty() {
                            let _ = events.send(Heard::Text(format!(" {}", text)));
                        }
                        events.send(Heard::Flushed)
                    }
                    Err(f) => events.send(Heard::Failed(fail_line(&f, &job.provider_name, &job.model, &job.billing_url))),
                };
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[cfg(test)]
#[path = "listen_tests.rs"]
mod tests;
