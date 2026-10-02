use super::*;

fn no_env(_: &str) -> Option<String> {
    None
}

fn setup(cfg: &str) -> Setup {
    Setup::from_text(Some(cfg), &no_env)
}

#[test]
fn the_builtin_list_parses_with_no_warning() {
    let c = Catalog::builtin();
    assert_eq!(c.warnings, Vec::<String>::new());
    for id in [
        "anthropic", "foundry", "openai", "google", "mistral", "openrouter", "groq", "xai", "deepseek",
        "together", "fireworks", "cerebras", "ollama", "lmstudio", "azure", "vertex", "bedrock",
    ] {
        let p = c.provider(id).unwrap_or_else(|| panic!("provider {id}"));
        assert!(FAMILIES.contains(&p.api.as_str()), "{id}: {}", p.api);
        assert!(!p.base_url.ends_with('/'), "{id}");
    }
    // every listed model has a provider, no duplicate
    let mut names: Vec<String> = c.models.iter().map(|m| m.name()).collect();
    for m in &c.models {
        assert!(c.provider(&m.provider).is_some(), "{}", m.name());
    }
    let n = names.len();
    names.sort();
    names.dedup();
    assert_eq!(n, names.len());
    // the cloud wrappers wait for BISE-149, the local ones need no key
    for id in ["azure", "vertex", "bedrock"] {
        assert_eq!(c.provider(id).unwrap().needs, "BISE-149");
    }
    for id in ["ollama", "lmstudio"] {
        assert_eq!(c.provider(id).unwrap().key_env, "");
    }
}

#[test]
fn no_model_by_default_and_opus_alias_still_goes_to_foundry() {
    // BISE-266: no built-in default model: none until a key is checked
    let s = Setup::from_text(None, &no_env);
    assert_eq!((s.model.as_str(), s.model_from), ("", "none"));
    assert_eq!((s.agent_model.as_str(), s.small_model.as_str()), ("", ""));
    assert_eq!(s.model_for("main").known, Known::NoProvider);
    // the old config line (Gabriel's, the old template's) keeps its setup
    let s = Setup::from_text(Some("model = \"opus-5.5\"\n"), &no_env);
    assert_eq!(s.model, "foundry/claude-opus-5-5");
    assert_eq!(s.model_from, "config");
    // bise ships no URL for the proxy (a private one)
    assert_eq!(s.model_for("main").base_url, "");
    let s = Setup::from_text(
        Some("model = \"opus-5.5\"\n[providers.foundry]\nbase_url = \"https://foundry.example.net/anthropic/v1\"\n"),
        &no_env,
    );
    let r = s.model_for("main");
    assert_eq!(r.known, Known::Listed);
    assert_eq!(r.api, "anthropic");
    assert_eq!(r.base_url, "https://foundry.example.net/anthropic/v1");
    assert_eq!(r.key_env, "ANTHROPIC_FOUNDRY_API_KEY");
    assert_eq!(r.caps.context, 1_000_000);
}

/// the direct Anthropic API's beta flags (models.toml)
const ANTH_BETAS: &str = "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14";

#[test]
fn a_listed_model_takes_its_own_fields_then_its_providers() {
    let c = Catalog::builtin();
    let r = c.resolve("anthropic/claude-haiku-4-5");
    assert_eq!(r.known, Known::Listed);
    assert_eq!(r.caps, Caps { context: 200_000, max_output: 64_000, vision: true, reasoning: true, tools: true, thinking: "budget".into(), betas: ANTH_BETAS.into(), efforts: String::new(), effort: String::new(), cache_key: String::new(), cache_header: String::new(), headers_env: String::new(), key_command: String::new(), idle_timeout_sec: 0 });
    let r = c.resolve("mistral/mistral-large-latest");
    assert_eq!((r.caps.context, r.caps.reasoning, r.caps.vision), (262_144, false, true));
    let r = c.resolve("openai/gpt-6-astra");
    assert_eq!((r.caps.context, r.caps.max_output, r.caps.reasoning, r.caps.vision), (1_050_000, 128_000, true, true));
}

#[test]
fn an_unlisted_model_gets_its_providers_defaults() {
    let c = Catalog::builtin();
    let r = c.resolve("anthropic/claude-future-9");
    assert_eq!(r.known, Known::Unlisted);
    assert_eq!((r.provider.as_str(), r.id.as_str()), ("anthropic", "claude-future-9"));
    assert_eq!(r.base_url, "https://api.anthropic.com/v1");
    assert_eq!(r.caps.context, 1_000_000);
    // a provider with no model default: the global ones
    let r = c.resolve("openrouter/acme/model-9");
    assert_eq!(r.caps.context, DEFAULT_CAPS.context);
    assert_eq!(r.key_env, "OPENROUTER_API_KEY");
    // local: any name
    let r = c.resolve("ollama/qwen3-coder:30b");
    assert_eq!((r.known, r.base_url.as_str(), r.key_env.as_str()), (Known::Unlisted, "http://localhost:11434/v1", ""));
}

#[test]
fn an_unknown_provider_resolves_without_failing() {
    let c = Catalog::builtin();
    let r = c.resolve("nowhere/some-model");
    assert_eq!(r.known, Known::NoProvider);
    assert_eq!(r.base_url, "");
    assert_eq!(r.caps, DEFAULT_CAPS);
    // and odd names never panic
    for n in ["", "/", "a/", "/b", "  ", "x//y", "ü/ß"] {
        let _ = c.resolve(n);
    }
}

#[test]
fn model_ids_may_hold_slashes() {
    let c = Catalog::builtin();
    let r = c.resolve("openrouter/anthropic/claude-sonnet-5.5");
    assert_eq!((r.provider.as_str(), r.id.as_str(), r.known), ("openrouter", "anthropic/claude-sonnet-5.5", Known::Listed));
    let r = c.resolve("groq/some/new-model");
    assert_eq!((r.id.as_str(), r.known), ("some/new-model", Known::Unlisted));
}

#[test]
fn old_bare_names_keep_working() {
    let c = Catalog::builtin();
    assert_eq!(c.canonical("opus-5.5"), "foundry/claude-opus-5-5");
    assert_eq!(c.canonical("claude-opus-5-5"), "foundry/claude-opus-5-5");
    assert_eq!(c.canonical("zai-glm-5-3"), "mistral/zai-glm-5-3");
    assert_eq!(c.resolve("mistral-large-latest").known, Known::Listed);
    assert_eq!(c.resolve("claude-opus-5-5").caps.context, 1_000_000);
}

#[test]
fn the_config_overrides_a_model_key_by_key() {
    let s = setup(
        r#"
model = "anthropic/claude-haiku-4-5"
[models."anthropic/claude-haiku-4-5"]
context = 1000000
"#,
    );
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let r = s.model_for("main");
    assert_eq!(r.caps.context, 1_000_000);
    assert_eq!(r.caps.max_output, 64_000); // kept from the built-in entry
    assert_eq!(s.catalog.model("anthropic/claude-haiku-4-5").unwrap().source, Source::Config);
}

#[test]
fn the_config_adds_a_model_to_a_known_provider() {
    let s = setup(
        r#"
[models."groq/new-model"]
context = 65536
vision = true
"#,
    );
    let r = s.catalog.resolve("groq/new-model");
    assert_eq!(r.known, Known::Listed);
    assert_eq!((r.caps.context, r.caps.vision, r.caps.max_output), (65_536, true, 65_536)); // max_output: groq's
    assert_eq!(r.base_url, "https://api.groq.com/openai/v1");
}

#[test]
fn the_config_adds_a_whole_provider() {
    let s = setup(
        r#"
model = "work/qwen3-coder-480b"
[providers.work]
name = "Corp LLM"
base_url = "https://llm.corp.example/v1/"
key_env = "CORP_LLM_KEY"
context = 262144
[models."work/small"]
context = 32768
"#,
    );
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let r = s.model_for("main");
    assert_eq!(r.known, Known::Unlisted);
    assert_eq!(r.api, "openai-chat"); // the default family
    assert_eq!(r.base_url, "https://llm.corp.example/v1"); // no trailing '/'
    assert_eq!(r.key_env, "CORP_LLM_KEY");
    assert_eq!(r.caps.context, 262_144);
    assert_eq!(s.catalog.resolve("work/small").caps.context, 32_768);
    assert_eq!(s.catalog.provider("work").unwrap().source, Source::Config);
}

