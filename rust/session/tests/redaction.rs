//! BISE-193: secrets never reach the file (fixture 14).
use bise_session::{read_dir, Redactor, Writer};
use serde_json::Value;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/session/14-redaction")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-redact-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// auth.json and .env of the fixture's secrets.json, as files of a home
fn home_files(t: &std::path::Path) -> (PathBuf, PathBuf) {
    let s: Value = serde_json::from_str(&std::fs::read_to_string(fixture().join("secrets.json")).unwrap()).unwrap();
    let auth = t.join("auth.json");
    // the real layout: {"<provider>": {"type": "api", "key": …}}
    let mut a = serde_json::Map::new();
    for (k, v) in s["auth.json"].as_object().unwrap() {
        a.insert(k.clone(), serde_json::json!({"type": "api", "key": v}));
    }
    std::fs::write(&auth, Value::Object(a).to_string()).unwrap();
    let env = t.join(".env");
    let lines: String = s[".env"].as_object().unwrap().iter().map(|(k, v)| format!("export {k}=\"{}\"\n", v.as_str().unwrap())).collect();
    std::fs::write(&env, lines).unwrap();
    (auth, env)
}

#[test]
fn the_writer_replaces_known_values_and_key_shapes() {
    let t = tmp("fixture");
    let (auth, env) = home_files(&t);
    let input = std::fs::read_to_string(fixture().join("input.jsonl")).unwrap();
    let want = read_dir(&fixture()).unwrap();
    let dir = t.join("s");
    let mut lines = input.lines().map(|l| serde_json::from_str::<Value>(l).unwrap());
    let first = lines.next().unwrap();
    let mut w = Writer::create(&dir, &t.join("blobs"), first["data"].clone(), "test").unwrap();
    w.redactor = Some(Redactor::from_home(&auth, &[env]));
    for e in lines.skip(1) {
        w.append(e["type"].as_str().unwrap(), e["data"].clone(), e["turn"].as_u64()).unwrap();
    }
    let got = read_dir(&dir).unwrap();
    let got: Vec<_> = got.events.iter().map(|e| (e.seq, e.typ.clone(), e.data.clone())).collect();
    let want: Vec<_> = want.events.iter().map(|e| (e.seq, e.typ.clone(), e.data.clone())).collect();
    // process_opened is the writer's own (pid, writer)
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(&want) {
        if g.1 != "process_opened" {
            assert_eq!(g, w);
        }
    }
    let raw = std::fs::read_to_string(dir.join("events.jsonl")).unwrap();
    assert!(!raw.contains("sk-ant-api03") && !raw.contains("hunter2"), "no key on disk");
    assert!(raw.contains("sk-not-a-key stays"));
}

#[test]
fn shapes_need_their_length_and_a_word_start() {
    let r = Redactor::new(vec![]);
    let k = "sk-ant-api03-abcdefghijklmnopqrstuvwxyz";
    assert_eq!(r.text(&format!("key={k}.")), "key=«redacted:anthropic».");
    assert_eq!(r.text("sk-ant-short"), "sk-ant-short");
    assert_eq!(r.text("xsk-ant-api03-abcdefghijklmnopqrstuvwxyz"), "xsk-ant-api03-abcdefghijklmnopqrstuvwxyz");
    assert_eq!(r.text("AKIAABCDEFGHIJKLMNOP and ghp_0123456789abcdefghijklmnopqrstuvwxyzAB"), "«redacted:aws» and «redacted:github»");
    assert_eq!(r.text("é sk-proj-ABCDEFGHIJKLMNOPQRSTUVWXYZ012345 é"), "é «redacted:openai» é");
}

/// A fake Anthropic signature with a '+AKIA…' run, the shape of the
/// 2026-10-03 incident (ambient-lead: 400 "Invalid `signature` in
/// `thinking` block" on every turn after a relaunch).
const SIG: &str = "CAQSow8KEAgSGAI4AUIIdGhpbmtpbmcSDKB3+AKIAKubtGw81qvCY6GbTERv8N1A5H98um7C/t++Kkfkeq==";

