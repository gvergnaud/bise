# hub/model.bend — the Switchboard hub's durable state (RFC 0001 §12) and
# its events. `apply` is the only way the durable state changes; the
# Rust daemon keeps a read-only mirror of it (rust/switchboard/src/
# model.rs) fed by the journal events the hub emits.
import Base

import ../vendor/json.bend as J

type Life is Data:
  LActive{}
  LStopped{}
  LArchived{}
  LFailed{}

type Run is Data:
  RDown{}
  RStarting{}
  RIdle{}
  RBusy{}

type Decl is Data:
  DWorking{}
  DDone{}
  DBlocked{}

type MSt is Data:
  Queued{reason: String}
  Delivered{}
  Rejected{error: String}
  Cancelled{}

type Msg is Data:
  Msg{id: Nat, thread: Nat, from: String, to: String, reply_to: Maybe<&2, Nat>,
    expect: Bool, auto: Bool, text: String, created: Nat, plain: Bool,
    queued: Bool, via: Maybe<&2, String>}

# a message, its delivery state and whether it got its (first) reply
type MRec is Data:
  MRec{msg: Msg, st: MSt, settled: Bool}

# `place` and `pr`: what a card asks about, when it is a place's (the
# ready-to-merge item: the place's id and the PR's number, pr-design §6.3)
type Card is Data:
  Card{id: Nat, kind: String, agent: String, text: String,
    for_msg: Maybe<&2, Nat>, created: Nat, place: Maybe<&2, String>,
    pr: Maybe<&2, Nat>}

type Agent is Data:
  Agent{name: String, is_main: Bool, parent: Maybe<&2, String>, life: Life,
    declared: Maybe<&2, Decl>, aliases: List<&2, String>,
    files: List<&2, String>, ws: J.Json, dir: String, brief: J.Json,
    created: Nat, failure: Maybe<&2, String>, note: String, report: J.Json,
    snap: Maybe<&2, String>}

type St is Data:
  St{agents: List<&2, Agent>, order: List<&2, String>, msgs: List<&2, MRec>,
    cards: List<&2, Card>, notes: List<&2, String>, next_msg: Nat,
    next_card: Nat}

type Ev is Data:
  ETaskCreated{name: String, parent: String, brief: J.Json, ws: J.Json, at: Nat}
  ELifecycle{name: String, life: Life, reason: Maybe<&2, String>}
  EWorkspace{name: String, ws: J.Json}
  ESnapshot{name: String, snap: Maybe<&2, String>}
  EDeclared{name: String, status: Maybe<&2, Decl>, note: String}
  EReported{name: String, report: J.Json}
  EFileTouched{name: String, path: String}
  ERenamed{name: String, new_name: String}
  EMsgSent{msg: Msg}
  EMsgState{id: Nat, st: MSt}
  ESettled{id: Nat}
  ECardOpened{card: Card}
  ECardClosed{id: Nat, resolution: String}
  EMainNote{text: String, at: Nat}
  ENotesFlushed{}

type AgentOp is Data:
  OLife{life: Life, reason: Maybe<&2, String>}
  OSnap{snap: Maybe<&2, String>}
  OReport{report: J.Json}
  OWs{ws: J.Json}
  ODeclared{d: Maybe<&2, Decl>, note: String}
  OFile{path: String}
  ORename{new_name: String}

type MsgOp is Data:
  OSt{st: MSt}
  OSettle{}

def MRec.msg(x: MRec) -> Msg:
  match x:
    case MRec{+msg, _, _}:
      msg

def MRec.st(x: MRec) -> MSt:
  match x:
    case MRec{_, +st, _}:
      st

def MRec.settled(x: MRec) -> Bool:
  match x:
    case MRec{_, _, +settled}:
      settled

def Card.id(x: Card) -> Nat:
  match x:
    case Card{+id, _, _, _, _, _, _, _}:
      id

def Card.kind(x: Card) -> String:
  match x:
    case Card{_, +kind, _, _, _, _, _, _}:
      kind

def Card.agent(x: Card) -> String:
  match x:
    case Card{_, _, +agent, _, _, _, _, _}:
      agent

