//! One parser per transcript line kind (architect m_10476): a line of an
//! agent's transcript (the runtime's `tool #…`/`  obs: …` lines, the
//! hub's `sb <kind> : …` lines) read into a typed [`Rec`]. The hub's
//! fold ([`super::fold`]) and the TUI (`wire.rs`, `sb.rs`) both read
//! lines through here, so a line means one thing on both sides; when
//! typed transcript records replace the text lines, only this module
//! changes. Pure: std and the line's own escapes, nothing else.

/// `\n` escapes back to newlines (the TUI's `markdown::unescape_md`).
pub fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'n') {
            chars.next();
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

/// A tool line's wire encoding undone (`\N` newline, `\R` return, `\\`
/// a backslash): the runtime's tool_code and tool lines.
pub fn wire_decode(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('N') => {
                    chars.next();
                    out.push('\n');
                    continue;
                }
                Some('R') => {
                    chars.next();
                    out.push('\r');
                    continue;
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

/// A failed bash result's exit code (`exit 1: <output>`).
pub fn exit_code(result: &str) -> Option<i32> {
    let rest = result.strip_prefix("exit ")?;
    let (code, _) = rest.split_once(':')?;
    if code.is_empty() || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    code.parse().ok()
}

/// A failed call's first error line: its output (after `exit N:`) as one
/// line, from the first word that reads like an error
/// (`UnicodeDecodeError: …`), else all of it; None when empty.
pub fn error_line(result: &str) -> Option<String> {
    let r = match exit_code(result) {
        Some(x) => result.strip_prefix(&format!("exit {x}:")).unwrap_or(result),
        None => result,
    };
    let text = r.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    // ASCII lowercase keeps the byte offsets
    let low = text.to_ascii_lowercase();
    let marks = ["error", "fail", "panic", "fatal", "not found", "denied", "cannot", "can't", "no such", "invalid", "exception"];
    let hit = marks.iter().filter_map(|m| low.find(m)).min();
    let from = hit.map_or(0, |p| text[..p].rfind(' ').map_or(0, |s| s + 1));
    Some(text[from..].to_string())
}

/// The files a patch touches (apply_patch, edit and write_file's
/// tool_code), each with its added and removed lines (a move reads
/// `old → new`).
pub fn patch_files(src: &str) -> Vec<(String, usize, usize)> {
    let mut files: Vec<(String, usize, usize)> = Vec::new();
    for l in src.split('\n') {
        let path = l
            .strip_prefix("*** Update File: ")
            .or_else(|| l.strip_prefix("*** Add File: "))
            .or_else(|| l.strip_prefix("*** Delete File: "));
        if let Some(p) = path {
            files.push((p.trim().to_string(), 0, 0));
            continue;
        }
        if let Some(p) = l.strip_prefix("*** Move to: ") {
            if let Some(f) = files.last_mut() {
                f.0 = format!("{} → {}", f.0, p.trim());
            }
            continue;
        }
        if let Some(f) = files.last_mut() {
            if l.starts_with('+') {
                f.1 += 1;
            } else if l.starts_with('-') {
                f.2 += 1;
            }
        }
    }
    files
}

/// A field of a hub line: its own `" : "` is escaped as `" \: "`
/// (sb-core's `line_fields`, core.rs `field_escape`), undone here, then
/// the newlines.
pub fn field(s: &str) -> String {
    unescape(&s.replace(" \\: ", " : "))
}

const THINK_START: &str = "<think>";
const THINK_END: &str = "</think>";

/// The model's thinking out of an assistant text: (thinking, visible), or
/// None when there is none.
pub fn split_thinking(s: &str) -> Option<(String, String)> {
    let mut visible = String::new();
    let mut think: Vec<&str> = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find(THINK_START) {
        let after = &rest[start + THINK_START.len()..];
        let Some(end) = after.find(THINK_END) else { break };
        visible.push_str(&rest[..start]);
        think.push(&after[..end]);
        rest = &after[end + THINK_END.len()..];
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

/// A message id: `m_<digits>`.
pub fn is_msg_id(s: &str) -> bool {
    s.strip_prefix("m_").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// A message id's number: `m_12` is 12; anything else is none.
pub fn msg_id(s: &str) -> Option<u64> {
    s.strip_prefix("m_").filter(|_| is_msg_id(s)).and_then(|n| n.parse().ok())
}

/// One transcript line, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rec {
    /// an empty line
    Empty,
    /// `--- idle`
    Idle,
    /// a session-log fact (`  ev: …`), for the hub's writer only
    Fact,
    /// a line of a known kind that doesn't parse: nothing to show
    Dropped,
    /// `tool #<id> <name> : <args>` (args as on the wire)
    Tool { id: u32, name: String, args: String },
    /// `tool_intent #<id> : <one line>` (bash, run_typescript: BISE-223)
    ToolIntent { id: u32, text: String },
    /// `tool_code #<id> : <full args, wire-encoded>`
    ToolCode { id: u32, code: String },
    /// `tool_result #<id> <ok|fail> : <preview>`
    ToolResult { id: u32, ok: bool, preview: String },
    /// `subtool <name> <ok|fail> : <preview>` (a run_typescript sub-call)
    Sub { name: String, ok: bool, preview: String },
    /// `core rejected: <why>`
    Rejected(String),
    /// the runtime's `  obs: …`
    Obs(Obs),
    /// the hub's `sb <kind> : …`
    Hub(Hub),
    /// a replayed user message (`history you : …`, unescaped)
    HistYou(String),
    /// steering or a notification the core committed (`history
    /// injected : …`, unescaped)
    Injected(String),
    /// any other line (the next line of a message)
    Raw(String),
}

/// The runtime's observations (`  obs: …`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Obs {
    TurnStarted,
    /// the reply's text as on the wire, its `<think>` parts in it; ""
    /// for a tool-call-only reply
    Assistant(String),
    ToolStarted(u32),
    ToolFinished { id: u32, ok: bool },
    /// plumbing nobody shows (`tool_result_committed`)
    Plumbing,
    SteeringReceived(String),
    Steered(String),
    NotificationReceived(String),
    NotificationDelivered(String),
    /// `2/10 · provider 529 (transient) · retry in 4s`
    ProviderRetry(String),
    HarnessRestarted(String),
    CandidateDiscarded(String),
    CompactionStarted,
    CompactionFailed(String),
    SessionRestored(String),
    /// the compaction's summary
    CompactionDone(String),
    /// the last model call's usage, as the runtime writes it
    Usage(String),
    NullIteration,
    TurnDone(TurnEnd),
    TurnStalled(String),
    Other(String),
}

/// How a turn ended (`turn_done: …`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnEnd {
    Completed,
    Failed(String),
    /// a stop someone asked for, not a failure: `interrupted`, or a call
    /// an interrupt stopped mid-answer (`failed: interrupted by main`):
    /// who (`main`, `the user`), when the line says
    Interrupted { by: Option<String> },
    Other(String),
}

/// The approvals gate of the running call (`sb gate : check|card|done <n>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateStep {
    Check,
    Card,
    Done,
}

/// The hub's own lines in a feed (`sb <kind> : <text>`, hub line
/// protocol C2). Texts are unescaped; fields too (their `" \: "`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hub {
    /// his message
    You(String),
    /// `you-id : m_<n>`: the id of the 'you' line right before it
    /// (sb-core writes it in the same step; the feed only)
    YouId(u64),
    /// `steered : m_<n> m_<n>`: the agent read these steered messages
    /// here, mid-turn (sb-core's receipt, the steer-mode deliveries since
    /// the last one)
    Steered(Vec<u64>),
    /// `steer-rx : m_<n> m_<n>`: the agent's runtime received these
    /// steered messages (before it reads them; sb-core's receipt at
    /// ISteerRx, its own cursor apart from steered's)
    SteerRx(Vec<u64>),
    /// his fn context, the raw JSON (S9)
    Context(String),
    /// BISE-86: `undelivered : {name} : {text}`
    Undelivered { to: String, text: String },
    /// what the feed's owner received: `{from} m_<n> : {text}`; `from`
    /// keeps its `@` (an old direct reply to the user)
    MsgIn { from: String, id: String, body: String },
    /// `{from} → {to} m_<n> : {text}` (no id before BISE-110)
    Msg { from: String, to: String, id: String, body: String },
    /// what the feed's owner sent: `sent : {to} : m_<n> : {ask} : {text}`
    Sent { to: String, id: String, ask: bool, body: String },
    /// an agent writing to the user: `msg-you : {from} : {text}`
    MsgYou { from: String, body: String },
    /// main answered an agent for the user
    Answered { agent: String, question: String, answer: String, why: String },
    /// an attention card `#3 question @docs : text`: its id and kind
    /// when its head reads, its body after the head
    Card { text: String, id: Option<u64>, kind: String, body: String },
    Gate(GateStep),
    /// a gate's card answered: `allowed|outside|outside-always|no : who :
    /// what [: note]`
    Approval { how: String, who: String, what: String, note: String },
    /// `card-closed : #3 answered`
    CardClosed { id: u64, res: String },
    /// an answer to an item: `you → @{asker} (answer to card #{id}) : {text}`
    Route { who: String, card: u64, said: String },
    /// pr-news: `pr : tone : number : url : text`
    Pr { state: crate::thread::PrNewsState, number: u64, url: String, text: String },
    /// `artifact : id : agent : title : kind : v`
    Artifact { id: String, agent: String, title: String, kind: String, v: u32 },
    /// `landed : agent : target : from : sha : files : add : del`
    Landed { agent: String, target: String, from: String, sha: String, files: u64, add: u64, del: u64 },
    Spawn(String),
    /// computer use (design §7.3)
    Computer(String),
    Direct(String),
    Warn(String),
    /// a scheduled task set or ended, its JSON
    Scheduled(String),
    /// an interrupt (sb-core writes it in the interrupt's step)
    Stopped(String),
    /// a kind this reader doesn't know, or a known kind's line that
    /// doesn't parse: its kind and unescaped text
    Other { kind: String, text: String },
}

/// A live transcript line, read.
pub fn read(line: &str) -> Rec {
    if line.is_empty() {
        return Rec::Empty;
    }
    if let Some(rest) = line.strip_prefix("sb ") {
        return Rec::Hub(hub(rest));
    }
    if line == "--- idle" {
        return Rec::Idle;
    }
    if line.starts_with("  ev: ") {
        return Rec::Fact;
    }
    if let Some(r) = line.strip_prefix("tool #") {
        return tool(r).unwrap_or(Rec::Dropped);
    }
    if let Some(r) = line.strip_prefix("tool_intent #") {
        return intent(r).unwrap_or(Rec::Dropped);
    }
    if let Some(r) = line.strip_prefix("tool_code #") {
        return code(r).unwrap_or(Rec::Dropped);
    }
    if let Some(r) = line.strip_prefix("tool_result #") {
        return result(r).unwrap_or(Rec::Dropped);
    }
    if let Some(r) = line.strip_prefix("subtool ") {
        return sub(r).unwrap_or(Rec::Dropped);
    }
    if let Some(r) = line.strip_prefix("core rejected: ") {
        return Rec::Rejected(r.to_string());
    }
    match line.strip_prefix("  obs: ") {
        Some(o) => Rec::Obs(obs(o)),
        None => Rec::Raw(line.to_string()),
    }
}

/// A line replayed on `--resume`/reload, its `history ` prefix taken
/// off: the two lines only a replay has (`you : …`, `injected : …`),
/// else the live line.
pub fn read_history(line: &str) -> Rec {
    if let Some(t) = line.strip_prefix("you : ") {
        return Rec::HistYou(unescape(t));
    }
    if let Some(t) = line.strip_prefix("injected : ") {
        return Rec::Injected(unescape(t));
    }
    read(line)
}

fn tool(r: &str) -> Option<Rec> {
    let (id, rest) = r.split_once(' ')?;
    let (name, args) = rest.split_once(" : ").unwrap_or((rest, ""));
    Some(Rec::Tool { id: id.parse().ok()?, name: name.trim().to_string(), args: args.to_string() })
}

fn intent(r: &str) -> Option<Rec> {
    let (id, rest) = r.split_once(" : ")?;
    let text = rest.trim();
    if text.is_empty() {
        return None;
    }
    Some(Rec::ToolIntent { id: id.trim().parse().ok()?, text: text.to_string() })
}

fn code(r: &str) -> Option<Rec> {
    let (id, rest) = r.split_once(" : ")?;
    Some(Rec::ToolCode { id: id.trim().parse().ok()?, code: rest.to_string() })
}

fn result(r: &str) -> Option<Rec> {
    let (id, rest) = r.split_once(' ')?;
    let (st, preview) = rest.split_once(" : ").unwrap_or((rest, ""));
    Some(Rec::ToolResult { id: id.parse().ok()?, ok: st.trim() == "ok", preview: preview.to_string() })
}

fn sub(r: &str) -> Option<Rec> {
    let (name, rest) = r.split_once(' ')?;
    let (st, preview) = rest.split_once(" : ").unwrap_or((rest, ""));
    Some(Rec::Sub { name: name.to_string(), ok: st.trim() == "ok", preview: preview.to_string() })
}

/// A line no feed shows as a step of the turn (F3, proto-lead m_15384): a
/// model call's usage (written just before its reply), plumbing, a
/// message's receipts (they move his marks, draw nothing), an empty model
/// call, the runtime's facts, idle and blank lines. A thinking's time
/// counts from the line before them.
pub fn is_hidden(rec: &Rec) -> bool {
    match rec {
        Rec::Empty | Rec::Idle | Rec::Fact | Rec::Dropped => true,
        Rec::Obs(o) => matches!(
            o,
            Obs::Usage(_) | Obs::Plumbing | Obs::SteeringReceived(_) | Obs::Steered(_) | Obs::NotificationReceived(_) | Obs::NotificationDelivered(_) | Obs::NullIteration
        ),
        _ => false,
    }
}

/// An observation made of the text after its prefix.
type ObsOf = fn(String) -> Obs;

/// The runtime's observation, the `  obs: ` taken off.
pub fn obs(o: &str) -> Obs {
    let s = |t: &str| t.to_string();
    if o == "turn_started" {
        return Obs::TurnStarted;
    }
    if let Some(t) = o.strip_prefix("assistant: ") {
        return Obs::Assistant(s(t));
    }
    if o == "assistant:" {
        return Obs::Assistant(String::new());
    }
    if let Some(n) = o.strip_prefix("tool_started #") {
        return n.parse().map(Obs::ToolStarted).unwrap_or(Obs::Plumbing);
    }
    if let Some(rest) = o.strip_prefix("tool_finished #") {
        let Some((id, tail)) = rest.split_once(' ') else { return Obs::Plumbing };
        return id.parse().map(|id| Obs::ToolFinished { id, ok: tail == "ok" }).unwrap_or(Obs::Plumbing);
    }
    if o.starts_with("tool_result_committed") {
        return Obs::Plumbing;
    }
    let with: [(&str, ObsOf); 11] = [
        ("steering_received: ", Obs::SteeringReceived),
        ("steered: ", Obs::Steered),
        ("notification_received: ", Obs::NotificationReceived),
        ("notification_delivered: ", Obs::NotificationDelivered),
        ("provider_retry: ", Obs::ProviderRetry),
        ("harness_restarted: ", Obs::HarnessRestarted),
        ("candidate_discarded: ", Obs::CandidateDiscarded),
        ("context_compaction_failed: ", Obs::CompactionFailed),
        ("session_restored: ", Obs::SessionRestored),
        ("compaction_done: ", Obs::CompactionDone),
        ("usage: ", Obs::Usage),
    ];
    if o.starts_with("compaction_started #") {
        return Obs::CompactionStarted;
    }
    for (p, f) in with {
        if let Some(t) = o.strip_prefix(p) {
            return f(s(t));
        }
    }
    if o == "null_iteration" {
        return Obs::NullIteration;
    }
    if let Some(t) = o.strip_prefix("turn_done: ") {
        return Obs::TurnDone(match t {
            "completed" => TurnEnd::Completed,
            "interrupted" => TurnEnd::Interrupted { by: None },
            t => match t.strip_prefix("failed: ") {
                Some(why) => match why.strip_prefix("interrupted by ") {
                    Some(by) => TurnEnd::Interrupted { by: Some(s(by)) },
                    None => TurnEnd::Failed(s(why)),
                },
                None => TurnEnd::Other(s(t)),
            },
        });
    }
    if let Some(t) = o.strip_prefix("turn_stalled: ") {
        return Obs::TurnStalled(s(t));
    }
    Obs::Other(s(o))
}

/// A hub line, its `sb ` taken off: `<kind> : <text>`.
pub fn hub(rest: &str) -> Hub {
    let (kind, raw) = rest.split_once(" : ").unwrap_or((rest, ""));
    let text = unescape(raw);
    let other = |text: String| Hub::Other { kind: kind.to_string(), text };
    match kind {
        "you" => Hub::You(text),
        "you-id" => match msg_id(raw.trim()) {
            Some(id) => Hub::YouId(id),
            None => other(text),
        },
        "steered" | "steer-rx" => match raw.split_whitespace().map(msg_id).collect::<Option<Vec<u64>>>() {
            Some(ids) if !ids.is_empty() && kind == "steered" => Hub::Steered(ids),
            Some(ids) if !ids.is_empty() => Hub::SteerRx(ids),
            _ => other(text),
        },
        "context" => Hub::Context(raw.to_string()),
        "undelivered" => {
            let (name, t) = raw.split_once(" : ").unwrap_or((raw, ""));
            Hub::Undelivered { to: field(name), text: field(t) }
        }
        "msg-in" => {
            let (head, body) = text.split_once(" : ").unwrap_or(("", text.as_str()));
            let (from, id) = head.split_once(' ').unwrap_or((head, ""));
            Hub::MsgIn { from: from.into(), id: id.into(), body: body.into() }
        }
        "msg" => {
            let (head, body) = text.split_once(" : ").unwrap_or(("", text.as_str()));
            let (from, to) = head.split_once(" → ").unwrap_or((head, ""));
            let (to, id) = match to.rsplit_once(' ') {
                Some((t, id)) if is_msg_id(id) => (t, id),
                _ => (to, ""),
            };
            Hub::Msg { from: from.into(), to: to.into(), id: id.into(), body: body.into() }
        }
        "sent" => {
            let f: Vec<String> = raw.splitn(4, " : ").map(field).collect();
            match f.as_slice() {
                [to, id, ask, body] => Hub::Sent { to: to.clone(), id: id.clone(), ask: ask == "1", body: body.clone() },
                _ => other(text),
            }
        }
        "msg-you" => {
            let (from, body) = text.split_once(" : ").unwrap_or(("", text.as_str()));
            Hub::MsgYou { from: from.into(), body: body.into() }
        }
        "answered" => {
            let mut f = raw.splitn(4, " : ").map(field);
            let mut next = || f.next().unwrap_or_default();
            Hub::Answered { agent: next(), question: next(), answer: next(), why: next() }
        }
        "card" => {
            let (head, body) = text.split_once(" : ").unwrap_or((text.as_str(), ""));
            let mut w = head.split_whitespace();
            let id = w.next().and_then(|i| i.strip_prefix('#')).and_then(|i| i.parse().ok());
            let kind = w.next().unwrap_or("question").to_string();
            let body = body.to_string();
            Hub::Card { text, id, kind, body }
        }
        "gate" => Hub::Gate(match text.split_whitespace().next() {
            Some("check") => GateStep::Check,
            Some("card") => GateStep::Card,
            _ => GateStep::Done,
        }),
        "approval" => {
            let f: Vec<String> = raw.split(" : ").map(field).collect();
            let get = |i: usize| f.get(i).cloned().unwrap_or_default();
            Hub::Approval { how: get(0), who: get(1), what: get(2), note: get(3) }
        }
        "card-closed" => match text.strip_prefix('#').and_then(|t| t.split_once(' ')) {
            Some((id, res)) if id.parse::<u64>().is_ok() => Hub::CardClosed { id: id.parse().unwrap_or(0), res: res.trim().to_string() },
            _ => other(text),
        },
        "route" => match answer_route(&text) {
            Some((who, card, said)) => Hub::Route { who: who.into(), card, said: said.into() },
            None => other(text),
        },
        "pr" => {
            let mut f = raw.splitn(4, " : ").map(field);
            let mut next = || f.next().unwrap_or_default();
            let (tone, number, url, t) = (next(), next(), next(), next());
            match number.parse::<u64>() {
                Ok(number) => Hub::Pr { state: pr_state(&tone), number, url, text: t },
                // no number: its text field alone, never lost
                Err(_) => other(t),
            }
        }
        "artifact" => {
            let mut f = raw.splitn(5, " : ").map(field);
            let mut next = || f.next().unwrap_or_default();
            let (id, agent, title, kind, v) = (next(), next(), next(), next(), next());
            if id.is_empty() {
                return other(text);
            }
            Hub::Artifact { id, agent, title, kind, v: v.trim_start_matches('v').parse().unwrap_or(1) }
        }
        "landed" => {
            let f: Vec<String> = raw.splitn(7, " : ").map(field).collect();
            let g = |i: usize| f.get(i).cloned().unwrap_or_default();
            let num = |i: usize| g(i).trim().parse::<u64>().unwrap_or(0);
            if g(3).is_empty() {
                return other(text);
            }
            Hub::Landed { agent: g(0), target: g(1), from: g(2), sha: g(3), files: num(4), add: num(5), del: num(6) }
        }
        "spawn" => Hub::Spawn(text),
        "computer" => Hub::Computer(text),
        "direct" => Hub::Direct(text),
        "warn" => Hub::Warn(text),
        "scheduled" => Hub::Scheduled(text),
        "stopped" => Hub::Stopped(text),
        _ => other(text),
    }
}

/// The hub's line for an answer to an item (core.bend `answer.q.sent`):
/// `you → @{asker} (answer to card #{id}) : {text}` → (asker, id, text).
pub fn answer_route(text: &str) -> Option<(&str, u64, &str)> {
    let (head, said) = text.split_once(" : ")?;
    let (who, id) = head.strip_prefix("you → @")?.split_once(" (answer to card #")?;
    Some((who, id.strip_suffix(')')?.parse().ok()?, said))
}

// ---- reading a bash call ----

/// `sb report <kind> "<text>"` in a bash call: its kind and text.
pub fn report_in(args: &str) -> Option<(crate::rows::ReportKind, String)> {
    use crate::rows::ReportKind;
    let at = args.find("sb report ")?;
    let rest = &args[at + "sb report ".len()..];
    let (kind, rest) = rest.trim_start().split_once(' ')?;
    let kind = match kind {
        "progress" => ReportKind::Progress,
        "done" => ReportKind::Done,
        "failed" => ReportKind::Failed,
        "blocked" => ReportKind::Blocked,
        _ => return None,
    };
    Some((kind, quoted(rest.trim_start())))
}

fn quoted(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(q @ ('"' | '\'')) => {
            let mut out = String::new();
            let mut esc = false;
            for c in chars {
                match c {
                    _ if esc => {
                        out.push(c);
                        esc = false;
                    }
                    '\\' if q == '"' => esc = true,
                    c if c == q => break,
                    c => out.push(c),
                }
            }
            out
        }
        _ => s.split(" --").next().unwrap_or("").trim().to_string(),
    }
}

