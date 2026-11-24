# core/checkpoint.bend — ADR 0002: a separate persistence model with a
# conversion layer, so internal Core state can change while the checkpoint
# stays coherent. In Bend the session value is already durable; the explicit
# versioned wrapper keeps the restore boundary and version discipline.
import Base
import ./types.bend as T
import ./text.bend as Tx
import ./wire.bend as W

# The current checkpoint version. Evolve backward-compatibly; treat a
# version bump as an intentional compatibility break.
type Checkpoint is Data:
  Chk{version: U32, session: T.Session}

def export(+s: T.Session) -> Checkpoint:
  Chk{2, s}

def restore(+c: Checkpoint) -> T.Session:
  match c:
    case Chk{_, session}:
      session

def version(+c: Checkpoint) -> U32:
  match c:
    case Chk{v, _}:
      v

# ---- the text form: what a session looks like on disk (ADR 0002) ----
# One line per field; the history keeps the wire conventions (a MSG
# line, its CALL sub-lines indented). Stored message texts are already
# single-line (newlines travel escaped), so the file is line-shaped.
# The MSG line carries the injected flag, so the compaction protection
# rules survive the roundtrip. Turn and pending are NOT serialized: checkpoints are written at
# turn boundaries, where they are empty, and the reload gate enforces
# exactly that state. The queue and the held notifications ARE
# serialized (QUEUE and NOTIF lines, format v2): a message the user
# queued mid-turn survives a reload, so the turn it owes starts by
# itself in the restarted process.


def role_text(r: T.Role) -> String:
  match r:
    case T.RUser{}:
      "user"
    case T.RAsst{}:
      "assistant"
    case T.RSys{}:
      "system"
    case T.RTool{}:
      "tool"

def role_of.tool(is_t: Bool, +r: String) -> Maybe<&2, T.Role>:
  match is_t:
    case True{}:
      Some{T.RTool{}}
    case False{}:
      None{}

def role_of.sys(is_s: Bool, +r: String) -> Maybe<&2, T.Role>:
  match is_s:
    case True{}:
      Some{T.RSys{}}
    case False{}:
      role_of.tool(String.eq(r, "tool"), r)

def role_of.asst(is_a: Bool, +r: String) -> Maybe<&2, T.Role>:
  match is_a:
    case True{}:
      Some{T.RAsst{}}
    case False{}:
      role_of.sys(String.eq(r, "system"), r)

def role_of.user(is_u: Bool, +r: String) -> Maybe<&2, T.Role>:
  match is_u:
    case True{}:
      Some{T.RUser{}}
    case False{}:
      role_of.asst(String.eq(r, "assistant"), r)

def role_of(+r: String) -> Maybe<&2, T.Role>:
  role_of.user(String.eq(r, "user"), r)

def inj_text(injected: Bool) -> String:
  match injected:
    case True{}:
      "True"
    case False{}:
      "False"

def calls_text(calls: List<&2, T.Call>) -> String:
  match calls:
    case Nil{}:
      ""
    case Con{+c, rest}:
      match c:
        case T.Call{+id, +name, +args}:
          String.append("\n  CALL ", String.append(U32.show(id),
            String.append(" ", String.append(name,
              String.append(" : ", String.append(W.wire_encode(args), calls_text(rest)))))))

def msg_text(m: T.Msg) -> String:
  match m:
    case T.Msg{+role, +text, +calls, +injected, +extra}:
      # the text stays on ONE line: a multi-line message (pasted code,
      # a paragraph) is escaped exactly like the wire carries it; its
      # provider extra rides an EXTRA sub-line after its calls (W.extra_line)
      String.append("MSG ", String.append(inj_text(injected),
        String.append(" ", String.append(role_text(role),
          String.append(" : ", String.append(W.escape_nl(text),
            String.append(calls_text(calls), W.extra_line(extra))))))))

def msgs_text(ms: List<&2, T.Msg>) -> String:
  match ms:
    case Nil{}:
      ""
    case Con{+m, rest}:
      String.append(msg_text(m), String.append("\n", msgs_text(rest)))

def tools_text(ts: List<&2, T.ToolDef>) -> String:
  match ts:
    case Nil{}:
      ""
    case Con{+t, rest}:
      match t:
        case T.ToolDef{+name, +desc}:
          String.append("TOOL ", String.append(name,
            String.append(" : ", String.append(desc,
              String.append("\n", tools_text(rest))))))

