//! The model of a task, chosen at its spawn (GitHub issue #4):
//! `sb spawn <name> ... [--model <id>] [--effort <e>] [--profile <name>]`.
//!
//! Profiles are named model + effort pairs of config.toml:
//!
//! ```toml
//! [profiles]
//! fast = "mistral/mistral-small-latest"   # the line form: a model
//!
//! [profiles.deep]                         # the table form
//! model = "anthropic/claude-opus-5-5"
//! effort = "high"
//! ```
//!
//! Every option is optional: nothing asked, the task runs on the agents
//! role (`bise config set agents ...`), as before. A model or a profile
//! that cannot run (unknown, no key, not signed in) never fails the
//! spawn: the task runs on the agents default and the [`Pick`] says
//! what was asked and why it does not run. What was picked is the
//! task's [`Choice`] file, the same one `/model` writes, so the REPL
//! reads it before each call and the views show it.

use crate::{names, Catalog, Choice, Known, Resolved, Setup};

/// One profile of `[profiles]`: a model, an effort, either may be "".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub model: String,
    pub effort: String,
}

/// Read `[profiles]`; what is wrong becomes a warning.
pub fn read_profiles(t: &toml::Table, warnings: &mut Vec<String>) -> Vec<(String, Profile)> {
    let mut out = Vec::new();
    let Some(v) = t.get("profiles") else { return out };
    let Some(v) = v.as_table() else {
        warnings.push("config.toml: profiles: not a table ([profiles.deep] then model = \"provider/model\")".into());
        return out;
    };
    for (name, x) in v {
        let p = match x {
            toml::Value::String(m) => Profile { model: m.trim().to_string(), effort: String::new() },
            toml::Value::Table(pt) => {
                let s = |k: &str| pt.get(k).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).unwrap_or_default();
                for k in pt.keys().filter(|k| !["model", "effort"].contains(&k.as_str())) {
                    warnings.push(format!("config.toml: profiles.{}: unknown key {} (model, effort)", name, k));
                }
                Profile { model: s("model"), effort: s("effort").to_ascii_lowercase() }
            }
            _ => {
                warnings.push(format!("config.toml: profiles.{}: a model, or a table with model and effort", name));
                continue;
            }
        };
        if p.model.is_empty() && p.effort.is_empty() {
            warnings.push(format!("config.toml: profiles.{}: no model and no effort", name));
            continue;
        }
        out.push((name.clone(), p));
    }
    out
}

/// What main asked at the spawn. All "": nothing asked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ask {
    pub model: String,
    pub effort: String,
    pub profile: String,
}

impl Ask {
    pub fn is_empty(&self) -> bool {
        self.model.trim().is_empty() && self.effort.trim().is_empty() && self.profile.trim().is_empty()
    }
}

/// What the task runs on, and the words that say it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pick {
    /// the task's choice file (empty: the agents default, as before)
    pub choice: Choice,
    /// the words after "agent x created" in main's answer: `on gpt-9 ·
    /// low (profile fast)`, `on opus 5.5, the agents default (asked
    /// gpt-9: unknown model)`; "" when nothing was asked
    pub answer: String,
    /// the quiet line at the top of the task's thread when it fell back;
    /// "" otherwise
    pub line: String,
}

/// Why a model cannot run a turn now, in the user's words (they read
/// after "asked <model>:"); None: it can. `ready`: a provider has its
/// key or its sign-in ([`crate::auth::Keys::ready`]).
pub fn why_not(c: &Catalog, r: &Resolved, ready: &dyn Fn(&crate::Provider) -> bool) -> Option<String> {
    let Some(p) = c.provider(&r.provider).filter(|_| r.known != Known::NoProvider) else {
        return Some("unknown provider".into());
    };
    if !p.chats() || c.model(&r.name).is_some_and(|m| m.stt) {
        return Some("not a chat model".into());
    }
    if !p.needs.is_empty() {
        return Some(format!("{} is not usable yet (needs {})", p.name, p.needs));
    }
    if ready(p) {
        return None;
    }
    Some(if p.signs_in() {
        format!("{} needs a sign-in, bise login {}", p.name, p.id)
    } else {
        format!("no {} key, /provider adds one", p.name)
    })
}