/// `sb page publish <file> [--id <id>]` in a bash call: the page id (the
/// option, else the file's stem).
pub fn publish_in(args: &str) -> Option<String> {
    let at = args.find("sb page publish ")?;
    let words: Vec<&str> = args[at + "sb page publish ".len()..].split_whitespace().collect();
    let unq = |w: &str| w.trim_matches(|c| c == '"' || c == '\'').to_string();
    if let Some(i) = words.iter().position(|w| *w == "--id") {
        return words.get(i + 1).map(|w| unq(w));
    }
    let file = unq(words.first()?);
    let name = file.rsplit('/').next().unwrap_or(&file);
    Some(name.split('.').next().unwrap_or(name).to_string())
}

/// The hub's own sender in a message line (`switchboard`, or `bise`).
pub fn is_hub_sender(from: &str) -> bool {
    from == "switchboard" || from == "bise"
}

/// A sender as people see it: the hub's id `switchboard` is `bise`.
pub fn shown_name(name: &str) -> String {
    if name == "switchboard" { "bise".into() } else { name.to_string() }
}

/// A timer's wake an agent reads from bise (every.rs `wake_text`):
/// `timer #48 "<name>" (every 2m, 2/6, set by x): <words>` then the stop
/// hint; an older hub's has no name (`timer #48 (every 2m, …): …`, law
/// an_old_wake_still_reads). Its id, its name ("" in the old shape), the
/// parenthesis's parts and its words; None: not a wake.
pub fn timer_wake(text: &str) -> Option<(u64, &str, &str, &str)> {
    let rest = text.strip_prefix("timer #")?;
    let (id, rest) = rest.split_once(" ")?;
    let id = id.parse().ok()?;
    let (name, rest) = match rest.strip_prefix('"') {
        Some(r) => r.split_once("\" ")?,
        None => ("", rest),
    };
    let rest = rest.strip_prefix('(')?;
    let (how, words) = rest.split_once("): ")?;
    let words = words.rsplit_once("\n(stop it: sb every --stop ").map_or(words, |(w, _)| w);
    Some((id, name, how, words))
}

