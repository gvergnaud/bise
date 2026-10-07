//! What bise's catalog says of the agents' models (BISE-150): the
//! context window of the gauge, the prices of the usage line, whether a
//! model reads images (checked before a message with images goes out).
//!
//! The catalog is the built-in list merged with config.toml, read once
//! (a new `[models]` table needs a restart, as for the REPLs). Names
//! are the full `provider/model` ids the REPL announces (usage lines,
//! harness-info); an old bare id goes through the legacy rule
//! (`claude*` -> foundry, else mistral), like everywhere else.

use bise_catalog::{Known, Resolved, Setup};
use std::sync::OnceLock;

fn setup() -> &'static Setup {
    static SETUP: OnceLock<Setup> = OnceLock::new();
    SETUP.get_or_init(load)
}

#[cfg(not(test))]
fn load() -> Setup {
    Setup::load(&bise_home::Home::from_env().config_file())
}

/// Tests: the built-in list alone, whatever the machine's config says.
#[cfg(test)]
fn load() -> Setup {
    Setup::from_text(None, &|_| None)
}

fn resolve(model: &str) -> Option<Resolved> {
    let m = model.trim();
    (!m.is_empty()).then(|| setup().catalog.resolve(m))
}

/// The context window of a model, in tokens: listed, else its
/// provider's default. None: no model, or a provider nobody knows.
pub(crate) fn context_window(model: &str) -> Option<u64> {
    resolve(model).filter(|r| r.known != Known::NoProvider).map(|r| r.caps.context)
}

/// The cost of one call in USD; None when its prices are not known.
pub(crate) fn cost(model: &str, input: u64, output: u64, cache_read: u64, cache_write: u64) -> Option<f64> {
    resolve(model)?.price.cost(input, output, cache_read, cache_write)
}

/// The model an agent starts with: main's, or the sub-agents'.
pub(crate) fn model_for(main: bool) -> String {
    setup().model_for(if main { "main" } else { "agent" }).name
}

/// A listed model the catalog says cannot read images. An unlisted one
/// is not refused here: the provider says (the no-vision line then
/// comes from its error).
/// The rule is bise_catalog's `Catalog::vision` (shared with the hub's
/// agent rows, bise desktop K4).
pub(crate) fn lacks_vision(model: &str) -> bool {
    setup().catalog.vision(model) == Some(false)
}

// ---- the model and effort shown (BISE-135) ----

/// A model's name for people (`foundry/claude-opus-5-5` -> `opus 5.5`):
/// [`bise_catalog::names::long_name`], the hub's `sb list` uses it too.
pub(crate) fn long_name(model: &str) -> String {
    bise_catalog::names::long_name(model)
}

/// The panel's tag, `opus·hi` (ASCII `opus.hi`):
/// [`bise_catalog::names::tag`], the same as `sb list`'s.
pub(crate) fn tag(model: &str, effort: &str, others: &[&str]) -> String {
    let dot = if crate::theme::ascii_mode() { "." } else { "·" };
    bise_catalog::names::tag(model, effort, others, dot, 10)
}

/// The efforts a model takes and the one it gets by default (the
/// catalog's rule, rust/catalog Resolved::efforts).
pub(crate) fn efforts(model: &str) -> (Vec<String>, String) {
    match resolve(model) {
        Some(r) => (r.efforts(), r.default_effort()),
        None => (Vec::new(), String::new()),
    }
}

/// One model the `/model` popup offers ([`bise_catalog::picks`], the
/// hub's typed `models` rows use the same rule).
pub(crate) use bise_catalog::picks::Pick;

/// The chat models whose provider can run a turn now, then the aliases
/// to them: `/model`'s list (BISE-117 completion).
pub(crate) fn picks() -> Vec<Pick> {
    let ready = ready_ids();
    bise_catalog::picks::picks(setup(), &|id| ready.iter().any(|r| r == id))
}

