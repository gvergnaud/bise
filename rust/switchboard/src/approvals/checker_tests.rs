//! The checker (design §4): the route, the state, the questions, the
//! rule, the cool-down; then [`Runner`] against a fake System One server
//! (both Jev routes, through curl) and a fake one-shot REPL (a chat
//! model).

use super::check::{CheckOut, CheckReq, Env, Net, Runner};
use super::checker::*;
use super::*;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CWD: &str = "/w/repo";

fn call(tool: &str, args: Value) -> Call {
    Call {
        tool: tool.into(),
        args,
        agent: "a".into(),
        cwd: CWD.into(),
        repo: CWD.into(),
        tmp: "/h/.bise/hubs/hx/agents/a/tmp".into(),
        home: "/h".into(),
        bise: "/h/.bise".into(),
        edit_tool: "edit".into(),
        flow: None,
    }
}

fn parts(cmd: &str) -> Vec<Part> {
    parse::parse(cmd).parts
}

fn scores(c: f32, t: f32, s: f32) -> Vec<(String, f32)> {
    vec![(CONTAINED.into(), c), (SERVES_TASK.into(), t), (SECRETS.into(), s)]
}

#[test]
fn the_role_resolves_to_a_route() {
    let none = |_: &str| false;
    let all = |_: &str| true;
    let or = |id: &str| id == "openrouter";
    let small = "mistral/mistral-small-latest";
    let jev = |via| Route::Jev { via, model: String::new() };
    let strip = |r: Route| match r {
        Route::Jev { via, .. } => jev(via),
        r => r,
    };
    // unset: TypeSafe's key, else OpenRouter's, else the small jobs model
    // (Jev tuned beats mistral-small: docs/approvals-eval)
    assert_eq!(strip(Route::of("", small, &all)), jev(Via::TypeSafe));
    assert_eq!(strip(Route::of("", small, &or)), jev(Via::OpenRouter));
    assert_eq!(Route::of("", small, &none), Route::Chat { model: small.into() });
    assert_eq!(Route::of("", "", &none), Route::Off);
    // picked
    assert_eq!(Route::of("off", small, &all), Route::Off);
    assert_eq!(Route::of("typesafe/jev-1.13", small, &none), Route::Jev { via: Via::TypeSafe, model: "jev-1.13.0".into() });
    assert_eq!(
        Route::of("openrouter/typesafe/jev-1.13", small, &none),
        Route::Jev { via: Via::OpenRouter, model: "typesafe/jev-1.13".into() }
    );
    assert_eq!(Route::of("anthropic/claude-haiku-4-5", small, &all), Route::Chat { model: "anthropic/claude-haiku-4-5".into() });
    assert_eq!(Route::of("mistral/mistral-small-latest", small, &all), Route::Chat { model: small.into() });
    assert_eq!(Route::of("off", small, &all).checker(), Checker::Off);
}