def Card.text(x: Card) -> String:
  match x:
    case Card{_, _, _, +text, _, _, _, _}:
      text

def Card.for_msg(x: Card) -> Maybe<&2, Nat>:
  match x:
    case Card{_, _, _, _, +for_msg, _, _, _}:
      for_msg

def Card.created(x: Card) -> Nat:
  match x:
    case Card{_, _, _, _, _, +created, _, _}:
      created

def Card.place(x: Card) -> Maybe<&2, String>:
  match x:
    case Card{_, _, _, _, _, _, +place, _}:
      place

def Card.pr(x: Card) -> Maybe<&2, Nat>:
  match x:
    case Card{_, _, _, _, _, _, _, +pr}:
      pr

def St.agents(x: St) -> List<&2, Agent>:
  match x:
    case St{+agents, _, _, _, _, _, _}:
      agents

def St.order(x: St) -> List<&2, String>:
  match x:
    case St{_, +order, _, _, _, _, _}:
      order

def St.msgs(x: St) -> List<&2, MRec>:
  match x:
    case St{_, _, +msgs, _, _, _, _}:
      msgs

def St.cards(x: St) -> List<&2, Card>:
  match x:
    case St{_, _, _, +cards, _, _, _}:
      cards

def St.notes(x: St) -> List<&2, String>:
  match x:
    case St{_, _, _, _, +notes, _, _}:
      notes

def St.next_msg(x: St) -> Nat:
  match x:
    case St{_, _, _, _, _, +next_msg, _}:
      next_msg

def St.next_card(x: St) -> Nat:
  match x:
    case St{_, _, _, _, _, _, +next_card}:
      next_card

def Msg.id(x: Msg) -> Nat:
  match x:
    case Msg{+id, _, _, _, _, _, _, _, _, _, _, _}:
      id

# the first entry whose key is `k`
def find(~A: Data, ~K: Data, ~key: A -> K, ~eq: K -> K -> Bool, xs: List<&2, A>, +k: K) -> Maybe<&2, A>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, A>, eq(key(h), k), Some{h}, find(~A, ~K, ~key, ~eq, t, k))

def rec_of(xs: List<&2, MRec>, +id: Nat) -> Maybe<&2, MRec>:
  find(~MRec, ~Nat, ~(r => Msg.id(MRec.msg(r))), ~(a => b => Nat.is_eq(a, b)), xs, id)

def new_agent(+name: String, +is_main: Bool, +parent: Maybe<&2, String>, +ws: J.Json, +brief: J.Json, +created: Nat) -> Agent:
  Agent{name, is_main, parent, LActive{}, None{}, Nil{}, Nil{}, ws, name, brief, created, None{}, "", J.JNull{}, None{}}

