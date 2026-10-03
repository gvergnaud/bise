//! bise's own model catalog (BISE-142).
//!
//! The built-in list (`models.toml`, compiled in) merged with the user's
//! `config.toml`: `[providers.<id>]` and `[models."<provider>/<model>"]`
//! tables add or override entries key by key. No network. Any
//! `provider/model` name resolves, listed or not (its provider's
//! defaults); a name nothing knows still resolves (to an empty base URL),
//! so a model never stops bise from starting: the provider call says
//! what is wrong.
//!
//! The Bend runtime gets the merged catalog as one file
//! ([`Setup::handoff_toml`], the path in `BISE_MODELS_FILE`); the format
//! and the per-call rule are in docs/research/providers.md §7.

use std::path::{Path, PathBuf};

// the tests run on a temp HOME, never the user's (bise_home::test_home)
bise_home::test_home!();

pub mod auth;
pub mod auth_cli;
pub mod cli;
pub mod config_cli;
pub mod roles;
pub mod voice;

/// The command's name in messages and usages (BISE-165: was `bend-harness`).
pub const CLI: &str = "bise";

/// The built-in list, shipped in the binary.
pub const BUILTIN: &str = include_str!("../models.toml");

/// The wire families a provider may speak.
pub const FAMILIES: [&str; 5] = [
    "openai-chat",
    "anthropic",
    "openai-responses",
    "gemini",
    "bedrock-converse",
];

/// What a model can do, every field known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caps {
    pub context: u64,
    pub max_output: u64,
    pub vision: bool,
    pub reasoning: bool,
    pub tools: bool,
    /// Anthropic family (BISE-146): "adaptive" | "budget" | "none"; ""
    /// = adaptive when the model reasons, else none (core/anthropic.bend)
    pub thinking: String,
    /// Anthropic family: the `anthropic-beta` header; "" = the family's
    /// default list (the foundry proxy's)
    pub betas: String,
    /// the reasoning efforts the model takes (BISE-135), comma-separated;
    /// "" = its family's list ([`Resolved::efforts`])
    pub efforts: String,
    /// the effort it gets when nobody picks one; "" = high
    pub effort: String,
    /// the prompt cache's routing key (BISE-268), sent with the session's
    /// key: the body field that carries it (OpenAI `prompt_cache_key`);
    /// "" = none
    pub cache_key: String,
    /// the same key as an HTTP header (xAI `x-grok-conv-id`); "" = none
    pub cache_header: String,
    /// a gateway in front of the provider: the env variable holding extra
    /// headers, one `Name: value` per line (Claude Code's
    /// `ANTHROPIC_CUSTOM_HEADERS`); "" = none
    pub headers_env: String,
    /// the command whose stdout is the key, run before every call
    /// (Claude Code's `apiKeyHelper`: a short-lived token); "" = none,
    /// the key comes from `key_env`
    pub key_command: String,
    /// how long one read of a streamed reply may wait for data, in
    /// seconds (the head, then each gap between two events): a slow
    /// gateway or local model; 0 = the runtime's 90 s
    /// (runtime/provider-pure.bend stream_wait)
    pub idle_timeout_sec: u64,
}

/// The thinking modes of the Anthropic family.
pub const THINKING: [&str; 3] = ["adaptive", "budget", "none"];

/// The reasoning efforts of the Anthropic family (BISE-135): `none` turns
/// thinking off; the others are output_config.effort (adaptive) or the
/// size of the thinking budget (budget).
pub const ANTHROPIC_EFFORTS: [&str; 5] = ["none", "low", "medium", "high", "max"];

/// The reasoning efforts of the OpenAI-compatible families
/// (`reasoning_effort`, sent as is).
pub const CHAT_EFFORTS: [&str; 4] = ["none", "low", "medium", "high"];

/// The effort a reasoning model gets when nobody picks one: today's.
pub const DEFAULT_EFFORT: &str = "high";

/// An effort word: letters, digits, '-' or '_' (a provider may take its
/// own, "minimal", "xhigh"...).
pub fn is_effort_word(w: &str) -> bool {
    !w.is_empty() && w.len() <= 20 && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// "none, low,high" -> ["none", "low", "high"]; empty when a word is not
/// an effort word.
pub fn effort_list(s: &str) -> Vec<String> {
    let l: Vec<String> = s.split(',').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect();
    if l.iter().all(|w| is_effort_word(w)) {
        l
    } else {
        Vec::new()
    }
}

/// The defaults of a model nothing describes.
pub const DEFAULT_CAPS: Caps = Caps {
    context: 128_000,
    max_output: 16_384,
    vision: false,
    reasoning: false,
    tools: true,
    thinking: String::new(),
    betas: String::new(),
    efforts: String::new(),
    effort: String::new(),
    cache_key: String::new(),
    cache_header: String::new(),
    headers_env: String::new(),
    key_command: String::new(),
    idle_timeout_sec: 0,
};

/// Caps as written in a table: a missing field comes from the level below
/// (model -> provider -> [`DEFAULT_CAPS`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartialCaps {
    pub context: Option<u64>,
    pub max_output: Option<u64>,
    pub vision: Option<bool>,
    pub reasoning: Option<bool>,
    pub tools: Option<bool>,
    pub thinking: Option<String>,
    pub betas: Option<String>,
    pub efforts: Option<String>,
    pub effort: Option<String>,
    pub cache_key: Option<String>,
    pub cache_header: Option<String>,
    pub headers_env: Option<String>,
    pub key_command: Option<String>,
    pub idle_timeout_sec: Option<u64>,
    /// prices (BISE-150), in [`Price`]'s unit
    pub input_price: Option<u64>,
    pub output_price: Option<u64>,
    pub cache_read_price: Option<u64>,
    pub cache_write_price: Option<u64>,
}

/// What a model costs, per million tokens, in millionths of a dollar
/// (`input_price = 3.0` in models.toml = 3 USD per million = 3_000_000
/// here: integers keep the catalog `Eq`). None: not known.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Price {
    pub input: Option<u64>,
    pub output: Option<u64>,
    /// a cached input token read back; unset: the input price
    pub cache_read: Option<u64>,
    /// an input token written to the cache; unset: the input price
    pub cache_write: Option<u64>,
}

impl Price {
    /// The cost in USD of one call, None without the input and output
    /// prices. `input` counts every input token, the cached ones
    /// included (runtime/usage-pure.bend): the cached ones are taken
    /// out and billed at their own price.
    pub fn cost(&self, input: u64, output: u64, cache_read: u64, cache_write: u64) -> Option<f64> {
        let (pi, po) = (self.input?, self.output?);
        let pr = self.cache_read.unwrap_or(pi);
        let pw = self.cache_write.unwrap_or(pi);
        let plain = input.saturating_sub(cache_read).saturating_sub(cache_write);
        let micro = |n: u64, p: u64| n as f64 * p as f64;
        let total = micro(plain, pi) + micro(output, po) + micro(cache_read, pr) + micro(cache_write, pw);
        Some(total / 1e12)
    }
}