/// The note bise sends an agent when he stops its scheduled task (for
/// the agent only: its ended line says it in the thread).
pub fn is_stop_note(text: &str) -> bool {
    text.starts_with(STOP_NOTE)
}

/// The stop note's head (architect m_11299: one owner): the hub's writer
/// (switchboard core.rs `timer_stop_by_user`) starts its note with it,
/// [`is_stop_note`] reads it.
pub const STOP_NOTE: &str = "the user stopped timer #";

/// The note bise sends main when the user answers one of main's own
/// cards (core.bend `answer.q.main`): for main's model only. The same
/// step writes the route line the thread shows ("you answered", his
/// answer whole), so a client drops this twin.
pub fn is_main_answer_note(text: &str) -> bool {
    text.starts_with(MAIN_ANSWER_NOTE)
}

/// The head of that note: core.bend's `MAIN_ANSWER_HEAD` (pinned by the
/// switchboard core test `answering_mains_card_routes_it_and_notes_main`
/// against the real sb-core, so a wording change there goes red).
pub const MAIN_ANSWER_NOTE: &str = "the user answered card #";

/// A `pr` line's tone word (forge::news::Tone, the hub's writer) as what
/// it means (architect m_11122: no color on the wire): `plain` is news,
/// `dim` nothing for him (done), `red` failing checks.
pub fn pr_state(tone: &str) -> crate::thread::PrNewsState {
    use crate::thread::PrNewsState;
    match tone {
        "plain" => PrNewsState::News,
        "dim" => PrNewsState::Done,
        "red" => PrNewsState::Failing,
        _ => PrNewsState::Unknown,
    }
}

