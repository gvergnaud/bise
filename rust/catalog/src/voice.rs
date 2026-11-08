//! Speech to text (BISE-130): which model transcribes the voice input,
//! and everything its call needs.
//!
//! The providers and models are catalog entries like the chat ones: a
//! provider that transcribes has `stt = "<family>"` (its wire format,
//! one of [`STT_FAMILIES`]) and shares `base_url` / `key_env` with its
//! chat models; `kind = "stt"` marks a provider or a model that only
//! transcribes. Adding an OpenAI-compatible one is data:
//!
//! ```toml
//! [providers.fireworks]
//! stt = "openai"
//! [models."fireworks/whisper-v3"]
//! kind = "stt"
//! ```
//!
//! The user's choice, in config.toml (env `BISE_VOICE_MODEL` wins):
//!
//! ```toml
//! [voice]
//! model = "mistral/voxtral-mini-transcribe-3"
//! language = "fr"                       # optional; unset: detected
//! vocabulary = ["bise", "config.toml"]  # optional: words to spell right
//! ```

use crate::auth::Keys;
use crate::{split_name, Catalog, Known, Setup, CLI};

// voice mode's keys of `[voice]` (listen, the voice, speed…) are below:
// [`VoiceModeKeys`].

/// The speech-to-text wire families:
///   mistral    POST {base}/audio/transcriptions, multipart, context_bias
///   openai     POST {base}/audio/transcriptions, multipart, prompt
///              (OpenAI, Groq and the other compatible servers)
///   elevenlabs POST {base}/speech-to-text, multipart, keyterms
///   deepgram   POST {base}/listen?model=..., the WAV as the body, keyterm
pub const STT_FAMILIES: [&str; 4] = ["mistral", "openai", "elevenlabs", "deepgram"];

/// `[voice]` as written in config.toml.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoiceConfig {
    pub model: Option<String>,
    pub language: Option<String>,
    pub vocabulary: Vec<String>,
}

impl VoiceConfig {
    /// `[roles.voice]` over `[voice]`: its keys win; an empty vocabulary
    /// keeps `[voice]`'s.
    pub fn over(self, base: VoiceConfig) -> VoiceConfig {
        VoiceConfig {
            model: self.model.or(base.model),
            language: self.language.or(base.language),
            vocabulary: if self.vocabulary.is_empty() { base.vocabulary } else { self.vocabulary },
        }
    }

    /// Read `[voice]`; what is wrong becomes a warning.
    pub fn read(t: &toml::Table, warnings: &mut Vec<String>) -> VoiceConfig {
        let mut c = VoiceConfig::default();
        let Some(v) = t.get("voice") else { return c };
        let Some(v) = v.as_table() else {
            warnings.push("config.toml: voice: not a table ([voice] then model = ...)".into());
            return c;
        };
        let mut warn = |w: String| warnings.push(format!("config.toml: voice.{}", w));
        for (k, x) in v {
            let s = x.as_str().map(|s| s.trim().to_string());
            match k.as_str() {
                "model" => match s {
                    Some(m) if !m.is_empty() => c.model = Some(m),
                    _ => warn("model: a \"provider/model\" name".into()),
                },
                "language" => match s {
                    // "" or "auto": detected by the model
                    Some(l) if l.is_empty() || l.eq_ignore_ascii_case("auto") => c.language = None,
                    Some(l) => c.language = Some(l),
                    None => warn("language: a language code (\"fr\", \"en\") or \"auto\"".into()),
                },
                "vocabulary" => match vocabulary(x) {
                    Some(words) => c.vocabulary = words,
                    None => warn("vocabulary: a list of words, [\"bise\", \"config.toml\"]".into()),
                },
                // voice mode's keys: read by [`VoiceModeKeys`], checked here
                // too so a wrong value is a warning wherever the config is
                k if MODE_KEYS.contains(&k) => {
                    if let Err(e) = VoiceModeKeys::default().set(k, x) {
                        warn(format!("{}: {}", k, e))
                    }
                }
                other => warn(format!(
                    "{}: unknown key (model, language, vocabulary, {})",
                    other,
                    MODE_KEYS.join(", ")
                )),
            }
        }
        c
    }
}

