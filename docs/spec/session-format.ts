// docs/spec/session-format.ts — the bise session log, normative (BISE-190).
//
// One session = ~/.bise/sessions/<id>/events.jsonl, one JSON object per
// line, typed by the union below. Rules: docs/research/session-format.md
// §5-§8 (readers skip unknown types unless `must`, keep unknown fields,
// read unknown enum values as "other"; writers only append). A type is
// never renamed or reused; a breaking change of a payload is a new `v`.
// The fixtures in tests/fixtures/session/ are the executable examples.
//
// Differences with the research draft (§4), found while building:
// - image parts keep the marker's fields, so the projection to the
//   Core's text is byte for byte;
// - a thinking part joins its signature with "\nBENDSIG::";
// - user_message / context_injected / agent_message carry the Core's
//   `injected` flag when it is not their default, and context_injected
//   a system `role`: both exist in today's sessions;
// - turn_ended carries the Core's `inputs` / `actions` counters;
// - a `text_blob` part: a text part moved to a blob by the 256 KiB rule;
// - context_injected kind "summary": a compaction summary migrated from
//   a .txt (the replaced messages were never on disk).

// ---------- envelope: the same on every line ----------
export type Seq = number;          // 1, 2, 3… per session, no gaps, never reused
export type Iso = string;          // "2026-10-01T09:14:03.120Z" (UTC, ms)

export interface Envelope {
  seq: Seq;
  at: Iso;                  // when the fact happened (writer clock)
  turn?: number;            // the turn it belongs to, when it belongs to one
  must?: true;              // a reader that does not know (type, v) must not
                            // rebuild the context from this file (§6.2)
}

// one variant = { type, v, data }; `v` changes only on a breaking change
export type Ev<T extends string, V extends number, D> =
  Envelope & { type: T; v: V; data: D };

// ---------- shared pieces ----------
export type BlobRef = {            // a file in ~/.bise/blobs/sha256/<2>/<62>
  sha256: string;
  bytes: number;
  mime: string;
};

export type Text = { text: string } | { blob: BlobRef };   // big text goes to a blob

export type Part =
  | { kind: "text"; text: string }
  | { kind: "text_blob"; blob: BlobRef }   // a text too big for its line (§8.2)
  | { kind: "image"; image: BlobRef;                  // the bytes the model got
      name: string;               // "[Image #1]"
      path: string;               // where it came from ("shot.png")
      b64: string }               // the base64 file the Core reads (§5.3):
                                  // the projection writes it from the blob
                                  // when it is missing
  | { kind: "file"; file: BlobRef; name: string };     // future attachments

export type AssistantPart =
  | { kind: "text"; text: string }
  | { kind: "thinking"; text: string;
      signature?: string;          // provider signature, stored as given
      provider?: string }          // who signed it ("anthropic", …)
  // Projection to the Core's text (§7 step 7): parts are concatenated;
  // thinking = "<think>" + text + ("\nBENDSIG::" + signature, when there
  // is one) + "</think>"; image = the marker
  // <image name="…" path="…" mime="…" b64="…">. Parts are a lossless
  // partition of that text: splitting then joining gives the same bytes.
  | { kind: "redacted_thinking"; data: string; provider?: string };

export type ToolCall = {
  id: string;               // the id the model sees: "call_<n>", n the
                            // Core's numeric call id
  name: string;
  args: string;             // the exact bytes the model produced
};

export type ToolDef = { name: string; description: string; schema?: unknown };

export type ModelRef = { model: string; effort?: string };  // "anthropic/claude-…", "high"

export type ErrorInfo = {
  kind: "http" | "network" | "timeout" | "parse" | "tool" | "internal" | "other";
  status?: number;
  message: string;          // no headers, no keys (§8)
};

