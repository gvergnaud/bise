//! One key, every role: a fresh install with the key of ONE provider
//! (and nothing else) gets a model for each role, of that provider. The
//! table below is the audit: main (the first run saves the provider's
//! pick, the first listed model when it has none), agents (same as
//! main), small jobs (titles, summaries), the checker of auto mode,
//! voice ("-": none of its keys listens, /models says so). Compaction
//! summarizes with the session's own model (main's or the agent's), so
//! it needs no row.

use super::*;

/// (provider, main, small jobs, checker, voice)
const TABLE: &[(&str, &str, &str, &str, &str)] = &[
    ("anthropic", "claude-opus-5-5", "claude-haiku-4-5", "anthropic/claude-haiku-4-5", "-"),
    ("foundry", "claude-opus-5-5", "claude-haiku-4-5", "foundry/claude-haiku-4-5", "-"),
    ("openai", "gpt-6-astra", "gpt-6-luna", "openai/gpt-6-luna", "openai/gpt-transcribe"),
    ("google", "gemini-3.8-flash", "gemini-3.5-flash-lite", "google/gemini-3.5-flash-lite", "-"),
    ("mistral", "mistral-medium-latest", "mistral-small-latest", "mistral/mistral-small-latest", "mistral/voxtral-transcribe-3"),
    ("openrouter", "anthropic/claude-sonnet-5.5", "google/gemini-3.8-flash", roles::JEV_OPENROUTER, "-"),
    // Groq transcribes, but is hidden from voice (the user, 2026-09-30)
    ("groq", "openai/gpt-oss-120b", "openai/gpt-oss-20b", "groq/openai/gpt-oss-20b", "-"),
    ("xai", "grok-4.7", "grok-4.3", "xai/grok-4.3", "-"),
    ("deepseek", "deepseek-v4-pro", "deepseek-flash", "deepseek/deepseek-flash", "-"),
    ("together", "zai-org/GLM-5.3", "zai-org/GLM-5.3-Flash", "together/zai-org/GLM-5.3-Flash", "-"),
    (
        "fireworks",
        "accounts/fireworks/models/glm-5p3",
        "accounts/fireworks/models/glm-5p3-flash",
        "fireworks/accounts/fireworks/models/glm-5p3-flash",
        "-",
    ),
    // no smaller model listed: the main one (already a fast one)
    ("cerebras", "gpt-oss-120b", "gpt-oss-120b", "cerebras/gpt-oss-120b", "-"),
    // the coding plans (keys too); Kimi and MiniMax list one model
    ("zai-coding", "glm-5.3", "glm-5.3-flash", "zai-coding/glm-5.3-flash", "-"),
    ("kimi-code", "kimi-for-coding", "kimi-for-coding", "kimi-code/kimi-for-coding", "-"),
    ("minimax", "MiniMax-M3", "MiniMax-M3", "minimax/MiniMax-M3", "-"),
];

/// What the first run saves as main for `p` (onboarding's `pick_of`,
/// else the first model the key step lists).
fn first_run_main(c: &Catalog, p: &Provider) -> String {
    let id = if p.model.is_empty() {
        c.models.iter().find(|m| m.provider == p.id && !m.stt).map(|m| m.id.clone()).unwrap_or_default()
    } else {
        p.model.clone()
    };
    format!("{}/{}", p.id, id)
}

/// The setup of a fresh install with only `pid`'s key, in auth.json
/// (`login`) or in the environment.
fn one_key(pid: &str, in_env: bool) -> Setup {
    let c = Catalog::builtin();
    let p = c.provider(pid).unwrap();
    let cfg = roles::with_role("", roles::MAIN, &first_run_main(&c, p));
    let store = if in_env {
        auth::Store::default()
    } else {
        auth::Store::parse(&format!("{{\"{}\": {{\"type\": \"api\", \"key\": \"sk-test\"}}}}", pid)).unwrap()
    };
    let var = p.key_env.split(',').next().unwrap().trim().to_string();
    let env = move |k: &str| (in_env && k == var).then(|| "sk-test".to_string());
    let s = Setup::from_text(Some(&cfg), &env);
    s.with_keys(&auth::Keys { env: &env, store: &store, files: &[] })
}