impl PartialCaps {
    fn over(&self, base: &Caps) -> Caps {
        Caps {
            context: self.context.unwrap_or(base.context),
            max_output: self.max_output.unwrap_or(base.max_output),
            vision: self.vision.unwrap_or(base.vision),
            reasoning: self.reasoning.unwrap_or(base.reasoning),
            tools: self.tools.unwrap_or(base.tools),
            thinking: self.thinking.clone().unwrap_or_else(|| base.thinking.clone()),
            betas: self.betas.clone().unwrap_or_else(|| base.betas.clone()),
            efforts: self.efforts.clone().unwrap_or_else(|| base.efforts.clone()),
            effort: self.effort.clone().unwrap_or_else(|| base.effort.clone()),
            cache_key: self.cache_key.clone().unwrap_or_else(|| base.cache_key.clone()),
            cache_header: self.cache_header.clone().unwrap_or_else(|| base.cache_header.clone()),
            headers_env: self.headers_env.clone().unwrap_or_else(|| base.headers_env.clone()),
            key_command: self.key_command.clone().unwrap_or_else(|| base.key_command.clone()),
            idle_timeout_sec: self.idle_timeout_sec.unwrap_or(base.idle_timeout_sec),
        }
    }
    fn price_over(&self, base: &Price) -> Price {
        Price {
            input: self.input_price.or(base.input),
            output: self.output_price.or(base.output),
            cache_read: self.cache_read_price.or(base.cache_read),
            cache_write: self.cache_write_price.or(base.cache_write),
        }
    }
    fn merge(&mut self, o: &PartialCaps) {
        self.context = o.context.or(self.context);
        self.max_output = o.max_output.or(self.max_output);
        self.vision = o.vision.or(self.vision);
        self.reasoning = o.reasoning.or(self.reasoning);
        self.tools = o.tools.or(self.tools);
        self.thinking = o.thinking.clone().or(self.thinking.take());
        self.betas = o.betas.clone().or(self.betas.take());
        self.efforts = o.efforts.clone().or(self.efforts.take());
        self.effort = o.effort.clone().or(self.effort.take());
        self.cache_key = o.cache_key.clone().or(self.cache_key.take());
        self.cache_header = o.cache_header.clone().or(self.cache_header.take());
        self.headers_env = o.headers_env.clone().or(self.headers_env.take());
        self.key_command = o.key_command.clone().or(self.key_command.take());
        self.idle_timeout_sec = o.idle_timeout_sec.or(self.idle_timeout_sec);
        self.input_price = o.input_price.or(self.input_price);
        self.output_price = o.output_price.or(self.output_price);
        self.cache_read_price = o.cache_read_price.or(self.cache_read_price);
        self.cache_write_price = o.cache_write_price.or(self.cache_write_price);
    }
}

/// Where an entry was last set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Builtin,
    Config,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: String,
    pub name: String,
    /// the wire family, one of [`FAMILIES`]
    pub api: String,
    /// no trailing '/'; "" when unknown
    pub base_url: String,
    /// the env variables that give `base_url` when config.toml does not
    /// (`base_url_env`, comma-separated, the first set wins; foundry:
    /// ANTHROPIC_FOUNDRY_BASE_URL, Claude Code's name); "" = none
    pub base_url_env: String,
    /// where `base_url` came from: "built-in", "config", the env
    /// variable's name; "" = no base URL
    pub base_url_from: String,
    /// "" = no key needed
    pub key_env: String,
    /// "" = usable; else the issue that makes it usable ("BISE-149")
    pub needs: String,
    /// a cheap model of this provider for short labels (BISE-126: the
    /// agents' role lines), its id without the provider; "" = none
    pub small_model: String,
    /// its speech-to-text wire family (BISE-130), one of
    /// [`voice::STT_FAMILIES`]; "" = it does not transcribe
    pub stt: String,
    /// `kind = "stt"`: it only transcribes (no chat models: not in the
    /// chat list, not in the runtime's hand-off)
    pub stt_only: bool,
    /// `kind = "decisions"`: it only answers typed questions (TypeSafe's
    /// Jev, the checker of auto mode): no chat models either
    pub decides: bool,
    /// the first run's key step (BISE-266): what it is for, in a few
    /// words ("claude, by the people who make it"); "" = none
    pub hint: String,
    /// the page where a key is created (a clickable link); "" = none
    pub keys_url: String,
    /// where a new account starts, when it is not the keys page; "" = none
    pub signup_url: String,
    /// where credit is added (BISE-282: a key check that says "no
    /// credit" links it); "" = none
    pub billing_url: String,
    /// the model a new user starts with, its id without the provider;
    /// "" = none (the key step then asks for a name)
    pub model: String,
    /// `hidden = true`: not offered to a new user (a private proxy); it
    /// still works when a config or a key picks it
    pub hidden: bool,
    /// its recommended voice model (the voice screen's pick, BISE-298),
    /// its id without the provider; "" = its first listed one
    pub voice_model: String,
    /// the defaults of its models
    pub caps: PartialCaps,
    pub source: Source,
}

impl Provider {
    /// The configured key source. Reading it does not run the command.
    pub fn key_command(&self) -> &str {
        self.caps.key_command.as_deref().unwrap_or("")
    }

    /// It runs chats: not voice only (`stt`), not decisions only.
    pub fn chats(&self) -> bool {
        !self.stt_only && !self.decides
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub provider: String,
    pub id: String,
    /// its own wire family when it differs from its provider's (a
    /// Responses-only model of a chat provider); None: the provider's
    pub api: Option<String>,
    /// `kind = "stt"`: a speech-to-text model (BISE-130), not a chat one
    pub stt: bool,
    pub caps: PartialCaps,
    pub source: Source,
}

impl Model {
    /// "provider/id"
    pub fn name(&self) -> String {
        format!("{}/{}", self.provider, self.id)
    }
}

/// How much the catalog knew about a resolved name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Known {
    /// the model is listed
    Listed,
    /// the provider is known, the model is not: its defaults
    Unlisted,
    /// neither: no base URL, the call will fail with a clear message
    NoProvider,
}

/// A model name resolved to everything a provider call needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// "provider/id"
    pub name: String,
    pub provider: String,
    pub id: String,
    pub api: String,
    pub base_url: String,
    pub key_env: String,
    pub needs: String,
    pub caps: Caps,
    /// its prices (BISE-150): the model's, else its provider's
    pub price: Price,
    pub known: Known,
}

impl Resolved {
    /// The reasoning efforts the model takes (BISE-135), in order: its
    /// `efforts` key, else its family's ([`ANTHROPIC_EFFORTS`],
    /// [`CHAT_EFFORTS`]); none for a model that does not reason (an
    /// Anthropic one with `thinking = "none"` included). The Bend
    /// runtime computes the same (runtime/provider-pure.bend `efforts`).
    pub fn efforts(&self) -> Vec<String> {
        if !self.caps.reasoning || (self.api == "anthropic" && self.caps.thinking == "none") {
            return Vec::new();
        }
        let own = effort_list(&self.caps.efforts);
        if !own.is_empty() {
            return own;
        }
        let family: &[&str] = if self.api == "anthropic" { &ANTHROPIC_EFFORTS } else { &CHAT_EFFORTS };
        family.iter().map(|w| w.to_string()).collect()
    }

    /// The effort nobody picked: its `effort` key when it takes it, else
    /// high, else the last of its list; "" when it takes none.
    pub fn default_effort(&self) -> String {
        let l = self.efforts();
        let has = |w: &str| l.iter().any(|x| x == w);
        if has(&self.caps.effort) {
            self.caps.effort.clone()
        } else if has(DEFAULT_EFFORT) {
            DEFAULT_EFFORT.into()
        } else {
            l.last().cloned().unwrap_or_default()
        }
    }