/// A session with one turn whose assistant message carries `parts`,
/// written through a redactor; the events as written.
fn turn_log(t: &std::path::Path, parts: Value) -> (PathBuf, Vec<Value>) {
    let dir = t.join("s");
    let start = serde_json::json!({"session": "s-test", "format": 1, "created_by": "test", "cwd": "/fake"});
    let mut w = Writer::create(&dir, &t.join("blobs"), start, "test").unwrap();
    w.redactor = Some(Redactor::new(vec![]));
    w.append("context_set", serde_json::json!({"system": {"text": "You are bise."}, "tools": [{"name": "bash", "description": "Run."}]}), None).unwrap();
    w.append("turn_started", serde_json::json!({"cause": "user"}), Some(1)).unwrap();
    w.append("user_message", serde_json::json!({"content": [{"kind": "text", "text": "hi"}], "delivery": "prompt"}), Some(1)).unwrap();
    let (_, data) = w
        .append_data("assistant_message", serde_json::json!({"req": 1, "model": "fake/m", "parts": parts, "calls": [], "stop": "end"}), Some(1))
        .unwrap();
    w.append("turn_ended", serde_json::json!({"outcome": "done"}), Some(1)).unwrap();
    (dir, vec![data])
}

#[test]
fn a_thinking_signature_is_never_redacted() {
    let t = tmp("sig");
    let key = "AKIAABCDEFGHIJKLMNOP";
    let parts = serde_json::json!([
        {"kind": "thinking", "text": format!("the key {key} leaked"), "signature": SIG, "provider": "anthropic"},
        {"kind": "redacted_thinking", "data": format!("x+{key}"), "provider": "anthropic"},
        {"kind": "text", "text": format!("signature: {key}")}
    ]);
    let (dir, written) = turn_log(&t, parts);
    let p = &written[0]["parts"];
    assert_eq!(p[0]["signature"], SIG, "the signature is written byte for byte");
    assert_eq!(p[1]["data"], format!("x+{key}"), "redacted thinking data too");
    assert_eq!(p[0]["text"], "the key «redacted:aws» leaked", "the thinking text is still redacted");
    assert_eq!(p[2]["text"], "signature: «redacted:aws»", "a text part is still redacted");
    let raw = std::fs::read_to_string(dir.join("events.jsonl")).unwrap();
    assert!(raw.contains(SIG));
    // a 'signature' key outside a thinking part is any other string
    let r = Redactor::new(vec![]);
    let mut v = serde_json::json!({"args": {"signature": key}});
    r.value(&mut v);
    assert_eq!(v["args"]["signature"], "«redacted:aws»");
}

#[test]
fn a_signature_a_redactor_rewrote_projects_unsigned() {
    let t = tmp("marker");
    // a log from before the fix: the signature carries the marker
    let bad = SIG.replace("AKIAKubtGw81qvCY6GbTERv8N1A5H98um7C", "«redacted:aws»");
    assert_ne!(bad, SIG);
    let parts = serde_json::json!([
        {"kind": "thinking", "text": "plan", "signature": bad, "provider": "anthropic"},
        {"kind": "text", "text": "ok"}
    ]);
    let (dir, _) = turn_log(&t, parts);
    let log = read_dir(&dir).unwrap();
    let text = bise_session::project::project(&log, &bise_session::State::rebuild(&log), &t.join("blobs")).unwrap();
    let line = text.lines().find(|l| l.starts_with("MSG False assistant")).unwrap();
    assert_eq!(line, "MSG False assistant : <think>plan</think>ok", "no BENDSIG: the Core drops the block");
    // a good signature still rides
    let t2 = tmp("good");
    let parts = serde_json::json!([{"kind": "thinking", "text": "plan", "signature": SIG, "provider": "anthropic"}, {"kind": "text", "text": "ok"}]);
    let (dir, _) = turn_log(&t2, parts);
    let log = read_dir(&dir).unwrap();
    let text = bise_session::project::project(&log, &bise_session::State::rebuild(&log), &t2.join("blobs")).unwrap();
    assert!(text.contains(&format!("<think>plan\\nBENDSIG::{SIG}</think>ok")), "{text}");
}

#[test]
fn short_values_are_not_redacted_and_the_longest_wins() {
    let r = Redactor::new(vec![("A".into(), "abc".into()), ("B".into(), "longsecret-1".into()), ("C".into(), "longsecret-12".into())]);
    assert_eq!(r.text("abc longsecret-12 longsecret-1"), "abc «redacted:C» «redacted:B»");
}
