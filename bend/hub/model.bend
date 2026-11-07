# hub/model.bend — the Switchboard hub's events and how they change its
# durable state (RFC 0001 §12; the state itself, S.St, is declared in
# bise_proto::core::state, gen/state.bend). `apply` is the only way the
# durable state changes, through S.St's setters (the agents split, issue
# 21 sha 2: St.agents the agents not archived, St.arch the archived ones,
# hub/agents.bend; lookups read St.agents, then arch); the
# Rust daemon keeps a read-only mirror of it (rust/switchboard/src/
# model.rs) fed by the journal events the hub emits.
import Base
import ../vendor/json.bend as J
import ./timers.bend as TM
import ./wakes.bend as WK
import ./xhub-model.bend as XH
import ./lazy.bend as LZ
import ./gone.bend as G
import ./names.bend as NM
import ./agents.bend as AG
import ./gen/state.bend as S

type Ev is Data:
  ETaskCreated{name: String, parent: String, brief: J.Json, ws: J.Json, at: Nat}
  ELifecycle{name: String, life: S.Life, reason: Maybe<&2, String>}
  EWorkspace{name: String, ws: J.Json}
  ESnapshot{name: String, snap: Maybe<&2, String>}
  # hub-loop: the worktree made for a placing task (`err` "": `ws`)
  EPlaced{name: String, ws: J.Json, err: String}
  EDeclared{name: String, status: Maybe<&2, S.Decl>, note: String}
  EReported{name: String, report: J.Json}
  EFileTouched{name: String, path: String}
  # land-claims: the agent's claim on `path` ends (its file had no change
  # left after a land, or at boot: IClaimsClean)
  EClaimDropped{name: String, path: String}
  ERenamed{name: String, new_name: String}
  # name-free: a live agent's hot messages to its old name `from` (taken)
  EMsgsMoved{from: String, to: String}
  EMsgSent{msg: S.Msg}
  EMsgState{id: Nat, st: S.MSt}
  ESettled{id: Nat}
  ECardOpened{card: S.Card}
  # `ans` his answer (via "": closed without him, `resolution` says why)
  ECardClosed{id: Nat, resolution: String, ans: S.Ans}
  # his answer's route line position in the asker's thread (answer_pos_once)
  ECardAnswerPos{card: Nat, agent: String, pos: Nat}
  # main withdrew card `id` (`text`), his late answer delivered (gone.bend)
  ECardWithdrawn{id: Nat, text: String}
  ECardLate{id: Nat}
  EMainNote{text: String, at: Nat}
  ENotesFlushed{}
  # `sb every`'s timers (timers.bend): set, fired, run, stop, name (its
  # name; `announce`: the ◷ set line its held timer waited for)
  ETimerSet{t: S.Timer, at: Nat}
  ETimerFired{id: Nat, fired: Nat, next: Nat, at: Nat, undelivered: Bool}
  ETimerRun{id: Nat, at: Nat}
  ETimerStop{id: Nat, why: String, at: Nat}
  ETimerName{id: Nat, name: String, announce: Bool, at: Nat}
  # bise desktop S2: the cross-hub state's lines (xhub-model.bend) and
  # the acts' (timers.bend): one constructor each, their kinds inside
  EX{e: XH.XEv}
  EAct{e: TM.ActEv}
  # S10: a follow (or its stop), a followed job's end: follow off (law
  # follow_one_shot); `hub` the asker, "" this hub (follow_hub_journaled)
  EFollow{name: String, on: Bool, hub: String}
  EJobEnd{name: String, state: String}
  # event-wake: `sb wake`'s watches (wakes.bend); an older core skips them
  EWake{e: WK.WkEv}
  # cost-pause (pause.bend): paused, resumed at `at`; an older core skips
  # them (after a rollback the project comes back unpaused)
  EPaused{at: Nat}
  EResumed{at: Nat}

type AgentOp is Data:
  OLife{life: S.Life, reason: Maybe<&2, String>}
  OSnap{snap: Maybe<&2, String>}
  OReport{report: J.Json}
  OWs{ws: J.Json}
  ODeclared{d: Maybe<&2, S.Decl>, note: String}
  OFile{path: String}
  OUnclaim{path: String}
  ORename{new_name: String}
  OFollow{f: Maybe<&2, String>}

type MsgOp is Data:
  OSt{st: S.MSt}
  OSettle{}

# the first entry whose key is `k`
def find(~A: Data, ~K: Data, ~key: A -> K, ~eq: K -> K -> Bool, xs: List<&2, A>, +k: K) -> Maybe<&2, A>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      LZ.pick(Maybe<&2, A>, eq(key(h), k), u => Some{h}, u => find(~A, ~K, ~key, ~eq, t, k))

def rec_of(xs: List<&2, S.MRec>, +id: Nat) -> Maybe<&2, S.MRec>:
  find(~S.MRec, ~Nat, ~(r => S.Msg.id(S.MRec.msg(r))), ~(a => b => Nat.is_eq(a, b)), xs, id)