#[test]
fn the_config_overrides_a_known_provider_keeping_the_rest() {
    let s = setup(
        r#"
[providers.anthropic]
base_url = "https://proxy.example/anthropic/v1"
key_env = "MY_KEY"
"#,
    );
    let p = s.catalog.provider("anthropic").unwrap();
    assert_eq!((p.base_url.as_str(), p.key_env.as_str(), p.api.as_str()), ("https://proxy.example/anthropic/v1", "MY_KEY", "anthropic"));
    assert_eq!(s.catalog.resolve("anthropic/claude-haiku-4-5").caps.max_output, 64_000);
    // the order of the list is kept (models.toml's, not alphabetical)
    let ids: Vec<&str> = s.catalog.providers.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(&ids[..4], ["anthropic", "foundry", "openai", "google"]);
}

#[test]
fn a_model_may_speak_another_family_than_its_provider() {
    let s = setup("[models.\"openai/gpt-5-pro\"]\napi = \"openai-chat\"\n");
    assert_eq!(s.catalog.resolve("openai/gpt-5-pro").api, "openai-chat");
    assert_eq!(s.catalog.resolve("openai/gpt-5").api, "openai-responses");
}

#[test]
fn bad_entries_are_warnings_never_errors() {
    let s = setup(
        r#"
[providers.x]
api = "carrier-pigeon"
context = -3
colour = "blue"
[models."no-slash"]
context = 1
[models."x/m"]
vision = "yes"
[provider.y]
base_url = "http://y"
"#,
    );
    let w = s.catalog.warnings.join("\n");
    for needle in ["providers.x.api", "providers.x.context", "unknown key colour", "models.\"no-slash\"", "models.\"x/m\".vision", "[providers.<id>]"] {
        assert!(w.contains(needle), "{needle} not in:\n{w}");
    }
    let r = s.catalog.resolve("x/m");
    assert_eq!((r.api.as_str(), r.caps.context, r.caps.vision), ("openai-chat", DEFAULT_CAPS.context, false));
}

#[test]
fn a_config_that_is_not_toml_still_gives_the_model() {
    // the Bend reader accepts bare words; the TOML one does not
    let s = setup("model = groq/openai/gpt-oss-120b # fast\nagent_model = \"cerebras/gpt-oss-120b\"\nthinking = high\n");
    assert_eq!(s.model, "groq/openai/gpt-oss-120b");
    assert_eq!(s.agent_model, "cerebras/gpt-oss-120b");
    assert!(s.catalog.warnings[0].contains("not valid TOML"));
    // the built-in list is still there
    assert!(s.catalog.provider("anthropic").is_some());
}

#[test]
fn agent_model_falls_back_to_model() {
    let s = setup("model = \"openai/gpt-5\"\n");
    assert_eq!(s.agent_model, "openai/gpt-5");
    assert_eq!(s.agent_model_from, "model");
    assert_eq!(s.model_for("agent").name, "openai/gpt-5");
    let s = setup("model = \"openai/gpt-5\"\nagent_model = \"openai/gpt-5-mini\"\n");
    assert_eq!(s.model_for("main").name, "openai/gpt-5");
    assert_eq!(s.model_for("agent").name, "openai/gpt-5-mini");
    // an alias as agent_model is resolved too
    let s = setup("model = \"openai/gpt-5\"\nagent_model = \"opus-5.5\"\n");
    assert_eq!(s.agent_model, "foundry/claude-opus-5-5");
}

#[test]
fn the_env_wins_over_the_config() {
    let cfg = "model = \"openai/gpt-5\"\nagent_model = \"openai/gpt-5-mini\"\n";
    let env = |k: &str| match k {
        "BEND_MODEL" => Some("mistral/devstral-medium-latest".to_string()),
        _ => None,
    };
    let s = Setup::from_text(Some(cfg), &env);
    assert_eq!((s.model.as_str(), s.model_from), ("mistral/devstral-medium-latest", "BEND_MODEL"));
    assert_eq!(s.agent_model, "openai/gpt-5-mini"); // its own key
    let env = |k: &str| match k {
        "BISE_MODEL" => Some("xai/grok-4".to_string()),
        "BEND_MODEL" => Some("ignored/x".to_string()),
        "BISE_AGENT_MODEL" => Some("  ".to_string()), // empty = unset
        _ => None,
    };
    let s = Setup::from_text(Some("model = \"openai/gpt-5\"\n"), &env);
    assert_eq!(s.model, "xai/grok-4");
    assert_eq!(s.agent_model, "xai/grok-4"); // follows the effective model
}

#[test]
fn the_handoff_reads_back_as_the_same_catalog() {
    let s = setup(
        r#"
[providers.work]
base_url = "https://llm.corp.example/v1"
key_env = "CORP"
[models."work/q"]
context = 99000
[models."openai/gpt-5-pro"]
api = "openai-responses"
[aliases]
fast = "groq/openai/gpt-oss-120b"
"#,
    );
    let text = s.handoff_toml();
    let back = Setup::from_text(Some(&text), &no_env);
    assert!(back.catalog.warnings.iter().all(|w| !w.contains("unknown key")), "{:?}", back.catalog.warnings);
    for m in &s.catalog.models {
        assert_eq!(back.catalog.resolve(&m.name()), s.catalog.resolve(&m.name()), "{}", m.name());
    }
    for p in &s.catalog.providers {
        let n = format!("{}/unlisted", p.id);
        assert_eq!(back.catalog.resolve(&n), s.catalog.resolve(&n));
    }
    assert_eq!(back.catalog.canonical("fast"), "groq/openai/gpt-oss-120b");
    assert_eq!(back.catalog.default_model, s.catalog.default_model);
    // the model choice is not in it (the runtime reads config.toml)
    assert!(!text.lines().any(|l| l.starts_with("model =") || l.starts_with("agent_model =")));
}

#[test]
fn the_handoff_is_small_and_flat_for_the_bend_reader() {
    let text = Setup::from_text(None, &no_env).handoff_toml();
    assert!(text.lines().count() < 400, "{} lines", text.lines().count());
    // core/config.bend: one `key = value` per line, [section] headers,
    // # comments; no inline tables, arrays or multi-line strings
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || (l.starts_with('[') && l.ends_with(']')) {
            continue;
        }
        let (_, v) = l.split_once(" = ").unwrap_or_else(|| panic!("{l}"));
        assert!(!v.starts_with('{') && !v.starts_with('[') && !v.starts_with("\"\"\""), "{l}");
    }
    // a model section only carries what differs from its provider
    assert!(text.contains("[models.\"fireworks/accounts/fireworks/models/kimi-k3\"]\nvision = true\n\n"), "{text}");
}