#[test]
fn the_table_covers_every_provider_a_user_can_bring_one_key_for() {
    let c = Catalog::builtin();
    let offered: Vec<&str> =
        c.providers.iter().filter(|p| p.chats() && p.needs.is_empty() && !p.key_env.is_empty()).map(|p| p.id.as_str()).collect();
    let rows: Vec<&str> = TABLE.iter().map(|r| r.0).collect();
    assert_eq!(offered, rows, "a new chat provider: add its row (and a small_model when it has a cheaper one)");
}

#[test]
fn one_key_gives_every_role_a_model_of_that_provider() {
    for &(pid, main, small, checker, voice) in TABLE {
        for in_env in [false, true] {
            let s = one_key(pid, in_env);
            let at = |m: &str| format!("{}/{}", pid, m);
            assert_eq!(s.model, at(main), "{pid}: main");
            assert_eq!(s.agent_model, at(main), "{pid}: agents follow main");
            assert_eq!(s.small_model, at(small), "{pid}: small jobs");
            assert_eq!(s.role_model(roles::CLASSIFY), (checker.to_string(), roles::Source::Auto), "{pid}: checker");
            // every chat role resolves to a listed model of a usable
            // provider (no "unknown model", no other provider's key)
            for m in [&s.model, &s.agent_model, &s.small_model] {
                let r = s.catalog.resolve(m);
                assert_eq!(r.known, Known::Listed, "{pid}: {m}");
                assert_eq!(r.provider, pid, "{pid}: {m}");
            }
            if checker != roles::JEV_OPENROUTER {
                assert_eq!(s.catalog.resolve(checker).known, Known::Listed, "{pid}: {checker}");
            }
            // a cheaper model for the small jobs whenever one is listed
            if !["cerebras", "kimi-code", "minimax"].contains(&pid) {
                assert_ne!(s.small_model, s.model, "{pid}");
            }
            // voice: its provider's when it listens, else the default,
            // which then has no key (the /models row says it is off)
            let v = s.role_model(roles::VOICE).0;
            if voice == "-" {
                assert_eq!(v, s.catalog.default_voice_model, "{pid}: voice");
            } else {
                assert_eq!(v, voice, "{pid}: voice");
            }
        }
    }
}