// ---- voice mode (docs/voice-mode-plan.md §4.7, design §6) ----
//
// ```toml
// [voice]
// listen = "auto"          # auto (hands-free with headphones, hold on speakers), hands-free, hold
// tts_model = "mistral/voxtral-mini-tts-2603"   # unset: the voice role's provider's
// tts_voice = "…"          # unset: the default voice
// speed = 1.0              # 0.8 to 1.6
// read_aloud = "needs"     # needs (what needs you + what you asked), all, nothing
// sounds = true
// seen_privacy = true      # the first voice mode showed who hears you
// ```

/// Voice mode's keys of `[voice]` (the dictation's are model, language,
/// vocabulary).
pub const MODE_KEYS: [&str; 7] = ["listen", "tts_model", "tts_voice", "speed", "read_aloud", "sounds", "seen_privacy"];

/// How fast the voice talks, 1 = as the model says it.
pub const SPEED_MIN: f32 = 0.8;
pub const SPEED_MAX: f32 = 1.6;

/// When voice mode listens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListenMode {
    /// hands-free with headphones, hold space on speakers
    #[default]
    Auto,
    HandsFree,
    Hold,
}

impl ListenMode {
    pub const ALL: [ListenMode; 3] = [ListenMode::Auto, ListenMode::HandsFree, ListenMode::Hold];

    /// As config.toml writes it.
    pub fn word(self) -> &'static str {
        match self {
            ListenMode::Auto => "auto",
            ListenMode::HandsFree => "hands-free",
            ListenMode::Hold => "hold",
        }
    }

    pub fn of(s: &str) -> Option<ListenMode> {
        let s = s.trim().to_lowercase().replace(['_', ' '], "-");
        ListenMode::ALL.into_iter().find(|m| m.word() == s || (s == "handsfree" && *m == ListenMode::HandsFree))
    }
}

/// What the agent's messages say aloud in voice mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReadAloud {
    /// what needs you + what you asked
    #[default]
    Needs,
    /// everything the agent would write
    All,
    Nothing,
}

impl ReadAloud {
    pub const ALL: [ReadAloud; 3] = [ReadAloud::Needs, ReadAloud::All, ReadAloud::Nothing];

    pub fn word(self) -> &'static str {
        match self {
            ReadAloud::Needs => "needs",
            ReadAloud::All => "all",
            ReadAloud::Nothing => "nothing",
        }
    }

    pub fn of(s: &str) -> Option<ReadAloud> {
        let s = s.trim().to_lowercase();
        ReadAloud::ALL.into_iter().find(|m| m.word() == s)
    }
}

/// Voice mode's keys as config.toml sets them (unset: the defaults).
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceModeKeys {
    pub listen: ListenMode,
    /// "provider/model"; None: the voice role's provider's ([`tts_model`])
    pub tts_model: Option<String>,
    /// None: the TTS's default voice
    pub tts_voice: Option<String>,
    /// [`SPEED_MIN`]..=[`SPEED_MAX`]
    pub speed: f32,
    pub read_aloud: ReadAloud,
    pub sounds: bool,
    pub seen_privacy: bool,
}

impl Default for VoiceModeKeys {
    fn default() -> Self {
        VoiceModeKeys {
            listen: ListenMode::Auto,
            tts_model: None,
            tts_voice: None,
            speed: 1.0,
            read_aloud: ReadAloud::Needs,
            sounds: true,
            seen_privacy: false,
        }
    }
}

impl VoiceModeKeys {
    /// Read voice mode's keys of `[voice]`; a wrong value keeps its
    /// default (the warning is [`VoiceConfig::read`]'s).
    pub fn read(t: &toml::Table) -> VoiceModeKeys {
        let mut c = VoiceModeKeys::default();
        if let Some(v) = t.get("voice").and_then(|v| v.as_table()) {
            for (k, x) in v {
                if MODE_KEYS.contains(&k.as_str()) {
                    let _ = c.set(k, x);
                }
            }
        }
        c
    }

    /// [`VoiceModeKeys::read`] of config.toml's text; not TOML: the
    /// defaults.
    pub fn from_text(config: Option<&str>) -> VoiceModeKeys {
        config.and_then(|t| t.parse::<toml::Table>().ok()).map(|t| VoiceModeKeys::read(&t)).unwrap_or_default()
    }

