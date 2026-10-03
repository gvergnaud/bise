//! The agent wire protocol as feed events: `Ev` and the parsers of the
//! runtime lines (live and replayed history).

use crate::{fmt_elapsed, sb, truncate_chars, unescape_md, usage};

// ---- feed events ----

#[derive(Clone)]
pub(crate) enum ToolState {
    Run,
    Ok,
    Fail,
}

// one tool call: enriched by the runtime annotations (name/args/result)
#[derive(Clone)]
pub(crate) struct ToolData {
    pub(crate) id: u32,
    // in memory only (approvals-design.md §3.1, §10): the approvals gate
    // holds the call: the checker runs, or a card waits on you; since when
    pub(crate) gate: Option<(Gate, std::time::Instant)>,
    pub(crate) name: Option<String>,
    pub(crate) args: Option<String>,
    // the source of a code tool (run_typescript args JSON, bash raw
    // command), wire-encoded (tool_code annotation); None otherwise
    pub(crate) code: Option<String>,
    // the model's one-line description of a bash/ts call (tool_intent
    // annotation, BISE-223); None: the call had none
    pub(crate) intent: Option<String>,
    pub(crate) state: ToolState,
    pub(crate) result: Option<(bool, String)>,
    pub(crate) started: std::time::Instant,
    // frozen at finish; None while running (elapsed ticks live)
    pub(crate) elapsed: Option<String>,
    // a long source block shows whole (a click on the tool toggles it)
    pub(crate) expanded: bool,
    // in memory only (BISE-110): a bash box that only sent messages drawn
    // below it in this feed (toolbox::sent_ids); hidden while not
    // `expanded` (ctrl+o shows it)
    pub(crate) quiet: bool,
    // in memory only (BISE-123): a bash/ts box that, closed, hides rows
    // at the width it was last drawn (toolbox::box_lines sets it; the
    // feed's disclosure reads it through toolbox::box_folds)
    pub(crate) clips: std::cell::Cell<bool>,
    // in memory only (BISE-223): in main's feed a bash/ts call is one
    // row until opened (a click: its box; ctrl+o: every box, whole)
    pub(crate) opened: bool,
    // in memory only (BISE-223): the first call of a folded run of done
    // calls in main (`▸ 6 commands`) keeps whether the fold is open
    pub(crate) fold_open: bool,
    // how long it took, frozen at finish (the fold's total)
    pub(crate) took: Option<std::time::Duration>,
}

impl ToolData {
    /// A tool event as the runtime announces it (started or finished),
    /// before its annotations (name, args, code, result) merge in. Only
    /// a running tool keeps its elapsed live.
    pub(crate) fn bare(id: u32, state: ToolState) -> ToolData {
        let started = std::time::Instant::now();
        let (elapsed, took) = match state {
            ToolState::Run => (None, None),
            _ => (Some(fmt_elapsed(started)), Some(started.elapsed())),
        };
        ToolData {
            id,
            name: None,
            args: None,
            code: None,
            intent: None,
            state,
            result: None,
            started,
            elapsed,
            expanded: false,
            quiet: false,
            clips: std::cell::Cell::new(false),
            opened: false,
            fold_open: false,
            gate: None,
            took,
        }
    }
}

/// Your message's mark (contract C3, book §13): `·` sent, `✓` the agent
/// got it (`steering_received`), `✓✓` the model read it (`steered`, or
/// its turn started). Only ever moves up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Mark {
    Sent,
    Received,
    Read,
    // the hub could not deliver it (C2 `undelivered`, BISE-86): `✗`
    Failed,
}

/// Where a call waits in the approvals gate (the hub's `sb gate` lines).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    /// the checker judges it: `checking…` after 250 ms
    Check,
    /// a card waits on you: `? waiting for you`
    Card,
    /// it goes on
    Done,
}