/// Design §4.3 and §12: nothing but the parts, a script they run, the
/// user's words and paths. A tool result, a file the command does not
/// run, the agent's own words: none can enter, whatever the call holds.
#[test]
fn no_tool_result_or_file_content_enters_the_state() {
    let dir = std::env::temp_dir().join(format!("bise-checker-state-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.md"), "FILE_CONTENT_NOT_RUN").unwrap();
    std::fs::write(dir.join("build.sh"), "echo SCRIPT_RUN").unwrap();
    let mut c = call(
        "bash",
        json!({"arg": "cat notes.md | wc -l && bash build.sh", "output": "TOOL_RESULT", "thought": "AGENT_WORDS"}),
    );
    c.cwd = dir.clone();
    c.repo = dir.clone();
    let ps = parts("cat notes.md | wc -l && bash build.sh");
    let paths = scripts_of(&ps, &c.cwd);
    assert_eq!(paths, vec![dir.join("build.sh")]);
    let scripts: Vec<Script> =
        paths.iter().map(|p| Script { path: p.clone(), content: std::fs::read_to_string(p).unwrap() }).collect();
    let st = checker_state(&c, &ps, "  build it please ", &scripts, None);
    let text = state_json(&st).to_string();
    for absent in ["TOOL_RESULT", "AGENT_WORDS", "FILE_CONTENT_NOT_RUN"] {
        assert!(!text.contains(absent), "{absent} in {text}");
    }
    assert!(text.contains("SCRIPT_RUN") && text.contains("build it please"), "{text}");
    let j = state_json(&st);
    for k in j.as_object().unwrap().keys() {
        assert!(["commands", "scripts", "task", "folder", "roots", "repo", "tool", "sandbox"].contains(&k.as_str()), "{k}");
    }
    // a script outside the roots is not read into it
    let outside = vec![Script { path: "/etc/x.sh".into(), content: "OUTSIDE".into() }];
    assert!(!state_json(&checker_state(&c, &ps, "", &outside, None)).to_string().contains("OUTSIDE"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_state_is_cut() {
    let long = "x".repeat(10_000);
    let ps = parts(&format!("echo {long} && echo {long}"));
    let script = vec![Script { path: format!("{CWD}/a.sh").into(), content: long.clone() }];
    let st = checker_state(&call("bash", json!({})), &ps, &long, &script, None);
    let n = |v: &[String]| v.iter().map(|s| s.chars().count()).sum::<usize>();
    assert_eq!(n(&st.commands), COMMANDS_MAX);
    assert_eq!(st.commands.len(), 1, "the second part is dropped once the first fills it");
    assert_eq!(st.scripts[0].content.chars().count(), SCRIPT_MAX);
    assert_eq!(st.task.as_ref().unwrap().chars().count(), TASK_MAX);
    assert!(st.task.as_ref().unwrap().ends_with('…'));
    // a connector: the tool and its arguments, no parts
    let st = checker_state(&call("gmail.send_email", json!({"to": "x@y.z"})), &[], "", &[], None);
    assert_eq!(st.tool.as_deref(), Some("gmail.send_email"));
    assert_eq!(st.commands, vec![r#"gmail.send_email {"to":"x@y.z"}"#.to_string()]);
    assert_eq!(st.task, None);
}

#[test]
fn jev_is_asked_five_risks_with_their_criteria() {
    let ps = parts("cargo test -p x");
    let with = checker_state(&call("bash", json!({})), &ps, "fix the tests", &[], None);
    let r = jev_request(&with, "jev-1.13.0");
    assert_eq!(r["model"], "jev-1.13.0");
    let q = r["questions"].as_object().unwrap();
    let mut ids: Vec<&str> = q.keys().map(String::as_str).collect();
    ids.sort();
    assert_eq!(ids, ["destroys", "machine", "others_processes", "publishes", "secrets"]);
    for (id, v) in q {
        assert_eq!(v["type"], "noul", "{id}");
        assert!(!v["criteria"]["true"].as_str().unwrap().is_empty() && !v["criteria"]["false"].as_str().unwrap().is_empty(), "{id}");
    }
    // the risks judge the command text, not what a project script might do
    assert!(q["destroys"]["instructions"].as_str().unwrap().contains("Judge what the command text itself does"));
    assert!(q["secrets"]["instructions"].as_str().unwrap().contains("credential"));
    assert_eq!(r["state"]["commands"][0], "cargo test -p x");
    // the task goes in the state; the questions are the same without one
    assert_eq!(r["state"]["task"], "fix the tests");
    let headless = checker_state(&call("bash", json!({})), &ps, "", &[], None);
    assert_eq!(jev_request(&headless, "m")["questions"].as_object().unwrap().len(), 5);
}

#[test]
fn jev_answers_are_read_strictly() {
    let ok: Value = serde_json::from_str(&answers(0.03, 0.01)).unwrap();
    let s = jev_scores(&ok).unwrap();
    assert_eq!(s.scores.len(), 5);
    assert_eq!(s.input_tokens, 1000);
    assert_eq!(s.scores[0].0, PUBLISHES);
    assert!((s.scores[0].1 - 0.03).abs() < 1e-6);
    let mut missing = ok.clone();
    missing["answers"].as_object_mut().unwrap().remove("secrets");
    assert!(matches!(jev_scores(&missing), Err(CheckErr::BadAnswer(_))));
    let mut out_of_range = ok.clone();
    out_of_range["answers"]["destroys"]["noul"] = json!(1.7);
    assert!(jev_scores(&out_of_range).is_err());
    // some routes wrap the number
    let mut wrapped = ok.clone();
    wrapped["answers"]["machine"]["noul"] = json!({"probability": 0.04});
    assert!(jev_scores(&wrapped).is_ok());
    assert!(jev_scores(&json!({"error": "x"})).is_err());
}

#[test]
fn jevs_rule_and_its_reason_words() {
    let at = |risk: f32, secrets: f32| -> Vec<(String, f32)> {
        JEV_QUESTIONS.iter().map(|q| (q.id.to_string(), if q.id == SECRETS { secrets } else { risk })).collect()
    };
    assert!(decide_jev(&at(0.2, 0.15)).allow);
    assert_eq!(decide_jev(&at(0.21, 0.0)).reason, WHY_UNDOABLE);
    assert_eq!(decide_jev(&at(0.0, 0.16)).reason, WHY_SECRETS);
    assert_eq!(decide_jev(&at(0.9, 0.9)).reason, WHY_SECRETS);
    // one risk is enough
    let mut one = at(0.01, 0.01);
    one[2].1 = 0.5;
    assert_eq!(decide_jev(&one).reason, WHY_UNDOABLE);
    // a missing score counts against
    assert!(!decide_jev(&at(0.01, 0.01)[1..]).allow);
}

#[test]
fn the_rule_and_its_reason_words() {
    let d = decide(&scores(0.95, 0.85, 0.05));
    assert!(d.allow && d.reason.is_empty());
    // the thresholds are inclusive
    assert!(decide(&scores(0.9, 0.8, 0.1)).allow);
    assert_eq!(decide(&scores(0.89, 0.9, 0.0)).reason, WHY_UNDOABLE);
    assert_eq!(decide(&scores(0.95, 0.79, 0.0)).reason, WHY_TASK);
    assert_eq!(decide(&scores(0.2, 0.2, 0.0)).reason, WHY_BOTH);
    assert_eq!(decide(&scores(0.95, 0.95, 0.11)).reason, WHY_SECRETS);
    assert_eq!(decide(&scores(0.1, 0.1, 0.9)).reason, WHY_SECRETS);
    // headless: no serves_task, not held against it; a missing one is
    let headless = vec![(CONTAINED.to_string(), 0.95), (SECRETS.to_string(), 0.0)];
    assert!(decide(&headless).allow);
    assert!(!decide(&[(SECRETS.to_string(), 0.0)]).allow);
    assert!(!decide(&[(CONTAINED.to_string(), 1.0)]).allow);
    // scores never in the words
    assert!(!decide(&scores(0.2, 0.2, 0.0)).reason.contains('0'));
    assert_eq!(scores_line(&scores(0.97, 0.91, 0.02)), "contained 0.97 · serves_task 0.91 · secrets 0.02");
}

#[test]
fn a_chat_model_answers_strict_json() {
    let st = checker_state(&call("bash", json!({})), &parts("ls"), "t", &[], None);
    let r = chat_request(&st);
    assert!(r.starts_with("MODEL default\nMSG system : # bise checker\\n"), "{r}");
    assert_eq!(r.lines().filter(|l| l.starts_with("MSG ")).count(), 2);
    let ok = r#"{"contained": true, "serves_task": true, "secrets": false, "reason": "a read"}"#;
    assert!(decide(&chat_scores(&st, ok).unwrap().scores).allow);
    let fenced = format!("```json\n{ok}\n```");
    assert!(chat_scores(&st, &fenced).is_ok());
    let no = r#"{"contained": false, "serves_task": true, "secrets": false}"#;
    assert_eq!(decide(&chat_scores(&st, no).unwrap().scores).reason, WHY_UNDOABLE);
    for bad in ["sure, it's fine", r#"{"contained": "yes", "serves_task": true, "secrets": false}"#, r#"{"contained": true}"#, ""] {
        assert!(chat_scores(&st, bad).is_err(), "{bad}");
    }
}

#[test]
fn three_errors_cool_it_down_for_two_minutes() {
    let mut h = Health::default();
    assert!(h.may_call(0));
    assert!(!h.record(false, 1_000));
    assert!(!h.record(false, 2_000));
    assert!(h.record(false, 3_000), "the 3rd error: the notice");
    assert!(!h.may_call(3_000 + COOL_MS - 1));
    assert!(h.may_call(3_000 + COOL_MS));
    assert!(h.record(false, 3_000 + COOL_MS), "still failing after the cool-down: once more");
    assert!(!h.may_call(3_000 + COOL_MS + 1));
    assert!(!h.record(false, 3_000 + 2 * COOL_MS), "then quiet");
    assert!(!h.record(true, 3_000 + 3 * COOL_MS));
    assert!(h.may_call(3_000 + 3 * COOL_MS));
    // a success in between: the count starts again
    let mut h = Health::default();
    h.record(false, 0);
    h.record(false, 0);
    h.record(true, 0);
    assert!(!h.record(false, 0) && h.may_call(0));
}

#[test]
fn a_script_part_is_never_cached() {
    let ps = parts("cargo test && bash tools/x.sh");
    let keys = vec![CacheKey::Pattern("cargo test *".into()), CacheKey::Exact("bash tools/x.sh".into())];
    assert_eq!(cache_keys(&ps, &keys), vec![CacheKey::Pattern("cargo test *".into())]);
    let k = vec![CacheKey::Exact("gmail.send_email {}".into())];
    assert_eq!(cache_keys(&[], &k), k);
}

#[test]
fn the_notice_names_who_said_what() {
    let r = Route::Jev { via: Via::TypeSafe, model: "jev-1.13.0".into() };
    let n = notice(&r, &CheckErr::Timeout);
    assert_eq!(n[0], "the checker failed 3 times in a row. for 2 minutes, what it would check asks you.");
    assert_eq!(n[1], "TypeSafe said: \"no answer in 5 s\"");
    assert_eq!(n[2], "/models changes the checker.");
    assert_eq!(CheckErr::Refused { status: 401, said: String::new() }.words(), "the key was refused");
    assert!((jev_cost(1_000_000) - 0.042).abs() < 1e-9);
}

// ---- the runner, against fakes ----

/// A home with `config`, in a fresh folder.
fn home(config: &str) -> (bise_home::Home, std::path::PathBuf) {
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bise-checker-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), config).unwrap();
    (bise_home::Home::at(&dir), dir)
}
static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn req(cmd: &str) -> CheckReq {
    let ps = parts(cmd);
    let keys = ps.iter().map(|p| CacheKey::Exact(p.exact())).collect();
    CheckReq { call: call("bash", json!({"arg": cmd})), parts: ps, keys, task: "run the tests".into(), script: None, denied: None }
}

/// A System One server on 127.0.0.1: answers each request with
/// `reply(request body)`; keeps the requests (head, body).
/// The requests a fake server saw: (head, body).
type Seen = Arc<Mutex<Vec<(String, Value)>>>;

fn server(reply: fn(&Value) -> (u16, String)) -> (u16, Seen) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for conn in l.incoming() {
            let Ok(mut c) = conn else { continue };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head, body) = loop {
                let n = c.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break (String::new(), Vec::new());
                }
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(i) = text.find("\r\n\r\n") {
                    let head = text[..i].to_string();
                    let len: usize = head
                        .lines()
                        .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
                        .unwrap_or(0);
                    if buf.len() >= i + 4 + len {
                        break (head, buf[i + 4..i + 4 + len].to_vec());
                    }
                }
            };
            let v: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let (status, out) = reply(&v);
            s2.lock().unwrap().push((head, v));
            let _ = write!(
                c,
                "HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                out.len(),
                out
            );
        }
    });
    (port, seen)
}