def Agent.name(x: Agent) -> String:
  match x:
    case Agent{+name, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      name

# replace (or add) the agent named `n`
def put_agent(xs: List<&2, Agent>, +n: String, +a: Agent) -> List<&2, Agent>:
  match xs:
    case Nil{}:
      [a]
    case Con{+h, +t}:
      Bool.pick(List<&2, Agent>, String.eq(Agent.name(h), n), a <> t, h <> put_agent(t, n, a))

def rename_in(xs: List<&2, String>, +old: String, +new: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(String, String.eq(h, old), new, h) <> rename_in(t, old, new)

def MAIN() -> String:
  "main"

def USER() -> String:
  "user"

def HUB() -> String:
  "switchboard"

# pr-news (pr-design §10): the pseudo sender of GitHub's news (reviews,
# comments, failing checks) to the agent that owns a PR; like a peer
def GITHUB() -> String:
  "github"

# ---- small helpers ----

def snoc(-A: Data, xs: List<&2, A>, +x: A) -> List<&2, A>:
  match xs:
    case Nil{}:
      [x]
    case Con{+h, +t}:
      h <> snoc(A, t, x)

def str_in(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      String.eq(h, s) || str_in(t, s)

def nat_in(xs: List<&2, Nat>, +n: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Nat.is_eq(h, n) || nat_in(t, n)

def maybe_str_is(m: Maybe<&2, String>, +s: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+x}:
      String.eq(x, s)

def maybe_nat_is(m: Maybe<&2, Nat>, +n: Nat) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+x}:
      Nat.is_eq(x, n)

def one_line.push(empty: Bool, +word: String, +acc: List<&2, String>) -> List<&2, String>:
  match empty:
    case True{}:
      acc
    case False{}:
      snoc(String, acc, String.reverse(word))

# matches, not Bool.pick: pick would push (snoc, reverse) at every char
def one_line.word(space: Bool, +c: Char, +word: String) -> String:
  match space:
    case True{}:
      ""
    case False{}:
      SCon{c, word}

def one_line.acc(space: Bool, +word: String, +acc: List<&2, String>) -> List<&2, String>:
  match space:
    case True{}:
      one_line.push(String.is_empty(word), word, acc)
    case False{}:
      acc

# one line: whitespace runs collapse (util::one_line)
def one_line.go(s: String, +word: String, +acc: List<&2, String>) -> List<&2, String>:
  match s:
    case SNil{}:
      one_line.push(String.is_empty(word), word, acc)
    case SCon{+c, +t}:
      one_line.go(t, one_line.word(Char.is_space(c), c, word), one_line.acc(Char.is_space(c), word, acc))

def one_line(+s: String) -> String:
  String.join(one_line.go(s, "", Nil{}), " ")

# at most `max` chars, with an ellipsis when cut (util::clip)
def clip.pick(fits: Bool, +s: String, +max: Nat) -> String:
  match fits:
    case True{}:
      s
    case False{}:
      String.append(String.take(s, Nat.sub(max, 1n)), "…")

def clip(+s: String, +max: Nat) -> String:
  clip.pick(Nat.is_le(String.length(s), max), s, max)

def trim_is_empty(+s: String) -> Bool:
  String.is_empty(String.trim(s))

# ---- lookups ----

def agent_named(xs: List<&2, Agent>, +n: String) -> Maybe<&2, Agent>:
  find(~Agent, ~String, ~Agent.name, ~(a => b => String.eq(a, b)), xs, n)

def Agent.aliases(x: Agent) -> List<&2, String>:
  match x:
    case Agent{_, _, _, _, _, +aliases, _, _, _, _, _, _, _, _, _}:
      aliases

def agent_alias(xs: List<&2, Agent>, +n: String) -> Maybe<&2, String>:
  match xs:
    case Nil{}:
      None{}
    case Con{+a, +t}:
      Bool.pick(Maybe<&2, String>, str_in(Agent.aliases(a), n), Some{Agent.name(a)}, agent_alias(t, n))

def get_agent(+st: St, +n: String) -> Maybe<&2, Agent>:
  agent_named(St.agents(st), n)

# the canonical name: itself, or the task an alias points to
def resolve.pick(m: Maybe<&2, Agent>, +st: St, +n: String) -> Maybe<&2, String>:
  match m:
    case Some{+a}:
      Some{Agent.name(a)}
    case None{}:
      agent_alias(St.agents(st), n)

def resolve(+st: St, +n: String) -> Maybe<&2, String>:
  resolve.pick(get_agent(st, n), st, n)

def name_taken(+st: St, +n: String) -> Bool:
  String.eq(n, MAIN()) || String.eq(n, USER()) || String.eq(n, HUB()) || String.eq(n, GITHUB()) || Maybe.is_some(&2, String, resolve(st, n))

def agent_op.decl(l: Life, +d: Maybe<&2, Decl>) -> Maybe<&2, Decl>:
  match l:
    case LActive{}:
      None{}
    case LStopped{}:
      d
    case LArchived{}:
      d
    case LFailed{}:
      d

def agent_op.file(known: Bool, +files: List<&2, String>, +p: String) -> List<&2, String>:
  match known:
    case True{}:
      files
    case False{}:
      snoc(String, files, p)

def agent_op(+a: Agent, op: AgentOp) -> Agent:
  match a:
    case Agent{+name, +is_main, +parent, +life, +declared, +aliases, +files, +ws, +dir, +brief, +created, +failure, +note, +report, +snap}:
      match op:
        case OLife{+l, +why}:
          Agent{name, is_main, parent, l, agent_op.decl(l, declared), aliases, files, ws, dir, brief, created, why, note, report, snap}
        case OWs{+w}:
          Agent{name, is_main, parent, life, declared, aliases, files, w, dir, brief, created, failure, note, report, snap}
        case ODeclared{+d, +n}:
          Agent{name, is_main, parent, life, d, aliases, files, ws, dir, brief, created, failure, n, report, snap}
        case OFile{+p}:
          Agent{name, is_main, parent, life, declared, aliases, agent_op.file(str_in(files, p), files, p), ws, dir, brief, created, failure, note, report, snap}
        case ORename{+nn}:
          Agent{nn, is_main, parent, life, declared, snoc(String, aliases, name), files, ws, dir, brief, created, failure, note, report, snap}
        case OSnap{+sn}:
          Agent{name, is_main, parent, life, declared, aliases, files, ws, dir, brief, created, failure, note, report, sn}
        case OReport{+rp}:
          Agent{name, is_main, parent, life, declared, aliases, files, ws, dir, brief, created, failure, note, rp, snap}

# apply an operation to the agent named `n` (no agent: nothing changes)
def map_agent(xs: List<&2, Agent>, +n: String, +op: AgentOp) -> List<&2, Agent>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, Agent>, String.eq(Agent.name(h), n), agent_op(h, op) <> t, h <> map_agent(t, n, op))

def rec_op(+r: MRec, op: MsgOp) -> MRec:
  match r:
    case MRec{+m, +s, +settled}:
      match op:
        case OSt{+ns}:
          MRec{m, ns, settled}
        case OSettle{}:
          MRec{m, s, True{}}

def map_msg(xs: List<&2, MRec>, +id: Nat, +op: MsgOp) -> List<&2, MRec>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, MRec>, Nat.is_eq(Msg.id(MRec.msg(h)), id), rec_op(h, op) <> t, h <> map_msg(t, id, op))

def drop_card(xs: List<&2, Card>, +id: Nat) -> List<&2, Card>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, Card>, Nat.is_eq(Card.id(h), id), t, h <> drop_card(t, id))

