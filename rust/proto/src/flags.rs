//! Feature flags (docs/flags.md, his pick feature-flags#q-shape,
//! architect m_67929): the one typed list of flags and the pure read of
//! what each one is.
//!
//! A feature that is not released yet lands behind its [`Flag`], off by
//! default; he turns it on for himself in the `[flags]` table of his
//! `~/.bise/config.toml`. The release is the update that deletes the flag
//! and its checks. Nothing is fetched: [`resolve`] reads `BISE_FLAG`, then
//! `config.toml`, then the flag's default.
//!
//! No flag is live today (tasks shipped without one). The full wiring a
//! flag can need (sb-core, the hub, the prompts, the app) is at
//! 7a72d2f5b (branch `sb/feature-flags-wiring`): the first flag copies
//! the parts it uses (docs/flags.md).
//!
//! Pure: the env's text and the config table come in as arguments
//! (`bise_home::flags` reads them).

use std::collections::BTreeMap;

/// A feature flag. A name is added, never renamed or reused; a released
/// one is deleted with its checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Flag {
    /// Tests only: the flag the precedence rows read.
    #[cfg(test)]
    Test,
}

/// What a flag is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub name: &'static str,
    /// its value when nothing says otherwise
    pub default: bool,
    /// the feature it hides (the name release branches use)
    pub feature: Option<&'static str>,
    pub note: &'static str,
}

impl Flag {
    /// Every known flag, in the order `bise flags` lists them.
    #[cfg(not(test))]
    pub const ALL: &'static [Flag] = &[];
    #[cfg(test)]
    pub const ALL: &'static [Flag] = &[Flag::Test];

    pub fn info(self) -> Info {
        match self {
            #[cfg(test)]
            Flag::Test => Info { name: "test", default: false, feature: Some("test"), note: "a test's flag" },
        }
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    /// The known flag named `name`.
    pub fn of_name(name: &str) -> Option<Flag> {
        Flag::ALL.iter().copied().find(|f| f.name() == name)
    }
}

/// Where a flag's value came from (`bise flags` says it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// `BISE_FLAG`
    Env,
    /// the `[flags]` table of `config.toml`
    Config,
    /// the flag's default
    Default,
}

impl Source {
    pub fn word(self) -> &'static str {
        match self {
            Source::Env => "BISE_FLAG",
            Source::Config => "config.toml",
            Source::Default => "default",
        }
    }
}

/// Values by flag name, as one source says them.
pub type Values = BTreeMap<String, bool>;

/// A word that says on or off.
pub fn word_bool(w: &str) -> Option<bool> {
    match w.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "1" | "yes" => Some(true),
        "off" | "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

/// `BISE_FLAG="name=on,other=off"`: its values, and each part it could
/// not read.
pub fn parse_env(text: &str) -> (Values, Vec<String>) {
    let mut vals = Values::new();
    let mut bad = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('=').and_then(|(k, v)| word_bool(v).map(|b| (k.trim().to_string(), b))) {
            Some((k, b)) if !k.is_empty() => {
                vals.insert(k, b);
            }
            _ => bad.push(part.to_string()),
        }
    }
    (vals, bad)
}

/// What every source says, read once.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inputs {
    /// `BISE_FLAG`'s values
    pub env: Values,
    /// `config.toml`'s `[flags]`
    pub config: Values,
}

impl Inputs {
    /// From what `bise_home::flags::read` gives: `BISE_FLAG`'s text and
    /// the config's table.
    pub fn read(env: Option<&str>, config: Values) -> Inputs {
        Inputs { env: env.map(|t| parse_env(t).0).unwrap_or_default(), config }
    }
}

/// A flag's value and where it came from: the first source that names it.
pub fn resolve(flag: Flag, i: &Inputs) -> (bool, Source) {
    let name = flag.name();
    if let Some(b) = i.env.get(name) {
        return (*b, Source::Env);
    }
    if let Some(b) = i.config.get(name) {
        return (*b, Source::Config);
    }
    (flag.info().default, Source::Default)
}

/// The flags that are on, in [`Flag::ALL`]'s order.
pub fn on(i: &Inputs) -> Vec<Flag> {
    Flag::ALL.iter().copied().filter(|f| resolve(*f, i).0).collect()
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod tests;
