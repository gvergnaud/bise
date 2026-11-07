use super::*;
use crate::thread::lines::{read, Rec};

/// `fixtures/obs_lines.jsonl`: a text line of each kind the runtime
/// wrote before the records (sha 2b), and the record line it became; a
/// kind born a record (network_wait) has no `text`.
fn pairs() -> Vec<(Option<String>, String)> {
    include_str!("../../../fixtures/obs_lines.jsonl")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            (v["text"].as_str().map(str::to_string), v["rec"].as_str().unwrap().to_string())
        })
        .collect()
}

/// The old text and the record read the same (the fold, the hub and the
/// TUI can't tell them apart), and the record writes back the same line.
#[test]
fn each_text_line_and_its_record_read_the_same() {
    let mut tags = std::collections::BTreeSet::new();
    for (text, rec) in pairs() {
        let new = read(&rec);
        assert!(matches!(new, Rec::Obs(_)), "{rec}");
        if let Some(text) = &text {
            assert_eq!(read(text), new, "{text}");
        }
        let r = ObsRec::read_rest(rec.strip_prefix(PREFIX).unwrap()).unwrap().1.unwrap();
        assert!(!matches!(r, ObsRec::Unknown { .. }), "{rec}");
        assert_eq!(r.line(), rec);
        tags.insert(r.tag().to_string());
        // a replay reads it too
        assert_eq!(crate::thread::lines::read_history(&rec), new, "history {rec}");
    }
    let missing: Vec<_> = ObsRec::TAGS.iter().filter(|t| !tags.contains(**t)).collect();
    assert!(missing.is_empty(), "no fixture line for {missing:?}");
}

/// A kind this side doesn't know, or a record off its shape: shown, never
/// a guess; a line that only looks like one is the text it is.
#[test]
fn an_unknown_or_bad_record_is_other() {
    let Rec::Obs(o) = read(r#"obs teleported :: {"to":"mars"}"#) else { panic!() };
    assert_eq!(o, Obs::Other(r#"teleported: {"obs":"teleported","to":"mars"}"#.into()));
    let Rec::Obs(o) = read(r#"obs tool_started :: {"call":"x"}"#) else { panic!() };
    assert_eq!(o, Obs::Other(r#"tool_started :: {"call":"x"}"#.into()));
    assert_eq!(read("obs nothing here"), Rec::Raw("obs nothing here".into()));
}

/// The line Bend writes for a compaction's end (bend/laws/session.bend
/// compaction_done_carries_whole_summary, character for character):
/// the whole summary, its newline, tab and quotes back as they were.
#[test]
fn a_compaction_line_carries_the_whole_summary() {
    let rest = r#"- veut\t\"ça\" en entier, sans coupe après quarante caractères"#;
    let line = format!(r###"obs compaction_done :: {{"summary":"## Le user{}n{rest}"}}"###, '\\');
    let Rec::Obs(o) = read(&line) else { panic!() };
    let summary = format!("## Le user{}- veut{}\"ça\" en entier, sans coupe après quarante caractères", char::from(10), char::from(9));
    assert_eq!(o, Obs::CompactionDone(summary));
}
