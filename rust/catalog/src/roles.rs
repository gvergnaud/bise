//! Model roles (BISE-298): which model does what. One table, [`ROLES`]:
//! a new role is one row and its words. Each role has a model
//! ("provider/id") and, for a chat role, a reasoning effort; unset, it
//! follows another role or the catalog.
//!
//! config.toml (a role is a string, or a table with its settings):
//!
//! ```toml
//! [roles]
//! main = "anthropic/claude-opus-5-5"   # your team lead
//! agents = "openai/gpt-6-luna"          # the agents main starts (unset: main's)
//! small = "mistral/mistral-small-latest"  # small jobs: titles, summaries
//! voice = "mistral/voxtral-mini-latest" # listens when you talk (ctrl+r)
//!
//! [roles.main]                          # the table form, instead of the line
//! model = "anthropic/claude-opus-5-5"
//! effort = "high"
//! ```
//!
//! Each role, first found wins: its env var(s) > `[roles]` > its old key
//! (`model`, `agent_model`, `small_model`, `[voice] model`; efforts
//! `reasoning_effort`, `agent_reasoning_effort`) > its fallback. The old
//! keys keep working; bise writes the new shape ([`with_role`]) and drops
//! the old key it replaces, the rest of the file as it was.

use crate::voice::VoiceConfig;

/// What a role's model does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// a chat model (the catalog's chat entries)
    Chat,
    /// a speech-to-text model (`kind = "stt"`)
    Voice,
}

/// One role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Role {
    /// its key under `[roles]`
    pub id: &'static str,
    /// the user's word for it (screens, `/provider`'s tags, doctor)
    pub name: &'static str,
    /// what it does: always shown next to [`Role::name`]
    pub about: &'static str,
    pub kind: Kind,
    /// the env vars that set it, the first set wins
    pub env: &'static [&'static str],
    /// the key it had before roles ("" none; "voice.model": a key of a
    /// table)
    pub old_key: &'static str,
    /// the old key of its effort ("" none)
    pub old_effort: &'static str,
    /// in the screens (a declared role waits for its feature)
    pub shown: bool,
}

pub const MAIN: &str = "main";
pub const AGENTS: &str = "agents";
pub const SMALL: &str = "small";
pub const VOICE: &str = "voice";
pub const CLASSIFY: &str = "classify";
/// The checker's model when it is off (`[roles] classify = "off"`).
pub const CHECKER_OFF: &str = "off";
/// Jev through TypeSafe's API, and through OpenRouter (design §4.2).
pub const JEV_TYPESAFE: &str = "typesafe/jev-1.13";
pub const JEV_OPENROUTER: &str = "openrouter/typesafe/jev-1.13";

/// A Jev model (`typesafe/<id>`, `openrouter/typesafe/<id>`): the
/// provider that serves it and its id on that provider's System One API
/// (TypeSafe names its versions `jev-1.13.0`, OpenRouter
/// `typesafe/jev-1.13`). None: not Jev.
pub fn jev_of(model: &str) -> Option<(&'static str, String)> {
    if let Some(id) = model.strip_prefix("typesafe/") {
        let wire = if id.starts_with("jev-") && id.matches('.').count() == 1 { format!("{}.0", id) } else { id.to_string() };
        return Some(("typesafe", wire));
    }
    model.strip_prefix("openrouter/typesafe/").map(|id| ("openrouter", format!("typesafe/{}", id)))
}

/// The checker's model when the role is unset: Jev through TypeSafe when
/// its key is ready, else through OpenRouter when that key is, else the
/// small jobs model (`ready`: a provider id has its key).
pub fn checker_default(small: &str, ready: &dyn Fn(&str) -> bool) -> String {
    if ready("typesafe") {
        JEV_TYPESAFE.into()
    } else if ready("openrouter") {
        JEV_OPENROUTER.into()
    } else {
        small.into()
    }
}

/// The voice model when the role is unset (one key, every role): the
/// catalog's `default_voice_model` when its provider has a key, else the
/// voice pick of the first provider that listens and has one (an OpenAI
/// key alone: OpenAI's), else the default (voice stays off until a key:
/// `/models` says so). `has_key`: a provider id has its key.
pub fn voice_default(c: &crate::Catalog, has_key: &dyn Fn(&str) -> bool) -> String {
    let d = c.default_voice_model.clone();
    if crate::split_name(&d).is_some_and(|(p, _)| has_key(p)) {
        return d;
    }
    c.providers
        .iter()
        .find(|p| !p.stt.is_empty() && !p.voice_model.is_empty() && !p.hidden && p.needs.is_empty() && has_key(&p.id))
        .map(|p| format!("{}/{}", p.id, p.voice_model))
        .unwrap_or(d)
}

