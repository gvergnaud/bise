//! One parser per transcript line kind (architect m_10476): a line of an
//! agent's transcript (the runtime's `tool #…`/`  obs: …` lines, the
//! hub's `sb <kind> …` lines) read into a typed [`Rec`]. The hub's
//! fold ([`super::fold`]) and the TUI (`wire.rs`, `sb.rs`) both read
//! lines through here, so a line means one thing on both sides. A hub
//! line is a [`Feed`] record (`sb <kind> :: <json>`, read with serde) or
//! an older text line, which only `thread::legacy` reads; [`Hub`] is the
//! view of a `Feed` record the fold and the TUI match on (Feed is the
//! source). Pure: std, serde and the line's own escapes, nothing else.

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
        think.push(super::think::unsigned(&after[..end]));
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

/// A `tool_view` line's ref: where its view and its full result are.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct ToolViewRef {
    pub plugin: String,
    pub server: String,
    pub uri: String,
    pub call: String,
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
    /// `tool_view #<id> : {"plugin","server","uri","call"}`: a view of
    /// that call (MCP Apps, docs/mcp-apps.md C3)
    ToolView { id: u32, view: ToolViewRef },
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
    /// `partial #<call>.<attempt> +<offset> : <text>`: the answer as it
    /// streams in (bend/runtime/live-pure.bend), the text since the last
    /// partial, escaped as on the wire; `off` counts the escaped chars
    /// this block already sent. Never journaled: the hub keeps the whole
    /// text in memory ([`Partial`]).
    Partial { call: String, off: usize, text: String },
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
    /// the call waits for the network: `waited_ms` so far in the outage,
    /// the next pause, its cause (no attempt spent)
    NetworkWait { waited_ms: u64, delay_ms: u64, why: String },
    HarnessRestarted(String),
    CandidateDiscarded(String),
    CompactionStarted,
    CompactionFailed(String),
    SessionRestored(String),
    /// a plan's swap said once per session (`model_swapped: <from> <to>`,
    /// runtime/provider-pure.bend swap_note): the id asked, the one run
    ModelSwapped { from: String, to: String },
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

impl TurnEnd {
    /// Its words as the text line said them (`completed`, `failed: <why>`,
    /// `failed: interrupted by <who>`, `interrupted`).
    pub fn text(&self) -> String {
        match self {
            TurnEnd::Completed => "completed".into(),
            TurnEnd::Failed(why) => format!("failed: {why}"),
            TurnEnd::Interrupted { by: Some(by) } => format!("failed: interrupted by {by}"),
            TurnEnd::Interrupted { by: None } => "interrupted".into(),
            TurnEnd::Other(t) => t.clone(),
        }
    }
}

/// The approvals gate of the running call (`sb gate : check|card|done <n>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateStep {
    Check,
    Card,
    Done,
}

/// The hub's own lines in a feed, as the fold and the TUI read them: a
/// view of a [`Feed`] record ([`view`]), whichever form the line has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hub {
    /// his message, and the view that brought it (`bise`: routed here)
    You(String, Option<String>),
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
    /// `you-parts : <json>`: the parts of the 'you' line before it (its
    /// you-id between them; sb-core writes all three in one step), the
    /// raw JSON of a `Vec<parts::Part>` (msg-parts)
    YouParts(String),
    /// `route-parts : <json>`: the parts of the route line right before
    /// it (his answer to a card, his message to another agent)
    RouteParts(String),
    /// bise's home hub: his message `msg` went to the project `to`
    /// (`name` its name) as out `xid`, not to main
    Routed { to: String, name: String, xid: u64, msg: u64 },
    /// at-start: his message `msg` (`said` his words), typed in this
    /// thread, went to the agent `to`: a route line with the message's
    /// id and no card (sb-core's trace), its parts right after it
    Went { to: String, said: String, msg: u64 },
    /// `msg-parts`: the parts of message `id` the feed's owner received
    /// (send-parts: `sb send --attach`), the raw JSON of a
    /// `Vec<parts::Part>`; on the entry of that id, never by position
    MsgParts { id: u64, raw: String },
    /// `sent-parts`: the parts of message `id` the feed's owner sent
    SentParts { id: u64, raw: String },
    /// BISE-86: `undelivered : {name} : {text}`
    Undelivered { to: String, text: String },
    /// what the feed's owner received: `{from} m_<n> : {text}`; `from`
    /// keeps its `@` (an old direct reply to the user)
    /// `view`: the plugin whose MCP App view wrote it (from `view`)
    MsgIn { from: String, id: String, body: String, view: Option<String> },
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
    /// an answer to an item: `you → @{asker} (answer to card #{id}) : {text}`, `msg` the message it became, `pick` the option's words (inbox picks)
    Route { who: String, card: u64, said: String, msg: Option<u64>, pick: Option<String> },
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
    /// event-wake: a watch ended, its JSON (`thread::WakeFold`)
    Wake(String),
    /// `kept : <text>`: the answer he had read as it streamed in, when
    /// its turn failed before the answer came (the hub writes it right
    /// before the failure's line, [`keeps_partial`]): drawn dim, never
    /// lost (designer m_13334)
    Kept(String),
    /// a kind this reader doesn't know, or a known kind's line that
    /// doesn't parse: its kind and unescaped text
    Other { kind: String, text: String },
}