/// Jev's reply: every risk at `risk`, `secrets` at `secrets`.
fn answers(risk: f64, secrets: f64) -> String {
    let a: serde_json::Map<String, Value> = JEV_QUESTIONS
        .iter()
        .map(|q| (q.id.to_string(), json!({"type": "noul", "noul": if q.id == SECRETS { secrets } else { risk }})))
        .collect();
    json!({"model": "jev-1.13.0", "answers": a, "usage": {"input_tokens": 1000, "output_tokens": 0}}).to_string()
}

fn env_with(k: &'static str, v: &'static str) -> Env {
    Box::new(move |n| (n == k).then(|| v.to_string()))
}

#[test]
fn jev_through_typesafe_against_a_fake_server() {
    let (port, seen) = server(|v| {
        let cmd = v["state"]["commands"][0].as_str().unwrap_or("");
        if cmd.contains("rm") {
            (200, answers(0.8, 0.0))
        } else {
            (200, answers(0.03, 0.01))
        }
    });
    let (h, dir) = home(&format!(
        "[roles]\nclassify = \"typesafe/jev-1.13\"\n[providers.typesafe]\nbase_url = \"http://127.0.0.1:{port}/v1\"\n"
    ));
    let r = Runner::with(&h, Box::new(super::check::Wire::default()), env_with("TYPESAFE_API_KEY", "ts-test-key-123"));
    assert_eq!(r.checker(), Checker::Jev);
    let out = r.check(&req("cargo test -p x"));
    assert_eq!(out, CheckOut::Allow { cache: vec![CacheKey::Exact("cargo test -p x".into())] });
    match r.check(&req("rm -rf ../other")) {
        CheckOut::Card { reason, detail } => {
            assert_eq!(reason, WHY_UNDOABLE);
            assert!(detail.contains("destroys 0.80"), "{detail}");
        }
        o => panic!("{o:?}"),
    }
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    let (head, body) = &seen[0];
    assert!(head.starts_with("POST /v1/systemone "), "{head}");
    assert!(head.contains("Authorization: Bearer ts-test-key-123"), "{head}");
    assert_eq!(body["model"], "jev-1.13.0");
    assert_eq!(body["state"]["task"], "run the tests");
    let u = r.usage();
    assert_eq!((u.checks, u.allowed, u.input_tokens), (2, 1, 2000));
    assert!((u.usd - 2000.0 * 0.042 / 1e6).abs() < 1e-12);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn jev_through_openrouter_and_the_default_chain() {
    let (port, seen) = server(|_| (200, answers(0.02, 0.0)));
    // unset: no TypeSafe key, OpenRouter's is there
    let (h, dir) = home(&format!("[providers.openrouter]\nbase_url = \"http://127.0.0.1:{port}/api/v1\"\n"));
    let r = Runner::with(&h, Box::new(super::check::Wire::default()), env_with("OPENROUTER_API_KEY", "or-test-key-456"));
    assert_eq!(r.route(), Route::Jev { via: Via::OpenRouter, model: "typesafe/jev-1.13".into() });
    assert!(matches!(r.check(&req("make lint")), CheckOut::Allow { .. }));
    let seen = seen.lock().unwrap();
    assert!(seen[0].0.starts_with("POST /api/v1/systemone "), "{}", seen[0].0);
    assert!(seen[0].0.contains("Bearer or-test-key-456"));
    assert_eq!(seen[0].1["model"], "typesafe/jev-1.13");
    let _ = std::fs::remove_dir_all(dir);
}

/// One key, every role: a fresh install with one provider's key (in
/// auth.json, as the first run saves it) and main set to its pick has a
/// checker at once, of that provider: auto mode works with any one key.
#[test]
fn one_key_alone_gives_auto_mode_a_checker() {
    for (pid, main, checker) in [
        ("anthropic", "anthropic/claude-opus-5-5", Route::Chat { model: "anthropic/claude-haiku-4-5".into() }),
        ("openai", "openai/gpt-6-astra", Route::Chat { model: "openai/gpt-6-luna".into() }),
        ("google", "google/gemini-3.8-flash", Route::Chat { model: "google/gemini-3.5-flash-lite".into() }),
        ("mistral", "mistral/mistral-medium-latest", Route::Chat { model: "mistral/mistral-small-latest".into() }),
        ("openrouter", "openrouter/anthropic/claude-sonnet-5.5", Route::Jev { via: Via::OpenRouter, model: "typesafe/jev-1.13".into() }),
        ("groq", "groq/openai/gpt-oss-120b", Route::Chat { model: "groq/openai/gpt-oss-20b".into() }),
        ("xai", "xai/grok-4.7", Route::Chat { model: "xai/grok-4.3".into() }),
        ("deepseek", "deepseek/deepseek-v4-pro", Route::Chat { model: "deepseek/deepseek-flash".into() }),
        ("cerebras", "cerebras/gpt-oss-120b", Route::Chat { model: "cerebras/gpt-oss-120b".into() }),
    ] {
        let (h, dir) = home(&format!("[roles]
main = \"{main}\"
"));
        std::fs::write(h.auth_file(), format!("{{\"{pid}\": {{\"type\": \"api\", \"key\": \"k-test\"}}}}")).unwrap();
        let r = Runner::with(&h, Box::new(super::check::Wire::default()), Box::new(|_| None));
        assert_eq!(r.route(), checker, "{pid}");
        assert_ne!(r.checker(), Checker::Off, "{pid}");
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn errors_are_cards_then_a_notice_then_a_cool_down() {
    let (port, seen) = server(|_| (401, r#"{"error": {"message": "invalid key ts-bad-key-999"}}"#.to_string()));
    let (h, dir) = home(&format!(
        "[roles]\nclassify = \"typesafe/jev-1.13\"\n[providers.typesafe]\nbase_url = \"http://127.0.0.1:{port}/v1\"\n"
    ));
    let r = Runner::with(&h, Box::new(super::check::Wire::default()), env_with("TYPESAFE_API_KEY", "ts-bad-key-999"));
    for i in 0..3 {
        match r.check(&req("make")) {
            CheckOut::Card { reason, detail } => {
                assert_eq!(reason, WHY_FAILED);
                assert!(!detail.contains("ts-bad-key-999"), "the key is masked: {detail}");
            }
            o => panic!("{o:?}"),
        }
        assert_eq!(r.take_notice().is_some(), i == 2, "call {i}");
    }
    assert!(r.take_notice().is_none(), "once");
    // cooling: no call goes out
    assert!(matches!(r.check(&req("make")), CheckOut::Card { .. }));
    assert_eq!(seen.lock().unwrap().len(), 3);
    // no key: a card, nothing sent
    let (h2, dir2) = home("[roles]\nclassify = \"typesafe/jev-1.13\"\n");
    let r2 = Runner::with(&h2, Box::new(super::check::Wire::default()), Box::new(|_| None));
    assert_eq!(r2.check(&req("make")), CheckOut::Card { reason: WHY_FAILED.into(), detail: "no key for TypeSafe".into() });
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(dir2);
}

#[test]
fn a_dead_endpoint_is_a_card() {
    // a port nothing listens on
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let (h, dir) = home(&format!(
        "[roles]\nclassify = \"typesafe/jev-1.13\"\n[providers.typesafe]\nbase_url = \"http://127.0.0.1:{port}/v1\"\n"
    ));
    let r = Runner::with(&h, Box::new(super::check::Wire::default()), env_with("TYPESAFE_API_KEY", "k-12345678"));
    assert!(matches!(r.check(&req("make")), CheckOut::Card { reason, .. } if reason == WHY_FAILED));
    let _ = std::fs::remove_dir_all(dir);
}

/// A fake `repl-live` for the one-shot: a script that answers with the
/// line it was given, after checking the request file.
fn fake_repl(dir: &std::path::Path, reply: &str) -> std::path::PathBuf {
    let p = dir.join("repl-live");
    std::fs::write(
        &p,
        format!(
            "#!/bin/sh\ngrep -q '^MSG system : # bise checker' \"$BISE_ONESHOT\" || {{ echo 'ONESHOT_ERR bad request'; exit 0; }}\necho \"model=$BISE_MODEL\" > \"$(dirname \"$0\")/model.txt\"\necho '{}'\n",
            reply
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn a_chat_model_through_the_oneshot() {
    let (h, dir) = home("[roles]\nclassify = \"mistral/mistral-small-latest\"\n");
    let repl = fake_repl(&dir, r#"ONESHOT_OK {"contained": true, "serves_task": true, "secrets": false, "reason": "tests"}"#);
    let r = Runner::with(&h, Box::new(super::check::Wire::default()), Box::new(|_| None)).with_oneshot(repl, dir.clone(), None);
    assert_eq!(r.checker(), Checker::Model);
    assert!(matches!(r.check(&req("cargo test")), CheckOut::Allow { .. }));
    assert_eq!(std::fs::read_to_string(dir.join("model.txt")).unwrap().trim(), "model=mistral/mistral-small-latest");
    // an answer that isn't the JSON: a card, never another checker
    let repl = fake_repl(&dir, "ONESHOT_OK looks fine to me");
    let r = Runner::with(&h, Box::new(super::check::Wire::default()), Box::new(|_| None)).with_oneshot(repl, dir.clone(), None);
    assert!(matches!(r.check(&req("cargo test")), CheckOut::Card { reason, .. } if reason == WHY_FAILED));
    let _ = std::fs::remove_dir_all(dir);
}

/// A network that must not be used.
struct NoNet;
impl Net for NoNet {
    fn post_json(&self, _: &str, _: &str, _: &str, _: Duration) -> Result<(u16, String), CheckErr> {
        panic!("no call expected")
    }
    fn chat(&self, _: &str, _: &str, _: Duration) -> Result<String, CheckErr> {
        panic!("no call expected")
    }
}

#[test]
fn off_asks_and_a_role_change_is_seen() {
    let (h, dir) = home("[roles]\nclassify = \"off\"\n");
    let r = Runner::with(&h, Box::new(NoNet), Box::new(|_| None));
    assert_eq!(r.checker(), Checker::Off);
    assert_eq!(r.check(&req("make")), CheckOut::Card { reason: CHECKER_OFF.into(), detail: "off".into() });
    assert!(!r.sync(), "nothing changed");
    // the role changes: sync says so (the hub clears the caches)
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(dir.join("config.toml"), "[roles]\nclassify = \"mistral/mistral-small-latest\"\n").unwrap();
    let t = std::time::SystemTime::now() + Duration::from_secs(2);
    let f = std::fs::File::options().write(true).open(dir.join("config.toml")).unwrap();
    f.set_modified(t).unwrap();
    assert!(r.sync());
    assert_eq!(r.checker(), Checker::Model);
    assert!(!r.sync());
    let _ = std::fs::remove_dir_all(dir);
}

/// brief 1e: the rerun of a command the sandbox stopped carries what it
/// tried (a path, cut), and the prompt names the field.
#[test]
fn a_sandbox_rerun_says_what_the_sandbox_stopped() {
    let ps = parts("touch ~/Desktop/x.txt");
    let line = "the sandbox stopped this command: a write to /h/Desktop/x.txt, outside the repo. if allowed, it runs again, without the sandbox.";
    let st = checker_state(&call("bash", json!({})), &ps, "t", &[], Some(line));
    assert_eq!(state_json(&st)["sandbox"], json!(line));
    assert!(chat_system(&st).contains("`sandbox`"));
    let long = "x".repeat(1000);
    let st = checker_state(&call("bash", json!({})), &ps, "t", &[], Some(&long));
    assert!(st.sandbox.unwrap().chars().count() <= SANDBOX_MAX);
    let st = checker_state(&call("bash", json!({})), &ps, "t", &[], None);
    assert!(state_json(&st).get("sandbox").is_none());
}