def put_card(xs: List<&2, Card>, +c: Card) -> List<&2, Card>:
  snoc(Card, drop_card(xs, Card.id(c)), c)

# the hub's items with numbered options (opened by the hub's Rust side,
# `card_open`): the user's digit goes back to Rust (`card_choice`), which
# acts and closes the item; typed words go to main
def choice_kind(+k: String) -> Bool:
  String.eq(k, "merge") || String.eq(k, "feature_try") || String.eq(k, "feature_merge") || String.eq(k, "update") || String.eq(k, "signin")

# BISE-299: the user's inbox holds only what needs the user, by kind, set
# at the card's creation: main's escalations (`question`: `sb card`,
# `drop`: main's drop of a task with attention) and, later, tool-call
# confirmations (`confirm`), and the hub's own items about a place
# (`merge`: a PR ready to merge, pr-design §6.3). Only the user answers or
# closes them. The agents' traffic (reports, blocked, overlap, crashes) is
# main's, never a card: an older journal's cards of other kinds close at
# boot.
def user_kind(+k: String) -> Bool:
  String.eq(k, "question") || String.eq(k, "drop") || String.eq(k, "confirm") || choice_kind(k)

# the ids of the open cards that are not the user's (an older journal's)
def legacy_cards(xs: List<&2, Card>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, Nat>, user_kind(Card.kind(h)), legacy_cards(t), Card.id(h) <> legacy_cards(t))

# the open card that holds message `id` (an escalated question)
def card_holding(xs: List<&2, Card>, +id: Nat) -> Maybe<&2, Nat>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, Nat>, maybe_nat_is(Card.for_msg(h), id), Some{Card.id(h)}, card_holding(t, id))

def apply.settle.if(ok: Bool, +msgs: List<&2, MRec>, +id: Nat) -> List<&2, MRec>:
  match ok:
    case True{}:
      map_msg(msgs, id, OSettle{})
    case False{}:
      msgs

def Msg.to(x: Msg) -> String:
  match x:
    case Msg{_, _, _, +to, _, _, _, _, _, _, _, _}:
      to