    /// The effort a call runs with when `asked` is asked ("" = nothing
    /// asked): `asked` when the model takes it, else its default.
    pub fn effort_for(&self, asked: &str) -> String {
        let asked = asked.trim();
        if self.efforts().iter().any(|w| w == asked) {
            asked.to_string()
        } else {
            self.default_effort()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub providers: Vec<Provider>,
    pub models: Vec<Model>,
    /// (alias, "provider/id"), in file order
    pub aliases: Vec<(String, String)>,
    /// "provider/id" when no model is configured
    pub default_model: String,
    /// the speech-to-text model when `[voice] model` is not set (BISE-130)
    pub default_voice_model: String,
    /// what was ignored while reading, one line each
    pub warnings: Vec<String>,
}

/// Split "provider/id" at the first '/' (ids may hold '/').
pub fn split_name(name: &str) -> Option<(&str, &str)> {
    let (p, m) = name.split_once('/')?;
    (!p.is_empty() && !m.is_empty()).then_some((p, m))
}

/// A name without a provider, as the config had before BISE-142: the
/// old table sent `claude*` to the foundry proxy and anything else to
/// Mistral.
pub fn legacy_name(name: &str) -> String {
    if name.to_ascii_lowercase().starts_with("claude") {
        format!("foundry/{}", name)
    } else {
        format!("mistral/{}", name)
    }
}

impl Catalog {
    fn empty() -> Catalog {
        Catalog {
            providers: Vec::new(),
            models: Vec::new(),
            aliases: Vec::new(),
            default_model: String::new(),
            default_voice_model: String::new(),
            warnings: Vec::new(),
        }
    }

    /// The built-in list alone.
    pub fn builtin() -> Catalog {
        let mut c = Catalog::empty();
        match BUILTIN.parse::<toml::Table>() {
            Ok(t) => c.apply(&t, Source::Builtin, "models.toml"),
            // a broken models.toml is a build bug: the tests catch it
            Err(e) => c.warnings.push(format!("models.toml: {}", e)),
        }
        c
    }

    pub fn provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn model(&self, name: &str) -> Option<&Model> {
        let (p, m) = split_name(name)?;
        self.models.iter().find(|x| x.provider == p && x.id == m)
    }

    /// The full name a config value means: an alias, then a
    /// "provider/id", then the legacy rule for a bare name.
    pub fn canonical(&self, name: &str) -> String {
        let name = name.trim();
        // no model at all (BISE-266): stays none, never a guess
        if name.is_empty() {
            return String::new();
        }
        if let Some((_, to)) = self.aliases.iter().find(|(a, _)| a == name) {
            return to.clone();
        }
        if split_name(name).is_some() {
            name.to_string()
        } else {
            legacy_name(name)
        }
    }

    /// The cheap model of `name`'s provider (its `small_model`), as
    /// "provider/id"; None when it has none.
    pub fn small_of(&self, name: &str) -> Option<String> {
        let full = self.canonical(name);
        let (pid, _) = split_name(&full)?;
        let p = self.provider(pid)?;
        (!p.small_model.is_empty()).then(|| format!("{}/{}", p.id, p.small_model))
    }

    /// A provider's base URL from its `base_url_env` variables, the
    /// first one set: config.toml's `base_url` wins (bise's own file, as
    /// auth.json's key wins over the environment's, BISE-269); the env
    /// wins over the built-in one. Run once the config layer is merged.
    fn base_urls_from_env(&mut self, env: &dyn Fn(&str) -> Option<String>) {
        for p in self.providers.iter_mut().filter(|p| p.base_url_from != "config") {
            let found = p
                .base_url_env
                .split(',')
                .map(str::trim)
                .filter(|k| !k.is_empty())
                .find_map(|k| env(k).map(|v| (k.to_string(), env_base_url(&p.api, &v))).filter(|(_, v)| !v.is_empty()));
            if let Some((k, v)) = found {
                p.base_url = v;
                p.base_url_from = k;
            }
        }
    }

    /// What to do about a provider with no base URL, in one line: its
    /// env variable when it has one, else the config.toml line. Never
    /// "check your network": nothing was called.
    pub fn no_base_url(&self, provider: &str) -> String {
        let env = self.provider(provider).map(|p| p.base_url_env.as_str()).unwrap_or("");
        let first = env.split(',').map(str::trim).find(|k| !k.is_empty());
        match first {
            Some(k) => format!(
                "{} has no base URL: set {} (in your shell or ~/.vibe/.env), or base_url under [providers.{}] in ~/.bise/config.toml",
                provider, k, provider
            ),
            None => format!("{} has no base URL: set base_url under [providers.{}] in ~/.bise/config.toml", provider, provider),
        }
    }

    /// The caps of a provider's unlisted models.
    pub fn provider_caps(&self, p: &Provider) -> Caps {
        p.caps.over(&DEFAULT_CAPS)
    }

    /// Resolve any name. Never fails: see [`Known`].
    pub fn resolve(&self, name: &str) -> Resolved {
        let full = self.canonical(name);
        let (pid, mid) = split_name(&full).unwrap_or(("", full.as_str()));
        let (pid, mid) = (pid.to_string(), mid.to_string());
        let Some(p) = self.provider(&pid) else {
            return Resolved {
                name: full.clone(),
                provider: pid,
                id: mid,
                api: "openai-chat".into(),
                base_url: String::new(),
                key_env: String::new(),
                needs: String::new(),
                caps: DEFAULT_CAPS,
                price: Price::default(),
                known: Known::NoProvider,
            };
        };
        let base = self.provider_caps(p);
        let base_price = p.caps.price_over(&Price::default());
        let (caps, price, api, known) = match self.models.iter().find(|m| m.provider == pid && m.id == mid) {
            Some(m) => (
                m.caps.over(&base),
                m.caps.price_over(&base_price),
                m.api.clone().unwrap_or_else(|| p.api.clone()),
                Known::Listed,
            ),
            None => (base, base_price, p.api.clone(), Known::Unlisted),
        };
        Resolved {
            name: full.clone(),
            provider: pid,
            id: mid,
            api,
            base_url: p.base_url.clone(),
            key_env: p.key_env.clone(),
            needs: p.needs.clone(),
            caps,
            price,
            known,
        }
    }

    /// The context window of a model, in tokens (listed or its
    /// provider's default; the default one when nothing knows it).
    pub fn context_window(&self, name: &str) -> u64 {
        self.resolve(name).caps.context
    }

    /// The compaction threshold a model gets when neither
    /// BEND_THRESHOLD nor config `compaction_threshold` says (BISE-150):
    /// 80 % of its context window, also the most any value gets
    /// (BISE-300, [`compaction_threshold`]). The Bend runtime computes
    /// the same (runtime/provider-pure.bend, `threshold_of`).
    pub fn default_threshold(&self, name: &str) -> u64 {
        threshold_of(self.context_window(name))
    }

    /// Merge one TOML layer (the built-in list, then the config).
    fn apply(&mut self, t: &toml::Table, src: Source, file: &str) {
        let mut warnings: Vec<String> = Vec::new();
        let mut warn = |w: String| warnings.push(format!("{}: {}", file, w));
        let mut providers: Vec<(String, Provider)> = Vec::new();
        let mut models: Vec<Model> = Vec::new();
        let mut aliases: Vec<(String, String)> = Vec::new();
        let mut default_model = None;
        let mut default_voice_model = None;
        for (k, v) in t {
            match k.as_str() {
                "default_voice_model" => match v.as_str() {
                    Some(s) if split_name(s.trim()).is_some() => default_voice_model = Some(s.trim().to_string()),
                    _ => warn("default_voice_model: not a \"provider/model\" name".into()),
                },
                "default_model" => match v.as_str() {
                    Some(s) if !s.trim().is_empty() => default_model = Some(s.trim().to_string()),
                    _ => warn("default_model: not a model name".into()),
                },
                "aliases" => match v.as_table() {
                    Some(a) => {
                        for (from, to) in a {
                            match to.as_str() {
                                Some(to) if split_name(to).is_some() => {
                                    aliases.push((from.clone(), to.to_string()))
                                }
                                _ => warn(format!("aliases.\"{}\": not a \"provider/model\" name", from)),
                            }
                        }
                    }
                    None => warn("aliases: not a table".into()),
                },
                "providers" => match v.as_table() {
                    Some(ps) => {
                        for (id, pv) in ps {
                            let Some(pt) = pv.as_table() else {
                                warn(format!("providers.{}: not a table", id));
                                continue;
                            };
                            let where_ = format!("providers.{}", id);
                            let mut p = self.provider(id).cloned().unwrap_or(Provider {
                                id: id.clone(),
                                name: id.clone(),
                                api: "openai-chat".into(),
                                base_url: String::new(),
                                base_url_env: String::new(),
                                base_url_from: String::new(),
                                key_env: String::new(),
                                needs: String::new(),
                                small_model: String::new(),
                                stt: String::new(),
                                stt_only: false,
                                decides: false,
                                hint: String::new(),
                                keys_url: String::new(),
                                signup_url: String::new(),
                                billing_url: String::new(),
                                voice_model: String::new(),
                                model: String::new(),
                                hidden: false,
                                caps: PartialCaps::default(),
                                source: src,
                            });
                            p.source = src;
                            let mut caps = PartialCaps::default();
                            for (fk, fv) in pt {
                                let s = || fv.as_str().map(|x| x.trim().to_string());
                                match fk.as_str() {
                                    "name" => set_str(&mut p.name, s(), &where_, fk, &mut warn),
                                    "api" => match s() {
                                        Some(a) if FAMILIES.contains(&a.as_str()) => p.api = a,
                                        _ => warn(format!(
                                            "{}.api: one of {}",
                                            where_,
                                            FAMILIES.join(", ")
                                        )),
                                    },
                                    // an empty one says nothing: it never
                                    // hides the env's or the built-in URL
                                    "base_url" => match s().map(|u| u.trim_end_matches('/').to_string()) {
                                        Some(u) if u.is_empty() => {}
                                        Some(u) => {
                                            p.base_url = u;
                                            p.base_url_from = match src {
                                                Source::Builtin => "built-in".into(),
                                                Source::Config => "config".into(),
                                            };
                                        }
                                        None => warn(format!("{}.base_url: not a string", where_)),
                                    },
                                    "base_url_env" => set_str(&mut p.base_url_env, s(), &where_, fk, &mut warn),
                                    "key_env" => set_str(&mut p.key_env, s(), &where_, fk, &mut warn),
                                    "needs" => set_str(&mut p.needs, s(), &where_, fk, &mut warn),
                                    "small_model" => set_str(&mut p.small_model, s(), &where_, fk, &mut warn),
                                    "hint" => set_str(&mut p.hint, s(), &where_, fk, &mut warn),
                                    "keys_url" => set_str(&mut p.keys_url, s(), &where_, fk, &mut warn),
                                    "signup_url" => set_str(&mut p.signup_url, s(), &where_, fk, &mut warn),
                                    "billing_url" => set_str(&mut p.billing_url, s(), &where_, fk, &mut warn),
                                    "model" => set_str(&mut p.model, s(), &where_, fk, &mut warn),
                                    "voice_model" => set_str(&mut p.voice_model, s(), &where_, fk, &mut warn),
                                    "hidden" => match fv.as_bool() {
                                        Some(b) => p.hidden = b,
                                        None => warn(format!("{}.hidden: true or false", where_)),
                                    },
                                    "stt" => match s() {
                                        Some(a) if a.is_empty() || voice::STT_FAMILIES.contains(&a.as_str()) => p.stt = a,
                                        _ => warn(format!(
                                            "{}.stt: one of {}",
                                            where_,
                                            voice::STT_FAMILIES.join(", ")
                                        )),
                                    },
                                    "kind" => match kind_of(fv) {
                                        Some(k) => {
                                            p.stt_only = k == Kind::Stt;
                                            p.decides = k == Kind::Decisions;
                                        }
                                        None => warn(format!("{}.kind: \"chat\", \"stt\" or \"decisions\"", where_)),
                                    },
                                    _ => cap_field(&mut caps, fk, fv, &where_, &mut warn),
                                }
                            }
                            p.caps.merge(&caps);
                            match providers.iter_mut().find(|(i, _)| i == id) {
                                Some(slot) => slot.1 = p,
                                None => providers.push((id.clone(), p)),
                            }
                        }
                    }
                    None => warn("providers: not a table".into()),
                },
                "models" => match v.as_table() {
                    Some(ms) => {
                        for (name, mv) in ms {
                            let where_ = format!("models.\"{}\"", name);
                            let Some((pid, mid)) = split_name(name) else {
                                warn(format!("{}: the name must be \"provider/model\"", where_));
                                continue;
                            };
                            let Some(mt) = mv.as_table() else {
                                warn(format!("{}: not a table", where_));
                                continue;
                            };
                            let mut caps = PartialCaps::default();
                            let mut api = None;
                            let mut stt = None;
                            for (fk, fv) in mt {
                                if fk == "api" {
                                    match fv.as_str().map(str::trim) {
                                        Some(a) if FAMILIES.contains(&a) => api = Some(a.to_string()),
                                        _ => warn(format!("{}.api: one of {}", where_, FAMILIES.join(", "))),
                                    }
                                } else if fk == "kind" {
                                    match kind_of(fv) {
                                        Some(k @ (Kind::Chat | Kind::Stt)) => stt = Some(k == Kind::Stt),
                                        _ => warn(format!("{}.kind: \"chat\" or \"stt\"", where_)),
                                    }
                                } else {
                                    cap_field(&mut caps, fk, fv, &where_, &mut warn);
                                }
                            }
                            let mut m = self.model(name).cloned().unwrap_or(Model {
                                provider: pid.to_string(),
                                id: mid.to_string(),
                                api: None,
                                stt: false,
                                caps: PartialCaps::default(),
                                source: src,
                            });
                            m.source = src;
                            m.api = api.or(m.api);
                            m.stt = stt.unwrap_or(m.stt);
                            m.caps.merge(&caps);
                            models.push(m);
                        }
                    }
                    None => warn("models: not a table".into()),
                },
                "provider" if src == Source::Config => {
                    warn("[provider.<id>] is not read: write [providers.<id>]".into())
                }
                "model" if src == Source::Builtin => {}
                _ => {} // the config's other keys (compaction_threshold, bg_after, [voice], ...)
            }
        }
        for (id, p) in providers {
            match self.providers.iter_mut().find(|x| x.id == id) {
                Some(slot) => *slot = p,
                None => self.providers.push(p),
            }
        }
        for m in models {
            match self
                .models
                .iter_mut()
                .find(|x| x.provider == m.provider && x.id == m.id)
            {
                Some(slot) => *slot = m,
                None => self.models.push(m),
            }
        }
        for (a, to) in aliases {
            self.aliases.retain(|(x, _)| *x != a);
            self.aliases.push((a, to));
        }
        if let Some(d) = default_model {
            self.default_model = d;
        }
        if let Some(d) = default_voice_model {
            self.default_voice_model = d;
        }
        self.warnings.extend(warnings);
    }
}

/// `kind = "chat" | "stt"` → Some(is stt).
/// What a provider or a model does (`kind = …`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Chat,
    Stt,
    Decisions,
}

fn kind_of(v: &toml::Value) -> Option<Kind> {
    match v.as_str().map(str::trim) {
        Some("stt") => Some(Kind::Stt),
        Some("chat") => Some(Kind::Chat),
        Some("decisions") => Some(Kind::Decisions),
        _ => None,
    }
}

/// 80 % of a context window, in tokens.
pub fn threshold_of(context: u64) -> u64 {
    context / 5 * 4 + context % 5 * 4 / 5
}

/// A compaction threshold as written (BISE-300), in tokens for a model
/// with this window: a number of tokens ("450000") or a share of the
/// window ("45%", above 100 counts as 100). None when it is no threshold
/// (0, empty, a word, a decimal share): the default applies. The Bend
/// runtime reads the same (runtime/provider-pure.bend `thr_value`).
pub fn threshold_written(v: &str, context: u64) -> Option<u64> {
    fn digits(s: &str) -> Option<u64> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some(s.parse::<u64>().unwrap_or(u64::MAX))
    }
    let v = v.trim();
    let n = match v.strip_suffix('%') {
        Some(p) => {
            let p = digits(p.trim())?.min(100);
            context / 100 * p + context % 100 * p / 100
        }
        None => digits(v)?,
    };
    (n > 0).then_some(n)
}