/// What a model of `pid` costs, for picking the cheapest: its own prices,
/// else its twin's under `twin` (a ChatGPT plan model has none: the plan
/// pays; the same id at `openai` says which is cheaper). (output, input)
/// per million tokens; unknown = the most.
fn price_rank(c: &crate::Catalog, pid: &str, twin: &str, id: &str) -> (u64, u64) {
    let mut p = c.resolve(&format!("{}/{}", pid, id)).price;
    if p.output.is_none() && !twin.is_empty() {
        p = c.resolve(&format!("{}/{}", twin, id)).price;
    }
    (p.output.unwrap_or(u64::MAX), p.input.unwrap_or(u64::MAX))
}

/// One login, every role (the ChatGPT plan alone): main and the small
/// jobs model of provider `pid` among `listed` (the account's own list,
/// the ids without the provider; empty = the catalog's entries of
/// `pid`). main: the provider's pick when listed, else the first one;
/// small: the cheapest one listed (prices of `pid`, else of `twin`, the
/// pay-per-token provider with the same ids: "openai" for "chatgpt").
/// None when nothing is listed. The agents follow main, the checker the
/// small model ([`checker_default`]).
pub fn one_login_defaults(c: &crate::Catalog, pid: &str, twin: &str, listed: &[String]) -> Option<(String, String)> {
    let p = c.provider(pid)?;
    let ids: Vec<String> = if listed.is_empty() {
        c.models.iter().filter(|m| m.provider == pid && !m.stt).map(|m| m.id.clone()).collect()
    } else {
        listed.to_vec()
    };
    let first = ids.first()?;
    let main = if ids.contains(&p.model) { p.model.clone() } else { first.clone() };
    let small = ids.iter().min_by_key(|id| price_rank(c, pid, twin, id)).cloned().unwrap_or_else(|| main.clone());
    Some((format!("{}/{}", pid, main), format!("{}/{}", pid, small)))
}

/// Every role, in the order the screens list them.
pub const ROLES: &[Role] = &[
    Role {
        id: MAIN,
        name: "main",
        about: "your team lead",
        kind: Kind::Chat,
        env: &["BISE_MODEL", "BEND_MODEL"],
        old_key: "model",
        old_effort: "reasoning_effort",
        shown: true,
    },
    Role {
        id: AGENTS,
        name: "agents",
        about: "the agents main starts",
        kind: Kind::Chat,
        env: &["BISE_AGENT_MODEL"],
        old_key: "agent_model",
        old_effort: "agent_reasoning_effort",
        shown: true,
    },
    Role {
        id: SMALL,
        name: "small jobs",
        about: "titles, summaries",
        kind: Kind::Chat,
        env: &["BISE_SMALL_MODEL"],
        old_key: "small_model",
        old_effort: "",
        shown: true,
    },
    Role {
        id: VOICE,
        name: "voice",
        about: "listens when you talk",
        kind: Kind::Voice,
        env: &["BISE_VOICE_MODEL"],
        old_key: "voice.model",
        old_effort: "",
        shown: true,
    },
    // approvals (design §4.2): Jev, a chat model, or off
    Role {
        id: CLASSIFY,
        name: "checker",
        about: "in auto, decides which commands run and which ask you",
        kind: Kind::Chat,
        env: &["BISE_CLASSIFY_MODEL"],
        old_key: "",
        old_effort: "",
        shown: true,
    },
];

pub fn role(id: &str) -> Option<&'static Role> {
    ROLES.iter().find(|r| r.id == id)
}

impl Role {
    /// `small jobs (titles, summaries)`: the name with what it does.
    pub fn label(&self) -> String {
        format!("{} ({})", self.name, self.about)
    }
}

/// A chat role as config.toml's `[roles]` writes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoleConfig {
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// `[roles]` read: the chat roles by id, the voice role's settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RolesConfig {
    pub chat: Vec<(&'static str, RoleConfig)>,
    /// `voice = "…"` or `[roles.voice]` (model, language, vocabulary);
    /// None: not in `[roles]`
    pub voice: Option<VoiceConfig>,
}

impl RolesConfig {
    pub fn get(&self, id: &str) -> RoleConfig {
        self.chat.iter().find(|(r, _)| *r == id).map(|(_, c)| c.clone()).unwrap_or_default()
    }