use crate::core::Feed;

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
    if let Some(r) = line.strip_prefix("tool_view #") {
        return tool_view(r).unwrap_or(Rec::Dropped);
    }
    if let Some(r) = line.strip_prefix("subtool ") {
        return sub(r).unwrap_or(Rec::Dropped);
    }
    if let Some(r) = line.strip_prefix("partial #") {
        return partial(r).unwrap_or(Rec::Dropped);
    }
    if let Some(r) = line.strip_prefix("core rejected: ") {
        return Rec::Rejected(r.to_string());
    }
    if let Some(rest) = line.strip_prefix(super::obs::PREFIX) {
        if let Some((_, r)) = super::obs::ObsRec::read_rest(rest) {
            // the writer's bug: its JSON shown, never a guess
            return Rec::Obs(r.map(super::obs::ObsRec::view).unwrap_or_else(|_| Obs::Other(rest.to_string())));
        }
    }
    match super::legacy::obs_line(line) {
        Some(o) => Rec::Obs(o),
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

fn partial(r: &str) -> Option<Rec> {
    let (head, text) = r.split_once(" : ").or_else(|| r.strip_suffix(" :").map(|h| (h, "")))?;
    let (call, off) = head.split_once(" +")?;
    if call.is_empty() || call.contains(' ') {
        return None;
    }
    Some(Rec::Partial { call: call.to_string(), off: off.parse().ok()?, text: text.to_string() })
}

/// The live answer of one agent, as the hub keeps it: its call
/// (`<call>.<attempt>`) and the escaped text of the block in progress.
/// A delta whose offset doesn't match drops the block's partials until
/// the next block (a new call, or offset 0), never a guess.
///
/// It also keeps the unescaped text it sent, so each line goes out as a
/// delta (`off` in UTF-16 code units of the unescaped text, architect
/// m_17813 and m_17866), never
/// the whole text again: a trailing `\` is held back until the next
/// delta says whether it starts a `\n`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Partial {
    pub call: String,
    esc: String,
    len: usize,
    broken: bool,
    /// the unescaped text sent so far, and its length in UTF-16 code
    /// units (the unit of a delta's `off`)
    sent: String,
    sent_len: usize,
    /// the escaped text ends in a `\` not sent yet
    held: bool,
}

/// What a `partial` line did to the live answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartialStep {
    /// send it: the unescaped `text` goes at `off` (chars) of the text so
    /// far ([`LiveText::apply`]); off 0 starts the answer again
    Delta { off: usize, text: String },
    /// an offset that doesn't match (the first one of the block): its
    /// log line; later ones of the block are dropped silently
    Mismatch { call: String, off: usize, have: usize },
    /// nothing to send (a block already broken, or a delta with nothing
    /// new to show yet)
    Drop,
}

impl Partial {
    /// The whole text so far, unescaped ("" while the block is dropped):
    /// what a new subscriber gets once, at offset 0.
    pub fn text(&self) -> String {
        if self.broken {
            String::new()
        } else if self.held {
            format!("{}\\", self.sent)
        } else {
            self.sent.clone()
        }
    }

