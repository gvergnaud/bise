//! The connectors' pure parts on a fake-shaped bootstrap answer
//! (`fixtures/bootstrap.json`), and the cache on temp dirs with a fake
//! fetch. `fixtures/index.txt` is what `bend/runtime/mcp.bend` wrote for
//! that same answer before this module replaced it (its blank lines
//! dropped): the index lines say the same (a schema compared as JSON).

use super::*;
use crate::model::AccountStatus;
use std::cell::Cell;
use std::path::PathBuf;

const BODY: &str = include_str!("fixtures/bootstrap.json");
const INDEX: &str = include_str!("fixtures/index.txt");

fn catalog(body: &str) -> Catalog {
    Catalog { connectors: parse(body).unwrap(), fetched_ms: 1, error: None }
}

/// A fetch whose bootstrap is `b` and where no connector lists accounts.
fn boot<'a>(b: &'a dyn Fn() -> Result<String, String>) -> Fetch<'a> {
    Fetch { bootstrap: b, accounts: &|_| Err("no accounts here".to_string()) }
}

fn all_on(_: &PluginId) -> bool {
    true
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bp-conn-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.join("connectors").join("bootstrap.json")
}

/// An index line split at its schema: the text before ` | input: ` and
/// the schema parsed (a number may be written another way: 1e-05 and
/// 0.00001 are the same JSON).
fn split(line: &str) -> (String, Option<Value>) {
    match line.split_once(" | input: ") {
        Some((head, schema)) => (head.to_string(), Some(serde_json::from_str(schema).unwrap())),
        None => (line.to_string(), None),
    }
}

#[test]
fn index_lines_say_what_mcp_bend_said() {
    let ours: Vec<_> = index_lines_with(&catalog(BODY), &all_on).iter().map(|l| split(l)).collect();
    let bend: Vec<_> = INDEX.lines().map(split).collect();
    assert_eq!(ours, bend);
}