#[derive(Clone)]
pub(crate) enum Ev {
    // your message; the bool: opened whole (a long one folds, BISE-239)
    You(String, Mark, bool),
    // the approvals gate of the running call (`sb gate : check|card|done <n>`)
    Gate(Gate),
    // an inbox item answered (`sb approval`, an answer's `sb route`, or
    // the box's own fold): allowed or not, the sentence, the user's own
    // words under it (a typed answer, a no's note), what the item asked,
    // opened (BISE-307: the full question and the whole answer)
    Approval { ok: bool, text: String, note: String, asked: String, open: bool },
    Assistant(String),
    // the model's reasoning for the message that follows: rendered
    // collapsed as "thought for Ns"; ctrl+o expands every section, a
    // click on the section toggles just that one
    Thinking {
        ms: u128,
        text: String,
        open: bool,
    },
    Tool(ToolData),
    // a sub-call made inside a run_typescript program
    Sub {
        name: String,
        ok: bool,
        preview: String,
    },
    // runtime annotations, merged into the matching Tool by id
    ToolInfo {
        id: u32,
        name: String,
        args: String,
    },
    ToolResult {
        id: u32,
        ok: bool,
        preview: String,
    },
    // the source of a code tool (tool_code annotation: run_typescript,
    // bash): the FULL args, wire-encoded, merged into the matching Tool
    // by id
    ToolCode {
        id: u32,
        code: String,
    },
    // the description of a bash/ts call (tool_intent annotation,
    // BISE-223), merged into the matching Tool by id
    ToolIntent {
        id: u32,
        text: String,
    },
    Turn,
    TurnDone,
    // a compaction started (`≡ compacting`, BISE-90: the wire's count and
    // cause are not shown)
    Compact,
    // the compaction summary; `open`, in memory only: shown under the
    // rail (BISE-90: folded behind `▸` by default)
    Compacted {
        text: String,
        open: bool,
    },
    // token usage of the last model call (hidden; feeds the status row)
    Usage(usage::Usage),
    Warn(String),
    Err(String),
    Info(String),
    // pr-news (pr-design §4, main's feed): a PR's news, `↑ #412 changes
    // asked · dark-mode is on it`; the number links to the PR. `tone`:
    // plain, dim (merged, closed) or red (checks fail; bold on NO_COLOR).
    // `url_row`: `/prs`'s rows, the URL dim under it (designer: for
    // terminals that can't click OSC 8)
    Pr {
        tone: String,
        number: u64,
        url: String,
        text: String,
        url_row: bool,
    },
    // in memory only (BISE-298): a failure in the turn errors' two-line
    // pattern (BISE-293): `glyph head` (✗ error, ? accent), then dim
    // lines (the provider's words, what to do)
    Said {
        glyph: &'static str,
        head: String,
        dim: Vec<String>,
    },
    // in memory only (BISE-245): one dim row `▸ head` that opens on
    // `text` (a click, ctrl+o); the setup's checks. No text: the dim
    // row alone, no glyph (a setup answer's result)
    Fold {
        head: String,
        text: String,
        open: bool,
    },
    Idle,
    Raw(String),
    // switchboard (hub line protocol v2, contract C2): a message between
    // agents. `to` empty: the owner of the feed it is in (v1 `msg-in`).
    // `level`: 3 between agents (`msg`, `msg-in`), 2 an agent writing to
    // the user (`msg-you`, to = "you"). `id`: the message id (`m_3`)
    // when the line carries one (`msg-in`), else empty.
    AgentMsg {
        from: String,
        to: String,
        text: String,
        level: u8,
        id: String,
        // in memory only (not on the wire): a folded message (a report,
        // a brief) is disclosed (BISE-12, a click or feed::toggle_event);
        // a level-3 line shows its whole text (BISE-14)
        open: bool,
        // in memory only: the fold of level-3 lines that starts at this
        // message is open (BISE-14, book §10)
        fold: bool,
    },
    // switchboard (C2 `answered`): main answered an agent's question for
    // the user (level 2); `why` may be empty.
    Answered {
        agent: String,
        question: String,
        answer: String,
        why: String,
        // in memory only: its `▸ why` is disclosed (BISE-14)
        open: bool,
    },
    // in memory only (BISE-271): the turn before it ended at this time
    // (ms since the epoch, the hub's `ts` of its `turn_done` line); no
    // row, not even in debug: a hover on the turn's reply shows it
    Ended(u64),
    // in memory only (BISE-14, book §10): a faint `· 14:31 ·` after a
    // pause of 5 minutes without a line; the text is the time
    TimeMark(String),
    // switchboard: an attention card (`#3 question @docs : text`);
    // `closed`, in memory only (BISE-31, book §12): how it was closed,
    // the hub's word (empty: open)
    Card {
        text: String,
        closed: String,
    },
    // in memory only (BISE-31): the hub closed card `id` (`card-closed :
    // #3 answered`); push_event fades that card in place, never
    // appended; shown as an info line when the card is not in the feed
    CardClosed {
        id: u64,
        res: String,
    },
    // in memory only (C3): a wire line that moves the mark of your last
    // message with this text (push_event applies it, never appended);
    // `or` shows instead when there is none (an injected notification)
    MarkYou {
        text: String,
        mark: Mark,
        or: Option<Box<Ev>>,
    },
    // switchboard (C2 `undelivered`, BISE-86): your message `text` did not
    // reach `name` (stopped, dropped, archived); your line gets `✗` and
    // this line asks `⏎ send again · esc drop` while `open` (in memory:
    // the last one only, until answered)
    Undelivered {
        name: String,
        text: String,
        open: bool,
    },
    // in memory only (BISE-235): a row of a `/release-bise` in main's
    // feed; a `Running` one is replaced by the next release row
    Release(crate::release_row::Row),
}

