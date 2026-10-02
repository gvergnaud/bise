# runtime/selftools-pure.bend - the pure half of runtime/selftools.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../core/wire.bend as W
import ../core/types.bend as T
import ./tools-pure.bend as Xp

# ---- self.* harness tools: requests the agent files mid-turn ----
# self.reload and self.compact are programmatic: a run_typescript
# program calls them (free identifiers become tool requests); they are
# no top-level tools (BISE-293). Both only make sense BETWEEN turns, so the tool
# result is an ACK; the deferred scan below finds it in the history
# at the end of the turn and the REPL executes the request (compact
# first, then reload — the checkpoint then carries the compacted
# session into the restarted process).

def RELOAD_ACK() -> String:
  "reload: scheduled - the harness will restart with the latest code when this turn ends; the session and background commands are preserved"

def COMPACT_ACK() -> String:
  "compact: scheduled - the conversation will be compacted when this turn ends"

# is this name one of the self.* harness tools? (self.sleep is NOT
# one: it is a programmatic tool executed inside run_typescript
# programs — see exec_program.call.sleep)
def is_self_tool.reload(+name: String) -> Bool:
  String.eq(name, "self.reload")

def is_self_tool(+name: String) -> Bool:
  Bool.or(is_self_tool.reload(name), String.eq(name, "self.compact"))

# the ack itself; routing guarantees only these two names arrive here
def self_exec.reload(is_reload: Bool) -> Xp.ExecRes:
  match is_reload:
    case True{}:
      Xp.Ex{True{}, RELOAD_ACK()}
    case False{}:
      Xp.Ex{True{}, COMPACT_ACK()}

def self_exec(+name: String) -> Xp.ExecRes:
  self_exec.reload(is_self_tool.reload(name))

# ---- the deferred-request scan (pure) ----
# What the JUST-FINISHED turn asked for. The scan walks the history
# forward, resetting at every turn boundary: the user's own message,
# and the message after a turn's final reply. Only tool results of the
# last turn count, so an old request can never fire twice. An injected
# message (steering, an agent's report delivered mid-turn) is NOT a
# boundary: it once wiped main's self.compact, filed just before an
# agent's report was steered in, and the turn ended without compacting.

type Def is Data:
  Def{compact: Bool, reload: Bool}

def def_none() -> Def:
  Def{False{}, False{}}

def def_merge(+d: Def, +compact: Bool, +reload: Bool) -> Def:
  match d:
    case Def{+c, +r}:
      Def{Bool.or(c, compact), Bool.or(r, reload)}

# BR-004: the scan must only trust TOOL results, and only an ack that
# stands alone. The direct self.* tool prefixes the ack ("tool X ok:
# <ack>") so the ack ends the text; a program's result carries the
# acks on their own lines, and tool texts are escape_nl'd, so there
# the ack is a whole line between literal backslash-n sequences.
# Assistant prose quoting the ack is not scanned at all, and a tool
# output quoting the harness source keeps its quotes and indentation
# around the constant - neither shape matches.
def def_ack.hit.ends(hit: Bool, +text: String, +ack: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      String.contains(text,
        String.append(W.escape_nl("\n"),
          String.append(ack, W.escape_nl("\n"))))

def def_ack.hit(+text: String, +ack: String) -> Bool:
  def_ack.hit.ends(String.ends_with(text, ack), text, ack)

def def_note(+d: Def, +text: String) -> Def:
  def_merge(d, def_ack.hit(text, COMPACT_ACK()),
    def_ack.hit(text, RELOAD_ACK()))

# a user message starts a new segment: requests before it are spent
def def_step.role_is_tool(role: T.Role) -> Bool:
  match role:
    case T.RTool{}:
      True{}
    case _:
      False{}

def def_step.role_is_user(role: T.Role) -> Bool:
  match role:
    case T.RUser{}:
      True{}
    case _:
      False{}

# only tool results are scanned; assistant prose is skipped entirely
def def_step.msg.tool(is_tool: Bool, +d: Def, +text: String) -> Def:
  match is_tool:
    case True{}:
      def_note(d, text)
    case False{}:
      d

# an injected user message (steering, a notification, an agent's
# message delivered mid-turn) is part of the running turn: the requests
# before it still stand. Only the user's own message starts a new turn.
def def_step.msg.injected(injected: Bool, +d: Def) -> Def:
  match injected:
    case True{}:
      d
    case False{}:
      def_none()

def def_step.msg.user(is_user: Bool, +role: T.Role, +d: Def,
    +text: String, +injected: Bool) -> Def:
  match is_user:
    case True{}:
      def_step.msg.injected(injected, d)
    case False{}:
      def_step.msg.tool(def_step.role_is_tool(role), d, text)

def def_step.msg(+d: Def, +m: T.Msg) -> Def:
  match m:
    case T.Msg{+role, +text, _, +injected}:
      def_step.msg.user(def_step.role_is_user(role), role, d, text, injected)

# a turn's final reply (an assistant message without tool calls) ends
# it: whatever comes after belongs to a later turn, even when that turn
# starts with an injected message (an agent's report waking main)
def def_step.no_calls(calls: List<&2, T.Call>) -> Bool:
  match calls:
    case Nil{}:
      True{}
    case Con{_, _}:
      False{}

def def_step.ends.role(role: T.Role, +calls: List<&2, T.Call>) -> Bool:
  match role:
    case T.RAsst{}:
      def_step.no_calls(calls)
    case _:
      False{}

def def_step.ends(+m: T.Msg) -> Bool:
  match m:
    case T.Msg{+role, _, +calls, _}:
      def_step.ends.role(role, calls)

# after a turn's end, the next message starts from no request
def def_step.fresh(ended: Bool, +d: Def) -> Def:
  match ended:
    case True{}:
      def_none()
    case False{}:
      d

# ended: the previous message was a turn's final reply
def def_scan(msgs: List<&2, T.Msg>, +d: Def, ended: Bool) -> Def:
  match msgs:
    case Nil{}:
      d
    case Con{+m, +rest}:
      def_scan(rest, def_step.msg(def_step.fresh(ended, d), m),
        def_step.ends(m))

def deferred_of(+history: List<&2, T.Msg>) -> Def:
  def_scan(history, def_none(), False{})

# the acks a request set carries (the program's result is suffixed
# with them, so the model reads its own requests back and the
# turn-boundary scan finds them)
def def_acks.reload_only(reload: Bool) -> String:
  match reload:
    case True{}:
      RELOAD_ACK()
    case False{}:
      ""

def def_acks.with_reload(reload: Bool, +acc: String) -> String:
  match reload:
    case True{}:
      String.append(acc, String.append("\n", RELOAD_ACK()))
    case False{}:
      acc

def def_acks.pick(compact: Bool, reload: Bool) -> String:
  match compact:
    case True{}:
      def_acks.with_reload(reload, COMPACT_ACK())
    case False{}:
      def_acks.reload_only(reload)

def def_acks(d: Def) -> String:
  match d:
    case Def{+compact, +reload}:
      def_acks.pick(compact, reload)

def def_acks.suffix.has(has: Bool, +d: Def) -> String:
  match has:
    case True{}:
      String.append("\n", def_acks(d))
    case False{}:
      ""

def def_acks.suffix(d: Def) -> String:
  match d:
    case Def{False{}, False{}}:
      ""
    case _:
      def_acks.suffix.has(True{}, d)
