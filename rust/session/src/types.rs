//! The typed payloads of docs/spec/session-format.ts, one per (type, v).
//! Unknown fields are ignored here (the raw line keeps them); an unknown
//! enum value reads as `Other` (§6.2 rule 5).
use serde::{Deserialize, Serialize};
use serde_json::Value;

macro_rules! open_enum {
    ($name:ident { $($var:ident = $s:literal),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $(#[serde(rename = $s)] $var,)*
            #[serde(other, rename = "other")]
            Other,
        }
        impl $name {
            pub const KNOWN: &'static [&'static str] = &[$($s,)* "other"];
        }
    };
}

open_enum!(Cause { User = "user", Queue = "queue", Notification = "notification", AgentMessage = "agent_message", Resume = "resume" });
open_enum!(Outcome { Done = "done", Interrupted = "interrupted", Failed = "failed", Crashed = "crashed" });
open_enum!(Delivery { Prompt = "prompt", Steer = "steer" });
open_enum!(InjectedKind { Preamble = "preamble", Notification = "notification", HubState = "hub_state", ResumeNote = "resume_note", Skill = "skill", Reminder = "reminder", Summary = "summary" });
open_enum!(Relation { Parent = "parent", Child = "child", Peer = "peer", User = "user", Hub = "hub" });
open_enum!(Stop { End = "end", ToolUse = "tool_use", MaxTokens = "max_tokens" });
open_enum!(ErrorKind { Http = "http", Network = "network", Timeout = "timeout", Parse = "parse", Tool = "tool", Internal = "internal" });
open_enum!(DiscardCause { Empty = "empty", Interrupted = "interrupted", Invalid = "invalid" });
open_enum!(By { User = "user", Restart = "restart", Agent = "agent" });
open_enum!(During { Request = "request", Tool = "tool", Compaction = "compaction", Idle = "idle" });
open_enum!(QueuedKind { User = "user", Notification = "notification", AgentMessage = "agent_message" });
open_enum!(DropReason { User = "user", Stale = "stale" });
open_enum!(Trigger { Auto = "auto", User = "user", Agent = "agent" });
open_enum!(ModelSource { Config = "config", Env = "env", User = "user", Agent = "agent", Fallback = "fallback", Migration = "migration" });
open_enum!(CloseReason { User = "user", TaskDone = "task_done", TaskDropped = "task_dropped" });
open_enum!(TitleSource { Auto = "auto", User = "user" });