// the wire carries the model's reasoning wrapped in think markers inside
// the assistant text (the transport the API re-send depends on); the TUI
// never shows the markers: it splits them into a Thinking section
pub(crate) const THINK_START: &str = "<think>";
pub(crate) const THINK_END: &str = "</think>";

pub(crate) fn split_thinking(s: &str) -> Option<(String, String)> {
    // one reply can carry several thinking blocks: every span joins the
    // section, the text around them stays visible
    let mut think: Vec<&str> = Vec::new();
    let mut visible = String::new();
    let mut rest = s;
    while let Some(start) = rest.find(THINK_START) {
        let after = &rest[start + THINK_START.len()..];
        let Some(end) = after.find(THINK_END) else {
            break;
        };
        visible.push_str(&rest[..start]);
        think.push(&after[..end]);
        rest = &after[end + THINK_END.len()..];
        // the wire carries newlines as a literal backslash-n escape
        if let Some(r) = rest.strip_prefix("\\n") {
            rest = r;
        }
    }
    if think.is_empty() {
        return None;
    }
    visible.push_str(rest);
    Some((think.join("\\n"), visible))
}

// --resume / reload: the REPL replays the restored history as the live
// wire lines, each prefixed "history " (runtime/main.bend replay). Two
// lines exist only there: "you : <text>" (a user message: live, the
// client echoes what it sends) and "injected : <text>" (steering and
// notifications the Core committed). Everything else is a live line.
pub(crate) fn strip_history(line: &str) -> (&str, bool) {
    match line.strip_prefix("history ") {
        Some(rest) => (rest, true),
        None => (line, false),
    }
}

pub(crate) fn parse_history_line(line: &str) -> Option<Ev> {
    // a replayed message was committed: the model read it
    if let Some(t) = line.strip_prefix("you : ") {
        return Some(Ev::You(unescape_md(t), Mark::Read, false));
    }
    // steering the Core committed: your message with this text was read;
    // none (a notification): the old info line
    if let Some(t) = line.strip_prefix("injected : ") {
        let text = unescape_md(t);
        let flat = text.replace('\n', " ");
        let info = Ev::Info(format!("injected · {}", truncate_chars(flat.trim(), 110)));
        return Some(Ev::MarkYou { text, mark: Mark::Read, or: Some(Box::new(info)) });
    }
    parse_line(line)
}

