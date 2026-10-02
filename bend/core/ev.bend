# core/ev.bend — the session-log facts on the wire (BISE-195), pure.
#
# The REPL prints one "  ev: <json>" line per fact of the session log
# (docs/spec/session-format.ts, docs/research/session-format.md §13.1): an
# Event of the union WITHOUT seq, at and turn (the Rust writer stamps
# them). Seq-valued fields the REPL cannot know follow the writer
# contract (BISE-195/192, agreed with session-log):
# - from_queue 0: the oldest queued input of the same kind with equal
#   content;
# - compaction_done.replaces / kept: 1-based POSITIONS in the current
#   context list (the Core history is one message per context event);
# - req: the Core's action id of the model request;
# - assistant_message.model "": the writer's current model.
# Texts are real text: the Core's backslash-n escapes are undone
# (W.unescape_nl); the projection re-escapes them.
#
# The facts come from one accepted apply: the session before, the
# command, the observations and the session after (evs below). The
# context events are the messages the apply appended to the history
# (a compaction replaces it: compaction_done, then what is new).
import Base
import ./types.bend as T
import ./wire.bend as W
import ./text.bend as Tx
import ../vendor/json.bend as J

# ---- one line ----

def must_kv(must: Bool) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match must:
    case True{}:
      Con{J.kv("must", J.true_val()), Nil{}}
    case False{}:
      Nil{}

def line(+typ: String, must: Bool, +data: J.Json) -> String:
  String.append("  ev: ", J.stringify(J.obj(
    Con{J.kv("type", J.str(typ)), Con{J.kv("v", J.num(1n)),
      List.append(&2, Sigma<&2, &2, String, _ => J.Json>, must_kv(must),
        Con{J.kv("data", data), Nil{}})}})))

def u32(+n: U32) -> J.Json:
  J.num(U32.to_nat(n))

def call_id(+n: U32) -> String:
  String.append("call_", U32.show(n))

# an optional field: present when `has`
def opt(has: Bool, +k: String, +v: J.Json)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match has:
    case True{}:
      Con{J.kv(k, v), Nil{}}
    case False{}:
      Nil{}

# ---- content parts ----

def text_part(+t: String) -> J.Json:
  J.obj(Con{J.kv("kind", J.str("text")), Con{J.kv("text", J.str(t)), Nil{}}})

# one text part of a Core text (escaped newlines undone)
def content(+t: String) -> J.Json:
  J.arr(Con{text_part(W.unescape_nl(t)), Nil{}})

# ---- assistant parts: a leading think span, then the visible text ----
# The join of the parts is the text byte for byte: thinking = "<think>"
# + text + ("\n" + "BENDSIG::" + signature) + "</think>". A span whose
# last line is the only line stays unsigned (its text is the span).

def think_part.pick(signed: Bool, +init: List<&2, String>, +last: String,
    +span: String) -> J.Json:
  match signed:
    case True{}:
      J.obj(Con{J.kv("kind", J.str("thinking")),
        Con{J.kv("text", J.str(String.join(init, W.nl_str()))),
          Con{J.kv("signature", J.str(String.drop(last, 9n))), Nil{}}}})
    case False{}:
      J.obj(Con{J.kv("kind", J.str("thinking")),
        Con{J.kv("text", J.str(span)), Nil{}}})

def think_part.last(m: Maybe<&2, String>, +lines: List<&2, String>,
    +span: String) -> J.Json:
  match m:
    case None{}:
      think_part.pick(False{}, Nil{}, "", span)
    case Some{+last}:
      think_part.pick(
        String.starts_with(last, "BENDSIG::")
          && Bool.not(List.is_empty(&2, String, W.lines_init(lines))),
        W.lines_init(lines), last, span)

def think_part(+span: String) -> J.Json:
  +lines = String.split(span, Tx.newline())
  think_part.last(W.lines_last(lines), lines, span)

def rest_parts(empty: Bool, +rest: String) -> List<&2, J.Json>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      Con{text_part(rest), Nil{}}