def apply.settle.go(orig: Maybe<&2, MRec>, +msgs: List<&2, MRec>, +id: Nat, +from: String) -> List<&2, MRec>:
  match orig:
    case None{}:
      msgs
    case Some{+o}:
      apply.settle.if(String.eq(Msg.to(MRec.msg(o)), from) || String.eq(from, USER()), msgs, id)

# a reply from the recipient (or from the user) settles the question
def apply.settle(r: Maybe<&2, Nat>, +msgs: List<&2, MRec>, +from: String) -> List<&2, MRec>:
  match r:
    case None{}:
      msgs
    case Some{+id}:
      apply.settle.go(rec_of(msgs, id), msgs, id, from)

def Msg.reply_to(x: Msg) -> Maybe<&2, Nat>:
  match x:
    case Msg{_, _, _, _, +reply_to, _, _, _, _, _, _, _}:
      reply_to

def Msg.from(x: Msg) -> String:
  match x:
    case Msg{_, _, +from, _, _, _, _, _, _, _, _, _}:
      from

def Msg.thread(x: Msg) -> Nat:
  match x:
    case Msg{_, +thread, _, _, _, _, _, _, _, _, _, _}:
      thread

def apply(+st: St, e: Ev) -> St:
  match st:
    case St{+agents, +order, +msgs, +cards, +notes, +nm, +nc}:
      match e:
        case ETaskCreated{+name, +parent, +brief, +ws, +at}:
          St{put_agent(agents, name, new_agent(name, False{}, Some{parent}, ws, brief, at)),
            Bool.pick(List<&2, String>, str_in(order, name), order, snoc(String, order, name)),
            msgs, cards, notes, nm, nc}
        case ELifecycle{+name, +life, +reason}:
          St{map_agent(agents, name, OLife{life, reason}), order, msgs, cards, notes, nm, nc}
        case EWorkspace{+name, +ws}:
          St{map_agent(agents, name, OWs{ws}), order, msgs, cards, notes, nm, nc}
        case ESnapshot{+name, +snap}:
          St{map_agent(agents, name, OSnap{snap}), order, msgs, cards, notes, nm, nc}
        case EDeclared{+name, +status, +note}:
          St{map_agent(agents, name, ODeclared{status, note}), order, msgs, cards, notes, nm, nc}
        case EReported{+name, +report}:
          St{map_agent(agents, name, OReport{report}), order, msgs, cards, notes, nm, nc}
        case EFileTouched{+name, +path}:
          St{map_agent(agents, name, OFile{path}), order, msgs, cards, notes, nm, nc}
        case ERenamed{+name, +nn}:
          St{map_agent(agents, name, ORename{nn}), rename_in(order, name, nn), msgs, cards, notes, nm, nc}
        case EMsgSent{+m}:
          St{agents, order,
            apply.settle(Msg.reply_to(m), snoc(MRec, msgs, MRec{m, Queued{"new"}, False{}}), Msg.from(m)),
            cards, notes, Nat.max(Nat.max(nm, 1n+Msg.id(m)), 1n+Msg.thread(m)), nc}
        case EMsgState{+id, +s}:
          St{agents, order, map_msg(msgs, id, OSt{s}), cards, notes, nm, nc}
        case ESettled{+id}:
          St{agents, order, map_msg(msgs, id, OSettle{}), cards, notes, nm, nc}
        case ECardOpened{+c}:
          St{agents, order, msgs, put_card(cards, c), notes, nm, Nat.max(nc, 1n+Card.id(c))}
        case ECardClosed{+id, _}:
          St{agents, order, msgs, drop_card(cards, id), notes, nm, nc}
        case EMainNote{+text, _}:
          St{agents, order, msgs, cards, snoc(String, notes, text), nm, nc}
        case ENotesFlushed{}:
          St{agents, order, msgs, cards, Nil{}, nm, nc}

# the state after a journal: the replay of a list of events
def apply_all(evs: List<&2, Ev>, +st: St) -> St:
  match evs:
    case Nil{}:
      st
    case Con{+e, +rest}:
      apply_all(rest, apply(st, e))