# a new agent, from its task_created line (the journal's named fields, so no
# two Strings or two Jsons can swap); `dir`: its folder, the stable id of its
# files (hub/names.bend free_dir). Any other line: the empty agent
def agent_of_new(e: Ev, +dir: String) -> S.Agent:
  match e:
    case ETaskCreated{+name, +parent, +brief, +ws, +at}:
      S.Agent.with_created(S.Agent.with_brief(S.Agent.with_ws(S.Agent.with_parent(S.Agent.with_dir(S.Agent.with_name(S.Agent.empty(), name), dir), Some{parent}), ws), brief), at)
    case _:
      S.Agent.empty()

# replace (or add) the agent named `n`
def put_agent(xs: List<&2, S.Agent>, +n: String, +a: S.Agent) -> List<&2, S.Agent>:
  match xs:
    case Nil{}:
      [a]
    case Con{+h, +t}:
      LZ.pick(List<&2, S.Agent>, String.eq(S.Agent.name(h), n), u => a <> t, u => h <> put_agent(t, n, a))

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

# mcp-apps (docs/mcp-apps.md, C5): the fixed sender of an MCP App view's
# `ui/message`; its plugin is the message's typed `view` field, never a
# part of this name. Never the user (law view_msg_never_user)
def VIEW() -> String:
  "view"

# ---- small helpers ----

def snoc(-A: Data, xs: List<&2, A>, +x: A) -> List<&2, A>:
  match xs:
    case Nil{}:
      [x]
    case Con{+h, +t}:
      h <> snoc(A, t, x)

# the strings of a JSON list (the rest skipped)
def jstr_list.one(h: J.Json, +rest: List<&2, String>) -> List<&2, String>:
  match h:
    case J.JStr{+s}:
      s <> rest
    case _:
      rest