// ---------- session and file ----------
export type SessionStart = Ev<"session_start", 1, {
  session: string;          // session id (§5.1)
  format: 1;                // format major (§6.1)
  created_by: string;       // "bise 0.9.3 (51c081a)"
  cwd: string;
  agent?: {                 // absent for a solo session
    hub: string;            // hub folder name, "harness-3abb2bd8"
    name: string;           // "session-format"
    parent?: string;        // "main"
  };
  migrated_from?: { path: string; format: string; sha256: string;
                    dropped_lines?: number };  // lines today's loader skips
}>;

export type SegmentStart = Ev<"segment_start", 1, {   // first line of segment 2, 3…
  session: string;
  format: 1;
  index: number;            // 2, 3…
  prev: { file: string; last_seq: Seq; sha256: string };
}>;

export type ProcessOpened = Ev<"process_opened", 1, {  // each time a process opens the log
  writer: string;           // "bise 0.9.4 (a1b2c3d)"
  pid: number;
  resume: boolean;
}>;

export type SessionClosed = Ev<"session_closed", 1, {
  reason: "user" | "task_done" | "task_dropped" | "other";
}>;

export type TitleSet = Ev<"title_set", 1, { title: string; source: "auto" | "user" }>;

// ---------- configuration: what the model sees besides messages ----------
export type ContextSet = Ev<"context_set", 1, {        // must: true
  system?: Text;            // absent = unchanged
  tools?: ToolDef[];        // the full list, absent = unchanged
}>;

export type LimitsSet = Ev<"limits_set", 1, {
  compact_threshold?: number;   // tokens
  select_budget?: number;
  max_nulls?: number;
}>;

export type ModelSet = Ev<"model_set", 1, ModelRef & {
  provider?: string;
  context_window?: number;
  source: "config" | "env" | "user" | "agent" | "fallback" | "migration" | "other";
}>;

// ---------- turns ----------
export type TurnStarted = Ev<"turn_started", 1, {
  cause: "user" | "queue" | "notification" | "agent_message" | "resume" | "other";
}>;

export type TurnEnded = Ev<"turn_ended", 1, {
  outcome: "done" | "interrupted" | "failed" | "crashed" | "other";
  error?: ErrorInfo;
  counts?: { inputs: number; actions: number };  // the Core's counters
}>;

// ---------- what enters the model context ----------
export type UserMessage = Ev<"user_message", 1, {      // must: true
  content: Part[];
  delivery: "prompt" | "steer";   // steer = typed while a turn runs
  from_queue?: Seq;               // the input_queued it consumes
  injected?: true;                // the Core's compaction flag; absent = false
}>;

export type ContextInjected = Ev<"context_injected", 1, {   // must: true
  // a user-role message the model sees but the user did not type
  kind: "preamble" | "notification" | "hub_state" | "resume_note"
      | "skill" | "reminder" | "summary" | "other";
  content: Part[];
  from_queue?: Seq;
  role?: "system";          // a system-role message (absent: user)
  injected?: false;         // the Core's compaction flag; absent = true
}>;

export type AgentMessage = Ev<"agent_message", 1, {     // must: true
  // a message from another agent or the hub, as the model saw it
  hub_msg: string;          // "m_1689": joins the hub journal
  thread?: string;
  from: string;
  relation: "parent" | "child" | "peer" | "user" | "hub";
  expects_reply: boolean;
  reply_to?: string;
  content: Part[];          // the exact text given to the model
  from_queue?: Seq;
  injected?: boolean;       // the Core's compaction flag; absent = true
}>;

export type AssistantMessage = Ev<"assistant_message", 1, {   // must: true
  req: number;              // request number in this session (joins usage, errors)
  model: string;            // the model that answered
  parts: AssistantPart[];
  calls: ToolCall[];
  stop?: "end" | "tool_use" | "max_tokens" | "other";
  response_id?: string;     // provider id, when given
}>;

export type ToolResult = Ev<"tool_result", 1, {        // must: true
  call: string;             // ToolCall.id
  ok: boolean;
  content: Part[];          // what the model sees; big text → a blob part
  ms?: number;
  exit?: number;            // bash exit code, when there is one
  after?: number;           // a synthetic result closing a cut call (BISE-242):
                            // the context seq it goes right after (absent: the end)
}>;