    /// One key; Err: what it takes.
    pub fn set(&mut self, key: &str, v: &toml::Value) -> Result<(), String> {
        let s = v.as_str().map(|s| s.trim().to_string());
        let b = v.as_bool();
        match key {
            "listen" => self.listen = s.as_deref().and_then(ListenMode::of).ok_or("auto, hands-free or hold")?,
            "read_aloud" => self.read_aloud = s.as_deref().and_then(ReadAloud::of).ok_or("needs, all or nothing")?,
            "tts_model" => match s {
                Some(m) if crate::split_name(&m).is_some() => self.tts_model = Some(m),
                Some(m) if m.is_empty() => self.tts_model = None,
                _ => return Err("a \"provider/model\" name".into()),
            },
            "tts_voice" => match s {
                Some(m) => self.tts_voice = (!m.is_empty()).then_some(m),
                None => return Err("a voice id (a string)".into()),
            },
            "speed" => {
                let n = match v {
                    toml::Value::Float(f) => *f as f32,
                    toml::Value::Integer(i) => *i as f32,
                    _ => f32::NAN,
                };
                if !(SPEED_MIN..=SPEED_MAX).contains(&n) {
                    return Err(format!("a number from {} to {}", SPEED_MIN, SPEED_MAX));
                }
                self.speed = n;
            }
            "sounds" => self.sounds = b.ok_or("true or false")?,
            "seen_privacy" => self.seen_privacy = b.ok_or("true or false")?,
            _ => return Err("unknown key".into()),
        }
        Ok(())
    }

    /// Each key as config.toml writes it (`listen = "auto"`), in
    /// [`MODE_KEYS`]' order; None: unset (the default).
    pub fn written(&self) -> Vec<(&'static str, Option<String>)> {
        let q = crate::toml_string;
        vec![
            ("listen", Some(q(self.listen.word()))),
            ("tts_model", self.tts_model.as_deref().map(q)),
            ("tts_voice", self.tts_voice.as_deref().map(q)),
            ("speed", Some(speed_written(self.speed))),
            ("read_aloud", Some(q(self.read_aloud.word()))),
            ("sounds", Some(self.sounds.to_string())),
            ("seen_privacy", Some(self.seen_privacy.to_string())),
        ]
    }
}

/// A speed as TOML: one decimal, always a float (`1.0`, `1.2`).
pub fn speed_written(speed: f32) -> String {
    format!("{:.1}", speed.clamp(SPEED_MIN, SPEED_MAX))
}

/// A provider's realtime transcription model, its id; None for every
/// provider now: the only one was Voxtral Mini's (retired), so voice mode
/// transcribes each turn with the voice role's batch model (the end of a
/// turn is the VAD's).
pub fn realtime_model(_provider: &str) -> Option<&'static str> {
    None
}

/// A provider's text-to-speech model, its id; None: it does not speak
/// (Mistral only for now).
pub fn tts_model(provider: &str) -> Option<&'static str> {
    match provider {
        "mistral" => Some("voxtral-mini-tts-2603"),
        _ => None,
    }
}

/// The providers that speak, in the order the settings offer them.
pub const TTS_PROVIDERS: [&str; 1] = ["mistral"];

/// A provider call of voice mode resolved, with its key. Debug never
/// shows the key.
#[derive(Clone, PartialEq, Eq)]
pub struct ModeCall {
    /// "provider/id"
    pub name: String,
    pub provider: String,
    /// "Mistral"
    pub provider_name: String,
    /// no trailing '/'
    pub base_url: String,
    pub model: String,
    pub key: String,
}

impl std::fmt::Debug for ModeCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ModeCall {{ name: {:?}, provider: {:?}, base_url: {:?}, model: {:?}, key: <{} bytes> }}",
            self.name,
            self.provider,
            self.base_url,
            self.model,
            self.key.len()
        )
    }
}

impl Setup {
    /// The voice role's provider ("mistral").
    pub fn voice_provider(&self) -> String {
        self.catalog.resolve_stt(&self.voice.model).provider
    }