# The boot replay of a journal in batches (main.bend `replay_many`).
# `apply` keeps the messages oldest first: a message_sent is a snoc and a
# message_state walks to a recent id, both linear in the messages, so a
# 20k-event journal replayed in ~18 s (quadratic). During a batch the
# messages are kept newest first (msgs_rev): a message_sent is a cons,
# and a message_state or a settle finds a recent id near the head and
# stops there (map_msg and rec_of use Bool.pick, which computes both
# branches: they always walk the whole list). The messages' own events
# take those early-stopping lookups; every other event is `apply`
# itself. A message is found by its id (unique): the order of the list
# does not change which one an event touches.

# the head of `xs` is the message `id`
def msg_at(xs: List<&2, MRec>, +id: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, _}:
      Nat.is_eq(Msg.id(MRec.msg(h)), id)

# map_msg that stops at the message; `hit`: msg_at(xs, id)
def map_msg.fast(xs: List<&2, MRec>, +hit: Bool, +id: Nat, +op: MsgOp) -> List<&2, MRec>:
  match xs hit:
    case Nil{} _:
      Nil{}
    case Con{+h, +t} True{}:
      rec_op(h, op) <> t
    case Con{+h, +t} False{}:
      h <> map_msg.fast(t, msg_at(t, id), id, op)

# rec_of that stops at the message; `hit`: msg_at(xs, id)
def rec_of.fast(xs: List<&2, MRec>, +hit: Bool, +id: Nat) -> Maybe<&2, MRec>:
  match xs hit:
    case Nil{} _:
      None{}
    case Con{+h, _} True{}:
      Some{h}
    case Con{_, +t} False{}:
      rec_of.fast(t, msg_at(t, id), id)

def map_msg.rev(+xs: List<&2, MRec>, +id: Nat, +op: MsgOp) -> List<&2, MRec>:
  map_msg.fast(xs, msg_at(xs, id), id, op)

# apply.settle, with the early-stopping lookups
def apply_rev.settle.if(ok: Bool, +msgs: List<&2, MRec>, +id: Nat) -> List<&2, MRec>:
  match ok:
    case True{}:
      map_msg.rev(msgs, id, OSettle{})
    case False{}:
      msgs

def apply_rev.settle.go(orig: Maybe<&2, MRec>, +msgs: List<&2, MRec>, +id: Nat, +from: String) -> List<&2, MRec>:
  match orig:
    case None{}:
      msgs
    case Some{+o}:
      apply_rev.settle.if(String.eq(Msg.to(MRec.msg(o)), from) || String.eq(from, USER()), msgs, id)

def apply_rev.settle(r: Maybe<&2, Nat>, +msgs: List<&2, MRec>, +from: String) -> List<&2, MRec>:
  match r:
    case None{}:
      msgs
    case Some{+id}:
      apply_rev.settle.go(rec_of.fast(msgs, msg_at(msgs, id), id), msgs, id, from)

def apply_rev.op(+st: St, +id: Nat, +op: MsgOp) -> St:
  match st:
    case St{+agents, +order, +msgs, +cards, +notes, +nm, +nc}:
      St{agents, order, map_msg.rev(msgs, id, op), cards, notes, nm, nc}

def apply_rev.sent(+st: St, +m: Msg) -> St:
  match st:
    case St{+agents, +order, +msgs, +cards, +notes, +nm, +nc}:
      St{agents, order,
        apply_rev.settle(Msg.reply_to(m), MRec{m, Queued{"new"}, False{}} <> msgs, Msg.from(m)),
        cards, notes, Nat.max(Nat.max(nm, 1n+Msg.id(m)), 1n+Msg.thread(m)), nc}

# `apply` on a state whose messages are newest first
def apply_rev(+e: Ev, +st: St) -> St:
  match e:
    case EMsgSent{+m}:
      apply_rev.sent(st, m)
    case EMsgState{+id, +s}:
      apply_rev.op(st, id, OSt{s})
    case ESettled{+id}:
      apply_rev.op(st, id, OSettle{})
    case _:
      apply(st, e)

# the messages turned around: oldest first <-> newest first
def msgs_rev(+st: St) -> St:
  match st:
    case St{+agents, +order, +msgs, +cards, +notes, +nm, +nc}:
      St{agents, order, List.reverse(&2, MRec, msgs), cards, notes, nm, nc}