#[test]
fn write_handoff_is_atomic_and_export_falls_back() {
    let dir = std::env::temp_dir().join(format!("bise-catalog-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = dir.join("config.toml");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&cfg, "model = \"openai/gpt-5\"\n").unwrap();
    let p = export_handoff(&cfg, &dir.join("cache"), &no_env).unwrap();
    assert_eq!(p, dir.join("cache/models.toml"));
    assert!(std::fs::read_to_string(&p).unwrap().contains("[providers.openai]"));
    let left: Vec<_> = std::fs::read_dir(dir.join("cache")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left.len(), 1, "{left:?}");
    // a cache dir that cannot be made (a file is in the way): the temp dir
    std::fs::write(dir.join("blocked"), "").unwrap();
    let p = export_handoff(&cfg, &dir.join("blocked/cache"), &no_env).unwrap();
    assert!(p.starts_with(std::env::temp_dir()));
    let _ = std::fs::remove_file(&p);
    // no config file at all: fine
    assert!(export_handoff(&dir.join("none.toml"), &dir.join("cache"), &no_env).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_listing_shows_keys_choices_and_warnings() {
    let s = setup("model = \"anthropic/claude-sonnet-5-5\"\nagent_model = \"work/x\"\n[providers.work]\nbase_url = \"http://w\"\n[models.\"z\"]\n");
    let env = |k: &str| (k == "ANTHROPIC_API_KEY").then(|| "sk".to_string());
    let mut store = crate::auth::Store::default();
    store.set("groq", "gsk-secret-1");
    let keys = crate::auth::Keys { env: &env, store: &store, files: &[] };
    let out = cli::render(&s, None, &keys, None);
    assert!(out.contains("model        anthropic/claude-sonnet-5-5  (config; listed)"), "{out}");
    assert!(out.contains("agent_model  work/x  (config; not listed: work's defaults)"), "{out}");
    assert!(out.contains("key: env ANTHROPIC_API_KEY"), "{out}");
    assert!(out.contains("key: auth.json"), "{out}");
    assert!(!out.contains("gsk-secret-1") && !out.contains("sk\n"), "{out}");
    assert!(out.contains("no key (OPENAI_API_KEY or 'bise login openai')"), "{out}");
    assert!(out.contains("ollama  Ollama (local) · openai-chat · no key needed"), "{out}");
    assert!(out.contains("not usable yet (BISE-149)"), "{out}");
    assert!(out.contains("from config.toml"), "{out}");
    assert!(out.contains("warning: config.toml: models.\"z\""), "{out}");
    // a filter keeps matching providers and models
    let out = cli::render(&s, Some("gpt-oss"), &keys, None);
    assert!(out.contains("groq/openai/gpt-oss-120b") && out.contains("cerebras/gpt-oss-120b"), "{out}");
    assert!(!out.contains("anthropic/claude-opus-5-5"), "{out}");
    assert!(!out.contains("no provider or model matches"), "{out}");
    // qa-explore D: a filter with no match says so
    let out = cli::render(&s, Some("zzz"), &keys, None);
    assert!(out.contains("no provider or model matches 'zzz'"), "{out}");
    let s = setup("model = \"nowhere/x\"\n");
    let empty = crate::auth::Store::default();
    let keys = crate::auth::Keys { env: &no_env, store: &empty, files: &[] };
    assert!(cli::render(&s, None, &keys, None).contains("unknown provider 'nowhere'"));
}

#[test]
fn token_counts_read_short() {
    assert_eq!(cli::tokens(128_000), "128k");
    assert_eq!(cli::tokens(1_000_000), "1M");
    assert_eq!(cli::tokens(1_047_576), "1.05M");
    assert_eq!(cli::tokens(950), "950");
    assert_eq!(cli::tokens(32_768), "32k");
}

#[test]
fn small_model_order() {
    // the agents' provider's small model by default
    let s = setup("model = \"anthropic/claude-opus-4-5\"\n");
    assert_eq!(s.small_model, "anthropic/claude-haiku-4-5");
    assert_eq!(s.small_model_from, "provider");
    // it follows agent_model's provider, not model's
    let s = setup("model = \"anthropic/claude-opus-4-5\"\nagent_model = \"openai/gpt-6.1-sol\"\n");
    assert_eq!(s.small_model, "openai/gpt-6-luna");
    // foundry has one; no model: no small model either (BISE-266)
    let s = setup("model = \"opus-5.5\"\n");
    assert_eq!(s.small_model, "foundry/claude-haiku-4-5");
    assert_eq!(setup("").small_model, "");
    // a provider without one: agent_model
    let s = setup("model = \"cerebras/gpt-oss-120b\"\n");
    assert_eq!(s.small_model, "cerebras/gpt-oss-120b");
    assert_eq!(s.small_model_from, "agent_model");
    // config, then env, win
    let s = setup("small_model = \"openai/gpt-5-mini\"\n");
    assert_eq!(s.small_model, "openai/gpt-5-mini");
    assert_eq!(s.small_model_from, "config");
    let s = Setup::from_text(Some("small_model = \"openai/gpt-5-mini\"\n"), &|k| {
        (k == "BISE_SMALL_MODEL").then(|| "opus-5.5".to_string())
    });
    assert_eq!(s.small_model, "foundry/claude-opus-5-5");
    assert_eq!(s.small_model_from, "BISE_SMALL_MODEL");
    // a provider of the config may name its own
    let s = setup("model = \"acme/big\"\n[providers.acme]\nbase_url = \"http://x\"\nsmall_model = \"tiny\"\n");
    assert_eq!(s.small_model, "acme/tiny");
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
}

#[test]
fn thinking_and_betas_per_model_reach_the_handoff() {
    // built in: the direct API's haiku thinks with a budget, the foundry proxy
    // keeps the family's defaults (nothing written: today's body/headers)
    let c = Catalog::builtin();
    let r = c.resolve("anthropic/claude-haiku-4-5");
    assert_eq!((r.caps.thinking.as_str(), r.caps.betas.as_str(), r.caps.max_output), ("budget", ANTH_BETAS, 64_000));
    let f = c.resolve("foundry/claude-opus-5-5");
    assert_eq!((f.caps.thinking.as_str(), f.caps.betas.as_str(), f.caps.max_output), ("", "", 32_768));
    // config: per model, inherited from the provider, bad values warned
    let cfg = "[models.\"anthropic/claude-opus-4-6\"]\nthinking = \"none\"\n[models.\"anthropic/x\"]\nthinking = \"high\"\nbetas = 3\n";
    let s = Setup::from_text(Some(cfg), &|_| None);
    assert_eq!(s.catalog.resolve("anthropic/claude-opus-4-6").caps.thinking, "none");
    assert_eq!(s.catalog.resolve("anthropic/x").caps.thinking, "adaptive", "a bad value keeps the provider's");
    assert!(s.catalog.warnings.iter().any(|w| w.contains("thinking: one of adaptive, budget, none")), "{:?}", s.catalog.warnings);
    assert!(s.catalog.warnings.iter().any(|w| w.contains("betas: a string")), "{:?}", s.catalog.warnings);
    let h = s.handoff_toml();
    let anth = &h[h.find("[providers.anthropic]").unwrap()..];
    let anth = &anth[..anth[1..].find("\n[").unwrap()];
    assert!(anth.contains("thinking = \"adaptive\"\n") && anth.contains(&format!("betas = \"{}\"", ANTH_BETAS)), "{anth}");
    let foundry = &h[h.find("[providers.foundry]").unwrap()..];
    let foundry = &foundry[..foundry[1..].find("\n[").unwrap()];
    assert!(!foundry.contains("thinking") && !foundry.contains("betas"), "{foundry}");
    assert!(h.contains("[models.\"anthropic/claude-opus-4-6\"]\nthinking = \"none\"\n"), "{h}");
}

#[test]
fn prices_come_from_the_model_then_its_provider() {
    let c = Catalog::builtin();
    let p = c.resolve("anthropic/claude-sonnet-5-5").price;
    assert_eq!(p, Price { input: Some(2_000_000), output: Some(10_000_000), cache_read: Some(200_000), cache_write: Some(2_500_000) });
    // nothing listed: no price, no cost
    assert_eq!(c.resolve("fireworks/accounts/fireworks/models/glm-5p3").price, Price::default());
    assert_eq!(c.resolve("anthropic/claude-future-9").price, Price::default());
    assert_eq!(c.resolve("nowhere/m").price.cost(10, 10, 0, 0), None);
    // a config provider's prices are its models' defaults; a model's win
    let s = setup(
        "[providers.acme]\nbase_url = \"http://x\"\ninput_price = 1\noutput_price = 2.5\n\
         [models.\"acme/big\"]\noutput_price = 10\n",
    );
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let small = s.catalog.resolve("acme/small").price;
    assert_eq!((small.input, small.output), (Some(1_000_000), Some(2_500_000)));
    let big = s.catalog.resolve("acme/big").price;
    assert_eq!((big.input, big.output), (Some(1_000_000), Some(10_000_000)));
    // cost: cached tokens at their price, the rest at the input price
    let c = big.cost(1_000_000, 100_000, 0, 0).unwrap();
    assert!((c - 2.0).abs() < 1e-9, "{c}");
    // bad prices are warnings
    let s = setup("[models.\"acme/x\"]\ninput_price = -1\noutput_price = \"cheap\"\n");
    assert_eq!(s.catalog.warnings.len(), 2, "{:?}", s.catalog.warnings);
}

#[test]
fn the_default_threshold_is_80_percent_of_the_window() {
    let c = Catalog::builtin();
    assert_eq!(c.default_threshold("foundry/claude-opus-5-5"), 800_000);
    assert_eq!(c.default_threshold("anthropic/claude-haiku-4-5"), 160_000);
    assert_eq!(c.default_threshold("nowhere/m"), 102_400);
    // the same rounding as runtime/provider-pure.bend threshold_of
    assert_eq!(threshold_of(131_072), 104_857);
    assert_eq!(threshold_of(0), 0);
}

/// BISE-300: config `compaction_threshold` is tokens or a share of the
/// window, never above 80 % of it; the old `threshold` key is not read
/// (a warning says to rename it). The same cases as the Bend laws
/// `threshold_tokens_or_percent` / `threshold_capped_at_80_percent` /
/// `threshold_bad_values_default`.
#[test]
fn the_compaction_threshold_is_tokens_or_a_share_capped_at_80_percent() {
    assert_eq!(compaction_threshold(Some("450000"), 1_000_000), 450_000);
    assert_eq!(compaction_threshold(Some("45%"), 1_000_000), 450_000);
    assert_eq!(compaction_threshold(Some("45%"), 200_000), 90_000);
    assert_eq!(compaction_threshold(Some(" 45 % "), 128_000), 57_600);
    // the cap: a 1M number on a 200k model, a share above 80 %
    assert_eq!(compaction_threshold(Some("450000"), 200_000), 160_000);
    assert_eq!(compaction_threshold(Some("95%"), 1_000_000), 800_000);
    assert_eq!(compaction_threshold(Some("250%"), 200_000), 160_000);
    assert_eq!(compaction_threshold(Some("99999999999999999999"), 200_000), 160_000);
    // no threshold: the default
    for bad in ["0", "0%", "lots", "4.5%", "%", "", "-5", "+5"] {
        assert_eq!(compaction_threshold(Some(bad), 200_000), 160_000, "{bad}");
    }
    assert_eq!(compaction_threshold(None, 200_000), 160_000);

    let s = Setup::from_text(Some("compaction_threshold = 450000\n"), &|_| None);
    assert_eq!(s.compaction_threshold.as_deref(), Some("450000"));
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let s = Setup::from_text(Some("compaction_threshold = \"45%\"\n"), &|_| None);
    assert_eq!(s.compaction_threshold.as_deref(), Some("45%"));
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let s = Setup::from_text(Some("threshold = 450000\n"), &|_| None);
    assert_eq!(s.compaction_threshold, None);
    assert_eq!(s.catalog.warnings, ["config.toml: threshold is no longer read: rename it compaction_threshold"]);
    let s = Setup::from_text(Some("compaction_threshold = \"lots\"\n"), &|_| None);
    assert!(s.catalog.warnings[0].starts_with("config.toml: compaction_threshold: a number of tokens"), "{:?}", s.catalog.warnings);
    let s = Setup::from_text(Some("compaction_threshold = 4.5\n"), &|_| None);
    assert_eq!(s.compaction_threshold, None);
    assert_eq!(s.catalog.warnings.len(), 1);
}

// ---- voice (BISE-130) ----

#[test]
fn the_voice_model_defaults_to_voxtral_and_the_config_picks_another() {
    let s = Setup::from_text(None, &no_env);
    assert_eq!(s.voice.model, "mistral/voxtral-transcribe-3");
    assert_eq!((s.voice.from, s.voice.language.clone()), ("default", None));
    let r = s.catalog.resolve_stt(&s.voice.model);
    assert_eq!((r.api.as_str(), r.base_url.as_str(), r.key_env.as_str()), ("mistral", "https://api.mistral.ai/v1", "MISTRAL_API_KEY"));
    assert_eq!(r.known, Known::Listed);
    let s = setup("[voice]\nmodel = \"groq/whisper-large-v3-turbo\"\nlanguage = \"fr\"\nvocabulary = [\"bise\", \" config.toml \", \"\"]\n");
    assert_eq!(s.voice.model, "groq/whisper-large-v3-turbo");
    assert_eq!(s.voice.language.as_deref(), Some("fr"));
    assert_eq!(s.voice.vocabulary, vec!["bise".to_string(), "config.toml".into()]);
    assert_eq!(s.catalog.resolve_stt(&s.voice.model).api, "openai");
    // a comma-separated string works too; "auto" is no language
    let s = setup("[voice]\nlanguage = \"auto\"\nvocabulary = \"a, b\"\n");
    assert_eq!((s.voice.language.clone(), s.voice.vocabulary.len()), (None, 2));
    // the env wins; a bare name goes to the default's provider
    let env = |k: &str| (k == "BISE_VOICE_MODEL").then(|| "voxtral-transcribe-3".to_string());
    let s = Setup::from_text(Some("[voice]\nmodel = \"openai/whisper-1\"\n"), &env);
    assert_eq!((s.voice.model.as_str(), s.voice.from), ("mistral/voxtral-transcribe-3", "BISE_VOICE_MODEL"));
    // the main model is not the voice one (sections are separate)
    let s = setup("model = \"openai/gpt-5\"\n[voice]\nmodel = \"deepgram/nova-3\"\n");
    assert_eq!((s.model.as_str(), s.voice.model.as_str()), ("openai/gpt-5", "deepgram/nova-3"));
}

#[test]
fn a_bad_voice_table_is_a_warning() {
    let s = setup("[voice]\nmodel = 3\nlanguage = [1]\nvocabulary = 2\npitch = 1\n");
    let w = s.catalog.warnings.join("\n");
    for k in ["voice.model", "voice.language", "voice.vocabulary", "voice.pitch: unknown key"] {
        assert!(w.contains(k), "{k}: {w}");
    }
    assert_eq!(s.voice.model, "mistral/voxtral-transcribe-3");
    let s = setup("voice = \"x\"\n");
    assert!(s.catalog.warnings.join("\n").contains("voice: not a table"));
    let s = setup("[providers.x]\nstt = \"nope\"\nkind = \"tts\"\n");
    let w = s.catalog.warnings.join("\n");
    assert!(w.contains("providers.x.stt: one of mistral, openai, elevenlabs, deepgram") && w.contains("providers.x.kind"), "{w}");
}

#[test]
fn the_voice_job_takes_the_chat_keys_resolution() {
    let mut store = crate::auth::Store::default();
    store.set("elevenlabs", "xi-secret");
    let files = vec![crate::auth::EnvFile::parse("/h/.vibe/.env".into(), "MISTRAL_API_KEY=m-file\n")];
    let env = |k: &str| (k == "OPENAI_API_KEY").then(|| "sk-env".to_string());
    let keys = crate::auth::Keys { env: &env, store: &store, files: &files };
    let job = setup("[voice]\nlanguage = \"fr\"\nvocabulary = [\"bise\"]\n").voice_job(&keys).unwrap();
    assert_eq!(
        (job.api.as_str(), job.base_url.as_str(), job.model.as_str(), job.key.as_str()),
        ("mistral", "https://api.mistral.ai/v1", "voxtral-transcribe-3", "m-file")
    );
    assert_eq!((job.language.as_deref(), job.vocabulary.clone()), (Some("fr"), vec!["bise".to_string()]));
    assert!(!format!("{:?}", job).contains("m-file"));
    let job = setup("[voice]\nmodel = \"openai/gpt-4o-transcribe\"\n").voice_job(&keys).unwrap();
    assert_eq!((job.api.as_str(), job.key.as_str()), ("openai", "sk-env"));
    let job = setup("[voice]\nmodel = \"elevenlabs/scribe_v2\"\n").voice_job(&keys).unwrap();
    assert_eq!((job.api.as_str(), job.base_url.as_str(), job.key.as_str()), ("elevenlabs", "https://api.elevenlabs.io/v1", "xi-secret"));
    // the failures say what to do, never with a key
    let e = setup("[voice]\nmodel = \"deepgram/nova-3\"\n").voice_job(&keys).unwrap_err();
    assert_eq!(e, "voice transcription needs an API key: set DEEPGRAM_API_KEY or run 'bise login deepgram'");
    let e = setup("[voice]\nmodel = \"nowhere/x\"\n").voice_job(&keys).unwrap_err();
    assert!(e.contains("unknown provider 'nowhere'"), "{e}");
    let e = setup("[voice]\nmodel = \"anthropic/claude-haiku-4-5\"\n").voice_job(&keys).unwrap_err();
    assert!(e.contains("anthropic does not transcribe"), "{e}");
    // a custom OpenAI-compatible server is data
    let s = setup("[providers.local]\nbase_url = \"http://127.0.0.1:9/v1/\"\nstt = \"openai\"\n[models.\"local/whisper\"]\nkind = \"stt\"\n[voice]\nmodel = \"local/whisper\"\n");
    let job = s.voice_job(&keys).unwrap();
    assert_eq!((job.api.as_str(), job.base_url.as_str(), job.key.as_str()), ("openai", "http://127.0.0.1:9/v1", ""));
    assert_eq!(s.catalog.resolve_stt("local/whisper").known, Known::Listed);
}

#[test]
fn voice_entries_stay_out_of_the_chat_list_and_the_handoff() {
    let s = Setup::from_text(None, &no_env);
    let h = s.handoff_toml();
    assert!(!h.contains("elevenlabs") && !h.contains("deepgram") && !h.contains("voxtral") && !h.contains("whisper"), "{h}");
    assert!(h.contains("[providers.mistral]") && h.contains("[providers.groq]"));
    let env = |k: &str| (k == "MISTRAL_API_KEY").then(|| "m".to_string());
    let store = crate::auth::Store::default();
    let keys = crate::auth::Keys { env: &env, store: &store, files: &[] };
    let out = cli::render(&s, None, &keys, None);
    assert!(out.contains("voice        mistral/voxtral-transcribe-3  (default; listed)"), "{out}");
    assert!(!out.contains("voxtral-mini-latest"), "{out}");
    assert!(out.contains("language auto"), "{out}");
    let (chat, voice) = out.split_once("\nvoice (speech to text").unwrap();
    assert!(!chat.contains("whisper") && !chat.contains("elevenlabs  ElevenLabs"), "{chat}");
    for l in [
        "  mistral  Mistral · mistral · key: env MISTRAL_API_KEY",
        "    mistral/voxtral-transcribe-3",
        "  elevenlabs  ElevenLabs · elevenlabs · no key (ELEVENLABS_API_KEY or 'bise login elevenlabs')",
        "    deepgram/nova-3",
        "    groq/whisper-large-v3-turbo",
    ] {
        assert!(voice.contains(l), "{l}: {voice}");
    }
    // 'voice' as the filter: only the voice providers
    let out = cli::render(&s, Some("voice"), &keys, None);
    assert!(out.contains("openai/gpt-4o-transcribe") && !out.contains("anthropic  Anthropic"), "{out}");
    // elevenlabs and deepgram take a key through 'bise login'
    assert!(crate::auth_cli::check_provider(&s.catalog, "deepgram").is_ok());
}

// ---- BISE-135: the effort, the session's choice ----

#[test]
fn efforts_follow_the_family_and_the_model() {
    let s = setup("");
    let c = &s.catalog;
    let opus = c.resolve("foundry/claude-opus-5-5");
    assert_eq!(opus.efforts(), ANTHROPIC_EFFORTS.map(String::from).to_vec());
    assert_eq!(opus.default_effort(), "high");
    assert_eq!(c.resolve("groq/openai/gpt-oss-120b").efforts(), CHAT_EFFORTS.map(String::from).to_vec());
    // OpenAI's GPT-6: its words, luna's with none
    assert_eq!(c.resolve("openai/gpt-6-astra").efforts(), ["low", "medium", "high", "xhigh", "max"]);
    assert_eq!(c.resolve("openai/gpt-6-luna").efforts()[0], "none");
    // the provider's own list
    let glm = c.resolve("mistral/zai-glm-5-3");
    assert_eq!(glm.efforts(), ["none", "high"]);
    assert_eq!(glm.effort_for("low"), "high", "a word it does not take: its default");
    assert_eq!(glm.effort_for("none"), "none");
    // no reasoning, no effort
    let gpt41 = c.resolve("mistral/mistral-large-latest");
    assert!(gpt41.efforts().is_empty());
    assert_eq!(gpt41.effort_for("high"), "");
    // config: a model's own list and default
    let s = setup(
        "[models.\"openai/o9\"]\nefforts = \"minimal, low,high\"\neffort = \"low\"\n\
         [models.\"anthropic/claude-x\"]\nthinking = \"none\"\n",
    );
    let o9 = s.catalog.resolve("openai/o9");
    assert_eq!(o9.efforts(), ["minimal", "low", "high"]);
    assert_eq!(o9.default_effort(), "low");
    assert!(s.catalog.resolve("anthropic/claude-x").efforts().is_empty(), "thinking none: no effort");
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let bad = setup("[models.\"openai/o9\"]\nefforts = \"a b\"\neffort = \"\"\n");
    assert_eq!(bad.catalog.warnings.len(), 2, "{:?}", bad.catalog.warnings);
}

#[test]
fn a_session_choice_wins_over_config_and_env() {
    let env = |k: &str| (k == "BISE_MODEL").then(|| "openai/gpt-5".to_string());
    let s = Setup::from_text(Some("agent_model = \"mistral/zai-glm-5-3\"\nreasoning_effort = \"medium\"\n"), &env);
    let none = Choice::default();
    let main = s.in_use("main", &none);
    assert_eq!((main.model.name.as_str(), main.model_from), ("openai/gpt-5", "BISE_MODEL"));
    assert_eq!((main.effort.as_str(), main.effort_from), ("medium", "config"));
    // the agent's model does not take medium: its default
    let agent = s.in_use("agent", &none);
    assert_eq!((agent.model.name.as_str(), agent.model_from), ("mistral/zai-glm-5-3", "config"));
    assert_eq!((agent.effort.as_str(), agent.effort_from), ("high", "model"));
    // the session's choice first, aliases resolved
    let c = Choice { model: "opus-5.5".into(), effort: "max".into() };
    let u = s.in_use("agent", &c);
    assert_eq!((u.model.name.as_str(), u.model_from), ("foundry/claude-opus-5-5", "session"));
    assert_eq!((u.effort.as_str(), u.effort_from), ("max", "session"));
    // an effort alone keeps the role's model
    let e = s.in_use("main", &Choice { model: String::new(), effort: "low".into() });
    assert_eq!((e.model.name.as_str(), e.effort.as_str()), ("openai/gpt-5", "low"));
    // agent_reasoning_effort for the sub-agents only
    let s = setup("model = \"opus-5.5\"\nreasoning_effort = \"low\"\nagent_reasoning_effort = \"max\"\n");
    assert_eq!(s.in_use("main", &none).effort, "low");
    assert_eq!(s.in_use("agent", &none).effort, "max");
}

#[test]
fn a_choice_round_trips_through_its_file() {
    let dir = std::env::temp_dir().join(format!("bise-choice-{}", std::process::id()));
    let path = dir.join("agent/choice.toml");
    assert_eq!(Choice::read(&path), Choice::default(), "no file: nothing picked");
    let c = Choice { model: "anthropic/claude-sonnet-4-5".into(), effort: "low".into() };
    c.write(&path).unwrap();
    assert_eq!(Choice::read(&path), c);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("model = \"anthropic/claude-sonnet-4-5\"\nreasoning_effort = \"low\"\n"), "{text}");
    Choice { model: String::new(), effort: "max".into() }.write(&path).unwrap();
    assert_eq!(Choice::read(&path), Choice { model: String::new(), effort: "max".into() });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_config_key_keeps_the_rest_of_the_file() {
    let t = "# mine\nmodel = \"a/b\" # old\nthreshold = 5\n\n[models.\"x/y\"]\nmodel = \"not top\"\n";
    assert_eq!(
        set_config_key(t, "model", "c/d"),
        "# mine\nmodel = \"c/d\"\nthreshold = 5\n\n[models.\"x/y\"]\nmodel = \"not top\"\n"
    );
    assert_eq!(
        set_config_key(t, "agent_model", "e/f"),
        "# mine\nmodel = \"a/b\" # old\nthreshold = 5\n\nagent_model = \"e/f\"\n[models.\"x/y\"]\nmodel = \"not top\"\n"
    );
    assert_eq!(set_config_key("", "model", "c/d"), "model = \"c/d\"\n");
    // what the Setup reads back
    let s = setup(&set_config_key(t, "agent_model", "e/f"));
    assert_eq!(s.agent_model, "e/f");
}

#[test]
fn efforts_reach_the_handoff() {
    let s = setup("[models.\"openai/o9\"]\nefforts = \"low,high\"\neffort = \"low\"\n");
    let h = s.handoff_toml();
    assert!(h.contains("[providers.mistral]") && h.contains("efforts = \"none,high\""), "{h}");
    let o9 = h.split("[models.\"openai/o9\"]").nth(1).unwrap();
    assert!(o9.contains("efforts = \"low,high\"\neffort = \"low\"\n"), "{o9}");
}

#[test]
fn with_model_sets_the_top_level_line_only() {
    use crate::with_model;
    assert_eq!(with_model("", "openai/gpt-5.5"), "model = \"openai/gpt-5.5\"\n");
    // replaced in place, the rest kept
    let t = "# mine\nmodel = \"foundry/x\"\nsmall_model = \"a/b\"\n\n[voice]\nmodel = \"mistral/v\"\n";
    assert_eq!(
        with_model(t, "mistral/m"),
        "# mine\nmodel = \"mistral/m\"\nsmall_model = \"a/b\"\n\n[voice]\nmodel = \"mistral/v\"\n"
    );
    // none yet: before the first table ([voice] model untouched)
    let t = "[voice]\nmodel = \"mistral/v\"\n";
    let out = with_model(t, "anthropic/claude-opus-5-5");
    assert!(out.starts_with("model = \"anthropic/claude-opus-5-5\"\n\n[voice]\nmodel = \"mistral/v\""), "{}", out);
    let s = crate::Setup::from_text(Some(&out), &|_| None);
    assert_eq!(s.model, "anthropic/claude-opus-5-5");
    assert_eq!(s.voice.model, "mistral/v");
}

#[test]
fn every_offered_provider_has_a_keys_page_and_a_model() {
    let c = crate::Catalog::builtin();
    for p in c.providers.iter().filter(|p| !p.key_env.is_empty() && p.needs.is_empty() && p.chats() && !p.hidden) {
        assert!(p.keys_url.starts_with("https://"), "{}: keys_url", p.id);
        assert!(!p.model.is_empty() && !p.hint.is_empty(), "{}: model and hint", p.id);
    }
    assert!(c.provider("foundry").unwrap().hidden);
    // the user's pick (2026-09-30): five providers, in this order; the
    // others stay usable, not offered
    let offered: Vec<&str> = c
        .providers
        .iter()
        .filter(|p| !p.key_env.is_empty() && p.needs.is_empty() && p.chats() && !p.hidden)
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(offered, ["anthropic", "openai", "google", "mistral", "openrouter"]);
    for id in ["xai", "deepseek", "groq", "together", "fireworks", "cerebras"] {
        assert!(c.provider(id).unwrap().hidden, "{id}");
        assert_eq!(c.resolve(&format!("{id}/x")).known, Known::Unlisted, "{id} still resolves");
    }
}

#[test]
fn cache_routing_keys_reach_the_handoff() {
    // BISE-268: built in, only where the provider's docs name one
    let c = Catalog::builtin();
    let key = |id: &str| {
        let r = c.resolve(id);
        (r.caps.cache_key, r.caps.cache_header)
    };
    assert_eq!(key("openai/gpt-5.5"), ("prompt_cache_key".into(), String::new()));
    assert_eq!(key("mistral/mistral-medium-latest"), ("prompt_cache_key".into(), String::new()));
    assert_eq!(key("xai/grok-4.7"), (String::new(), "x-grok-conv-id".into()));
    assert_eq!(key("foundry/claude-opus-5-5"), (String::new(), String::new()));
    // config: a model overrides its provider, a bad name is warned
    let cfg = "[models.\"openai/x\"]\ncache_key = \"user\"\n[models.\"openai/y\"]\ncache_header = \"a b\"\n";
    let s = Setup::from_text(Some(cfg), &|_| None);
    assert_eq!(s.catalog.resolve("openai/x").caps.cache_key, "user");
    assert_eq!(s.catalog.resolve("openai/y").caps.cache_header, "", "a bad name sets nothing");
    assert!(s.catalog.warnings.iter().any(|w| w.contains("cache_header: a field or header name")), "{:?}", s.catalog.warnings);
    let h = s.handoff_toml();
    let oai = &h[h.find("[providers.openai]").unwrap()..];
    let oai = &oai[..oai[1..].find("\n[").unwrap()];
    assert!(oai.contains("cache_key = \"prompt_cache_key\""), "{oai}");
    assert!(h.contains("cache_key = \"user\"\n"), "{h}");
}

#[test]
fn gateway_keys_reach_the_handoff() {
    // a gateway in front of Anthropic, set up like Claude Code's
    // ANTHROPIC_BASE_URL + ANTHROPIC_CUSTOM_HEADERS + apiKeyHelper
    let cfg = "[providers.gw]\napi = \"anthropic\"\nbase_url = \"https://gw.example/v1\"\nkey_env = \"\"\n\
               headers_env = \"ANTHROPIC_CUSTOM_HEADERS\"\nkey_command = \"tool auth token gw\"\n\
               [models.\"gw/b\"]\nkey_command = \"other\"\n\
               [models.\"gw/bad\"]\nheaders_env = \"A-B\"\nkey_command = 'echo \"k\"'\n";
    let s = Setup::from_text(Some(cfg), &|_| None);
    let a = s.catalog.resolve("gw/a").caps;
    assert_eq!((a.headers_env.as_str(), a.key_command.as_str()), ("ANTHROPIC_CUSTOM_HEADERS", "tool auth token gw"));
    assert_eq!(s.catalog.resolve("gw/b").caps.key_command, "other", "a model overrides its provider");
    assert_eq!(s.catalog.resolve("gw/bad").caps.headers_env, "ANTHROPIC_CUSTOM_HEADERS", "a bad name sets nothing");
    assert!(s.catalog.warnings.iter().any(|w| w.contains("headers_env: an env variable name")), "{:?}", s.catalog.warnings);
    assert_eq!(s.catalog.resolve("gw/bad").caps.key_command, "tool auth token gw", "a '\"' sets nothing");
    assert!(s.catalog.warnings.iter().any(|w| w.contains("key_command: a shell command")), "{:?}", s.catalog.warnings);
    assert_eq!(Catalog::builtin().resolve("anthropic/claude-opus-5-5").caps.key_command, "", "none built in");
    let h = s.handoff_toml();
    let gw = &h[h.find("[providers.gw]").unwrap()..];
    assert!(gw.contains("headers_env = \"ANTHROPIC_CUSTOM_HEADERS\"\n"), "{gw}");
    assert!(gw.contains("key_command = \"tool auth token gw\"\n"), "{gw}");
    assert!(h.contains("key_command = \"other\"\n"), "{h}");
}

#[test]
fn idle_timeout_sec_per_provider() {
    // provider-timeout: a slow local model or gateway waits longer
    // (the runtime reads the same key: provider-pure.bend stream_wait)
    let cfg = "[providers.slow]\napi = \"openai-chat\"\nbase_url = \"http://localhost:11434/v1\"\nkey_env = \"\"\n\
               idle_timeout_sec = 600\n\
               [models.\"slow/quick\"]\nidle_timeout_sec = 30\n\
               [models.\"slow/bad\"]\nidle_timeout_sec = 0\n\
               [models.\"slow/huge\"]\nidle_timeout_sec = 100000\n";
    let s = Setup::from_text(Some(cfg), &|_| None);
    assert_eq!(s.catalog.resolve("slow/a").caps.idle_timeout_sec, 600);
    assert_eq!(s.catalog.resolve("slow/quick").caps.idle_timeout_sec, 30, "a model overrides its provider");
    assert_eq!(s.catalog.resolve("slow/bad").caps.idle_timeout_sec, 600, "0 sets nothing");
    assert_eq!(s.catalog.resolve("slow/huge").caps.idle_timeout_sec, 600, "past a day sets nothing");
    let w: Vec<_> = s.catalog.warnings.iter().filter(|w| w.contains("idle_timeout_sec: a number of seconds")).collect();
    assert_eq!(w.len(), 2, "{:?}", s.catalog.warnings);
    assert!(!s.catalog.warnings.iter().any(|w| w.contains("unknown key")), "{:?}", s.catalog.warnings);
    assert_eq!(Catalog::builtin().resolve("ollama/llama3").caps.idle_timeout_sec, 0, "none built in: the runtime's 90 s");
    let h = s.handoff_toml();
    let p = &h[h.find("[providers.slow]").unwrap()..];
    assert!(p.contains("idle_timeout_sec = 600\n"), "{p}");
    assert!(h.contains("idle_timeout_sec = 30\n"), "{h}");
}

// ---- model roles (BISE-298) ----

#[test]
fn roles_come_from_the_roles_table_then_the_old_keys() {
    // the line form
    let s = setup("[roles]\nmain = \"mistral/a\"\nagents = \"openai/b\"\nsmall = \"mistral/c\"\nvoice = \"openai/gpt-transcribe\"\nclassify = \"mistral/d\"\n");
    assert_eq!((s.model.as_str(), s.agent_model.as_str(), s.small_model.as_str()), ("mistral/a", "openai/b", "mistral/c"));
    assert_eq!((s.voice.model.as_str(), s.voice.from), ("openai/gpt-transcribe", "config"));
    assert_eq!((s.classify_model.as_str(), s.classify_model_from), ("mistral/d", "config"));
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    // the table form, with the efforts and the voice settings
    let s = setup(
        "[roles.main]\nmodel = \"mistral/a\"\neffort = \"high\"\n[roles.agents]\nmodel = \"openai/b\"\neffort = \"low\"\n\
         [roles.voice]\nmodel = \"mistral/voxtral-mini-latest\"\nlanguage = \"fr\"\nvocabulary = [\"bise\"]\n",
    );
    assert_eq!((s.model.as_str(), s.effort.as_str()), ("mistral/a", "high"));
    assert_eq!((s.agent_model.as_str(), s.agent_effort.as_str()), ("openai/b", "low"));
    assert_eq!((s.voice.language.as_deref(), s.voice.vocabulary.clone()), (Some("fr"), vec!["bise".to_string()]));
    // [roles] wins over the old keys; the old keys still work alone
    let s = setup("model = \"mistral/old\"\nagent_model = \"mistral/old2\"\n[roles]\nmain = \"mistral/new\"\n[voice]\nmodel = \"openai/whisper-1\"\nlanguage = \"en\"\n");
    assert_eq!((s.model.as_str(), s.agent_model.as_str()), ("mistral/new", "mistral/old2"));
    assert_eq!((s.voice.model.as_str(), s.voice.language.as_deref()), ("openai/whisper-1", Some("en")));
    // the env wins over both
    let env = |k: &str| (k == "BISE_AGENT_MODEL").then(|| "openai/env".to_string());
    let s = Setup::from_text(Some("[roles]\nagents = \"openai/b\"\n"), &env);
    assert_eq!((s.agent_model.as_str(), s.agent_model_from), ("openai/env", "BISE_AGENT_MODEL"));
    // the fallbacks: agents = main, small = the agents' provider's, classify = small
    let s = setup("[roles]\nmain = \"mistral/mistral-medium-latest\"\n");
    assert_eq!(s.role_model(roles::AGENTS), ("mistral/mistral-medium-latest".to_string(), roles::Source::SameAs(roles::MAIN)));
    assert_eq!(s.role_model(roles::SMALL), ("mistral/mistral-small-latest".to_string(), roles::Source::Auto));
    assert_eq!(s.role_model(roles::CLASSIFY).1, roles::Source::Auto);
    assert_eq!(s.role_model(roles::VOICE), ("mistral/voxtral-transcribe-3".to_string(), roles::Source::Auto));
}

#[test]
fn a_wrong_roles_table_says_what_is_wrong() {
    let w = |cfg: &str| setup(cfg).catalog.warnings;
    assert_eq!(w("roles = 3\n"), vec!["config.toml: roles: not a table ([roles] then main = \"provider/model\")"]);
    assert_eq!(w("[roles]\nboss = \"a/b\"\n"), vec!["config.toml: roles.boss: unknown role (main, agents, small, voice, classify)"]);
    assert_eq!(w("[roles.main]\nmodle = \"a/b\"\n"), vec!["config.toml: roles.main.modle: unknown key (model, effort)"]);
    assert_eq!(w("[roles]\nvoice = 3\n"), vec!["config.toml: roles.voice: not a table ([voice] then model = ...)"]);
}

#[test]
fn a_role_is_written_in_roles_and_its_old_key_goes() {
    use roles::with_role;
    // a new file
    assert_eq!(with_role("", "main", "mistral/a"), "[roles]\nmain = \"mistral/a\"\n");
    // the old key goes, the rest stays; [roles] at the end
    let t = "# mine\nmodel = \"mistral/old\"\nreasoning_effort = \"high\"\n\n[providers.x]\nname = \"X\"\n";
    assert_eq!(
        with_role(t, "main", "mistral/a"),
        "# mine\nreasoning_effort = \"high\"\n\n[providers.x]\nname = \"X\"\n\n[roles]\nmain = \"mistral/a\"\n"
    );
    // an existing [roles]: the line replaced, or added at its end
    let t = "[roles]\nmain = \"mistral/a\"\n\n[providers.x]\nname = \"X\"\n";
    assert_eq!(with_role(t, "main", "mistral/b"), "[roles]\nmain = \"mistral/b\"\n\n[providers.x]\nname = \"X\"\n");
    assert_eq!(with_role(t, "agents", "openai/c"), "[roles]\nmain = \"mistral/a\"\nagents = \"openai/c\"\n\n[providers.x]\nname = \"X\"\n");
    // [roles.main]: its model line
    let t = "[roles.main]\nmodel = \"mistral/a\"\neffort = \"high\"\n";
    assert_eq!(with_role(t, "main", "mistral/b"), "[roles.main]\nmodel = \"mistral/b\"\neffort = \"high\"\n");
    // voice: [voice] model goes, its language stays
    let t = "[voice]\nmodel = \"openai/whisper-1\"\nlanguage = \"fr\"\n";
    let out = with_role(t, "voice", "mistral/voxtral-transcribe-3");
    assert_eq!(out, "[voice]\nlanguage = \"fr\"\n\n[roles]\nvoice = \"mistral/voxtral-transcribe-3\"\n");
    let s = setup(&out);
    assert_eq!((s.voice.model.as_str(), s.voice.language.as_deref()), ("mistral/voxtral-transcribe-3", Some("fr")));
    // agent_model and small_model go too
    assert_eq!(with_role("agent_model = \"a/b\"\n", "agents", "c/d"), "\n[roles]\nagents = \"c/d\"\n".trim_start());
    assert_eq!(with_role("small_model = \"a/b\"\nx = 1\n", "small", "c/d"), "x = 1\n\n[roles]\nsmall = \"c/d\"\n");
}

#[test]
fn every_role_has_its_words() {
    for r in roles::ROLES {
        assert!(!r.name.is_empty() && !r.about.is_empty() && !r.env.is_empty(), "{:?}", r);
    }
    assert_eq!(roles::role("small").map(|r| r.label()), Some("small jobs (titles, summaries)".to_string()));
    // approvals (design §4.2): the checker, designer's words
    let c = roles::role("classify").unwrap();
    assert!(c.shown);
    assert_eq!(c.label(), "checker (in auto, decides which commands run and which ask you)");
}

#[test]
fn the_checker_is_jev_first_then_the_small_model() {
    let small = "mistral/mistral-small-latest";
    assert_eq!(roles::checker_default(small, &|_| true), roles::JEV_TYPESAFE);
    assert_eq!(roles::checker_default(small, &|p| p == "openrouter"), roles::JEV_OPENROUTER);
    assert_eq!(roles::checker_default(small, &|_| false), small);
    assert_eq!(roles::jev_of("typesafe/jev-1.13"), Some(("typesafe", "jev-1.13.0".to_string())));
    assert_eq!(roles::jev_of("openrouter/typesafe/jev-1.13"), Some(("openrouter", "typesafe/jev-1.13".to_string())));
    assert_eq!(roles::jev_of("openrouter/anthropic/claude-sonnet-5.5"), None);
    // off stays off; TypeSafe answers questions, it does not chat
    let s = crate::Setup::from_text(Some("[roles]\nclassify = \"off\"\n"), &|_| None);
    assert_eq!(s.role_model(roles::CLASSIFY), ("off".to_string(), roles::Source::Picked));
    let t = s.catalog.provider("typesafe").unwrap();
    assert!(t.decides && !t.chats() && t.key_env == "TYPESAFE_API_KEY");
}

#[test]
fn a_role_is_set_with_its_effort_or_unset() {
    use roles::set_role;
    // an effort: the table form; the old keys go
    let t = "model = \"a/b\"\nreasoning_effort = \"low\"\n";
    let out = set_role(t, "main", Some("mistral/m"), Some("high"));
    assert_eq!(out, "\n[roles.main]\nmodel = \"mistral/m\"\neffort = \"high\"\n".trim_start());
    let s = setup(&out);
    assert_eq!((s.model.as_str(), s.effort.as_str()), ("mistral/m", "high"));
    // no effort: the line again, the table's effort gone
    let out = set_role(&out, "main", Some("mistral/n"), None);
    assert_eq!(out, "[roles.main]\nmodel = \"mistral/n\"\n");
    let out = set_role("[roles]\nmain = \"a/b\"\n", "main", Some("mistral/n"), None);
    assert_eq!(out, "[roles]\nmain = \"mistral/n\"\n");
    // unset: the line goes, main's fallback again
    let t = "agent_model = \"x/y\"\n[roles]\nmain = \"a/b\"\nagents = \"c/d\"\n";
    let out = set_role(t, "agents", None, None);
    assert_eq!(out, "[roles]\nmain = \"a/b\"\n");
    assert_eq!(setup(&out).agent_model, "a/b");
    let out = set_role("[roles.agents]\nmodel = \"c/d\"\neffort = \"low\"\n\n[x]\ny = 1\n", "agents", None, None);
    assert_eq!(out, "[x]\ny = 1\n");
}

// ---- base URLs from the env (Ben's report, 2026-10-01: foundry's URL in
// ANTHROPIC_FOUNDRY_BASE_URL was ignored, the hand-off kept base_url = "")

fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

const OPUS: &str = "model = \"foundry/claude-opus-5-5\"\n";

#[test]
fn foundry_base_url_comes_from_its_env_variable_in_claude_codes_form() {
    // Claude Code's form, no /v1: bise adds it (the runtime adds /messages)
    for raw in ["https://proxy.example/anthropic", "https://proxy.example/anthropic/", "https://proxy.example/anthropic/v1", " https://proxy.example/anthropic/v1/ "] {
        let env = move |k: &str| (k == "ANTHROPIC_FOUNDRY_BASE_URL").then(|| raw.to_string());
        let s = Setup::from_text(Some(OPUS), &env);
        assert_eq!(s.model_for("main").base_url, "https://proxy.example/anthropic/v1", "{raw:?}");
        assert_eq!(s.catalog.provider("foundry").unwrap().base_url_from, "ANTHROPIC_FOUNDRY_BASE_URL");
    }
    // blank: as unset
    let s = Setup::from_text(Some(OPUS), &env_of(&[("ANTHROPIC_FOUNDRY_BASE_URL", "  ")]));
    assert_eq!(s.model_for("main").base_url, "");
    assert_eq!(s.catalog.provider("foundry").unwrap().base_url_from, "");
    // only the Anthropic family adds /v1: an OpenAI-style variable keeps its path
    assert_eq!(env_base_url("openai-chat", "https://x.example/v1/"), "https://x.example/v1");
    assert_eq!(env_base_url("openai-chat", "https://x.example/api"), "https://x.example/api");
    assert_eq!(env_base_url("anthropic", ""), "");
}

#[test]
fn config_base_url_wins_over_the_env_and_an_empty_one_hides_nothing() {
    let env = env_of(&[("ANTHROPIC_FOUNDRY_BASE_URL", "https://env.example/anthropic")]);
    let cfg = format!("{OPUS}[providers.foundry]\nbase_url = \"https://cfg.example/anthropic/v1/\"\n");
    let s = Setup::from_text(Some(&cfg), &env);
    assert_eq!(s.model_for("main").base_url, "https://cfg.example/anthropic/v1");
    assert_eq!(s.catalog.provider("foundry").unwrap().base_url_from, "config");
    // base_url = "" in config.toml: never authoritative, the env's applies
    let cfg = format!("{OPUS}[providers.foundry]\nbase_url = \"\"\n");
    let s = Setup::from_text(Some(&cfg), &env);
    assert_eq!(s.model_for("main").base_url, "https://env.example/anthropic/v1");
    // and it does not blank a built-in URL either
    let s = setup("[providers.anthropic]\nbase_url = \"\"\n");
    assert_eq!(s.catalog.resolve("anthropic/claude-opus-5-5").base_url, "https://api.anthropic.com/v1");
    assert_eq!(s.catalog.provider("anthropic").unwrap().base_url_from, "built-in");
}

#[test]
fn a_config_provider_may_name_its_own_base_url_variable() {
    let cfg = "[providers.work]\napi = \"openai-chat\"\nbase_url_env = \"WORK_URL, OTHER_URL\"\n";
    let s = Setup::from_text(Some(cfg), &env_of(&[("OTHER_URL", "http://w.example/v1/")]));
    assert_eq!(s.catalog.resolve("work/m").base_url, "http://w.example/v1");
    let s = Setup::from_text(Some(cfg), &env_of(&[("WORK_URL", "http://first.example/v1"), ("OTHER_URL", "http://w.example/v1")]));
    assert_eq!(s.catalog.resolve("work/m").base_url, "http://first.example/v1");
}

#[test]
fn the_env_then_the_env_files_in_order() {
    let files = vec![
        auth::EnvFile::parse("/a/.env".into(), "ANTHROPIC_FOUNDRY_BASE_URL=\nX=from-a\n"),
        auth::EnvFile::parse("/b/.vibe/.env".into(), "export ANTHROPIC_FOUNDRY_BASE_URL=\"https://vibe.example/anthropic\"\nX=from-b\n"),
    ];
    let none = |_: &str| None;
    // a blank line in the first file does not hide the second's
    assert_eq!(with_files(&none, &files, "ANTHROPIC_FOUNDRY_BASE_URL").as_deref(), Some("https://vibe.example/anthropic"));
    assert_eq!(with_files(&none, &files, "X").as_deref(), Some("from-a"));
    let env = env_of(&[("X", "from-env"), ("Y", " ")]);
    assert_eq!(with_files(&env, &files, "X").as_deref(), Some("from-env"));
    assert_eq!(with_files(&env, &files, "Y"), None);
    // Ben's setup: key and URL in ~/.vibe/.env, nothing in config.toml
    let s = Setup::from_parts(Some(OPUS), &none, &|k| with_files(&none, &files, k));
    assert_eq!(s.model_for("main").base_url, "https://vibe.example/anthropic/v1");
}

#[test]
fn no_base_url_says_what_to_set() {
    let c = Catalog::builtin();
    let m = c.no_base_url("foundry");
    assert!(m.contains("ANTHROPIC_FOUNDRY_BASE_URL") && m.contains("[providers.foundry]"), "{m}");
    assert!(!m.contains("network"), "{m}");
    let m = c.no_base_url("nobody");
    assert!(m.contains("[providers.nobody]") && !m.contains("_BASE_URL"), "{m}");
}

/// LAW: the hand-off never says `base_url = ""`: a provider without a URL
/// has no base_url line (the runtime then names the fix), whatever the
/// config and the env say.
#[test]
fn the_handoff_never_stores_an_empty_base_url() {
    let cfgs = [
        None,
        Some(OPUS.to_string()),
        Some(format!("{OPUS}[providers.foundry]\nbase_url = \"\"\n")),
        Some("[providers.empty]\napi = \"anthropic\"\nbase_url = \"\"\n[providers.bare]\nkey_env = \"K\"\n".to_string()),
    ];
    let envs: [&'static [(&'static str, &'static str)]; 3] = [&[], &[("ANTHROPIC_FOUNDRY_BASE_URL", "")], &[("ANTHROPIC_FOUNDRY_BASE_URL", "https://p.example/anthropic")]];
    for cfg in &cfgs {
        for e in envs {
            let s = Setup::from_text(cfg.as_deref(), &env_of(e));
            let h = s.handoff_toml();
            assert!(!h.contains("base_url = \"\""), "{cfg:?} {e:?}");
            let t: toml::Table = h.parse().unwrap();
            for (id, p) in t["providers"].as_table().unwrap() {
                if let Some(u) = p.get("base_url") {
                    assert!(!u.as_str().unwrap().is_empty(), "{id}");
                }
            }
        }
    }
    // no URL: the variable that would give it, for the runtime's message
    let h = Setup::from_text(Some(OPUS), &no_env).handoff_toml();
    let foundry = h.split("[providers.foundry]").nth(1).unwrap().split("\n[").next().unwrap();
    assert!(foundry.contains("base_url_env = \"ANTHROPIC_FOUNDRY_BASE_URL\"") && !foundry.contains("base_url ="), "{foundry}");
    // a URL: the URL, not the variable
    let h = Setup::from_text(Some(OPUS), &env_of(&[("ANTHROPIC_FOUNDRY_BASE_URL", "https://p.example/anthropic")])).handoff_toml();
    let foundry = h.split("[providers.foundry]").nth(1).unwrap().split("\n[").next().unwrap();
    assert!(foundry.contains("base_url = \"https://p.example/anthropic/v1\"") && !foundry.contains("base_url_env"), "{foundry}");
}

/// The hub writes the hand-off again (each spawn, each input): a config
/// edit or a new variable replaces the old file, never kept from a
/// first run without a URL.
#[test]
fn export_handoff_again_follows_config_and_env_edits() {
    let dir = std::env::temp_dir().join(format!("bise-catalog-url-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("config.toml");
    let cache = dir.join("cache");
    std::fs::write(&cfg, OPUS).unwrap();
    let read = |p: &Path| std::fs::read_to_string(p).unwrap();
    // the first run: no URL anywhere
    let p = export_handoff(&cfg, &cache, &no_env).unwrap();
    assert!(!read(&p).contains("foundry.example"));
    // the variable shows up (a .env file edited): the next write has it
    let env = env_of(&[("ANTHROPIC_FOUNDRY_BASE_URL", "https://env.foundry.example/anthropic")]);
    assert_eq!(export_handoff(&cfg, &cache, &env).unwrap(), p);
    assert!(read(&p).contains("base_url = \"https://env.foundry.example/anthropic/v1\""));
    // config.toml edited: it wins at the next write
    std::fs::write(&cfg, format!("{OPUS}[providers.foundry]\nbase_url = \"https://cfg.foundry.example/anthropic/v1\"\n")).unwrap();
    export_handoff(&cfg, &cache, &env).unwrap();
    assert!(read(&p).contains("base_url = \"https://cfg.foundry.example/anthropic/v1\""));
    // both gone: no URL again, and no empty one
    std::fs::write(&cfg, OPUS).unwrap();
    export_handoff(&cfg, &cache, &no_env).unwrap();
    assert!(!read(&p).contains("foundry.example") && !read(&p).contains("base_url = \"\""));
    let _ = std::fs::remove_dir_all(&dir);
}