def parts.span(m: Maybe<&2, Nat>, +body: String, +u: String) -> J.Json:
  match m:
    case None{}:
      J.arr(Con{text_part(u), Nil{}})
    case Some{+n}:
      +rest = String.drop(body, Nat.add(n, 8n))
      J.arr(Con{think_part(String.take(body, n)),
        rest_parts(String.is_empty(rest), rest)})

def parts.start(is_think: Bool, +u: String) -> J.Json:
  match is_think:
    case True{}:
      +body = String.drop(u, 7n)
      parts.span(Tx.find(body, "</think>", 0n), body, u)
    case False{}:
      J.arr(Con{text_part(u), Nil{}})

# the parts of an assistant Core text
def parts(+t: String) -> J.Json:
  +u = W.unescape_nl(t)
  parts.start(String.starts_with(u, "<think>"), u)

def calls_json(cs: List<&2, T.Call>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{c, rest}:
      match c:
        case T.Call{+id, +name, +args}:
          Con{J.obj(Con{J.kv("id", J.str(call_id(id))),
              Con{J.kv("name", J.str(name)),
                Con{J.kv("args", J.str(args)), Nil{}}}}),
            calls_json(rest)}

def stop_of(empty: Bool) -> String:
  match empty:
    case True{}:
      "end"
    case False{}:
      "tool_use"

# ---- agent messages: "<agent_message from=.. id=..>..</agent_message>" ----

def attr.close(m: Maybe<&2, Nat>, +rest: String) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+k}:
      Some{String.take(rest, k)}

def attr.open(m: Maybe<&2, Nat>, +head: String, +plen: Nat)
    -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+n}:
      +rest = String.drop(head, Nat.add(n, plen))
      attr.close(Tx.find(rest, "\"", 0n), rest)

# the value of ` name="…"` in the opening tag (up to its first ">")
def attr(+head: String, +name: String) -> Maybe<&2, String>:
  +pat = String.concat([" ", name, "=\""])
  attr.open(Tx.find(head, pat, 0n), head, String.length(pat))

def head_of(m: Maybe<&2, Nat>, +t: String) -> String:
  match m:
    case None{}:
      t
    case Some{+n}:
      String.take(t, n)

def attr_or(m: Maybe<&2, String>, +d: String) -> String:
  match m:
    case None{}:
      d
    case Some{+v}:
      v

def attr_opt(m: Maybe<&2, String>, +k: String)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match m:
    case None{}:
      Nil{}
    case Some{+v}:
      Con{J.kv(k, J.str(v)), Nil{}}