#[test]
fn keys_never_override_a_role_set_in_config_or_env() {
    let store = auth::Store::parse("{\"openai\": {\"type\": \"api\", \"key\": \"k\"}}").unwrap();
    let cfg = "[roles]
main = \"openai/gpt-6-astra\"
voice = \"mistral/voxtral-transcribe-3\"
classify = \"off\"
";
    let s = Setup::from_text(Some(cfg), &|_| None).with_keys(&auth::Keys { env: &|_| None, store: &store, files: &[] });
    assert_eq!(s.voice.model, "mistral/voxtral-transcribe-3");
    assert_eq!(s.classify_model, roles::CHECKER_OFF);
    let env = |k: &str| (k == "BISE_CLASSIFY_MODEL").then(|| "openai/gpt-6-astra".to_string());
    let s = Setup::from_text(Some("[roles]
main = \"openai/gpt-6-astra\"
"), &env).with_keys(&auth::Keys { env: &env, store: &store, files: &[] });
    assert_eq!(s.classify_model, "openai/gpt-6-astra");
}

#[test]
fn a_typesafe_key_next_to_one_chat_key_makes_jev_the_checker() {
    let store = auth::Store::parse(
        "{\"anthropic\": {\"type\": \"api\", \"key\": \"k\"}, \"typesafe\": {\"type\": \"api\", \"key\": \"t\"}}",
    )
    .unwrap();
    let s = Setup::from_text(Some("[roles]
main = \"anthropic/claude-opus-5-5\"
"), &|_| None)
        .with_keys(&auth::Keys { env: &|_| None, store: &store, files: &[] });
    assert_eq!(s.classify_model, roles::JEV_TYPESAFE);
}

#[test]
fn a_voice_only_key_gives_voice_its_provider() {
    // ElevenLabs alone listens; Mistral's key wins over it (the default)
    let el = auth::Store::parse("{\"elevenlabs\": {\"type\": \"api\", \"key\": \"e\"}}").unwrap();
    let s = Setup::from_text(None, &|_| None).with_keys(&auth::Keys { env: &|_| None, store: &el, files: &[] });
    assert_eq!(s.voice.model, "elevenlabs/scribe_v2");
    let both = auth::Store::parse(
        "{\"elevenlabs\": {\"type\": \"api\", \"key\": \"e\"}, \"mistral\": {\"type\": \"api\", \"key\": \"m\"}}",
    )
    .unwrap();
    let s = Setup::from_text(None, &|_| None).with_keys(&auth::Keys { env: &|_| None, store: &both, files: &[] });
    assert_eq!(s.voice.model, "mistral/voxtral-transcribe-3");
}

/// A ChatGPT sign-in alone (no key at all): ready when signed in, every
/// chat role on a chatgpt model, the small jobs on the cheapest one listed.
#[test]
fn a_chatgpt_sign_in_alone_gives_every_role_a_plan_model() {
    let c = Catalog::builtin();
    let p = c.provider("chatgpt").unwrap();
    assert!(p.signs_in() && p.key_env.is_empty() && p.shape == "chatgpt-plan");
    let signed_in = auth::Store::parse(
        r#"{"chatgpt": {"type": "oauth", "client_id": "oaiapp_x", "email": "you@example.com", "access": "a", "refresh": "r", "expires": 1}}"#,
    )
    .unwrap();
    let keys = auth::Keys { env: &|_| None, store: &signed_in, files: &[] };
    assert!(keys.ready(p));
    assert_eq!(keys.source(p, None).as_deref(), Some("ChatGPT sign-in (you@example.com)"));
    // signed out (the client kept), or no entry: not ready, never a key
    let mut out = signed_in.clone();
    assert!(out.sign_out("chatgpt", false));
    for s in [&out, &auth::Store::default()] {
        let k = auth::Keys { env: &|_| None, store: s, files: &[] };
        assert!(!k.ready(p) && k.source(p, None).is_none() && k.for_provider(p).is_none());
    }
    // the defaults after the sign-in: the catalog's list, then the account's
    let (main, small) = roles::one_login_defaults(&c, "chatgpt", "openai", &[]).unwrap();
    assert_eq!((main.as_str(), small.as_str()), ("chatgpt/gpt-6.1-sol", "chatgpt/gpt-6-luna"));
    let listed: Vec<String> = ["gpt-6-astra", "gpt-6.1-sol"].iter().map(|s| s.to_string()).collect();
    let (main, small) = roles::one_login_defaults(&c, "chatgpt", "openai", &listed).unwrap();
    assert_eq!((main.as_str(), small.as_str()), ("chatgpt/gpt-6.1-sol", "chatgpt/gpt-6.1-sol"));
    // what the first run then saves: every role a listed plan model
    let cfg = roles::with_role("", roles::MAIN, "chatgpt/gpt-6.1-sol");
    let s = Setup::from_text(Some(&cfg), &|_| None).with_keys(&keys);
    for m in [&s.model, &s.agent_model, &s.small_model, &s.classify_model] {
        let r = s.catalog.resolve(m);
        assert_eq!((r.provider.as_str(), r.known), ("chatgpt", Known::Listed), "{m}");
        assert_eq!(r.price.input, None, "{m}: the plan pays");
    }
    assert_eq!(s.small_model, "chatgpt/gpt-6-luna");
}