/// The compaction threshold of a model with this window (BISE-300): the
/// value written (BEND_THRESHOLD, else config `compaction_threshold`),
/// never above 80 % of the window; none or no threshold: that 80 %. A
/// number written for a 1M model stays safe on a 200k one.
pub fn compaction_threshold(written: Option<&str>, context: u64) -> u64 {
    let cap = threshold_of(context);
    written.and_then(|v| threshold_written(v, context)).map_or(cap, |n| n.min(cap))
}

fn set_str(slot: &mut String, v: Option<String>, where_: &str, k: &str, warn: &mut dyn FnMut(String)) {
    match v {
        Some(v) => *slot = v,
        None => warn(format!("{}.{}: not a string", where_, k)),
    }
}

fn cap_field(caps: &mut PartialCaps, k: &str, v: &toml::Value, where_: &str, warn: &mut dyn FnMut(String)) {
    let int = || v.as_integer().filter(|n| *n > 0).map(|n| n as u64);
    let bool_ = || v.as_bool();
    let bad = |what: &str| format!("{}.{}: {}", where_, k, what);
    match k {
        "context" => match int() {
            Some(n) => caps.context = Some(n),
            None => warn(bad("a number of tokens > 0")),
        },
        "max_output" => match int() {
            Some(n) => caps.max_output = Some(n),
            None => warn(bad("a number of tokens > 0")),
        },
        // the runtime caps it at a day (provider-pure.bend idle_ms)
        "idle_timeout_sec" => match int().filter(|n| *n <= 86_400) {
            Some(n) => caps.idle_timeout_sec = Some(n),
            None => warn(bad("a number of seconds, 1 to 86400")),
        },
        "input_price" | "output_price" | "cache_read_price" | "cache_write_price" => {
            // USD per million tokens, stored in millionths of a dollar
            let usd = v.as_float().or_else(|| v.as_integer().map(|n| n as f64));
            match usd.filter(|x| x.is_finite() && *x >= 0.0 && *x < 1e6) {
                Some(x) => {
                    let n = Some((x * 1e6).round() as u64);
                    match k {
                        "input_price" => caps.input_price = n,
                        "output_price" => caps.output_price = n,
                        "cache_read_price" => caps.cache_read_price = n,
                        _ => caps.cache_write_price = n,
                    }
                }
                None => warn(bad("USD per million tokens, a number >= 0")),
            }
        }
        "vision" | "reasoning" | "tools" => match bool_() {
            Some(b) => match k {
                "vision" => caps.vision = Some(b),
                "reasoning" => caps.reasoning = Some(b),
                _ => caps.tools = Some(b),
            },
            None => warn(bad("true or false")),
        },
        "thinking" => match v.as_str().map(str::trim) {
            Some(t) if THINKING.contains(&t) => caps.thinking = Some(t.to_string()),
            _ => warn(bad(&format!("one of {}", THINKING.join(", ")))),
        },
        "betas" => match v.as_str() {
            Some(b) => caps.betas = Some(b.trim().to_string()),
            None => warn(bad("a string (comma-separated anthropic-beta flags)")),
        },
        "efforts" => match v.as_str().map(effort_list) {
            Some(l) if !l.is_empty() => caps.efforts = Some(l.join(",")),
            _ => warn(bad("a string of effort words, comma-separated (\"none,low,medium,high\")")),
        },
        "effort" => match v.as_str().map(str::trim) {
            Some(e) if is_effort_word(e) => caps.effort = Some(e.to_string()),
            _ => warn(bad("an effort word (low, medium, high, ...)")),
        },
        // BISE-268: a field or header name (letters, digits, - and _)
        "cache_key" | "cache_header" => match v.as_str().map(str::trim) {
            Some(n) if n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') => {
                if k == "cache_key" {
                    caps.cache_key = Some(n.to_string())
                } else {
                    caps.cache_header = Some(n.to_string())
                }
            }
            _ => warn(bad("a field or header name (letters, digits, - and _)")),
        },
        // a gateway: the variable of its extra headers, the command of its key
        "headers_env" => match v.as_str().map(str::trim) {
            Some(n) if n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => caps.headers_env = Some(n.to_string()),
            _ => warn(bad("an env variable name (letters, digits and _)")),
        },
        // the runtime's reader (core/config.bend) takes a value as written,
        // escapes included: no '"' or '\\' (single quotes work)
        "key_command" => match v.as_str().map(str::trim) {
            Some(c) if !c.contains(['\n', '"', '\\']) => caps.key_command = Some(c.to_string()),
            _ => warn(bad("a shell command on one line printing the key, with no \" or \\ (use single quotes)")),
        },
        _ => warn(format!("{}: unknown key {}", where_, k)),
    }
}