# the agent_message data of one tagged text (u: real text); the tail
# fields (content, from_queue, injected) are the caller's
def agent_data(+u: String,
    +tail: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> J.Json:
  +h = head_of(Tx.find(u, ">", 0n), u)
  J.obj(List.concat(&2, Sigma<&2, &2, String, _ => J.Json>,
    [Con{J.kv("hub_msg", J.str(attr_or(attr(h, "id"), ""))), Nil{}},
     attr_opt(attr(h, "thread"), "thread"),
     Con{J.kv("from", J.str(attr_or(attr(h, "from"), ""))),
       Con{J.kv("relation", J.str(attr_or(attr(h, "relation"), "peer"))),
         Con{J.kv("expects_reply",
           J.bool(String.eq(attr_or(attr(h, "expects_reply"), ""), "true"))),
           Nil{}}}},
     attr_opt(attr(h, "reply_to"), "reply_to"),
     tail]))

def is_agent(+u: String) -> Bool:
  String.starts_with(u, "<agent_message ")

# ---- the apply being described ----

type Ctx is Data:
  Ctx{fromq: Bool, cmd: T.Cmd, obs: List<&2, T.Obs>,
      pending: Maybe<&2, T.Pending>, actions: U32}

def ctx_obs(+c: Ctx) -> List<&2, T.Obs>:
  match c:
    case Ctx{_, _, +obs, _, _}:
      obs

def ctx_fromq(+c: Ctx) -> Bool:
  match c:
    case Ctx{+fromq, _, _, _, _}:
      fromq

def req.pending(p: Maybe<&2, T.Pending>) -> U32:
  match p:
    case Some{T.PAgent{+a}}:
      a
    case Some{T.PComp{_, _, +a, _, _}}:
      a
    case _:
      0

# the model request an apply answers: the command's action, else the
# pending one
def req_of(+c: Ctx) -> U32:
  match c:
    case Ctx{_, +cmd, _, +pending, _}:
      match cmd:
        case T.CompletionOk{+a, _, _}:
          a
        case T.CompletionFail{+a, _}:
          a
        case _:
          req.pending(pending)

def comp_id(+c: Ctx) -> U32:
  match c:
    case Ctx{_, _, _, +pending, +actions}:
      match pending:
        case Some{T.PComp{+id, _, _, _, _}}:
          id
        case _:
          actions

def tool_ok(+c: Ctx) -> Bool:
  match c:
    case Ctx{_, +cmd, _, _, _}:
      match cmd:
        case T.ToolOk{_, _}:
          True{}
        case _:
          False{}

def waiting_ids(cs: List<&2, T.Call>) -> List<&2, U32>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{c, rest}:
      match c:
        case T.Call{+id, _, _}:
          Con{id, waiting_ids(rest)}

def pending_ids(p: Maybe<&2, T.Pending>) -> List<&2, U32>:
  match p:
    case Some{T.PTools{+w}}:
      waiting_ids(w)
    case _:
      Nil{}

# the call ids of the tool results an apply appends, in order
def tool_ids(+c: Ctx) -> List<&2, U32>:
  match c:
    case Ctx{_, +cmd, _, +pending, _}:
      match cmd:
        case T.ToolOk{+id, _}:
          Con{id, Nil{}}
        case T.ToolFail{+id, _}:
          Con{id, Nil{}}
        case T.Interrupt{}:
          pending_ids(pending)
        case _:
          Nil{}

# ---- how an injected user message entered ----

type Inj is Data:
  ISteer{}
  INotifQ{}
  INotif{}
  INone{}

def inj.pick(hit: Bool, +k: Inj, +other: Inj) -> Inj:
  match hit:
    case True{}:
      k
    case False{}:
      other

def inj_kind(os: List<&2, T.Obs>, +t: String) -> Inj:
  match os:
    case Nil{}:
      INone{}
    case Con{o, rest}:
      match o:
        case T.Steered{+x}:
          inj.pick(String.eq(x, t), ISteer{}, inj_kind(rest, t))
        case T.NotifDelivered{+x}:
          inj.pick(String.eq(x, t), INotifQ{},
            inj.pick(String.eq(String.append("[notification] ", x), t),
              INotif{}, inj_kind(rest, t)))
        case _:
          inj_kind(rest, t)

def fq_kv(fromq: Bool) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  opt(fromq, "from_queue", J.num(0n))

# an injected text that is not steering: agent message, notification,
# compaction summary or other
def injected_ev.kind(agent: Bool, +fromq: Bool, +kind: String, +t: String)
    -> String:
  match agent:
    case True{}:
      line("agent_message", True{},
        agent_data(W.unescape_nl(t), Con{J.kv("content", content(t)), fq_kv(fromq)}))
    case False{}:
      line("context_injected", True{},
        J.obj(Con{J.kv("kind", J.str(kind)),
          Con{J.kv("content", content(t)), fq_kv(fromq)}}))

def other_kind(+t: String) -> String:
  Bool.pick(String, String.starts_with(t, "Summary of the earlier conversation:"),
    "summary", "other")

def injected_ev(k: Inj, +t: String) -> String:
  match k:
    case ISteer{}:
      line("user_message", True{},
        J.obj(Con{J.kv("content", content(t)),
          Con{J.kv("delivery", J.str("steer")),
            Con{J.kv("injected", J.true_val()), Nil{}}}}))
    case INotifQ{}:
      injected_ev.kind(is_agent(W.unescape_nl(t)), True{}, "notification", t)
    case INotif{}:
      injected_ev.kind(is_agent(W.unescape_nl(t)), False{}, "notification", t)
    case INone{}:
      injected_ev.kind(is_agent(W.unescape_nl(t)), False{}, other_kind(t), t)

def plain_user_ev(agent: Bool, +fromq: Bool, +t: String) -> String:
  match agent:
    case True{}:
      line("agent_message", True{},
        agent_data(W.unescape_nl(t), Con{J.kv("content", content(t)),
          List.append(&2, Sigma<&2, &2, String, _ => J.Json>, fq_kv(fromq),
            Con{J.kv("injected", J.false_val()), Nil{}})}))
    case False{}:
      line("user_message", True{},
        J.obj(Con{J.kv("content", content(t)),
          Con{J.kv("delivery", J.str("prompt")), fq_kv(fromq)}}))

def user_ev(inj: Bool, +c: Ctx, +t: String) -> String:
  match inj:
    case True{}:
      injected_ev(inj_kind(ctx_obs(c), t), t)
    case False{}:
      plain_user_ev(is_agent(W.unescape_nl(t)), ctx_fromq(c), t)

# ---- one context message ----

def msg_ev(+c: Ctx, m: T.Msg, +id: U32) -> String:
  match m:
    case T.Msg{role, +text, +calls, +inj}:
      match role:
        case T.RUser{}:
          user_ev(inj, c, text)
        case T.RAsst{}:
          line("assistant_message", True{},
            J.obj(Con{J.kv("req", u32(req_of(c))),
              Con{J.kv("model", J.str("")),
                Con{J.kv("parts", parts(text)),
                  Con{J.kv("calls", J.arr(calls_json(calls))),
                    Con{J.kv("stop", J.str(stop_of(List.is_empty(&2, T.Call, calls)))),
                      Nil{}}}}}}))
        case T.RTool{}:
          line("tool_result", True{},
            J.obj(Con{J.kv("call", J.str(call_id(id))),
              Con{J.kv("ok", J.bool(tool_ok(c))),
                Con{J.kv("content", content(text)), Nil{}}}}))
        case T.RSys{}:
          line("context_injected", True{},
            J.obj(Con{J.kv("kind", J.str("other")),
              Con{J.kv("content", content(text)),
                Con{J.kv("role", J.str("system")),
                  Con{J.kv("injected", J.false_val()), Nil{}}}}}))

def is_tool(m: T.Msg) -> Bool:
  match m:
    case T.Msg{T.RTool{}, _, _, _}:
      True{}
    case _:
      False{}

def ids_head(ids: List<&2, U32>) -> U32:
  match ids:
    case Nil{}:
      0
    case Con{+h, _}:
      h

def ids_next(tool: Bool, +ids: List<&2, U32>) -> List<&2, U32>:
  match tool:
    case True{}:
      List.tail(&2, U32, ids)
    case False{}:
      ids

def msgs_evs(ms: List<&2, T.Msg>, +c: Ctx, +ids: List<&2, U32>)
    -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case Con{+m, rest}:
      Con{msg_ev(c, m, ids_head(ids)),
        msgs_evs(rest, c, ids_next(is_tool(m), ids))}

# ---- compaction: the replaced history ----

def role_key(r: T.Role) -> String:
  match r:
    case T.RUser{}:
      "u"
    case T.RAsst{}:
      "a"
    case T.RSys{}:
      "s"
    case T.RTool{}:
      "t"

def calls_key(cs: List<&2, T.Call>) -> String:
  match cs:
    case Nil{}:
      ""
    case Con{c, rest}:
      match c:
        case T.Call{+id, +name, +args}:
          String.concat(["\n", U32.show(id), " ", name, " ", args,
            calls_key(rest)])

def msg_key(m: T.Msg) -> String:
  match m:
    case T.Msg{+role, +text, +calls, +inj}:
      String.concat([role_key(role), Bool.pick(String, inj, "1", "0"),
        text, calls_key(calls)])

type Found is Data:
  Found{pos: Nat, rest: List<&2, T.Msg>}

def find_msg.pick(hit: Bool, +pos: Nat, +rest: List<&2, T.Msg>,
    +later: Maybe<&2, Found>) -> Maybe<&2, Found>:
  match hit:
    case True{}:
      Some{Found{pos, rest}}
    case False{}:
      later

# the first message of `bs` equal to `key`, its 1-based position and
# what follows it
def find_msg(bs: List<&2, T.Msg>, +key: String, +pos: Nat) -> Maybe<&2, Found>:
  match bs:
    case Nil{}:
      None{}
    case Con{+b, +rest}:
      find_msg.pick(String.eq(msg_key(b), key), pos, rest,
        find_msg(rest, key, Nat.add(pos, 1n)))

# kept: positions (in the old history) of the messages the replacement
# keeps as they were; fresh: the rest of the replacement, from the
# first message that is not an old one (context events after the
# compaction_done, in order)
type Kept is Data:
  Kept{kept: List<&2, Nat>, fresh: List<&2, T.Msg>}

def found_rest(f: Maybe<&2, Found>) -> List<&2, T.Msg>:
  match f:
    case None{}:
      Nil{}
    case Some{Found{_, +rest}}:
      rest

def found_next(f: Maybe<&2, Found>, +pos: Nat) -> Nat:
  match f:
    case None{}:
      pos
    case Some{Found{+p, _}}:
      Nat.add(p, 1n)

def kept.step(f: Maybe<&2, Found>, +a: T.Msg, +rest: List<&2, T.Msg>,
    +later: Kept) -> Kept:
  match f:
    case None{}:
      Kept{Nil{}, Con{a, rest}}
    case Some{Found{+p, _}}:
      match later:
        case Kept{+ks, +fresh}:
          Kept{Con{p, ks}, fresh}

def kept_walk(as: List<&2, T.Msg>, +bs: List<&2, T.Msg>, +pos: Nat) -> Kept:
  match as:
    case Nil{}:
      Kept{Nil{}, Nil{}}
    case Con{+a, +rest}:
      +f = find_msg(bs, msg_key(a), pos)
      kept.step(f, a, rest, kept_walk(rest, found_rest(f), found_next(f, pos)))

def nats(ns: List<&2, Nat>) -> List<&2, J.Json>:
  match ns:
    case Nil{}:
      Nil{}
    case Con{+n, rest}:
      Con{J.num(n), nats(rest)}

def compaction.kept(+c: Ctx, +pre: String, +old: Nat, k: Kept)
    -> List<&2, String>:
  match k:
    case Kept{+ks, +fresh}:
      Con{line("compaction_done", True{},
          J.obj(Con{J.kv("id", u32(comp_id(c))),
            Con{J.kv("summary", content(pre)),
              Con{J.kv("replaces", J.obj(Con{J.kv("from", J.num(1n)),
                  Con{J.kv("to", J.num(old)), Nil{}}})),
                Con{J.kv("kept", J.arr(nats(ks))), Nil{}}}}})),
        msgs_evs(fresh, c, Nil{})}

# the Core's replacement: [summary message, kept…] (core/compaction.bend
# replace); the summary message is the compaction_done summary (inserted
# at the start of the range), so /log shows the real summary
def compaction(+c: Ctx, +bh: List<&2, T.Msg>, nh: List<&2, T.Msg>)
    -> List<&2, String>:
  match nh:
    case Nil{}:
      Nil{}
    case Con{m, +rest}:
      match m:
        case T.Msg{_, +pre, _, _}:
          compaction.kept(c, pre, List.length(&2, T.Msg, bh),
            kept_walk(rest, bh, 1n))

# ---- the observations ----

def has_done(os: List<&2, T.Obs>) -> Bool:
  match os:
    case Nil{}:
      False{}
    case Con{o, rest}:
      match o:
        case T.CompactionDone{_}:
          True{}
        case _:
          has_done(rest)

def has_turn(os: List<&2, T.Obs>) -> Bool:
  match os:
    case Nil{}:
      False{}
    case Con{o, rest}:
      match o:
        case T.TurnStarted{}:
          True{}
        case _:
          has_turn(rest)

def delivered(os: List<&2, T.Obs>, +t: String) -> Bool:
  match os:
    case Nil{}:
      False{}
    case Con{o, rest}:
      match o:
        case T.NotifDelivered{+x}:
          String.eq(x, t) || delivered(rest, t)
        case _:
          delivered(rest, t)

def queued_ev(+kind: String, +t: String) -> String:
  line("input_queued", True{},
    J.obj(Con{J.kv("kind", J.str(kind)),
      Con{J.kv("content", content(t)), Nil{}}}))

def queued_notif(agent: Bool, +t: String) -> String:
  match agent:
    case True{}:
      line("input_queued", True{},
        J.obj(Con{J.kv("kind", J.str("agent_message")),
          Con{J.kv("content", content(t)),
            Con{J.kv("agent", agent_data(W.unescape_nl(t),
              Con{J.kv("content", content(t)), Nil{}})), Nil{}}}}))
    case False{}:
      queued_ev("notification", t)

def notif_held(held: Bool, +t: String) -> List<&2, String>:
  match held:
    case True{}:
      Con{queued_notif(is_agent(W.unescape_nl(t)), t), Nil{}}
    case False{}:
      Nil{}

def cause_of(fromq: Bool) -> String:
  Bool.pick(String, fromq, "queue", "user")

# before the context events: the turn start, the inputs that wait
def pre_evs(os: List<&2, T.Obs>, +all: List<&2, T.Obs>, +fromq: Bool)
    -> List<&2, String>:
  match os:
    case Nil{}:
      Nil{}
    case Con{o, rest}:
      match o:
        case T.TurnStarted{}:
          Con{line("turn_started", False{},
              J.obj(Con{J.kv("cause", J.str(cause_of(fromq))), Nil{}})),
            pre_evs(rest, all, fromq)}
        case T.NotifReceived{+t}:
          List.append(&2, String,
            notif_held(Bool.not(delivered(all, t)), t),
            pre_evs(rest, all, fromq))
        case _:
          pre_evs(rest, all, fromq)

def queued_user.pick(started: Bool, +t: String) -> List<&2, String>:
  match started:
    case True{}:
      Nil{}
    case False{}:
      Con{queued_ev("user", t), Nil{}}

# a user message the Core queued (busy): it waits for the next turn
def queued_user(+cmd: T.Cmd, +os: List<&2, T.Obs>) -> List<&2, String>:
  match cmd:
    case T.UserMsg{T.Queue{}, +t}:
      queued_user.pick(has_turn(os), t)
    case _:
      Nil{}

def err(+kind: String, +message: String) -> J.Json:
  J.obj(Con{J.kv("kind", J.str(kind)),
    Con{J.kv("message", J.str(message)), Nil{}}})

def discarded(+req: U32, +cause: String) -> String:
  line("response_discarded", False{},
    J.obj(Con{J.kv("req", u32(req)),
      Con{J.kv("cause", J.str(cause)), Nil{}}}))

def during(p: Maybe<&2, T.Pending>) -> String:
  match p:
    case None{}:
      "idle"
    case Some{T.PAgent{_}}:
      "request"
    case Some{T.PTools{_}}:
      "tool"
    case Some{T.PComp{_, _, _, _, _}}:
      "compaction"

def ids_json(ids: List<&2, U32>) -> List<&2, J.Json>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+i, rest}:
      Con{J.str(call_id(i)), ids_json(rest)}