impl Setup {
    /// A profile of `[profiles]` by its name.
    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|(n, _)| n == name.trim()).map(|(_, p)| p)
    }

    /// The full name of a model main named: an alias or "provider/id"
    /// as the catalog says ([`Catalog::canonical`]); a bare id that a
    /// provider lists, the first such provider that can run it (`gpt-5.5`
    /// -> `openai/gpt-5.5` with an OpenAI key); else None, unknown (no
    /// legacy guess here: `gpt-9` is not `mistral/gpt-9`).
    fn spawn_name(&self, name: &str, ready: &dyn Fn(&crate::Provider) -> bool) -> Option<String> {
        let c = &self.catalog;
        let name = name.trim();
        if c.aliases.iter().any(|(a, _)| a == name) || crate::split_name(name).is_some() {
            return Some(c.canonical(name));
        }
        let listed: Vec<&crate::Model> = c.models.iter().filter(|m| m.id == name && !m.stt).collect();
        let usable = |m: &&&crate::Model| c.provider(&m.provider).is_some_and(|p| ready(p) && p.chats());
        listed.iter().find(usable).or(listed.first()).map(|m| format!("{}/{}", m.provider, m.id))
    }

    /// The model of a new task (issue #4). `ready`: a provider has its
    /// key or sign-in. Never refuses: a model or profile that cannot run
    /// gives the agents default and the words of why.
    pub fn spawn_pick(&self, ask: &Ask, ready: &dyn Fn(&crate::Provider) -> bool) -> Pick {
        if ask.is_empty() {
            return Pick::default();
        }
        let profile_name = ask.profile.trim();
        let profile = match profile_name {
            "" => Profile::default(),
            n => match self.profile(n) {
                Some(p) => p.clone(),
                None => return self.fall_back(n, &format!("no profile named {}", n)),
            },
        };
        let asked_model = Some(ask.model.trim()).filter(|m| !m.is_empty()).unwrap_or(&profile.model).to_string();
        let effort = Some(ask.effort.trim()).filter(|e| !e.is_empty()).unwrap_or(&profile.effort).to_ascii_lowercase();
        let by = if profile_name.is_empty() {
            if ask.model.trim().is_empty() { "--effort".to_string() } else { "--model".to_string() }
        } else {
            format!("profile {}", profile_name)
        };
        let mut choice = Choice { by: by.clone(), ..Choice::default() };
        // the model: the one asked, else the agents default (an effort alone)
        let target = if asked_model.is_empty() {
            self.model_for("agent")
        } else {
            let Some(full) = self.spawn_name(&asked_model, ready) else {
                return self.fall_back(&asked_model, "unknown model");
            };
            let r = self.catalog.resolve(&full);
            if let Some(why) = why_not(&self.catalog, &r, ready) {
                return self.fall_back(&full, &why);
            }
            choice.model = r.name.clone();
            r
        };
        if !effort.is_empty() {
            let words = target.efforts();
            let short = names::long_name(&target.name);
            if words.is_empty() {
                choice.note = format!("{} has no effort setting: {} is ignored", short, effort);
            } else if !words.contains(&effort) {
                // `takes none or high`: its levels in order, never a comma list
                choice.note = format!("{} takes {}: {} is ignored", short, words.join(" or "), effort);
            } else {
                choice.effort = effort.clone();
            }
        }
        let used = self.in_use("agent", &choice);
        let mut why: Vec<String> = Vec::new();
        if !profile_name.is_empty() {
            why.push(by);
        }
        if !choice.note.is_empty() {
            why.push(choice.note.clone());
        }
        let tail = if why.is_empty() { String::new() } else { format!(" ({})", why.join("; ")) };
        Pick {
            answer: format!("on {}{}", names::with_effort(&used.model.name, &used.effort), tail),
            choice,
            line: String::new(),
        }
    }

    /// The agents default, with what was asked and why it does not run.
    fn fall_back(&self, asked: &str, why: &str) -> Pick {
        let choice = Choice { asked: asked.to_string(), why: why.to_string(), ..Choice::default() };
        let used = self.in_use("agent", &choice);
        let on = names::with_effort(&used.model.name, &used.effort);
        // a model of a known provider by its name for people; anything
        // else as it was written (`nowhere/x-1`, `gpt-9`, a profile)
        let known = crate::split_name(asked).is_some_and(|(p, _)| self.catalog.provider(p).is_some());
        let short = if known {
            names::long_name(asked)
        } else {
            asked.to_string()
        };
        Pick {
            answer: format!("on {}, the agents default (asked {}: {})", on, short, why),
            line: format!("asked for {}: {}. running on {}, the agents default.", short, why, on),
            choice,
        }
    }

    /// The `model:` line of `sb tasks` for a task with choice `c`: its
    /// full id and effort, then where it comes from (`profile fast`,
    /// `agents default; asked openai/gpt-9: unknown model`).
    pub fn model_line(&self, c: &Choice) -> String {
        let used = self.in_use("agent", c);
        let mut from: Vec<String> = Vec::new();
        if c.model.trim().is_empty() {
            from.push("agents default".into());
        } else {
            from.push(if c.by.is_empty() { "/model".to_string() } else { c.by.clone() });
        }
        if !c.why.is_empty() {
            from.push(format!("asked {}: {}", c.asked, c.why));
        }
        if !c.note.is_empty() {
            from.push(c.note.clone());
        }
        let effort = if used.effort.is_empty() { String::new() } else { format!(" · {}", used.effort) };
        format!("{}{} ({})", used.model.name, effort, from.join("; "))
    }
}