def card_of(xs: List<&2, Card>, +id: Nat) -> Maybe<&2, Card>:
  find(~Card, ~Nat, ~Card.id, ~(a => b => Nat.is_eq(a, b)), xs, id)

def is_queued(s: MSt) -> Bool:
  match s:
    case Queued{_}:
      True{}
    case Delivered{}:
      False{}
    case Rejected{_}:
      False{}
    case Cancelled{}:
      False{}

def is_delivered(s: MSt) -> Bool:
  match s:
    case Queued{_}:
      False{}
    case Delivered{}:
      True{}
    case Rejected{_}:
      False{}
    case Cancelled{}:
      False{}

def queued_in(xs: List<&2, MRec>, +ns: List<&2, String>) -> List<&2, Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      +rest = queued_in(t, ns)
      Bool.pick(List<&2, Msg>, str_in(ns, Msg.to(MRec.msg(r))) && is_queued(MRec.st(r)), MRec.msg(r) <> rest, rest)

def names_of.go(m: Maybe<&2, Agent>) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{+a}:
      Agent.aliases(a)

# an agent's names: its own and its old ones (a message sent before a
# rename is still for it)
def names_of(+st: St, +n: String) -> List<&2, String>:
  n <> names_of.go(get_agent(st, n))

# messages waiting for delivery to `n` (under any of its names), oldest first
def queued_for(+st: St, +n: String) -> List<&2, Msg>:
  queued_in(St.msgs(st), names_of(st, n))

def Msg.expect(x: Msg) -> Bool:
  match x:
    case Msg{_, _, _, _, _, +expect, _, _, _, _, _, _}:
      expect

def unanswered_in(xs: List<&2, MRec>, +ns: List<&2, String>) -> List<&2, Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      +rest = unanswered_in(t, ns)
      Bool.pick(List<&2, Msg>, str_in(ns, Msg.to(MRec.msg(r))) && Msg.expect(MRec.msg(r)) && Bool.not(MRec.settled(r)) && is_delivered(MRec.st(r)), MRec.msg(r) <> rest, rest)

# delivered messages to `n` that expect a reply it has not given
def unanswered_for(+st: St, +n: String) -> List<&2, Msg>:
  unanswered_in(St.msgs(st), names_of(st, n))

def msg_of.go(m: Maybe<&2, MRec>) -> Maybe<&2, Msg>:
  match m:
    case None{}:
      None{}
    case Some{+r}:
      Some{MRec.msg(r)}

def msg_of(+st: St, +id: Nat) -> Maybe<&2, Msg>:
  msg_of.go(rec_of(St.msgs(st), id))

def state_of.go(m: Maybe<&2, MRec>) -> Maybe<&2, MSt>:
  match m:
    case None{}:
      None{}
    case Some{+r}:
      Some{MRec.st(r)}

def state_of(+st: St, +id: Nat) -> Maybe<&2, MSt>:
  state_of.go(rec_of(St.msgs(st), id))

def ws_worktree(+ws: J.Json) -> Bool:
  String.eq(J.get_str_or(ws, "mode", "shared"), "worktree")

def ws_dropped(+ws: J.Json) -> Bool:
  J.get_bool_or(ws, "dropped", False{})

def ws_branch(+ws: J.Json) -> J.Json:
  Maybe.default(&2, J.Json, J.get(ws, "branch"), J.JNull{})

def ws_path(+ws: J.Json) -> String:
  J.get_str_or(ws, "path", "")

def init(+workspace: String) -> St:
  +ws = J.obj([J.kv("mode", J.str("shared")), J.kv("path", J.str(workspace)),
    J.kv("branch", J.JNull{}), J.kv("base_commit", J.JNull{}), J.kv("dropped", J.JBool{False{}})])
  +brief = J.obj([J.kv("objective", J.str("")), J.kv("context", J.str("")), J.kv("constraints", J.JArr{Nil{}}),
    J.kv("done_when", J.JNull{}), J.kv("report_format", J.JNull{})])
  St{[new_agent(MAIN(), True{}, None{}, ws, brief, 0n)], [MAIN()], Nil{}, Nil{}, Nil{}, 1n, 1n}