def to_text.cfg(+cfg: T.Cfg) -> String:
  match cfg:
    case T.Cfg{+threshold, +select_budget, +max_nulls, +system, +tools}:
      String.append(tools_text(tools), String.append("CFG ", String.append(Nat.show(threshold), String.append(" ", String.append(Nat.show(select_budget), String.append(" ", String.append(Nat.show(max_nulls), String.append(" ", String.append(W.escape_nl(system), "\n")))))))))

# ---- the queue and the held notifications ride the checkpoint (v2) ----
# A message the user queued mid-turn used to vanish at a reload, so
# the turn it owed never started on its own. The QUEUE and NOTIF
# lines carry them wire-encoded (reversible, multi-line safe).
def queued_text.go(qs: List<&2, String>) -> String:
  match qs:
    case Nil{}:
      ""
    case Con{+t, rest}:
      String.append("QUEUE ",
        String.append(W.wire_encode(t),
          String.append("
", queued_text.go(rest))))

def queued_text(qs: List<&2, String>) -> String:
  queued_text.go(qs)

def notifs_text.go(ns: List<&2, String>) -> String:
  match ns:
    case Nil{}:
      ""
    case Con{+t, rest}:
      String.append("NOTIF ",
        String.append(W.wire_encode(t),
          String.append("
", notifs_text.go(rest))))

def notifs_text(ns: List<&2, String>) -> String:
  notifs_text.go(ns)

def to_text(+c: Checkpoint) -> String:
  match c:
    case Chk{+v, +session}:
      match session:
        case T.Sess{+cfg, +history, +inputs, +actions, _, _, +queued, +notifs}:
          String.append("BEND-SESSION ", String.append(U32.show(v),
            String.append("\n", String.append(to_text.cfg(cfg),
              String.append("COUNT ", String.append(Nat.show(inputs),
                String.append(" ", String.append(U32.show(actions),
                  String.append("\n", String.append(queued_text(queued),
                    String.append(notifs_text(notifs),
                      msgs_text(history))))))))))))

# ---- from_text: the lines become a session again ----

type FS is Data:
  FS{tools: List<&2, T.ToolDef>, cur: Maybe<&2, T.Msg>,
    msgs: List<&2, T.Msg>, cfg: Maybe<&2, T.Cfg>,
    inputs: Maybe<&2, U32>, actions: Maybe<&2, U32>,
    queued: List<&2, String>, notifs: List<&2, String>, ok: Bool}

def u32_of(m: Maybe<&2, U32>) -> U32:
  match m:
    case Some{+u}:
      u
    case None{}:
      0

# a session saved before BISE-118 holds live run_typescript calls under
# the runtime's old routing name node_program: they load as
# run_typescript, the name the model called (and the one the runtime
# dispatches now)
def call_name.pick(is_old: Bool, +name: String) -> String:
  match is_old:
    case True{}:
      "run_typescript"
    case False{}:
      name

def call_name(+name: String) -> String:
  call_name.pick(String.eq(name, "node_program"), name)

def fcall.name(sp: Maybe<&2, Nat>, +id_str: String, +tail: String) -> T.Call:
  match sp:
    case Some{+q}:
      T.Call{u32_of(U32.read(id_str)), call_name(String.take(tail, q)),
        W.wire_decode(String.drop(tail, Nat.add(q, 3n)))}
    case None{}:
      T.Call{0, tail, ""}

def fcall.id(sp: Maybe<&2, Nat>, +line: String) -> T.Call:
  match sp:
    case Some{+p}:
      fcall.name(Tx.find(String.drop(line, Nat.add(p, 1n)), " : ", 0n),
        String.take(line, p), String.drop(line, Nat.add(p, 1n)))
    case None{}:
      T.Call{0, line, ""}

def fcall(+line: String) -> T.Call:
  fcall.id(Tx.find(String.drop(line, 7n), " ", 0n), String.drop(line, 7n))

def msg_make.user(injected: Bool, +text: String) -> T.Msg:
  match injected:
    case True{}:
      T.injected_user(text)
    case False{}:
      T.user(text)

def msg_make(injected: Bool, +role: Maybe<&2, T.Role>, +text: String) -> T.Msg:
  match role:
    case Some{T.RUser{}}:
      msg_make.user(injected, text)
    case Some{T.RAsst{}}:
      T.asst(text, Nil{})
    case Some{T.RTool{}}:
      T.tool_res(text)
    case Some{T.RSys{}}:
      T.sys(text)
    case None{}:
      T.user(text)

def step_ver(st: FS, +line: String) -> FS:
  match st:
    case FS{+tools, +cur, +msgs, +cfg, +inputs, +actions, +queued, +notifs, _}:
      FS{tools, cur, msgs, cfg, inputs, actions, queued, notifs,
        Bool.or(U32.is_eq(u32_of(U32.read(String.drop(line, 13n))), 1),
          U32.is_eq(u32_of(U32.read(String.drop(line, 13n))), 2))}

# QUEUE <wire-encoded text> / NOTIF <wire-encoded text>: the queued
# messages and held notifications of a mid-turn session (v2); the
# wire arg encoding keeps every line single-line and reversible
def step_queue(st: FS, +line: String) -> FS:
  match st:
    case FS{+tools, +cur, +msgs, +cfg, +inputs, +actions, +queued, +notifs, +ok}:
      FS{tools, cur, msgs, cfg, inputs, actions,
        Con{W.wire_decode(String.drop(line, 6n)), queued}, notifs, ok}

def step_notif(st: FS, +line: String) -> FS:
  match st:
    case FS{+tools, +cur, +msgs, +cfg, +inputs, +actions, +queued, +notifs, +ok}:
      FS{tools, cur, msgs, cfg, inputs, actions, queued,
        Con{W.wire_decode(String.drop(line, 6n)), notifs}, ok}

# a CALL line goes at the END of the message's calls: they were written
# in order, and results pair with calls by position (prepending reversed
# a multi-call message at every reload: 40 of 243 such messages in the
# user's sessions had flipped, BISE-197)
def add_call(st: FS, +line: String) -> FS:
  match st:
    case FS{+tools, +cur, +msgs, +cfg, +inputs, +actions, +queued, +notifs, +ok}:
      match cur:
        case Some{+m}:
          match m:
            case T.Msg{+role, +text, +calls, +injected, +extra}:
              FS{tools, Some{T.Msg{role, text,
                List.append(&2, T.Call, calls, Con{fcall(line), Nil{}}),
                injected, extra}}, msgs, cfg, inputs, actions, queued, notifs, ok}
        case None{}:
          st

# "  EXTRA <wire-encoded JSON>": the provider's extra of the message
# above (T.Msg extra). A file written before it has none ("" extra); a
# binary older than it skips the line (step.pick: an unknown line).
def add_extra(st: FS, +line: String) -> FS:
  match st:
    case FS{+tools, +cur, +msgs, +cfg, +inputs, +actions, +queued, +notifs, +ok}:
      match cur:
        case Some{+m}:
          match m:
            case T.Msg{+role, +text, +calls, +injected, _}:
              FS{tools, Some{T.Msg{role, text, calls, injected,
                W.extra_of_line(line)}}, msgs, cfg, inputs, actions, queued,
                notifs, ok}
        case None{}:
          st

def push_cur(st: FS) -> FS:
  match st:
    case FS{+tools, +cur, +msgs, +cfg, +inputs, +actions, +queued, +notifs, +ok}:
      match cur:
        case Some{+m}:
          FS{tools, None{}, Con{m, msgs}, cfg, inputs, actions, queued, notifs, ok}
        case None{}:
          st

def step_msg.put(m: T.Msg, st: FS) -> FS:
  match st:
    case FS{+tools, _, +msgs, +cfg, +inputs, +actions, +queued, +notifs, +ok}:
      FS{tools, Some{m}, msgs, cfg, inputs, actions, queued, notifs, ok}

def step_msg.build(role: Maybe<&2, T.Role>, injected: Bool, +text: String,
    st: FS) -> FS:
  step_msg.put(msg_make(injected, role, text), st)

def step_msg.role(role: Maybe<&2, T.Role>, injected: Bool, +text: String,
    st: FS) -> FS:
  step_msg.build(role, injected, W.unescape_nl(text), push_cur(st))

def step_msg.cut(sp: Maybe<&2, Nat>, injected: Bool, +r: String, st: FS) -> FS:
  match sp:
    case Some{+q}:
      step_msg.role(role_of(String.take(r, q)), injected,
        String.drop(r, Nat.add(q, 3n)), st)
    case None{}:
      st

def step_msg.inj(injected: Bool, +r: String, st: FS) -> FS:
  step_msg.cut(Tx.find(r, " : ", 0n), injected, r, st)

def step_msg.mark(sp: Maybe<&2, Nat>, st: FS, +rest: String) -> FS:
  match sp:
    case Some{+p}:
      step_msg.inj(String.eq(String.take(rest, p), "True"),
        String.drop(rest, Nat.add(p, 1n)), st)
    case None{}:
      st

def step_msg(st: FS, +line: String) -> FS:
  step_msg.mark(Tx.find(String.drop(line, 4n), " ", 0n), st,
    String.drop(line, 4n))

def step_tool.cut(sp: Maybe<&2, Nat>, st: FS, +line: String) -> FS:
  match sp:
    case Some{+p}:
      match st:
        case FS{+tools, +cur, +msgs, +cfg, +inputs, +actions, +queued, +notifs, +ok}:
          FS{Con{T.ToolDef{String.take(String.drop(line, 5n), p),
            String.drop(String.drop(line, 5n), Nat.add(p, 3n))}, tools},
            cur, msgs, cfg, inputs, actions, queued, notifs, ok}
    case None{}:
      st

def step_tool(st: FS, +line: String) -> FS:
  step_tool.cut(Tx.find(String.drop(line, 5n), " : ", 0n), st, line)

def step_cfg.sys(threshold: Nat, select: Nat, max_nulls: Nat, +sys: String,
    st: FS) -> FS:
  match st:
    case FS{+tools, +cur, +msgs, _, +inputs, +actions, +queued, +notifs, +ok}:
      FS{tools, cur, msgs,
        Some{T.Cfg{threshold, select, max_nulls, W.unescape_nl(sys),
          List.reverse(&2, T.ToolDef, tools)}},
        inputs, actions, queued, notifs, ok}

def step_cfg.n3(sp: Maybe<&2, Nat>, +rest: String,
    +threshold: Nat, +select: Nat, st: FS) -> FS:
  match sp:
    case Some{+p}:
      step_cfg.sys(threshold, select,
        U32.to_nat(u32_of(U32.read(String.take(rest, p)))),
        String.drop(rest, Nat.add(p, 1n)), st)
    case None{}:
      st

def step_cfg.n2(sp: Maybe<&2, Nat>, +rest: String, +threshold: Nat,
    st: FS) -> FS:
  match sp:
    case Some{+p}:
      step_cfg.n3(Tx.find(String.drop(rest, Nat.add(p, 1n)), " ", 0n),
        String.drop(rest, Nat.add(p, 1n)), threshold,
        U32.to_nat(u32_of(U32.read(String.take(rest, p)))), st)
    case None{}:
      st

def step_cfg.n1(sp: Maybe<&2, Nat>, +rest: String, st: FS) -> FS:
  match sp:
    case Some{+p}:
      step_cfg.n2(Tx.find(String.drop(rest, Nat.add(p, 1n)), " ", 0n),
        String.drop(rest, Nat.add(p, 1n)),
        U32.to_nat(u32_of(U32.read(String.take(rest, p)))), st)
    case None{}:
      st

def step_cfg(st: FS, +line: String) -> FS:
  step_cfg.n1(Tx.find(String.drop(line, 4n), " ", 0n),
    String.drop(line, 4n), st)

def step_count.act(sp: Maybe<&2, Nat>, +inputs: U32, +rest: String,
    st: FS) -> FS:
  match sp:
    case Some{+p}:
      match st:
        case FS{+tools, +cur, +msgs, +cfg, _, +actions, +queued, +notifs, +ok}:
          FS{tools, cur, msgs, cfg, Some{inputs},
            Some{u32_of(U32.read(String.take(rest, p)))}, queued, notifs, ok}
    case None{}:
      # the actions number is the last thing on the line
      match st:
        case FS{+tools, +cur, +msgs, +cfg, _, _, +queued, +notifs, +ok}:
          FS{tools, cur, msgs, cfg, Some{inputs},
            Some{u32_of(U32.read(rest))}, queued, notifs, ok}

def step_count.cut(sp: Maybe<&2, Nat>, st: FS, +line: String) -> FS:
  match sp:
    case Some{+p}:
      step_count.act(Tx.find(String.drop(String.drop(line, 6n),
          Nat.add(p, 1n)), " ", 0n),
        u32_of(U32.read(String.take(String.drop(line, 6n), p))),
        String.drop(String.drop(line, 6n), Nat.add(p, 1n)), st)
    case None{}:
      st

def step_count(st: FS, +line: String) -> FS:
  step_count.cut(Tx.find(String.drop(line, 6n), " ", 0n), st, line)

def step.call_or_extra(is_call: Bool, st: FS, +line: String) -> FS:
  match is_call:
    case True{}:
      add_call(st, line)
    case False{}:
      add_extra(st, line)

def step.msg_or_call(is_msg: Bool, st: FS, +line: String) -> FS:
  match is_msg:
    case True{}:
      step_msg(st, line)
    case False{}:
      step.call_or_extra(String.starts_with(line, "  CALL "), st, line)

def step.meta3(is_tool: Bool, st: FS, +line: String) -> FS:
  match is_tool:
    case True{}:
      step_tool(st, line)
    case False{}:
      step_ver(st, line)

def step.meta2(is_count: Bool, st: FS, +line: String) -> FS:
  match is_count:
    case True{}:
      step_count(st, line)
    case False{}:
      step.meta3(String.starts_with(line, "TOOL "), st, line)

def step.meta(is_cfg: Bool, st: FS, +line: String) -> FS:
  match is_cfg:
    case True{}:
      step_cfg(st, line)
    case False{}:
      step.meta2(String.starts_with(line, "COUNT "), st, line)

def step.metaqn(is_notif: Bool, st: FS, +line: String) -> FS:
  match is_notif:
    case True{}:
      step_notif(st, line)
    case False{}:
      step.meta(String.starts_with(line, "CFG "), st, line)

def step.metaq(is_queue: Bool, st: FS, +line: String) -> FS:
  match is_queue:
    case True{}:
      step_queue(st, line)
    case False{}:
      step.metaqn(String.starts_with(line, "NOTIF "), st, line)
def step.kind(is_msg: Bool, st: FS, +line: String) -> FS:
  match is_msg:
    case True{}:
      step.msg_or_call(String.starts_with(line, "MSG "), st, line)
    case False{}:
      step.metaq(String.starts_with(line, "QUEUE "), st, line)


def step.pick(known: Bool, st: FS, +line: String) -> FS:
  match known:
    case False{}:
      st
    case True{}:
      step.kind(Bool.or(String.starts_with(line, "  CALL "),
        Bool.or(String.starts_with(line, "  EXTRA "),
          String.starts_with(line, "MSG "))), st, line)

def step(st: FS, +line: String) -> FS:
  step.pick(Bool.or(String.starts_with(line, "  CALL "),
    Bool.or(String.starts_with(line, "  EXTRA "),
    Bool.or(String.starts_with(line, "MSG "),
      Bool.or(String.starts_with(line, "TOOL "),
        Bool.or(String.starts_with(line, "CFG "),
          Bool.or(String.starts_with(line, "COUNT "),
            Bool.or(String.starts_with(line, "QUEUE "),
              Bool.or(String.starts_with(line, "NOTIF "),
                String.starts_with(line, "BEND-SESSION "))))))))), st, line)

def from_go(lines: List<&2, String>, st: FS) -> FS:
  match lines:
    case Nil{}:
      st
    case Con{+line, rest}:
      from_go(rest, step(st, line))

def push_cur2(cur: Maybe<&2, T.Msg>, +msgs: List<&2, T.Msg>)
    -> List<&2, T.Msg>:
  match cur:
    case Some{+m}:
      Con{m, msgs}
    case None{}:
      msgs

def from_finish.ok(ok: Bool, +cur: Maybe<&2, T.Msg>,
    +msgs: List<&2, T.Msg>, +cfg: Maybe<&2, T.Cfg>,
    +inputs: Maybe<&2, U32>, +actions: Maybe<&2, U32>,
    +queued: List<&2, String>, +notifs: List<&2, String>)
    -> Maybe<&2, Checkpoint>:
  match ok:
    case False{}:
      None{}
    case True{}:
      match cfg:
        case None{}:
          None{}
        case Some{+c}:
          Some{Chk{2, T.Sess{c,
            List.reverse(&2, T.Msg, push_cur2(cur, msgs)),
            U32.to_nat(u32_of(inputs)), u32_of(actions), None{}, None{},
            List.reverse(&2, String, queued),
            List.reverse(&2, String, notifs)}}}

def from_finish(st: FS) -> Maybe<&2, Checkpoint>:
  match st:
    case FS{_, +cur, +msgs, +cfg, +inputs, +actions, +queued, +notifs, +ok}:
      from_finish.ok(ok, cur, msgs, cfg, inputs, actions, queued,
        notifs)

def from_text(+s: String) -> Maybe<&2, Checkpoint>:
  from_finish(from_go(String.split(s, Tx.newline()),
    FS{Nil{}, None{}, Nil{}, None{}, None{}, None{}, Nil{}, Nil{}, False{}}))
