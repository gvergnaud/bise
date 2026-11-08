//! The tables can't drift from the serde derive (architect m_18326): for
//! every kind of every enum, a value with each field set reads and writes
//! back unchanged (its names and types), and an object with only the tag
//! writes each field as its kind says (written empty, null, or absent).

use super::*;
use serde_json::{json, Map, Value};

/// A table and its enum's read-then-write.
type Rw = fn(Value) -> Result<Value, String>;

fn tables() -> [(Table, Rw); 4] {
    [
        (In::TABLE, |v| In::decode(v).map(|x| x.to_value())),
        (Link::TABLE, |v| Link::decode(v).map(|x| x.to_value())),
        (Fx::TABLE, |v| Fx::decode(v).map(|x| x.to_value())),
        (Feed::TABLE, |v| Feed::decode(v).map(|x| x.to_value())),
    ]
}

/// A value of each kind that is not its default.
fn full(k: FieldKind, name: &str) -> Value {
    use FieldKind::*;
    match k {
        // a body: as the shell filled it (body.rs), what serde reads
        Str | StrOmit | OptStr | OptStrOmit | Body | OptBodyOmit => json!(format!("{name}-x")),
        BodyJson => json!([{"kind": "text", "text": "t"}]),
        Nat | OptNat | OptNatOmit => json!(42),
        Bool | OptBoolOmit => json!(true),
        Json | OptJsonOmit => json!({"k": [1, "two", null]}),
        Strs | OptStrsOmit => json!(["a", "b c"]),
        Nats => json!([3, 5]),
        Jsons => json!([{"type": "x"}, 7]),
        Msgs => json!([{"id": 9, "rel": "peer"}]),
        Ver => json!(2),
        Recs(r) => json!([full_rec(&VIEW, r)]),
        OptJsonsOmit => json!([{"id": 4}, "x"]),
        Flag | Rec(_) | Inline(_) | InlineOpt(_) => unreachable!("a journal field: no serde type (journal.rs)"),
    }
}

/// A record of a table with every field set (a `Recs` field: one item).
fn full_rec(t: &Table, rust: &str) -> Value {
    let k = t.kinds.iter().find(|k| k.rust == rust).unwrap_or_else(|| panic!("{}: no record {rust}", t.name));
    Value::Object(k.fields.iter().map(|f| (f.name.to_string(), full_field(f))).collect())
}

/// A field with a value that is not its default: a typed one's in its
/// Rust type's shape ([`Field::rust`]).
fn full_field(f: &Field) -> Value {
    match f.rust {
        None => full(f.kind, f.name),
        Some("Brief") => json!({"objective": "o", "context": "c", "constraints": ["a"], "done_when": "d", "report_format": "r"}),
        Some("Workspace") => json!({"mode": "worktree", "path": "/w", "branch": "b", "base_commit": "c", "dropped": true, "place": "wt:a", "feature": "f", "state": "placing"}),
        Some("Lifecycle") => json!("stopped"),
        Some("Run") => json!("busy"),
        Some("Option<Declaration>") => json!({"status": "done", "note": "n"}),
        Some("Option<Report>") => json!({"at_ms": 1, "kind": "done", "summary": "s", "decisions": ["d"], "auto": true, "step": 1, "of": 2, "result": {"k": 1}}),
        Some(t) => panic!("{}: no sample for {t}", f.name),
    }
}

/// What a missing field writes back: None when absent.
fn empty(k: FieldKind) -> Option<Value> {
    use FieldKind::*;
    match k {
        Str | Body => Some(json!("")),
        BodyJson => Some(Value::Null),
        Nat => Some(json!(0)),
        Bool => Some(json!(false)),
        OptStr | OptNat | Json => Some(Value::Null),
        Strs | Nats | Jsons | Msgs | Recs(_) => Some(json!([])),
        Ver => Some(json!(1)),
        StrOmit | OptStrOmit | OptBodyOmit | OptNatOmit | OptBoolOmit | OptStrsOmit | OptJsonsOmit | OptJsonOmit => None,
        Flag | Rec(_) | Inline(_) | InlineOpt(_) => unreachable!("a journal field: no serde type (journal.rs)"),
    }
}