/// What one feed item says about an agent's context: a usage after a
/// call, a compaction's end, or nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum UsageMark<T> {
    Usage(T),
    Compacted,
    Other,
}

/// The usage of an agent's current context (architect m_10999, the one
/// rule): the last usage, unless a compaction ended after it. The TUI
/// calls it over its feed events, the hub over a transcript tail.
pub fn current_usage<T>(marks: impl DoubleEndedIterator<Item = UsageMark<T>>) -> Option<T> {
    for m in marks.rev() {
        match m {
            UsageMark::Usage(u) => return Some(u),
            UsageMark::Compacted => return None,
            UsageMark::Other => {}
        }
    }
    None
}

/// A transcript line as a [`UsageMark`]: its usage text (after `obs:
/// usage: `, for bise_session's parser).
pub fn usage_mark(line: &str) -> UsageMark<String> {
    match read(line) {
        Rec::Obs(Obs::Usage(t)) => UsageMark::Usage(t),
        Rec::Obs(Obs::CompactionDone(_)) => UsageMark::Compacted,
        _ => UsageMark::Other,
    }
}

// ---- his message's mark (G1, contract C3, BISE-86): the one rule ----

use crate::thread::Delivery;

/// What a line does to the marks of his messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    /// steering (`steering_received`, `steered`) or a message the core
    /// committed (`history injected`): his newest message with these
    /// words goes up to `to`. None with these words: `or_turn` (steering,
    /// BISE-90: the hub steered his words in a block with other words)
    /// raises his messages of this turn instead; else nothing moves (an
    /// injected notification: the TUI shows its info line)
    Text { text: String, to: Delivery, or_turn: bool },
    /// a turn started: the model reads what he sent since the last one
    /// (a message at idle goes straight to read)
    Turn,
    /// BISE-86: his message `text` didn't reach `to` (his line is the
    /// text, or `@to text` from another view)
    Failed { to: String, text: String },
    /// sb-core's receipts (`steer-rx`, `steered`): his messages with
    /// these ids go up to `to` (no words matched)
    Ids { ids: Vec<u64>, to: Delivery },
}