    /// Read `[roles]`; what is wrong becomes a warning.
    pub fn read(t: &toml::Table, warnings: &mut Vec<String>) -> RolesConfig {
        let mut c = RolesConfig::default();
        let Some(v) = t.get("roles") else { return c };
        let Some(v) = v.as_table() else {
            warnings.push("config.toml: roles: not a table ([roles] then main = \"provider/model\")".into());
            return c;
        };
        let ids = || ROLES.iter().map(|r| r.id).collect::<Vec<_>>().join(", ");
        for (k, x) in v {
            let Some(r) = role(k) else {
                warnings.push(format!("config.toml: roles.{}: unknown role ({})", k, ids()));
                continue;
            };
            if r.kind == Kind::Voice {
                let mut t = toml::Table::new();
                let v = match x {
                    toml::Value::String(s) => {
                        let mut m = toml::Table::new();
                        m.insert("model".into(), toml::Value::String(s.clone()));
                        toml::Value::Table(m)
                    }
                    other => other.clone(),
                };
                t.insert("voice".into(), v);
                let mut w = Vec::new();
                let vc = VoiceConfig::read(&t, &mut w);
                warnings.extend(w.into_iter().map(|w| w.replacen("config.toml: voice", "config.toml: roles.voice", 1)));
                c.voice = Some(vc);
                continue;
            }
            let mut rc = RoleConfig::default();
            let s = |x: &toml::Value| x.as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            match x {
                toml::Value::String(_) => match s(x) {
                    Some(m) => rc.model = Some(m),
                    None => warnings.push(format!("config.toml: roles.{}: a \"provider/model\" name", k)),
                },
                toml::Value::Table(tb) => {
                    for (fk, fv) in tb {
                        match (fk.as_str(), s(fv)) {
                            ("model", Some(m)) => rc.model = Some(m),
                            ("effort", Some(e)) => rc.effort = Some(e),
                            ("model" | "effort", None) => {
                                warnings.push(format!("config.toml: roles.{}.{}: a non-empty string", k, fk))
                            }
                            _ => warnings.push(format!("config.toml: roles.{}.{}: unknown key (model, effort)", k, fk)),
                        }
                    }
                }
                _ => warnings.push(format!("config.toml: roles.{}: a \"provider/model\" name or a table", k)),
            }
            c.chat.push((r.id, rc));
        }
        c
    }
}

// ---- writing config.toml ----

/// The table a header line opens (`[roles]` → "roles", `[roles.voice] #
/// x` → "roles.voice"); None: not a header (an array of tables too).
fn header(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.starts_with('[') || t.starts_with("[[") {
        return None;
    }
    let end = t.find(']')?;
    Some(t[1..end].split('.').map(|p| p.trim().trim_matches('"')).collect::<Vec<_>>().join("."))
}

/// The key a line sets (`model = "x"` → "model"), comments aside.
fn key_of(line: &str) -> Option<&str> {
    let t = line.trim();
    if t.starts_with('#') || t.starts_with('[') {
        return None;
    }
    t.split_once('=').map(|(k, _)| k.trim().trim_matches('"'))
}

/// `text` without the line setting `key` in `table` ("" = top level).
pub fn without_key(text: &str, table: &str, key: &str) -> String {
    let mut cur = String::new();
    let mut out: Vec<&str> = Vec::new();
    for l in text.lines() {
        if let Some(h) = header(l) {
            cur = h;
        } else if cur == table && key_of(l) == Some(key) {
            continue;
        }
        out.push(l);
    }
    let mut s = out.join("\n");
    if !s.is_empty() {
        s.push('\n');
    }
    s
}

