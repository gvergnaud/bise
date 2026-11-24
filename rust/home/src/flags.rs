//! Feature flags' reads (docs/flags.md, architect m_67929): the machine's
//! part, with no protocol type (bise_proto::flags resolves what this
//! returns). Nothing is fetched: the only sources are `BISE_FLAG` and the
//! `[flags]` table of his `config.toml` ([`config_flags`]), read only.

use std::collections::BTreeMap;
use std::path::Path;

use crate::Home;

/// His overrides, `name=on,other=off` (a test-kind row: no REPL inherits it).
pub const ENV_VAR: &str = "BISE_FLAG";

/// `config.toml`'s `[flags]`: each boolean by name, and a line for each
/// value that is not one (skipped). No file, no table: nothing.
pub fn config_flags(path: &Path) -> (BTreeMap<String, bool>, Vec<String>) {
    let Ok(text) = std::fs::read_to_string(path) else { return Default::default() };
    let table = match text.parse::<toml::Table>() {
        Ok(t) => t,
        Err(_) if !text.contains("[flags]") => return Default::default(),
        Err(e) => return (BTreeMap::new(), vec![format!("config.toml does not read: {}", e.message())]),
    };
    let Some(flags) = table.get("flags") else { return Default::default() };
    let Some(flags) = flags.as_table() else { return (BTreeMap::new(), vec!["config.toml: [flags] is not a table".into()]) };
    let mut vals = BTreeMap::new();
    let mut bad = Vec::new();
    for (k, v) in flags {
        match v.as_bool() {
            Some(b) => {
                vals.insert(k.clone(), b);
            }
            None => bad.push(format!("config.toml: flags.{k} = {v} is not true or false, skipped")),
        }
    }
    (vals, bad)
}

/// What every source says, read now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Raw {
    /// `BISE_FLAG`'s text
    pub env: Option<String>,
    /// `config.toml`'s `[flags]`
    pub config: BTreeMap<String, bool>,
    /// what in `config.toml` was skipped
    pub config_bad: Vec<String>,
}

/// Read every source.
pub fn read(home: &Home, env: Option<String>) -> Raw {
    let (config, config_bad) = config_flags(&home.config_file());
    Raw { env, config, config_bad }
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod tests;