def Msg.auto(x: Msg) -> Bool:
  match x:
    case Msg{_, _, _, _, _, _, +auto, _, _, _, _, _}:
      auto

def Msg.text(x: Msg) -> String:
  match x:
    case Msg{_, _, _, _, _, _, _, +text, _, _, _, _}:
      text

def Msg.created(x: Msg) -> Nat:
  match x:
    case Msg{_, _, _, _, _, _, _, _, +created, _, _, _}:
      created

def Msg.plain(x: Msg) -> Bool:
  match x:
    case Msg{_, _, _, _, _, _, _, _, _, +plain, _, _}:
      plain

def Msg.queued(x: Msg) -> Bool:
  match x:
    case Msg{_, _, _, _, _, _, _, _, _, _, +queued, _}:
      queued

def Msg.via(x: Msg) -> Maybe<&2, String>:
  match x:
    case Msg{_, _, _, _, _, _, _, _, _, _, _, +via}:
      via

# the last `max` chars, after an ellipsis when cut (util::clip_tail)
def clip_tail.pick(fits: Bool, +s: String, +max: Nat) -> String:
  match fits:
    case True{}:
      s
    case False{}:
      String.append("…", String.drop(s, Nat.sub(String.length(s), Nat.sub(max, 1n))))

def clip_tail(+s: String, +max: Nat) -> String:
  clip_tail.pick(Nat.is_le(String.length(s), max), s, max)

def Agent.is_main(x: Agent) -> Bool:
  match x:
    case Agent{_, +is_main, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      is_main

def Agent.parent(x: Agent) -> Maybe<&2, String>:
  match x:
    case Agent{_, _, +parent, _, _, _, _, _, _, _, _, _, _, _, _}:
      parent

def Agent.life(x: Agent) -> Life:
  match x:
    case Agent{_, _, _, +life, _, _, _, _, _, _, _, _, _, _, _}:
      life

def Agent.declared(x: Agent) -> Maybe<&2, Decl>:
  match x:
    case Agent{_, _, _, _, +declared, _, _, _, _, _, _, _, _, _, _}:
      declared

def Agent.files(x: Agent) -> List<&2, String>:
  match x:
    case Agent{_, _, _, _, _, _, +files, _, _, _, _, _, _, _, _}:
      files

def Agent.ws(x: Agent) -> J.Json:
  match x:
    case Agent{_, _, _, _, _, _, _, +ws, _, _, _, _, _, _, _}:
      ws

def Agent.dir(x: Agent) -> String:
  match x:
    case Agent{_, _, _, _, _, _, _, _, +dir, _, _, _, _, _, _}:
      dir

def Agent.brief(x: Agent) -> J.Json:
  match x:
    case Agent{_, _, _, _, _, _, _, _, _, +brief, _, _, _, _, _}:
      brief

def Agent.created(x: Agent) -> Nat:
  match x:
    case Agent{_, _, _, _, _, _, _, _, _, _, +created, _, _, _, _}:
      created

def Agent.failure(x: Agent) -> Maybe<&2, String>:
  match x:
    case Agent{_, _, _, _, _, _, _, _, _, _, _, +failure, _, _, _}:
      failure

def Agent.note(x: Agent) -> String:
  match x:
    case Agent{_, _, _, _, _, _, _, _, _, _, _, _, +note, _, _}:
      note

def Agent.report(x: Agent) -> J.Json:
  match x:
    case Agent{_, _, _, _, _, _, _, _, _, _, _, _, _, +report, _}:
      report

def Agent.snap(x: Agent) -> Maybe<&2, String>:
  match x:
    case Agent{_, _, _, _, _, _, _, _, _, _, _, _, _, _, +snap}:
      snap

# dev-flow §3.1: the place of a ws: "shared", or its worktree's id (the
# Rust Workspace::place_id; an older journal's worktree has none: its
# agent's own place, wt:<dir>)
def ws_place(+ws: J.Json, +dir: String) -> String:
  Bool.pick(String, ws_worktree(ws), J.get_str_or(ws, "place", String.append("wt:", dir)), "shared")

def Agent.place(+a: Agent) -> String:
  ws_place(Agent.ws(a), Agent.dir(a))
