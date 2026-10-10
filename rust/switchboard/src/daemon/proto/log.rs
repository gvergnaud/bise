//! The window's `/log` (`HubCmd::Log`, TP-N2, architect m_17272): a page
//! of an agent's raw session. The rows are `bise_session::history`'s, the
//! TUI's `/log` builder (every text field redacted inside it); here they
//! only become wire rows ([`items`], bodies cut to TOOL_TEXT_CAP) and a
//! page (`bise_proto::log::page`: rows and bytes capped).

use bise_proto::log::{page, LogBody, LogBodyKind, LogItem, LogRole};
use bise_proto::thread::TOOL_TEXT_CAP;
use bise_session::history::{history, iso_ms, Body, Item, Role, Source};
use bise_session::tool_output::cut_to;
use bise_session::{Log, Redactor};
use std::path::Path;

/// The history's rows as the wire's, in the same order.
pub(super) fn items(rows: &[Item], log: &Log) -> Vec<LogItem> {
    rows.iter().map(|it| item(it, log)).collect()
}

fn item(it: &Item, log: &Log) -> LogItem {
    let (role, ok) = match &it.role {
        Role::You => (LogRole::You, None),
        Role::Assistant => (LogRole::Assistant, None),
        Role::Thinking => (LogRole::Thinking, None),
        Role::Call => (LogRole::Call, None),
        Role::Result { ok } => (LogRole::Result, Some(*ok)),
        Role::Message => (LogRole::Message, None),
        Role::Injected => (LogRole::Injected, None),
        Role::System => (LogRole::System, None),
        Role::Tools => (LogRole::Tools, None),
        Role::Summary => (LogRole::Summary, None),
        Role::Error => (LogRole::Error, None),
        Role::Event => (LogRole::Event, None),
    };
    let body = |kind, lang: Option<&str>, text: &str| {
        let o = cut_to(text, TOOL_TEXT_CAP);
        Some(LogBody { kind, lang: lang.map(str::to_string), text: o.text, cut: o.cut })
    };
    let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
    LogItem {
        seq: it.seq,
        turn: it.turn,
        at_ms: log.events.get(it.idx).and_then(|e| iso_ms(&e.at)),
        role,
        ok,
        tool: some(&it.tool),
        head: it.head.clone(),
        body: match &it.body {
            Body::None => None,
            Body::Md(t) => body(LogBodyKind::Md, None, t),
            Body::Code { lang, text } => body(LogBodyKind::Code, Some(lang), text),
            Body::Plain(t) => body(LogBodyKind::Plain, None, t),
        },
        rule: it.rule.clone(),
        tokens: it.tokens.map(|(n, _)| n),
        estimated: it.tokens.is_some_and(|(_, e)| e),
        meta: some(&it.meta),
    }
}

/// A page of the session log in `dir` (the agent's current session):
/// the rows with a seq under `before`, and whether older ones remain.
pub(super) fn answer(dir: &Path, blobs: &Path, redact: &Redactor, before: Option<u64>) -> Result<(Vec<LogItem>, bool), String> {
    let log = bise_session::read_dir(dir).map_err(|e| format!("no session log: {e}"))?;
    let rows = history(&Source { log: &log, blobs, redact });
    Ok(page(items(&rows, &log), before))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// a session of one turn with `key` in every text the rows show: his
    /// prompt, a call's id (a result's meta), its args, its result, the
    /// reply
    fn planted(dir: &Path, key: &str) {
        let line = |seq: u64, turn: Option<u64>, typ: &str, data: serde_json::Value| {
            let mut v = json!({"seq": seq, "at": format!("2026-10-02T14:02:{:02}.000Z", seq), "type": typ, "v": 1, "data": data});
            if let Some(t) = turn {
                v["turn"] = t.into();
            }
            v.to_string()
        };
        let text = |t: &str| json!([{"kind": "text", "text": t}]);
        let id = format!("call_{key}");
        let lines = [
            line(1, None, "session_start", json!({"session": "s-1", "format": 1, "created_by": "bise", "cwd": "/w"})),
            line(2, Some(1), "turn_started", json!({"cause": "user"})),
            line(3, Some(1), "user_message", json!({"content": text(&format!("use {key} for the api")), "delivery": "prompt"})),
            line(4, Some(1), "assistant_message", json!({"req": 1, "model": "m", "parts": [], "calls": [{"id": id, "name": "bash", "args": format!("curl -H 'auth: {key}' x")}]})),
            line(5, Some(1), "tool_result", json!({"call": id, "ok": true, "content": text(&format!("token {key} ok")), "exit": 0})),
            line(6, Some(1), "assistant_message", json!({"req": 2, "model": "m", "parts": [{"kind": "text", "text": format!("done with {key}")}], "calls": []})),
            line(7, Some(1), "turn_ended", json!({"outcome": "done"})),
        ];
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("events.jsonl"), lines.join("\n") + "\n").unwrap();
    }

    /// Laws (architect m_17272): the window's rows are the TUI's history()
    /// for the same log, field by field (one builder), and a key the
    /// redactor knows is in no text field of either.
    #[test]
    fn the_windows_rows_are_the_tuis_and_a_key_is_in_neither() {
        let tmp = std::env::temp_dir().join(format!("sb-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let key = "sk-test-0123456789abcdefKEY";
        planted(&tmp, key);
        let redact = Redactor::new(vec![("TEST_KEY".into(), key.into())]);
        let log = bise_session::read_dir(&tmp).unwrap();
        let rows = history(&Source { log: &log, blobs: &tmp, redact: &redact });
        let wire = items(&rows, &log);
        assert_eq!(wire.len(), rows.len());
        assert!(rows.len() >= 4, "{rows:?}");
        for (w, r) in wire.iter().zip(&rows) {
            assert_eq!((w.seq, w.turn, &w.head, w.rule.as_ref(), w.tool.as_deref().unwrap_or(""), w.meta.as_deref().unwrap_or("")), (r.seq, r.turn, &r.head, r.rule.as_ref(), r.tool.as_str(), r.meta.as_str()));
            assert_eq!(w.body.as_ref().map_or("", |b| b.text.as_str()), r.body.text());
        }
        let json = serde_json::to_string(&wire).unwrap();
        let tui = format!("{rows:?}");
        assert!(!json.contains(key) && !tui.contains(key), "the key leaked: {json}");
        assert!(json.contains("you") && json.contains("call") && json.contains("result"), "{json}");
        let (p, more) = answer(&tmp, &tmp, &redact, None).unwrap();
        assert_eq!((p, more), (wire.clone(), false));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
