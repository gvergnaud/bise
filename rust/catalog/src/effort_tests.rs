//! effort-table: each default model's reasoning-effort list, the APIs'
//! lists (live_efforts, live_fetch) and their place in the order, and no
//! list left in the Bend runtime.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::*;
use crate::live_efforts::{self, Store};

fn no_env(_: &str) -> Option<String> {
    None
}

/// A model read back from the hand-off: a list it had from its family is
/// written there as its provider's `efforts` (the runtime has none).
pub(super) fn with_family(c: &Catalog, mut r: Resolved) -> Resolved {
    // a speech-to-text provider stays out of the file (a chat provider's
    // speech model reads back as one of its unlisted models)
    if let Some(p) = c.provider(&r.provider).filter(|p| p.chats() && r.caps.efforts.is_empty()) {
        r.caps.efforts = family_efforts(&p.api).join(",");
    }
    r
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-efforts-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn each_default_model_has_its_own_list() {
    let s = Setup::from_text(Some(""), &no_env);
    let c = &s.catalog;
    // Opus 5.5 takes Claude's xhigh ("extra high"), high stays its default
    for id in ["anthropic/claude-opus-5-5", "foundry/claude-opus-5-5"] {
        let opus = c.resolve(id);
        assert_eq!(opus.efforts(), ["none", "low", "medium", "high", "xhigh", "max"], "{id}");
        assert_eq!(opus.default_effort(), "high");
        assert_eq!(opus.effort_for("xhigh"), "xhigh");
    }
    // Haiku's efforts are budget sizes: no xhigh
    assert_eq!(c.resolve("anthropic/claude-haiku-4-5").efforts(), ["none", "low", "medium", "high", "max"]);
    // OpenRouter refuses none on Claude; grok-4.7 always reasons, takes xhigh
    assert_eq!(c.resolve("openrouter/anthropic/claude-opus-5.5").efforts(), ["low", "medium", "high", "xhigh", "max"]);
    assert_eq!(c.resolve("xai/grok-4.7").efforts(), ["low", "medium", "high", "xhigh"]);
    assert_eq!(c.resolve("google/gemini-3.8-flash").efforts(), ["low", "medium", "high"]);
}

/// Rust owns the lists: no effort word list, and none of the old
/// definitions, in the runtime's code (bend/ outside laws/ and proofs/,
/// whose texts stand for a models file).
#[test]
fn no_effort_list_is_left_in_bend() {
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../bend"));
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            let skip = ["laws", "proofs", "vendor"].iter().any(|s| p.file_name().is_some_and(|n| n == *s));
            if p.is_dir() && !skip {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "bend") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(files.len() > 10, "{files:?}");
    for f in files {
        let src = std::fs::read_to_string(&f).unwrap();
        for bad in ["anth_efforts", "chat_efforts", "efforts.family", "\"low\", \"medium\"", "low,medium,high"] {
            assert!(!src.contains(bad), "{}: an effort list ({bad}) in bend/: Rust owns them (efforts.rs)", f.display());
        }
    }
}

#[test]
fn the_apis_answers_parse_into_lists_in_bises_order() {
    let anth = json!({"data": [
        {"id": "claude-opus-5-5", "capabilities": {"effort": {"max": {"supported": true}, "low": {"supported": true},
            "xhigh": {"supported": true}, "medium": {"supported": true}, "high": {"supported": true}}}},
        {"id": "claude-haiku-4-5", "capabilities": {}},
        {"id": "claude-x", "capabilities": {"effort": {"low": {"supported": false}}}}]});
    let opus: Vec<String> = ["none", "low", "medium", "high", "xhigh", "max"].map(String::from).to_vec();
    assert_eq!(live_efforts::parse_anthropic(&anth), [("anthropic/claude-opus-5-5".to_string(), opus)]);
    let or = json!({"data": [
        {"id": "anthropic/claude-opus-5.5", "reasoning": {"supported_efforts": ["max", "xhigh", "high", "medium", "low"]}},
        {"id": "x-ai/grok-4.7", "reasoning": {"supported_efforts": ["turbo", "high", "low", "bad word"]}},
        {"id": "meta/llama", "reasoning": {}}]});
    let l = live_efforts::parse_openrouter(&or);
    assert_eq!(l[0].1, ["low", "medium", "high", "xhigh", "max"]);
    assert_eq!(l[1], ("openrouter/x-ai/grok-4.7".into(), vec!["low".to_string(), "high".into(), "turbo".into()]), "unknown words after, bad ones out");
    assert_eq!(l.len(), 2);
    // ChatGPT: the API's levels, and bise's cache of them
    let plan = json!({"models": [
        {"slug": "gpt-6.1-sol", "supported_reasoning_levels": [{"effort": "high"}, {"effort": "low"}, {"effort": "xhigh"}]},
        {"slug": "gpt-6-astra", "efforts": ["low", "max"]},
        {"slug": "hidden", "visibility": "hide"}]});
    let m = live_efforts::plan_models(&plan);
    let got: Vec<(&str, String)> = m.iter().map(|m| (m.slug.as_str(), m.efforts.join(","))).collect();
    assert_eq!(got, [("gpt-6.1-sol", "low,high,xhigh".to_string()), ("gpt-6-astra", "low,max".to_string())]);
}

#[test]
fn the_store_keeps_each_provider_apart_and_says_when_it_is_due() {
    let a = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let s = Store::default().with("anthropic", 1_000, vec![("anthropic/m".into(), a(&["none", "high"])), ("openrouter/sneaky".into(), a(&["low"]))]);
    assert_eq!(s.models.keys().collect::<Vec<_>>(), ["anthropic/m"], "a provider writes its own models only");
    let s = s.with("openrouter", 2_000, vec![("openrouter/o".into(), a(&["low"]))]);
    let s = s.with("anthropic", 3_000, vec![("anthropic/n".into(), a(&["max"]))]);
    assert_eq!(s.models.keys().collect::<Vec<_>>(), ["anthropic/n", "openrouter/o"], "a new fetch replaces the old lists");
    assert!(!s.due("anthropic", 3_000 + live_efforts::TTL_MS - 1));
    assert!(s.due("anthropic", 3_000 + live_efforts::TTL_MS) && s.due("chatgpt", 0));
    assert_eq!(Store::parse(&s.to_json()), Some(s));
    assert_eq!(Store::parse("{not json"), None);
    assert_eq!(Store::parse("[1, 2]"), None);
}

/// The order of a model's list, one layer at a time: its family < the
/// table (models.toml) < the API's list < config.toml.
#[test]
fn the_list_comes_from_config_then_the_api_then_the_table_then_the_family() {
    let words = |s: &Setup, m: &str| s.catalog.resolve(m).efforts().join(",");
    let live = vec![
        ("anthropic/claude-opus-5-5".to_string(), vec!["none".to_string(), "low".into(), "max".into()]),
        ("anthropic/claude-sonnet-5-5".to_string(), vec!["low".to_string(), "high".into()]),
        ("anthropic/not-listed".to_string(), vec!["low".to_string()]),
    ];
    let cfg = "[models.\"anthropic/claude-sonnet-5-5\"]\nefforts = \"medium\"\n";
    let s = Setup::from_parts(Some(cfg), &live, &no_env, &no_env);
    assert_eq!(words(&s, "anthropic/claude-sonnet-5-5"), "medium", "config.toml first");
    assert_eq!(words(&s, "anthropic/claude-opus-5-5"), "none,low,max", "then the API's");
    assert_eq!(words(&s, "anthropic/claude-fable-5-1"), "none,low,medium,high,xhigh,max", "then the table's");
    assert_eq!(words(&s, "anthropic/not-listed"), ANTHROPIC_EFFORTS.join(","), "then the family's (an API list of a model bise does not list: not used)");
    assert_eq!(s.catalog.resolve("anthropic/claude-opus-5-5").default_effort(), "max", "high gone: the last word");
    // the hand-off carries the API's list, and each provider's family list
    let h = s.handoff_toml();
    // (every listed model says its own vision first: tests.rs
    // the_handoff_says_each_listed_models_vision)
    assert!(h.contains("[models.\"anthropic/claude-opus-5-5\"]\nvision = true\nefforts = \"none,low,max\""), "{h}");
    let table = |name: &str| {
        let t = &h[h.find(name).unwrap()..];
        t[..t[1..].find("\n[").unwrap_or(t.len() - 1)].to_string()
    };
    assert!(table("[providers.anthropic]").contains("efforts = \"none,low,medium,high,xhigh,max\""), "{h}");
    assert!(table("[providers.groq]").contains("efforts = \"none,low,medium,high\""), "{h}");
}

/// A fake models endpoint: answers each request with `body`, `n` times.
fn serve(body: String, n: usize) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut s in l.incoming().take(n).flatten() {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = s.write_all(head.as_bytes()).and_then(|_| s.write_all(body.as_bytes()));
        }
    });
    port
}