/// The mark a line moves, if any.
pub fn mark_of(rec: &Rec) -> Option<Mark> {
    let text = |text: &str, to, or_turn| Some(Mark::Text { text: text.to_string(), to, or_turn });
    match rec {
        Rec::Obs(Obs::SteeringReceived(t)) => text(t, Delivery::Received, true),
        Rec::Obs(Obs::Steered(t)) => text(t, Delivery::Read, true),
        Rec::Injected(t) => text(t, Delivery::Read, false),
        Rec::Obs(Obs::TurnStarted) => Some(Mark::Turn),
        Rec::Hub(Hub::Undelivered { to, text }) => Some(Mark::Failed { to: to.clone(), text: text.clone() }),
        Rec::Hub(Hub::SteerRx(ids)) => Some(Mark::Ids { ids: ids.clone(), to: Delivery::Received }),
        Rec::Hub(Hub::Steered(ids)) => Some(Mark::Ids { ids: ids.clone(), to: Delivery::Read }),
        _ => None,
    }
}

/// A thread's item as [`deliver`] reads it: the TUI's feed events, the
/// fold's entries.
pub trait Delivered {
    /// his message: its text and its mark
    fn yours(&self) -> Option<(&str, Delivery)>;
    fn set_mark(&mut self, to: Delivery);
    /// his message's id (its `sb you-id` line), None when it has none
    /// (an item that is not his; a transcript before 9633cd2a)
    fn msg_id(&self) -> Option<u64> {
        None
    }
    /// a turn started here (the TUI keeps one in its feed; the fold's
    /// turns are no entries: it passes the items since its last one)
    fn turn_start(&self) -> bool {
        false
    }
}