#[test]
fn the_index_line_format() {
    let c = Catalog {
        connectors: vec![ConnectorEntry {
            name: "gmail".into(),
            display_name: "Gmail".into(),
            id: "c-1".into(),
            auth: ConnectorAuth::Ready,
            tools: vec![
                ToolEntry { name: "send\nmail".into(), description: "Send\r\na\tmail".into(), input_schema: r#"{"type":"object"}"#.into() },
                ToolEntry { name: "list".into(), description: String::new(), input_schema: String::new() },
            ],
            accounts: Vec::new(),
        }],
        ..Catalog::default()
    };
    assert_eq!(index_lines_with(&c, &all_on), ["c-1 gmail send mail : #Send  a\tmail | input: {\"type\":\"object\"}", "c-1 gmail list : #"]);
}

#[test]
fn the_2_53_schema_maximums_parse() {
    let c = catalog(BODY);
    let s = &c.connectors[0].tools[0].input_schema;
    assert!(s.contains(r#""maximum":9007199254740991"#) && s.contains(r#""minimum":-9007199254740991"#), "{s}");
    // compact, keys sorted by bytes, the same values
    let post = &c.connectors[0].tools[1].input_schema;
    assert!(post.starts_with(r#"{"10":5,"9":6,"Alpha":2,"_u":4,"alpha":3,"#), "{post}");
    let v: Value = serde_json::from_str(post).unwrap();
    assert_eq!(v["nested"]["b"], serde_json::json!([1.0, 0.5, 0.00001, -0.0, 1000.0]));
}

#[test]
fn ready_and_each_auth_kind() {
    let c = catalog(BODY);
    let kind = |id: &str| c.connectors.iter().find(|e| e.id == id).unwrap().auth;
    assert_eq!(kind("c0ffee-01"), ConnectorAuth::Ready);
    assert_eq!(kind("c0ffee-03"), ConnectorAuth::OAuth);
    assert_eq!(kind("c0ffee-06"), ConnectorAuth::NoAuthConnect);
    assert_eq!(kind("c0ffee-07"), ConnectorAuth::CredentialsSetup);
    // every connector is in the catalog, ready or not
    assert_eq!(c.connectors.len(), 11);
    let gh = c.connectors.iter().find(|e| e.id == "c0ffee-03").unwrap();
    assert_eq!((gh.name.as_str(), gh.display_name.as_str()), ("github_app", "GitHub"));
    // not ready with no auth_action, or one bise doesn't know: not
    // connectable from bise
    let body = r#"{"connectors":[{"name":"a","status":{"is_ready":false}},{"name":"b","auth_action":{"type":"magic"}},{"name":"c","auth_action":{"type":"oauth"},"status":{"is_ready":true}}]}"#;
    let auths: Vec<_> = catalog(body).connectors.into_iter().map(|c| c.auth).collect();
    assert_eq!(auths, [ConnectorAuth::Unknown, ConnectorAuth::Unknown, ConnectorAuth::Ready]);
}

#[test]
fn a_not_ready_connector_lists_no_tool() {
    // mcp.bend wrote these with an empty alias (`c  t : #...`): left out now
    let body = r#"{"connectors":[{"id":"c","name":"Odd","status":{"is_ready":"yes"},"auth_action":{"type":"oauth"},"tools":[{"name":"t"}]}]}"#;
    assert!(index_lines_with(&catalog(body), &all_on).is_empty());
}

#[test]
fn image_generation_gets_bises_note_and_web_search_none() {
    let lines = index_lines_with(&catalog(BODY), &all_on);
    let img: Vec<_> = lines.iter().filter(|l| l.starts_with("image_generation ")).collect();
    assert_eq!(img.len(), 2);
    assert!(img.iter().all(|l| l.contains(IMAGE_NOTE) && l.contains("expires")));
    let web = lines.iter().find(|l| l.starts_with("web_search ")).unwrap();
    assert!(!web.contains("NOTE (bise)"), "{web}");
}

#[test]
fn an_off_connector_and_an_off_tool_leave_the_index() {
    let c = catalog(BODY);
    let off_slack = PluginId::mistral("Slack");
    let off_tool = PluginId::mistral("web_search").part(PartKind::Tool, None, "web_search");
    let lines = index_lines_with(&c, &|id| id != &off_slack && id != &off_tool);
    assert!(!lines.iter().any(|l| l.starts_with("c0ffee-01 ")), "{lines:?}");
    assert!(!lines.iter().any(|l| l.starts_with("web_search ")), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("c0ffee-02 slack ")), "{lines:?}");
    assert_eq!(lines.len(), index_lines_with(&c, &all_on).len() - 6);
}

#[test]
fn alias_normalizes() {
    // the law mcp_alias_normalizes (bend/laws/runtime.bend), moved here
    assert_eq!(normalize_name("github app!"), "github_app");
    assert_eq!(normalize_name("--x.y--"), "x_y");
    assert_eq!(normalize_name("ok-name_1"), "ok-name_1");
    assert_eq!(normalize_name(""), "unnamed");
    assert_eq!(normalize_name(&"a".repeat(300)).len(), 256);
    // vibe: a name of symbols only is unnamed too (mcp.bend left it empty)
    assert_eq!(normalize_name("__"), "unnamed");
}

#[test]
fn alias_dedupes() {
    // the law mcp_alias_dedupes, moved here
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(dedupe("github", &[]), "github");
    assert_eq!(dedupe("github", &s(&["github"])), "github_2");
    assert_eq!(dedupe("github", &s(&["github", "github_2"])), "github_3");
    assert_eq!(dedupe(&"a".repeat(256), &s(&[&"a".repeat(256)])).len(), 256);
}

#[test]
fn every_alias_is_unique() {
    // mcp.bend compared with the earlier names, not their aliases: a, a, a_2
    // gave a_2 twice
    let body = r#"{"connectors":[{"name":"a","status":{"is_ready":true}},{"name":"a","status":{"is_ready":true}},{"name":"a_2","status":{"is_ready":true}}]}"#;
    let names: Vec<_> = catalog(body).connectors.into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["a", "a_2", "a_2_2"]);
}

#[test]
fn bootstrap_url_parity() {
    // the law mcp_bootstrap_url_parity, moved here
    let u = bootstrap_url("https://api.mistral.ai/");
    assert!(u.starts_with("https://api.mistral.ai/v1/connectors/bootstrap?supports_mcp=true"), "{u}");
    assert!(u.contains("include_auth_actionable_connectors=true"), "{u}");
    assert!(u.contains("&builtin_connectors=web_search") && u.contains("&builtin_connectors=image_generation"), "{u}");
}

#[test]
fn a_bad_answer_is_an_error() {
    for b in ["", "{", r#"{"detail":"Internal Server Error"}"#, r#"{"connectors":null}"#, "[1]", r#"{"connectors":[]} x"#] {
        assert!(parse(b).is_err(), "{b:?}");
    }
    assert_eq!(parse(r#"{"connectors":[]}"#), Ok(vec![]));
}

#[test]
fn the_cache_round_trips_without_a_key() {
    let c = Cached { catalog: Catalog { error: Some(Problem::Fetch("HTTP 500".into())), ..catalog(BODY) }, failed_ms: 7 };
    let text = to_cache_text(&c);
    assert_eq!(of_cache_text(&text), c);
    assert!(!text.contains("Bearer") && !text.contains("key"), "{text}");
    assert_eq!(of_cache_text("not json"), Cached::default());
}

#[test]
fn the_plan_follows_ttl_backoff_and_force() {
    let fresh = Cached { catalog: Catalog { fetched_ms: 1_000, ..Catalog::default() }, failed_ms: 0 };
    assert_eq!(plan(&fresh, 1_000 + TTL_MS - 1, false), Plan::Keep);
    assert_eq!(plan(&fresh, 1_000 + TTL_MS, false), Plan::Fetch);
    assert_eq!(plan(&fresh, 1_001, true), Plan::Fetch);
    assert_eq!(plan(&Cached::default(), 5, false), Plan::Fetch);
    let failed = Cached { catalog: Catalog { error: Some(Problem::Fetch("x".into())), ..fresh.catalog.clone() }, failed_ms: 2_000 };
    assert_eq!(plan(&failed, 2_000 + FAIL_BACKOFF_MS - 1, false), Plan::Keep);
    assert_eq!(plan(&failed, 2_000 + FAIL_BACKOFF_MS, false), Plan::Fetch);
}

#[test]
fn load_fetches_once_per_ttl_and_keeps_the_last_good_on_a_failure() {
    let cache = tmp("load");
    let calls = Cell::new(0);
    let ok = || {
        calls.set(calls.get() + 1);
        Ok(BODY.to_string())
    };
    let fail = || {
        calls.set(calls.get() + 1);
        Err("HTTP 500 from api.mistral.ai".to_string())
    };
    let c1 = load_with(&cache, 1_000, false, &boot(&ok));
    assert_eq!((calls.get(), c1.connectors.len(), c1.fetched_ms, c1.error.clone()), (1, 11, 1_000, None));
    // fresh: from the file, no fetch
    assert_eq!(load_with(&cache, 2_000, false, &boot(&ok)), c1);
    assert_eq!(calls.get(), 1);
    // stale and the fetch fails: the last good connectors, and why
    let c2 = load_with(&cache, 1_000 + TTL_MS, false, &boot(&fail));
    assert_eq!((calls.get(), c2.connectors.len(), c2.fetched_ms), (2, 11, 1_000));
    assert_eq!(c2.error, Some(Problem::Fetch("HTTP 500 from api.mistral.ai".into())));
    // within the backoff: no new try
    assert_eq!(load_with(&cache, 1_000 + TTL_MS + 1, false, &boot(&fail)), c2);
    assert_eq!(calls.get(), 2);
    // a bad body is a failure too, the good catalog kept
    let bad = || Ok("{\"detail\":1}".to_string());
    let c3 = load_with(&cache, 1_000 + TTL_MS + FAIL_BACKOFF_MS, false, &boot(&bad));
    assert_eq!(c3.connectors.len(), 11);
    assert!(c3.error.is_some());
    // force fetches at once; a good one clears the error
    let c4 = load_with(&cache, 1_000 + TTL_MS + FAIL_BACKOFF_MS + 1, true, &boot(&ok));
    assert_eq!((calls.get(), c4.error), (3, None));
    let left: Vec<_> = std::fs::read_dir(cache.parent().unwrap()).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains(".tmp")).collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn no_key_or_a_locked_keychain_answers_the_cache_and_says_which() {
    let cache = tmp("nokey");
    load_with(&cache, 1_000, false, &boot(&|| Ok(BODY.to_string())));
    let before = std::fs::read_to_string(&cache).unwrap();
    for (key, problem) in [(Key::NoKey, Problem::NoKey), (Key::Locked, Problem::Locked)] {
        let c = load(&cache, &key, 1_000 + TTL_MS * 2, true);
        assert_eq!((c.connectors.len(), c.error), (11, Some(problem)));
        // nothing written
        assert_eq!(std::fs::read_to_string(&cache).unwrap(), before);
    }
    assert_eq!(Problem::Locked.to_string(), LOCKED);
    assert!(!LOCKED.contains("login") && NO_KEY.contains("login"));
}

#[test]
fn a_locked_keychain_is_never_no_key() {
    use bise_secrets::ReadError;
    let auth = Ok(Some(r#"{"mistral":{"type":"api","key":"k-auth"}}"#.to_string()));
    assert_eq!(key_of(auth, Some("k-env".into())), Key::Key("k-auth".into()), "auth.json wins (BISE-269)");
    assert_eq!(key_of(Ok(None), Some(" k-env ".into())), Key::Key("k-env".into()));
    assert_eq!(key_of(Ok(None), Some(" ".into())), Key::NoKey);
    assert_eq!(key_of(Err(ReadError::Locked), None), Key::Locked);
    assert_eq!(key_of(Err(ReadError::Locked), Some("k-env".into())), Key::Key("k-env".into()));
    assert_eq!(key_of(Err(ReadError::Keychain("x".into())), None), Key::NoKey);
}

#[test]
fn bridges_at_once_fetch_once() {
    let cache = tmp("race");
    let calls = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                load_with(
                    &cache,
                    5_000,
                    false,
                    &boot(&|| {
                        calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        Ok(BODY.to_string())
                    }),
                )
            });
        }
    });
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn the_key_is_auth_jsons_api_key() {
    assert_eq!(key_of_auth(r#"{"mistral":{"type":"api","key":" k1 "}}"#).as_deref(), Some("k1"));
    assert_eq!(key_of_auth(r#"{"mistral":{"type":"oauth","key":"k1"}}"#), None);
    assert_eq!(key_of_auth(r#"{"mistral":{"type":"api","key":""}}"#), None);
    assert_eq!(key_of_auth("nope"), None);
}

#[test]
fn a_catalog_finds_a_connector_by_its_id() {
    let c = catalog(BODY);
    assert_eq!(c.find(&PluginId::mistral("github_app_2")).map(|e| e.id.as_str()), Some("c0ffee-04"));
    assert_eq!(c.find(&PluginId::mistral("x_y").part(PartKind::Tool, None, "t")).map(|e| e.id.as_str()), Some("c0ffee-08"));
    assert!(c.find(&PluginId::plugin("Slack")).is_none());
}

#[test]
fn index_lines_follow_plugins_json() {
    let c = catalog(BODY);
    let st = StateV2 {
        disabled: vec!["mistral:Slack".into()],
        parts_off: vec![PluginId::mistral("web_search").part(PartKind::Tool, None, "web_search")],
        ..StateV2::default()
    };
    let lines = index_lines(&c, &st);
    assert!(!lines.iter().any(|l| l.starts_with("c0ffee-01 ") || l.starts_with("web_search ")), "{lines:?}");
    assert_eq!(lines.len(), index_lines(&c, &StateV2::default()).len() - 6);
}

const ACCOUNTS: &str = r#"{"credentials": [
  {"name": "default", "title": "perso", "is_default": true, "authentication_type": "oauth2", "scope": "user", "status": {"status_type": "valid"}},
  {"name": "account-2", "title": " ", "is_default": false, "status": {"status_type": "invalid"}},
  {"name": "account-3", "status": {"status_type": "rotten"}},
  {"name": "account-4", "status": null},
  {"title": "no name"}
]}"#;

#[test]
fn an_accounts_answer_reads_name_title_default_and_status() {
    let a = parse_accounts(ACCOUNTS).unwrap();
    let row = |a: &ConnectorAccount| (a.name.clone(), a.title.clone(), a.default, a.status);
    assert_eq!(
        a.iter().map(row).collect::<Vec<_>>(),
        vec![
            ("default".into(), Some("perso".into()), true, AccountStatus::Valid),
            ("account-2".into(), None, false, AccountStatus::Invalid),
            ("account-3".into(), None, false, AccountStatus::Unknown),
            ("account-4".into(), None, false, AccountStatus::Unknown),
        ]
    );
    assert!(parse_accounts(r#"{"detail": "no"}"#).is_err());
    assert!(parse_accounts("nope").is_err());
}

/// The fixture's catalog where `c0ffee-02` (slack) lists `body`, and
/// every other ready connector fails.
fn accounts_of(raw: &str, body: &str) -> Result<String, String> {
    if raw == "c0ffee-02" {
        Ok(body.to_string())
    } else {
        Err(format!("HTTP 500 for {raw}"))
    }
}

#[test]
fn accounts_are_read_for_ready_connectors_only_and_a_failed_list_keeps_the_last() {
    let cache = tmp("accounts");
    let asked = std::sync::Mutex::new(Vec::<String>::new());
    let list = |body: &'static str| {
        let asked = &asked;
        move |raw: &str| {
            asked.lock().unwrap().push(raw.to_string());
            accounts_of(raw, body)
        }
    };
    let first = list(ACCOUNTS);
    let c1 = load_with(&cache, 1_000, false, &Fetch { bootstrap: &|| Ok(BODY.to_string()), accounts: &first });
    let mut want: Vec<String> = c1.connectors.iter().filter(|c| has_accounts(c)).map(|c| c.id.clone()).collect();
    let mut got = asked.lock().unwrap().clone();
    want.sort();
    got.sort();
    assert_eq!(got, want, "one list per ready connector that is not a built-in");
    assert!(!got.iter().any(|id| c1.connectors.iter().any(|c| &c.id == id && (BUILTINS.contains(&c.name.as_str()) || c.auth != ConnectorAuth::Ready))));
    let slack = |c: &Catalog| c.connectors.iter().find(|c| c.id == "c0ffee-02").unwrap().accounts.clone();
    assert_eq!(slack(&c1).len(), 4);
    assert!(c1.connectors.iter().filter(|c| c.id != "c0ffee-02").all(|c| c.accounts.is_empty()));
    // the cache keeps them, read back the same
    assert_eq!(cached(&cache), c1);
    // the bootstrap is good and the list fails: the catalog is whole, slack keeps its accounts
    let c2 = load_with(&cache, 2_000, true, &Fetch { bootstrap: &|| Ok(BODY.to_string()), accounts: &|_| Err("HTTP 502".to_string()) });
    assert_eq!((c2.connectors.len(), c2.error.clone(), c2.fetched_ms), (11, None, 2_000));
    assert_eq!(slack(&c2), slack(&c1));
    // a list that answers again replaces them
    let one = list(r#"{"credentials": [{"name": "default", "is_default": true}]}"#);
    let c3 = load_with(&cache, 3_000, true, &Fetch { bootstrap: &|| Ok(BODY.to_string()), accounts: &one });
    assert_eq!(slack(&c3).iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["default"]);
}

#[test]
fn a_cache_from_before_accounts_reads_none() {
    let mut c = catalog(BODY);
    let text = to_cache_text(&Cached { catalog: c.clone(), failed_ms: 0 });
    assert!(!text.contains("\"accounts\""), "no accounts: no key written");
    assert_eq!(of_cache_text(&text).catalog, c);
    c.connectors[0].accounts = parse_accounts(ACCOUNTS).unwrap();
    let back = of_cache_text(&to_cache_text(&Cached { catalog: c.clone(), failed_ms: 0 })).catalog;
    assert_eq!(back, c);
}

#[test]
fn the_accounts_file_has_ready_connectors_on_with_accounts_only() {
    let mut c = catalog(BODY);
    let accts = parse_accounts(ACCOUNTS).unwrap();
    for e in c.connectors.iter_mut() {
        e.accounts = accts.clone();
    }
    let off = StateV2 { disabled: vec!["mistral:slack".into()], ..StateV2::default() };
    let text = accounts_file(&c, &off);
    let v: Value = serde_json::from_str(&text).unwrap();
    let ids: Vec<&str> = v.as_array().unwrap().iter().map(|e| e["cid"].as_str().unwrap()).collect();
    let want: Vec<&str> = c.connectors.iter().filter(|e| e.auth == ConnectorAuth::Ready && e.name != "slack").map(|e| e.id.as_str()).collect();
    assert_eq!(ids, want, "ready ones only, an off connector out");
    // one entry, as the runtime reads it (tests/fake_bootstrap.py writes the same)
    let one = Catalog {
        connectors: vec![ConnectorEntry { name: "slack".into(), display_name: "Slack".into(), id: "c-slack".into(), auth: ConnectorAuth::Ready, tools: vec![], accounts: accts[..2].to_vec() }],
        ..Catalog::default()
    };
    assert_eq!(
        accounts_file(&one, &StateV2::default()),
        "[{\"accounts\":[{\"default\":true,\"name\":\"default\",\"title\":\"perso\"},{\"name\":\"account-2\"}],\"cid\":\"c-slack\",\"group\":\"slack\"}]\n"
    );
    // no accounts: an empty list
    assert_eq!(accounts_file(&catalog(BODY), &StateV2::default()), "[]\n");
    assert!(ACCOUNTS_FILE.ends_with(".json") && include_str!("../../../../bend/runtime/plugins.bend").contains(&format!("\"/{ACCOUNTS_FILE}\"")), "plugins.bend reads the same name");
}