// ---- the config: the catalog + the models the agents use ----

/// The catalog merged with the config, and the two model choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setup {
    pub catalog: Catalog,
    /// the main agent's model, canonical "provider/id"
    pub model: String,
    /// the sub-agents' model, canonical; = model when unset
    pub agent_model: String,
    /// where each choice came from ("BISE_MODEL", "config", "default", ...)
    pub model_from: &'static str,
    pub agent_model_from: &'static str,
    /// the model of short labels (BISE-126), canonical:
    /// BISE_SMALL_MODEL > config `small_model` > the agents' provider's
    /// `small_model` > agent_model
    pub small_model: String,
    pub small_model_from: &'static str,
    /// the reasoning effort asked for (BISE-135): config
    /// `reasoning_effort`; "" = the model's default
    pub effort: String,
    /// the sub-agents': config `agent_reasoning_effort`, else `effort`
    pub agent_effort: String,
    /// the voice input's choices (BISE-130): `[voice]` in config.toml
    pub voice: voice::VoiceSetup,
    /// the checker's model (approvals, design §4.2): BISE_CLASSIFY_MODEL,
    /// else `[roles] classify` ("off" included), else small_model with
    /// "default": with the keys, [`roles::checker_default`] picks Jev first
    pub classify_model: String,
    pub classify_model_from: &'static str,
    /// config `compaction_threshold` as written (BISE-300): "450000" or
    /// "45%"; None when unset. [`compaction_threshold`] resolves it.
    pub compaction_threshold: Option<String>,
}

/// config.toml's text with its top-level `model` set to `model`
/// (BISE-266: the first run's pick): the line replaced where it is, else
/// added before the first table; the rest of the file as it was.
pub fn with_model(config: &str, model: &str) -> String {
    with_key(config, "model", &toml_string(model))
}

