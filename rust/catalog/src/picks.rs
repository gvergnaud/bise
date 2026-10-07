//! The models `/model` offers (BISE-117, BISE-294, BISE-301), one rule
//! for the TUI's popup and the hub's typed `models` event (bar A.5,
//! architect m_10427): the chat models of the catalog (built in and
//! config.toml's) whose provider can run a turn now, then the aliases to
//! them. Pure: the TUI keeps its own cache of the ready providers, the
//! hub reads them with its own environment ([`ready_in`]).

use crate::{auth, names, Setup, Source};

/// One model the list offers: what `/model` takes and the facts its row
/// is made from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    /// what `/model` takes: a full id, or an alias
    pub value: String,
    /// the popup's description: `opus 5.5 · Anthropic · 1M · config.toml`
    pub desc: String,
    /// its provider's name, the header `/model` groups it under ("" for
    /// an alias)
    pub provider: String,
    /// what its row says under that header ([`short`])
    pub short: String,
    /// its context window in tokens (none for an alias)
    pub context: Option<u64>,
    /// set in config.toml
    pub config: bool,
    /// an alias: the model it names
    pub alias_of: Option<String>,
}

/// A context window in words: `1M`, `128k`.
pub fn context_words(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}M", n / 1_000_000)
    } else {
        format!("{}k", n / 1000)
    }
}

/// A row's short text: `1M`, `128k · config.toml`, `= anthropic/claude-opus-5-5`.
pub fn short(context: Option<u64>, config: bool, alias_of: Option<&str>) -> String {
    if let Some(to) = alias_of {
        return format!("= {}", to);
    }
    let mine = if config { " · config.toml" } else { "" };
    format!("{}{}", context.map(context_words).unwrap_or_default(), mine)
}

/// The list for the providers `ready` says yes to.
pub fn picks(setup: &Setup, ready: &dyn Fn(&str) -> bool) -> Vec<Pick> {
    let c = &setup.catalog;
    let mut out = Vec::new();
    for m in c.models.iter().filter(|m| !m.stt) {
        let Some(p) = c.provider(&m.provider).filter(|p| p.chats() && p.needs.is_empty() && ready(&p.id)) else {
            continue;
        };
        let r = c.resolve(&m.name());
        let config = m.source == Source::Config;
        let ctx = context_words(r.caps.context);
        let mine = if config { " · config.toml" } else { "" };
        out.push(Pick {
            value: m.name(),
            desc: format!("{} · {} · {}{}", names::long_name(&m.name()), p.name, ctx, mine),
            provider: p.name.clone(),
            short: short(Some(r.caps.context), config, None),
            context: Some(r.caps.context),
            config,
            alias_of: None,
        });
    }
    for (a, to) in c.aliases.iter().filter(|(_, to)| ready(&c.resolve(to).provider)) {
        out.push(Pick {
            value: a.clone(),
            desc: format!("= {}", to),
            provider: String::new(),
            short: short(None, false, Some(to)),
            context: None,
            config: false,
            alias_of: Some(to.clone()),
        });
    }
    out
}

/// The providers that can run a turn now for an environment and a bise
/// home: a key found where the harness finds it (the environment,
/// auth.json, the old .env files), or none needed.
pub fn ready_in(setup: &Setup, env: &dyn Fn(&str) -> Option<String>, home: &bise_home::Home) -> Vec<String> {
    use auth::{EnvFile, Keys, Store};
    let store = Store::read(&home.auth_file()).unwrap_or_default();
    let files = EnvFile::read_all(&home.env_files());
    let keys = Keys { env, store: &store, files: &files };
    setup.catalog.providers.iter().filter(|p| keys.ready(p)).map(|p| p.id.clone()).collect()
}

/// The role whose default a model is (config.toml's [roles], or the
/// built-in default): `main`, `agents`, both or none.
pub fn default_for(setup: &Setup, model: &str) -> (bool, bool) {
    let full = setup.catalog.resolve(model).name;
    (setup.model_for("main").name == full, setup.model_for("agent").name == full)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Setup {
        Setup::from_text(None, &|_| None)
    }

    #[test]
    fn the_model_list_has_only_the_models_of_ready_providers() {
        let s = setup();
        let picks = picks(&s, &|id| id == "anthropic");
        assert!(!picks.is_empty());
        for p in picks.iter().filter(|p| p.alias_of.is_none()) {
            assert!(p.value.starts_with("anthropic/"), "{}", p.value);
            assert!(p.context.is_some() && p.short == short(p.context, p.config, None), "{p:?}");
        }
        for p in picks.iter().filter(|p| p.alias_of.is_some()) {
            assert!(p.provider.is_empty() && p.short.starts_with("= "), "{p:?}");
        }
        let all = super::picks(&s, &|_| true);
        assert!(all.iter().any(|p| p.value.starts_with("openrouter/")));
        assert!(all.len() > picks.len());
    }

    #[test]
    fn a_rows_short_text_is_made_from_its_facts() {
        assert_eq!(short(Some(1_000_000), false, None), "1M");
        assert_eq!(short(Some(128_000), true, None), "128k · config.toml");
        assert_eq!(short(None, false, Some("anthropic/claude-opus-5-5")), "= anthropic/claude-opus-5-5");
        assert_eq!(context_words(200_000), "200k");
    }

    #[test]
    fn a_machine_with_no_key_has_only_the_keyless_providers_ready() {
        let h = std::env::temp_dir().join(format!("bise-picks-{}", std::process::id()));
        let hs = h.to_string_lossy().to_string();
        let env = |k: &str| (k == "HOME").then(|| hs.clone());
        let home = bise_home::Home::from_lookup(&env);
        let ready = ready_in(&setup(), &env, &home);
        assert!(ready.iter().any(|r| r == "ollama") && !ready.iter().any(|r| r == "openrouter"), "{ready:?}");
    }

    #[test]
    fn the_roles_default_model_says_so() {
        let s = setup();
        let main = s.model_for("main").name;
        assert!(default_for(&s, &main).0);
        assert_eq!(default_for(&s, "nowhere/x"), (false, false));
    }
}