/// How far back a steering line looks for his message.
pub const MARK_LOOKBACK: usize = 500;

/// The words of a text, for matching a steering line to his message
/// (the wire flattens its line breaks).
fn same_words(a: &str, b: &str) -> bool {
    a.split_whitespace().eq(b.split_whitespace())
}

/// Mark `m` on `items` (oldest first; a turn mark: the items since the
/// turn's start, or a feed where [`Delivered::turn_start`] says it): the
/// indices whose mark moved, or None when no message of his matched
/// (an injected notification; an undelivered line from another view).
/// Marks only move up ([`Delivery`]'s order); `failed` stays.
pub fn deliver<T: Delivered>(items: &mut [T], m: &Mark) -> Option<Vec<usize>> {
    let mut moved = Vec::new();
    let mut raise = |items: &mut [T], i: usize, to: Delivery| {
        if items[i].yours().is_some_and(|(_, d)| to > d) {
            items[i].set_mark(to);
            moved.push(i);
        }
    };
    match m {
        Mark::Text { text, to, or_turn } => {
            let plain = unescape(text);
            let from = items.len().saturating_sub(MARK_LOOKBACK);
            if let Some(i) = (from..items.len()).rev().find(|&i| items[i].yours().is_some_and(|(t, _)| same_words(t, &plain))) {
                raise(items, i, *to);
            } else if *or_turn {
                this_turn(items, |items, i| raise(items, i, *to));
            } else {
                return None;
            }
        }
        Mark::Turn => this_turn(items, |items, i| raise(items, i, Delivery::Read)),
        Mark::Failed { to, text } => {
            let mine = |t: &str| t == text || t.strip_prefix('@').and_then(|r| r.split_once(' ')).is_some_and(|(n, r)| n == to && r.trim() == text);
            let i = (0..items.len()).rev().find(|&i| items[i].yours().is_some_and(|(t, d)| d != Delivery::Failed && mine(t)))?;
            items[i].set_mark(Delivery::Failed);
            moved.push(i);
        }
        Mark::Ids { ids, to } => {
            for i in items.len().saturating_sub(MARK_LOOKBACK)..items.len() {
                if items[i].msg_id().is_some_and(|id| ids.contains(&id)) {
                    raise(items, i, *to);
                }
            }
        }
    }
    Some(moved)
}