/// What a missing field writes back: its declared default, else its kind's.
fn empty_field(f: &Field) -> Option<Value> {
    match (f.default, f.rust) {
        (Some(d), _) => Some(serde_json::from_str(d).expect("a default is JSON")),
        (None, None) => empty(f.kind),
        (None, Some("Brief")) => Some(json!({"objective": "", "context": "", "constraints": [], "done_when": null, "report_format": null})),
        (None, Some("Workspace")) => Some(json!({"mode": "shared", "path": "", "branch": null, "base_commit": null, "dropped": false})),
        (None, Some("Lifecycle")) => Some(json!("active")),
        (None, Some("Run")) => Some(json!("down")),
        (None, Some(_)) => Some(Value::Null),
    }
}

#[test]
fn every_tables_kinds_match_what_serde_writes() {
    for (t, rw) in tables() {
        for k in t.kinds {
            let mut m = Map::new();
            m.insert(t.tag_key.into(), json!(k.tag));
            for f in k.fields {
                m.insert(f.name.into(), full(f.kind, f.name));
            }
            let v = Value::Object(m);
            assert_eq!(rw(v.clone()).unwrap(), v, "{}::{} with every field set", t.name, k.rust);

            let mut want = Map::new();
            want.insert(t.tag_key.into(), json!(k.tag));
            for f in k.fields {
                if let Some(e) = empty_field(f) {
                    want.insert(f.name.into(), e);
                }
            }
            let bare = json!({ t.tag_key: k.tag });
            assert_eq!(rw(bare).unwrap(), Value::Object(want), "{}::{} with only its tag", t.name, k.rust);
        }
    }
}

#[test]
fn tags_are_unique_and_the_table_is_the_enums() {
    for (t, _) in tables() {
        let mut tags: Vec<&str> = t.kinds.iter().map(|k| k.tag).collect();
        let n = tags.len();
        tags.sort();
        tags.dedup();
        assert_eq!(tags.len(), n, "{}: a tag twice", t.name);
    }
    assert_eq!(In::TAGS.len(), In::TABLE.kinds.len());
    // two tags share a Bend constructor only when their fields are the same
    for t in [In::TABLE, Link::TABLE, Fx::TABLE, Feed::TABLE] {
        for a in t.kinds {
            for b in t.kinds.iter().filter(|b| b.bend == a.bend) {
                assert_eq!(a.fields, b.fields, "{}: {} and {} share {}", t.name, a.tag, b.tag, a.bend);
            }
        }
    }
}

#[test]
fn an_unknown_tag_is_kept_whole() {
    let v = json!({"t": "weather", "sky": "blue"});
    let i = In::decode(v.clone()).unwrap();
    assert!(matches!(&i, In::Unknown { tag, .. } if tag == "weather"));
    assert_eq!(i.to_value(), v);
    assert!(matches!(Fx::decode(json!({"fx": "dance"})).unwrap(), Fx::Unknown { .. }));
    assert!(In::decode(json!({"agent": "x"})).is_err(), "no tag: a bug of the sender");
}

#[test]
fn a_step_carries_the_envelope() {
    let s = In::Ready(input::Ready { agent: "docs".into() }).step(7, true, vec![json!({"ok": true})]);
    let v = s.to_value();
    assert_eq!(v, json!({"t": "ready", "agent": "docs", "now": 7, "git": true, "ans": [{"ok": true}]}));
    assert_eq!(Step::decode(v).unwrap(), s);
    let bare = Step::decode(json!({"t": "tick"})).unwrap();
    assert_eq!((bare.now, bare.git, bare.ans.len()), (0, false, 0));
}