def interrupted_ev(+p: Maybe<&2, T.Pending>) -> String:
  +ids = pending_ids(p)
  line("interrupted", False{},
    J.obj(Con{J.kv("by", J.str("user")),
      Con{J.kv("during", J.str(during(p))),
        opt(Bool.not(List.is_empty(&2, U32, ids)), "pending_calls",
          J.arr(ids_json(ids)))}}))

# an interrupt: the request in flight is discarded, then the interruption
def interrupted_evs(+p: Maybe<&2, T.Pending>) -> List<&2, String>:
  match p:
    case Some{T.PAgent{+a}}:
      Con{discarded(a, "interrupted"), Con{interrupted_ev(p), Nil{}}}
    case _:
      Con{interrupted_ev(p), Nil{}}

def discard_evs.other(+c: Ctx, +cause: String) -> List<&2, String>:
  match c:
    case Ctx{_, +cmd, _, _, _}:
      match cmd:
        case T.CompletionFail{+a, +e}:
          Con{line("request_failed", False{},
              J.obj(Con{J.kv("req", u32(a)),
                Con{J.kv("attempt", J.num(1n)),
                  Con{J.kv("error", err("other", e)), Nil{}}}})),
            Nil{}}
        case _:
          Con{discarded(req_of(c), "invalid"), Nil{}}