/// His messages since the turn started, newest first, still sent or
/// received.
fn this_turn<T: Delivered>(items: &mut [T], mut f: impl FnMut(&mut [T], usize)) {
    for i in (0..items.len()).rev() {
        if items[i].turn_start() {
            break;
        }
        if items[i].yours().is_some_and(|(_, d)| matches!(d, Delivery::Sent | Delivery::Received)) {
            f(items, i);
        }
    }
}

// ---- tool rows (G3): the TUI's rule, the fold's too ----

/// What a line does to a thread's tool rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolMove {
    /// `tool_started`: a running row of its own; a row exists from here
    Start(u32),
    /// `tool #<id> <name> : <args>`: its name and args (as on the wire)
    /// go on the newest row with that id; none: nothing
    Info { id: u32, name: String, args: String },
    /// `tool_intent`: on the newest row with that id
    Intent { id: u32, text: String },
    /// `tool_code`: on the newest row with that id
    Code { id: u32, code: String },
    /// `tool_result`: its output on the newest row with that id
    Result { id: u32, ok: bool, preview: String },
    /// `tool_finished`: the running row with that id ends, ok or not;
    /// none running: an ended row of its own
    Finish { id: u32, ok: bool },
}

/// The tool move of a line, if any.
///
/// Outside the wire rule on purpose (lib.rs; architect m_14688): since
/// P4d0 a call line with no `tool_started` before it makes no tool row, in
/// the hub's fold as in the TUI. That drops a phantom row ('0.0s', never
/// run) for every reader alike: a fix, not a field changing meaning, so
/// tests/released.rs's fold law covers message lines only.
pub fn tool_move(rec: &Rec) -> Option<ToolMove> {
    Some(match rec.clone() {
        Rec::Obs(Obs::ToolStarted(id)) => ToolMove::Start(id),
        Rec::Obs(Obs::ToolFinished { id, ok }) => ToolMove::Finish { id, ok },
        Rec::Tool { id, name, args } => ToolMove::Info { id, name, args },
        Rec::ToolIntent { id, text } => ToolMove::Intent { id, text },
        Rec::ToolCode { id, code } => ToolMove::Code { id, code },
        Rec::ToolResult { id, ok, preview } => ToolMove::Result { id, ok, preview },
        _ => return None,
    })
}

#[cfg(test)]
#[path = "lines_tests.rs"]
mod tests;
