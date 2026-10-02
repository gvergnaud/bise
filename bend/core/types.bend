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
  ToolOk{call: U32, out: String}
  ToolFail{call: U32, error: String}
  Interrupt{}
  FailTurn{error: String}

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
  TurnDone{outcome: String}

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
  PComp{compaction: U32, attempt: Nat, action: U32, extra: String, auto: Bool}

# ---- active turn state ----
type Turn is Data:
  Turn{nulls: Nat, steer: List<&2, String>, notifs: List<&2, String>}

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

def new_session(cfg: Cfg) -> Session:
  Sess{cfg, Nil{}, 0n, 0, None{}, None{}, Nil{}, Nil{}}