def discard_evs(is_int: Bool, +c: Ctx, +cause: String) -> List<&2, String>:
  match is_int:
    case True{}:
      match c:
        case Ctx{_, _, _, +p, _}:
          interrupted_evs(p)
    case False{}:
      discard_evs.other(c, cause)

def outcome.failed(is_failed: Bool, +o: String)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match is_failed:
    case True{}:
      Con{J.kv("outcome", J.str("failed")),
        Con{J.kv("error", err("other", String.drop(o, 8n))), Nil{}}}
    case False{}:
      Con{J.kv("outcome", J.str("other")), Nil{}}

def outcome.int(is_int: Bool, +o: String)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match is_int:
    case True{}:
      Con{J.kv("outcome", J.str("interrupted")), Nil{}}
    case False{}:
      outcome.failed(String.starts_with(o, "failed: "), o)

def outcome.done(is_done: Bool, +o: String)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match is_done:
    case True{}:
      Con{J.kv("outcome", J.str("done")), Nil{}}
    case False{}:
      outcome.int(String.eq(o, "interrupted"), o)

def outcome(+o: String) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  outcome.done(String.eq(o, "completed"), o)

def turn_ended(+o: String, +inputs: Nat, +actions: U32) -> String:
  line("turn_ended", False{},
    J.obj(List.append(&2, Sigma<&2, &2, String, _ => J.Json>, outcome(o),
      Con{J.kv("counts", J.obj(Con{J.kv("inputs", J.num(inputs)),
        Con{J.kv("actions", u32(actions)), Nil{}}})), Nil{}})))