    /// Voice Realtime on the voice role's provider; None: it has none.
    /// Err: its key is missing (the batch job says the same).
    pub fn realtime_call(&self, keys: &Keys) -> Result<Option<ModeCall>, String> {
        let p = self.voice_provider();
        let Some(id) = realtime_model(&p) else { return Ok(None) };
        self.mode_call(&format!("{}/{}", p, id), "voice mode's listening", keys).map(Some)
    }

    /// The TTS model voice mode speaks with: `tts_model`, else the voice
    /// role's provider's (one key, one company hears you: never another
    /// provider's by default). Err: the one-line reason.
    pub fn tts_name(&self, keys: &VoiceModeKeys) -> Result<String, String> {
        if let Some(m) = &keys.tts_model {
            return Ok(m.clone());
        }
        let p = self.voice_provider();
        match tts_model(&p) {
            Some(id) => Ok(format!("{}/{}", p, id)),
            None => Err(format!(
                "{} can't speak yet: voice mode talks with Mistral (/voice picks its voice)",
                self.catalog.provider(&p).map_or(p.clone(), |x| x.name.clone())
            )),
        }
    }

    /// The TTS call of voice mode, with its key.
    pub fn tts_call(&self, mode: &VoiceModeKeys, keys: &Keys) -> Result<ModeCall, String> {
        let name = self.tts_name(mode)?;
        let (p, _) = split_name(&name).unwrap_or(("", ""));
        if tts_model(p).is_none() {
            let pname = self.catalog.provider(p).map_or(p.to_string(), |x| x.name.clone());
            return Err(format!("voice mode's voice {}: {} can't speak yet (Mistral can)", name, pname));
        }
        self.mode_call(&name, "voice mode's voice", keys)
    }

    /// The small-jobs model's call (the spoken "on it"), with its key
    /// and its chat wire family.
    pub fn small_call(&self, keys: &Keys) -> Result<(ModeCall, String), String> {
        let r = self.catalog.resolve(&self.small_model);
        let call = self.mode_call(&r.name, "the small-jobs model", keys)?;
        Ok((call, r.api))
    }

    /// A provider call by "provider/id", with its key (env > auth.json
    /// > the old .env files, as the chat keys).
    fn mode_call(&self, name: &str, what: &str, keys: &Keys) -> Result<ModeCall, String> {
        let (pid, mid) = split_name(name).ok_or_else(|| format!("{} {}: not a \"provider/model\" name", what, name))?;
        let Some(p) = self.catalog.provider(pid) else {
            return Err(format!("{} {}: unknown provider '{}'", what, name, pid));
        };
        if !p.needs.is_empty() {
            return Err(format!("{} {}: {} is not usable yet ({})", what, name, p.name, p.needs));
        }
        if p.base_url.is_empty() {
            return Err(format!("{} {}: [providers.{}] has no base_url", what, name, pid));
        }
        let key = if p.key_env.is_empty() {
            String::new()
        } else {
            match keys.find(pid, &p.key_env) {
                Some(f) => f.key,
                None => {
                    return Err(format!("{} needs a {} key: set {} or run '{} login {}'", what, p.name, p.key_env, CLI, pid))
                }
            }
        };
        Ok(ModeCall {
            name: name.to_string(),
            provider: pid.to_string(),
            provider_name: p.name.clone(),
            base_url: p.base_url.trim_end_matches('/').to_string(),
            model: mid.to_string(),
            key,
        })
    }
}

#[cfg(test)]
#[path = "voice_tests.rs"]
mod tests;

/// A list of strings, or one comma-separated string.
fn vocabulary(v: &toml::Value) -> Option<Vec<String>> {
    let words: Vec<String> = match v {
        toml::Value::String(s) => s.split(',').map(|w| w.trim().to_string()).collect(),
        toml::Value::Array(a) => a
            .iter()
            .map(|w| w.as_str().map(|s| s.trim().to_string()))
            .collect::<Option<Vec<_>>>()?,
        _ => return None,
    };
    Some(words.into_iter().filter(|w| !w.is_empty()).collect())
}

/// The voice input's choices, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceSetup {
    /// canonical "provider/id"
    pub model: String,
    /// "BISE_VOICE_MODEL", "config" or "default"
    pub from: &'static str,
    /// None: the model detects it
    pub language: Option<String>,
    pub vocabulary: Vec<String>,
}