/// The enum fields a reader checks for unknown values: (type, field).
/// `error` is ErrorInfo's `kind`.
pub(crate) const ENUM_FIELDS: &[(&str, &str, &[&str])] = &[
    ("turn_started", "cause", Cause::KNOWN),
    ("turn_ended", "outcome", Outcome::KNOWN),
    ("user_message", "delivery", Delivery::KNOWN),
    ("context_injected", "kind", InjectedKind::KNOWN),
    ("agent_message", "relation", Relation::KNOWN),
    ("assistant_message", "stop", Stop::KNOWN),
    ("response_discarded", "cause", DiscardCause::KNOWN),
    ("interrupted", "by", By::KNOWN),
    ("interrupted", "during", During::KNOWN),
    ("input_queued", "kind", QueuedKind::KNOWN),
    ("input_dropped", "reason", DropReason::KNOWN),
    ("compaction_started", "trigger", Trigger::KNOWN),
    ("model_set", "source", ModelSource::KNOWN),
    ("session_closed", "reason", CloseReason::KNOWN),
    ("title_set", "source", TitleSource::KNOWN),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobRef {
    pub sha256: String,
    pub bytes: u64,
    pub mime: String,
}

/// Big text goes to a blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Text {
    Inline { text: String },
    Blob { blob: BlobRef },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Part {
    Text { text: String },
    /// a text too big for its line (§8.2): the bytes are in the blob
    TextBlob { blob: BlobRef },
    Image { image: BlobRef, name: String, path: String, b64: String },
    File { file: BlobRef, name: String },
    Thinking {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
    },
    RedactedThinking {
        data: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub kind: ErrorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelRef {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRef {
    pub hub: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigratedFrom {
    pub path: String,
    pub format: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dropped_lines: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStart {
    pub session: String,
    pub format: u64,
    pub created_by: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrated_from: Option<MigratedFrom>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrevSegment {
    pub file: String,
    pub last_seq: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentStart {
    pub session: String,
    pub format: u64,
    pub index: u64,
    pub prev: PrevSegment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessOpened {
    pub writer: String,
    pub pid: u64,
    pub resume: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Limits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_threshold: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select_budget: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_nulls: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDef>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSet {
    #[serde(flatten)]
    pub model: ModelRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    pub source: ModelSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Counts {
    pub inputs: u64,
    pub actions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEnded {
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts: Option<Counts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserMessage {
    pub content: Vec<Part>,
    pub delivery: Delivery,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_queue: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub injected: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextInjected {
    pub kind: InjectedKind,
    pub content: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_queue: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub injected: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessage {
    pub hub_msg: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    pub from: String,
    pub relation: Relation,
    pub expects_reply: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    pub content: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_queue: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub injected: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub req: u64,
    pub model: String,
    pub parts: Vec<Part>,
    pub calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub call: String,
    pub ok: bool,
    pub content: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i64>,
    /// a synthetic result closing a call cut before its result (BISE-242):
    /// the context seq it goes right after (absent: the context's end)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Usage {
    pub req: u64,
    pub model: String,
    pub input: u64,
    pub output: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Value>,
    /// the call's times in ms (bend/core/ev.bend Timing): the request
    /// body's build, the checkpoint before the call, the response's head
    /// after the send (0: none), the attempt's whole time; and the
    /// attempt (1-based) that answered. Absent before c2-turn-survives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttfb_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFailed {
    pub req: u64,
    pub attempt: u64,
    pub error: ErrorInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_in_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseDiscarded {
    pub req: u64,
    pub cause: DiscardCause,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial: Option<Vec<Part>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interrupted {
    pub by: By,
    pub during: During,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_calls: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputQueued {
    pub kind: QueuedKind,
    pub content: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputDropped {
    pub queued: u64,
    pub reason: DropReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionStarted {
    pub id: u64,
    pub trigger: Trigger,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_before: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionDone {
    pub id: u64,
    pub summary: Vec<Part>,
    pub replaces: Range,
    pub kept: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_after: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionFailed {
    pub id: u64,
    pub attempt: u64,
    pub error: ErrorInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Counters {
    #[serde(default)]
    pub req: u64,
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    pub compaction: u64,
    #[serde(default)]
    pub inputs: u64,
    #[serde(default)]
    pub actions: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UsageTotal {
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default)]
    pub cache_read: u64,
    #[serde(default)]
    pub cache_write: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub upto: u64,
    pub context: Vec<u64>,
    pub system: Text,
    pub tools: Vec<ToolDef>,
    pub model: ModelRef,
    pub limits: Limits,
    pub queue: Vec<u64>,
    pub counters: Counters,
    pub usage_total: UsageTotal,
}

/// A known (type, v): its payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    SessionStart(SessionStart),
    SegmentStart(SegmentStart),
    ProcessOpened(ProcessOpened),
    SessionClosed { reason: CloseReason },
    TitleSet { title: String, source: TitleSource },
    ContextSet(ContextSet),
    LimitsSet(Limits),
    ModelSet(ModelSet),
    TurnStarted { cause: Cause },
    TurnEnded(TurnEnded),
    UserMessage(UserMessage),
    ContextInjected(ContextInjected),
    AgentMessage(AgentMessage),
    AssistantMessage(AssistantMessage),
    ToolResult(ToolResult),
    Usage(Usage),
    RequestFailed(RequestFailed),
    ResponseDiscarded(ResponseDiscarded),
    Interrupted(Interrupted),
    ToolStarted { call: String },
    InputQueued(InputQueued),
    InputDropped(InputDropped),
    CompactionStarted(CompactionStarted),
    CompactionDone(CompactionDone),
    CompactionFailed(CompactionFailed),
    Checkpoint(Checkpoint),
}

#[derive(Deserialize)]
struct ReasonOnly {
    reason: CloseReason,
}
#[derive(Deserialize)]
struct TitleOnly {
    title: String,
    source: TitleSource,
}
#[derive(Deserialize)]
struct CauseOnly {
    cause: Cause,
}
#[derive(Deserialize)]
struct CallOnly {
    call: String,
}

impl Payload {
    /// The payload of a known (type, v); None for an unknown one; Err for
    /// a known one whose data does not fit its shape.
    pub fn parse(typ: &str, v: u64, data: &Value) -> Option<Result<Payload, String>> {
        fn p<T: for<'de> Deserialize<'de>>(d: &Value) -> Result<T, String> {
            T::deserialize(d).map_err(|e| e.to_string())
        }
        if v != 1 {
            return None;
        }
        Some(match typ {
            "session_start" => p(data).map(Payload::SessionStart),
            "segment_start" => p(data).map(Payload::SegmentStart),
            "process_opened" => p(data).map(Payload::ProcessOpened),
            "session_closed" => p::<ReasonOnly>(data).map(|r| Payload::SessionClosed { reason: r.reason }),
            "title_set" => p::<TitleOnly>(data).map(|t| Payload::TitleSet { title: t.title, source: t.source }),
            "context_set" => p(data).map(Payload::ContextSet),
            "limits_set" => p(data).map(Payload::LimitsSet),
            "model_set" => p(data).map(Payload::ModelSet),
            "turn_started" => p::<CauseOnly>(data).map(|c| Payload::TurnStarted { cause: c.cause }),
            "turn_ended" => p(data).map(Payload::TurnEnded),
            "user_message" => p(data).map(Payload::UserMessage),
            "context_injected" => p(data).map(Payload::ContextInjected),
            "agent_message" => p(data).map(Payload::AgentMessage),
            "assistant_message" => p(data).map(Payload::AssistantMessage),
            "tool_result" => p(data).map(Payload::ToolResult),
            "usage" => p(data).map(Payload::Usage),
            "request_failed" => p(data).map(Payload::RequestFailed),
            "response_discarded" => p(data).map(Payload::ResponseDiscarded),
            "interrupted" => p(data).map(Payload::Interrupted),
            "tool_started" => p::<CallOnly>(data).map(|c| Payload::ToolStarted { call: c.call }),
            "input_queued" => p(data).map(Payload::InputQueued),
            "input_dropped" => p(data).map(Payload::InputDropped),
            "compaction_started" => p(data).map(Payload::CompactionStarted),
            "compaction_done" => p(data).map(Payload::CompactionDone),
            "compaction_failed" => p(data).map(Payload::CompactionFailed),
            "checkpoint" => p(data).map(Payload::Checkpoint),
            _ => return None,
        })
    }

    /// Context events build the model's message list (§4).
    pub fn is_context(&self) -> bool {
        matches!(
            self,
            Payload::UserMessage(_)
                | Payload::ContextInjected(_)
                | Payload::AgentMessage(_)
                | Payload::AssistantMessage(_)
                | Payload::ToolResult(_)
                | Payload::CompactionDone(_)
        )
    }
}

/// Every event type of the union and whether it carries `must: true`.
pub const TYPES: &[(&str, bool)] = &[
    ("session_start", false), ("segment_start", false), ("process_opened", false),
    ("session_closed", false), ("title_set", false), ("context_set", true),
    ("limits_set", false), ("model_set", false), ("turn_started", false),
    ("turn_ended", false), ("user_message", true), ("context_injected", true),
    ("agent_message", true), ("assistant_message", true), ("tool_result", true),
    ("usage", false), ("request_failed", false), ("response_discarded", false),
    ("interrupted", false), ("tool_started", false), ("input_queued", true),
    ("input_dropped", true), ("compaction_started", false), ("compaction_done", true),
    ("compaction_failed", false), ("checkpoint", true),
];

/// Whether an event of this type must carry `must: true`.
pub fn must_of(typ: &str) -> bool {
    TYPES.iter().any(|(t, m)| *t == typ && *m)
}