/// The providers that can run a turn now: a key found where the
/// harness finds it (`bise_catalog::auth::Keys`: the environment,
/// auth.json, the old .env files) or none needed. Read again at most
/// once a second (the popup asks at each frame), and at once after
/// `/provider` ([`forget_keys`]).
pub(crate) fn ready_ids() -> Vec<String> {
    #[cfg(test)]
    if let Some(ids) = TEST_READY.with(|r| r.borrow().clone()) {
        return ids;
    }
    #[cfg(test)]
    return setup().catalog.providers.iter().map(|p| p.id.clone()).collect();
    #[cfg(not(test))]
    {
        use std::sync::Mutex;
        use std::time::{Duration, Instant};
        static CACHE: Mutex<Option<(Instant, Vec<String>)>> = Mutex::new(None);
        let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let forgot = FORGOT.swap(false, std::sync::atomic::Ordering::SeqCst);
        if let Some((_, ids)) = c.as_ref().filter(|(at, _)| at.elapsed() < Duration::from_secs(1) && !forgot) {
            return ids.clone();
        }
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let ids = ready_in(&env, &bise_home::Home::from_env());
        *c = Some((Instant::now(), ids.clone()));
        ids
    }
}

// tests: the providers ready on this thread (None: all of them)
#[cfg(test)]
thread_local! {
    pub(crate) static TEST_READY: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

static FORGOT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The keys changed (`/provider`): the next list reads them again.
pub(crate) fn forget_keys() {
    FORGOT.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// The ready providers for an environment and a bise home
/// ([`bise_catalog::picks::ready_in`]).
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn ready_in(env: &dyn Fn(&str) -> Option<String>, home: &bise_home::Home) -> Vec<String> {
    bise_catalog::picks::ready_in(setup(), env, home)
}

/// A model `/model` may not switch to yet: its provider is known and
/// usable but has no key. Its provider's id and name.
pub(crate) fn keyless(model: &str) -> Option<(String, String)> {
    let r = resolve(model)?;
    let c = &setup().catalog;
    let p = c.provider(&r.provider)?;
    let ok = r.known == Known::NoProvider || !p.needs.is_empty() || !p.chats() || ready_ids().contains(&p.id);
    (!ok).then(|| (p.id.clone(), p.name.clone()))
}

/// `/model <model> [default]` of a provider without a key: its provider
/// id and the full model id. The one rule for the TUI (sb.rs: the
/// provider's setup opens, then the line runs) and the window core (R8:
/// the line is held until the provider works).
pub(crate) fn model_needs_key(line: &str) -> Option<(String, String)> {
    let mut w = line.split_whitespace();
    if w.next() != Some("/model") {
        return None;
    }
    let full = full_name(w.next()?)?;
    keyless(&full).map(|(id, _)| (id, full))
}

/// A model as `/model` takes it (a full id, a bare one, an alias) as its
/// full `provider/model` id; None for a provider nobody knows.
pub(crate) fn full_name(model: &str) -> Option<String> {
    resolve(model).filter(|r| r.known != Known::NoProvider).map(|r| r.name)
}

/// A provider's name for people, by its id or its key variable
/// (`OPENROUTER_API_KEY` -> OpenRouter); the id or the variable itself
/// when the catalog has none.
pub(crate) fn provider_name(id: &str, key_env: &str) -> String {
    setup().catalog.provider_name(id, key_env)
}

/// The providers not ready, the offered ones first: `/model`'s last
/// row names them (`OpenRouter, Groq, xAI…`).
pub(crate) fn not_ready_names() -> Vec<String> {
    let ready = ready_ids();
    let c = &setup().catalog;
    let usable = |p: &&bise_catalog::Provider| p.needs.is_empty() && p.chats() && !p.key_env.is_empty() && !ready.contains(&p.id);
    let mut v: Vec<&bise_catalog::Provider> = c.providers.iter().filter(usable).collect();
    v.sort_by_key(|p| p.hidden);
    v.into_iter().map(|p| p.name.clone()).collect()
}

/// A typed model id the pickers may use as is (BISE-289), with its
/// provider: `gpt-6-astra` -> `openai/gpt-6-astra` (`provider`, the
/// current model's), `openai/gpt-6-astra` as typed. None: nothing
/// typed, blanks, or no provider to put in front.
pub(crate) fn free_id(typed: &str, provider: &str) -> Option<String> {
    let t = plain_id(typed)?;
    match (t.contains('/'), provider.is_empty()) {
        (true, _) => Some(t.to_string()),
        (false, false) => Some(format!("{}/{}", provider, t)),
        (false, true) => None,
    }
}

/// The same for a picker of one provider (the first run's `which
/// model?`): the id is always that provider's, so `openai/gpt-6-astra`
/// under OpenRouter is `openrouter/openai/gpt-6-astra`.
pub(crate) fn free_id_of(typed: &str, provider: &str) -> Option<String> {
    let t = plain_id(typed)?;
    match t.strip_prefix(provider).and_then(|r| r.strip_prefix('/')) {
        Some(rest) if !rest.is_empty() => Some(t.to_string()),
        _ => Some(format!("{}/{}", provider, t)),
    }
}

fn plain_id(typed: &str) -> Option<&str> {
    let t = typed.trim();
    let ok = !t.is_empty() && !t.chars().any(char::is_whitespace) && !t.starts_with('/') && !t.ends_with('/');
    ok.then_some(t)
}

/// `$0.0042`, `$0.13`, `$2.40`.
pub(crate) fn fmt_cost(usd: f64) -> String {
    if usd >= 0.1 {
        format!("${:.2}", usd)
    } else {
        format!("${:.4}", usd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_come_from_the_catalog() {
        assert_eq!(context_window("foundry/claude-opus-5-5"), Some(1_000_000));
        assert_eq!(context_window("anthropic/claude-haiku-4-5"), Some(200_000));
        assert_eq!(context_window("openai/gpt-6-astra"), Some(1_050_000));
        // an unlisted model: its provider's default
        assert_eq!(context_window("groq/brand-new"), Some(131_072));
        // the old bare ids of the usage lines
        assert_eq!(context_window("claude-opus-5-5"), Some(1_000_000));
        assert_eq!(context_window("zai-glm-5-3"), Some(1_048_576));
        // nothing knows it: no window, never a panic
        assert_eq!(context_window("nowhere/m"), None);
        assert_eq!(context_window(""), None);
    }

    #[test]
    fn a_call_costs_its_prices() {
        // haiku 4.5: 1 in, 5 out, 0.10 cache read, 1.25 cache write
        let c = cost("anthropic/claude-haiku-4-5", 1_000_000, 100_000, 0, 0).unwrap();
        assert!((c - 1.5).abs() < 1e-9, "{c}");
        let c = cost("anthropic/claude-haiku-4-5", 100_000, 0, 90_000, 10_000).unwrap();
        assert!((c - (0.009 + 0.0125)).abs() < 1e-9, "{c}");
        assert_eq!(cost("fireworks/accounts/fireworks/models/glm-5p3", 10, 10, 0, 0), None);
        assert_eq!(cost("nowhere/m", 10, 10, 0, 0), None);
        assert_eq!(fmt_cost(4.5), "$4.50");
        assert_eq!(fmt_cost(0.0042), "$0.0042");
    }

    #[test]
    fn names_and_tags() {
        assert_eq!(long_name("foundry/claude-opus-5-5"), "opus 5.5");
        assert_eq!(long_name("anthropic/claude-sonnet-4-5"), "sonnet 4.5");
        assert_eq!(long_name("mistral/devstral-medium-2509"), "devstral-medium");
        assert_eq!(long_name("mistral/mistral-large-latest"), "mistral-large");
        assert_eq!(long_name("openai/gpt-5.1-codex"), "gpt-5.1-codex");
        assert_eq!(long_name("mistral/zai-glm-5-3"), "glm 5.3");
        assert_eq!(long_name("anthropic/claude-3-5-sonnet-20241022"), "3-5-sonnet");
        let family = bise_catalog::names::family;
        assert_eq!(family("foundry/claude-opus-5-5"), "opus");
        assert_eq!(family("openai/gpt-5.1-codex"), "gpt-5.1");
        assert_eq!(family("mistral/devstral-medium-2509"), "devstral");
        assert_eq!(tag("foundry/claude-opus-5-5", "high", &[]), "opus·hi");
        assert_eq!(tag("anthropic/claude-sonnet-4-5", "low", &[]), "sonnet·lo");
        assert_eq!(tag("openai/gpt-4.1", "", &[]), "gpt-4.1");
        assert_eq!(tag("mistral/zai-glm-5-3", "none", &[]), "glm·off");
        // two models of one family: the version tells them apart
        let others = ["foundry/claude-opus-5-5", "anthropic/claude-opus-4-5"];
        assert_eq!(tag("anthropic/claude-opus-4-5", "medium", &others), "opus4.5·med");
        assert_eq!(tag("foundry/claude-opus-5-5", "max", &others[..1]), "opus·max");
        assert_eq!(tag("x/averyveryverylongmodelname", "high", &[]), "averyvery…·hi");
    }

    #[test]
    fn the_model_list_has_the_catalog_and_its_aliases() {
        let p = picks();
        let has = |v: &str| p.iter().find(|x| x.value == v);
        assert!(has("foundry/claude-opus-5-5").unwrap().desc.starts_with("opus 5.5 · Anthropic (foundry proxy) · 1M"));
        assert!(has("anthropic/claude-sonnet-5-5").is_some());
        assert!(has("openai/gpt-6-astra").is_some());
        assert_eq!(has("opus-5.5").unwrap().desc, "= foundry/claude-opus-5-5");
        // no speech-to-text model, no provider that is not usable yet
        assert!(has("mistral/voxtral-mini-latest").is_none());
        assert!(!p.iter().any(|x| x.value.starts_with("bedrock/")));
        assert_eq!(efforts("foundry/claude-opus-5-5").1, "high");
        assert_eq!(efforts("mistral/zai-glm-5-3").0, ["none", "high"]);
        assert!(efforts("mistral/mistral-large-latest").0.is_empty());
        // BISE-294: only the providers set up, their aliases too
        TEST_READY.with(|r| *r.borrow_mut() = Some(vec!["anthropic".into(), "ollama".into()]));
        let p = picks();
        assert!(p.iter().any(|x| x.value == "anthropic/claude-sonnet-5-5"));
        assert!(!p.iter().any(|x| x.value.starts_with("foundry/") || x.value.starts_with("openai/")));
        assert!(!p.iter().any(|x| x.value == "opus-5.5"), "its alias goes with it");
        assert!(not_ready_names().starts_with(&["OpenAI".to_string()]), "{:?}", not_ready_names());
        assert_eq!(keyless("openai/gpt-6-astra"), Some(("openai".into(), "OpenAI".into())));
        assert_eq!(keyless("ollama/anything"), None);
        TEST_READY.with(|r| *r.borrow_mut() = None);
    }

    #[test]
    fn a_typed_id_gets_its_provider() {
        assert_eq!(free_id("gpt-6-astra", "openai").as_deref(), Some("openai/gpt-6-astra"));
        assert_eq!(free_id(" openai/gpt-6-astra ", "foundry").as_deref(), Some("openai/gpt-6-astra"));
        assert_eq!(free_id("x", ""), None);
        for bad in ["", "  ", "a b", "/x", "x/"] {
            assert_eq!(free_id(bad, "openai"), None, "{bad:?}");
        }
        assert_eq!(free_id_of("gpt-6-astra", "openai").as_deref(), Some("openai/gpt-6-astra"));
        assert_eq!(free_id_of("openai/gpt-6-astra", "openai").as_deref(), Some("openai/gpt-6-astra"));
        assert_eq!(free_id_of("openai/gpt-6-astra", "openrouter").as_deref(), Some("openrouter/openai/gpt-6-astra"));
        assert_eq!(free_id_of("openrouter/x/y", "openrouter").as_deref(), Some("openrouter/x/y"));
        assert_eq!(free_id_of("openai", "openai").as_deref(), Some("openai/openai"));
    }
}