impl VoiceSetup {
    /// model = BISE_VOICE_MODEL > `[voice] model` > default_voice_model
    pub fn of(c: &Catalog, cfg: VoiceConfig, env: &dyn Fn(&str) -> Option<String>) -> VoiceSetup {
        let (model, from) = match (env("BISE_VOICE_MODEL"), cfg.model) {
            (Some(m), _) => (m, "BISE_VOICE_MODEL"),
            (None, Some(m)) => (m, "config"),
            (None, None) => (c.default_voice_model.clone(), "default"),
        };
        VoiceSetup { model: c.canonical_stt(&model), from, language: cfg.language, vocabulary: cfg.vocabulary }
    }
}

/// A speech-to-text model name resolved (never the key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SttResolved {
    /// "provider/id"
    pub name: String,
    pub provider: String,
    pub id: String,
    /// the provider's STT family; "" = it does not transcribe
    pub api: String,
    pub base_url: String,
    pub key_env: String,
    pub needs: String,
    pub known: Known,
}

impl Catalog {
    /// A bare voice model name goes to the default voice model's provider;
    /// a full one is used as written (a model the provider took away
    /// falls back at the call: [`VoiceJob::fallback`]).
    pub fn canonical_stt(&self, name: &str) -> String {
        let name = name.trim();
        if split_name(name).is_some() {
            name.to_string()
        } else {
            let p = split_name(&self.default_voice_model).map(|(p, _)| p).unwrap_or("mistral");
            format!("{}/{}", p, name)
        }
    }

    /// The model id to try when the provider says it doesn't know `id`
    /// (Mistral took voxtral-transcribe-3 away, 2026-10-19): the
    /// provider's voice pick, else its next listed voice model; None
    /// when it has no other one.
    pub fn stt_fallback(&self, provider: &str, id: &str) -> Option<String> {
        let pick = self.provider(provider).map(|p| p.voice_model.clone()).unwrap_or_default();
        let listed = self.models.iter().filter(|m| m.stt && m.provider == provider).map(|m| m.id.clone());
        std::iter::once(pick).chain(listed).find(|m| !m.is_empty() && m != id)
    }

    /// The providers' lists of what they transcribe now
    /// ([`crate::voice_live`]) over the table: a provider's voice models
    /// it no longer lists leave (pickers, fallback), its voice pick and
    /// the default voice model move to one it lists (the table's first
    /// still listed, else its list's first, added as a voice model). No
    /// list, or an empty one: the table as it is.
    pub fn with_live_stt(mut self, lists: &[(String, Vec<String>)]) -> Catalog {
        for (pid, ids) in lists.iter().filter(|(_, ids)| !ids.is_empty()) {
            let Some(p) = self.providers.iter().position(|p| p.id == *pid && !p.stt.is_empty()) else { continue };
            self.models.retain(|m| !(m.stt && m.provider == *pid && !ids.contains(&m.id)));
            let first = self.models.iter().find(|m| m.stt && m.provider == *pid).map(|m| m.id.clone());
            let first = first.unwrap_or_else(|| {
                self.models.push(crate::Model {
                    provider: pid.clone(),
                    id: ids[0].clone(),
                    api: None,
                    stt: true,
                    replaces: None,
                    caps: Default::default(),
                    source: crate::Source::Builtin,
                });
                ids[0].clone()
            });
            if !ids.contains(&self.providers[p].voice_model) {
                self.providers[p].voice_model = first;
            }
            let pick = format!("{}/{}", pid, self.providers[p].voice_model);
            if split_name(&self.default_voice_model).is_some_and(|(dp, id)| dp == pid && !ids.iter().any(|x| x == id)) {
                self.default_voice_model = pick;
            }
        }
        self
    }

    pub fn resolve_stt(&self, name: &str) -> SttResolved {
        let full = self.canonical_stt(name);
        let (pid, mid) = split_name(&full).unwrap_or(("", full.as_str()));
        let mut r = SttResolved {
            name: full.clone(),
            provider: pid.to_string(),
            id: mid.to_string(),
            api: String::new(),
            base_url: String::new(),
            key_env: String::new(),
            needs: String::new(),
            known: Known::NoProvider,
        };
        if let Some(p) = self.provider(pid) {
            r.api = p.stt.clone();
            r.base_url = p.base_url.clone();
            r.key_env = p.key_env.clone();
            r.needs = p.needs.clone();
            r.known = match self.model(&full) {
                Some(m) if m.stt => Known::Listed,
                _ => Known::Unlisted,
            };
        }
        r
    }