    /// One `partial` line into the live answer.
    pub fn step(&mut self, call: &str, off: usize, text: &str) -> PartialStep {
        if off == 0 {
            *self = Partial { call: call.to_string(), ..Partial::default() };
        } else if self.broken && self.call == call {
            return PartialStep::Drop;
        } else if self.call != call || off != self.len {
            let have = if self.call == call { self.len } else { 0 };
            *self = Partial { call: call.to_string(), broken: true, ..Partial::default() };
            return PartialStep::Mismatch { call: call.to_string(), off, have };
        }
        self.esc.push_str(text);
        self.len += text.chars().count();
        // the new escaped chars, with a held `\` first; a `\` at the end
        // waits (it may start a `\n`), as unescape would read it whole
        let mut new = if self.held { format!("\\{text}") } else { text.to_string() };
        self.held = new.ends_with('\\');
        if self.held {
            new.pop();
        }
        let delta = unescape(&new);
        let at = self.sent_len;
        if delta.is_empty() && off != 0 {
            return PartialStep::Drop;
        }
        self.sent.push_str(&delta);
        self.sent_len += utf16_len(&delta);
        PartialStep::Delta { off: at, text: delta }
    }

    /// The `kept` record's line that keeps the text so far in the thread
    /// (a failed turn, [`keeps_partial`]); None when there is nothing to
    /// keep (no text, or a block dropped on a bad offset).
    pub fn kept_line(&self) -> Option<String> {
        let text = unescape(&self.esc);
        (!self.broken && !self.esc.trim().is_empty()).then(|| Feed::Kept(crate::core::feed::Kept { text }).line())
    }
}

/// A client's live answer, folded from the `partial` events
/// ([`PartialStep::Delta`]: `text` goes at `off` UTF-16 code units): the
/// text keeps `text[..off]` and appends, so an event that comes twice
/// changes nothing. Offset 0 starts again (a new subscriber gets the
/// whole text that way); "" at 0 is the end. An offset past the text,
/// inside a surrogate pair, or another call's delta (a gap) empties it
/// until the next offset 0: never a guess.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveText {
    pub call: String,
    pub text: String,
    /// the text's length in UTF-16 code units (the unit of `off`)
    units: usize,
}

impl LiveText {
    /// One event; false when it was a gap (the text is empty now).
    pub fn apply(&mut self, call: &str, off: usize, text: &str) -> bool {
        if off == 0 {
            self.call = call.to_string();
            self.text.clear();
            self.units = 0;
        } else if call != self.call || off > self.units {
            return self.gap();
        } else if off < self.units {
            let Some(at) = utf16_byte_index(&self.text, off) else { return self.gap() };
            self.text.truncate(at);
            self.units = off;
        }
        self.text.push_str(text);
        self.units += utf16_len(text);
        true
    }

    fn gap(&mut self) -> bool {
        self.text.clear();
        self.units = 0;
        false
    }

    /// Nothing to show.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// The length of `s` in UTF-16 code units: the unit of a partial's `off`,
/// as JS indexes strings (architect m_17866; VoiceLit.said too).
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// The byte index of UTF-16 code unit `off` in `s`; None when `off` falls
/// inside a surrogate pair or past the end.
fn utf16_byte_index(s: &str, off: usize) -> Option<usize> {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units == off {
            return Some(i);
        }
        if units > off {
            return None;
        }
        units += c.len_utf16();
    }
    (units == off).then_some(s.len())
}

/// The lines that end a streamed attempt without its answer: a provider
/// retry, a discarded candidate, a failure, a stall, a harness restart.
/// The text he already read as it streamed in stays in the thread (an
/// `sb kept` line written right before that line), dim, at the place it
/// streamed; the notices go under it in arrival order (designer m_13334
/// and m_13562: text he read never vanishes and never moves). Each of
/// these lines also clears ([`clears_partial`]), so a next attempt (its
/// text, its tool calls) never runs with the old text still live. A stop
/// clears with nothing kept.
pub fn keeps_partial(rec: &Rec) -> bool {
    matches!(
        rec,
        Rec::Obs(
            Obs::TurnDone(TurnEnd::Failed(_))
                | Obs::TurnStalled(_)
                | Obs::HarnessRestarted(_)
                | Obs::ProviderRetry(_)
                | Obs::NetworkWait { .. }
                | Obs::CandidateDiscarded(_)
        )
    )
}