fn paths(dir: &Path) -> auth_cli::Paths {
    auth_cli::Paths { auth_file: dir.join("auth.json"), config: dir.join("config.toml"), env_files: Vec::new(), home: None }
}

/// `cfg` with Anthropic's base URL on port `to` instead of `from`.
fn setup_with_url(cfg: &str, from: u16, to: u16) -> Setup {
    Setup::from_text(Some(&cfg.replace(&format!(":{from}/v1"), &format!(":{to}/v1"))), &no_env)
}

#[test]
fn refresh_fetches_the_providers_he_uses_and_keeps_the_old_file_on_failure() {
    let dir = tmp("refresh");
    let cache = dir.join("cache");
    let anth = json!({"data": [{"id": "claude-opus-5-5", "capabilities": {"effort": {"low": {"supported": true}, "xhigh": {"supported": true}}}}]});
    let port = serve(anth.to_string(), 1);
    let cfg = format!("[providers.anthropic]\nbase_url = \"http://127.0.0.1:{port}/v1\"\n[providers.openrouter]\nbase_url = \"http://127.0.0.1:1/api/v1\"\n");
    std::fs::write(dir.join("config.toml"), &cfg).unwrap();
    let setup = Setup::from_text(Some(&cfg), &no_env);
    let store = auth::Store::default();
    // a key for Anthropic only: OpenRouter is not his, never called
    let env = |k: &str| (k == "ANTHROPIC_API_KEY").then(|| "sk-test".to_string());
    let keys = auth::Keys { env: &env, store: &store, files: &[] };
    let lines = live_fetch::refresh(&paths(&dir), &cache, &setup, &keys, 1_000);
    assert_eq!(lines, ["efforts: 1 lists from anthropic"], "{lines:?}");
    let live = live_fetch::read(&cache);
    assert_eq!(live, [("anthropic/claude-opus-5-5".to_string(), vec!["none".to_string(), "low".into(), "xhigh".into()])]);
    // fresh: no second call (the server answers once only)
    assert!(live_fetch::refresh(&paths(&dir), &cache, &setup, &keys, 2_000).is_empty());
    // a day later, the server down: the old file stays, no wait
    let t = std::time::Instant::now();
    let lines = live_fetch::refresh(&paths(&dir), &cache, &setup_with_url(&cfg, port, 1), &keys, 1_000 + live_efforts::TTL_MS);
    assert!(t.elapsed() < std::time::Duration::from_secs(3), "{:?}", t.elapsed());
    assert!(lines.len() == 1 && lines[0].starts_with("efforts: anthropic kept its old lists"), "{lines:?}");
    assert!(lines.iter().all(|l| !l.contains("sk-test")));
    assert_eq!(live_fetch::read(&cache), live);
    // an answer that is not a list: the old file stays too
    let bad = serve("{\"data\": \"nope\"}".into(), 1);
    let lines = live_fetch::refresh(&paths(&dir), &cache, &setup_with_url(&cfg, port, bad), &keys, 1_000 + live_efforts::TTL_MS);
    assert!(lines[0].contains("has no effort list"), "{lines:?}");
    assert_eq!(live_fetch::read(&cache), live);
    // the hand-off with it: Opus 5.5 gets the API's list
    let p = export_handoff(&dir.join("config.toml"), &cache, &no_env).unwrap();
    assert!(std::fs::read_to_string(p).unwrap().contains("efforts = \"none,low,xhigh\""));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A hub with no provider key (a test hub: tests/bise_env.py drops his
/// keys) makes no fetch at all: both providers point at a server that
/// counts, and it is never called (docs/issues/26: a test hub with his
/// real OPENROUTER_API_KEY fetched OpenRouter's real list).
#[test]
fn no_key_no_fetch() {
    let dir = tmp("nokey");
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.set_nonblocking(true).unwrap();
    let port = l.local_addr().unwrap().port();
    let cfg = format!("[providers.anthropic]
base_url = \"http://127.0.0.1:{port}/v1\"
[providers.openrouter]
base_url = \"http://127.0.0.1:{port}/api/v1\"
");
    let setup = Setup::from_text(Some(&cfg), &no_env);
    let store = auth::Store::default();
    let keys = auth::Keys { env: &no_env, store: &store, files: &[] };
    let lines = live_fetch::refresh(&paths(&dir), &dir.join("cache"), &setup, &keys, 1_000);
    assert!(lines.is_empty(), "{lines:?}");
    assert!(l.accept().is_err(), "a fetch with no key");
    assert!(live_fetch::read(&dir.join("cache")).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The hub with no network starts the same: no file, no wait, the
/// table's and the family's lists.
#[test]
fn no_network_no_file_the_table_and_the_family() {
    let dir = tmp("offline");
    let cfg = "[providers.anthropic]\nbase_url = \"http://127.0.0.1:1/v1\"\n";
    let setup = Setup::from_text(Some(cfg), &no_env);
    let store = auth::Store::default();
    let env = |k: &str| (k == "ANTHROPIC_API_KEY").then(|| "sk-test".to_string());
    let keys = auth::Keys { env: &env, store: &store, files: &[] };
    let t = std::time::Instant::now();
    let lines = live_fetch::refresh(&paths(&dir), &dir.join("cache"), &setup, &keys, 1_000);
    assert!(t.elapsed() < std::time::Duration::from_secs(3) && lines.len() == 1, "{lines:?} {:?}", t.elapsed());
    assert!(live_fetch::read(&dir.join("cache")).is_empty());
    let s = Setup::from_parts(Some(cfg), &live_fetch::read(&dir.join("cache")), &no_env, &no_env);
    assert_eq!(s.catalog.resolve("anthropic/claude-opus-5-5").efforts(), ["none", "low", "medium", "high", "xhigh", "max"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn openai_models_default_to_medium() {
    // issue #18 (page token-audit, Gabriel's pick): OpenAI's models default
    // to medium on both routes, as Codex; the other providers keep theirs
    let s = Setup::from_text(Some(""), &no_env);
    for p in ["openai", "chatgpt"] {
        for m in ["gpt-6.1-sol", "gpt-6-astra", "gpt-6-luna"] {
            assert_eq!(s.catalog.resolve(&format!("{p}/{m}")).caps.effort, "medium", "{p}/{m}");
        }
        let h = s.handoff_toml();
        let t = &h[h.find(&format!("[providers.{p}]")).unwrap()..];
        assert!(t[..t[1..].find("\n[").unwrap()].contains("effort = \"medium\""), "{p}: {t}");
    }
    assert_eq!(s.catalog.resolve("anthropic/claude-opus-5-5").caps.effort, "");
}