    /// The providers that transcribe, in catalog order.
    pub fn stt_providers(&self) -> impl Iterator<Item = &crate::Provider> {
        self.providers.iter().filter(|p| !p.stt.is_empty())
    }
}

/// Everything one transcription call needs. Debug never shows the key.
#[derive(Clone, PartialEq, Eq)]
pub struct VoiceJob {
    /// "provider/id", for messages
    pub name: String,
    /// the provider's name ("Mistral") and where credit is added ("" =
    /// none): the lines of a failed transcription (BISE-298)
    pub provider_name: String,
    pub billing_url: String,
    /// one of [`STT_FAMILIES`]
    pub api: String,
    /// no trailing '/'
    pub base_url: String,
    /// the model id the API takes
    pub model: String,
    pub key: String,
    pub language: Option<String>,
    pub vocabulary: Vec<String>,
    /// the model id tried once when the provider says it doesn't know
    /// `model` (a 404 / model_not_found): [`Catalog::stt_fallback`]
    pub fallback: Option<String>,
}

impl std::fmt::Debug for VoiceJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VoiceJob {{ name: {:?}, provider_name: {:?}, billing_url: {:?}, api: {:?}, base_url: {:?}, model: {:?}, key: <{} bytes>, language: {:?}, vocabulary: {:?}, fallback: {:?} }}",
            self.name,
            self.provider_name,
            self.billing_url,
            self.api,
            self.base_url,
            self.model,
            self.key.len(),
            self.language,
            self.vocabulary,
            self.fallback
        )
    }
}

impl Setup {
    /// [`Catalog::with_live_stt`], before [`Setup::with_keys`]: a voice
    /// model left to the default follows it; one he wrote stays as
    /// written (a failed take falls back: [`VoiceJob::fallback`]).
    pub fn with_live_stt(mut self, lists: &[(String, Vec<String>)]) -> Setup {
        self.catalog = std::mem::replace(&mut self.catalog, Catalog::empty()).with_live_stt(lists);
        if self.voice.from == "default" {
            self.voice.model = self.catalog.canonical_stt(&self.catalog.default_voice_model);
        }
        self
    }

    /// The voice model's call, with its key (the chat keys' resolution:
    /// env > auth.json > the old .env files). Err: one line saying what
    /// to do.
    pub fn voice_job(&self, keys: &Keys) -> Result<VoiceJob, String> {
        let r = self.catalog.resolve_stt(&self.voice.model);
        if r.known == Known::NoProvider {
            return Err(format!(
                "voice model {}: unknown provider '{}' ('{} models' lists the voice ones)",
                r.name, r.provider, CLI
            ));
        }
        if r.api.is_empty() {
            return Err(format!(
                "voice model {}: {} does not transcribe ('{} models' lists the voice ones)",
                r.name, r.provider, CLI
            ));
        }
        if !r.needs.is_empty() {
            return Err(format!("voice model {}: {} is not usable yet ({})", r.name, r.provider, r.needs));
        }
        if r.base_url.is_empty() {
            return Err(format!("voice model {}: [providers.{}] has no base_url", r.name, r.provider));
        }
        let key = if r.key_env.is_empty() {
            String::new()
        } else {
            match keys.find(&r.provider, &r.key_env) {
                Some(f) => f.key,
                None => {
                    return Err(format!(
                        "voice transcription needs an API key: set {} or run '{} login {}'",
                        r.key_env, CLI, r.provider
                    ))
                }
            }
        };
        let (provider_name, billing_url) = self
            .catalog
            .provider(&r.provider)
            .map(|p| (p.name.clone(), p.billing_url.clone()))
            .unwrap_or_default();
        let fallback = self.catalog.stt_fallback(&r.provider, &r.id);
        Ok(VoiceJob {
            fallback,
            name: r.name,
            provider_name,
            billing_url,
            api: r.api,
            base_url: r.base_url,
            model: r.id,
            key,
            language: self.voice.language.clone(),
            vocabulary: self.voice.vocabulary.clone(),
        })
    }
}