def extra_of(+cmd: T.Cmd) -> String:
  match cmd:
    case T.Compact{+x}:
      x
    case _:
      ""

def comp_started(+c: Ctx, +id: U32, auto: Bool) -> String:
  match c:
    case Ctx{_, +cmd, _, _, _}:
      +x = extra_of(cmd)
      line("compaction_started", False{},
        J.obj(Con{J.kv("id", u32(id)),
          Con{J.kv("trigger", J.str(Bool.pick(String, auto, "auto", "user"))),
            opt(Bool.not(String.is_empty(x)), "extra", J.str(x))}}))

# one observation after the context events
def post_ev(+c: Ctx, +inputs: Nat, +actions: U32, o: T.Obs) -> List<&2, String>:
  match o:
    case T.ToolStarted{+id}:
      Con{line("tool_started", False{},
        J.obj(Con{J.kv("call", J.str(call_id(id))), Nil{}})), Nil{}}
    case T.CandidateDiscarded{+cause}:
      discard_evs(String.eq(cause, "interrupt"), c, cause)
    case T.NullIteration{}:
      Con{discarded(req_of(c), "empty"), Nil{}}
    case T.CompactionStarted{+id, auto}:
      Con{comp_started(c, id, auto), Nil{}}
    case T.CompactionFailed{+code}:
      Con{line("compaction_failed", False{},
        J.obj(Con{J.kv("id", u32(comp_id(c))),
          Con{J.kv("attempt", J.num(1n)),
            Con{J.kv("error", err("other", code)), Nil{}}}})), Nil{}}
    case T.TurnDone{+out}:
      Con{turn_ended(out, inputs, actions), Nil{}}
    case _:
      Nil{}

