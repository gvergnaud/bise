# runtime/remote.bend — the provider request/reply line format.
#
# A leftover of the removed HTTP bridge: the Runtime still serializes
# the projected model input to this line format, and core/api.bend
# parses it back in the same process to build the provider JSON. The
# reply takes the same detour (api.bend renders OK/CALL/END lines,
# parse_reply reads them). bend/laws/ pins both directions.
#
# Request (newline-separated):
#   MODEL default
#   TOOL <name> : <description>          (the Core's tool catalog)
#   MSG <role> : <text, newlines escaped>
#     CALL <id> <name> : <args, wire_encode'd>
#   END
#
# Reply:
#   OK <assistant text, newlines escaped>
#   CALL <id> <name> : <args, wire_encode'd>   (0..n tool calls)
#   END
# or
#   ERROR <message>
#   END
import Base
import ../core/types.bend as T
import ../core/text.bend as Tx
import ../core/wire.bend as W
import ../core/oai-chat.bend as Oai

# ---- the parsed reply ----

# extra: the provider's extra of the reply's message (T.Msg extra), ""
# none: its EXTRA line, stamped by `stamp` with whose it is
type RemoteRes is Data:
  RROk{text: String, calls: List<&2, T.Call>, extra: String}
  RRErr{why: String}


# ---- serialization ----

def role_str(role: T.Role) -> String:
  match role:
    case T.RUser{}:
      "user"
    case T.RAsst{}:
      "assistant"
    case T.RSys{}:
      "system"
    case T.RTool{}:
      "tool"

def ser_call(+c: T.Call) -> String:
  match c:
    case T.Call{+id, +name, +args}:
      String.append("\n  CALL ", String.append(U32.show(id),
        String.append(" ", String.append(name,
          String.append(" : ", W.wire_encode(args))))))

def ser_calls(calls: List<&2, T.Call>) -> String:
  match calls:
    case Nil{}:
      ""
    case Con{c, rest}:
      String.append(ser_call(c), String.append("\n", ser_calls(rest)))

def ser_msg(m: T.Msg) -> String:
  match m:
    case T.Msg{+role, +text, +calls, _, +extra}:
      # the text rides ONE wire line: real newlines escape (the model
      # sees them back after parse_wire's unescape - flattening turned
      # a pasted multi-line block into one space-joined line); the
      # provider's extra rides an EXTRA sub-line (W.extra_line)
      String.append("MSG ", String.append(role_str(role),
        String.append(" : ", String.append(W.escape_nl(text),
          String.append(ser_calls(calls), W.extra_line(extra))))))

def ser_msgs(ms: List<&2, T.Msg>) -> String:
  match ms:
    case Nil{}:
      ""
    case Con{m, rest}:
      String.append(ser_msg(m), String.append("\n", ser_msgs(rest)))

def ser_tool(t: T.ToolDef) -> String:
  match t:
    case T.ToolDef{+name, +desc}:
      String.append("TOOL ", String.append(name,
        String.append(" : ", Tx.flatten(desc))))

def ser_tools(ts: List<&2, T.ToolDef>) -> String:
  match ts:
    case Nil{}:
      ""
    case Con{t, rest}:
      String.append(ser_tool(t), String.append("\n", ser_tools(rest)))

# the complete request for one provider call
# the complete request for one provider call
def build_request(cfg: T.Cfg, input: List<&2, T.Msg>) -> String:
  match cfg:
    case T.Cfg{_, _, _, _, +tools}:
      String.append("MODEL default\n",
        String.append(ser_tools(tools),
          String.append("\n", String.append(ser_msgs(input),
            String.append("\n", "END\n")))))

# a compaction call offers NO tools: the model cannot answer with a
# tool call, which would make the summary malformed by definition
def build_request.bare(cfg: T.Cfg, input: List<&2, T.Msg>) -> String:
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      String.append("MODEL default\n",
        String.append(ser_msgs(input),
          String.append("\n", "END\n")))

# BISE-268: the first attempt of a compaction shares the agent calls'
# prefix (the prompt cache): the same tools, system and history, the
# compaction prompt last. Without the tools, the provider's cache (tools
# come first) missed and the whole history was written again, at 1.25x
# on Anthropic. The COMPACT line (ignored by the wire parser) keeps the
# ephemeral context out (with_context). The prompt asks for no tool
# call; a reply that makes one anyway is malformed, and the retry
# (attempt 2) goes bare, where no tool can be called at all.
def build_request.compact(cfg: T.Cfg, input: List<&2, T.Msg>) -> String:
  match cfg:
    case T.Cfg{_, _, _, _, +tools}:
      String.append("MODEL default\nCOMPACT\n",
        String.append(ser_tools(tools),
          String.append("\n", String.append(ser_msgs(input),
            String.append("\n", "END\n")))))


# ---- BEND_CONTEXT_FILE: an ephemeral context message ----
# A switchboard hub rewrites a small context file (the task board for
# main, the group roster for a task) whenever its state changes. The
# runtime appends it to every AGENT request as a last user message: the
# model always sees the current state, and the canonical history never
# stores it, so compaction can never lose or stale it. Compaction
# requests (no TOOL lines) are left untouched: a summary must not
# freeze a snapshot of the board.
def is_agent_request(+request: String) -> Bool:
  Bool.and(String.contains(request, "\nTOOL "),
    Bool.not(String.starts_with(request, "MODEL default\nCOMPACT\n")))

