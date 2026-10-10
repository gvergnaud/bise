//! The window's `/log` (TP-N2, architect m_17272): an agent's raw session,
//! every entry in order, as the TUI's `/log` lists it. The rows are built
//! by `bise_session::history` (the TUI's builder, moved there), every text
//! field redacted inside it; the hub pages them (`HubCmd::Log`, newest
//! first by seq, capped by items and by bytes) and sends `HubEv::Log`.
use serde::{Deserialize, Serialize};

/// What an entry is (the TUI's role column; `system` and `tools` are the
/// context's system prompt and tool list).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LogRole {
    /// his turn (a prompt or a steer)
    You,
    Assistant,
    Thinking,
    /// a tool call (its name in `tool`)
    Call,
    /// a tool result (`ok` says how it went)
    Result,
    /// from or to another agent
    Message,
    /// what bise put in the context: notes, task status, hub state
    Injected,
    System,
    Tools,
    /// a compaction's summary
    Summary,
    Error,
    /// the rest: model set, process opened, queued input, a turn's or a
    /// compaction's rule (`rule` set)
    Event,
    #[serde(other)]
    Unknown,
}

/// How a body draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LogBodyKind {
    /// markdown
    Md,
    /// code in a box, colored by `lang`
    Code,
    /// raw text in a box, as is
    Plain,
    #[serde(other)]
    Unknown,
}

/// An entry's body, cut to [`crate::thread::TOOL_TEXT_CAP`] bytes (`cut`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LogBody {
    pub kind: LogBodyKind,
    /// a code body's language (bash, ts, json, diff)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub lang: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub cut: bool,
}

/// One row of `/log`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LogItem {
    /// the log's seq (0: the context's system prompt and tools); pages
    /// go by it (`HubCmd::Log.before`)
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub turn: Option<u64>,
    /// the event's time
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub at_ms: Option<u64>,
    pub role: LogRole,
    /// a result's: did the call work
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ok: Option<bool>,
    /// a call's or a result's tool
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tool: Option<String>,
    /// the one-line summary of its header
    pub head: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub body: Option<LogBody>,
    /// a rule (a turn's start, a compaction): its words; the row is that
    /// rule
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rule: Option<String>,
    /// the tokens the model saw for it, when the log knows them
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tokens: Option<u64>,
    /// `tokens` is an estimate (bytes / 4), shown with a `~`
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub estimated: bool,
    /// a faint line above the body (a result's call id, exit, time)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub meta: Option<String>,
}

/// At most this many rows in one `log` event.
pub const LOG_PAGE_ITEMS: usize = 200;
/// At most about this many bytes of bodies in one `log` event (a page of
/// 200 bodies of 4 KB would be ~800 KB on the hub's socket).
pub const LOG_PAGE_BYTES: usize = 256 * 1024;

/// One page of `items` (oldest first): the newest ones with a seq under
/// `before` (all: none), newest first until [`LOG_PAGE_ITEMS`] rows or
/// [`LOG_PAGE_BYTES`] bytes of bodies, whichever comes first (at least
/// one row); returned oldest first, with whether older rows remain.
pub fn page(items: Vec<LogItem>, before: Option<u64>) -> (Vec<LogItem>, bool) {
    let mut older: Vec<LogItem> = items.into_iter().filter(|i| before.is_none_or(|b| i.seq < b)).collect();
    let mut out = Vec::new();
    let mut bytes = 0;
    while let Some(it) = older.pop() {
        let n = it.body.as_ref().map_or(0, |b| b.text.len());
        if !out.is_empty() && (out.len() >= LOG_PAGE_ITEMS || bytes + n > LOG_PAGE_BYTES) {
            older.push(it);
            break;
        }
        bytes += n;
        out.push(it);
    }
    out.reverse();
    (out, !older.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(seq: u64, body: usize) -> LogItem {
        LogItem {
            seq,
            turn: None,
            at_ms: None,
            role: LogRole::Assistant,
            ok: None,
            tool: None,
            head: format!("row {seq}"),
            body: (body > 0).then(|| LogBody { kind: LogBodyKind::Md, lang: None, text: "x".repeat(body), cut: false }),
            rule: None,
            tokens: None,
            estimated: false,
            meta: None,
        }
    }

    /// Laws (architect m_17272): a page is the newest rows under `before`,
    /// oldest first, at most LOG_PAGE_ITEMS rows and LOG_PAGE_BYTES of
    /// bodies (one row always), `more` while older rows remain; paging by
    /// the first seq walks the whole log once.
    #[test]
    fn a_page_is_capped_by_rows_and_bytes_and_pages_walk_the_log_once() {
        let all: Vec<LogItem> = (1..=450).map(|s| item(s, 10)).collect();
        let (p, more) = page(all.clone(), None);
        assert_eq!((p.len(), more, p.first().map(|i| i.seq), p.last().map(|i| i.seq)), (LOG_PAGE_ITEMS, true, Some(251), Some(450)));
        let (p2, more2) = page(all.clone(), Some(251));
        assert_eq!((p2.len(), more2, p2.last().map(|i| i.seq)), (200, true, Some(250)));
        let mut seen = Vec::new();
        let mut before = None;
        loop {
            let (p, more) = page(all.clone(), before);
            before = p.first().map(|i| i.seq);
            seen.splice(0..0, p.into_iter().map(|i| i.seq));
            if !more {
                break;
            }
        }
        assert_eq!(seen, (1..=450).collect::<Vec<_>>());
        // bytes: 4 KB bodies stop the page near 256 KB
        let big: Vec<LogItem> = (1..=200).map(|s| item(s, 4096)).collect();
        let (p, more) = page(big, None);
        assert!(more && p.len() == LOG_PAGE_BYTES / 4096, "{}", p.len());
        // one row always, even past the cap
        let (p, more) = page(vec![item(1, LOG_PAGE_BYTES * 2)], None);
        assert_eq!((p.len(), more), (1, false));
        assert_eq!(page(Vec::new(), None), (Vec::new(), false));
    }

    #[test]
    fn an_unknown_role_or_body_kind_reads_as_unknown() {
        let i: LogItem = serde_json::from_str(r#"{"seq":3,"role":"weather","head":"h","body":{"kind":"svg","text":"t"}}"#).unwrap();
        assert_eq!((i.role, i.body.map(|b| b.kind)), (LogRole::Unknown, Some(LogBodyKind::Unknown)));
    }
}