def post_evs(os: List<&2, T.Obs>, +c: Ctx, +inputs: Nat, +actions: U32)
    -> List<&2, String>:
  match os:
    case Nil{}:
      Nil{}
    case Con{o, rest}:
      List.append(&2, String, post_ev(c, inputs, actions, o),
        post_evs(rest, c, inputs, actions))

# ---- one accepted apply ----

def ctx_evs(compacted: Bool, +c: Ctx, +bh: List<&2, T.Msg>,
    +nh: List<&2, T.Msg>) -> List<&2, String>:
  match compacted:
    case True{}:
      compaction(c, bh, nh)
    case False{}:
      msgs_evs(List.drop(&2, T.Msg, nh, List.length(&2, T.Msg, bh)), c,
        tool_ids(c))

def evs.accepted(+fromq: Bool, +b: T.Session, +cmd: T.Cmd,
    +obs: List<&2, T.Obs>, +next: T.Session) -> List<&2, String>:
  match b:
    case T.Sess{_, +bh, _, +bact, _, +pending, _, _}:
      match next:
        case T.Sess{_, +nh, +inputs, +actions, _, _, _, _}:
          +c : Ctx = Ctx{fromq, cmd, obs, pending, bact}
          List.concat(&2, String,
            [pre_evs(obs, obs, fromq), queued_user(cmd, obs),
             ctx_evs(has_done(obs), c, bh, nh),
             post_evs(obs, c, inputs, actions)])

