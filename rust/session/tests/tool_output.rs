//! A tool call's whole output read back from the log (architect m_14218,
//! m_14228): joined by call, ok and time, confirmed by the caller's check.
use bise_session::tool_output::{cut_to, tool_output, Out, Query, WITHIN_MS};
use bise_session::writer::{iso, ms_of_iso};
use bise_session::{read_dir, Writer};
use serde_json::json;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-toolout-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn start() -> serde_json::Value {
    json!({"session": "s-1", "format": 1, "created_by": "test", "cwd": "/"})
}

fn result(call: &str, ok: bool, text: &str) -> serde_json::Value {
    json!({"call": call, "ok": ok, "content": [{"kind": "text", "text": text}]})
}

/// The event times, by seq.
fn at(t: &std::path::Path, seq: u64) -> u64 {
    let log = read_dir(&t.join("s")).unwrap();
    ms_of_iso(&log.by_seq(seq).unwrap().at).unwrap()
}

#[test]
fn iso_reads_back() {
    for ms in [0u64, 1_791_100_001_234, 951_782_400_000 /* 2000-02-29 */, 4_102_444_799_999] {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms);
        assert_eq!(ms_of_iso(&iso(t)), Some(ms), "{}", iso(t));
    }
    assert_eq!(ms_of_iso("2026-10-01 09:14:03.120Z"), None);
    assert_eq!(ms_of_iso("nope"), None);
}

#[test]
fn a_result_is_found_by_call_ok_and_time_and_cut_on_a_char_boundary() {
    let t = tmp("find");
    let blobs = t.join("blobs");
    let mut w = Writer::create(&t.join("s"), &blobs, start(), "test").unwrap();
    let lines: String = (1..=400).map(|i| format!("{i}\n")).collect();
    let a = w.append("tool_result", result("call_1", true, &lines), Some(1)).unwrap();
    // a text big enough for a blob (the writer moves it out of its line)
    let big = "y".repeat(300 * 1024);
    let b = w.append("tool_result", result("call_2", false, &big), Some(1)).unwrap();
    let yes = |_: &str| true;
    let q = |n: u64, ok: bool, at_ms: u64| Query { session: "s", n, name: "bash", ok, at_ms };
    let got = tool_output(&t, &blobs, &q(1, true, at(&t, a)), 4096, &yes).unwrap();
    assert_eq!(got, Out { text: lines.clone(), cut: false });
    let got = tool_output(&t, &blobs, &q(1, true, at(&t, a)), 100, &yes).unwrap();
    assert_eq!((got.text.as_str(), got.cut), (&lines[..100], true));
    // the blob reads back, cut
    let got = tool_output(&t, &blobs, &q(2, false, at(&t, b)), 4096, &yes).unwrap();
    assert_eq!((got.text.len(), got.cut), (4096, true));
    // a missing call, the other ok, too far in time, a refused check, a bad session
    assert_eq!(tool_output(&t, &blobs, &q(3, true, at(&t, a)), 4096, &yes), None);
    assert_eq!(tool_output(&t, &blobs, &q(1, false, at(&t, a)), 4096, &yes), None);
    assert_eq!(tool_output(&t, &blobs, &q(1, true, at(&t, a) + WITHIN_MS + 1), 4096, &yes), None);
    assert_eq!(tool_output(&t, &blobs, &q(1, true, at(&t, a)), 4096, &|_| false), None);
    assert_eq!(tool_output(&t, &blobs, &Query { session: "../x", ..q(1, true, at(&t, a)) }, 4096, &yes), None);
    // a 3-byte char across the cap: cut before it
    let s = format!("{}€tail", "a".repeat(9));
    assert_eq!(cut_to(&s, 10), Out { text: "a".repeat(9), cut: true });
    assert_eq!(cut_to(&s, 12), Out { text: format!("{}€", "a".repeat(9)), cut: true });
}

/// The log keeps the message the model reads, `tool <name> ok: <out>`
/// (bend/core/session.bend tool_result_msg); the window gets `<out>`, the
/// same text the transcript's preview starts (proto_e2e caught it: the
/// check refused every real result).
#[test]
fn the_runtimes_header_is_not_part_of_the_output() {
    use bise_session::tool_output::result_body;
    assert_eq!(result_body("tool bash ok: 1\n2", "bash", true), "1\n2");
    assert_eq!(result_body("tool bash failed: exit 2", "bash", false), "exit 2");
    // another tool's header, the other ok, or none: the text whole
    assert_eq!(result_body("tool read ok: x", "bash", true), "tool read ok: x");
    assert_eq!(result_body("tool bash ok: x", "bash", false), "tool bash ok: x");
    assert_eq!(result_body("1\n2", "bash", true), "1\n2");
    let t = tmp("head");
    let blobs = t.join("blobs");
    let mut w = Writer::create(&t.join("s"), &blobs, start(), "test").unwrap();
    let a = w.append("tool_result", result("call_3", true, "tool bash ok: 1\n2\n3\n"), Some(1)).unwrap();
    let q = Query { session: "s", n: 3, name: "bash", ok: true, at_ms: at(&t, a) };
    let got = tool_output(&t, &blobs, &q, 4096, &|full| full.starts_with("1\n2")).unwrap();
    assert_eq!(got, Out { text: "1\n2\n3\n".into(), cut: false });
}

/// The same command run twice, same preview, different outputs (a test
/// rerun): each transcript line gets its own result (architect m_14228).
#[test]
fn two_runs_with_the_same_preview_each_get_their_own() {
    let t = tmp("twice");
    let blobs = t.join("blobs");
    let mut w = Writer::create(&t.join("s"), &blobs, start(), "test").unwrap();
    let head = "running 40 tests\n".repeat(20);
    let one = w.append("tool_result", result("call_7", true, &format!("{head}first run: 40 passed")), Some(1)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    // a fresh REPL counted from 1 again: the same call id
    let two = w.append("tool_result", result("call_7", true, &format!("{head}second run: 39 passed")), Some(2)).unwrap();
    let preview_ok = |full: &str| full.starts_with(&head);
    let q = |at_ms: u64| Query { session: "s", n: 7, name: "bash", ok: true, at_ms };
    let a = tool_output(&t, &blobs, &q(at(&t, one)), 4096, &preview_ok).unwrap();
    let b = tool_output(&t, &blobs, &q(at(&t, two)), 4096, &preview_ok).unwrap();
    assert!(a.text.ends_with("first run: 40 passed"), "{}", a.text);
    assert!(b.text.ends_with("second run: 39 passed"), "{}", b.text);
}