#[test]
fn a_feed_record_is_one_line_with_its_kind_in_front() {
    let r = Feed::CardClosed(feed::CardClosed { id: 3, res: "answered".into() });
    let l = r.line();
    assert_eq!(l, r#"sb card-closed :: {"id":3,"res":"answered"}"#);
    let (kind, back) = Feed::read_rest(l.strip_prefix("sb ").unwrap()).unwrap();
    assert_eq!((kind, back.unwrap()), ("card-closed", r));
    // an older text line is the legacy reader's, whatever its text
    for old in ["card-closed : #3 answered", "you : {\"text\": \"x\"}", "you : a :: b", "you : :: x"] {
        assert!(Feed::read_rest(old).is_none(), "{old}");
    }
    assert!(Feed::read_rest("card :: [1]").unwrap().1.is_err(), "not an object: the writer's bug, never a guess");
}

#[test]
fn each_answer_shape_reads_as_its_row_and_writes_back() {
    let kill = json!({"fx": "kill", "agent": "a"});
    for k in link::ANSWER.kinds {
        let mut m = Map::new();
        for f in k.fields {
            let v = match (k.tag, f.name) {
                ("step", "fx") => json!([kill, {"fx": "dance", "x": 1}]),
                _ => full(f.kind, f.name),
            };
            m.insert(f.name.into(), v);
        }
        let v = Value::Object(m);
        let a: Answer = serde_json::from_value(v.clone()).unwrap();
        let rust = match &a {
            Answer::Step(_) => "Step",
            Answer::Need(_) => "Need",
            Answer::Photo(_) => "Photo",
            Answer::View(_) => "View",
            Answer::Skipped(_) => "Skipped",
            Answer::Error(_) => "Error",
            Answer::Done(_) => "Done",
            Answer::Unknown(_) => "Unknown",
        };
        assert_eq!(rust, k.rust, "{}: {v}", k.tag);
        assert_eq!(a.to_value(), v, "{}", k.tag);
    }
}

#[test]
fn an_answer_keeps_what_it_does_not_know() {
    let a = Answer::decode_line(r#"{"fx": [{"fx": "dance"}], "dirty": true, "view": {}}"#).unwrap();
    let Answer::Step(s) = &a else { panic!("{a:?}") };
    assert!(matches!(&s.fx[0], Fx::Unknown { tag, .. } if tag == "dance"), "an unknown effect, not a failed answer");
    // an old sb-core's step without a view
    assert!(matches!(Answer::decode_line(r#"{"fx": [], "dirty": false}"#).unwrap(), Answer::Step(_)));
    let odd = json!({"weather": "blue"});
    assert_eq!(serde_json::from_value::<Answer>(odd.clone()).unwrap(), Answer::Unknown(odd));
    assert!(matches!(Answer::decode_line("{}").unwrap(), Answer::Done(_)));
    assert!(Answer::decode_line("nope").is_err());
}

#[test]
fn an_answer_with_fx_is_never_unknown_whatever_its_items() {
    for fx in [
        json!([]),
        json!([{"fx": "kill"}]),
        json!([{"fx": "kill", "agent": 3}]),
        json!([7, "x", null, {"no": "tag"}, {"fx": 1}]),
        json!("not a list"),
    ] {
        for v in [json!({"fx": fx.clone(), "dirty": true, "view": {}}), json!({"fx": fx.clone()})] {
            let a: Answer = serde_json::from_value(v.clone()).unwrap();
            assert!(matches!(a, Answer::Step(_)), "{v} -> {a:?}");
        }
    }
}

#[test]
fn a_bad_effect_costs_only_itself() {
    let a = Answer::decode_line(
        r#"{"fx": [{"fx": "kill", "agent": "a"}, {"fx": "rt", "agent": "b", "turns": "many"}, {"fx": "kill", "agent": "c"}], "dirty": true, "view": {}}"#,
    )
    .unwrap();
    let Answer::Step(s) = a else { panic!("{a:?}") };
    let kills: Vec<&str> = s.fx.iter().filter_map(|f| if let Fx::Kill(k) = f { Some(k.agent.as_str()) } else { None }).collect();
    assert_eq!(kills, ["a", "c"], "both good effects, in order");
    assert!(matches!(&s.fx[1], Fx::Unknown { tag, raw } if tag == "rt" && raw["turns"] == "many"), "the bad one kept whole");
}

/// The view's records, read and written by serde as VIEW says: with every
/// field set, and from `{}` (each field's declared default).
#[test]
fn the_view_table_is_what_serde_writes() {
    type Rw = fn(Value) -> Value;
    let rws: [(&str, Rw); 3] = [
        ("ViewAgent", |v| serde_json::to_value(serde_json::from_value::<ViewAgent>(v).unwrap()).unwrap()),
        ("ViewMsg", |v| serde_json::to_value(serde_json::from_value::<ViewMsg>(v).unwrap()).unwrap()),
        ("View", |v| View::decode(&v).unwrap().to_value()),
    ];
    assert_eq!(VIEW.kinds.len(), rws.len());
    for (rust, rw) in rws {
        let k = VIEW.kinds.iter().find(|k| k.rust == rust).unwrap();
        let v = full_rec(&VIEW, rust);
        assert_eq!(rw(v.clone()), v, "{rust} with every field set");
        let want: Map<String, Value> = k.fields.iter().filter_map(|f| empty_field(f).map(|e| (f.name.to_string(), e))).collect();
        assert_eq!(rw(json!({})), Value::Object(want), "{rust} from nothing");
    }
    // today's defaults of the keys sb-core always writes
    let bare = View::decode(&json!({})).unwrap();
    assert_eq!((bare.all_agents, bare.next_msg, bare.next_card, bare.all_msgs), (true, 1, 1, false));
    assert!(View::decode(&json!({"agents": 3})).is_err(), "a view that doesn't fit is an error, never a guess");
}

/// One real view per shape (fixtures/core_view.jsonl, sb-core's answers
/// on a throwaway hub with the fake provider): each reads and writes back
/// equal as a JSON value, but an agent's report's nulls (an older writer's
/// `"step":null`: the typed Report reads none and omits it).
#[test]
fn every_captured_view_reads_and_writes_back() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/core_view.jsonl")).unwrap();
    let mut n = 0;
    for l in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let mut v: Value = serde_json::from_str(l).unwrap();
        let read = View::decode(&v).unwrap().to_value();
        for a in v["agents"].as_array_mut().into_iter().flatten() {
            if let Some(r) = a["last_report"].as_object_mut() {
                r.retain(|_, x| !x.is_null());
            }
        }
        assert_eq!(read, v, "{l}");
        n += 1;
    }
    assert!(n >= 4, "one view per shape: {n}");
}

/// The raw text of each object under `key` in a fixture line (the key's
/// value, or each item of its array): serde_json's map sorts its keys,
/// the pin is on the line's own bytes.
fn raw_items<'a>(l: &'a str, key: &str) -> Vec<&'a str> {
    let pat = format!("\"{key}\":");
    let mut out = Vec::new();
    for (at, _) in l.match_indices(&pat) {
        let start = at + pat.len();
        let array = l[start..].starts_with('[');
        if !array && !l[start..].starts_with('{') {
            continue;
        }
        let (mut depth, mut open, mut in_str, mut esc) = (0, 0, false, false);
        for (i, c) in l[start..].char_indices() {
            let i = start + i;
            match c {
                _ if esc => esc = false,
                '\\' if in_str => esc = true,
                '"' => in_str = !in_str,
                _ if in_str => {}
                '{' => {
                    if depth == 0 {
                        open = i;
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        out.push(&l[open..=i]);
                        if !array {
                            break;
                        }
                    }
                }
                ']' if depth == 0 => break,
                _ => {}
            }
        }
    }
    out
}

/// Each item reads into `T` and writes back the same bytes.
fn pin<T: serde::de::DeserializeOwned + serde::Serialize>(l: &str, key: &str) -> usize {
    let items = raw_items(l, key);
    for raw in &items {
        let t: T = serde_json::from_str(raw).unwrap_or_else(|e| panic!("{key}: {e}: {raw}"));
        assert_eq!(serde_json::to_string(&t).unwrap(), *raw, "{key} doesn't write back its bytes");
    }
    items.len()
}

/// Each item reads into `T` and writes back the same keys and values,
/// but a null it omits (an older writer's `"step":null`, read as none).
fn same<T: serde::de::DeserializeOwned + serde::Serialize>(l: &str, key: &str) -> usize {
    let items = raw_items(l, key);
    for raw in &items {
        let v: Value = serde_json::from_str(raw).unwrap();
        let t: T = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{key}: {e}: {raw}"));
        let w = serde_json::to_value(&t).unwrap();
        let (vo, wo) = (v.as_object().unwrap(), w.as_object().unwrap());
        for (k, x) in vo {
            assert!(wo.get(k) == Some(x) || (x.is_null() && !wo.contains_key(k)), "{key}.{k} doesn't write back: {raw}");
        }
        assert!(wo.keys().all(|k| vo.contains_key(k)), "{key} writes a key it didn't read: {w}");
    }
    items.len()
}

/// sb-core's records Rust reads (state::RUST, generated, and the
/// Rust-only ones in its agents' JSON), on every captured view and
/// journal line that carries one: read, then written back byte for byte.
#[test]
fn the_state_types_read_and_write_back_the_fixtures() {
    use state::{Brief, Card, Msg, MsgState, Report, Workspace};
    let read = |f: &str| std::fs::read_to_string(format!("{}/fixtures/{f}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (view, journal) = (read("core_view.jsonl"), read("core_journal.jsonl"));
    let mut n = std::collections::BTreeMap::new();
    for l in view.lines().chain(journal.lines()).filter(|l| l.starts_with('{')) {
        // a view's message; a journal line carries a Sent (its words
        // too, c6), which no Rust type reads
        if !l.contains("\"type\":\"message_sent\"") {
            *n.entry("msg").or_insert(0) += pin::<Msg>(l, "msg");
        }
        *n.entry("card").or_insert(0) += pin::<Card>(l, "card") + pin::<Card>(l, "cards");
        // a view row's message state (the journal's message_state is flat)
        if l.contains("\"msgs\":[{") {
            *n.entry("state").or_insert(0) += pin::<MsgState>(l, "state");
        }
    }
    for (k, c) in &n {
        assert!(*c > 0, "no {k} pinned");
    }
    // the Rust-only ones as the view writes them, equal as JSON: sb-core
    // keeps them opaque, in whatever key order they came (a journal line
    // keeps an older shape: the hub's own readers take it, core_link.rs)
    for l in view.lines().filter(|l| l.starts_with('{')) {
        *n.entry("ws").or_insert(0) += same::<Workspace>(l, "ws");
        *n.entry("brief").or_insert(0) += same::<Brief>(l, "brief");
        *n.entry("last_report").or_insert(0) += same::<Report>(l, "last_report");
    }
    for (k, c) in &n {
        assert!(*c > 0, "no {k} pinned");
    }
    for l in view.lines().filter(|l| l.starts_with('{')) {
        let v: Value = serde_json::from_str(l).unwrap();
        for a in v["agents"].as_array().into_iter().flatten() {
            let w = &a["lifecycle"];
            assert_eq!(serde_json::to_value(serde_json::from_value::<state::Lifecycle>(w.clone()).unwrap()).unwrap(), *w);
            if !a["declared"].is_null() {
                let d = &a["declared"]["status"];
                assert_eq!(serde_json::to_value(serde_json::from_value::<state::Declared>(d.clone()).unwrap()).unwrap(), *d);
            }
        }
    }
}

/// fixtures/core_state_defaults.jsonl: a missing key (or one of another
/// type) reads its default, as sb-core reads it (the same lines are
/// gen/state-codec.bend's `T.defaults_pinned_<n>` laws).
#[test]
fn a_missing_key_reads_as_sb_core_reads_it() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/core_state_defaults.jsonl")).unwrap();
    let mut n = 0;
    for l in text.lines().filter(|l| l.starts_with('{')) {
        let v: Value = serde_json::from_str(l).unwrap();
        let reads = &l[l.find("\"reads\":").unwrap() + 8..l.len() - 1];
        let wrote = match v["record"].as_str().unwrap() {
            "Msg" => serde_json::to_string(&serde_json::from_value::<state::Msg>(v["json"].clone()).unwrap()),
            "Card" => serde_json::to_string(&serde_json::from_value::<state::Card>(v["json"].clone()).unwrap()),
            r => panic!("{r}: no Rust type"),
        };
        assert_eq!(wrote.unwrap(), reads, "{l}");
        n += 1;
    }
    assert!(n >= 2);
    // an enum's other word reads its fallback; not a string: an error
    assert_eq!(serde_json::from_value::<state::Lifecycle>(json!("gone")).unwrap(), state::Lifecycle::Active);
    assert!(serde_json::from_value::<state::Lifecycle>(json!(3)).is_err());
    let none: state::MsgState = serde_json::from_value(json!({"reason": "busy"})).unwrap();
    assert_eq!(none, state::MsgState::Queued { reason: "busy".into() });
}