// "2/10 · provider 529 (transient) · retry in 4s" -> the warning the
// user reads while the call waits
pub(crate) fn provider_retry_text(t: &str) -> String {
    let parts: Vec<&str> = t.split(" · ").collect();
    match parts.as_slice() {
        // "2/10" failed: the plan is attempt 3/10 after the pause
        [n, why, wait] => {
            let next = n
                .split_once('/')
                .and_then(|(a, b)| Some((a.parse::<u32>().ok()? + 1, b)))
                .map(|(a, b)| format!("retry {}/{}", a, b))
                .unwrap_or_else(|| "retry".into());
            format!(
                "model call failed (attempt {}): {} · {} in {}",
                n,
                why,
                next,
                wait.trim_start_matches("retry in ")
            )
        }
        _ => format!("model call failed: {}", t),
    }
}

pub(crate) fn parse_line(line: &str) -> Option<Ev> {
    if line.is_empty() {
        return None;
    }
    // switchboard: the hub's own lines in a feed
    if let Some(rest) = line.strip_prefix("sb ") {
        return sb::parse_hub_line(rest);
    }
    if line == "--- idle" {
        return Some(Ev::Idle);
    }
    // the session-log facts (BISE-195): for the hub's writer, not the feed
    if line.starts_with("  ev: ") {
        return None;
    }
    // runtime annotations: tool #<id> <name> : <args>
    if let Some(r) = line.strip_prefix("tool #") {
        let (id_s, rest) = r.split_once(' ')?;
        let id: u32 = id_s.parse().ok()?;
        let (name, args) = rest.split_once(" : ").unwrap_or((rest, ""));
        return Some(Ev::ToolInfo {
            id,
            name: name.trim().to_string(),
            args: args.to_string(),
        });
    }
    // tool_intent #<id> : <one line> (bash, run_typescript: BISE-223)
    if let Some(r) = line.strip_prefix("tool_intent #") {
        let (id_s, rest) = r.split_once(" : ")?;
        let id: u32 = id_s.trim().parse().ok()?;
        let text = rest.trim();
        if text.is_empty() {
            return None;
        }
        return Some(Ev::ToolIntent { id, text: text.to_string() });
    }
    // tool_code #<id> : <full args, wire-encoded> (run_typescript only)
    if let Some(r) = line.strip_prefix("tool_code #") {
        let (id_s, rest) = r.split_once(" : ")?;
        let id: u32 = id_s.trim().parse().ok()?;
        return Some(Ev::ToolCode {
            id,
            code: rest.to_string(),
        });
    }
    // tool_result #<id> <ok|fail> : <preview>
    if let Some(r) = line.strip_prefix("tool_result #") {
        let (id_s, rest) = r.split_once(' ')?;
        let id: u32 = id_s.parse().ok()?;
        let (st, preview) = rest.split_once(" : ").unwrap_or((rest, ""));
        return Some(Ev::ToolResult {
            id,
            ok: st.trim() == "ok",
            preview: preview.to_string(),
        });
    }
    // subtool <name> <ok|fail> : <preview>
    if let Some(r) = line.strip_prefix("subtool ") {
        let (name, rest) = r.split_once(' ')?;
        let (st, preview) = rest.split_once(" : ").unwrap_or((rest, ""));
        return Some(Ev::Sub {
            name: name.to_string(),
            ok: st.trim() == "ok",
            preview: preview.to_string(),
        });
    }
    if let Some(r) = line.strip_prefix("core rejected: ") {
        // BR-003: right after an interrupt the in-flight completion has
        // no turn to land on - expected plumbing, not an error
        if r == "no pending completion" || r == "no pending tool result" {
            return Some(Ev::Info(
                "in-flight response dropped (turn interrupted)".into(),
            ));
        }
        return Some(Ev::Err(r.to_string()));
    }
    let Some(o) = line.strip_prefix("  obs: ") else {
        return Some(Ev::Raw(line.to_string()));
    };
    if o == "turn_started" {
        return Some(Ev::Turn);
    }
    if let Some(t) = o.strip_prefix("assistant: ") {
        // tool-call-only replies carry no text
        return if t.is_empty() {
            None
        } else {
            Some(Ev::Assistant(t.to_string()))
        };
    }
    if o == "assistant:" {
        return None;
    }
    if let Some(n) = o.strip_prefix("tool_started #") {
        let id: u32 = n.parse().ok()?;
        return Some(Ev::Tool(ToolData::bare(id, ToolState::Run)));
    }
    if let Some(rest) = o.strip_prefix("tool_finished #") {
        let (id_s, tail) = rest.split_once(' ')?;
        let id: u32 = id_s.parse().ok()?;
        let state = match tail {
            "ok" => ToolState::Ok,
            _ => ToolState::Fail,
        };
        return Some(Ev::Tool(ToolData::bare(id, state)));
    }
    if o.starts_with("tool_result_committed") {
        return None;
    }
    // C3: steering moves the mark of your message, no info line
    if let Some(t) = o.strip_prefix("steering_received: ") {
        return Some(Ev::MarkYou { text: t.to_string(), mark: Mark::Received, or: None });
    }
    if let Some(t) = o.strip_prefix("steered: ") {
        return Some(Ev::MarkYou { text: t.to_string(), mark: Mark::Read, or: None });
    }
    if let Some(t) = o.strip_prefix("notification_received: ") {
        return Some(Ev::Info(format!("notification : {}", t)));
    }
    if let Some(t) = o.strip_prefix("notification_delivered: ") {
        return Some(Ev::Info(format!("notification delivered to the model: {}", t)));
    }
    if let Some(t) = o.strip_prefix("provider_retry: ") {
        return Some(Ev::Warn(provider_retry_text(t)));
    }
    if let Some(t) = o.strip_prefix("harness_restarted: ") {
        return Some(Ev::Err(format!(
            "the harness crashed ({}) and restarted — the current turn is interrupted, the history is restored up to the last model call",
            t
        )));
    }
    if let Some(t) = o.strip_prefix("candidate_discarded: ") {
        // no key: the turn's end says it, once (BISE-294)
        if no_key(t).is_some() {
            return None;
        }
        return Some(Ev::Warn(format!("candidate discarded: {}", t)));
    }
    if o.starts_with("compaction_started #") {
        return Some(Ev::Compact);
    }
    if let Some(t) = o.strip_prefix("context_compaction_failed: ") {
        return Some(Ev::Err(format!("compaction failed: {}", t)));
    }
    if let Some(t) = o.strip_prefix("session_restored: ") {
        return Some(Ev::Info(format!(
            "session restored · {} messages",
            t.trim_end_matches(" messages")
        )));
    }
    if let Some(t) = o.strip_prefix("compaction_done: ") {
        return Some(Ev::Compacted { text: t.to_string(), open: false });
    }
    if let Some(t) = o.strip_prefix("usage: ") {
        return usage::Usage::parse(t).map(Ev::Usage);
    }
    if o == "null_iteration" {
        return Some(Ev::Warn(
            "empty response from the model — retrying".into(),
        ));
    }
    if let Some(t) = o.strip_prefix("turn_done: ") {
        // a completed turn needs no annotation; a failure or an
        // interrupt must never disappear — the turn just stops
        if t == "completed" {
            return Some(Ev::TurnDone);
        }
        if let Some(why) = t.strip_prefix("failed: ") {
            // a call an interrupt stopped mid-answer ("interrupted by
            // main", "by the user"): a stop someone asked for, not a
            // failure
            if why.starts_with("interrupted by ") {
                return Some(Ev::Warn(format!("turn {}", why)));
            }
            // the ChatGPT plan's own lines (subscriptions design, item 5):
            // a limit, plan use off, usage not checked, the sign-in
            // expired: a ▲ that says what to do, not a ✗ failure
            if is_plan_line(why) {
                return Some(Ev::Warn(why.to_string()));
            }
            if let Some(line) = no_key(why) {
                return Some(Ev::Err(line));
            }
            return Some(Ev::Err(format!("turn failed: {}", why)));
        }
        if t == "interrupted" {
            return Some(Ev::Warn("turn interrupted".into()));
        }
        return Some(Ev::Err(format!("turn stopped: {}", t)));
    }
    // the runtime ran out of execution budget mid-turn (never silent)
    if let Some(t) = o.strip_prefix("turn_stalled: ") {
        return Some(Ev::Err(format!("turn stopped: {}", t)));
    }
    Some(Ev::Raw(o.to_string()))
}