/// A TOML basic string: `"…"`, `\` and `"` escaped.
pub fn toml_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// config.toml's text with its top-level `key` set to `value` (a TOML
/// value as written: `"x"`, `["a", "b"]`), like [`with_model`]
/// (`bise config set`, BISE-273).
pub fn with_key(config: &str, key: &str, value: &str) -> String {
    let line = format!("{} = {}", key, value);
    let mut out: Vec<String> = Vec::new();
    let (mut done, mut in_table) = (false, false);
    for l in config.lines() {
        let t = l.trim();
        if t.starts_with('[') && !in_table {
            in_table = true;
            if !done {
                out.push(line.clone());
                if out.len() > 1 || !t.is_empty() {
                    out.push(String::new());
                }
                done = true;
            }
        }
        let is_model = !in_table && t.split_once('=').is_some_and(|(k, _)| k.trim() == key);
        if is_model {
            if !done {
                out.push(line.clone());
                done = true;
            }
            continue;
        }
        out.push(l.to_string());
    }
    if !done {
        out.push(line);
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// config `compaction_threshold` as written (BISE-300), and what is wrong
/// with it: not a number nor a share, or the old key `threshold` (no
/// longer read).
fn read_threshold(t: &toml::Table, warnings: &mut Vec<String>) -> Option<String> {
    const WHAT: &str = "a number of tokens (450000) or a share of the model's window (\"45%\")";
    if t.contains_key("threshold") {
        warnings.push("config.toml: threshold is no longer read: rename it compaction_threshold".into());
    }
    let v = match t.get("compaction_threshold")? {
        toml::Value::Integer(n) => n.to_string(),
        toml::Value::String(s) => s.trim().to_string(),
        _ => {
            warnings.push(format!("config.toml: compaction_threshold: {}", WHAT));
            return None;
        }
    };
    if threshold_written(&v, 1_000_000).is_none() {
        warnings.push(format!("config.toml: compaction_threshold: {} (80% of the window applies)", WHAT));
    }
    Some(v)
}

/// A string key of the config, even when the file is not valid TOML (the
/// Bend reader accepts bare words): `key = value # comment`.
fn loose_key(text: &str, key: &str) -> Option<String> {
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            return None; // top-level keys only
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        if k.trim() != key {
            continue;
        }
        let v = v.trim();
        let v = match v.strip_prefix('"') {
            Some(rest) => rest.split('"').next().unwrap_or(""),
            None => v.split('#').next().unwrap_or("").trim(),
        };
        return (!v.is_empty()).then(|| v.to_string());
    }
    None
}

impl Setup {
    /// Precedence on each key: env > config > default.
    ///   model       = BISE_MODEL > BEND_MODEL > config `model` > default_model
    ///   agent_model = BISE_AGENT_MODEL > config `agent_model` > model
    /// `config` is the text of config.toml (None: no file). A broken file
    /// is a warning: its tables are ignored, `model`/`agent_model` lines
    /// are still read.
    pub fn from_text(config: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Setup {
        Setup::from_parts(config, env, env)
    }

    /// [`Setup::from_text`] with the base URLs' variables read through
    /// `url_env` (the env, then the .env files: [`Setup::load`]).
    pub fn from_parts(
        config: Option<&str>,
        env: &dyn Fn(&str) -> Option<String>,
        url_env: &dyn Fn(&str) -> Option<String>,
    ) -> Setup {
        let mut catalog = Catalog::builtin();
        let (mut model_cfg, mut agent_cfg, mut small_cfg) = (None, None, None);
        let (mut effort_cfg, mut agent_effort_cfg) = (None, None);
        let mut voice_cfg = voice::VoiceConfig::default();
        let mut classify_cfg: Option<String> = None;
        let mut threshold_cfg: Option<String> = None;
        if let Some(text) = config {
            match text.parse::<toml::Table>() {
                Ok(t) => {
                    catalog.apply(&t, Source::Config, "config.toml");
                    voice_cfg = voice::VoiceConfig::read(&t, &mut catalog.warnings);
                    let s = |k: &str| {
                        t.get(k)
                            .and_then(|v| v.as_str())
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                    };
                    // BISE-298: [roles] first, then the keys it replaces
                    let r = roles::RolesConfig::read(&t, &mut catalog.warnings);
                    let (main, agents, small) = (r.get(roles::MAIN), r.get(roles::AGENTS), r.get(roles::SMALL));
                    model_cfg = main.model.or_else(|| s("model"));
                    agent_cfg = agents.model.or_else(|| s("agent_model"));
                    small_cfg = small.model.or_else(|| s("small_model"));
                    effort_cfg = main.effort.or_else(|| s("reasoning_effort"));
                    agent_effort_cfg = agents.effort.or_else(|| s("agent_reasoning_effort"));
                    classify_cfg = r.get(roles::CLASSIFY).model;
                    threshold_cfg = read_threshold(&t, &mut catalog.warnings);
                    if let Some(v) = r.voice {
                        voice_cfg = voice::VoiceConfig {
                            model: v.model.or(voice_cfg.model),
                            language: v.language.or(voice_cfg.language),
                            vocabulary: if v.vocabulary.is_empty() { voice_cfg.vocabulary } else { v.vocabulary },
                        };
                    }
                }
                Err(e) => {
                    let first = e.to_string().lines().next().unwrap_or("").to_string();
                    catalog.warnings.push(format!(
                        "config.toml is not valid TOML ({}): its [providers] and [models] tables are ignored",
                        first
                    ));
                    model_cfg = loose_key(text, "model");
                    agent_cfg = loose_key(text, "agent_model");
                    small_cfg = loose_key(text, "small_model");
                    effort_cfg = loose_key(text, "reasoning_effort");
                    agent_effort_cfg = loose_key(text, "agent_reasoning_effort");
                    threshold_cfg = loose_key(text, "compaction_threshold");
                }
            }
        }
        catalog.base_urls_from_env(url_env);
        let envv = |k: &str| env(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let (model, model_from) = if let Some(m) = envv("BISE_MODEL") {
            (m, "BISE_MODEL")
        } else if let Some(m) = envv("BEND_MODEL") {
            (m, "BEND_MODEL")
        } else if let Some(m) = model_cfg {
            (m, "config")
        } else {
            let d = catalog.default_model.clone();
            // BISE-266: no built-in default: "none" until a key is checked
            let from = if d.is_empty() { "none" } else { "default" };
            (d, from)
        };
        let model = catalog.canonical(&model);
        let (agent_model, agent_model_from) = if let Some(m) = envv("BISE_AGENT_MODEL") {
            (catalog.canonical(&m), "BISE_AGENT_MODEL")
        } else if let Some(m) = agent_cfg {
            (catalog.canonical(&m), "config")
        } else {
            (model.clone(), "model")
        };
        let (small_model, small_model_from) = if let Some(m) = envv("BISE_SMALL_MODEL") {
            (catalog.canonical(&m), "BISE_SMALL_MODEL")
        } else if let Some(m) = small_cfg {
            (catalog.canonical(&m), "config")
        } else {
            match catalog.small_of(&agent_model) {
                Some(m) => (m, "provider"),
                None => (agent_model.clone(), "agent_model"),
            }
        };
        let voice = voice::VoiceSetup::of(&catalog, voice_cfg, &envv);
        // the checker (approvals): "off" is a choice, not a model name
        let checker = |m: String| if m == roles::CHECKER_OFF { m } else { catalog.canonical(&m) };
        let (classify_model, classify_model_from) = if let Some(m) = envv("BISE_CLASSIFY_MODEL") {
            (checker(m), "BISE_CLASSIFY_MODEL")
        } else if let Some(m) = classify_cfg {
            (checker(m), "config")
        } else {
            // unset: TypeSafe, OpenRouter, the small jobs model, by the
            // keys (roles::checker_default); the last one here
            (small_model.clone(), "default")
        };
        let effort = effort_cfg.unwrap_or_default();
        let agent_effort = agent_effort_cfg.unwrap_or_else(|| effort.clone());
        Setup {
            effort,
            agent_effort,
            catalog,
            model,
            agent_model,
            model_from,
            agent_model_from,
            small_model,
            small_model_from,
            voice,
            classify_model,
            classify_model_from,
            compaction_threshold: threshold_cfg,
        }
    }

    /// The roles bise picks by the keys found, when unset (one key gives
    /// every role a model): the checker ([`roles::checker_default`]: Jev
    /// by TypeSafe or OpenRouter, else the small jobs model) and voice
    /// ([`roles::voice_default`]: the provider that has a key and
    /// listens). A role set in config.toml or an env var stays.
    pub fn with_keys(mut self, keys: &auth::Keys) -> Setup {
        let c = &self.catalog;
        let has_key =
            |id: &str| c.provider(id).is_some_and(|p| p.needs.is_empty() && (p.key_env.is_empty() || keys.for_provider(p).is_some()));
        let ready = |id: &str| c.provider(id).is_some_and(|p| keys.ready(p) || (!p.chats() && has_key(id)));
        let classify = (self.classify_model_from == "default").then(|| roles::checker_default(&self.small_model, &ready));
        let voice = (self.voice.from == "default").then(|| c.canonical_stt(&roles::voice_default(c, &has_key)));
        if let Some(m) = classify {
            self.classify_model = m;
        }
        if let Some(m) = voice {
            self.voice.model = m;
        }
        self
    }

    /// A role's model (canonical "provider/id") and where it came from
    /// ([`roles::Source`]; BISE-298). Unknown role: main's.
    pub fn role_model(&self, id: &str) -> (String, roles::Source) {
        let (m, from) = match id {
            roles::AGENTS => (&self.agent_model, self.agent_model_from),
            roles::SMALL => (&self.small_model, self.small_model_from),
            roles::VOICE => (&self.voice.model, self.voice.from),
            roles::CLASSIFY => (&self.classify_model, self.classify_model_from),
            _ => (&self.model, self.model_from),
        };
        (m.clone(), roles::Source::of(from))
    }

    /// A chat role's effort as config.toml sets it ("" = the model's
    /// default).
    pub fn role_effort(&self, id: &str) -> &str {
        match id {
            roles::MAIN => &self.effort,
            roles::AGENTS => &self.agent_effort,
            _ => "",
        }
    }

    /// Read `config` (a missing file is no config) with the real env.
    /// The base URLs' variables (`base_url_env`) are also read from the
    /// .env files, like the keys (bise's, then ~/.vibe/.env, where Vibe
    /// keeps ANTHROPIC_FOUNDRY_BASE_URL): the TUI, `bise doctor` and the
    /// hub see the same URL.
    pub fn load(config: &Path) -> Setup {
        let text = std::fs::read_to_string(config).ok();
        let home = bise_home::Home::from_env();
        let files = auth::EnvFile::read_all(&home.env_files());
        let real = |k: &str| std::env::var(k).ok();
        let store = auth::Store::read(&home.auth_file()).unwrap_or_default();
        Setup::from_parts(text.as_deref(), &real, &|k| with_files(&real, &files, k))
            .with_keys(&auth::Keys { env: &real, store: &store, files: &files })
    }

    /// The model of a role: "main" or "agent".
    pub fn model_for(&self, role: &str) -> Resolved {
        match role {
            "agent" => self.catalog.resolve(&self.agent_model),
            _ => self.catalog.resolve(&self.model),
        }
    }

    /// What a session of `role` runs with (BISE-135): its own choice
    /// (`/model`, `/reasoning`) over the role's model and effort.
    ///   model  = choice > the role's ([`Setup::model_for`])
    ///   effort = choice > config (agent: `agent_reasoning_effort` >
    ///            `reasoning_effort`) > the model's default; a word the
    ///            model does not take falls to its default
    /// The Bend runtime resolves the same (runtime/provider-pure.bend).
    pub fn in_use(&self, role: &str, c: &Choice) -> InUse {
        let (model, model_from) = if c.model.trim().is_empty() {
            let from = if role == "agent" { self.agent_model_from } else { self.model_from };
            (self.model_for(role), from)
        } else {
            (self.catalog.resolve(&c.model), "session")
        };
        let role_effort = if role == "agent" { &self.agent_effort } else { &self.effort };
        let (asked, from) = if !c.effort.trim().is_empty() {
            (c.effort.trim(), "session")
        } else if !role_effort.is_empty() {
            (role_effort.as_str(), "config")
        } else {
            ("", "model")
        };
        let effort = model.effort_for(asked);
        let effort_from = if effort == asked { from } else { "model" };
        InUse { model, effort, model_from, effort_from }
    }

    /// The hand-off to the Bend runtime: the merged catalog in the
    /// config's own format, so core/config.bend reads it (a key's path
    /// keeps the quotes: `models."openai/gpt-5".context`). A provider has
    /// every key; a model only the keys that differ from its provider's
    /// (the lookup is per key: the model's, else its provider's). The
    /// model choices are not in it: the runtime reads `model` and
    /// `agent_model` from config.toml on each call (providers.md §7).
    pub fn handoff_toml(&self) -> String {
        let c = &self.catalog;
        let mut o = String::new();
        o.push_str("# bise's model catalog, merged with config.toml. Written by bise at start\n");
        o.push_str("# for the Bend runtime (BISE_MODELS_FILE); do not edit: edit config.toml.\n");
        o.push_str("version = 1\n");
        o.push_str(&format!("default_model = {}\n", q(&c.default_model)));
        o.push_str("\n[aliases]\n");
        for (a, to) in &c.aliases {
            o.push_str(&format!("{} = {}\n", q(a), q(to)));
        }
        // the runtime only calls chat models: the speech-to-text entries
        // (BISE-130) stay out, the file is the same as before them
        for p in c.providers.iter().filter(|p| p.chats()) {
            o.push_str(&format!("\n[providers.{}]\n", key(&p.id)));
            o.push_str(&format!("name = {}\n", q(&p.name)));
            o.push_str(&format!("api = {}\n", q(&p.api)));
            // no URL: no line, never `base_url = ""` (the runtime then
            // says the provider has none and how to set it, instead of
            // calling "/messages"); the variables that would give it
            if !p.base_url.is_empty() {
                o.push_str(&format!("base_url = {}\n", q(&p.base_url)));
            } else if !p.base_url_env.is_empty() {
                o.push_str(&format!("base_url_env = {}\n", q(&p.base_url_env)));
            }
            o.push_str(&format!("key_env = {}\n", q(&p.key_env)));
            o.push_str(&format!("needs = {}\n", q(&p.needs)));
            caps_lines(&mut o, &c.provider_caps(p));
        }
        for m in c.models.iter().filter(|m| !m.stt) {
            let r = c.resolve(&m.name());
            o.push_str(&format!("\n[models.{}]\n", q(&m.name())));
            let Some(p) = c.provider(&m.provider) else {
                caps_lines(&mut o, &r.caps); // no provider to fall back to
                continue;
            };
            if r.api != p.api {
                o.push_str(&format!("api = {}\n", q(&r.api)));
            }
            let base = c.provider_caps(p);
            let (a, b) = (&r.caps, &base);
            if a.context != b.context {
                o.push_str(&format!("context = {}\n", a.context));
            }
            if a.max_output != b.max_output {
                o.push_str(&format!("max_output = {}\n", a.max_output));
            }
            if a.idle_timeout_sec != b.idle_timeout_sec {
                o.push_str(&format!("idle_timeout_sec = {}\n", a.idle_timeout_sec));
            }
            for (k, x, y) in [
                ("vision", a.vision, b.vision),
                ("reasoning", a.reasoning, b.reasoning),
                ("tools", a.tools, b.tools),
            ] {
                if x != y {
                    o.push_str(&format!("{} = {}\n", k, x));
                }
            }
            for (k, x, y) in [
                ("thinking", a.thinking.clone(), b.thinking.clone()),
                ("betas", a.betas.clone(), b.betas.clone()),
                ("efforts", a.efforts.clone(), b.efforts.clone()),
                ("effort", a.effort.clone(), b.effort.clone()),
                ("cache_key", a.cache_key.clone(), b.cache_key.clone()),
                ("cache_header", a.cache_header.clone(), b.cache_header.clone()),
                ("headers_env", a.headers_env.clone(), b.headers_env.clone()),
                ("key_command", a.key_command.clone(), b.key_command.clone()),
            ] {
                if x != y {
                    o.push_str(&format!("{} = {}\n", k, q(&x)));
                }
            }
        }
        o
    }

    /// Write the hand-off file atomically (temp file + rename).
    pub fn write_handoff(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp: PathBuf = path.with_extension(format!("toml.{}.tmp", std::process::id()));
        std::fs::write(&tmp, self.handoff_toml())?;
        std::fs::rename(&tmp, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }
}

/// The env var that gives a REPL the path of its session's [`Choice`].
pub const CHOICE_ENV: &str = "BISE_SESSION_CHOICE";

/// A session's own model and effort (BISE-135): what `/model` and
/// `/reasoning` picked, over every config and env value. A small TOML
/// file in the agent's state dir (its REPL gets the path in
/// [`CHOICE_ENV`] and reads it before each call):
/// `model = "provider/id"`, `reasoning_effort = "high"`; "" = not picked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
    pub model: String,
    pub effort: String,
}

impl Choice {
    pub fn parse(text: &str) -> Choice {
        Choice {
            model: loose_key(text, "model").unwrap_or_default(),
            effort: loose_key(text, "reasoning_effort").unwrap_or_default(),
        }
    }

    /// A missing or unreadable file: nothing picked.
    pub fn read(path: &Path) -> Choice {
        std::fs::read_to_string(path).map(|t| Choice::parse(&t)).unwrap_or_default()
    }

    pub fn to_toml(&self) -> String {
        let mut o = String::from("# this session's model (BISE-135: /model, /reasoning); written by bise\n");
        for (k, v) in [("model", &self.model), ("reasoning_effort", &self.effort)] {
            if !v.is_empty() {
                o.push_str(&format!("{} = {}\n", k, q(v)));
            }
        }
        o
    }

    /// Write it atomically (temp file + rename): a REPL reading it
    /// mid-write sees the old choice or the new one.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp: PathBuf = path.with_extension(format!("tmp.{}", std::process::id()));
        std::fs::write(&tmp, self.to_toml())?;
        std::fs::rename(&tmp, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }
}

/// What a session runs with ([`Setup::in_use`]) and where each part
/// came from: "session", "BISE_MODEL", "config", "default", "model"...
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InUse {
    pub model: Resolved,
    /// "" when the model takes no effort
    pub effort: String,
    pub model_from: &'static str,
    pub effort_from: &'static str,
}

/// Set one top-level key of config.toml (BISE-135: `/model ... default`),
/// keeping the rest of the file as it is: the key's line is replaced, or
/// a new line goes before the first table.
pub fn set_config_key(text: &str, key: &str, value: &str) -> String {
    let line = format!("{} = {}", key, q(value));
    let mut out: Vec<String> = Vec::new();
    let mut done = false;
    let mut in_table = false;
    for l in text.lines() {
        let t = l.trim();
        if t.starts_with('[') {
            if !done {
                out.push(line.clone());
                done = true;
            }
            in_table = true;
        }
        let is_key = !in_table && t.split_once('=').is_some_and(|(k, _)| k.trim() == key) && !t.starts_with('#');
        if is_key && !done {
            out.push(line.clone());
            done = true;
        } else if !is_key {
            out.push(l.to_string());
        }
    }
    if !done {
        out.push(line);
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

fn caps_lines(o: &mut String, c: &Caps) {
    o.push_str(&format!(
        "context = {}\nmax_output = {}\nvision = {}\nreasoning = {}\ntools = {}\n",
        c.context, c.max_output, c.vision, c.reasoning, c.tools
    ));
    if !c.thinking.is_empty() {
        o.push_str(&format!("thinking = {}\n", q(&c.thinking)));
    }
    if !c.betas.is_empty() {
        o.push_str(&format!("betas = {}\n", q(&c.betas)));
    }
    if !c.efforts.is_empty() {
        o.push_str(&format!("efforts = {}\n", q(&c.efforts)));
    }
    if !c.effort.is_empty() {
        o.push_str(&format!("effort = {}\n", q(&c.effort)));
    }
    if !c.cache_key.is_empty() {
        o.push_str(&format!("cache_key = {}\n", q(&c.cache_key)));
    }
    if !c.cache_header.is_empty() {
        o.push_str(&format!("cache_header = {}\n", q(&c.cache_header)));
    }
    if !c.headers_env.is_empty() {
        o.push_str(&format!("headers_env = {}\n", q(&c.headers_env)));
    }
    if !c.key_command.is_empty() {
        o.push_str(&format!("key_command = {}\n", q(&c.key_command)));
    }
    if c.idle_timeout_sec > 0 {
        o.push_str(&format!("idle_timeout_sec = {}\n", c.idle_timeout_sec));
    }
}

/// A TOML basic string.
fn q(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// A base URL from an env variable, in the catalog's form (no trailing
/// '/'). The Anthropic family's variables follow its SDK and Claude Code:
/// no `/v1` (ANTHROPIC_FOUNDRY_BASE_URL=https://proxy/anthropic); the
/// catalog's anthropic base_url ends with it (the runtime adds
/// `/messages`), so it is added when missing. "" = blank.
pub fn env_base_url(api: &str, raw: &str) -> String {
    let u = raw.trim().trim_end_matches('/');
    if u.is_empty() {
        return String::new();
    }
    if api == "anthropic" && !u.ends_with("/v1") {
        format!("{}/v1", u)
    } else {
        u.to_string()
    }
}

/// `k` from `env` when set and not blank, else from the first .env file
/// that has it (the keys' order, auth::EnvFile).
pub fn with_files(env: &dyn Fn(&str) -> Option<String>, files: &[auth::EnvFile], k: &str) -> Option<String> {
    env(k)
        .filter(|v| !v.trim().is_empty())
        .or_else(|| files.iter().find_map(|f| f.vars.get(k).filter(|v| !v.trim().is_empty()).cloned()))
}

/// A bare key when it can be one, else quoted.
fn key(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        s.to_string()
    } else {
        q(s)
    }
}

/// Write the hand-off where the runtime will find it and return its
/// path: `<cache_dir>/models.toml`, else the temp dir. None when neither
/// is writable (the runtime then keeps its old table). `env` gives the
/// providers' `base_url_env` variables (the hub: its environment, then
/// the .env files read again, so an edit reaches the next call).
pub fn export_handoff(config: &Path, cache_dir: &Path, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let text = std::fs::read_to_string(config).ok();
    let setup = Setup::from_text(text.as_deref(), env);
    let fallback = std::env::temp_dir().join(format!("bise-models-{}.toml", std::process::id()));
    [cache_dir.join("models.toml"), fallback]
        .into_iter()
        .find(|p| setup.write_handoff(p).is_ok())
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod auth_tests;
#[cfg(test)]
#[path = "one_key_tests.rs"]
mod one_key_tests;
