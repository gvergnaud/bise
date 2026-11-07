# core/types.bend — Harness Core datatypes.
# Mirrors spec/step-protocol.ts: commands in, actions + observations out,
# one session value owning all model-visible context. Pure data, no behavior.
import Base

# ---- roles and messages (text-only content; multimodal is out of scope) ----
type Role is Data:
  RUser{}
  RAsst{}
  RSys{}
  RTool{}

# one model tool call, correlated by id
type Call is Data:
  Call{id: U32, name: String, args: String}

# one model-visible message. `injected` marks Core-injected context
# (preamble, steering, notifications, summary); kept user messages
# are never injected, so "most recent non-injected user message" is decidable.
type Msg is Data:
  Msg{role: Role, text: String, calls: List<&2, Call>, injected: Bool}

# ---- step protocol: commands (Runtime -> Core) ----
type Mode is Data:
  Queue{}
  Steer{}

type Cmd is Data:
  UserMsg{mode: Mode, text: String}
  Notify{text: String}
  Compact{extra: String}
  CompletionOk{action: U32, text: String, calls: List<&2, Call>}
  CompletionFail{action: U32, error: String}
  # compact: the tool asked to compact the conversation (self.compact,
  # filed from a program): the turn compacts before its next model call
  # and goes on from the summary
  ToolOk{call: U32, out: String, compact: Bool}
  ToolFail{call: U32, error: String, compact: Bool}
  Interrupt{}
  FailTurn{error: String}
  # the restored REPL's turn was cut by a restart during a model call:
  # the turn goes on from its checkpointed history, with this note
  # (session.bend apply.resume: at most once per turn)
  Resume{note: String}

# ---- step protocol: actions (Core -> Runtime) ----
# AgentCall and CompactionCall carry the projected model input: the Core
# emits the projected messages while retaining the canonical history.
type Act is Data:
  AgentCall{action: U32, input: List<&2, Msg>}
  ToolAct{action: U32, call: Call}
  CompactionCall{action: U32, input: List<&2, Msg>}

# ---- step protocol: observations (committed Core facts) ----
type Obs is Data:
  TurnStarted{}
  SteerReceived{text: String}
  Steered{text: String}
  NotifReceived{text: String}
  NotifDelivered{text: String}
  CandidateDiscarded{cause: String}
  Assistant{text: String}
  ToolStarted{call: U32}
  ToolFinished{call: U32, ok: Bool}
  ToolCommitted{call: U32}
  CompactionStarted{compaction: U32, auto: Bool}
  CompactionFailed{code: String}
  CompactionDone{summary: String}
  NullIteration{}
  # how the turn ended: end completed|interrupted|failed; why: the
  # failure, or who interrupted ("" none) (issue 38: typed at its builders)
  TurnDone{end: String, why: String}

# ---- configuration (resolved before session creation) ----
type ToolDef is Data:
  ToolDef{name: String, desc: String}

type Cfg is Data:
  Cfg{threshold: Nat, select_budget: Nat, max_nulls: Nat,
      system: String, tools: List<&2, ToolDef>}

# ---- pending work the Runtime owes results for ----
type Pending is Data:
  PAgent{action: U32}
  PTools{waiting: List<&2, Call>}
  # ADR 0011: stable compaction id, one-based attempt, per-request action id.
  # in_turn: the compaction runs inside a turn (past the threshold, or
  # the agent's own request): when it is done the turn goes on
  PComp{compaction: U32, attempt: Nat, action: U32, extra: String, in_turn: Bool}

# ---- active turn state ----
# compact: a tool of this turn asked for a compaction; the next dispatch
# runs it (and clears it) before the next model call
type Turn is Data:
  Turn{nulls: Nat, steer: List<&2, String>, notifs: List<&2, String>,
    compact: Bool}

# ---- apply result: rejection never mutates (no `next` to corrupt) ----
# (defined after Session: a type may only name already-defined types)

# ---- one agent session: the Harness Core's whole private state ----
type Session is Data:
  Sess{cfg: Cfg, history: List<&2, Msg>, inputs: Nat, actions: U32,
       turn: Maybe<&2, Turn>, pending: Maybe<&2, Pending>,
       queued: List<&2, String>, notifs: List<&2, String>}

type Res is Data:
  Rejected{reason: String}
  Accepted{obs: List<&2, Obs>, acts: List<&2, Act>, next: Session}

# ---- constructors ----
def user(text: String) -> Msg:
  Msg{RUser{}, text, Nil{}, False{}}

def asst(text: String, calls: List<&2, Call>) -> Msg:
  Msg{RAsst{}, text, calls, False{}}

def tool_res(text: String) -> Msg:
  Msg{RTool{}, text, Nil{}, False{}}

def sys(text: String) -> Msg:
  Msg{RSys{}, text, Nil{}, False{}}

def injected_user(text: String) -> Msg:
  Msg{RUser{}, text, Nil{}, True{}}

# a turn a failed call ends: an interrupt's error (the provider's
# "interrupted by <who>", provider-pure interrupted_by) ends it as
# interrupted by that one, any other as failed with its error
def turn_failed.go(is_int: Bool, +error: String) -> Obs:
  match is_int:
    case True{}:
      TurnDone{"interrupted", String.drop(error, 15n)}
    case False{}:
      TurnDone{"failed", error}

def turn_failed(+error: String) -> Obs:
  turn_failed.go(String.starts_with(error, "interrupted by "), error)

# the words a turn's end had before issue 38 ("completed", "interrupted",
# "failed: <why>", "failed: interrupted by <who>"): the session log's
# turn_ended keeps them (bise_session reads them on resume)
def turn_words.failed(is_failed: Bool, +end: String, +why: String) -> String:
  match is_failed:
    case True{}:
      String.append("failed: ", why)
    case False{}:
      end

def turn_words.by(nobody: Bool, +why: String) -> String:
  match nobody:
    case True{}:
      "interrupted"
    case False{}:
      String.append("failed: interrupted by ", why)

def turn_words.int(is_int: Bool, +end: String, +why: String) -> String:
  match is_int:
    case True{}:
      turn_words.by(String.eq(why, ""), why)
    case False{}:
      turn_words.failed(String.eq(end, "failed"), end, why)

def turn_words.done(is_done: Bool, +end: String, +why: String) -> String:
  match is_done:
    case True{}:
      "completed"
    case False{}:
      turn_words.int(String.eq(end, "interrupted"), end, why)

def turn_words(+end: String, +why: String) -> String:
  turn_words.done(String.eq(end, "completed"), end, why)

def new_session(cfg: Cfg) -> Session:
  Sess{cfg, Nil{}, 0n, 0, None{}, None{}, Nil{}, Nil{}}