def jstr_list(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = jstr_list(t)
      jstr_list.one(h, rest)

def str_in(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.or(String.eq(h, s), u => str_in(t, s))

def nat_in(xs: List<&2, Nat>, +n: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.or(Nat.is_eq(h, n), u => nat_in(t, n))

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

# one_line, clip, clip_tail: hub/text.bend
def trim_is_empty(+s: String) -> Bool:
  String.is_empty(String.trim(s))

# ---- lookups ----

def agent_named(xs: List<&2, S.Agent>, +n: String) -> Maybe<&2, S.Agent>:
  find(~S.Agent, ~String, ~S.Agent.name, ~(a => b => String.eq(a, b)), xs, n)

def agent_alias(xs: List<&2, S.Agent>, +n: String) -> Maybe<&2, String>:
  match xs:
    case Nil{}:
      None{}
    case Con{+a, +t}:
      LZ.pick(Maybe<&2, String>, str_in(S.Agent.aliases(a), n), u => Some{S.Agent.name(a)}, u => agent_alias(t, n))

# the agents split (agents.bend): St.agents, then the archived ones
def get_agent.or(m: Maybe<&2, S.Agent>, +st: S.St, +n: String) -> Maybe<&2, S.Agent>:
  match m:
    case Some{+a}:
      Some{a}
    case None{}:
      agent_named(S.St.arch(st), n)

def get_agent(+st: S.St, +n: String) -> Maybe<&2, S.Agent>:
  get_agent.or(agent_named(S.St.agents(st), n), st, n)

# the agents not archived, in creation order (the live walks: a tick's
# boot, overlaps, retirees)
def live_names(+st: S.St) -> List<&2, String>:
  AG.names(S.St.agents(st))

def alias_of.or(m: Maybe<&2, String>, +st: S.St, +n: String) -> Maybe<&2, String>:
  match m:
    case Some{+a}:
      Some{a}
    case None{}:
      agent_alias(S.St.arch(st), n)

# the agent that has `n` as an old name
def alias_of(+st: S.St, +n: String) -> Maybe<&2, String>:
  alias_of.or(agent_alias(S.St.agents(st), n), st, n)

# the canonical name: itself, or the task an alias points to
def resolve.pick(m: Maybe<&2, S.Agent>, +st: S.St, +n: String) -> Maybe<&2, String>:
  match m:
    case Some{+a}:
      Some{S.Agent.name(a)}
    case None{}:
      alias_of(st, n)

def resolve(+st: S.St, +n: String) -> Maybe<&2, String>:
  resolve.pick(get_agent(st, n), st, n)

def reserved(+n: String) -> Bool:
  String.eq(n, MAIN()) || String.eq(n, USER()) || String.eq(n, HUB()) || String.eq(n, GITHUB()) || String.eq(n, VIEW())

def live_life(l: S.Life) -> Bool:
  match l:
    case S.LArchived{}:
      False{}
    case _:
      True{}

def live_agent(m: Maybe<&2, S.Agent>) -> Bool:
  match m:
    case Some{+a}:
      live_life(S.Agent.life(a))
    case None{}:
      False{}

# name-free: a live (not archived) agent's name or bise's is taken; an
# archived one's and an old name are free (core take, apply's strip)
def name_taken(+st: S.St, +n: String) -> Bool:
  reserved(n) || live_agent(get_agent(st, n))

# no agent has it (any life), nor bise: a take's event (cx FreshEv)
def name_held(+st: S.St, +n: String) -> Bool:
  reserved(n) || Maybe.is_some(&2, S.Agent, get_agent(st, n))

def agent_op.decl(l: S.Life, +d: Maybe<&2, S.Decl>) -> Maybe<&2, S.Decl>:
  match l:
    case S.LActive{}:
      None{}
    case S.LStopped{}:
      d
    case S.LArchived{}:
      d
    case S.LFailed{}:
      d

def agent_op.file(known: Bool, +files: List<&2, String>, +p: String) -> List<&2, String>:
  match known:
    case True{}:
      files
    case False{}:
      snoc(String, files, p)

def agent_op(+a: S.Agent, op: AgentOp) -> S.Agent:
  match op:
    case OLife{+l, +why}:
      S.Agent.with_failure(S.Agent.with_declared(S.Agent.with_life(a, l), agent_op.decl(l, S.Agent.declared(a))), why)
    case OWs{+w}:
      S.Agent.with_ws(a, w)
    case ODeclared{+d, +n}:
      S.Agent.with_note(S.Agent.with_declared(a, d), n)
    case OFile{+p}:
      S.Agent.with_files(a, agent_op.file(str_in(S.Agent.files(a), p), S.Agent.files(a), p))
    case OUnclaim{+p}:
      S.Agent.with_files(a, NM.drop_str(S.Agent.files(a), p))
    case ORename{+nn}:
      S.Agent.with_aliases(S.Agent.with_name(a, nn), snoc(String, S.Agent.aliases(a), S.Agent.name(a)))
    case OSnap{+sn}:
      S.Agent.with_snap(a, sn)
    case OReport{+rp}:
      S.Agent.with_report(a, rp)
    case OFollow{+f}:
      S.Agent.with_follow(a, f)

def Agent.strip(+a: S.Agent, +n: String) -> S.Agent:
  S.Agent.with_aliases(a, NM.drop_str(S.Agent.aliases(a), n))

# the agents without the old name `n` (another agent takes it)
def strip(xs: List<&2, S.Agent>, +n: String) -> List<&2, S.Agent>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +t}:
      Agent.strip(a, n) <> strip(t, n)

def dirs_of(xs: List<&2, S.Agent>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +t}:
      S.Agent.dir(a) <> dirs_of(t)

# the head of `xs` is the agent named `n`
def name_at(xs: List<&2, S.Agent>, +n: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, _}:
      String.eq(S.Agent.name(h), n)

# map_agent's walk: it stops at the agent (`hit`: name_at(xs, n)); a match,
# not Bool.pick (both branches: every agent, at every agent event)
def map_agent.go(xs: List<&2, S.Agent>, +hit: Bool, +n: String, +op: AgentOp) -> List<&2, S.Agent>:
  match xs hit:
    case Nil{} _:
      Nil{}
    case Con{+h, +t} True{}:
      agent_op(h, op) <> t
    case Con{+h, +t} False{}:
      h <> map_agent.go(t, name_at(t, n), n, op)

# apply an operation to the agent named `n` (no agent: nothing changes)
def map_agent(+xs: List<&2, S.Agent>, +n: String, +op: AgentOp) -> List<&2, S.Agent>:
  map_agent.go(xs, name_at(xs, n), n, op)

def rec_op(+r: S.MRec, op: MsgOp) -> S.MRec:
  match op:
    case OSt{+ns}:
      S.MRec.with_st(r, ns)
    case OSettle{}:
      S.MRec.with_settled(r, True{})

def map_msg(xs: List<&2, S.MRec>, +id: Nat, +op: MsgOp) -> List<&2, S.MRec>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      LZ.pick(List<&2, S.MRec>, Nat.is_eq(S.Msg.id(S.MRec.msg(h)), id), u => rec_op(h, op) <> t, u => h <> map_msg(t, id, op))

def drop_card(xs: List<&2, S.Card>, +id: Nat) -> List<&2, S.Card>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      LZ.pick(List<&2, S.Card>, Nat.is_eq(S.Card.id(h), id), u => t, u => h <> drop_card(t, id))

def put_card(xs: List<&2, S.Card>, +c: S.Card) -> List<&2, S.Card>:
  snoc(S.Card, drop_card(xs, S.Card.id(c)), c)

def is_queued(s: S.MSt) -> Bool:
  match s:
    case S.Queued{_}:
      True{}
    case S.Delivered{}:
      False{}
    case S.Rejected{_}:
      False{}
    case S.Cancelled{}:
      False{}
def is_delivered(s: S.MSt) -> Bool:
  match s:
    case S.Queued{_}:
      False{}
    case S.Delivered{}:
      True{}
    case S.Rejected{_}:
      False{}
    case S.Cancelled{}:
      False{}
# one of the messages is from the user
def any_from_user(xs: List<&2, S.Msg>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.pick(Bool, String.eq(S.Msg.from(h), USER()), u => True{}, u => any_from_user(t))
# the record with its message to `to` (msgs_moved)
def MRec.to(+r: S.MRec, +to: String) -> S.MRec:
  S.MRec.with_msg(r, S.Msg.with_to(S.MRec.msg(r), to))
# a sent message's record: queued ("new"), not settled
def new_rec(+m: S.Msg) -> S.MRec:
  S.MRec.with_st(S.MRec.with_msg(S.MRec.empty(), m), S.Queued{"new"})
# a delivered message that expects a reply its recipient has not given:
# the steps (unanswered_in) and the view (view.bend rec_json) both read it
def MRec.owes(+r: S.MRec) -> Bool:
  S.Msg.expect(S.MRec.msg(r)) && Bool.not(S.MRec.settled(r)) && is_delivered(S.MRec.st(r))
# queued, or owing a reply (issue 21): St.hot's one predicate; "cold" else
def MRec.is_hot(+r: S.MRec) -> Bool:
  is_queued(S.MRec.st(r)) || MRec.owes(r)
# the hot messages of `xs`, in their order
def hot_of(xs: List<&2, S.MRec>) -> List<&2, S.MRec>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      +rest = hot_of(t)
      Bool.pick(List<&2, S.MRec>, MRec.is_hot(r), r <> rest, rest)
# the cold ones, in their order
def cold_of(xs: List<&2, S.MRec>) -> List<&2, S.MRec>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      +rest = cold_of(t)
      Bool.pick(List<&2, S.MRec>, MRec.is_hot(r), rest, r <> rest)
# ---- the split (issue 21 steps 3-4): St.hot ascending, St.old descending ----
def MRec.id(+r: S.MRec) -> Nat:
  S.Msg.id(S.MRec.msg(r))
# the head of `xs` has an id above `n`
def id_above(xs: List<&2, S.MRec>, +n: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, _}:
      Nat.is_gt(MRec.id(h), n)
# `r` into `xs` (ids descending), before the first id not above its own
def ins_desc.go(xs: List<&2, S.MRec>, gt: Bool, +r: S.MRec) -> List<&2, S.MRec>:
  match xs gt:
    case Nil{} _:
      [r]
    case Con{+h, +t} True{}:
      h <> ins_desc.go(t, id_above(t, MRec.id(r)), r)
    case Con{+h, +t} False{}:
      r <> h <> t
def ins_desc(+xs: List<&2, S.MRec>, +r: S.MRec) -> List<&2, S.MRec>:
  ins_desc.go(xs, id_above(xs, MRec.id(r)), r)
# `r` into `xs` (ids ascending), before the first id above its own
def ins_asc.go(xs: List<&2, S.MRec>, gt: Bool, +r: S.MRec) -> List<&2, S.MRec>:
  match xs gt:
    case Nil{} _:
      [r]
    case Con{+h, +t} True{}:
      r <> h <> t
    case Con{+h, +t} False{}:
      h <> ins_asc.go(t, id_above(t, MRec.id(r)), r)
def ins_asc(+xs: List<&2, S.MRec>, +r: S.MRec) -> List<&2, S.MRec>:
  ins_asc.go(xs, id_above(xs, MRec.id(r)), r)
# the messages of `hs` put into `os` (ids descending)
def ins_all(hs: List<&2, S.MRec>, +os: List<&2, S.MRec>) -> List<&2, S.MRec>:
  match hs:
    case Nil{}:
      os
    case Con{+r, +t}:
      ins_desc(ins_all(t, os), r)
# every message in send order (view_all; the laws' HI.Unique, HI.Kept)
def every(+st: S.St) -> List<&2, S.MRec>:
  List.reverse(&2, S.MRec, ins_all(S.St.hot(st), S.St.old(st)))
def drop_msg(xs: List<&2, S.MRec>, +id: Nat) -> List<&2, S.MRec>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      LZ.pick(List<&2, S.MRec>, Nat.is_eq(MRec.id(h), id), u => t, u => h <> drop_msg(t, id))
# the two sides after a message event
type Ms is Data:
  Ms{hot: List<&2, S.MRec>, old: List<&2, S.MRec>}
def Ms.hot(x: Ms) -> List<&2, S.MRec>:
  match x:
    case Ms{+hot, _}:
      hot
def Ms.old(x: Ms) -> List<&2, S.MRec>:
  match x:
    case Ms{_, +old}:
      old
# the op on a cold message: back to the hot side if it made it hot, else in place
def msg_op.cold.go(back: Bool, +c: S.MRec, +hot: List<&2, S.MRec>, +old: List<&2, S.MRec>, +id: Nat, +op: MsgOp) -> Ms:
  match back:
    case True{}:
      Ms{ins_asc(hot, rec_op(c, op)), drop_msg(old, id)}
    case False{}:
      Ms{hot, map_msg(old, id, op)}
def msg_op.cold(m: Maybe<&2, S.MRec>, +hot: List<&2, S.MRec>, +old: List<&2, S.MRec>, +id: Nat, +op: MsgOp) -> Ms:
  match m:
    case None{}:
      Ms{hot, old}
    case Some{+c}:
      msg_op.cold.go(MRec.is_hot(rec_op(c, op)), c, hot, old, id, op)
# the op on a hot message: the one it made cold goes to the old side
def msg_op.hot(+hs: List<&2, S.MRec>, +old: List<&2, S.MRec>) -> Ms:
  Ms{hot_of(hs), ins_all(cold_of(hs), old)}
def msg_op.in(in_hot: Bool, +hot: List<&2, S.MRec>, +old: List<&2, S.MRec>, +id: Nat, +op: MsgOp) -> Ms:
  match in_hot:
    case True{}:
      msg_op.hot(map_msg(hot, id, op), old)
    case False{}:
      msg_op.cold(rec_of(old, id), hot, old, id, op)
# a message event's op on the message `id`, and its move when it turns cold (or hot)
def msg_op(+hot: List<&2, S.MRec>, +old: List<&2, S.MRec>, +id: Nat, +op: MsgOp) -> Ms:
  msg_op.in(Maybe.is_some(&2, S.MRec, rec_of(hot, id)), hot, old, id, op)
def rec_in.go(m: Maybe<&2, S.MRec>, +old: List<&2, S.MRec>, +id: Nat) -> Maybe<&2, S.MRec>:
  match m:
    case None{}:
      rec_of(old, id)
    case Some{+r}:
      Some{r}
# the record of the message `id`, hot or cold
def rec_in(+hot: List<&2, S.MRec>, +old: List<&2, S.MRec>, +id: Nat) -> Maybe<&2, S.MRec>:
  rec_in.go(rec_of(hot, id), old, id)
def rec_at(+st: S.St, +id: Nat) -> Maybe<&2, S.MRec>:
  rec_in(S.St.hot(st), S.St.old(st), id)
# a reply from the recipient (or from the user) settles the question
def settle_in.if(ok: Bool, +hot: List<&2, S.MRec>, +old: List<&2, S.MRec>, +id: Nat) -> Ms:
  match ok:
    case True{}:
      msg_op(hot, old, id, OSettle{})
    case False{}:
      Ms{hot, old}
def settle_in.go(orig: Maybe<&2, S.MRec>, +hot: List<&2, S.MRec>, +old: List<&2, S.MRec>, +id: Nat, +from: String) -> Ms:
  match orig:
    case None{}:
      Ms{hot, old}
    case Some{+o}:
      settle_in.if(String.eq(S.Msg.to(S.MRec.msg(o)), from) || String.eq(from, USER()), hot, old, id)
def settle_in(r: Maybe<&2, Nat>, +hot: List<&2, S.MRec>, +old: List<&2, S.MRec>, +from: String) -> Ms:
  match r:
    case None{}:
      Ms{hot, old}
    case Some{+id}:
      settle_in.go(rec_in(hot, old, id), hot, old, id, from)

# name-free (msgs_moved): a hot record to `from` goes to `to`, same id
def moved.if(hit: Bool, +r: S.MRec, +to: String) -> S.MRec:
  match hit:
    case True{}:
      MRec.to(r, to)
    case False{}:
      r

def moved(xs: List<&2, S.MRec>, +from: String, +to: String) -> List<&2, S.MRec>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      moved.if(MRec.is_hot(r) && String.eq(S.Msg.to(S.MRec.msg(r)), from), r, to) <> moved(t, from, to)

# ---- the agents split (agents.bend; laws HI.AgentsSplit, names_apart) ----
# Each agent event computes the two sides from St.agents, arch and
# St.order: `side` True the new St.agents, False the new arch (AG.side);
# AG.set puts both back, so every other field is unchanged by computation.

# an agent event, on the side that holds the agent (St.agents first: a
# match, so arch is walked only for an archived agent's event)
def on_agent.go(live: Bool, side: Bool, +ag: List<&2, S.Agent>, +ar: List<&2, S.Agent>, +n: String, +op: AgentOp) -> List<&2, S.Agent>:
  match live side:
    case True{} True{}:
      map_agent(ag, n, op)
    case True{} False{}:
      ar
    case False{} True{}:
      ag
    case False{} False{}:
      map_agent(ar, n, op)

def on_agent(+st: S.St, +n: String, +op: AgentOp) -> S.St:
  +c = AG.has(S.St.agents(st), n)
  AG.set(st, on_agent.go(c, True{}, S.St.agents(st), S.St.arch(st), n, op), on_agent.go(c, False{}, S.St.agents(st), S.St.arch(st), n, op))

# hub-loop: a placed task's worktree (`ok`: no error), else nothing
def placed.go(ok: Bool, side: Bool, +ag: List<&2, S.Agent>, +ar: List<&2, S.Agent>, +n: String, +ws: J.Json) -> List<&2, S.Agent>:
  match ok:
    case True{}:
      on_agent.go(AG.has(ag, n), side, ag, ar, n, OWs{ws})
    case False{}:
      AG.side(side, ag, ar)

def placed(+ok: Bool, +st: S.St, +n: String, +ws: J.Json) -> S.St:
  AG.set(st, placed.go(ok, True{}, S.St.agents(st), S.St.arch(st), n, ws), placed.go(ok, False{}, S.St.agents(st), S.St.arch(st), n, ws))

# a lifecycle: a live agent archived goes to arch's head, an archived one
# brought back to its creation place in St.agents; else it stays
def life.live.to(stay: Bool, side: Bool, +a: S.Agent, +pre: List<&2, S.Agent>, +post: List<&2, S.Agent>, +ar: List<&2, S.Agent>) -> List<&2, S.Agent>:
  match stay side:
    case True{} True{}:
      List.append(&2, S.Agent, pre, a <> post)
    case True{} False{}:
      ar
    case False{} True{}:
      List.append(&2, S.Agent, pre, post)
    case False{} False{}:
      a <> ar

def life.arch.to(back: Bool, side: Bool, +a: S.Agent, +pre: List<&2, S.Agent>, +post: List<&2, S.Agent>, +ag: List<&2, S.Agent>, +ord: List<&2, String>) -> List<&2, S.Agent>:
  match back side:
    case True{} True{}:
      AG.ins(ord, ag, a)
    case True{} False{}:
      List.append(&2, S.Agent, pre, post)
    case False{} True{}:
      ag
    case False{} False{}:
      List.append(&2, S.Agent, pre, a <> post)

def life.arch(m: Maybe<&2, S.Agent>, side: Bool, +pre: List<&2, S.Agent>, +post: List<&2, S.Agent>, +ag: List<&2, S.Agent>, +ar: List<&2, S.Agent>, +ord: List<&2, String>, +l: S.Life, +why: Maybe<&2, String>) -> List<&2, S.Agent>:
  match m:
    case None{}:
      AG.side(side, ag, ar)
    case Some{+a}:
      life.arch.to(live_life(l), side, agent_op(a, OLife{l, why}), pre, post, ag, ord)

def life.live(m: Maybe<&2, S.Agent>, side: Bool, +pre: List<&2, S.Agent>, +post: List<&2, S.Agent>, +ag: List<&2, S.Agent>, +ar: List<&2, S.Agent>, +ord: List<&2, String>, +n: String, +l: S.Life, +why: Maybe<&2, String>) -> List<&2, S.Agent>:
  match m:
    case None{}:
      life.arch(AG.hit(ar, n), side, AG.pre(ar, n), AG.post(ar, n), ag, ar, ord, l, why)
    case Some{+a}:
      life.live.to(live_life(l), side, agent_op(a, OLife{l, why}), pre, post, ar)

def life_ev(+st: S.St, +n: String, +l: S.Life, +why: Maybe<&2, String>) -> S.St:
  +ag = S.St.agents(st)
  +m = AG.hit(ag, n)
  +p = AG.pre(ag, n)
  +q = AG.post(ag, n)
  AG.set(st, life.live(m, True{}, p, q, ag, S.St.arch(st), S.St.order(st), n, l, why), life.live(m, False{}, p, q, ag, S.St.arch(st), S.St.order(st), n, l, why))

# a new task `a` named `n` (`held`: an archived agent has the name, an
# older journal's reuse: the new one takes its creation place, as
# put_agent replaced it in place before the split)
def new_task.go(held: Bool, side: Bool, +ag: List<&2, S.Agent>, +ar: List<&2, S.Agent>, +ord: List<&2, String>, +n: String, +a: S.Agent) -> List<&2, S.Agent>:
  match held side:
    case False{} True{}:
      put_agent(strip(ag, n), n, a)
    case False{} False{}:
      strip(ar, n)
    case True{} True{}:
      AG.ins(ord, strip(ag, n), a)
    case True{} False{}:
      AG.drop(strip(ar, n), n)

def new_task(+st: S.St, +n: String, +a: S.Agent) -> S.St:
  +c = AG.has(S.St.arch(st), n)
  AG.set(st, new_task.go(c, True{}, S.St.agents(st), S.St.arch(st), S.St.order(st), n, a), new_task.go(c, False{}, S.St.agents(st), S.St.arch(st), S.St.order(st), n, a))

# a rename: the new name out of every old names list, then the agent
def renamed(+st: S.St, +n: String, +nn: String) -> S.St:
  +ag = strip(S.St.agents(st), nn)
  +ar = strip(S.St.arch(st), nn)
  +c = AG.has(ag, n)
  AG.set(st, on_agent.go(c, True{}, ag, ar, n, ORename{nn}), on_agent.go(c, False{}, ag, ar, n, ORename{nn}))

def apply(+st: S.St, e: Ev) -> S.St:
  match e:
    case ETaskCreated{+name, +parent, +brief, +ws, +at}:
      +dirs = dirs_of(List.append(&2, S.Agent, S.St.agents(st), S.St.arch(st)))
      S.St.over_order(new_task(st, name, agent_of_new(ETaskCreated{name, parent, brief, ws, at}, NM.free_dir(dirs, name))),
        +od => Bool.pick(List<&2, String>, str_in(od, name), od, snoc(String, od, name)))
    case ELifecycle{+name, +life, +reason}:
      life_ev(st, name, life, reason)
    case EWorkspace{+name, +ws}:
      on_agent(st, name, OWs{ws})
    case ESnapshot{+name, +snap}:
      on_agent(st, name, OSnap{snap})
    case EPlaced{+name, +ws, +err}:
      placed(String.is_empty(err), st, name, ws)
    case EDeclared{+name, +status, +note}:
      on_agent(st, name, ODeclared{status, note})
    case EReported{+name, +report}:
      on_agent(st, name, OReport{report})
    case EFileTouched{+name, +path}:
      on_agent(st, name, OFile{path})
    case EClaimDropped{+name, +path}:
      on_agent(st, name, OUnclaim{path})
    case ERenamed{+name, +nn}:
      S.St.over_order(renamed(st, name, nn), od => NM.rename_in(od, name, nn))
    case EMsgsMoved{+from, +to}:
      S.St.over_hot(st, h => moved(h, from, to))
    # the three message events, on the split (law cold_is_inert: on
    # every message, the same as one list's snoc, map_msg, settle)
    # (both lists change together: taken out first, so the state no longer
    # holds them and each keeps one owner, like over_f's field)
    case EMsgSent{+m}:
      +nm = Nat.max(Nat.max(S.St.next_msg(st), 1n+S.Msg.id(m)), 1n+S.Msg.thread(m))
      +h = S.St.hot(st)
      +o = S.St.old(st)
      +rest = S.St.with_old(S.St.with_hot(st, Nil{}), Nil{})
      +p = settle_in(S.Msg.reply_to(m), snoc(S.MRec, h, new_rec(m)), o, S.Msg.from(m))
      S.St.with_old(S.St.with_next_msg(S.St.with_hot(rest, Ms.hot(p)), nm), Ms.old(p))
    case EMsgState{+id, +s}:
      +h = S.St.hot(st)
      +o = S.St.old(st)
      +rest = S.St.with_old(S.St.with_hot(st, Nil{}), Nil{})
      +p = msg_op(h, o, id, OSt{s})
      S.St.with_old(S.St.with_hot(rest, Ms.hot(p)), Ms.old(p))
    case ESettled{+id}:
      +h = S.St.hot(st)
      +o = S.St.old(st)
      +rest = S.St.with_old(S.St.with_hot(st, Nil{}), Nil{})
      +p = msg_op(h, o, id, OSettle{})
      S.St.with_old(S.St.with_hot(rest, Ms.hot(p)), Ms.old(p))
    case ECardOpened{+c}:
      S.St.over_next_card(S.St.over_cards(st, cs => put_card(cs, c)), nc => Nat.max(nc, 1n+S.Card.id(c)))
    case ECardClosed{+id, _, _}:
      S.St.over_cards(st, cs => drop_card(cs, id))
    case ECardAnswerPos{_, _, _}:
      st
    case ECardWithdrawn{+id, +text}:
      S.St.over_gone(st, gn => G.add(gn, id, text))
    case ECardLate{+id}:
      S.St.over_gone(st, gn => G.drop(gn, id))
    case EMainNote{+text, _}:
      S.St.over_notes(st, ns => snoc(String, ns, text))
    case ENotesFlushed{}:
      S.St.with_notes(st, Nil{})
    case ETimerSet{+t, _}:
      S.St.over_tm(st, tm => TM.set(tm, t))
    case ETimerFired{+id, +fired, +next, +at, +undelivered}:
      S.St.over_tm(st, tm => TM.fired(tm, id, fired, next, at, undelivered))
    case ETimerRun{+id, +at}:
      S.St.over_tm(st, tm => TM.run(tm, id, at))
    case ETimerStop{+id, +why, +at}:
      S.St.over_tm(st, tm => TM.stop(tm, id, why, at))
    case ETimerName{+id, +name, _, _}:
      S.St.over_tm(st, tm => TM.named(tm, id, name))
    case EX{+xe}:
      S.St.over_x(st, xs => XH.apply(xs, xe))
    case EAct{+ae}:
      S.St.over_tm(st, tm => TM.act_apply(tm, ae))
    case EFollow{+name, +on, +hub}:
      on_agent(st, name, OFollow{Bool.pick(Maybe<&2, String>, on, Some{hub}, None{})})
    case EJobEnd{+name, _}:
      on_agent(st, name, OFollow{None{}})
    case EWake{+we}:
      S.St.over_tm(st, tm => TM.wk_apply(tm, we))
    case EPaused{+at}:
      S.St.with_paused(st, Some{at})
    case EResumed{_}:
      S.St.with_paused(st, None{})

# an event that changes St.order (apply's over_order): a new task, a
# rename. Every other one keeps it (law apply_keeps_order), so a step
# without one leaves the order out of its view (view.bend order_when)
def order_ev(e: Ev) -> Bool:
  match e:
    case ETaskCreated{_, _, _, _, _}:
      True{}
    case ERenamed{_, _}:
      True{}
    case _:
      False{}

# the state after a journal: the replay of a list of events
def apply_all(evs: List<&2, Ev>, +st: S.St) -> S.St:
  match evs:
    case Nil{}:
      st
    case Con{+e, +rest}:
      apply_all(rest, apply(st, e))

def card_of(xs: List<&2, S.Card>, +id: Nat) -> Maybe<&2, S.Card>:
  find(~S.Card, ~Nat, ~S.Card.id, ~(a => b => Nat.is_eq(a, b)), xs, id)

# queued_in's step: `q` first (most are delivered), then the names
def queued_in.keep(q: Bool, +ns: List<&2, String>, +m: S.Msg, +rest: List<&2, S.Msg>) -> List<&2, S.Msg>:
  match q:
    case True{}:
      Bool.pick(List<&2, S.Msg>, str_in(ns, S.Msg.to(m)), m <> rest, rest)
    case False{}:
      rest

def queued_in(xs: List<&2, S.MRec>, +ns: List<&2, String>) -> List<&2, S.Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      queued_in.keep(is_queued(S.MRec.st(r)), ns, S.MRec.msg(r), queued_in(t, ns))

def names_of.go(m: Maybe<&2, S.Agent>) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{+a}:
      S.Agent.aliases(a)

# an agent's names: its own and its old ones (until another agent takes one)
def names_of(+st: S.St, +n: String) -> List<&2, String>:
  n <> names_of.go(get_agent(st, n))

# messages waiting for delivery to `n` (under any of its names), oldest first
def queued_for(+st: S.St, +n: String) -> List<&2, S.Msg>:
  queued_in(S.St.hot(st), names_of(st, n))

def unanswered_in(xs: List<&2, S.MRec>, +ns: List<&2, String>) -> List<&2, S.Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      +rest = unanswered_in(t, ns)
      Bool.pick(List<&2, S.Msg>, str_in(ns, S.Msg.to(S.MRec.msg(r))) && MRec.owes(r), S.MRec.msg(r) <> rest, rest)

# delivered messages to `n` that expect a reply it has not given
def unanswered_for(+st: S.St, +n: String) -> List<&2, S.Msg>:
  unanswered_in(S.St.hot(st), names_of(st, n))

def msg_of.go(m: Maybe<&2, S.MRec>) -> Maybe<&2, S.Msg>:
  match m:
    case None{}:
      None{}
    case Some{+r}:
      Some{S.MRec.msg(r)}

def msg_of(+st: S.St, +id: Nat) -> Maybe<&2, S.Msg>:
  msg_of.go(rec_at(st, id))

def state_of.go(m: Maybe<&2, S.MRec>) -> Maybe<&2, S.MSt>:
  match m:
    case None{}:
      None{}
    case Some{+r}:
      Some{S.MRec.st(r)}

def state_of(+st: S.St, +id: Nat) -> Maybe<&2, S.MSt>:
  state_of.go(rec_at(st, id))

def ws_worktree(+ws: J.Json) -> Bool:
  String.eq(J.get_str_or(ws, "mode", "shared"), "worktree")

def ws_dropped(+ws: J.Json) -> Bool:
  J.get_bool_or(ws, "dropped", False{})

# hub-loop: a worktree task whose worktree the shell is still making
def ws_placing(+ws: J.Json) -> Bool:
  String.eq(J.get_str_or(ws, "state", ""), "placing")

def ws_branch(+ws: J.Json) -> J.Json:
  Maybe.default(&2, J.Json, J.get(ws, "branch"), J.JNull{})

def ws_path(+ws: J.Json) -> String:
  J.get_str_or(ws, "path", "")

def init(+workspace: String) -> S.St:
  +ws = J.obj([J.kv("mode", J.str("shared")), J.kv("path", J.str(workspace)),
    J.kv("branch", J.JNull{}), J.kv("base_commit", J.JNull{}), J.kv("dropped", J.JBool{False{}})])
  +brief = J.obj([J.kv("objective", J.str("")), J.kv("context", J.str("")), J.kv("constraints", J.JArr{Nil{}}),
    J.kv("done_when", J.JNull{}), J.kv("report_format", J.JNull{})])
  +main = S.Agent.with_parent(S.Agent.with_is_main(agent_of_new(ETaskCreated{MAIN(), "", brief, ws, 0n}, MAIN()), True{}), None{})
  S.St.with_order(S.St.with_agents(S.St.empty(), [main]), [MAIN()])

# bise desktop S10: someone asked to hear when it ends (one job: off again
# at its job_end)
def follows(x: S.Agent) -> Bool:
  Maybe.is_some(&2, String, S.Agent.follow(x))

# J: the hub its end goes back to ("" none: not followed, or followed by
# this hub's own main or window)
def follow_hub(x: S.Agent) -> String:
  Maybe.default(&2, String, S.Agent.follow(x), "")
