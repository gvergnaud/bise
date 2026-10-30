//! BISE-194: every fixture's rebuilt state, repair and projection.
use bise_session::{project, read_dir, resume::resume, State};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/session")
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

fn state_json(st: &State) -> Value {
    let system = match &st.system {
        bise_session::types::Text::Inline { text } => json!(text),
        bise_session::types::Text::Blob { blob } => json!({"blob": blob.sha256}),
    };
    let mut usage = serde_json::to_value(&st.usage_total).unwrap();
    usage.as_object_mut().unwrap().remove("usd");
    json!({
        "context": st.context, "queue": st.queue, "system": system,
        "tools": st.tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
        "model": st.model, "limits": st.limits, "counters": st.counters, "usage_total": usage,
    })
}

#[test]
fn every_fixture_resumes_to_its_state_and_projection() {
    let mut n = 0;
    for e in std::fs::read_dir(root()).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(exp) = std::fs::read_to_string(e.path().join("expect.json")) else { continue };
        let exp: Value = serde_json::from_str(&exp).unwrap();
        if exp["open"] == "refused" {
            continue;
        }
        let blobs = std::env::temp_dir().join(format!("bise-resume-blobs-{}", std::process::id()));
        let (st, log, repair) = if exp["open"] == "read_only" {
            let log = read_dir(&e.path()).unwrap();
            (State::rebuild(&log), log, vec![])
        } else {
            let t = std::env::temp_dir().join(format!("bise-resume-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&t);
            copy(&e.path(), &t);
            let r = resume(&t, &blobs, "test").unwrap_or_else(|e| panic!("{name}: {e}"));
            let repair: Vec<Value> = r
                .repaired
                .iter()
                .map(|s| {
                    let ev = r.log.by_seq(*s).unwrap();
                    let mut o = json!({"type": ev.typ, "data": ev.data});
                    if let Some(t) = ev.turn {
                        o["turn"] = json!(t);
                    }
                    o
                })
                .collect();
            let last = r.log.events.last().unwrap();
            assert_eq!(last.typ, "process_opened", "{name}");
            assert_eq!(last.data["resume"], true, "{name}");
            // resuming again changes nothing but a new process_opened
            drop(r.writer);
            let again = resume(&t, &blobs, "test").unwrap();
            assert!(again.repaired.is_empty(), "{name}: a second resume repairs nothing");
            assert_eq!(state_json(&again.state), state_json(&r.state), "{name}");
            (r.state, r.log, repair)
        };
        assert_eq!(state_json(&st), exp["state"], "{name}: state");
        assert_eq!(json!(repair), exp["repair"], "{name}: repair");
        if let Some(p) = exp["projection"].as_str() {
            let want = std::fs::read_to_string(e.path().join(p)).unwrap();
            assert_eq!(project::project(&log, &st, &blobs).unwrap(), want, "{name}: projection");
        }
        n += 1;
    }
    assert_eq!(n, 15);
}

#[test]
fn a_compaction_replays_the_same_without_its_checkpoint() {
    let raw = std::fs::read_to_string(root().join("11-compaction/events.jsonl")).unwrap();
    let no_cp: String = raw.lines().filter(|l| !l.contains("\"type\":\"checkpoint\"")).map(|l| format!("{l}\n")).collect();
    let with = State::rebuild(&bise_session::read_bytes(&[("events.jsonl".into(), raw.into_bytes())]));
    let without = State::rebuild(&bise_session::read_bytes(&[("events.jsonl".into(), no_cp.into_bytes())]));
    assert_eq!(state_json(&with), state_json(&without));
}

#[test]
fn the_text_codecs_mirror_the_cores() {
    use project::*;
    for s in ["a\nb", "a\\nb", "tail\\", "\\\\n", "x\r\ny", "\\N\\R"] {
        assert_eq!(wire_decode(&wire_encode(s)), s);
        let e = escape_nl(s);
        assert!(!e.contains('\n'));
        assert_eq!(escape_nl(&unescape_nl(&e)), e, "{s:?}");
    }
    assert_eq!(unescape_nl("a\\\\nb"), "a\\\\nb", "a backslash takes the next char with it");
    assert_eq!(wire_encode("a\\b\nc\rd"), "a\\\\b\\Nc\\Rd");
    assert_eq!(wire_decode("x\\qy"), "x\\qy", "legacy backslashes stay");
    assert_eq!(base64(b"hello!?"), "aGVsbG8hPw==");
}

#[test]
fn a_resumed_hub_compaction_shape_rebuilds_the_core_history() {
    // BISE-195's shape: compaction_done{summary: the preamble} at the start
    // of the range, the kept messages, then context_injected{summary}
    let l = |seq: u64, typ: &str, data: Value| {
        let must = bise_session::types::must_of(typ);
        format!("{}\n", json!({"seq": seq, "type": typ, "v": 1, "must": must, "data": data}))
    };
    let txt = |t: &str| json!([{"kind": "text", "text": t}]);
    let mut raw = l(1, "session_start", json!({"session": "s", "format": 1, "created_by": "t", "cwd": "/"}));
    raw += &l(2, "user_message", json!({"content": txt("q1"), "delivery": "prompt"}));
    raw += &l(3, "assistant_message", json!({"req": 1, "model": "m", "parts": txt("a1"), "calls": []}));
    raw += &l(4, "user_message", json!({"content": txt("q2"), "delivery": "prompt"}));
    raw += &l(5, "compaction_done", json!({"id": 1, "summary": txt("The earlier conversation was compacted."), "replaces": {"from": 2, "to": 4}, "kept": [4]}));
    raw += &l(6, "context_injected", json!({"kind": "summary", "content": txt("Summary of the earlier conversation:\n<summary>s</summary>")}));
    let log = bise_session::read_bytes(&[("events.jsonl".into(), raw.into_bytes())]);
    let st = State::rebuild(&log);
    assert_eq!(st.context, [5, 4, 6]);
    let p = project::project(&log, &st, Path::new("/nonexistent")).unwrap();
    assert!(p.ends_with("MSG True user : The earlier conversation was compacted.\nMSG False user : q2\nMSG True user : Summary of the earlier conversation:\\n<summary>s</summary>\n"), "{p}");
}