/// The lines that end an agent's live answer: its answer came (the
/// assistant line, also written before a reply's tool calls), the turn
/// ended (done, failed, interrupted, stalled), a provider retry or a
/// discarded candidate (a new attempt starts empty), a stop, a restart.
/// One rule for the hub and the TUI; the ones that end an attempt
/// without its answer keep the text first ([`keeps_partial`]).
pub fn clears_partial(rec: &Rec) -> bool {
    matches!(
        rec,
        Rec::Obs(
            Obs::Assistant(_)
                | Obs::TurnDone(_)
                | Obs::TurnStalled(_)
                | Obs::HarnessRestarted(_)
                | Obs::ProviderRetry(_)
                | Obs::NetworkWait { .. }
                | Obs::CandidateDiscarded(_)
        ) | Rec::Hub(Hub::Stopped(_))
            | Rec::Idle
    )
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

/// `<id> : <ref json>`; a ref without its four strings, or a call id
/// that is not `[0-9]+-[0-9]+`: dropped.
fn tool_view(r: &str) -> Option<Rec> {
    let (id, rest) = r.split_once(" : ")?;
    let view: ToolViewRef = serde_json::from_str(rest.trim()).ok()?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    let ok = view.call.split_once('-').is_some_and(|(a, b)| digits(a) && digits(b)) && view.uri.starts_with("ui://") && !view.plugin.is_empty();
    ok.then_some(Rec::ToolView { id: id.trim().parse().ok()?, view })
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

/// A hub line, its `sb ` taken off: a [`Feed`] record
/// (`<kind> :: <json>`, read with serde) or an older text line
/// (`thread::legacy`), as the view the fold and the TUI read.
pub fn hub(rest: &str) -> Hub {
    match Feed::read_rest(rest) {
        Some((_, Ok(f))) => view(f),
        // the writer's bug: its JSON shown, never a guess
        Some((kind, Err(_))) => Hub::Other { kind: kind.to_string(), text: rest[kind.len() + crate::core::feed::SEP.len()..].to_string() },
        None => super::legacy::hub(rest),
    }
}

/// A record as the view [`Hub`] (`Feed` is the source; this is what the
/// fold and the TUI match on).
pub fn view(f: Feed) -> Hub {
    use crate::thread::PrNewsState;
    let m = |id: Option<u64>| id.map(|n| format!("m_{n}")).unwrap_or_default();
    let other = |kind: &str, text: String| Hub::Other { kind: kind.to_string(), text };
    match f {
        Feed::You(r) => Hub::You(r.text, r.via),
        Feed::YouId(r) => Hub::YouId(r.id),
        Feed::YouParts(r) => Hub::YouParts(r.parts.to_string()),
        Feed::RouteParts(r) => Hub::RouteParts(r.parts.to_string()),
        Feed::Routed(r) => Hub::Routed { to: r.to, name: r.name, xid: r.xid, msg: r.msg },
        Feed::MsgParts(r) => Hub::MsgParts { id: r.id, raw: r.parts.to_string() },
        Feed::SentParts(r) => Hub::SentParts { id: r.id, raw: r.parts.to_string() },
        Feed::Steered(r) => Hub::Steered(r.ids),
        Feed::SteerRx(r) => Hub::SteerRx(r.ids),
        Feed::Context(r) => Hub::Context(r.context.to_string()),
        Feed::Undelivered(r) => Hub::Undelivered { to: r.to, text: r.text },
        Feed::MsgIn(r) => Hub::MsgIn { from: r.from, id: m(r.id), body: r.body, view: r.view },
        Feed::Msg(r) => Hub::Msg { from: r.from, to: r.to, id: m(r.id), body: r.body },
        Feed::Sent(r) => Hub::Sent { to: r.to, id: m(Some(r.id)), ask: r.ask, body: r.body },
        Feed::MsgYou(r) => Hub::MsgYou { from: r.from, body: r.body },
        Feed::Answered(r) => Hub::Answered { agent: r.agent, question: r.question, answer: r.answer, why: r.why },
        Feed::Card(r) => {
            let head = format!("#{} {} @{}", r.id, r.kind, r.agent);
            let text = if r.body.is_empty() { head } else { format!("{head} : {}", r.body) };
            Hub::Card { text, id: Some(r.id), kind: r.kind, body: r.body }
        }
        Feed::Gate(r) => Hub::Gate(match r.step.as_str() {
            "check" => GateStep::Check,
            "card" => GateStep::Card,
            _ => GateStep::Done,
        }),
        Feed::Approval(r) => Hub::Approval { how: r.how, who: r.who, what: r.what, note: r.note },
        Feed::CardClosed(r) => Hub::CardClosed { id: r.id, res: r.res },
        Feed::Route(r) => match (r.card, r.msg) {
            (Some(card), _) => Hub::Route { who: r.who, card, said: r.said, msg: r.msg, pick: r.pick },
            // at-start: his words typed here went to `who`, as message `msg`
            (None, Some(msg)) => Hub::Went { to: r.who, said: r.said, msg },
            // his words to an agent (an older line, no id): its text, as before
            (None, None) => other("route", format!("you → @{} : {}", r.who, r.said)),
        },
        Feed::Pr(r) => {
            let state = serde_json::from_value(serde_json::Value::String(r.state)).unwrap_or(PrNewsState::Unknown);
            Hub::Pr { state, number: r.number, url: r.url, text: r.text }
        }
        Feed::Artifact(r) => Hub::Artifact { id: r.id, agent: r.agent, title: r.title, kind: r.kind, v: u32::try_from(r.version).unwrap_or(u32::MAX) },
        Feed::Landed(r) => Hub::Landed { agent: r.agent, target: r.target, from: r.from, sha: r.sha, files: r.files, add: r.add, del: r.del },
        Feed::Spawn(r) => Hub::Spawn(r.text),
        Feed::Computer(r) => Hub::Computer(r.text),
        Feed::Direct(r) => Hub::Direct(r.text),
        Feed::Info(r) => other("info", r.text),
        Feed::Warn(r) => Hub::Warn(r.text),
        Feed::Scheduled(r) => Hub::Scheduled(r.timer.to_string()),
        Feed::Stopped(r) => Hub::Stopped(r.text),
        // the TUI reads it as an info line (feed.rs after_pause_cut), as
        // the unknown kind it was before it had a record
        Feed::Paused(r) => other("paused", r.text),
        Feed::Wake(r) => Hub::Wake(r.watch.to_string()),
        Feed::Kept(r) => Hub::Kept(r.text),
        // a newer hub's kind: its text when it has one, else its JSON
        Feed::Unknown { tag, raw } => {
            let text = raw.get("text").and_then(|t| t.as_str()).map(str::to_string);
            let text = text.unwrap_or_else(|| {
                let mut r = raw.clone();
                if let Some(o) = r.as_object_mut() {
                    o.remove("sb");
                }
                r.to_string()
            });
            other(&tag, text)
        }
    }
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

/// H9: a model call's retry as data, from the runtime's
/// `provider_retry: 2/10 · <why> · retry in 4s` body
/// (bend/runtime/provider-pure.bend retry_line). The one reader of that
/// text: [`super::words::provider_retry`] says it from this. The source
/// is still the runtime's text obs line, the next untyped seam (runtime →
/// transcript); a typed obs record would remove this parser. None: not
/// that shape (the words then say the line as it came).
pub fn retry(t: &str) -> Option<super::Retry> {
    let [n, why, wait] = t.split(" · ").collect::<Vec<_>>()[..] else { return None };
    let (a, b) = n.split_once('/')?;
    let wait = wait.strip_prefix("retry in ")?.strip_suffix('s')?;
    Some(super::Retry { attempt: a.parse().ok()?, max: b.parse().ok()?, why: why.to_string(), wait_ms: wait.parse::<u64>().ok()? * 1000, end: None })
}

/// H9: a retry joins the row of the one before it (one row, updated in
/// place) when that one's run has not ended and no turn started between
/// them. One rule for the fold and the TUI.
pub fn is_same_retry_run(prev: Option<&super::Retry>, starts_turn: bool) -> bool {
    !starts_turn && prev.is_some_and(|r| r.end.is_none())
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
/// notifications (core.bend `answer.q.main`): for main's model only. The
/// same step writes the route line the thread shows, so a client drops this
/// twin, old head too. TODO(answer-route): read the route line's typed card.
pub fn is_main_answer_note(text: &str) -> bool {
    text.starts_with(MAIN_ANSWER_NOTE) || text.starts_with("the user answered card #")
}

/// The head of that note: core.bend's `MAIN_ANSWER_HEAD` (pinned by the
/// switchboard core test `answering_mains_card_routes_it_and_notes_main`
/// against the real sb-core, so a wording change there goes red).
pub const MAIN_ANSWER_NOTE: &str = "the user answered notification #";

// an agent's context usage (architect m_10999): lines_usage.rs
#[path = "lines_usage.rs"]
mod usage;
pub use usage::{current_usage, usage_mark, UsageMark};

// his message's mark (G1, contract C3, BISE-86): lines_mark.rs
#[path = "lines_mark.rs"]
mod mark;
pub use mark::{deliver, mark_of, Delivered, Mark, MARK_LOOKBACK};

// tool rows (G3), the TUI's rule and the fold's: lines_tool.rs
#[path = "lines_tool.rs"]
mod tool_rows;
pub use tool_rows::{tool_move, ToolMove};

#[cfg(test)]
#[path = "lines_tests.rs"]
mod tests;