/// The openings of the runtime's ChatGPT plan lines (bend/runtime, the
/// designer's final words): matched on these, not the whole line.
const PLAN_LINES: [&str; 4] = [
    "your ChatGPT plan's limit for bise is reached.",
    "ChatGPT plan use is off for bise.",
    "ChatGPT couldn't check your plan's usage.",
    "your ChatGPT sign-in expired.",
];

/// A turn ended on one of the ChatGPT plan's lines.
pub(crate) fn is_plan_line(t: &str) -> bool {
    PLAN_LINES.iter().any(|p| t.starts_with(p))
}

/// The plan's usage page, as the limit line says it, and where it links.
pub(crate) const PLAN_USAGE_TEXT: &str = "chatgpt.com/settings/usage";
pub(crate) const PLAN_USAGE_URL: &str = "https://chatgpt.com/settings/usage";

/// BISE-293: a turn failed on a refused request, as the runtime words
/// it (`provider-pure.bend` refusal): `turn failed: OpenAI refused the
/// request (400). OpenAI said: "..."`. Bise's part (up to its last
/// sentence end before the quote) and the provider's (`OpenAI said:
/// "..."`, "" when it said nothing); None for any other error.
pub(crate) fn refusal_parts(t: &str) -> Option<(&str, &str)> {
    let t = t.strip_prefix("turn failed: ")?;
    let refused = t.find(" refused the ")?;
    match t[refused..].find(" said: \"") {
        Some(j) => {
            let k = refused + t[refused..refused + j].rfind(". ")?;
            Some((&t[..=k], &t[k + 2..]))
        }
        None => t.ends_with('.').then_some((t, "")),
    }
}