# is `s` the "END" line that closes the request (its very last line)?
# starts_with first: at almost every position the first char differs,
# so the full comparison runs only where an "END" line begins
def with_context.last.pick(here: Bool, +s: String) -> Bool:
  match here:
    case True{}:
      String.eq(s, "END
")
    case False{}:
      False{}

def with_context.last(+s: String) -> Bool:
  with_context.last.pick(String.starts_with(s, "END
"), s)

# the request with its closing "END" line replaced by `tail`, in one
# pass (a request that does not end with it comes back unchanged). It
# used to take length + reverse (ends_with) + take + append: four walks
# and three copies of a multi-megabyte request on every model call.
def with_context.swap(s: String, +tail: String, last: Bool) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{h, +t}:
      match last:
        case True{}:
          tail
        case False{}:
          SCon{h, with_context.swap(t, tail, with_context.last(t))}

def with_context.put(inject: Bool, +ctx: String, +request: String)
    -> String:
  match inject:
    case False{}:
      request
    case True{}:
      with_context.swap(request,
        String.append("MSG context : ",
          String.append(W.escape_nl(ctx), "
END
")),
        with_context.last(request))

def with_context(+ctx: String, +request: String) -> String:
  with_context.put(
    Bool.and(Bool.not(String.is_empty(String.trim(ctx))),
      is_agent_request(request)),
    ctx, request)

# ---- reply parsing ----

def u32_or(m: Maybe<&2, U32>) -> U32:
  match m:
    case Some{x}:
      x
    case None{}:
      0

def parse_call.b(sp2: Maybe<&2, Nat>, +id_str: String, +rest: String)
    -> T.Call:
  match sp2:
    case None{}:
      T.Call{u32_or(U32.read(id_str)), rest, ""}
    case Some{+q}:
      # the separator is " : " (space, colon, space) after the name
      T.Call{u32_or(U32.read(id_str)), String.take(rest, q),
        W.wire_decode(String.drop(rest, Nat.add(Nat.add(q, 2n), 1n)))}

def parse_call.a(sp: Maybe<&2, Nat>, +line: String) -> T.Call:
  match sp:
    case None{}:
      T.Call{0, "bad", ""}
    case Some{+p}:
      +id_str = String.take(line, p)
      +rest = String.drop(line, Nat.add(p, 1n))
      parse_call.b(Tx.find(rest, " ", 0n), id_str, rest)

def parse_call(+line: String) -> T.Call:
  # "CALL <id> <name> : <args>"
  parse_call.a(Tx.find(line, " ", 0n), line)

# bottom-up parse: the tail is parsed first, then the current line is
# combined into it (the List.filter trick), so every helper is a leaf

def comb.err(is_err: Bool, +h: String, +tail: RemoteRes) -> RemoteRes:
  match is_err:
    case True{}:
      RRErr{String.drop(h, 6n)}
    case False{}:
      tail

def comb.call(is_call: Bool, +h: String, +tail: RemoteRes) -> RemoteRes:
  match is_call:
    case True{}:
      match tail:
        case RROk{+text, +calls, +extra}:
          RROk{text, Con{parse_call(String.drop(h, 5n)), calls}, extra}
        case RRErr{+why}:
          RRErr{why}
    case False{}:
      comb.err(String.starts_with(h, "ERROR "), h, tail)

# "EXTRA <wire-encoded JSON>": the message's extra (Oai.reply_ok)
def comb.extra(is_extra: Bool, +h: String, +tail: RemoteRes) -> RemoteRes:
  match is_extra:
    case True{}:
      match tail:
        case RROk{+text, +calls, _}:
          RROk{text, calls, W.wire_decode(String.drop(h, 6n))}
        case RRErr{+why}:
          RRErr{why}
    case False{}:
      comb.call(String.starts_with(h, "CALL "), h, tail)

def comb.ok(is_ok: Bool, +h: String, +tail: RemoteRes) -> RemoteRes:
  match is_ok:
    case True{}:
      match tail:
        case RROk{_, +calls, +extra}:
          RROk{String.drop(h, 3n), calls, extra}
        case RRErr{+why}:
          RRErr{why}
    case False{}:
      comb.extra(String.starts_with(h, "EXTRA "), h, tail)

def combine(+h: String, tail: RemoteRes) -> RemoteRes:
  comb.ok(String.starts_with(h, "OK "), h, tail)

def parse_go.step(is_end: Bool, +h: String, tail: RemoteRes) -> RemoteRes:
  match is_end:
    case True{}:
      # the END line: the empty success the lines above combine into
      RROk{"", Nil{}, ""}
    case False{}:
      combine(h, tail)

def parse_go(lines: List<&2, String>) -> RemoteRes:
  match lines:
    case Nil{}:
      RRErr{"truncated reply"}
    case +h <> rest:
      parse_go.step(String.eq(h, "END"), h, parse_go(rest))

def parse_reply(reply: String) -> RemoteRes:
  parse_go(String.split(reply, Tx.newline()))

# the reply becomes exactly the completion command the Core expects
def remote_cmd(res: RemoteRes, +action: U32) -> T.Cmd:
  match res:
    case RROk{+text, +calls, +extra}:
      T.CompletionOk{action, text, calls, extra}
    case RRErr{+why}:
      T.CompletionFail{action, why}

# the reply's extra stamped with whose it is: the provider and model
# the call went to (core/oai-chat.bend stamp; replayed only to them)
def stamp(res: RemoteRes, +prov: String, +model: String) -> RemoteRes:
  match res:
    case RROk{+text, +calls, +extra}:
      RROk{text, calls, Oai.stamp(extra, prov, model)}
    case RRErr{+why}:
      RRErr{why}