# the ev lines of one apply: `fromq` when the command is a queued
# input the runtime popped at the idle boundary
def evs(+fromq: Bool, +b: T.Session, +cmd: T.Cmd, res: T.Res)
    -> List<&2, String>:
  match res:
    case T.Rejected{_}:
      Nil{}
    case T.Accepted{+obs, _, +next}:
      evs.accepted(fromq, b, cmd, obs, next)

# ---- the configuration, once per process (and per client) ----

def tools_json(ts: List<&2, T.ToolDef>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case Con{t, rest}:
      match t:
        case T.ToolDef{+name, +desc}:
          Con{J.obj(Con{J.kv("name", J.str(name)),
              Con{J.kv("description", J.str(desc)), Nil{}}}),
            tools_json(rest)}

def cfg_evs(+cfg: T.Cfg, +model: String, +source: String) -> List<&2, String>:
  match cfg:
    case T.Cfg{+threshold, +select, +nulls, +system, +tools}:
      [line("context_set", True{},
         J.obj(Con{J.kv("system", J.obj(Con{J.kv("text", J.str(system)), Nil{}})),
           Con{J.kv("tools", J.arr(tools_json(tools))), Nil{}}})),
       line("limits_set", False{},
         J.obj(Con{J.kv("compact_threshold", J.num(threshold)),
           Con{J.kv("select_budget", J.num(select)),
             Con{J.kv("max_nulls", J.num(nulls)), Nil{}}}})),
       line("model_set", False{},
         J.obj(Con{J.kv("model", J.str(model)),
           Con{J.kv("source", J.str(source)), Nil{}}}))]

def sess_evs(+s: T.Session, +model: String, +source: String)
    -> List<&2, String>:
  match s:
    case T.Sess{+cfg, _, _, _, _, _, _, _}:
      cfg_evs(cfg, model, source)

# the usage of one model request (the provider's reply)
def usage(+req: U32, +model: String, +inp: Nat, +out: Nat, +cr: Nat,
    +cw: Nat) -> String:
  line("usage", False{},
    J.obj(Con{J.kv("req", u32(req)), Con{J.kv("model", J.str(model)),
      Con{J.kv("input", J.num(inp)), Con{J.kv("output", J.num(out)),
        Con{J.kv("cache_read", J.num(cr)),
          Con{J.kv("cache_write", J.num(cw)), Nil{}}}}}}}))

# a failed attempt that will be retried (the provider's retry loop)
def retry(+req: U32, +attempt: Nat, +why: String, +delay_ms: U32) -> String:
  line("request_failed", False{},
    J.obj(Con{J.kv("req", u32(req)), Con{J.kv("attempt", J.num(attempt)),
      Con{J.kv("error", err("other", why)),
        Con{J.kv("retry_in_ms", u32(delay_ms)), Nil{}}}}}))