// decode the tool_code wire encoding: "\N" newline, "\R" CR, backslash
// doubled (the same reversible encoding the provider wire uses)
pub(crate) fn wire_decode(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < cs.len() {
        if cs[i] == '\\' && i + 1 < cs.len() {
            match cs[i + 1] {
                'N' => {
                    out.push('\n');
                    i += 2;
                    continue;
                }
                'R' => {
                    out.push('\r');
                    i += 2;
                    continue;
                }
                '\\' => {
                    out.push('\\');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

// switchboard (C2 `history`, amended): one line of a page of older feed
// lines, `{pos, line, ts?}`. `ts` is when the hub's transcript wrote the
// line (ms since the epoch); a hub before the amendment sends no `ts`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HistLine {
    pub(crate) pos: usize,
    pub(crate) line: String,
    pub(crate) ts: Option<u64>,
}

// the `lines` of a `history` event; a line without `pos` or `line` is
// skipped
pub(crate) fn parse_history(v: &serde_json::Value) -> Vec<HistLine> {
    let Some(a) = v.get("lines").and_then(|l| l.as_array()) else {
        return Vec::new();
    };
    a.iter()
        .filter_map(|x| {
            Some(HistLine {
                pos: x.get("pos")?.as_u64()? as usize,
                line: x.get("line")?.as_str()?.to_string(),
                ts: x.get("ts").and_then(|t| t.as_u64()),
            })
        })
        .collect()
}

/// The runtime's words for a model whose provider has no key (BISE-294,
/// runtime/provider.bend model_call.key: `no openrouter key yet
/// (OPENROUTER_API_KEY is not set): /provider sets it up`), and the older
/// runtime's bare `OPENROUTER_API_KEY is not set`, as the designer's line:
/// `turn stopped: no OpenRouter key yet. /provider sets it up.`
pub(crate) fn no_key(why: &str) -> Option<String> {
    let why = why.trim();
    let name = if let Some(rest) = why.strip_prefix("no ") {
        let (id, _) = rest.split_once(" key yet (")?;
        why.contains(" is not set)").then(|| crate::models::provider_name(id, ""))?
    } else {
        let var = why.strip_suffix(" is not set")?;
        (var.ends_with("_KEY") && !var.contains(' ')).then(|| crate::models::provider_name("", var))?
    };
    Some(format!("turn stopped: no {} key yet. /provider sets it up.", name))
}