/// `text` with `key = value` (`value` already TOML) in `table`: its line
/// replaced where it is, else added at the end of the table, else a new
/// table at the end of the file.
pub fn with_table_key(text: &str, table: &str, key: &str, value: &str) -> String {
    let line = format!("{} = {}", key, value);
    let lines: Vec<&str> = text.lines().collect();
    let mut cur = String::new();
    // the table's last non-blank line, and the line of the key
    let (mut last, mut at) = (None, None);
    for (i, l) in lines.iter().enumerate() {
        if let Some(h) = header(l) {
            cur = h;
            if cur == table {
                last = Some(i);
            }
            continue;
        }
        if cur == table {
            if key_of(l) == Some(key) && at.is_none() {
                at = Some(i);
            }
            if !l.trim().is_empty() {
                last = Some(i);
            }
        }
    }
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    match (at, last) {
        (Some(i), _) => out[i] = line,
        (None, Some(i)) => out.insert(i + 1, line),
        (None, None) => {
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push(String::new());
            }
            out.push(format!("[{}]", table));
            out.push(line);
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// config.toml's text with role `id`'s model set to `model` (BISE-298):
/// in `[roles.<id>]` when that table is there, else `<id> = "…"` in
/// `[roles]`; the old key it replaces goes. The rest of the file as it
/// was.
pub fn with_role(text: &str, id: &str, model: &str) -> String {
    let value = crate::toml_string(model);
    let mut t = text.to_string();
    if let Some(r) = role(id) {
        match r.old_key.split_once('.') {
            Some((table, key)) => t = without_key(&t, table, key),
            None if !r.old_key.is_empty() => t = without_key(&t, "", r.old_key),
            None => {}
        }
    }
    let own = format!("roles.{}", id);
    if t.lines().any(|l| header(l).as_deref() == Some(own.as_str())) {
        with_table_key(&t, &own, "model", &value)
    } else {
        with_table_key(&t, "roles", id, &value)
    }
}

/// config.toml's text with role `id` set (BISE-298): `model` None = unset
/// (it follows its fallback again), `effort` None = the model's default.
/// A role with an effort is written as its table (`[roles.<id>]` model,
/// effort), else as its line of `[roles]` (or its table's model when the
/// table is there). The old keys it replaces go; its other settings
/// (the voice's language) stay.
pub fn set_role(text: &str, id: &str, model: Option<&str>, effort: Option<&str>) -> String {
    let mut t = text.to_string();
    if let Some(r) = role(id) {
        for old in [r.old_key, r.old_effort] {
            match old.split_once('.') {
                Some((table, key)) => t = without_key(&t, table, key),
                None if !old.is_empty() => t = without_key(&t, "", old),
                None => {}
            }
        }
    }
    let own = format!("roles.{}", id);
    t = without_key(&t, "roles", id);
    t = without_key(&t, &own, "model");
    t = without_key(&t, &own, "effort");
    let has_table = t.lines().any(|l| header(l).as_deref() == Some(own.as_str()));
    let Some(model) = model else {
        // an empty [roles.<id>] goes too
        return if has_table && table_is_empty(&t, &own) { without_table(&t, &own) } else { t };
    };
    let value = crate::toml_string(model);
    match effort {
        Some(e) => {
            let t = with_table_key(&t, &own, "model", &value);
            with_table_key(&t, &own, "effort", &crate::toml_string(e))
        }
        None if has_table => with_table_key(&t, &own, "model", &value),
        None => with_table_key(&t, "roles", id, &value),
    }
}

/// Set role `id` in `config.toml` (the file at `file`, [`set_role`]'s
/// edit): the one writer of a role for the TUI's roles screen and the
/// desktop's core (architect m_10795). An exclusive flock on
/// `config.toml.lock` around the whole read-edit-write, a tmp name only
/// this call uses, then a rename: two writers at once never lose a change.
pub fn save_role(file: &std::path::Path, id: &str, model: Option<&str>, effort: Option<&str>) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("couldn't write config.toml: {e}"))?;
    }
    let _lock = lock(&file.with_extension("toml.lock")).map_err(|e| format!("couldn't lock config.toml: {e}"))?;
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let tmp = file.with_extension(format!("toml.tmp-{}-{}", std::process::id(), TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::write(&tmp, set_role(&text, id, model, effort)).map_err(|e| format!("couldn't write config.toml: {e}"))?;
    std::fs::rename(&tmp, file).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("couldn't write config.toml: {e}")
    })
}

/// Each write's own tmp name in this process (with the pid: across them).
static TMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// An exclusive flock on `path`, held until the file is dropped.
fn lock(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
    // SAFETY: flock on a descriptor this function owns; blocks until free
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(f)
}

fn table_is_empty(text: &str, table: &str) -> bool {
    let mut cur = String::new();
    let mut empty = true;
    for l in text.lines() {
        if let Some(h) = header(l) {
            cur = h;
        } else if cur == table && !l.trim().is_empty() && !l.trim().starts_with('#') {
            empty = false;
        }
    }
    empty
}

fn without_table(text: &str, table: &str) -> String {
    let mut cur = String::new();
    let mut out: Vec<&str> = Vec::new();
    for l in text.lines() {
        if let Some(h) = header(l) {
            cur = h;
            if cur == table {
                continue;
            }
        }
        if cur != table {
            out.push(l);
        }
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    let mut s = out.join("\n");
    if !s.is_empty() {
        s.push('\n');
    }
    s
}

/// Where a role's model comes from, in the user's words, for the
/// screens: picked (in config.toml or an env var) or following another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// set in config.toml
    Picked,
    /// an env var sets it (its name)
    Env(&'static str),
    /// unset: the model of that role ("main", "agents")
    SameAs(&'static str),
    /// unset: chosen by bise (a provider's small model, the default
    /// voice model)
    Auto,
    /// none at all
    None,
}

impl Source {
    /// A `Setup` "from" label as a [`Source`].
    pub fn of(from: &'static str) -> Source {
        match from {
            "config" => Source::Picked,
            "model" => Source::SameAs(MAIN),
            "agent_model" => Source::SameAs(AGENTS),
            "small_model" => Source::SameAs(SMALL),
            "provider" | "default" => Source::Auto,
            "none" | "" => Source::None,
            env => Source::Env(env),
        }
    }
}