// ---------- facts that do not enter the context ----------
export type Usage = Ev<"usage", 1, {
  req: number;
  model: string;
  input: number;
  output: number;
  cache_read?: number;
  cache_write?: number;
  reasoning?: number;
  request_sha256?: string;  // sha256 of the request body sent (§7 step 8)
  cost?: { usd: number; prices: string };  // prices: price table id, "2026-10-01"
}>;

export type RequestFailed = Ev<"request_failed", 1, {
  req: number;
  attempt: number;          // 1, 2…
  error: ErrorInfo;
  retry_in_ms?: number;     // absent = no retry
  // a timeout (error.kind "timeout"): the limit that ran out, the
  // response head or a gap in the body, and its ms; absent otherwise
  limit?: "head" | "gap" | "other";
  waited_ms?: number;
}>;

export type ResponseDiscarded = Ev<"response_discarded", 1, {
  req: number;
  cause: "empty" | "interrupted" | "invalid" | "other";
  partial?: AssistantPart[];      // what streamed before the cut, for the UI
}>;

export type Interrupted = Ev<"interrupted", 1, {
  by: "user" | "restart" | "agent" | "other";
  during: "request" | "tool" | "compaction" | "idle";
  pending_calls?: string[];       // tool calls with no result yet
}>;

export type ToolStarted = Ev<"tool_started", 1, { call: string }>;  // UI and crash repair

// ---------- queue: inputs that wait for the next turn ----------
export type InputQueued = Ev<"input_queued", 1, {      // must: true
  kind: "user" | "notification" | "agent_message";
  content: Part[];
  agent?: AgentMessage["data"];   // when kind = agent_message
}>;

export type InputDropped = Ev<"input_dropped", 1, {    // must: true
  queued: Seq;
  reason: "user" | "stale" | "other";
}>;

// ---------- compaction ----------
export type CompactionStarted = Ev<"compaction_started", 1, {
  id: number;
  trigger: "auto" | "user" | "agent";
  tokens_before?: number;
  extra?: string;           // user instructions for the summary
}>;

export type CompactionDone = Ev<"compaction_done", 1, {  // must: true
  id: number;
  summary: Part[];          // becomes an injected message
  replaces: { from: Seq; to: Seq };  // context events it removes
  kept: Seq[];              // context events inside the range kept as they were
  tokens_after?: number;
}>;

export type CompactionFailed = Ev<"compaction_failed", 1, {
  id: number;
  attempt: number;
  error: ErrorInfo;
}>;

// ---------- full state, for fast resume ----------
export type Checkpoint = Ev<"checkpoint", 1, {         // must: true
  upto: Seq;                // state after this seq (usually seq - 1)
  context: Seq[];           // the context events, in order (§7)
  system: Text;
  tools: ToolDef[];
  model: ModelRef & { provider?: string };
  limits: LimitsSet["data"];
  queue: Seq[];             // input_queued not yet consumed or dropped
  counters: { req: number; turn: number; compaction: number; inputs: number; actions: number };
  usage_total: { input: number; output: number; cache_read: number; cache_write: number; usd?: number };
}>;

// ---------- the union ----------
export type Event =
  | SessionStart | SegmentStart | ProcessOpened | SessionClosed | TitleSet
  | ContextSet | LimitsSet | ModelSet
  | TurnStarted | TurnEnded
  | UserMessage | ContextInjected | AgentMessage | AssistantMessage | ToolResult
  | Usage | RequestFailed | ResponseDiscarded | Interrupted | ToolStarted
  | InputQueued | InputDropped
  | CompactionStarted | CompactionDone | CompactionFailed
  | Checkpoint;

// what a reader holds for a line it cannot type (§6)
export type UnknownEvent = Envelope & { type: string; v: number; data: unknown };

// the format major of this file (§6.1)
export const FORMAT = 1;
