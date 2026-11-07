# hub/core.bend — the hub's decisions (RFC 0001, RFC 0003): a pure step
# from (state, input) to (state, effects). Ported from
# rust/switchboard/src/core.rs; the Rust daemon runs the effects. The
# input is decoded and the effects encoded by gen/ (bise_proto::core).
import Base

import ../vendor/json.bend as J

import ./model.bend as M
import ./gen/state.bend as S

import ./cx.bend as X
import ./text.bend as T
import ./cold.bend as CO
import ./newtask.bend as NT

import ./state-hand.bend as SH
import ./timers.bend as TM
import ./wakes.bend as WK

import ./xhub-model.bend as XH
import ./xhub.bend as XB
import ./gen/state-codec.bend as SC

import ./picks.bend as PK
import ./answers.bend as A

# the seam, generated from bise_proto::core (issue 25): the inputs'
# decode, the effects' encoders
import ./gen/in.bend as GI

import ./gen/fx.bend as F

import ./gen/kit.bend as K

import ./pause.bend as P
import ./reach.bend as R
import ./gen/feed.bend as FD
import ./keyed.bend as KL
import ./feature.bend as FT
import ./status.bend as ST
import ./lazy.bend as LZ
import ./place.bend as PL
import ./cardkind.bend as CK
import ./gone.bend as G
import ./take.bend as TK
import ./peer.bend as PR
import ./stall.bend as SL
import ./forward.bend as FW
import ./viewctx.bend as VC

type Snd is Data:
  Snd{from: String, to: String, text: String, expect: Bool,
    reply_to: Maybe<&2, Nat>, plain: Bool, auto: Bool, queued: Bool,
    via: Maybe<&2, String>, parts: String, fwd: Maybe<&2, Nat>}

type SendR is Data:
  SOk{m: S.Msg}
  SErr{e: String}

type TaskR is Data:
  TOk{name: String}
  TErr{e: String}
  THalt{}

type DropR is Data:
  DDone{}
  DAsk{q: String}
  DErr{e: String}
  DHalt{}

type NewTask is Data:
  NewTask{parent: String, base: String, brief: J.Json, brief_text: String,
    objective: String, worktree: Bool, with_changes: Bool, parts: String,
    fwd: Maybe<&2, Nat>}

type Hub is Data:
  Hub{d: S.St, r: X.R, fresh: Bool}

type CardK is Data:
  KQuestion{}
  KDrop{}
  KConfirm{}
  KChoice{}
  KOther{}

type Cmd is Data:
  CSend{}
  CWait{}
  CAsk{}
  CStatus{}
  CReport{}
  CFollow{}
  CSpawn{}
  CInterrupt{}
  CStop{}
  CDrop{}
  CCard{}
  CClose{}
  CRename{}
  CRestore{}
  CIsolate{}
  CMove{}
  CWithdraw{}
  CProjectSend{}
  CProjectAsk{}

# the hub after an input: the new state and its effects, or, when a git
# answer is missing, the hub unchanged and the query (the daemon replays
# the input with the answer)
type Turn is Data:
  Turn{hub: Hub, fx: List<&2, X.Fx>, need: Maybe<&2, J.Json>}

# a sequence of inputs: the hub at the end and every journal event, in
# order (the model of sb-core's loop, for the law run_replays)
type RunR is Data:
  RunR{hub: Hub, journal: List<&2, M.Ev>}

def WAIT_CAP() -> Nat:
  25n

def MAX_CRASHES() -> Nat:
  5n

def ok_body(+kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> J.Json:
  J.obj(M.snoc(Sigma<&2, &2, String, _ => J.Json>, kvs, J.kv("ok", J.JBool{True{}})))

def err_body(+e: String) -> J.Json:
  J.obj([J.kv("ok", J.JBool{False{}}), J.kv("error", J.JStr{e})])

def ns(+n: Nat) -> String:
  Nat.show(n)

def mid(+n: Nat) -> String:
  String.append("m_", Nat.show(n))

def cat(+xs: List<&2, String>) -> String:
  String.concat(xs)

def run_str(r: S.Run) -> String:
  match r:
    case S.RDown{}:
      "down"
    case S.RStarting{}:
      "starting"
    case S.RIdle{}:
      "idle"
    case S.RBusy{}:
      "busy"

def is_busy(r: S.Run) -> Bool:
  match r:
    case S.RBusy{}:
      True{}
    case _:
      False{}

def is_idle(r: S.Run) -> Bool:
  match r:
    case S.RIdle{}:
      True{}
    case _:
      False{}

def is_down(r: S.Run) -> Bool:
  match r:
    case S.RDown{}:
      True{}
    case _:
      False{}

# issue 22: an agent's ended turns after its run goes from `was` to
# `now`: one more when it leaves RBusy (to idle, down or starting), the
# same otherwise. set_rt, the one writer of a runtime state, applies it,
# so a turn that ends and a next one that starts in the same step (the
# idle's pump) still counts.
def turns_after(was: S.Run, now: S.Run, +n: Nat) -> Nat:
  match was:
    case S.RBusy{}:
      match now:
        case S.RBusy{}:
          n
        case _:
          1n+n
    case _:
      n

def is_active(l: S.Life) -> Bool:
  match l:
    case S.LActive{}:
      True{}
    case _:
      False{}

# ---- the runtime lists ----

# an agent's runtime, read by its name only: the order of the runtimes
# is never observable (laws rt_put_finds_it, rt_put_keeps_others)
def rt_find(xs: List<&2, X.Rt>, +n: String) -> X.Rt:
  match xs:
    case Nil{}:
      X.Rt{n, S.RDown{}, False{}, None{}, 0n}
    case Con{+h, +t}:
      LZ.pick(X.Rt, String.eq(X.Rt.name(h), n), u => h, u => rt_find(t, n))

# the runtimes without the one named `n` (at most one: rt_put's)
def rt_del(xs: List<&2, X.Rt>, +n: String) -> List<&2, X.Rt>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      LZ.pick(List<&2, X.Rt>, String.eq(X.Rt.name(h), n), u => t, u => h <> rt_del(t, n))

# `x` at the head: the runtimes a step sets are the live agents', so a
# live agent's is found in a few steps and an archived agent's, set last
# when it stopped, sinks behind them (c4b-core-step-flat: his hub keeps
# ~900 runtimes of archived agents)
def rt_put(xs: List<&2, X.Rt>, +x: X.Rt) -> List<&2, X.Rt>:
  x <> rt_del(xs, X.Rt.name(x))

# (the keyed lists' helpers, `KL.`: hub/keyed.bend)

def ids_of(xs: List<&2, S.Msg>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      S.Msg.id(h) <> ids_of(t)

# ---- context shortcuts ----

def st(-o: S.St, +cx: X.Cx<o>) -> S.St:
  X.d(o, cx)

def rr(-o: S.St, +cx: X.Cx<o>) -> X.R:
  X.Cx.r(o, cx)

def set_r(-o: S.St, +cx: X.Cx<o>, +r: X.R) -> X.Cx<o>:
  X.with_r(o, cx, r)

def out(-o: S.St, +cx: X.Cx<o>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> X.Cx<o>:
  X.out(o, cx, J.obj(kvs))

# one record of an agent's feed (FD: bise_proto::core::Feed's encoders);
# the hub writes it as the line 'sb <kind> :: <json>'
def feed(-o: S.St, +cx: X.Cx<o>, +agent: String, +rec: J.Json) -> X.Cx<o>:
  out(o, cx, F.feed(agent, rec))

def parts_json.of(r: Result<&2, &2, String, J.Json>) -> J.Json:
  match r:
    case Done{+j}:
      j
    case Fail{_}:
      J.JNull{}

# his message's parts (msg-parts JSON, kept as a string on the message);
# unreadable: null (its readers drop it)
def parts_json(+s: String) -> J.Json:
  parts_json.of(J.parse(s))

# his message's parts after his you-id (msg-parts); none when he sent
# none, so those lines are what they always were (law no_parts_no_line)
def you_parts(-o: S.St, +cx: X.Cx<o>, +agent: String, +parts: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.is_empty(parts), u => cx, u => feed(o, cx, agent, FD.you_parts(parts_json(parts))))

# an agent's message parts (send-parts: `sb send --attach`) in `agent`'s
# feed, after its msg-in (`sent`: after its sent line), by its id; none
# when it has none (law agent_no_parts_no_line)
def msg_parts(-o: S.St, +cx: X.Cx<o>, +agent: String, +m: S.Msg) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.is_empty(S.Msg.parts(m)), u => cx, u => feed(o, cx, agent, FD.msg_parts(S.Msg.id(m), parts_json(S.Msg.parts(m)))))

def sent_parts(-o: S.St, +cx: X.Cx<o>, +agent: String, +m: S.Msg) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.is_empty(S.Msg.parts(m)), u => cx, u => feed(o, cx, agent, FD.sent_parts(S.Msg.id(m), parts_json(S.Msg.parts(m)))))

# a route's parts after it, likewise
def route_parts.at(-o: S.St, +cx: X.Cx<o>, +at: String, +parts: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.is_empty(parts), u => cx, u => feed(o, cx, at, FD.route_parts(parts_json(parts))))

def route_parts(-o: S.St, +cx: X.Cx<o>, +parts: String) -> X.Cx<o>:
  route_parts.at(o, cx, M.MAIN(), parts)

# his route in main's feed (his answer to `who`'s card, or his words to
# `who`), then its parts
def route_line(-o: S.St, +cx: X.Cx<o>, +who: String, +card: Maybe<&2, Nat>, +said: String, +parts: String) -> X.Cx<o>:
  route_parts(o, feed(o, cx, M.MAIN(), FD.route(who, card, said, None{}, None{}, None{})), parts)


def reply(-o: S.St, +cx: X.Cx<o>, +token: Nat, +body: J.Json) -> X.Cx<o>:
  out(o, cx, F.reply(token, body))

def reply_err(-o: S.St, +cx: X.Cx<o>, +token: Nat, +e: String) -> X.Cx<o>:
  reply(o, cx, token, err_body(e))

def notice(-o: S.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  out(o, cx, F.notice(text))

def rt(-o: S.St, +cx: X.Cx<o>, +n: String) -> X.Rt:
  rt_find(X.R.rts(rr(o, cx)), n)

# the runtime state `x` as set_rt stores it over `was`, the agent's
# runtime before: its ended turns are was's, one more when the run
# leaves RBusy (turns_after); x's own count is never read
def rt_next(+was: X.Rt, +x: X.Rt) -> X.Rt:
  X.Rt.with_turns(x, turns_after(X.Rt.run(was), X.Rt.run(x), X.Rt.turns(was)))

# set the runtime state of an agent (the Rust mirror follows): every
# runtime change goes through here, so its ended turns count here
def set_rt(-o: S.St, +cx: X.Cx<o>, +x0: X.Rt) -> X.Cx<o>:
  +x = rt_next(rt(o, cx, X.Rt.name(x0)), x0)
  +cx2 = set_r(o, cx, X.R.with_rts(rr(o, cx), rt_put(X.R.rts(rr(o, cx)), x)))
  out(o, cx2, F.rt(X.Rt.name(x), run_str(X.Rt.run(x)), X.Rt.waiting(x), X.Rt.turn(x), X.Rt.turns(x)))

def set_run(-o: S.St, +cx: X.Cx<o>, +n: String, +run: S.Run) -> X.Cx<o>:
  set_rt(o, cx, X.Rt.with_run(rt(o, cx, n), run))

def has_agent(-o: S.St, +cx: X.Cx<o>, +n: String) -> Bool:
  Maybe.is_some(&2, S.Agent, M.get_agent(st(o, cx), n))

# the runtime change applies only to a known agent (like the Rust
# `if let Some(a) = self.st.agents.get_mut(..)`)
def if_agent(-o: S.St, known: Bool, +cx: X.Cx<o>, +x: X.Rt) -> X.Cx<o>:
  match known:
    case True{}:
      set_rt(o, cx, x)
    case False{}:
      cx

def upd_rt(-o: S.St, +cx: X.Cx<o>, +x: X.Rt) -> X.Cx<o>:
  if_agent(o, has_agent(o, cx, X.Rt.name(x)), cx, x)

def dirty(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  X.dirty(o, cx)

def now(-o: S.St, +cx: X.Cx<o>) -> Nat:
  X.now(o, cx)

# every event keeps the journal's invariants: a caller that is not sending
# a message proves them with `Unit{}, Unit{}`
def emit(-o: S.St, +cx: X.Cx<o>, +e: M.Ev, k: X.FreshEv(X.d(o, cx), e), kq: X.QuietEv(e), kv: X.DelivEv(X.d(o, cx), e)) -> X.Cx<o>:
  X.emit(o, cx, e, k, kq, kv)

# ---- env queries (replayed answers, see hub/main.bend) ----

def ask.go(-o: S.St, ans: List<&2, J.Json>, +cx: X.Cx<o>, +q: J.Json) -> X.Cx<o> & Maybe<&2, J.Json>:
  match ans:
    case Con{+a, +rest}:
      (set_r(o, cx, X.R.with_ans(rr(o, cx), rest)), Some{a})
    case Nil{}:
      (set_r(o, cx, X.R.with_need(rr(o, cx), Maybe.or(&2, J.Json, X.R.need(rr(o, cx)), Some{q}))), None{})

# the answer to an env query, or None: the step halts and the daemon
# replays the input with the answer
def ask(-o: S.St, +cx: X.Cx<o>, +q: J.Json) -> X.Cx<o> & Maybe<&2, J.Json>:
  ask.go(o, X.R.ans(rr(o, cx)), cx, q)

# ---- notes and cards ----

def note_main(-o: S.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  emit(o, cx, M.EMainNote{text, now(o, cx)}, Unit{}, Unit{}, Unit{})

# opens card number next_card (the caller reads it first when it needs it)
# in the user's inbox: `kind` is a user kind (CK.user_kind), never an
# agent's traffic (BISE-299)
#
# open_card_at: the same, about a place (and its PR): the hub's own items
#
# open_card_full: the same, with what it is `about` (the shell's ArtLink:
# a page at its block, an artifact), kept on the card as is and never
# read here (inbox-answered, architect m_55388; tests/state_guard.py)
def card_new(+id: Nat, +kind: String, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>, +created: Nat, +place: Maybe<&2, String>, +pr: Maybe<&2, Nat>, +about: Maybe<&2, J.Json>) -> S.Card:
  S.Card.with_about(S.Card.with_pr(S.Card.with_place(S.Card.with_created(S.Card.with_for_msg(S.Card.with_text(S.Card.with_agent(S.Card.with_kind(S.Card.with_id(S.Card.empty(), id), kind), agent), text), for_msg), created), place), pr), about)

def open_card_full(-o: S.St, +cx: X.Cx<o>, +kind: String, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>, +place: Maybe<&2, String>, +pr: Maybe<&2, Nat>, +about: Maybe<&2, J.Json>) -> X.Cx<o>:
  +id = S.St.next_card(st(o, cx))
  +cx2 = emit(o, cx, M.ECardOpened{card_new(id, kind, agent, text, for_msg, now(o, cx), place, pr, about)}, {==}, Unit{}, Unit{})
  feed(o, cx2, M.MAIN(), FD.card(id, kind, agent, text))

def open_card_at(-o: S.St, +cx: X.Cx<o>, +kind: String, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>, +place: Maybe<&2, String>, +pr: Maybe<&2, Nat>) -> X.Cx<o>:
  open_card_full(o, cx, kind, agent, text, for_msg, place, pr, None{})

def open_card(-o: S.St, +cx: X.Cx<o>, +kind: String, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>) -> X.Cx<o>:
  open_card_at(o, cx, kind, agent, text, for_msg, None{}, None{})

# open_card about an artifact (`sb notify --page|--artifact`, a page's own
# card)
def open_card_about(-o: S.St, +cx: X.Cx<o>, +kind: String, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>, +about: Maybe<&2, J.Json>) -> X.Cx<o>:
  open_card_full(o, cx, kind, agent, text, for_msg, None{}, None{}, about)

# the hub's Rust side opens one of its items (`card_open`: a PR ready to
# merge, pr-design §6.3); a kind outside the user's inbox opens nothing
# (BISE-299)
def hub_card(-o: S.St, +cx: X.Cx<o>, +kind: String, +agent: String, +text: String, +place: Maybe<&2, String>, +pr: Maybe<&2, Nat>) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, CK.choice_kind(kind),
    u => open_card_at(o, cx, kind, agent, text, None{}, place, pr),
    u => notice(o, cx, String.append("not a hub item: ", kind)))

# BISE-299, the hook for tool-call confirmations: a tool call that needs
# the user opens a `confirm` card for `agent` straight in the user's
# inbox, whoever runs it (the approvals gate, approvals-design.md §9);
# the user's answer goes to the gate (answer.kind: a `confirm` effect).
def open_confirm(-o: S.St, +cx: X.Cx<o>, +agent: String, +text: String) -> X.Cx<o>:
  open_card(o, cx, "confirm", agent, text, None{})

def close_ids(-o: S.St, ids: List<&2, Nat>, +cx: X.Cx<o>, +res: String) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      +cx2 = emit(o, cx, M.ECardClosed{id, res, S.Ans.empty()}, Unit{}, Unit{}, Unit{})
      close_ids(o, rest, feed(o, cx2, M.MAIN(), FD.card_closed(id, res)), res)

# the cards of `agent` whose kind is in `kinds` (Nil: every kind)
def card_ids(cs: List<&2, S.Card>, +agent: String, +kinds: List<&2, String>) -> List<&2, Nat>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      +rest = card_ids(t, agent, kinds)
      +hit = String.eq(S.Card.agent(c), agent) && (List.is_empty(&2, String, kinds) || M.str_in(kinds, S.Card.kind(c)))
      Bool.pick(List<&2, Nat>, hit, S.Card.id(c) <> rest, rest)

def close_cards(-o: S.St, +cx: X.Cx<o>, +agent: String, +kinds: List<&2, String>, +res: String) -> X.Cx<o>:
  close_ids(o, card_ids(S.St.cards(st(o, cx)), agent, kinds), cx, res)

# ---- REPL lifecycle ----

def spawn.if(-o: S.St, down: Bool, +cx: X.Cx<o>, +n: String, +resume: Bool) -> X.Cx<o>:
  match down:
    case True{}:
      +cx2 = set_run(o, cx, n, S.RStarting{})
      dirty(o, out(o, cx2, F.spawn(n, resume, None{}, False{})))
    case False{}:
      cx

def spawn(-o: S.St, +cx: X.Cx<o>, +n: String, +resume: Bool) -> X.Cx<o>:
  spawn.if(o, Bool.not(PL.waits(M.get_agent(st(o, cx), n))) && has_agent(o, cx, n) && is_down(X.Rt.run(rt(o, cx, n))), cx, n, resume)

# ---- waiters ----

# a wait gets its message (the message is not marked delivered here)
def waiter_reply(-o: S.St, +cx: X.Cx<o>, +w: X.Waiter, +m: S.Msg, +kind: String) -> X.Cx<o>:
  +cx3 = reply(o, cx, X.Waiter.token(w), ok_body([
    J.kv("type", J.JStr{kind}), J.kv("from", J.JStr{S.Msg.from(m)}),
    J.kv("message_id", J.JStr{mid(S.Msg.id(m))}),
    J.kv("thread", J.JStr{String.append("t_", ns(S.Msg.thread(m)))}),
    J.kv("expects_reply", J.JBool{S.Msg.expect(m)}), J.kv("auto", J.JBool{S.Msg.auto(m)}),
    J.kv("asked", J.JStr{mid(X.Waiter.msg(w))}),
    J.kv("message", J.JStr{S.Msg.text(m)})]))
  +cx4 = msg_parts(o, feed(o, cx3, X.Waiter.agent(w), FD.msg_in(S.Msg.from(m), Some{S.Msg.id(m)}, S.Msg.text(m), S.Msg.view(m))), X.Waiter.agent(w), m)
  +still = X.any_waiter(X.R.waiters(rr(o, cx4)), X.Waiter.agent(w))
  +x = X.Rt.with_waiting(rt(o, cx4, X.Waiter.agent(w)), still)
  dirty(o, upd_rt(o, cx4, x))

def mark_delivered.go(-o: S.St, +cx: X.Cx<o>, +id: Nat, c: Bool, +e: {X.queued_at(X.d(o, cx), id) == c : Bool}) -> X.Cx<o>:
  match c:
    case True{}:
      emit(o, cx, M.EMsgState{id, S.Delivered{}}, Unit{}, Unit{}, e)
    case False{}:
      cx

# a message becomes delivered only while it is queued (law
# delivered_once): the check never fails in a state the hub reached
def mark_delivered(-o: S.St, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  mark_delivered.go(o, cx, id, X.queued_at(X.d(o, cx), id), {==})

def deliver_to_waiter(-o: S.St, +cx: X.Cx<o>, +w: X.Waiter, +m: S.Msg, +kind: String) -> X.Cx<o>:
  waiter_reply(o, mark_delivered(o, cx, S.Msg.id(m)), w, m, kind)

def reply_all(-o: S.St, ws: List<&2, X.Waiter>, +cx: X.Cx<o>, +why: String) -> X.Cx<o>:
  match ws:
    case Nil{}:
      cx
    case Con{+w, +rest}:
      reply_all(o, rest, reply_err(o, cx, X.Waiter.token(w), why), why)

def fail_waiters(-o: S.St, +cx: X.Cx<o>, +agent: String, +why: String) -> X.Cx<o>:
  +ws = X.R.waiters(rr(o, cx))
  +cx2 = set_r(o, cx, X.R.with_waiters(rr(o, cx), X.waiters_not(ws, agent)))
  +cx3 = reply_all(o, X.waiters_of(ws, agent), cx2, why)
  upd_rt(o, cx3, X.Rt.with_waiting(rt(o, cx3, agent), False{}))

# ---- delivery (RFC 0003 §6.1, RFC 0001 §10.1) ----

def not_queued_mode(xs: List<&2, S.Msg>) -> List<&2, S.Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = not_queued_mode(t)
      Bool.pick(List<&2, S.Msg>, S.Msg.queued(h), rest, h <> rest)

def leftover_at.go(m: Maybe<&2, S.MSt>) -> Bool:
  match m:
    case Some{S.Queued{+r}}:
      String.eq(r, "steer_leftover")
    case _:
      False{}

# a steer_leftover was steered in a turn that never read it: its feed line
# was written then (qa-explore bug A)
def leftover_at(+s: S.St, +id: Nat) -> Bool:
  leftover_at.go(M.state_of(s, id))

# the feed line of a delivered message: once, when it is first handed over
def deliver.line(-o: S.St, again: Bool, you: Bool, +cx: X.Cx<o>, +name: String, +m: S.Msg) -> X.Cx<o>:
  match again you:
    case True{} _:
      cx
    # his line, then its message id on the next one (the feed only: the
    # clients key his entry by it, the steered receipt names it), then
    # his parts when he sent some (msg-parts: written at the delivery, so
    # a queued or steered message gets them when it is said)
    case False{} True{}:
      +cx2 = feed(o, feed(o, cx, name, FD.you(S.Msg.text(m), S.Msg.via(m))), name, FD.you_id(S.Msg.id(m)))
      you_parts(o, cx2, name, S.Msg.parts(m))
    case False{} False{}:
      msg_parts(o, feed(o, cx, name, FD.msg_in(S.Msg.from(m), Some{S.Msg.id(m)}, S.Msg.text(m), S.Msg.view(m))), name, m)

def deliver_each(-o: S.St, batch: List<&2, S.Msg>, +cx: X.Cx<o>, +name: String, +acc: List<&2, K.Delivered>) -> X.Cx<o> & List<&2, K.Delivered>:
  match batch:
    case Nil{}:
      (cx, acc)
    case Con{+m, +rest}:
      +s = st(o, cx)
      +rel = ST.relation(S.Msg.from(m), S.Msg.to(m), ST.parent_of(M.get_agent(s, S.Msg.from(m))), ST.parent_of(M.get_agent(s, S.Msg.to(m))))
      +cx2 = mark_delivered(o, cx, S.Msg.id(m))
      +cx3 = deliver.line(o, leftover_at(s, S.Msg.id(m)), S.Msg.plain(m) || Maybe.is_some(&2, String, S.Msg.via(m)), cx2, name, m)
      deliver_each(o, rest, cx3, name, M.snoc(K.Delivered, acc, K.Delivered{S.Msg.id(m), rel}))

def pump.flush(-o: S.St, flush: Bool, +cx: X.Cx<o>) -> X.Cx<o>:
  match flush:
    case True{}:
      emit(o, cx, M.ENotesFlushed{}, Unit{}, Unit{}, Unit{})
    case False{}:
      cx

# the delivery (`notes`: main's, `status`: the task status block)
def pump.send.go(-o: S.St, busy: Bool, +cx: X.Cx<o>, +name: String, +ids: List<&2, Nat>, +notes: List<&2, String>, +status: Bool, +msgs: List<&2, K.Delivered>) -> X.Cx<o>:
  match busy:
    case True{}:
      +r = rr(o, cx)
      +cx2 = set_r(o, cx, X.R.with_steered(r, KL.steer_add(X.R.steered(r), name, ids)))
      dirty(o, out(o, cx2, F.deliver("steer", name, notes, status, msgs)))
    case False{}:
      # the turn starts now: nothing else may `say` meanwhile
      +cx2 = upd_rt(o, cx, X.Rt{name, S.RBusy{}, X.Rt.waiting(rt(o, cx, name)), Some{now(o, cx)}, 0n})
      dirty(o, out(o, cx2, F.deliver("say", name, notes, status, msgs)))

def pump.send(-o: S.St, busy: Bool, p: X.Cx<o> & List<&2, K.Delivered>, +name: String, +ids: List<&2, Nat>, +notes: List<&2, String>, +status: Bool) -> X.Cx<o>:
  match p:
    case (+cx, +msgs):
      pump.send.go(o, busy, cx, name, ids, notes, status, msgs)

def jstrs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.JStr{h} <> jstrs(t)

def pump.batch(-o: S.St, batch: List<&2, S.Msg>, +cx: X.Cx<o>, +name: String, +busy: Bool, +is_main: Bool) -> X.Cx<o>:
  match batch:
    case Nil{}:
      cx
    case Con{+h, +t}:
      +b = {h <> t : List<&2, S.Msg>}
      +notes = Bool.pick(List<&2, String>, is_main, S.St.notes(st(o, cx)), Nil{})
      +cx2 = pump.flush(o, Bool.not(List.is_empty(&2, String, notes)), cx)
      +status = is_main && M.any_from_user(b)
      pump.send(o, busy, deliver_each(o, b, cx2, name, Nil{}), name, ids_of(b), notes, status)

def pump.run(-o: S.St, run: S.Run, +cx: X.Cx<o>, +name: String, +queued: List<&2, S.Msg>, +is_main: Bool) -> X.Cx<o>:
  match run:
    case S.RDown{}:
      cx
    case S.RStarting{}:
      cx
    case S.RBusy{}:
      # a queued-mode message waits for the end of the turn
      pump.batch(o, not_queued_mode(queued), cx, name, True{}, is_main)
    case S.RIdle{}:
      pump.batch(o, queued, cx, name, False{}, is_main)

def pump.active(-o: S.St, active: Bool, +cx: X.Cx<o>, +name: String, +is_main: Bool) -> X.Cx<o>:
  match active:
    case True{}:
      pump.run(o, X.Rt.run(rt(o, cx, name)), cx, name, M.queued_for(st(o, cx), name), is_main)
    case False{}:
      cx

def pump.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +name: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      pump.active(o, is_active(S.Agent.life(a)), cx, name, S.Agent.is_main(a))

def pump.paused(-o: S.St, paused: Bool, +cx: X.Cx<o>, +name: String) -> X.Cx<o>:
  match paused:
    case True{}:
      cx
    case False{}:
      pump.agent(o, M.get_agent(st(o, cx), name), cx, name)

# deliver what is queued for `name`, when its state allows it; a paused
# project delivers nothing (law paused_delivers_nothing): it stays queued
def pump(-o: S.St, +cx: X.Cx<o>, +name: String) -> X.Cx<o>:
  pump.paused(o, P.is_paused(st(o, cx)), cx, name)

# ---- the watches' journal, before a send uses it (ask-wait) ----
def tms(-o: S.St, +cx: X.Cx<o>) -> S.Tms:
  S.St.tm(st(o, cx))

def wks(-o: S.St, +cx: X.Cx<o>) -> S.Wakes:
  S.Tms.wk(tms(o, cx))

def wake_end(-o: S.St, +cx: X.Cx<o>, +id: Nat, +why: String) -> X.Cx<o>:
  emit(o, cx, M.EWake{WK.WEnd{id, why, now(o, cx)}}, Unit{}, Unit{}, Unit{})

def wake_ends(-o: S.St, ids: List<&2, Nat>, +cx: X.Cx<o>, +why: String) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      wake_ends(o, rest, wake_end(o, cx, id, why), why)

# a message sent: the reply watches it answers end, in the same step, with
# no wake (law ask_reply_ends_quiet)
def reply_answered(-o: S.St, +cx: X.Cx<o>, +from: String, +to: String, reply_to: Maybe<&2, Nat>) -> X.Cx<o>:
  match reply_to:
    case None{}:
      cx
    case Some{+r}:
      wake_ends(o, WK.answered_of(S.Wakes.live(wks(o, cx)), to, r, M.names_of(st(o, cx), from), String.eq(from, M.USER())), cx, "answered")

# ---- messages (RFC 0003) ----

def serr(-o: S.St, +cx: X.Cx<o>, +e: String) -> X.Cx<o> & SendR:
  (cx, SErr{e})

def send.waiter(-o: S.St, mw: Maybe<&2, X.Waiter>, +cx: X.Cx<o>, +m: S.Msg) -> X.Cx<o> & SendR:
  match mw:
    case Some{+w}:
      # a waiting recipient gets it as the result of its `sb wait`
      +r = rr(o, cx)
      +cx2 = set_r(o, cx, X.R.with_waiters(r, X.waiter_drop(X.R.waiters(r), X.Waiter.token(w))))
      +kind = Bool.pick(String, M.maybe_nat_is(S.Msg.reply_to(m), X.Waiter.msg(w)), "reply", "incoming_request")
      (deliver_to_waiter(o, cx2, w, m, kind), SOk{m})
    case None{}:
      (pump(o, cx, S.Msg.to(m)), SOk{m})

def quiet_ok(a: Bool, e: Bool) -> {Bool.and(a, Bool.and(e, Bool.not(a))) == False{} : Bool}:
  match a e:
    case True{} True{}:
      {==}
    case True{} False{}:
      {==}
    case False{} _:
      {==}

# a send's message, by name from its Snd: an automatic reply never asks (law auto_never_asks);
# the id is the next free one (law msg_ids_fresh); his parts kept as sent (parts_never_change_text)
def msg_of_snd(+s: Snd, +id: Nat, +thread: Nat, +now: Nat) -> S.Msg:
  match s:
    case Snd{+from, +to, +text, +expect, +reply_to, +plain, +auto, +queued, +via, +parts, +fwd}:
      +m = S.Msg.with_reply_to(S.Msg.with_to(S.Msg.with_from(S.Msg.with_thread(S.Msg.with_id(S.Msg.empty(), id), thread), from), to), reply_to)
      +m2 = S.Msg.with_created(S.Msg.with_text(S.Msg.with_auto(S.Msg.with_expect(m, Bool.and(expect, Bool.not(auto))), auto), text), now)
      S.Msg.with_fwd(S.Msg.with_parts(S.Msg.with_via(S.Msg.with_queued(S.Msg.with_plain(m2, plain), queued), via), parts), fwd)

def send.emit(-o: S.St, +cx: X.Cx<o>, +s: Snd, +thread: Nat) -> X.Cx<o> & SendR:
  match s:
    case Snd{+from, +to, +text, +expect, +reply_to, +plain, +auto, +queued, +via, +parts, _}:
      +m = msg_of_snd(s, S.St.next_msg(st(o, cx)), thread, now(o, cx))
      # a card is closed by the user's answer only (answer_card): no reply
      # here touches the user's inbox (BISE-299)
      +cx1 = emit(o, cx, M.EMsgSent{m}, {==}, quiet_ok(auto, expect), Unit{})
      # the reply watch it answers ends now (ask-wait), whoever gets it
      +cx2 = reply_answered(o, cx1, from, to, reply_to)
      # paused: no waiter gets it either (it would wake an agent): queued
      +w = Bool.pick(Maybe<&2, X.Waiter>, queued || P.is_paused(st(o, cx2)), None{},
        X.waiter_pick(X.R.waiters(rr(o, cx2)), to, reply_to, expect && Bool.not(plain) && Bool.not(String.eq(from, M.USER()))))
      send.waiter(o, w, cx2, m)

def send.orig(-o: S.St, m: Maybe<&2, S.Msg>, +cx: X.Cx<o>, +s: Snd, +r: Nat) -> X.Cx<o> & SendR:
  match m:
    case None{}:
      serr(o, cx, String.append("reply_to inconnu : ", mid(r)))
    case Some{+orig}:
      send.emit(o, cx, s, S.Msg.thread(orig))

def send.thread(-o: S.St, reply_to: Maybe<&2, Nat>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match reply_to:
    case None{}:
      send.emit(o, cx, s, S.St.next_msg(st(o, cx)))
    case Some{+r}:
      send.orig(o, M.msg_of(st(o, cx), r), cx, s, r)

def Snd.to(x: Snd) -> String:
  match x:
    case Snd{_, +to, _, _, _, _, _, _, _, _, _}:
      to

def Snd.reply_to(x: Snd) -> Maybe<&2, Nat>:
  match x:
    case Snd{_, _, _, _, +reply_to, _, _, _, _, _, _}:
      reply_to

def send.revive(-o: S.St, ok: Bool, +cx: X.Cx<o>, +s: Snd, +why: String) -> X.Cx<o> & SendR:
  match ok:
    case False{}:
      serr(o, cx, why)
    case True{}:
      +to = Snd.to(s)
      +cx2 = emit(o, cx, M.ELifecycle{to, S.LActive{}, None{}}, Unit{}, Unit{}, Unit{})
      +r = rr(o, cx2)
      +cx3 = set_r(o, cx2, X.R.with_crashes(r, KL.kn_del(X.R.crashes(r), to)))
      send.thread(o, Snd.reply_to(s), spawn(o, cx3, to, True{}), s)

def dropped_wt(+a: S.Agent) -> Bool:
  M.ws_worktree(S.Agent.ws(a)) && M.ws_dropped(S.Agent.ws(a))

def unavailable(-o: S.St, +cx: X.Cx<o>, +s: Snd, +a: S.Agent) -> String:
  cat(["recipient_unavailable: @", Snd.to(s), " is ", ST.status_str(a, rt(o, cx, Snd.to(s))),
    Bool.pick(String, dropped_wt(a), " (its worktree was deleted: /restore)", "")])

def create.sent(-o: S.St, p: X.Cx<o> & SendR, +name: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, SOk{_}):
      (cx, TOk{name})
    case (+cx, SErr{+e}):
      (cx, TErr{e})

def NewTask.parent(x: NewTask) -> String:
  match x:
    case NewTask{+parent, _, _, _, _, _, _, _, _}:
      parent

def NewTask.objective(x: NewTask) -> String:
  match x:
    case NewTask{_, _, _, _, +objective, _, _, _, _}:
      objective

def NewTask.brief(x: NewTask) -> J.Json:
  match x:
    case NewTask{_, _, +brief, _, _, _, _, _, _}:
      brief

def NewTask.parts(x: NewTask) -> String:
  match x:
    case NewTask{_, _, _, _, _, _, _, +parts, _}:
      parts

def NewTask.fwd(x: NewTask) -> Maybe<&2, Nat>:
  match x:
    case NewTask{_, _, _, _, _, _, _, _, +fwd}:
      fwd

def NewTask.brief_text(x: NewTask) -> String:
  match x:
    case NewTask{_, _, _, +brief_text, _, _, _, _, _}:
      brief_text

def Snd.from(x: Snd) -> String:
  match x:
    case Snd{+from, _, _, _, _, _, _, _, _, _, _}:
      from

def send.life(-o: S.St, life: S.Life, +cx: X.Cx<o>, +s: Snd, +a: S.Agent) -> X.Cx<o> & SendR:
  match life:
    case S.LActive{}:
      send.thread(o, Snd.reply_to(s), cx, s)
    case S.LFailed{}:
      send.revive(o, String.eq(Snd.from(s), M.USER()) || M.maybe_str_is(S.Agent.parent(a), Snd.from(s)), cx, s, unavailable(o, cx, s, a))
    case S.LStopped{}:
      send.revive(o, String.eq(Snd.from(s), M.USER()) && Bool.not(dropped_wt(a)), cx, s, unavailable(o, cx, s, a))
    case S.LArchived{}:
      send.revive(o, String.eq(Snd.from(s), M.USER()) && Bool.not(dropped_wt(a)), cx, s, unavailable(o, cx, s, a))

def send.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match m:
    case None{}:
      serr(o, cx, String.append("recipient_unknown: no agent named ", Snd.to(s)))
    case Some{+a}:
      send.life(o, S.Agent.life(a), cx, s, a)

def send.checked(-o: S.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match pre:
    case Some{+e}:
      serr(o, cx, e)
    case None{}:
      send.agent(o, M.get_agent(st(o, cx), Snd.to(s)), cx, s)

def Snd.text(x: Snd) -> String:
  match x:
    case Snd{_, _, +text, _, _, _, _, _, _, _, _}:
      text

def Snd.parts(x: Snd) -> String:
  match x:
    case Snd{_, _, _, _, _, _, _, _, _, +parts, _}:
      parts

def Snd.with_parts(x: Snd, +v: String) -> Snd:
  match x:
    case Snd{+from, +to, +text, +expect, +reply_to, +plain, +auto, +queued, +via, _, +fwd}:
      Snd{from, to, text, expect, reply_to, plain, auto, queued, via, v, fwd}

def Snd.with_to(x: Snd, +v: String) -> Snd:
  match x:
    case Snd{+from, _, +text, +expect, +reply_to, +plain, +auto, +queued, +via, +parts, +fwd}:
      Snd{from, v, text, expect, reply_to, plain, auto, queued, via, parts, fwd}

def Snd.with_text(x: Snd, +v: String) -> Snd:
  match x:
    case Snd{+from, +to, _, +expect, +reply_to, +plain, +auto, +queued, +via, +parts, +fwd}:
      Snd{from, to, v, expect, reply_to, plain, auto, queued, via, parts, fwd}

def Snd.fwd(x: Snd) -> Maybe<&2, Nat>:
  match x:
    case Snd{_, _, _, _, _, _, _, _, _, _, +fwd}:
      fwd

def Snd.with_fwd(x: Snd, +v: Maybe<&2, Nat>) -> Snd:
  match x:
    case Snd{+from, +to, +text, +expect, +reply_to, +plain, +auto, +queued, +via, +parts, _}:
      Snd{from, to, text, expect, reply_to, plain, auto, queued, via, parts, v}

def send.known(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match m:
    case None{}:
      serr(o, cx, String.append("recipient_unknown: no agent named ", Snd.to(s)))
    case Some{+to}:
      +pre = Bool.pick(Maybe<&2, String>, String.eq(to, Snd.from(s)), Some{"invalid_recipient: an agent cannot write to itself"},
        Bool.pick(Maybe<&2, String>, M.trim_is_empty(Snd.text(s)), Some{"message vide"}, None{}))
      send.checked(o, pre, cx, Snd.with_to(s, to))

def send.user(-o: S.St, user: Bool, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match user:
    case True{}:
      serr(o, cx, "invalid_recipient: to talk to the user, go through main (sb notify)")
    case False{}:
      send.known(o, M.resolve(st(o, cx), Snd.to(s)), cx, s)

# ---- main's feed (BISE-04, hub line protocol v2, contract C2) ----

# a hub line whose text is several fields joined by " : " (the daemon
# escapes " : " inside each field, see core.rs)
# C2 amendment (BISE-86): a message from the user that cannot reach its
# agent (stopped, dropped, archived) says so in the feed where the user
# wrote it: an `undelivered` record {to, text} (the TUI marks it ✗)
def undelivered.if(-o: S.St, gone: Bool, +cx: X.Cx<o>, +view: String, +name: String, +text: String) -> X.Cx<o>:
  match gone:
    case True{}:
      feed(o, cx, view, FD.undelivered(name, text))
    case False{}:
      cx

# the feed the user wrote a message in: its `via` (another view), else
# the agent's own
def undelivered.feed(m: Maybe<&2, String>, +to: String) -> String:
  match m:
    case Some{+v}:
      v
    case None{}:
      to

# the user's messages still queued for an agent that stops
def undelivered_msgs(-o: S.St, xs: List<&2, S.Msg>, +cx: X.Cx<o>) -> X.Cx<o>:
  match xs:
    case Nil{}:
      cx
    case Con{+m, +rest}:
      +to = S.Msg.to(m)
      undelivered_msgs(o, rest, undelivered.if(o, String.eq(S.Msg.from(m), M.USER()), cx,
        undelivered.feed(S.Msg.via(m), to), to, S.Msg.text(m)))

def unavailable_err(+e: String) -> Bool:
  String.starts_with(e, "recipient_unavailable")

# the message `m` points to (a card's, a reply's)
def for_msg_of(+s: S.St, m: Maybe<&2, Nat>) -> Maybe<&2, S.Msg>:
  match m:
    case None{}:
      None{}
    case Some{+id}:
      M.msg_of(s, id)

# the question `to` asked main, that `m` answers
def asked_main(q: Maybe<&2, S.Msg>, +to: String) -> Bool:
  match q:
    case None{}:
      False{}
    case Some{+x}:
      String.eq(S.Msg.from(x), to) && String.eq(S.Msg.to(x), M.MAIN()) && S.Msg.expect(x)

def question_text(q: Maybe<&2, S.Msg>) -> String:
  match q:
    case None{}:
      ""
    case Some{+x}:
      S.Msg.text(x)

# main's feed shows every message between agents (including between two
# tasks), so what happened stays readable: `msg : from → to m_<n> : text`
# (the id, BISE-110: the TUI hides main's `sb send` box when its message
# shows; an older TUI reads `to m_<n>` as the receiver's name).
# What main receives already shows as `msg-in` when delivered; main
# answering a task's question shows as `answered` (level 2) instead.
def feed_main(-o: S.St, +cx: X.Cx<o>, +m: S.Msg, +why: String) -> X.Cx<o>:
  +from = S.Msg.from(m)
  +to = S.Msg.to(m)
  +q = for_msg_of(st(o, cx), S.Msg.reply_to(m))
  +answered = String.eq(from, M.MAIN()) && asked_main(q, to)
  +agents = Bool.not(String.eq(from, M.USER())) && Bool.not(String.eq(to, M.MAIN()))
  LZ.pick(X.Cx<o>, answered,
    u => feed(o, cx, M.MAIN(), FD.answered(to, question_text(q), S.Msg.text(m), why)),
    u => LZ.pick(X.Cx<o>, agents,
      v => msg_parts(o, feed(o, cx, M.MAIN(), FD.msg(from, to, Some{S.Msg.id(m)}, S.Msg.text(m))), M.MAIN(), m),
      v => cx))

# What a task sent is a fact of its own thread (desktop bar A.3, S16;
# architect m_10203): a message from a task (not main, whose feed has
# `msg` for every message, not the user) to another agent writes one
# record in the task's own feed, `sent` {to, id, ask, body} (ask: it
# waits for a reply, --expect-reply or sb ask). A message to
# the user is `msg-you` already. A view only: no state changes.
# The sender must be an agent of this hub (architect m_10280): '@bise'
# (a cross-hub delegation) or the hub's own name has no feed here.
def is_task_send(-o: S.St, +cx: X.Cx<o>, +from: String, +to: String) -> Bool:
  has_agent(o, cx, from) && Bool.not(String.eq(from, M.MAIN())) && Bool.not(String.eq(to, M.USER()))

def feed_sender(-o: S.St, +cx: X.Cx<o>, +m: S.Msg) -> X.Cx<o>:
  +from = S.Msg.from(m)
  +to = S.Msg.to(m)
  LZ.pick(X.Cx<o>, is_task_send(o, cx, from, to),
    u => sent_parts(o, feed(o, cx, from, FD.sent(to, S.Msg.id(m), S.Msg.expect(m), S.Msg.text(m))), from, m),
    u => cx)

def send.fed(-o: S.St, p: X.Cx<o> & SendR, +why: String) -> X.Cx<o> & SendR:
  match p:
    case (+cx, SOk{+m}):
      (feed_main(o, feed_sender(o, cx, m), m, why), SOk{m})
    case (+cx, SErr{+e}):
      (cx, SErr{e})

# send a message, with main's reason when it answers for the user
# (`sb send --why`); answers it, or why it was refused
def send_why(-o: S.St, +cx: X.Cx<o>, +s: Snd, +why: String) -> X.Cx<o> & SendR:
  send.fed(o, send.user(o, String.eq(Snd.to(s), M.USER()), cx, s), why)

# send a message; answers it, or why it was refused
def send(-o: S.St, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  send_why(o, cx, s, "")

def msg(+from: String, +to: String, +text: String, +expect: Bool, +reply_to: Maybe<&2, Nat>, +plain: Bool, +auto: Bool) -> Snd:
  Snd{from, to, text, expect, reply_to, plain, auto, False{}, None{}, "", None{}}

def fst(-o: S.St, p: X.Cx<o> & SendR) -> X.Cx<o>:
  match p:
    case (+cx, _):
      cx

# ---- an MCP App view's message (docs/mcp-apps.md, C5, decision q-message) ----
# A view's `ui/message` is one message from the fixed sender `view`, its
# plugin in the typed field `view`, to that view's agent only, queued
# (never steered in): the agent reads it at its next turn, wrapped like a
# page note; it is never his words (laws view_msg_*). An agent gone or not
# active: nothing is sent, the hub says so to the view.

def view_snd(+agent: String, +text: String) -> Snd:
  Snd{M.VIEW(), agent, text, False{}, None{}, False{}, False{}, True{}, None{}, "", None{}}

# the message a view's text becomes: msg_of_snd's, with its plugin
def view_msg_of(+agent: String, +plugin: String, +text: String, +id: Nat, +now: Nat) -> S.Msg:
  S.Msg.with_view(msg_of_snd(view_snd(agent, text), id, id, now), Some{plugin})

def view_message.go(-o: S.St, ok: Bool, +cx: X.Cx<o>, +agent: String, +plugin: String, +text: String) -> X.Cx<o>:
  match ok:
    case False{}:
      notice(o, cx, cat(["the view's message didn't reach @", agent, ": it is not active"]))
    case True{}:
      +m = view_msg_of(agent, plugin, text, S.St.next_msg(st(o, cx)), now(o, cx))
      fst(o, send.waiter(o, None{}, emit(o, cx, M.EMsgSent{m}, {==}, {==}, Unit{}), m))

# an agent of the hub, not archived, stopped nor failed
def live_agent(m: Maybe<&2, S.Agent>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      is_active(S.Agent.life(a))

def view_message(-o: S.St, +cx: X.Cx<o>, +agent: String, +plugin: String, +text: String) -> X.Cx<o>:
  view_message.go(o, live_agent(M.agent_named(S.St.agents(st(o, cx)), agent)), cx, agent, plugin, text)

# a notification of the hub to main: it wakes main like any message (a
# main note waits for main's next turn instead)
def notify_main(-o: S.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  fst(o, send(o, cx, msg(M.HUB(), M.MAIN(), text, False{}, None{}, False{}, False{})))

# hub-loop (place.bend): a placing task's worktree is made by the shell
def place.ask(-o: S.St, placing: Bool, +cx: X.Cx<o>, +name: String, +ws: J.Json) -> X.Cx<o>:
  match placing:
    case True{}:
      out(o, cx, F.worktree_create(name, J.get_bool_or(ws, "with_changes", False{})))
    case False{}:
      cx

def create.go.new(-o: S.St, +cx: X.Cx<o>, +t: NewTask, +name: String, +ws: J.Json, +e: {M.name_held(X.d(o, cx), name) == False{} : Bool}) -> X.Cx<o> & TaskR:
  +parent = NewTask.parent(t)
  +label = Bool.pick(String, M.ws_placing(ws), " (worktree being made)", NT.branch_label(M.ws_branch(ws)))
  +obj = T.one_line(NewTask.objective(t))
  +cx2 = emit(o, cx, M.ETaskCreated{name, parent, NewTask.brief(t), ws, now(o, cx)}, e, Unit{}, Unit{})
  +who = Bool.pick(String, String.eq(parent, M.USER()), "you", parent)
  +cx3 = feed(o, cx2, M.MAIN(), FD.spawn(cat([who, " → new agent @", name, label, " : ", obj])))
  +cx4 = LZ.pick(X.Cx<o>, String.eq(parent, M.USER()), u => note_main(o, cx3, cat(["the user created the agent @", name, label, ": ", obj])), u => cx3)
  +cx5 = place.ask(o, PL.makes(ws), spawn(o, cx4, name, False{}), name, ws)
  +head = cat(["# Task `", name, "`\n\n"])
  +m = msg(parent, name, String.append(head, NewTask.brief_text(t)), String.eq(parent, M.MAIN()), None{}, String.eq(parent, M.USER()), False{})
  # send-parts: the brief's parts (`sb spawn --attach`), the head in front
  # (law brief_keeps_parts)
  create.sent(o, send(o, cx5, Snd.with_fwd(Snd.with_parts(m, NT.brief_parts(head, NewTask.parts(t))), NewTask.fwd(t))), name)

def create.go.free(-o: S.St, taken: Bool, +cx: X.Cx<o>, +t: NewTask, +name: String, +ws: J.Json, +e: {M.name_held(X.d(o, cx), name) == taken : Bool}) -> X.Cx<o> & TaskR:
  match taken:
    case True{}:
      (cx, TErr{String.append("name already taken: ", name)})
    case False{}:
      create.go.new(o, cx, t, name, ws, e)

# the name is still free (unique_name found it; it can only be taken if
# its search ran out); an archived agent's or an old name is freed first
def create.go(-o: S.St, +cx: X.Cx<o>, +t: NewTask, +name: String, +ws: J.Json) -> X.Cx<o> & TaskR:
  +cx2 = TK.take(o, cx, name)
  create.go.free(o, M.name_held(st(o, cx2), name), cx2, t, name, ws, {==})

def NewTask.with_changes(x: NewTask) -> Bool:
  match x:
    case NewTask{_, _, _, _, _, _, +with_changes, _, _}:
      with_changes

def create.git(-o: S.St, git: Bool, +cx: X.Cx<o>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match git:
    case False{}:
      (cx, TErr{"the workspace is not a git repository: no worktree possible"})
    case True{}:
      # hub-loop: the shell makes it off its loop (place.bend)
      create.go(o, cx, t, name, PL.placing_ws(NewTask.with_changes(t)))

def create.wt(-o: S.St, wt: Bool, +cx: X.Cx<o>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match wt:
    case False{}:
      create.go(o, cx, t, name, PL.shared_ws(X.R.workspace(rr(o, cx))))
    case True{}:
      create.git(o, X.R.git(rr(o, cx)), cx, t, name)

def NewTask.worktree(x: NewTask) -> Bool:
  match x:
    case NewTask{_, _, _, _, _, +worktree, _, _, _}:
      worktree

def NewTask.base(x: NewTask) -> String:
  match x:
    case NewTask{_, +base, _, _, _, _, _, _, _}:
      base

# RFC 0001 §7.1 and RFC 0002 §3-4 (the name was validated by the daemon).
# An empty name is never an agent's (M.never_agent): refused, never made
# `-2` by unique_name (law empty_name_refused)
def create_task.go(-o: S.St, empty: Bool, +cx: X.Cx<o>, +t: NewTask) -> X.Cx<o> & TaskR:
  match empty:
    case True{}:
      (cx, TErr{"a new task needs a name"})
    case False{}:
      create.wt(o, NewTask.worktree(t), cx, t, NT.unique_name(st(o, cx), NewTask.base(t)))

def create_task(-o: S.St, +cx: X.Cx<o>, +t: NewTask) -> X.Cx<o> & TaskR:
  create_task.go(o, String.is_empty(NewTask.base(t)), cx, t)

def create_in.go(-o: S.St, empty: Bool, +cx: X.Cx<o>, +t: NewTask, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> X.Cx<o> & TaskR:
  match empty:
    case True{}:
      (cx, TErr{"a new task needs a name"})
    case False{}:
      create.go(o, cx, t, NT.unique_name(st(o, cx), NewTask.base(t)), J.JObj{kvs})

# `sb spawn --place <agent>|<branch>` (dev-flow §3.1): the new task joins
# that place (`join`: its ws, found by the daemon); else as create_task
def create_in(-o: S.St, +cx: X.Cx<o>, +t: NewTask, j: Maybe<&2, J.Json>) -> X.Cx<o> & TaskR:
  match j:
    case Some{J.JObj{+kvs}}:
      create_in.go(o, String.is_empty(NewTask.base(t)), cx, t, kvs)
    case _:
      create_task(o, cx, t)

# ---- timers (`sb every`, hub/timers.bend) ----
# The steps of the timers, wired around timers.bend's pure decisions.
# They live here, not in timers.bend: they call send, pump, emit and ask,
# which are core's, and timers.bend (imported by model.bend) cannot
# import core (Bend has no import cycles).

def live(-o: S.St, +cx: X.Cx<o>) -> List<&2, S.Timer>:
  S.Tms.live(tms(o, cx))

# a message from bise is queued for `n` (a wake not delivered yet)
def bise_queued(+s: S.St, +n: String) -> Bool:
  R.from_bise(M.queued_for(s, n))

def ready.go(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      is_active(S.Agent.life(a)) && R.reachable(X.Rt.run(rt(o, cx, S.Agent.name(a))))

# active, idle or busy: a wake reaches it now (a turn, or steered into its
# turn: law timer_reaches_busy); starting or down: it waits
def timer_ready(-o: S.St, +cx: X.Cx<o>, +n: String) -> Bool:
  ready.go(o, R.agent_of(st(o, cx), n), cx)

# a paused project fires no timer (law timer_held_while_paused): a due
# one fires at the first tick after /resume
def timer_may(-o: S.St, +cx: X.Cx<o>, +t: S.Timer, +due: Bool) -> Bool:
  Bool.not(P.is_paused(st(o, cx))) && TM.may_fire(bise_queued(st(o, cx), S.Timer.agent(t)), due, t, timer_ready(o, cx, S.Timer.agent(t)), now(o, cx))

def timer_spent(-o: S.St, spent: Bool, +cx: X.Cx<o>, +id: Nat, +at: Nat) -> X.Cx<o>:
  match spent:
    case True{}:
      emit(o, cx, M.ETimerStop{id, "it ran its times", at}, Unit{}, Unit{}, Unit{})
    case False{}:
      cx

# the wake reached the queue: a due one counts (and ends the timer when it
# ran its times, law times_bound), a run now is only a run
def timer_count(-o: S.St, due: Bool, +cx: X.Cx<o>, +t: S.Timer, +next: Nat) -> X.Cx<o>:
  match due:
    case False{}:
      emit(o, cx, M.ETimerRun{S.Timer.id(t), now(o, cx)}, Unit{}, Unit{}, Unit{})
    case True{}:
      timer_spent(o, TM.spent(t),
        emit(o, cx, M.ETimerFired{S.Timer.id(t), 1n+S.Timer.fired(t), next, now(o, cx), False{}}, Unit{}, Unit{}, Unit{}),
        S.Timer.id(t), now(o, cx))

# a fire counts only when its message was queued (law fire_is_a_message,
# amb-tools m_5822: a one-shot timer was spent on a lost wake)
# the wake was refused (unreachable today, a safety net never driven live:
# timer_may wants it idle and active, and send then always queues): a due
# one is tried again in a minute, nothing counted, one line that says so
# (every_fired undelivered, the hub's warn); a run now is just not done
def timer_retry(-o: S.St, due: Bool, +cx: X.Cx<o>, +t: S.Timer) -> X.Cx<o>:
  match due:
    case False{}:
      cx
    case True{}:
      emit(o, cx, M.ETimerFired{S.Timer.id(t), S.Timer.fired(t), Nat.add(now(o, cx), TM.MIN_MS()), S.Timer.last(t), True{}},
        Unit{}, Unit{}, Unit{})

def timer_fire.sent(-o: S.St, p: X.Cx<o> & SendR, +t: S.Timer, +due: Bool, +next: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SOk{_}):
      timer_count(o, due, cx, t, next)
    case (+cx, SErr{_}):
      timer_retry(o, due, cx, t)

def timer_fire.go(-o: S.St, ok: Bool, +cx: X.Cx<o>, +t: S.Timer, +text: String, +due: Bool, +next: Nat) -> X.Cx<o>:
  match ok:
    case False{}:
      cx
    case True{}:
      timer_fire.sent(o, send(o, cx, msg(M.HUB(), S.Timer.agent(t), text, False{}, None{}, False{}, False{})), t, due, next)

# the only way a timer wakes its agent: a due wake (`due`, counted, its
# next time `next`) from the tick, or /scheduled's run now; never while a
# wake from bise is still queued for it (law wake_never_stacked)
def timer_fire(-o: S.St, +cx: X.Cx<o>, +t: S.Timer, +text: String, +due: Bool, +next: Nat) -> X.Cx<o>:
  timer_fire.go(o, timer_may(o, cx, t, due), cx, t, text, due, next)

# {"t":"every_set"}: the hub checked the request and computed `next_ms`
# (its clock); a timer only for an active agent
def timer_set.new(-o: S.St, s: Maybe<&2, S.Sched>, +cx: X.Cx<o>, +agent: String, +i: GI.In) -> X.Cx<o>:
  match s:
    case None{}:
      cx
    case Some{+sc}:
      match i:
        case GI.IEverySet{_, +by, +text, +next_ms, +name, _, _, +until_ms, +times, +page}:
          +t = S.Timer.with_next(S.Timer.with_sched(S.Timer.with_text(S.Timer.with_by(S.Timer.with_agent(S.Timer.with_id(S.Timer.empty(), 1n+S.Tms.last_id(tms(o, cx))), agent), by), text), sc), next_ms)
          emit(o, cx, M.ETimerSet{S.Timer.with_held(S.Timer.with_name(S.Timer.with_page(S.Timer.with_times(S.Timer.with_until(t, until_ms), times), page), name), String.is_empty(name)), now(o, cx)}, Unit{}, Unit{}, Unit{})
        case _:
          cx

def timer_set.active(-o: S.St, ok: Bool, +cx: X.Cx<o>, +agent: String, +sc: Maybe<&2, S.Sched>, +i: GI.In) -> X.Cx<o>:
  match ok:
    case False{}:
      cx
    case True{}:
      timer_set.new(o, sc, cx, agent, i)

def timer_set.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +sc: Maybe<&2, S.Sched>, +i: GI.In) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      timer_set.active(o, is_active(S.Agent.life(a)), cx, S.Agent.name(a), sc, i)

# `i`: the input, its IEverySet record
def timer_set(-o: S.St, +cx: X.Cx<o>, +agent: String, +every_ms: Maybe<&2, Nat>, +daily_min: Maybe<&2, Nat>, +i: GI.In) -> X.Cx<o>:
  timer_set.agent(o, R.agent_of(st(o, cx), agent), cx, SH.sched.go(every_ms, daily_min), i)

# {"t":"every_stop"}: `sb every --stop`, /scheduled's x; `wake` (the
# user's stop): what bise tells its agent, in the same step
def timer_stop.found(-o: S.St, m: Maybe<&2, S.Timer>, +cx: X.Cx<o>, +id: Nat, +why: String, +wake: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+t}:
      +cx2 = emit(o, cx, M.ETimerStop{id, why, now(o, cx)}, Unit{}, Unit{}, Unit{})
      LZ.pick(X.Cx<o>, String.is_empty(wake), u => cx2,
        u => fst(o, send(o, cx2, msg(M.HUB(), S.Timer.agent(t), wake, False{}, None{}, False{}, False{}))))


# {"t":"every_run"}: /scheduled's run now, through timer_fire
def timer_run.found(-o: S.St, m: Maybe<&2, S.Timer>, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+t}:
      timer_fire(o, cx, t, text, False{}, 0n)


# {"t":"every_name"}: the hub's name for a live timer (its small model's,
# else its plain fallback); an ended or unknown one, or no name: nothing.
# A timer set without a name is held: its ◷ set line waits for this step,
# which announces it (architect m_15603: in sb-core, so a restart between
# set and name still writes it, once; law named_announces_once)
def timer_name.found(-o: S.St, m: Maybe<&2, S.Timer>, +cx: X.Cx<o>, +id: Nat, +name: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+t}:
      LZ.pick(X.Cx<o>, String.is_empty(name), u => cx,
        u => emit(o, cx, M.ETimerName{id, name, S.Timer.held(t), now(o, cx)}, Unit{}, Unit{}, Unit{}))


def timers_gone.each(-o: S.St, ids: List<&2, Nat>, +cx: X.Cx<o>, +why: String) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      timers_gone.each(o, rest, emit(o, cx, M.ETimerStop{id, why, now(o, cx)}, Unit{}, Unit{}, Unit{}), why)

def timers_gone.if(-o: S.St, archived: Bool, +cx: X.Cx<o>, +n: String, +ns: List<&2, String>) -> X.Cx<o>:
  match archived:
    case False{}:
      cx
    case True{}:
      timers_gone.each(o, TM.ids_in(live(o, cx), ns), cx, cat(["@", n, " is gone"]))

# an archived (dropped) agent's timers end with it, in the same step (law
# stop_leaves_no_timer); a stopped one's wait for it, as before
def timers_gone(-o: S.St, +life: S.Life, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  timers_gone.if(o, R.life_gone(life), cx, n, M.names_of(st(o, cx), n))

# ---- the timers at a tick ----

def wake_q.go(ok: Bool, +t: S.Timer, +now: Nat) -> List<&2, J.Json>:
  match ok:
    case False{}:
      Nil{}
    case True{}:
      [J.obj(List.append(&2, Sigma<&2, &2, String, _ => J.Json>,
        [J.kv("id", J.JNum{S.Timer.id(t)}), J.kv("fired", J.JNum{1n+S.Timer.fired(t)}),
          J.kv("waited_ms", J.JNum{Nat.sub(now, S.Timer.next(t))})],
        Bool.pick(List<&2, Sigma<&2, &2, String, _ => J.Json>>, TM.is_daily(S.Timer.sched(t)), [SH.sched_kv(S.Timer.sched(t))], Nil{})))]

# the wakes this tick may send (their texts and daily next times come
# from the hub: one {need} for them all, answered by id)
def wake_qs(-o: S.St, ts: List<&2, S.Timer>, +cx: X.Cx<o>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case Con{+t, +rest}:
      +ok = Maybe.is_none(&2, String, TM.end_why(t, R.asked_gone(st(o, cx), S.Timer.agent(t)), now(o, cx))) && timer_may(o, cx, t, True{})
      List.append(&2, J.Json, wake_q.go(ok, t, now(o, cx)), wake_qs(o, rest, cx))

def wake_ans(xs: List<&2, J.Json>, +id: Nat) -> Maybe<&2, J.Json>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      LZ.pick(Maybe<&2, J.Json>, Nat.is_eq(K.get_n(h, "id"), id), u => Some{h}, u => wake_ans(t, id))

def timers_tick.wake(-o: S.St, a: Maybe<&2, J.Json>, +cx: X.Cx<o>, +t: S.Timer) -> X.Cx<o>:
  match a:
    case None{}:
      cx
    case Some{+w}:
      timer_fire(o, cx, t, K.get_s(w, "text"), True{},
        TM.next_of(S.Timer.sched(t), S.Timer.next(t), now(o, cx), K.get_n(w, "next_ms")))

def timers_tick.end(-o: S.St, w: Maybe<&2, String>, +cx: X.Cx<o>, +t: S.Timer, +ans: List<&2, J.Json>) -> X.Cx<o>:
  match w:
    case Some{+why}:
      emit(o, cx, M.ETimerStop{S.Timer.id(t), why, now(o, cx)}, Unit{}, Unit{}, Unit{})
    case None{}:
      timers_tick.wake(o, wake_ans(ans, S.Timer.id(t)), cx, t)

def timers_tick.each(-o: S.St, ts: List<&2, S.Timer>, +cx: X.Cx<o>, +ans: List<&2, J.Json>) -> X.Cx<o>:
  match ts:
    case Nil{}:
      cx
    case Con{+t, +rest}:
      +cx2 = timers_tick.end(o, TM.end_why(t, R.asked_gone(st(o, cx), S.Timer.agent(t)), now(o, cx)), cx, t, ans)
      timers_tick.each(o, rest, cx2, ans)

def json_arr(m: Maybe<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case Some{J.JArr{+vals}}:
      vals
    case _:
      Nil{}

def timers_tick.asked(-o: S.St, p: X.Cx<o> & Maybe<&2, J.Json>, +ts: List<&2, S.Timer>) -> X.Cx<o>:
  match p:
    case (+cx, None{}):
      # the step halts: the hub answers, then runs the tick again (same now)
      cx
    case (+cx, Some{+a}):
      timers_tick.each(o, ts, cx, json_arr(J.get(a, "wakes")))

def timers_tick.ask(-o: S.St, qs: List<&2, J.Json>, +cx: X.Cx<o>, +ts: List<&2, S.Timer>) -> X.Cx<o>:
  match qs:
    case Nil{}:
      timers_tick.each(o, ts, cx, Nil{})
    case Con{+q, +rest}:
      timers_tick.asked(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"every_wake"}), J.kv("wakes", J.JArr{q <> rest})])), ts)

def timers_tick.go(-o: S.St, work: Bool, +cx: X.Cx<o>) -> X.Cx<o>:
  match work:
    case False{}:
      cx
    case True{}:
      timers_tick.ask(o, wake_qs(o, live(o, cx), cx), cx, live(o, cx))

# the timers due now wake their agents (a busy one in its turn, steered:
# law timer_fires_busy), the ended ones stop. A tick with none due or past its end does
# nothing (idle-cpu)
def timers_tick(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  timers_tick.go(o, TM.any_work(live(o, cx), now(o, cx)), cx)

# ---- watches (`sb wake`, hub/wakes.bend) ----
# Call lines only: the decisions are WK's (hit, due, ids_of), the words
# Rust's (wake.rs: the hit text in the input, the max text in the watch).

# a watch's wake from bise: never queued mode, so an idle agent starts a
# turn and a busy one gets it in its turn, steered (law wake_steers_busy)
def wake_snd(+agent: String, +text: String) -> Snd:
  Snd{M.HUB(), agent, text, False{}, None{}, False{}, False{}, False{}, None{}, "", None{}}

def wake_say(-o: S.St, +cx: X.Cx<o>, +agent: String, +text: String) -> X.Cx<o>:
  fst(o, send(o, cx, wake_snd(agent, text)))

# {"t":"wake_hit"}: the look saw it happen. A live watch of an active
# agent ends and its agent gets ONE message from bise in the same step
# (a busy one in its turn, steered). Ended or unknown:
# nothing (laws wake_hit_once, wake_gone_never_hits); a stopped agent's
# watch waits for it.
def wake_hit.found(-o: S.St, m: Maybe<&2, S.Watch>, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+w}:
      LZ.pick(X.Cx<o>, R.agent_active(st(o, cx), S.Watch.agent(w)),
        u => wake_say(o, wake_end(o, cx, S.Watch.id(w), "hit"), S.Watch.agent(w), text),
        u => cx)

def wake_hit(-o: S.St, +cx: X.Cx<o>, +id: Nat, +text: String) -> X.Cx<o>:
  wake_hit.found(o, WK.hit(wks(o, cx), id), cx, text)

# {"t":"wake_set"}: the hub checked the request and wrote the words; a
# watch only for an active agent, its id the next free one
def wake_set.agent(-o: S.St, ok: Bool, +cx: X.Cx<o>, +agent: String, +spec: J.Json, +max_text: String, +max_ms: Nat, +quiet: Bool, +then: Maybe<&2, Bool>) -> X.Cx<o>:
  match ok:
    case False{}:
      cx
    case True{}:
      +w = WK.make(wks(o, cx), agent, spec, max_text, now(o, cx), max_ms, quiet, then)
      emit(o, cx, M.EWake{WK.WSet{w}}, Unit{}, Unit{}, Unit{})

def wake_set(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +spec: J.Json, +max_text: String, +max_ms: Nat, +quiet: Bool, +then: Maybe<&2, Bool>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+n}:
      wake_set.agent(o, R.agent_active(st(o, cx), n), cx, n, spec, max_text, max_ms, quiet, then)

# {"t":"wake_stop"}: `sb wake --stop`, or his stop (`tell`: its agent hears it once); ended or unknown: nothing
def wake_stop.found(-o: S.St, m: Maybe<&2, S.Watch>, +cx: X.Cx<o>, +why: String, +tell: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+w}:
      LZ.pick(X.Cx<o>, WK.stop_tells(tell, R.agent_active(st(o, cx), S.Watch.agent(w))),
        u => wake_say(o, wake_end(o, cx, S.Watch.id(w), why), S.Watch.agent(w), tell), u => wake_end(o, cx, S.Watch.id(w), why))

# a watch past its max ends, with its "still running" wake unless quiet
# (law wake_max_once: ended, it is never due again)
def wakes_tick.each(-o: S.St, ws: List<&2, S.Watch>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ws:
    case Nil{}:
      cx
    case Con{+w, +rest}:
      +cx2 = wake_end(o, cx, S.Watch.id(w), "max")
      +cx3 = LZ.pick(X.Cx<o>, WK.max_says(w, R.agent_active(st(o, cx2), S.Watch.agent(w))),
        u => wake_say(o, cx2, S.Watch.agent(w), S.Watch.max_text(w)), u => cx2)
      wakes_tick.each(o, rest, cx3)

def wakes_tick.go(-o: S.St, work: Bool, +cx: X.Cx<o>) -> X.Cx<o>:
  match work:
    case False{}:
      cx
    case True{}:
      wakes_tick.each(o, WK.due(S.Wakes.live(wks(o, cx)), now(o, cx)), cx)

# none past its max (the common case): nothing at all (idle-cpu)
def wakes_tick(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  wakes_tick.go(o, WK.any_due(S.Wakes.live(wks(o, cx)), now(o, cx)), cx)

def wakes_gone.each(-o: S.St, ids: List<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      wakes_gone.each(o, rest, wake_end(o, cx, id, "gone"))

# ---- a reply still pending (ask-wait, hub/wakes.bend make_reply) ----
# A hold that ends with no reply leaves a pause from the ask; its reply
# ends it with no wake, its agent's done/failed ends it quietly, the asked
# agent's drop tells it once (laws ask_*: bend/laws/wakes.bend).

# its agent moved on (done, failed): its reply watches end quietly (law
# ask_done_ends)
def replies_end(-o: S.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  wake_ends(o, WK.replies_of(S.Wakes.live(wks(o, cx)), M.names_of(st(o, cx), agent)), cx, "moved on")

def replies_gone.each(-o: S.St, ws: List<&2, S.Watch>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ws:
    case Nil{}:
      cx
    case Con{+w, +rest}:
      +cx2 = wake_end(o, cx, S.Watch.id(w), "gone")
      +cx3 = LZ.pick(X.Cx<o>, R.agent_active(st(o, cx2), S.Watch.agent(w)),
        u => wake_say(o, cx2, S.Watch.agent(w), WK.reply_gone_text(w)), u => cx2)
      replies_gone.each(o, rest, cx3)

# the tick: a reply watch whose asked agent is gone ends, its agent told
# once (law ask_target_gone_tells: ended, it is never an orphan again)
def replies_tick(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  replies_gone.each(o, R.replies_orphan(st(o, cx), S.Wakes.live(wks(o, cx))), cx)

# an archived (dropped) agent's watches end with it, in the same step
# (law wake_drop_ends); a stopped one's wait, as its timers do. The reply
# watches waiting on it end at the next tick (replies_tick)
def wakes_gone(-o: S.St, +life: S.Life, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, R.life_gone(life),
    u => wakes_gone.each(o, WK.ids_of(S.Wakes.live(wks(o, cx)), M.names_of(st(o, cx), n)), cx),
    u => cx)

def reject_ids(-o: S.St, ids: List<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      reject_ids(o, rest, emit(o, cx, M.EMsgState{id, S.Rejected{"recipient_unavailable"}}, Unit{}, Unit{}, Unit{}))

def set_waiters(-o: S.St, +cx: X.Cx<o>, +ws: List<&2, X.Waiter>) -> X.Cx<o>:
  set_r(o, cx, X.R.with_waiters(rr(o, cx), ws))

# the answer a view's card (docs/mcp-apps.md, C5) gets when its agent stops
def VIEW_CARD_STOPPED() -> String:
  "no: the agent stopped"

# a view's call's card: its typed `view` (Card.view), never its place
def is_view(m: Maybe<&2, S.ViewAsk>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{_}:
      True{}

def view_no.one(-o: S.St, hit: Bool, +cx: X.Cx<o>, +c: S.Card) -> X.Cx<o>:
  match hit:
    case True{}:
      out(o, cx, F.confirm_card(S.Card.id(c), S.Card.agent(c), VIEW_CARD_STOPPED()))
    case False{}:
      cx

# an agent stops: each of its view cards in `cs` (the cards before the
# stop closed them) gets its no, the `confirm` effect card_due sends at
# its due (law view_card_stop_answers): a view's call is answered once,
# by him, by its card's due or by its agent's stop
def view_nos(-o: S.St, cs: List<&2, S.Card>, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  match cs:
    case Nil{}:
      cx
    case Con{+c, +rest}:
      view_nos(o, rest, view_no.one(o, String.eq(S.Card.agent(c), agent) && is_view(S.Card.view(c)), cx, c), agent)

def stop_task.go(-o: S.St, +cx: X.Cx<o>, +name: String, +life: S.Life, +reason: String) -> X.Cx<o>:
  +cx1 = out(o, cx, F.kill(name))
  +cx2 = fail_waiters(o, cx1, name, "stopped")
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_steered(r, KL.steer_del(X.R.steered(r), name)))
  +cx4 = upd_rt(o, cx3, X.Rt.with_turn(X.Rt.with_run(rt(o, cx3, name), S.RDown{}), None{}))
  +cx5 = emit(o, cx4, M.ELifecycle{name, life, Bool.pick(Maybe<&2, String>, String.is_empty(reason), None{}, Some{reason})}, Unit{}, Unit{}, Unit{})
  +cx6 = reject_ids(o, ids_of(M.queued_for(st(o, cx5), name)), cx5)
  view_nos(o, S.St.cards(st(o, cx6)), close_cards(o, cx6, name, Nil{}, "agent stopped"), name)

# `agent`'s contexts leave (his message took them, or it stopped: an MCP App view's
# context, review d5)
def view_ctx_drop(-o: S.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  set_r(o, cx, X.R.with_ctx(rr(o, cx), VC.drop(X.R.ctx(rr(o, cx)), agent)))

# no wait of the stopped task survives (law stop_leaves_no_waiter), nor
# any view's context for it (d5)
def stop_task(-o: S.St, +cx: X.Cx<o>, +name: String, +life: S.Life, +reason: String) -> X.Cx<o>:
  set_waiters(o, view_ctx_drop(o, wakes_gone(o, life, timers_gone(o, life, stop_task.go(o, cx, name, life, reason), name), name), name), X.waiters_not(X.R.waiters(rr(o, cx)), name))

# stop_task, after saying which of the user's queued messages will never
# arrive (BISE-86)
def stop_noted(-o: S.St, +cx: X.Cx<o>, +name: String, +life: S.Life, +reason: String) -> X.Cx<o>:
  stop_task(o, undelivered_msgs(o, M.queued_for(st(o, cx), name), cx), name, life, reason)

# ---- places (dev-flow §3.1): a worktree shared by agents (place.bend) ----


# the place's folder is gone: so is it for the agents that left it
# before (archived), with the same saved work (RFC 0002 §5, per place)
def drop.left(-o: S.St, ns: List<&2, String>, +cx: X.Cx<o>, +snap: Maybe<&2, String>) -> X.Cx<o>:
  match ns:
    case Nil{}:
      cx
    case Con{+n, +t}:
      +cx2 = emit(o, cx, M.EWorkspace{n, PL.drop.left.ws(M.get_agent(st(o, cx), n))}, Unit{}, Unit{}, Unit{})
      drop.left(o, t, emit(o, cx2, M.ESnapshot{n, snap}, Unit{}, Unit{}, Unit{}), snap)

def drop.finish(-o: S.St, +cx: X.Cx<o>, +a: S.Agent, +label: String) -> X.Cx<o> & DropR:
  +cx2 = feed(o, cx, M.MAIN(), FD.info(cat(["@", S.Agent.name(a), " archived", label])))
  # a client looking at it goes back to main
  (out(o, cx2, F.focus_main(S.Agent.name(a))), DDone{})

# issue 32 3b: the archive is committed now; the worktree's save and
# removal run off the loop (fx worktree_drop), its ws `dropping` until
# the shell's `unplaced` (unplaced.in)
def drop.off(-o: S.St, +cx: X.Cx<o>, +a: S.Agent, +dirty: Nat, +unpushed: Nat) -> X.Cx<o>:
  +n = S.Agent.name(a)
  +cx2 = emit(o, stop_noted(o, cx, n, S.LArchived{}, "drop"), M.EWorkspace{n, PL.dropping_ws(S.Agent.ws(a), dirty, unpushed)}, Unit{}, Unit{}, Unit{})
  out(o, cx2, F.worktree_drop(n, S.Agent.ws(a), dirty, unpushed))

def drop.now(-o: S.St, has_wt: Bool, +cx: X.Cx<o>, +a: S.Agent, +loss: J.Json) -> X.Cx<o> & DropR:
  match has_wt:
    case False{}:
      # a worktree others still work in stays (it goes with its last agent)
      +label = PL.drop.stays(PL.mates_of(st(o, cx), a, True{}))
      drop.finish(o, stop_noted(o, cx, S.Agent.name(a), S.LArchived{}, "drop"), a, label)
    case True{}:
      drop.finish(o, drop.off(o, cx, a, J.get_num_or(loss, "dirty", 0n), J.get_num_or(loss, "unpushed", 0n)), a, PL.removing_note())

# the shell's `unplaced` for a dropping task: removed (its mates that left
# before lose the folder too, with the same saved work), or not (git's
# words in main's feed; the worktree stays, a /restore reuses it)
def unplaced.end(-o: S.St, ok: Bool, +cx: X.Cx<o>, +a: S.Agent, +snap: Maybe<&2, String>, +err: String) -> X.Cx<o>:
  match ok:
    case True{}:
      drop.left(o, PL.mates_of(st(o, cx), a, False{}), cx, snap)
    case False{}:
      feed(o, cx, M.MAIN(), FD.warn(PL.undropped_text(S.Agent.name(a), err)))

def unplaced.go(-o: S.St, dropping: Bool, +cx: X.Cx<o>, +a: S.Agent, +snap: Maybe<&2, String>, +err: String) -> X.Cx<o>:
  match dropping:
    case False{}:
      # not dropping (an orphan worktree removed, a second answer): nothing
      cx
    case True{}:
      +cx2 = emit(o, cx, M.EDropped{S.Agent.name(a), snap, err}, Unit{}, Unit{}, Unit{})
      unplaced.end(o, String.is_empty(err), cx2, a, snap, err)

def unplaced.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +snap: Maybe<&2, String>, +err: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      unplaced.go(o, M.ws_dropping(S.Agent.ws(a)), cx, a, snap, err)

def unplaced.in(-o: S.St, +cx: X.Cx<o>, +name: String, +snap: Maybe<&2, String>, +err: String) -> X.Cx<o>:
  unplaced.agent(o, M.get_agent(st(o, cx), name), cx, snap, err)

def drop.decide(-o: S.St, confirm: Bool, +cx: X.Cx<o>, +a: S.Agent, +has_wt: Bool, +loss: J.Json, +busy: Bool) -> X.Cx<o> & DropR:
  match confirm:
    case True{}:
      +dirty = J.get_num_or(loss, "dirty", 0n)
      +unpushed = J.get_num_or(loss, "unpushed", 0n)
      (cx, DAsk{PL.drop_body(busy, dirty, unpushed)})
    case False{}:
      drop.now(o, has_wt, cx, a, loss)

def drop.loss(-o: S.St, p: X.Cx<o> & Maybe<&2, J.Json>, +a: S.Agent, +force: Bool, +has_wt: Bool) -> X.Cx<o> & DropR:
  match p:
    case (+cx, None{}):
      (cx, DHalt{})
    case (+cx, Some{+loss}):
      +busy = is_busy(X.Rt.run(rt(o, cx, S.Agent.name(a))))
      +any = Nat.is_gt(Nat.add(J.get_num_or(loss, "dirty", 0n), J.get_num_or(loss, "unpushed", 0n)), 0n)
      drop.decide(o, Bool.not(force) && (busy || any), cx, a, has_wt, loss, busy)

def drop.wt(-o: S.St, has_wt: Bool, +cx: X.Cx<o>, +a: S.Agent, +force: Bool) -> X.Cx<o> & DropR:
  match has_wt:
    case True{}:
      drop.loss(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_loss"}), J.kv("name", J.JStr{S.Agent.name(a)}), J.kv("ws", S.Agent.ws(a))])), a, force, True{})
    case False{}:
      drop.loss(o, (cx, Some{J.obj([J.kv("dirty", J.JNum{0n}), J.kv("unpushed", J.JNum{0n})])}), a, force, False{})

def drop.checks(-o: S.St, is_main: Bool, life: S.Life, +cx: X.Cx<o>, +a: S.Agent, +force: Bool) -> X.Cx<o> & DropR:
  match is_main:
    case True{}:
      (cx, DErr{"main cannot be dropped"})
    case False{}:
      match life:
        case S.LArchived{}:
          (cx, DErr{cat(["@", S.Agent.name(a), " is already archived"])})
        case _:
          # the folder goes with the place's last agent only
          +last = List.is_empty(&2, String, PL.mates_of(st(o, cx), a, True{}))
          drop.wt(o, M.ws_worktree(S.Agent.ws(a)) && Bool.not(M.ws_dropped(S.Agent.ws(a)) || M.ws_placing(S.Agent.ws(a))) && last, cx, a, force)

def drop.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +name: String, +force: Bool) -> X.Cx<o> & DropR:
  match m:
    case None{}:
      (cx, DErr{String.append("no agent named ", name)})
    case Some{+a}:
      drop.checks(o, S.Agent.is_main(a), S.Agent.life(a), cx, a, force)

def agent_of(+s: S.St, m: Maybe<&2, String>) -> Maybe<&2, S.Agent>:
  match m:
    case None{}:
      None{}
    case Some{+n}:
      M.get_agent(s, n)

# RFC 0001 §9.4 + RFC 0002 §5
def drop_task(-o: S.St, +cx: X.Cx<o>, +name: String, +force: Bool) -> X.Cx<o> & DropR:
  drop.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name, force)

def restore.go.as(-o: S.St, +cx: X.Cx<o>, +a: S.Agent, +label: String) -> X.Cx<o> & TaskR:
  +n = S.Agent.name(a)
  +cx2 = emit(o, cx, M.ELifecycle{n, S.LActive{}, None{}}, Unit{}, Unit{}, Unit{})
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_crashes(r, KL.kn_del(X.R.crashes(r), n)))
  +cx4 = spawn(o, cx3, n, True{})
  # an idle task back from failed gets the mail it kept (law L4)
  (feed(o, pump(o, cx4, n), M.MAIN(), FD.info(cat(["@", n, " restored", label]))), TOk{n})

def restore.go(-o: S.St, +cx: X.Cx<o>, +a: S.Agent) -> X.Cx<o> & TaskR:
  restore.go.as(o, cx, a, "")

# issue 32 3b: its dropped worktree is made again off the loop: the task
# is active now, its ws placing (no REPL, its messages wait) until the
# shell's `placed` (placed: PRestored, PUnrestored)
def restore.off(-o: S.St, +cx: X.Cx<o>, +a: S.Agent) -> X.Cx<o> & TaskR:
  +n = S.Agent.name(a)
  +cx2 = emit(o, cx, M.EWorkspace{n, PL.restoring_ws(S.Agent.ws(a))}, Unit{}, Unit{}, Unit{})
  restore.go.as(o, out(o, cx2, F.worktree_restore(n, S.Agent.ws(a), S.Agent.snap(a))), a, PL.restoring_note())

def restore.wt(-o: S.St, dropped: Bool, +cx: X.Cx<o>, +a: S.Agent) -> X.Cx<o> & TaskR:
  match dropped:
    case True{}:
      restore.off(o, cx, a)
    case False{}:
      restore.go(o, cx, a)

# its place is live again (a mate restored it): it rejoins the folder
def restore.join(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +a: S.Agent) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      restore.wt(o, True{}, cx, a)
    case Some{+b}:
      +n = S.Agent.name(a)
      +cx2 = emit(o, cx, M.EWorkspace{n, PL.joined_ws(S.Agent.ws(b))}, Unit{}, Unit{}, Unit{})
      restore.go(o, emit(o, cx2, M.ESnapshot{n, None{}}, Unit{}, Unit{}, Unit{}), a)

def restore.dropped(-o: S.St, dropped: Bool, +cx: X.Cx<o>, +a: S.Agent) -> X.Cx<o> & TaskR:
  match dropped:
    case True{}:
      restore.join(o, PL.first_agent(st(o, cx), PL.mates_of(st(o, cx), a, True{})), cx, a)
    case False{}:
      restore.go(o, cx, a)

# issue 32 3b: no restore while the shell still removes its worktree
def restore.dropping(-o: S.St, dropping: Bool, +cx: X.Cx<o>, +a: S.Agent) -> X.Cx<o> & TaskR:
  match dropping:
    case True{}:
      (cx, TErr{PL.dropping_text(S.Agent.name(a))})
    case False{}:
      restore.dropped(o, M.ws_worktree(S.Agent.ws(a)) && M.ws_dropped(S.Agent.ws(a)), cx, a)

def restore.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      (cx, TErr{String.append("no agent named ", name)})
    case Some{+a}:
      LZ.pick(X.Cx<o> & TaskR, is_active(S.Agent.life(a)), u => (cx, TErr{cat(["@", S.Agent.name(a), " is active"])}),
        u => restore.dropping(o, M.ws_dropping(S.Agent.ws(a)), cx, a))

def restore_task(-o: S.St, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  restore.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name)

def isolate.sent(-o: S.St, p: X.Cx<o> & SendR, +n: String, +branch: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, SErr{+e}):
      (cx, TErr{e})
    case (+cx, SOk{_}):
      (feed(o, cx, M.MAIN(), FD.info(cat(["@", n, " now works in the worktree ", branch]))), TOk{n})

def isolate.res(-o: S.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +n: String) -> X.Cx<o> & TaskR:
  match r:
    case Fail{+e}:
      (cx, TErr{e})
    case Done{+ws}:
      +path = M.ws_path(ws)
      +branch = J.get_str_or(ws, "branch", "")
      +cx2 = emit(o, cx, M.EWorkspace{n, ws}, Unit{}, Unit{}, Unit{})
      # the REPL takes its new BEND_WORKDIR at the next spawn
      +cx3 = out(o, cx2, F.kill(n))
      +cx4 = spawn(o, upd_rt(o, cx3, X.Rt.with_run(rt(o, cx3, n), S.RDown{})), n, True{})
      +text = cat(["[bise] Your working directory is now the isolated git worktree `", path, "` (branch `", branch, "`). Work only there from now on."])
      isolate.sent(o, send(o, cx4, msg(M.USER(), n, text, False{}, None{}, True{}, False{})), n, branch)

def isolate.ans(-o: S.St, p: X.Cx<o> & Maybe<&2, J.Json>, +n: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, None{}):
      (cx, THalt{})
    case (+cx, Some{+ans}):
      isolate.res(o, PL.ans_ok(ans), cx, n)

def isolate.pre(-o: S.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +n: String) -> X.Cx<o> & TaskR:
  match pre:
    case Some{+e}:
      (cx, TErr{e})
    case None{}:
      isolate.ans(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_create"}), J.kv("name", J.JStr{n}), J.kv("with_changes", J.JBool{False{}})])), n)

def isolate.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      (cx, TErr{String.append("no agent named ", name)})
    case Some{+a}:
      +n = S.Agent.name(a)
      +pre = Bool.pick(Maybe<&2, String>, S.Agent.is_main(a) || M.ws_worktree(S.Agent.ws(a)), Some{cat(["@", n, " already has its own folder"])},
        Bool.pick(Maybe<&2, String>, Bool.not(List.is_empty(&2, String, S.Agent.files(a))),
          Some{cat(["@", n, " already changed files of the workspace (", String.join(List.take(&2, String, S.Agent.files(a), 3n), ", "), "): its work cannot be separated from yours"])},
          Bool.pick(Maybe<&2, String>, is_busy(X.Rt.run(rt(o, cx, n))), Some{cat(["@", n, " is mid-turn: wait for the end of its turn"])},
            Bool.pick(Maybe<&2, String>, X.R.git(rr(o, cx)), None{}, Some{"the workspace is not a git repository"}))))
      isolate.pre(o, pre, cx, n)

def isolate_task(-o: S.St, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  isolate.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name)

# ---- `sb move <agent> <place>` (dev-flow §3.1, /isolate generalized) ----

def move.sent(-o: S.St, p: X.Cx<o> & SendR, +n: String, +dest: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, SErr{+e}):
      (cx, TErr{e})
    case (+cx, SOk{_}):
      (feed(o, cx, M.MAIN(), FD.info(cat(["@", n, " moved to ", dest]))), TOk{n})

def move.res(-o: S.St, +cx: X.Cx<o>, +n: String, +ws: J.Json) -> X.Cx<o> & TaskR:
  +path = M.ws_path(ws)
  +branch = J.get_str_or(ws, "branch", "")
  +wt = M.ws_worktree(ws)
  +cx2 = emit(o, cx, M.EWorkspace{n, ws}, Unit{}, Unit{}, Unit{})
  # the REPL takes its new BEND_WORKDIR at the next spawn
  +cx3 = out(o, cx2, F.kill(n))
  +cx4 = spawn(o, upd_rt(o, cx3, X.Rt.with_run(rt(o, cx3, n), S.RDown{})), n, True{})
  +text = Bool.pick(String, wt,
    cat(["[bise] Your working directory is now the git worktree `", path, "` (branch `", branch, "`), shared with other agents: change and commit only your own files there (`sb land --here`). Work only there from now on."]),
    cat(["[bise] Your working directory is now the shared folder `", path, "`. Work only there from now on; do not revert changes you did not make."]))
  +dest = Bool.pick(String, wt, cat(["the worktree ", branch]), "the shared folder")
  move.sent(o, send(o, cx4, msg(M.USER(), n, text, False{}, None{}, True{}, False{})), n, dest)

def move.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +name: String, +ws: J.Json) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      (cx, TErr{String.append("no agent named ", name)})
    case Some{+a}:
      +n = S.Agent.name(a)
      +pre = Bool.pick(Maybe<&2, String>, S.Agent.is_main(a), Some{"main works in the shared folder"},
        Bool.pick(Maybe<&2, String>, String.eq(PL.Agent.place(a), PL.ws_place(ws, S.Agent.dir(a))), Some{cat(["@", n, " is already there"])},
          Bool.pick(Maybe<&2, String>, Bool.not(List.is_empty(&2, String, S.Agent.files(a))),
            Some{cat(["@", n, " already changed files (", String.join(List.take(&2, String, S.Agent.files(a), 3n), ", "), "): only an agent that changed nothing yet moves"])},
            Bool.pick(Maybe<&2, String>, is_busy(X.Rt.run(rt(o, cx, n))), Some{cat(["@", n, " is mid-turn: wait for the end of its turn"])}, None{}))))
      LZ.pick(X.Cx<o> & TaskR, Maybe.is_some(&2, String, pre), u => (cx, TErr{Maybe.default(&2, String, pre, "")}), u => move.res(o, cx, n, ws))

def move.to(-o: S.St, j: Maybe<&2, J.Json>, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  match j:
    case Some{J.JObj{+kvs}}:
      move.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name, J.JObj{kvs})
    case _:
      isolate_task(o, cx, name)

# `ws`: the target place's (the daemon finds it; none: a new worktree,
# /isolate); `pre_err`: the daemon refused (no such place...)
def move_task(-o: S.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +name: String, +j: Maybe<&2, J.Json>) -> X.Cx<o> & TaskR:
  match pre:
    case Some{+e}:
      (cx, TErr{e})
    case None{}:
      move.to(o, j, cx, name)

# ---- `sb feature retire <name>` (hub/feature.bend: who, the words) ----

# an active agent gets the note from the hub, queued (a busy one reads it
# after its turn, never a steer); a stopped or failed one only its new
# workspace: never a revive (law feature_retire_stops_nothing)
def feature_retire.tell(-o: S.St, life: S.Life, +cx: X.Cx<o>, +n: String, +text: String) -> X.Cx<o>:
  match life:
    case S.LActive{}:
      fst(o, send.thread(o, None{}, cx, Snd{M.HUB(), n, text, False{}, None{}, True{}, False{}, True{}, None{}, "", None{}}))
    case _:
      cx

def feature_retire.one(-o: S.St, +cx: X.Cx<o>, +a: S.Agent, +text: String) -> X.Cx<o>:
  +n = S.Agent.name(a)
  feature_retire.tell(o, S.Agent.life(a), emit(o, cx, M.EWorkspace{n, FT.unfeature(S.Agent.ws(a))}, Unit{}, Unit{}, Unit{}), n, text)

def feature_retire.all(-o: S.St, xs: List<&2, S.Agent>, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  match xs:
    case Nil{}:
      cx
    case Con{+a, +t}:
      feature_retire.all(o, t, feature_retire.one(o, cx, a, text), text)

def feature_retire(-o: S.St, +cx: X.Cx<o>, +name: String, +kept: String, +main: String) -> X.Cx<o>:
  +xs = FT.retirees(st(o, cx), name)
  +cx2 = feature_retire.all(o, xs, cx, FT.note(name, kept, main))
  feed(o, cx2, M.MAIN(), FD.info(FT.line(name, kept, main, xs)))

# ---- the user ----

def user_sent(-o: S.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  out(o, cx, F.user_sent(text))

# a card closed with his answer `a` (inbox-record): the journal keeps it
# for the inbox's history
def close_card_ans(-o: S.St, +cx: X.Cx<o>, +id: Nat, +res: String, +a: S.Ans) -> X.Cx<o>:
  +cx2 = emit(o, cx, M.ECardClosed{id, res, a}, Unit{}, Unit{}, Unit{})
  feed(o, cx2, M.MAIN(), FD.card_closed(id, res))

# a card closed without his answer (withdrawn, closed, stopped): its
# journal line carries no answer (law closed_without_him_has_no_answer)
def close_card(-o: S.St, +cx: X.Cx<o>, +id: Nat, +res: String) -> X.Cx<o>:
  close_card_ans(o, cx, id, res, S.Ans.empty())

# one of the card's options (picks.bend); its laws read it here
def is_option(+card: String, +t: String) -> Bool:
  PK.is_option(card, t)

# his answer `a` as its route line (inbox picks, architect m_22243): what
# he said as he said it, `msg` the message it became (the asker's thread
# folds it into this row), his pick when he picked an option by its number
def answer_rec(+a: PK.Answer, +msg: Maybe<&2, Nat>) -> J.Json:
  FD.route(PK.ans_who(a), Some{PK.ans_card(a)}, PK.ans_said(a), msg, PK.pick_n(PK.ans_pick(a)), PK.pick_words(PK.ans_pick(a)))

# that route line in `agent`'s feed, then its parts; the step's answer
# names the same answer (X.answered: inbox picks (3), architect m_22636)
def answer_line(-o: S.St, +cx: X.Cx<o>, +agent: String, +who: String, +c: S.Card, +said: String, +parts: String, +msg: Maybe<&2, Nat>) -> X.Cx<o>:
  +a = PK.ans(S.Card.id(c), S.Card.text(c), who, said)
  +cx2 = feed(o, X.answered(o, cx, a), agent, answer_rec(a, msg))
  LZ.pick(X.Cx<o>, String.is_empty(parts), u => cx2, u => feed(o, cx2, agent, FD.route_parts(parts_json(parts))))

# the same line in the asker's own feed (inbox picks (2)): an agent's
# card has it in both feeds, main's own in main's alone
def asker_line(-o: S.St, +cx: X.Cx<o>, +asker: String, +c: S.Card, +said: String, +parts: String, +msg: Maybe<&2, Nat>) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.eq(asker, M.MAIN()), u => cx, u => answer_line(o, cx, asker, asker, c, said, parts, msg))

def answer.q.sent(-o: S.St, p: X.Cx<o> & SendR, +c: S.Card, +asker: String, +text: String, +parts: String, +via: String) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      notice(o, cx, e)
    case (+cx, SOk{+m}):
      +id = S.Card.id(c)
      +mid = S.Msg.id(m)
      +cx2 = note_main(o, cx, cat(["the user answered notification #", ns(id), " (@", asker, " asked: \"", T.one_line(S.Card.text(c)), "\"): \"", T.one_line(text), "\""]))
      +cx3 = answer_line(o, cx2, M.MAIN(), asker, c, text, parts, Some{mid})
      close_card_ans(o, A.due_add(o, asker_line(o, cx3, asker, c, text, parts, Some{mid}), id), id, "answered", A.his_ans(c, text, via))

# main's own card: the answer wakes main with a turn that holds it, like
# a card for an agent reaches that agent (a main note would wait for the
# user's next message)

# the head of main's note when the user answers one of main's own cards
# (the clients match it to drop that note: lines.rs MAIN_ANSWER_HEAD)
def MAIN_ANSWER_HEAD() -> String:
  "the user answered notification #"

#
# The same route line as an agent's card (answer.q.sent), his answer whole:
# the clients fold that line into the "you answered" row and drop the
# notify's twin (lines.rs is_main_answer_note, its head MAIN_ANSWER_HEAD)
def answer.q.main(-o: S.St, +cx: X.Cx<o>, +c: S.Card, +text: String, +parts: String, +via: String) -> X.Cx<o>:
  +id = S.Card.id(c)
  +cx2 = notify_main(o, cx, cat([MAIN_ANSWER_HEAD(), ns(id), " (\"", T.one_line(S.Card.text(c)), "\"): \"", T.one_line(text), "\""]))
  +cx3 = answer_line(o, cx2, M.MAIN(), M.MAIN(), c, text, parts, None{})
  close_card_ans(o, A.due_add(o, cx3, id), id, "answered", A.his_ans(c, text, via))

# his answer as a message to the asker, his parts with it
def answer.snd(+to: String, +text: String, +reply_to: Maybe<&2, Nat>, +parts: String) -> Snd:
  Snd{M.USER(), to, text, False{}, reply_to, False{}, False{}, False{}, None{}, parts, None{}}

def answer.q(-o: S.St, m: Maybe<&2, S.Msg>, +cx: X.Cx<o>, +c: S.Card, +text: String, +parts: String, +via: String) -> X.Cx<o>:
  match m:
    case Some{+q}:
      answer.q.sent(o, send(o, cx, answer.snd(S.Msg.from(q), text, Some{S.Msg.id(q)}, parts)), c, S.Msg.from(q), text, parts, via)
    case None{}:
      # pr-news: a card the hub opened about an agent (checks that still
      # fail after two tries): the answer goes to that agent, main reads it
      +ag = S.Card.agent(c)
      LZ.pick(X.Cx<o>, Bool.not(String.eq(ag, M.MAIN())) && Maybe.is_some(&2, String, M.resolve(st(o, cx), ag)),
        u => answer.q.sent(o, send(o, cx, answer.snd(ag, text, None{}, parts)), c, ag, text, parts, via),
        u => answer.q.main(o, cx, c, text, parts, via))

def answer.dropped(-o: S.St, p: X.Cx<o> & DropR, +c: S.Card, +a: S.Ans) -> X.Cx<o>:
  match p:
    case (+cx, DErr{+e}):
      close_card_ans(o, notice(o, cx, e), S.Card.id(c), "accepted", a)
    case (+cx, _):
      close_card_ans(o, note_main(o, cx, String.append("the user accepted to drop @", S.Card.agent(c))), S.Card.id(c), "accepted", a)

def answer.drop(-o: S.St, yes: Bool, +cx: X.Cx<o>, +c: S.Card, +a: S.Ans) -> X.Cx<o>:
  match yes:
    case True{}:
      answer.dropped(o, drop_task(o, cx, S.Card.agent(c), True{}), c, a)
    case False{}:
      close_card_ans(o, note_main(o, cx, String.append("the user refused to drop @", S.Card.agent(c))), S.Card.id(c), "refused", a)

def answer.plain.sent(-o: S.St, p: X.Cx<o> & SendR, +id: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      notice(o, cx, e)
    case (+cx, SOk{_}):
      close_card(o, cx, id, "answered")

def is_yes(+t: String) -> Bool:
  +s = String.to_lower(String.trim(t))
  String.eq(s, "y") || String.eq(s, "yes") || String.eq(s, "o") || String.eq(s, "oui") || String.eq(s, "ok")

def card_kind(+k: String) -> CardK:
  Bool.pick(CardK, String.eq(k, "question"), KQuestion{},
    Bool.pick(CardK, String.eq(k, "drop"), KDrop{},
      Bool.pick(CardK, String.eq(k, "confirm"), KConfirm{},
        Bool.pick(CardK, CK.choice_kind(k), KChoice{}, KOther{}))))


# typed words on an approval card: the card stays open, nothing wakes
def words_refused(-o: S.St, +cx: X.Cx<o>, +c: S.Card, +opts: String) -> X.Cx<o>:
  notice(o, cx, cat(["notification #", ns(S.Card.id(c)), " waits for ", opts, " — typed words don't answer it"]))

def answer.confirm(-o: S.St, ok: Bool, +cx: X.Cx<o>, +c: S.Card, +text: String, +via: String) -> X.Cx<o>:
  match ok:
    case True{}:
      # the approvals gate (approvals-design.md §9): the answer is the
      # verdict of the waiting call, written to its gate by the hub's
      # Rust side (the `confirm` effect), never a message to the agent
      +cx2 = out(o, cx, F.confirm_card(S.Card.id(c), S.Card.agent(c), text))
      close_card_ans(o, cx2, S.Card.id(c), "answered", A.confirm_ans(text, via))
    case False{}:
      words_refused(o, cx, c, A.confirm_opts())

# a hub item's answer (CK.choice_kind): one of its numbers goes to the
# hub's Rust side (the `card_choice` effect), which acts and closes the
# item (a merge runs `gh pr merge`: the item stays open meanwhile); typed
# words don't answer it (the composer rule: the window sends them to the
# agent as a message)
def answer.choice(-o: S.St, num: Bool, +cx: X.Cx<o>, +c: S.Card, +text: String) -> X.Cx<o>:
  match num:
    case True{}:
      out(o, cx, F.card_choice(S.Card.id(c), S.Card.kind(c), S.Card.agent(c), S.Card.place(c), S.Card.pr(c), String.trim(text)))
    case False{}:
      words_refused(o, cx, c, "one of its numbers")

def answer.kind(-o: S.St, k: CardK, +cx: X.Cx<o>, +c: S.Card, +text: String, +parts: String, +via: String) -> X.Cx<o>:
  match k:
    case KQuestion{}:
      answer.q(o, for_msg_of(st(o, cx), S.Card.for_msg(c)), cx, c, text, parts, via)
    case KDrop{}:
      answer.drop(o, is_yes(text) || PK.picks(S.Card.text(c), text, 1n), cx, c, A.his_ans(c, text, via))
    case KConfirm{}:
      answer.confirm(o, A.confirm_ok(text), cx, c, text, via)
    case KChoice{}:
      answer.choice(o, is_option(S.Card.text(c), text), cx, c, text)
    case KOther{}:
      close_card_ans(o, cx, S.Card.id(c), "vue", A.said_ans(text, via))

# his late answer to the card main withdrew (`it`, hub/gone.bend), as a
# hub message to main: the question on one line, then his words whole,
# his parts on the message (law late_answer_keeps_its_parts)
def late.head(+it: S.Item) -> String:
  cat(["he answered #", ns(S.Item.id(it)), " after you withdrew it (\"", T.one_line(S.Item.text(it)), "\"):\n"])

def late.snd(+it: S.Item, +text: String, +parts: String) -> Snd:
  Snd{M.HUB(), M.MAIN(), String.append(late.head(it), text), False{}, None{}, False{}, False{}, False{}, None{}, parts, None{}}

# sent: card_late takes the card out of the list (law late_answer_once);
# not sent: the card stays answerable, he hears why
def answer.late.sent(-o: S.St, p: X.Cx<o> & SendR, +id: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      notice(o, cx, e)
    case (+cx, SOk{_}):
      emit(o, cx, M.ECardLate{id}, Unit{}, Unit{}, Unit{})

# his answer to a card that isn't open (withdrawn-answer, architect
# m_30039): a card main withdrew while he answered it (`g`, still in
# St.gone) reaches main, once, never dropped; any other card: refused
def answer.late(-o: S.St, g: Maybe<&2, S.Item>, +cx: X.Cx<o>, +id: Nat, +text: String, +parts: String) -> X.Cx<o>:
  match g:
    case None{}:
      notice(o, cx, cat(["notification #", ns(id), " isn't open"]))
    case Some{+it}:
      answer.late.sent(o, send(o, cx, late.snd(it, text, parts)), id)

def answer.card(-o: S.St, m: Maybe<&2, S.Card>, +cx: X.Cx<o>, +id: Nat, +text: String, +parts: String, +via: String) -> X.Cx<o>:
  match m:
    case None{}:
      answer.late(o, G.find(S.St.gone(st(o, cx)), id), cx, id, text, parts)
    case Some{+c}:
      answer.kind(o, card_kind(S.Card.kind(c)), cx, c, text, parts, via)

# his answer to card `id`; `parts`: his message's parts (msg-parts, ""
# none), kept on the asker's message and written after the route line;
# `via`: where he answered ("inbox": the inbox's answer; "thread": his
# words to the asker in its own view, says.card)
def answer_card(-o: S.St, +cx: X.Cx<o>, +id: Nat, +text: String, +parts: String, +via: String) -> X.Cx<o>:
  answer.card(o, M.card_of(S.St.cards(st(o, cx)), id), cx, id, text, parts, via)

def asked_by(m: Maybe<&2, S.Msg>, +to: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+q}:
      String.eq(S.Msg.from(q), to)

# RFC 0001 §7.3: answering a task that waits on the user, from its own
# view, answers its question
def question_card(cs: List<&2, S.Card>, +s: S.St, +to: String) -> Maybe<&2, Nat>:
  match cs:
    case Nil{}:
      None{}
    case Con{+c, +t}:
      LZ.pick(Maybe<&2, Nat>, String.eq(S.Card.kind(c), "question") && asked_by(for_msg_of(s, S.Card.for_msg(c)), to),
        u => Some{S.Card.id(c)}, u => question_card(t, s, to))

# at-start: his words typed in live agent `from`'s thread went to `to` as `id`: one route
# row there with that id and his parts (laws say_from_*); none from to's own thread.
# The || is strict, so get_agent(from) runs for from = "" too: fine, an
# empty name ends its lookup at once (M.never_agent, c4b-core-step-flat)
def says.trace(-o: S.St, +cx: X.Cx<o>, +from: String, +to: String, +id: Nat, +text: String, +parts: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.is_empty(from) || String.eq(from, to) || Bool.not(M.live_agent(M.get_agent(st(o, cx), from))), u => cx,
    u => route_parts.at(o, feed(o, cx, from, FD.route(to, None{}, text, Some{id}, None{}, None{})), from, parts))

def says.sent(-o: S.St, p: X.Cx<o> & SendR, +to: String, +text: String, +from: String, +parts: String) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      undelivered.if(o, unavailable_err(e), notice(o, cx, e), to, to, text)
    case (+cx, SOk{+m}):
      +cx2 = says.trace(o, cx, from, to, S.Msg.id(m), text, parts)
      LZ.pick(X.Cx<o>, String.eq(to, M.MAIN()), u => cx2, u => user_sent(o, cx2, text))

def says.card(-o: S.St, card: Maybe<&2, Nat>, +cx: X.Cx<o>, +to: String, +text: String, +queued: Bool, +parts: String, +from: String) -> X.Cx<o>:
  match card:
    case Some{+id}:
      answer_card(o, user_sent(o, cx, text), id, text, parts, "thread")
    case None{}:
      says.sent(o, send(o, cx, Snd{M.USER(), to, text, False{}, None{}, True{}, False{}, queued, None{}, parts, None{}}), to, text, from, parts)

# plain text from the user to the agent in focus. `queued` (the window's
# "send queued"): a queued-mode message, held while the agent is busy and
# said at its turn's end (law user_queued_waits_for_turn_end); it never
# answers a question card (law user_queued_never_answers_a_card).
# `parts`: his message's parts (msg-parts, "" none), kept on it; `from`: says.trace
def user_says(-o: S.St, +cx: X.Cx<o>, +to: String, +text: String, +queued: Bool, +parts: String, +from: String) -> X.Cx<o>:
  says.card(o, Bool.pick(Maybe<&2, Nat>, queued || String.eq(to, M.MAIN()), None{}, question_card(S.St.cards(st(o, cx)), st(o, cx), to)), cx, to, text, queued, parts, from)

def route.sent(-o: S.St, p: X.Cx<o> & SendR, +name: String, +text: String, +focus: String, +parts: String) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      undelivered.if(o, unavailable_err(e), notice(o, cx, e), focus, name, text)
    case (+cx, SOk{+m}):
      +cx2 = route_line(o, cx, name, None{}, text, parts)
      +cx3 = LZ.pick(X.Cx<o>, Bool.not(String.eq(focus, M.MAIN())) || Bool.not(String.eq(name, focus)),
        u => note_main(o, cx2, cat(["the user wrote directly to @", name, ": \"", T.one_line(text), "\""])), u => cx2)
      cx3

def route.target(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +target: String, +text: String, +focus: String, +queued: Bool, +parts: String) -> X.Cx<o>:
  match m:
    case None{}:
      +close = T.close_names(S.St.order(st(o, cx)), target)
      +hint = Bool.pick(String, List.is_empty(&2, String, close), "", cat([" — did you mean ", String.join(close, ", "), "?"]))
      notice(o, cx, cat(["no agent named @", target, hint]))
    case Some{+name}:
      # RFC 0003 §5.1: from another view, the task reads it tagged and
      # its answer comes back to that view
      +elsewhere = Bool.not(String.eq(name, focus))
      route.sent(o, send(o, cx, Snd{M.USER(), name, text, elsewhere, None{}, Bool.not(elsewhere), False{}, queued,
        Bool.pick(Maybe<&2, String>, elsewhere, Some{focus}, None{}), parts, None{}}), name, text, focus, parts)

# `@task text`: a direct message, without main; `queued` and `parts` as
# user_says
def route(-o: S.St, +cx: X.Cx<o>, +target: String, +text: String, +focus: String, +queued: Bool, +parts: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.eq(target, M.MAIN()), u => user_says(o, cx, M.MAIN(), text, queued, parts, ""),
    u => route.target(o, M.resolve(st(o, cx), target), cx, target, text, focus, queued, parts))

# ---- an MCP App view's context (docs/mcp-apps.md, C5, review d5) ----
# A view's `ui/update-model-context` is kept for its agent (the last one
# per view, runtime only: X.R.ctx) and joins his next message to that
# agent once, before his words, whatever client sent it (the Send and the
# Slash paths both reach the say and route inputs). An agent not active
# keeps none; a stop drops its contexts. The tag is the shell's render.

# kept or not, the step says it last (fx view_context, law
# view_context_says_kept): the shell answers the view from it
# why a view's context was not kept ("" kept)
def view_context.why(ok: Bool, +agent: String) -> String:
  match ok:
    case False{}:
      cat(["the view's context didn't reach @", agent, ": it is not active"])
    case True{}:
      ""

def view_context.go(-o: S.St, ok: Bool, +cx: X.Cx<o>, +c: S.Vctx) -> X.Cx<o>:
  match ok:
    case False{}:
      out(o, cx, F.view_context(S.Vctx.agent(c), S.Vctx.call(c), False{}, view_context.why(False{}, S.Vctx.agent(c))))
    case True{}:
      +cx2 = set_r(o, cx, X.R.with_ctx(rr(o, cx), VC.put(X.R.ctx(rr(o, cx)), c)))
      out(o, cx2, F.view_context(S.Vctx.agent(c), S.Vctx.call(c), True{}, ""))

def view_context(-o: S.St, +cx: X.Cx<o>, +agent: String, +call: String, +text: String, +part: J.Json) -> X.Cx<o>:
  view_context.go(o, live_agent(M.agent_named(S.St.agents(st(o, cx)), agent)), cx, S.Vctx.with_part(S.Vctx.with_text(S.Vctx.with_call(S.Vctx.with_agent(S.Vctx.empty(), agent), call), text), part))

# his message to `to`, its views' contexts before his words, taken once
def says_ctx(-o: S.St, +cx: X.Cx<o>, +to: String, +text: String, +queued: Bool, +parts: String, +from: String) -> X.Cx<o>:
  +cs = VC.of(X.R.ctx(rr(o, cx)), to)
  user_says(o, view_ctx_drop(o, cx, to), to, VC.join_text(cs, text), queued, VC.join_parts(cs, parts, text), from)

# his `@agent` message likewise (an agent it names: its contexts)
def route_ctx(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +target: String, +text: String, +focus: String, +queued: Bool, +parts: String) -> X.Cx<o>:
  match m:
    case None{}:
      route(o, cx, target, text, focus, queued, parts)
    case Some{+name}:
      +cs = VC.of(X.R.ctx(rr(o, cx)), name)
      route(o, view_ctx_drop(o, cx, name), target, VC.join_text(cs, text), focus, queued, VC.join_parts(cs, parts, text))

def user_new.res(-o: S.St, p: X.Cx<o> & TaskR) -> X.Cx<o>:
  match p:
    case (+cx, TErr{+e}):
      notice(o, cx, e)
    case (+cx, _):
      cx

def user_drop.res(-o: S.St, p: X.Cx<o> & DropR, +name: String) -> X.Cx<o>:
  match p:
    case (+cx, DDone{}):
      note_main(o, cx, String.append("the user dropped the agent @", name))
    case (+cx, DAsk{+q}):
      # his own /archive asks like main's drop: the 1/2 card (drop-card)
      open_card(o, cx, "drop", Maybe.default(&2, String, M.resolve(st(o, cx), name), name), PL.drop_asked(q), None{})
    case (+cx, DErr{+e}):
      notice(o, cx, e)
    case (+cx, DHalt{}):
      cx

def user_restore.res(-o: S.St, p: X.Cx<o> & TaskR) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      note_main(o, cx, String.append("the user restored the agent @", n))
    case (+cx, TErr{+e}):
      notice(o, cx, e)
    case (+cx, THalt{}):
      cx

def user_isolate.res(-o: S.St, p: X.Cx<o> & TaskR) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      note_main(o, cx, cat(["the user moved @", n, " into its own git worktree"]))
    case (+cx, TErr{+e}):
      notice(o, cx, e)
    case (+cx, THalt{}):
      cx

# the rename itself (the user's /rename and main's sb rename): take.bend
# frees the new name first (an archived agent's, an old name: name-free)
def rename.apply(-o: S.St, +cx: X.Cx<o>, +old: String, +new: String) -> X.Cx<o>:
  TK.rename_to(o, cx, old, new)

def rename.go(-o: S.St, ok: Bool, +cx: X.Cx<o>, +old: String, +new: String) -> X.Cx<o>:
  match ok:
    case False{}:
      notice(o, cx, String.append("invalid or taken name: ", new))
    case True{}:
      note_main(o, rename.apply(o, cx, old, new), cat(["the user renamed @", old, " to @", new, " (the old name still works)"]))

def rename.old(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +name: String, +new: String, +valid: Bool) -> X.Cx<o>:
  match m:
    case None{}:
      notice(o, cx, String.append("no agent named @", name))
    case Some{+old}:
      LZ.pick(X.Cx<o>, String.eq(old, M.MAIN()), u => notice(o, cx, String.append("no agent named @", name)),
        u => rename.go(o, valid && Bool.not(M.name_taken(st(o, cx), new)), cx, old, new))

# the typed fact of a stop (desktop reset, architect m_9650 D): in the same
# step as the interrupt, a 'stopped' line in that agent's feed; the thread's
# fold turns it into an entry of kind 'stopped' at its position
def stopped(-o: S.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  feed(o, cx, agent, FD.stopped("stopped"))

# the interrupt's effect for that agent (the shell flags its REPL)
def interrupt.fx(-o: S.St, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  out(o, cx, F.interrupt(n))

def interrupt.go(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case Some{+n}:
      LZ.pick(X.Cx<o>, is_busy(X.Rt.run(rt(o, cx, n))),
        u => stopped(o, interrupt.fx(o, cx, n), n),
        u => notice(o, cx, "no turn in progress to interrupt"))
    case None{}:
      notice(o, cx, "no turn in progress to interrupt")

# ---- the project's pause (cost-pause, hub/pause.bend) ----

# each busy turn stops: the interrupt and its 'stopped' line
def pause.stop(-o: S.St, ns: List<&2, String>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ns:
    case Nil{}:
      cx
    case Con{+n, +rest}:
      # its thread says why (designer m_18789), and bise's go-on note
      # waits in its queue for /resume (held like the rest)
      +cx2 = feed(o, stopped(o, interrupt.fx(o, cx, n), n), n, FD.paused("‖ paused · its turn stopped"))
      pause.stop(o, rest, fst(o, send(o, cx2, msg(M.HUB(), n, P.GO_ON(), False{}, None{}, False{}, False{}))))

# `/pause`: journaled, then every busy turn stops; already paused:
# nothing changes (law pause_idempotent)
def pause.go(-o: S.St, already: Bool, +cx: X.Cx<o>) -> X.Cx<o>:
  match already:
    case True{}:
      notice(o, cx, P.ALREADY())
    case False{}:
      +ns = P.busy(st(o, cx), X.R.rts(rr(o, cx)))
      +cx2 = emit(o, cx, M.EPaused{now(o, cx)}, Unit{}, Unit{}, Unit{})
      notice(o, dirty(o, pause.stop(o, ns, cx2)), P.paused_text(List.length(&2, String, ns)))

def pause(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  pause.go(o, P.is_paused(st(o, cx)), cx)

def resume.pump(-o: S.St, ns: List<&2, String>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ns:
    case Nil{}:
      cx
    case Con{+n, +rest}:
      resume.pump(o, rest, pump(o, cx, n))

# `/resume`: journaled, then every agent gets what was held, in the hub's
# order (law resume_delivers_held); not paused: nothing changes
def resume.go(-o: S.St, paused: Bool, +cx: X.Cx<o>) -> X.Cx<o>:
  match paused:
    case False{}:
      notice(o, cx, P.NOT_PAUSED())
    case True{}:
      +agents = P.woken(st(o, cx))
      +his = P.his(st(o, cx))
      +cx2 = emit(o, cx, M.EResumed{now(o, cx)}, Unit{}, Unit{}, Unit{})
      notice(o, dirty(o, resume.pump(o, S.St.order(st(o, cx2)), cx2)), P.resumed_text(agents, his))

def resume(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  resume.go(o, P.is_paused(st(o, cx)), cx)

# his message `id`, still queued, taken back: Rejected{"taken_back"},
# never delivered (an older sb-core replays it so); the queue loses it
def unqueue.take(-o: S.St, s: S.MSt, +cx: X.Cx<o>, +id: Nat, +to: String) -> X.Cx<o>:
  match s:
    case S.Queued{_}:
      emit(o, cx, M.EMsgState{id, S.Rejected{"taken_back"}}, Unit{}, Unit{}, Unit{})
    case S.Delivered{}:
      notice(o, cx, cat([mid(id), " already reached @", to, ": it can't be taken back"]))
    case S.Rejected{_}:
      notice(o, cx, cat([mid(id), " isn't waiting any more"]))
    case S.Cancelled{}:
      notice(o, cx, cat([mid(id), " isn't waiting any more"]))

def unqueue.go(-o: S.St, m: Maybe<&2, S.MRec>, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      notice(o, cx, cat(["no message ", mid(id), " to take back"]))
    case Some{+r}:
      +msg = S.MRec.msg(r)
      LZ.pick(X.Cx<o>, String.eq(S.Msg.from(msg), M.USER()),
        u => unqueue.take(o, S.MRec.st(r), cx, id, S.Msg.to(msg)),
        u => notice(o, cx, cat([mid(id), " isn't one of your messages"])))

# a record's state once set to `x` (none stays none): the law of a
# take-back's own record (unqueue_marks_its_own_record)
def st_after(m: Maybe<&2, S.MRec>, +x: S.MSt) -> Maybe<&2, S.MSt>:
  match m:
    case None{}:
      None{}
    case Some{_}:
      Some{x}

# the user takes back a queued message of his (the window's "edit · drop":
# HubCmd queued_take, the client socket only, issue 16): only a queued
# message from the user changes (law unqueue_takes_back_only_his_queued);
# anything else is a notice saying why, and nothing changes
def unqueue(-o: S.St, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  unqueue.go(o, M.rec_at(st(o, cx), id), cx, id)

# his queued message said now (the window's 'send now', architect
# m_25139): the pump's own delivery of that one message, its id and parts
# kept, steered into the running turn or a turn of its own (laws
# steer_queued_*); the agent can't take it (down, starting, gone, the
# project paused): a notice, and it stays queued
def steer_queued.run(-o: S.St, run: S.Run, +cx: X.Cx<o>, +m: S.Msg, +name: String, +is_main: Bool) -> X.Cx<o>:
  match run:
    case S.RBusy{}:
      pump.batch(o, [m], cx, name, True{}, is_main)
    case S.RIdle{}:
      pump.batch(o, [m], cx, name, False{}, is_main)
    case _:
      notice(o, cx, cat([mid(S.Msg.id(m)), " stays queued: @", name, " isn't running"]))

def steer_queued.agent(-o: S.St, a: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +m: S.Msg) -> X.Cx<o>:
  match a:
    case None{}:
      notice(o, cx, cat([mid(S.Msg.id(m)), " stays queued: @", S.Msg.to(m), " is gone"]))
    case Some{+g}:
      LZ.pick(X.Cx<o>, is_active(S.Agent.life(g)) && Bool.not(P.is_paused(st(o, cx))),
        u => steer_queued.run(o, X.Rt.run(rt(o, cx, S.Agent.name(g))), cx, m, S.Agent.name(g), S.Agent.is_main(g)),
        u => notice(o, cx, cat([mid(S.Msg.id(m)), " stays queued: @", S.Agent.name(g), " can't take it now"])))

def steer_queued.take(-o: S.St, s: S.MSt, +cx: X.Cx<o>, +m: S.Msg) -> X.Cx<o>:
  match s:
    case S.Queued{_}:
      +to = S.Msg.to(m)
      steer_queued.agent(o, M.get_agent(st(o, cx), Maybe.default(&2, String, M.resolve(st(o, cx), to), to)), cx, m)
    case _:
      notice(o, cx, cat([mid(S.Msg.id(m)), " isn't waiting any more"]))

def steer_queued.go(-o: S.St, r: Maybe<&2, S.MRec>, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  match r:
    case None{}:
      notice(o, cx, cat(["no message ", mid(id), " to send now"]))
    case Some{+x}:
      LZ.pick(X.Cx<o>, String.eq(S.Msg.from(S.MRec.msg(x)), M.USER()),
        u => steer_queued.take(o, S.MRec.st(x), cx, S.MRec.msg(x)),
        u => notice(o, cx, cat([mid(id), " isn't one of your messages"])))

def steer_queued(-o: S.St, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  steer_queued.go(o, M.rec_at(st(o, cx), id), cx, id)

def passthrough(-o: S.St, +cx: X.Cx<o>, +focus: String, +l: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, has_agent(o, cx, focus) && is_idle(X.Rt.run(rt(o, cx, focus))),
    u => out(o, dirty(o, set_run(o, cx, focus, S.RBusy{})), F.passthrough(focus, l)),
    u => notice(o, cx, "the agent is not idle: retry at the end of its turn"))

# the daemon computed what main learns about a direct exchange (RFC
# 0001 §7.4)
def focus(-o: S.St, +cx: X.Cx<o>, +f: String, note: Maybe<&2, String>, +direct: String) -> X.Cx<o>:
  match note:
    case Some{+n}:
      feed(o, note_main(o, cx, n), M.MAIN(), FD.direct(direct))
    case None{}:
      cx

# ---- cross-hub (S2, hub/xhub.bend) ----
# The steps of bise's cross-hub messages, wired around xhub.bend's pure
# decisions: they call send, emit, out and reply, which are core's, and
# xhub.bend cannot import core (Bend has no import cycles). Nothing is
# decided here. Home side (bise's hub): `sb project send|ask` puts an
# entry in the outbox and asks the hub for an `xdeliver`; the delivery's
# ack or failure (a retry act, timers.bend's one clock) and the project's
# answer (to main, from `@<project>`) come back as inputs. Project side:
# `x_in` says it to main once per origin, as the user's words via bise
# or as `@bise`'s question; main's answer (the end of its turn, or
# `sb send @bise --reply-to`) goes back once, as an `xreply`.

def xst(-o: S.St, +cx: X.Cx<o>) -> S.Xs:
  S.St.x(st(o, cx))

def xemit(-o: S.St, +cx: X.Cx<o>, +e: XH.XEv) -> X.Cx<o>:
  emit(o, cx, M.EX{e}, Unit{}, Unit{}, Unit{})

def aemit(-o: S.St, +cx: X.Cx<o>, +e: TM.ActEv) -> X.Cx<o>:
  emit(o, cx, M.EAct{e}, Unit{}, Unit{}, Unit{})

def xdeliver(-o: S.St, +cx: X.Cx<o>, +ob: S.Out) -> X.Cx<o>:
  out(o, cx, F.xdeliver(S.Out.xid(ob), S.Out.project(ob), SC.XKind.word(S.Out.kind(ob)), S.Out.msg(ob), S.Out.text(ob), S.Out.context(ob), S.Out.parts(ob)))

# home: a new outbox entry, journaled, then its first delivery
def xsend.new(-o: S.St, +cx: X.Cx<o>, +token: Nat, +ob: S.Out) -> X.Cx<o>:
  +cx2 = xdeliver(o, xemit(o, cx, XH.XOut{ob}), ob)
  reply(o, cx2, token, ok_body([J.kv("xid", J.JNum{S.Out.xid(ob)}), J.kv("project", J.JStr{S.Out.project(ob)})]))

def xsend.fwd(-o: S.St, f: XB.Fwd, +cx: X.Cx<o>, +token: Nat, +project: String, +id: Nat) -> X.Cx<o>:
  match f:
    case XB.FwdErr{+e}:
      reply_err(o, cx, token, e)
    case XB.FwdOk{+text}:
      xsend.new(o, cx, token, S.Out.with_text(S.Out.with_msg(XB.out_to(xst(o, cx), project, S.KInput{}), id), text))

# `sb project send <p> --input m_<n>` (main only, req.main): `pre_err`,
# the hub's (an unknown project, this hub itself)
def req_xsend(-o: S.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match pre:
    case Some{+e}:
      reply_err(o, cx, token, e)
    case None{}:
      xsend.fwd(o, XB.forward(st(o, cx), M.MAIN(), K.get_n(q, "msg")), cx, token, K.get_s(q, "project"), K.get_n(q, "msg"))

def req_xask.go(-o: S.St, err: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match err:
    case Some{+e}:
      reply_err(o, cx, token, e)
    case None{}:
      xsend.new(o, cx, token, S.Out.with_text(XB.out_to(xst(o, cx), K.get_s(q, "project"), S.KAsk{}), K.get_s(q, "text")))

# `sb project ask <p> "<question>"` (main only): bise's own words
def req_xask(-o: S.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  req_xask.go(o, Maybe.or(&2, String, pre, XB.ask_ok(K.get_s(q, "text"))), cx, token, q)

# home: the project hub has it (a late ack of a settled send: nothing)
def xack(-o: S.St, m: Maybe<&2, S.Out>, +cx: X.Cx<o>, +xid: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{_}:
      xemit(o, cx, XH.XOp{xid, XH.OpAcked{}})

# home: a delivery failed: one more try, and the next one's act
def xfail(-o: S.St, m: Maybe<&2, S.Out>, +cx: X.Cx<o>, +xid: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+ob}:
      +cx2 = xemit(o, cx, XH.XOp{xid, XH.OpTried{}})
      aemit(o, cx2, TM.ActSet{XB.retry_act(tms(o, cx2), xid, S.Out.tries(ob), now(o, cx2))})

def xanswer.kind(k: S.XKind, +ob: S.Out, +text: String) -> String:
  match k:
    case S.KInput{}:
      cat(["(answer to the user's ", mid(S.Out.msg(ob)), " you forwarded) ", text])
    case S.KAsk{}:
      cat(["(answer to your question \"", T.clip(T.one_line(S.Out.text(ob)), 200n), "\") ", text])
    case S.KJobEnd{}:
      text

# home: the project's answer settles its send, once (law
# xhub_outbox_settles_once) and reaches main from `@<project>`
def xanswer(-o: S.St, m: Maybe<&2, S.Out>, +cx: X.Cx<o>, +xid: Nat, +name: String, +text: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+ob}:
      +cx2 = xemit(o, cx, XH.XOp{xid, XH.OpSettled{}})
      fst(o, send(o, cx2, msg(String.append("@", name), M.MAIN(), xanswer.kind(S.Out.kind(ob), ob, text), False{}, None{}, False{}, False{})))

def xretry(-o: S.St, m: Maybe<&2, S.Out>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+ob}:
      xdeliver(o, cx, ob)

# ---- the routing hold (S2 step 2) ----
# His words he addressed to a project (input route_ask, XB.addressed) wait
# HOLD_MS on the one clock (a RouteSend act): the window shows them (fx
# route) and he may correct the target or cancel. At the fire they become
# his message to main (an id, recorded, never a turn: law
# route_fire_makes_no_turn) forwarded by the outbox, and main gets a note
# with the id. A cancel says them to main as today.

def route_ev(-o: S.St, +cx: X.Cx<o>, +r: S.Route) -> X.Cx<o>:
  out(o, cx, F.route(S.Route.rid(r), S.Route.to(r), S.Route.name(r), S.Route.text(r), S.Route.due(r), S.Route.why(r)))

def route_done(-o: S.St, +cx: X.Cx<o>, +rid: Nat, +state: String, +to: String, +xid: Maybe<&2, Nat>, +msg: Maybe<&2, Nat>) -> X.Cx<o>:
  out(o, cx, F.route_done(rid, state, to, xid, msg))

# home: his words for project `to` (`d`, XB.route_for): held, act set, told
def route_held(-o: S.St, +cx: X.Cx<o>, +d: S.Route) -> X.Cx<o>:
  +r = XB.route_of_hold(xst(o, cx), tms(o, cx), d, now(o, cx))
  +cx2 = xemit(o, cx, XH.XRouteHeld{r})
  route_ev(o, aemit(o, cx2, TM.ActSet{XB.route_act(r)}), r)

# input route_ask (issue 37 #1): held for `m` (XB.addressed) or not, then
# one fx route_asked says which, all the shell reads (laws/route.bend).
# `text` (whole, `@<project>` in), `parts` (R41 (6)) go to the project.
def route_ask(-o: S.St, m: Maybe<&2, XB.Proj()>, +cx: X.Cx<o>, +text: String, +via: String, +context: String, +parts: String) -> X.Cx<o>:
  match m:
    case None{}:
      out(o, cx, F.route_asked(False{}))
    case Some{+p}:
      out(o, route_held(o, cx, XB.route_for(p, text, via, context, parts)), F.route_asked(True{}))

# a held route with no target (left by an older hub's route model in its
# journal) whose act fired: it ends, and his words reach bise's main as a
# plain message (the cancel path, no route_done: the window never saw it)
def route_plain(-o: S.St, +cx: X.Cx<o>, +r: S.Route) -> X.Cx<o>:
  +cx2 = xemit(o, cx, XH.XRoute{S.Route.rid(r), XH.REnd{S.RCancelled{}}})
  says.sent(o, send(o, cx2, Snd{M.USER(), M.MAIN(), S.Route.text(r), False{}, None{}, True{}, False{}, False{}, XB.via_of(S.Route.via(r)), S.Route.parts(r), None{}}), M.MAIN(), S.Route.text(r), "", "")

# his routed words in bise's main thread, where he wrote them: his line,
# its id, his parts, then where they went (the window and the TUI fold
# the `routed` record onto his entry by the id; law route_fire_says_where)
def route_fire.lines(-o: S.St, +cx: X.Cx<o>, +r: S.Route, +id: Nat, +xid: Nat) -> X.Cx<o>:
  +cx2 = feed(o, feed(o, cx, M.MAIN(), FD.you(S.Route.text(r), XB.via_of(S.Route.via(r)))), M.MAIN(), FD.you_id(id))
  feed(o, you_parts(o, cx2, M.MAIN(), S.Route.parts(r)), M.MAIN(), FD.routed(S.Route.to(r), S.Route.name(r), xid, id))

def route_fire.sent(-o: S.St, +cx: X.Cx<o>, +r: S.Route, +id: Nat) -> X.Cx<o>:
  +cx2 = emit(o, mark_delivered(o, cx, id), M.ESettled{id}, Unit{}, Unit{}, Unit{})
  +ob = XB.out_of_route(xst(o, cx2), r, id)
  +cx3 = xdeliver(o, xemit(o, route_fire.lines(o, cx2, r, id, S.Out.xid(ob)), XH.XOut{ob}), ob)
  +cx4 = xemit(o, cx3, XH.XRoute{S.Route.rid(r), XH.REnd{S.RSent{}}})
  +cx5 = route_done(o, cx4, S.Route.rid(r), "sent", S.Route.to(r), Some{S.Out.xid(ob)}, Some{id})
  note_main(o, cx5, cat(["the user's words went to the project ", S.Route.name(r), " (", mid(id), ", routed by bise): \"",
    T.clip(T.one_line(S.Route.text(r)), 300n), "\"; its answer comes back to you from @", S.Route.name(r)]))

# his routed words as his message to main, journaled only: no send (a
# send would pump main, a turn), no waiter, no card (law
# route_fire_makes_no_turn)
def route_said(-o: S.St, +cx: X.Cx<o>, +r: S.Route) -> X.Cx<o>:
  +msg = XB.msg_of_route(r, S.St.next_msg(st(o, cx)), now(o, cx))
  emit(o, cx, M.EMsgSent{msg}, {==}, quiet_ok(False{}, False{}), Unit{})

# home: a held route's act fired: his message to main is created now (an
# id for the history and `--input`), delivered and settled at once with
# no turn (law route_fire_makes_no_turn), then forwarded once
def route_fire(-o: S.St, m: Maybe<&2, S.Route>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+r}:
      route_fire.sent(o, route_said(o, cx, r), r, S.St.next_msg(st(o, cx)))

def route_gone(-o: S.St, +cx: X.Cx<o>, +rid: Nat) -> X.Cx<o>:
  notice(o, cx, cat(["route ", Nat.show(rid), " already went or was cancelled"]))

# home: the input route_cancel: a held route ends, its act with it, and
# his words reach main as a plain message (today's path)
def route_cancel.go(-o: S.St, m: Maybe<&2, S.Route>, +cx: X.Cx<o>, +rid: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      route_gone(o, cx, rid)
    case Some{+r}:
      +cx2 = aemit(o, cx, TM.ActDone{S.Route.act(r)})
      +cx3 = xemit(o, cx2, XH.XRoute{rid, XH.REnd{S.RCancelled{}}})
      +cx4 = route_done(o, cx3, rid, "cancelled", S.Route.to(r), None{}, None{})
      says.sent(o, send(o, cx4, Snd{M.USER(), M.MAIN(), S.Route.text(r), False{}, None{}, True{}, False{}, False{}, XB.via_of(S.Route.via(r)), S.Route.parts(r), None{}}), M.MAIN(), S.Route.text(r), "", "")

def route_cancel(-o: S.St, +cx: X.Cx<o>, +rid: Nat) -> X.Cx<o>:
  route_cancel.go(o, XB.held(xst(o, cx), rid), cx, rid)

def route_correct.go(-o: S.St, m: Maybe<&2, S.Route>, +cx: X.Cx<o>, +rid: Nat, +to: String, +name: String) -> X.Cx<o>:
  match m:
    case None{}:
      route_gone(o, cx, rid)
    case Some{+r}:
      route_ev(o, xemit(o, cx, XH.XRoute{rid, XH.RTo{to, name}}), XH.route_to(r, to, name))

# home: the input route_correct (the shell checked `to` in the registry):
# a held route gets its new project, same due time; `bise` keeps his
# words here (a cancel); a route not held: a notice (law
# route_correct_only_held)
def route_correct(-o: S.St, +cx: X.Cx<o>, +rid: Nat, +to: String, +name: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.eq(to, XB.VIA_BISE()),
    u => route_cancel(o, cx, rid),
    u => route_correct.go(o, XB.held(xst(o, cx), rid), cx, rid, to, name))

# a held route's act fired: with no target (an older hub's), plain to
# main (law route_unclear_goes_plain); else forwarded (route_fire)
def route_go(-o: S.St, m: Maybe<&2, S.Route>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+r}:
      LZ.pick(X.Cx<o>, XB.is_unclear(r),
        u => route_plain(o, cx, r),
        u => route_fire(o, Some{r}, cx))

# ---- an MCP App view's call (docs/mcp-apps.md, C5, risk 3) ----
# A view's call to a tool the model sees too asks him like the agent's
# own call, but outside the agent's turn: a `confirm` card placed
# `view:<agent>/<call>`, never the agent's runtime (no wait, no run
# change: law view_card_keeps_runtime); his answer goes out as today's
# `confirm` effect, which the hub's Rust side gives to that view's call by
# the card's id. Left unanswered, its due act closes it once as timed
# out, with a no for the call (laws view_card_due_*).

# how long a view's card waits for him: bise_proto's VIEW_CARD_MS, the one
# deadline of a view's call (the shell's and the clients' bounds derive
# from it, view_rpc::VIEW_CALL_MAX_MS; d3)
def VIEW_CARD_MS() -> Nat:
  K.VIEW_CARD_MS()

def view_place(+agent: String, +call: String) -> String:
  cat(["view:", agent, "/", call])

# the answer a view's card gets at its due: a no, as the gate reads it
def VIEW_CARD_LATE() -> String:
  "no: nobody answered in time"

# the answer a view's card left open gets at boot: its call ended with
# the hub that ran it
def VIEW_CARD_ENDED() -> String:
  "no: the view's call ended with the hub"

# what a view's card asks: its view (agent, call) and the shell's ask
def view_ask(+agent: String, +call: String, +ask: String) -> S.ViewAsk:
  S.ViewAsk.with_ask(S.ViewAsk.with_call(S.ViewAsk.with_agent(S.ViewAsk.empty(), agent), call), ask)

# a view's call's card in his inbox: `confirm`, placed view:<agent>/<call>
# (shown only), its ask typed (law view_card_typed)
def view_card_new(+id: Nat, +text: String, +v: S.ViewAsk, +now: Nat) -> S.Card:
  +c = S.Card.with_kind(S.Card.with_id(S.Card.empty(), id), "confirm")
  +c2 = S.Card.with_created(S.Card.with_text(S.Card.with_agent(c, S.ViewAsk.agent(v)), text), now)
  S.Card.with_view(S.Card.with_place(c2, Some{view_place(S.ViewAsk.agent(v), S.ViewAsk.call(v))}), Some{v})

# the card id a view's call waits on, said to the shell (fx view_card):
# none when no card opened (law view_card_says_its_id)
def view_card_said(-o: S.St, +cx: X.Cx<o>, +v: S.ViewAsk, +card: Maybe<&2, Nat>) -> X.Cx<o>:
  out(o, cx, F.view_card(S.ViewAsk.agent(v), S.ViewAsk.call(v), card))

def view_card_act(+tm: S.Tms, +card: Nat, +now: Nat) -> S.Act:
  TM.act_of(S.Tms.next_act(tm), Nat.add(now, VIEW_CARD_MS()), S.CardDue{card})

# a view's ask is that card's: its agent, call and ask (the tool and its
# arguments) all the same
def view_is(m: Maybe<&2, S.ViewAsk>, +v: S.ViewAsk) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      String.eq(S.ViewAsk.agent(a), S.ViewAsk.agent(v)) && String.eq(S.ViewAsk.call(a), S.ViewAsk.call(v)) && String.eq(S.ViewAsk.ask(a), S.ViewAsk.ask(v))

# the open card of that ask, if any (law view_card_of_its_ask)
def view_card_of(cs: List<&2, S.Card>, +v: S.ViewAsk) -> Maybe<&2, S.Card>:
  match cs:
    case Nil{}:
      None{}
    case Con{+c, +rest}:
      LZ.pick(Maybe<&2, S.Card>, view_is(S.Card.view(c), v), u => Some{c}, u => view_card_of(rest, v))

# a new view card: journaled, in main's feed, its due set...
def open_view_confirm.card(-o: S.St, +cx: X.Cx<o>, +v: S.ViewAsk, +text: String) -> X.Cx<o>:
  +id = S.St.next_card(st(o, cx))
  +cx2 = emit(o, cx, M.ECardOpened{view_card_new(id, text, v, now(o, cx))}, {==}, Unit{}, Unit{})
  +cx3 = feed(o, cx2, M.MAIN(), FD.card(id, "confirm", S.ViewAsk.agent(v), text))
  aemit(o, cx3, TM.ActSet{view_card_act(tms(o, cx3), id, now(o, cx3))})

# ...and its id said (law view_card_says_its_id)
def open_view_confirm.new(-o: S.St, +cx: X.Cx<o>, +v: S.ViewAsk, +text: String) -> X.Cx<o>:
  view_card_said(o, open_view_confirm.card(o, cx, v, text), v, Some{S.St.next_card(st(o, cx))})

# the same ask while its card is open (a page asking again after a
# reload): no second card, the open one's id said; the shell's waiters on
# it get his one answer (d3, law view_card_joins_same_ask)
def open_view_confirm.at(-o: S.St, m: Maybe<&2, S.Card>, +cx: X.Cx<o>, +v: S.ViewAsk, +text: String) -> X.Cx<o>:
  match m:
    case None{}:
      open_view_confirm.new(o, cx, v, text)
    case Some{+c}:
      view_card_said(o, cx, v, Some{S.Card.id(c)})

# an agent not active (stopped, archived, gone): no card, the hub says so
# and the view's call gets a no (laws view_card_needs_active,
# view_card_says_no_card)
def open_view_confirm.go(-o: S.St, ok: Bool, +cx: X.Cx<o>, +v: S.ViewAsk, +text: String) -> X.Cx<o>:
  match ok:
    case False{}:
      view_card_said(o, notice(o, cx, cat(["the view's call didn't reach @", S.ViewAsk.agent(v), ": it is not active"])), v, None{})
    case True{}:
      open_view_confirm.at(o, view_card_of(S.St.cards(st(o, cx)), v), cx, v, text)

def open_view_confirm(-o: S.St, +cx: X.Cx<o>, +agent: String, +call: String, +text: String, +ask: String) -> X.Cx<o>:
  open_view_confirm.go(o, live_agent(M.agent_named(S.St.agents(st(o, cx)), agent)), cx, view_ask(agent, call, ask), text)

# boot: a view's card left open has no call behind it any more (its hub
# ended): it gets its no, the `confirm` card_due sends, and closes (law
# boot_closes_view_cards); an answered or closed card is not among them,
# so a view's call is answered once
def view_ends.one(-o: S.St, hit: Bool, +cx: X.Cx<o>, +c: S.Card) -> X.Cx<o>:
  match hit:
    case True{}:
      close_card(o, out(o, cx, F.confirm_card(S.Card.id(c), S.Card.agent(c), VIEW_CARD_ENDED())), S.Card.id(c), "ended with the hub")
    case False{}:
      cx

def view_ends(-o: S.St, cs: List<&2, S.Card>, +cx: X.Cx<o>) -> X.Cx<o>:
  match cs:
    case Nil{}:
      cx
    case Con{+c, +rest}:
      view_ends(o, rest, view_ends.one(o, is_view(S.Card.view(c)), cx, c))

# a view card's due: still open, it closes as timed out and its call gets
# a no (the `confirm` effect); answered or closed before, nothing
def card_due(-o: S.St, m: Maybe<&2, S.Card>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+c}:
      +cx2 = out(o, cx, F.confirm_card(S.Card.id(c), S.Card.agent(c), VIEW_CARD_LATE()))
      close_card(o, cx2, S.Card.id(c), "timed out")

def act_fire(-o: S.St, w: S.What, +cx: X.Cx<o>) -> X.Cx<o>:
  match w:
    case S.Retry{+xid}:
      xretry(o, XB.pending(xst(o, cx), xid), cx)
    case S.RouteSend{+rid}:
      route_go(o, XB.held(xst(o, cx), rid), cx)
    case S.CardDue{+card}:
      card_due(o, M.card_of(S.St.cards(st(o, cx)), card), cx)

def acts_tick.each(-o: S.St, xs: List<&2, S.Act>, +cx: X.Cx<o>) -> X.Cx<o>:
  match xs:
    case Nil{}:
      cx
    case Con{+a, +rest}:
      acts_tick.each(o, rest, act_fire(o, S.Act.what(a), aemit(o, cx, TM.ActDone{S.Act.id(a)})))

# the acts due at the tick fire once each (laws act_fires_once,
# act_due_only); none due: nothing (idle-cpu)
def acts_tick(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  acts_tick.each(o, TM.acts_due(S.Tms.acts(tms(o, cx)), now(o, cx)), cx)

def xboot.each(-o: S.St, xs: List<&2, S.Out>, +cx: X.Cx<o>) -> X.Cx<o>:
  match xs:
    case Nil{}:
      cx
    case Con{+ob, +rest}:
      xboot.each(o, rest, xdeliver(o, cx, ob))

# home: at boot the pending sends go again (the project drops a second
# copy: law xhub_xin_delivered_once)
def xboot(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  xboot.each(o, XB.pendings(S.Xs.outbox(xst(o, cx))), cx)

# `parts`: his routed message's (R41 (6), "" none), kept on his message
# here like a direct input's; bise's own question and a job's end: none
def xin.snd(k: S.XKind, +text: String, +parts: String) -> Snd:
  match k:
    case S.KInput{}:
      Snd{M.USER(), M.MAIN(), text, True{}, None{}, False{}, False{}, False{}, Some{XB.VIA_BISE()}, parts, None{}}
    case S.KAsk{}:
      Snd{XB.FROM_BISE(), M.MAIN(), text, True{}, None{}, False{}, False{}, False{}, None{}, "", None{}}
    case S.KJobEnd{}:
      Snd{XB.FROM_BISE(), M.MAIN(), text, False{}, None{}, False{}, False{}, False{}, None{}, "", None{}}

def xin.sent(-o: S.St, p: X.Cx<o> & SendR, +token: Nat, +hub: String, +xid: Nat, +k: S.XKind) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      reply(o, xemit(o, cx, XH.XGot{S.XIn.with_kind(S.XIn.with_msg(XH.xin_from(hub, xid), S.Msg.id(m)), k)}), token, ok_body(Nil{}))

# J, bise's home hub: the line a followed task's end says in bise's main,
# from `@<project>`: the same words as a local task's report to main
def xjob_end.line(+j: J.Json) -> String:
  cat(["[report: ", K.get_s(j, "state"), "] @", K.get_s(j, "agent"), " (followed): ", String.trim(K.get_s(j, "summary"))])

def xjob_end.sent(-o: S.St, p: X.Cx<o> & SendR, +token: Nat, +hub: String, +xid: Nat, +j: J.Json) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      +cx2 = xemit(o, cx, XH.XGot{S.XIn.with_replied(S.XIn.with_kind(S.XIn.with_msg(XH.xin_from(hub, xid), S.Msg.id(m)), S.KJobEnd{}), True{})})
      +cx3 = out(o, cx2, F.followed_end(hub, K.get_s(j, "agent"), K.get_n(j, "key"), K.get_s(j, "state"), K.get_s(j, "label"), K.get_s(j, "summary")))
      reply(o, cx3, token, ok_body(Nil{}))

# J, bise's home hub: a task it follows in the project `name` ended: one
# line in bise's main from `@<name>`, a message like a local task's end
# report (it wakes main by the same rule: main tells him), recorded as
# received once (law xhub_xin_delivered_once), and fx followed_end for
# the window
def xjob_end(-o: S.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +token: Nat, +hub: String, +xid: Nat, +name: String) -> X.Cx<o>:
  match r:
    case Fail{+e}:
      reply_err(o, cx, token, cat(["a job_end that isn't JSON: ", e]))
    case Done{+j}:
      xjob_end.sent(o, send(o, cx, msg(String.append("@", name), M.MAIN(), xjob_end.line(j), False{}, None{}, False{}, False{})), token, hub, xid, j)

def xin.kind(-o: S.St, k: S.XKind, +cx: X.Cx<o>, +token: Nat, +hub: String, +xid: Nat, +text: String, +name: String, +parts: String) -> X.Cx<o>:
  match k:
    case S.KJobEnd{}:
      xjob_end(o, J.parse(text), cx, token, hub, xid, name)
    case S.KInput{}:
      xin.sent(o, send(o, cx, xin.snd(S.KInput{}, text, parts)), token, hub, xid, S.KInput{})
    case S.KAsk{}:
      xin.sent(o, send(o, cx, xin.snd(S.KAsk{}, text, "")), token, hub, xid, S.KAsk{})

# project: bise's message, said to main once per origin (hub, xid); bise's
# home hub: a followed task's end (kind job_end, `name` the project's)
def xin(-o: S.St, seen: Bool, +cx: X.Cx<o>, +token: Nat, +hub: String, +xid: Nat, +k: S.XKind, +text: String, +name: String, +parts: String) -> X.Cx<o>:
  match seen:
    case True{}:
      reply(o, cx, token, ok_body([J.kv("seen", J.JBool{True{}})]))
    case False{}:
      xin.kind(o, k, cx, token, hub, xid, text, name, parts)

# project: main answered bise's message: settled here, sent back once
# (law xhub_xin_replied_once)
def xreplied(-o: S.St, +cx: X.Cx<o>, +i: S.XIn, +text: String) -> X.Cx<o>:
  +cx2 = emit(o, cx, M.ESettled{S.XIn.msg(i)}, Unit{}, Unit{}, Unit{})
  +cx3 = xemit(o, cx2, XH.XReplied{S.XIn.hub(i), S.XIn.xid(i)})
  out(o, cx3, F.xreply(S.XIn.hub(i), S.XIn.xid(i), text))

def xreply_due(+s: S.St, +from: String, r: Maybe<&2, Nat>) -> Maybe<&2, S.XIn>:
  match r:
    case None{}:
      None{}
    case Some{+id}:
      Bool.pick(Maybe<&2, S.XIn>, String.eq(from, M.MAIN()), XB.reply_due(S.St.x(s), id), None{})

# project: `sb send @bise --reply-to m_<n> "..."`: main's answer to an
# open message of bise's, before its turn ends
def req_xreply(-o: S.St, r: Maybe<&2, S.XIn>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match r:
    case None{}:
      reply_err(o, cx, token, "@bise takes only main's answer to an open message from bise: sb send @bise --reply-to m_<n> \"...\"")
    case Some{+i}:
      reply(o, xreplied(o, cx, i, K.get_s(q, "text")), token, ok_body([J.kv("to", J.JStr{XB.FROM_BISE()}), J.kv("delivery", J.JStr{"sent"})]))

# ---- the agents' `sb` requests (RFC 0003 §4, RFC 0001 §7.2, §7.5) ----

def main_only(-o: S.St, +cx: X.Cx<o>, +token: Nat) -> X.Cx<o>:
  reply_err(o, cx, token, "reserved for main (the parent of the agents)")

def req_send.delivered(m: Maybe<&2, S.MSt>) -> Bool:
  match m:
    case Some{S.Delivered{}}:
      True{}
    case _:
      False{}

def req_send.res(-o: S.St, p: X.Cx<o> & SendR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      +delivered = req_send.delivered(M.state_of(st(o, cx), S.Msg.id(m)))
      reply(o, cx, token, ok_body([J.kv("message_id", J.JStr{mid(S.Msg.id(m))}),
        J.kv("thread", J.JStr{String.append("t_", ns(S.Msg.thread(m)))}), J.kv("to", J.JStr{S.Msg.to(m)}),
        J.kv("delivery", J.JStr{Bool.pick(String, delivered, "delivered", "queued")})]))

# the oldest delivered message from `asker` to `me` that still expects
# `me`'s reply
def first_from(xs: List<&2, S.Msg>, +asker: String) -> Maybe<&2, Nat>:
  match xs:
    case Nil{}:
      None{}
    case Con{+m, +t}:
      LZ.pick(Maybe<&2, Nat>, String.eq(S.Msg.from(m), asker), u => Some{S.Msg.id(m)}, u => first_from(t, asker))

def open_question.go(m: Maybe<&2, String>, +s: S.St, +me: String) -> Maybe<&2, Nat>:
  match m:
    case None{}:
      None{}
    case Some{+asker}:
      first_from(M.unanswered_for(s, me), asker)

def open_question(+s: S.St, +asker: String, +me: String) -> Maybe<&2, Nat>:
  open_question.go(M.resolve(s, asker), s, me)

# the forward of `m` as the sender's message: its note (the text it
# typed) and the header before m's text, the parts likewise, and m's id
# on the message (law forward_records_source)
def fwd_snd(+s: Snd, +m: S.Msg, +at: String) -> Snd:
  +note = Snd.text(s)
  +by = Snd.from(s)
  Snd.with_fwd(Snd.with_parts(Snd.with_text(s, FW.text(note, m, by, at)), FW.parts(note, m, by, at)), Some{S.Msg.id(m)})

def req_send.door(-o: S.St, d: FW.Door, +cx: X.Cx<o>, +token: Nat, +s: Snd, +q: J.Json) -> X.Cx<o>:
  match d:
    case FW.FErr{+e}:
      reply_err(o, cx, token, e)
    case FW.FOk{+m}:
      req_send.res(o, send_why(o, cx, fwd_snd(s, m, K.get_s(q, "fwd_at")), K.get_s(q, "why")), token)

# msg-forward: `sb send --forward m_<n>`: the message the sender got or
# sent, through FW.door; with `--attach` too: refused (v1)
def req_send.fwd(-o: S.St, f: Maybe<&2, Nat>, bare: Bool, +cx: X.Cx<o>, +token: Nat, +s: Snd, +q: J.Json) -> X.Cx<o>:
  match f bare:
    case None{} _:
      req_send.res(o, send_why(o, cx, s, K.get_s(q, "why")), token)
    case Some{_} False{}:
      reply_err(o, cx, token, "--forward and --attach go in two messages")
    case Some{+id} True{}:
      req_send.door(o, FW.door(st(o, cx), Snd.from(s), id), cx, token, s, q)

# an agent's `sb send` as the message it makes
def req_send.snd(+from: String, +to: String, +expect: Bool, +reply_to: Maybe<&2, Nat>, +q: J.Json) -> Snd:
  Snd{from, to, K.get_s(q, "text"), expect, reply_to, False{}, False{}, K.get_b(q, "queued"), None{}, K.get_s(q, "parts"), None{}}

def req_send.go(-o: S.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  +to = K.get_s(q, "to")
  +expect = K.get_b(q, "expect_reply")
  # a plain message to an agent that asked us something answers it
  +reply_to = Maybe.or(&2, Nat, K.opt_n(J.get(q, "reply_to")),
    Bool.pick(Maybe<&2, Nat>, expect, None{}, open_question(st(o, cx), to, from)))
  # send-parts: its parts as the hub checked them (`sb send --attach`;
  # "" none), the text already theirs (law agent_send_keeps_parts)
  +s = req_send.snd(from, to, expect, reply_to, q)
  req_send.fwd(o, K.opt_n(J.get(q, "fwd")), String.is_empty(Snd.parts(s)), cx, token, s, q)


# BISE-299: a question main escalated (`sb card --for`) is in the user's
# inbox: only the user answers it; an agent's reply to it is refused
def req_send.held(-o: S.St, held: Maybe<&2, Nat>, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  match held:
    case Some{+id}:
      +r = K.get_n(q, "reply_to")
      reply_err(o, cx, token, cat([mid(r), " is in the user's inbox (notification #", ns(id), "): only the user answers it",
        Bool.pick(String, String.eq(from, M.MAIN()), cat([". It is moot? `sb notify --withdraw ", ns(id), " \"why\"` takes it back first"]), "")]))
    case None{}:
      req_send.go(o, cx, token, from, q)

def req_send.hold(+s: S.St, r: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match r:
    case None{}:
      None{}
    case Some{+id}:
      CK.card_holding(S.St.cards(s), id)

def req_send(-o: S.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.eq(K.get_s(q, "to"), XB.FROM_BISE()),
    u => req_xreply(o, xreply_due(st(o, cx), from, K.opt_n(J.get(q, "reply_to"))), cx, token, q),
    u => req_send.held(o, req_send.hold(st(o, cx), K.opt_n(J.get(q, "reply_to"))), cx, token, from, q))
def wait.park(-o: S.St, +cx: X.Cx<o>, +w: X.Waiter) -> X.Cx<o>:
  +r = rr(o, cx)
  +cx2 = set_r(o, cx, X.R.with_waiters(r, M.snoc(X.Waiter, X.R.waiters(r), w)))
  dirty(o, upd_rt(o, cx2, X.Rt.with_waiting(rt(o, cx2, X.Waiter.agent(w)), True{})))

# the reply is already there (RFC 0003 §6.1-6.2): a queued one is
# delivered to the wait, unless it is a queued-mode message (it waits for
# the next turn); one already delivered is read again, without a second
# delivery; a rejected or cancelled one never reaches the wait
def wait.state(-o: S.St, s: Maybe<&2, S.MSt>, +cx: X.Cx<o>, +w: X.Waiter, +r: S.Msg) -> X.Cx<o>:
  match s:
    case Some{S.Queued{_}}:
      LZ.pick(X.Cx<o>, S.Msg.queued(r), u => wait.park(o, cx, w), u => deliver_to_waiter(o, cx, w, r, "reply"))
    case Some{S.Delivered{}}:
      waiter_reply(o, cx, w, r, "reply")
    case _:
      wait.park(o, cx, w)

def wait.existing(-o: S.St, m: Maybe<&2, S.Msg>, +cx: X.Cx<o>, +w: X.Waiter) -> X.Cx<o>:
  match m:
    case Some{+r}:
      wait.state(o, M.state_of(st(o, cx), S.Msg.id(r)), cx, w, r)
    case None{}:
      wait.park(o, cx, w)

# the first reply to a message: CO.reply_at (hub/cold.bend)
def wait.mine(-o: S.St, mine: Bool, +cx: X.Cx<o>, +token: Nat, +from: String, +orig: S.Msg, +timeout: Nat) -> X.Cx<o>:
  match mine:
    case False{}:
      reply_err(o, cx, token, "you can only wait for the reply to your own messages")
    case True{}:
      +w = {X.Waiter{token, from, S.Msg.id(orig), Nat.add(now(o, cx), Nat.mul(1000n, Nat.min(timeout, WAIT_CAP())))} : X.Waiter}
      wait.existing(o, CO.reply_at(st(o, cx), S.Msg.id(orig), S.Msg.to(orig)), cx, w)

def wait.orig(-o: S.St, m: Maybe<&2, S.Msg>, +cx: X.Cx<o>, +token: Nat, +from: String, +id: Nat, +timeout: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("message inconnu : ", mid(id)))
    case Some{+orig}:
      wait.mine(o, String.eq(S.Msg.from(orig), from), cx, token, from, orig, timeout)

def wait(-o: S.St, +cx: X.Cx<o>, +token: Nat, +from: String, +id: Nat, +timeout: Nat) -> X.Cx<o>:
  wait.orig(o, M.msg_of(st(o, cx), id), cx, token, from, id, timeout)

def req_ask.res(-o: S.St, p: X.Cx<o> & SendR, +token: Nat, +from: String, +timeout: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      wait(o, cx, token, from, S.Msg.id(m), timeout)

# ---- the REPLs ----

def decl_of(+s: String) -> S.Decl:
  Bool.pick(S.Decl, String.eq(s, "done"), S.DDone{}, Bool.pick(S.Decl, String.eq(s, "blocked"), S.DBlocked{}, S.DWorking{}))

def req_status(-o: S.St, +cx: X.Cx<o>, +token: Nat, +from: String, +is_main: Bool, +q: J.Json) -> X.Cx<o>:
  +status = K.get_s(q, "status")
  +note = K.get_s(q, "note")
  +cx1 = emit(o, cx, M.EDeclared{from, Some{decl_of(status)}, note}, Unit{}, Unit{}, Unit{})
  # done: it moved on, its reply watches end (ask-wait)
  +cx2 = LZ.pick(X.Cx<o>, String.eq(status, "done"), u => replies_end(o, cx1, from), u => cx1)
  # a blocked task is main's to handle (BISE-299): main escalates to the
  # user with `sb card` when it needs them
  +cx3 = LZ.pick(X.Cx<o>, String.eq(status, "blocked") && Bool.not(is_main),
    u => notify_main(o, cx2, cat(["@", from, " is blocked: ", Bool.pick(String, String.is_empty(note), "(no note)", note)])), u => cx2)
  reply(o, cx3, token, ok_body(Nil{}))

# ---- followed jobs (bise desktop S10) ----
# Someone (his main, the window) follows a task: when it ends, done or
# failed, the hub says so once (fx job_end, for his notification) and the
# follow ends with it (one job per follow: law follow_one_shot). Before
# its end, a followed task's reports make no job_end (law
# follow_quiet_before_end); its progress (`sb report progress --step n/m`)
# only moves the `jobs` rows.

# the job's name: the task's objective, one line
def job_label(+a: S.Agent) -> String:
  T.clip(T.one_line(J.get_str_or(S.Agent.brief(a), "objective", "")), 120n)

# J: the end of a job followed for another hub (bise's home hub): to its
# outbox, journaled, delivered until that hub has it (a restart of either
# hub loses nothing: the project's boot sends its pending entries again,
# the home hub drops a second copy, law xhub_xin_delivered_once)
def job_end.back(-o: S.St, local: Bool, +cx: X.Cx<o>, +hub: String, +j: J.Json) -> X.Cx<o>:
  match local:
    case True{}:
      cx
    case False{}:
      +ob = S.Out.with_text(XB.out_to(xst(o, cx), hub, S.KJobEnd{}), J.stringify(j))
      xdeliver(o, xemit(o, cx, XH.XOut{ob}), ob)

# `key`: the job's (its end's time on this hub's clock), the same in the
# fx job_end and in what goes back to `hub`, so a window that hears both
# shows one line
def job_end.go(-o: S.St, f: Bool, +cx: X.Cx<o>, +name: String, +state: String, +summary: String, +label: String, +hub: String) -> X.Cx<o>:
  match f:
    case True{}:
      +cx2 = emit(o, cx, M.EJobEnd{name, state}, Unit{}, Unit{}, Unit{})
      +key = now(o, cx2)
      +cx3 = out(o, cx2, F.job_end(name, state, label, summary, key, hub))
      job_end.back(o, String.is_empty(hub), cx3, hub, J.obj([J.kv("agent", J.JStr{name}), J.kv("key", J.JNum{key}),
        J.kv("state", J.JStr{state}), J.kv("label", J.JStr{label}), J.kv("summary", J.JStr{summary})]))
    case False{}:
      cx

def job_end.of(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +name: String, +state: String, +summary: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      job_end.go(o, M.follows(a), cx, name, state, summary, job_label(a), M.follow_hub(a))

# a task's end (done, failed): its job_end when followed, once (laws
# follow_done_once, follow_failed_at_once)
def job_end(-o: S.St, +cx: X.Cx<o>, +name: String, +state: String, +summary: String) -> X.Cx<o>:
  job_end.of(o, M.get_agent(st(o, cx), name), cx, name, state, summary)

def is_archived_life(l: S.Life) -> Bool:
  match l:
    case S.LArchived{}:
      True{}
    case _:
      False{}

def follow_err.of(m: Maybe<&2, S.Agent>, +name: String) -> Maybe<&2, String>:
  match m:
    case None{}:
      Some{cat(["no agent named @", name])}
    case Some{+a}:
      Bool.pick(Maybe<&2, String>, S.Agent.is_main(a), Some{"main isn't a job: follow one of its tasks"},
        Bool.pick(Maybe<&2, String>, is_archived_life(S.Agent.life(a)), Some{cat(["@", name, " is archived"])}, None{}))

# who can be followed: a task of this hub, not main, not archived
def follow_err(+s: S.St, +name: String) -> Maybe<&2, String>:
  follow_err.of(M.get_agent(s, name), name)

# `hub`: who asked ("" this hub's own main or window; else bise's home
# hub, J): journaled with the follow, cleared at its job_end
def follow_set(-o: S.St, +cx: X.Cx<o>, +name: String, +on: Bool, +hub: String) -> X.Cx<o>:
  emit(o, cx, M.EFollow{name, on, hub}, Unit{}, Unit{}, Unit{})

# the input follow (the window's typed command, or bise's `sb follow
# <p>/<agent>` on this hub's socket): an error is the asker's notice
def follow_in(-o: S.St, err: Maybe<&2, String>, +cx: X.Cx<o>, +name: String, +on: Bool) -> X.Cx<o>:
  match err:
    case Some{+e}:
      notice(o, cx, e)
    case None{}:
      follow_set(o, cx, name, on, "")

def req_follow.go(-o: S.St, err: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +name: String, +on: Bool, +hub: String) -> X.Cx<o>:
  match err:
    case Some{+e}:
      reply_err(o, cx, token, e)
    case None{}:
      reply(o, follow_set(o, cx, name, on, hub), token, ok_body([J.kv("agent", J.JStr{name}), J.kv("on", J.JBool{on})]))

# `sb follow <agent> [--off]`: main only (his say-so goes through main)
def req_follow(-o: S.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  +name = K.get_s(q, "agent")
  req_follow.go(o,
    Bool.pick(Maybe<&2, String>, String.eq(from, M.MAIN()), follow_err(st(o, cx), name), Some{"only main follows a task: ask main"}),
    cx, token, name, Bool.not(K.get_b(q, "off")), K.get_s(q, "hub"))

# the report's done and failed: their state change, then the job's end
# (ask-wait: it moved on, its reply watches end)
def report_done(-o: S.St, +cx: X.Cx<o>, +from: String) -> X.Cx<o>:
  replies_end(o, emit(o, cx, M.EDeclared{from, Some{S.DDone{}}, ""}, Unit{}, Unit{}, Unit{}), from)

def report_failed(-o: S.St, +cx: X.Cx<o>, +from: String, +summary: String) -> X.Cx<o>:
  replies_end(o, emit(o, cx, M.ELifecycle{from, S.LFailed{}, Some{summary}}, Unit{}, Unit{}, Unit{}), from)

def report_lines(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      String.append("\n- ", h) <> report_lines(t)

def report_text(+kind: String, +summary: String, +decisions: List<&2, String>) -> String:
  +head = cat(["[report: ", kind, "] ", String.trim(summary)])
  Bool.pick(String, List.is_empty(&2, String, decisions), head,
    cat([head, "\nDecisions:", String.concat(report_lines(decisions))]))

def req_report.sent(-o: S.St, p: X.Cx<o> & SendR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      reply(o, cx, token, ok_body([J.kv("message_id", J.JStr{mid(S.Msg.id(m))})]))

def req_report.kind(-o: S.St, +cx: X.Cx<o>, +from: String, +kind: String, +summary: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, String.eq(kind, "done"),
    u => job_end(o, report_done(o, cx, from), from, "done", summary),
    u => LZ.pick(X.Cx<o>, String.eq(kind, "blocked"),
      u => emit(o, cx, M.EDeclared{from, Some{S.DBlocked{}}, summary}, Unit{}, Unit{}, Unit{}),
      u => LZ.pick(X.Cx<o>, String.eq(kind, "failed"),
        u => job_end(o, report_failed(o, cx, from, summary), from, "failed", summary),
        u => cx)))

def req_report.go(-o: S.St, is_main: Bool, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  match is_main:
    case True{}:
      reply_err(o, cx, token, "main reports to no one")
    case False{}:
      +kind = K.get_s(q, "kind")
      +summary = K.get_s(q, "summary")
      +decisions = M.jstr_list(Maybe.default(&2, List<&2, J.Json>, J.get_arr(q, "decisions"), Nil{}))
      # S10: a progress's --step n/m; emitter 5: its --result (proto's
      # DiffResult, checked by the CLI); null when absent
      +cx2 = emit(o, cx, M.EReported{from, J.obj([J.kv("at_ms", J.JNum{now(o, cx)}), J.kv("kind", J.JStr{kind}),
        J.kv("summary", J.JStr{summary}), J.kv("decisions", J.JArr{jstrs(decisions)}), J.kv("auto", J.JBool{False{}}),
        J.kv("step", Maybe.default(&2, J.Json, J.get(q, "step"), J.JNull{})), J.kv("of", Maybe.default(&2, J.Json, J.get(q, "of"), J.JNull{})),
        J.kv("result", Maybe.default(&2, J.Json, J.get(q, "result"), J.JNull{}))])}, Unit{}, Unit{}, Unit{})
      +cx3 = req_report.kind(o, cx2, from, kind, summary)
      +parent = Maybe.default(&2, String, ST.parent_of(M.get_agent(st(o, cx3), from)), M.MAIN())
      +to = Bool.pick(String, String.eq(parent, M.USER()), M.MAIN(), parent)
      # the report answers the parent's open request, if any: no
      # automatic reply repeats it at the end of the turn
      +reply_to = open_question(st(o, cx3), to, from)
      req_report.sent(o, send(o, cx3, msg(from, to, report_text(kind, summary, decisions), False{}, reply_to, False{}, False{})), token)

def req_spawn.ws(m: Maybe<&2, S.Agent>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+a}:
      S.Agent.ws(a)

def req_spawn.res(-o: S.St, p: X.Cx<o> & TaskR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      +ws = req_spawn.ws(M.get_agent(st(o, cx), n))
      reply(o, cx, token, ok_body([J.kv("name", J.JStr{n}), J.kv("path", J.JStr{M.ws_path(ws)}), J.kv("branch", M.ws_branch(ws)), J.kv("placing", J.JBool{M.ws_placing(ws)})]))
    case (+cx, TErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, THalt{}):
      cx

def new_task(+parent: String, +q: J.Json) -> NewTask:
  NewTask{parent, K.get_s(q, "base"), Maybe.default(&2, J.Json, J.get(q, "brief"), J.JNull{}), K.get_s(q, "brief_text"),
    K.get_s(q, "objective"), K.get_b(q, "worktree"), K.get_b(q, "with_changes"), K.get_s(q, "brief_parts"), None{}}

# msg-forward: `sb spawn --forward m_<n>`: the brief gets the message main
# got or sent as its last section, its parts behind the brief's (law
# forward_brief_records_source: the brief's message carries the id)
def new_task.fwd(t: NewTask, +m: S.Msg, +at: String) -> NewTask:
  match t:
    case NewTask{+parent, +base, +brief, +brief_text, +objective, +worktree, +with_changes, _, _}:
      NewTask{parent, base, brief, FW.brief(brief_text, m, parent, at), objective, worktree, with_changes, FW.brief_parts(brief_text, m, parent, at), Some{S.Msg.id(m)}}

def req_spawn.door(-o: S.St, d: FW.Door, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match d:
    case FW.FErr{+e}:
      reply_err(o, cx, token, e)
    case FW.FOk{+m}:
      req_spawn.res(o, create_in(o, cx, new_task.fwd(new_task(M.MAIN(), q), m, K.get_s(q, "fwd_at")), J.get(q, "join")), token)

def req_spawn.fwd(-o: S.St, f: Maybe<&2, Nat>, bare: Bool, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match f bare:
    case None{} _:
      req_spawn.res(o, create_in(o, cx, new_task(M.MAIN(), q), J.get(q, "join")), token)
    case Some{_} False{}:
      reply_err(o, cx, token, "--forward and --attach go in two messages")
    case Some{+id} True{}:
      req_spawn.door(o, FW.door(st(o, cx), M.MAIN(), id), cx, token, q)

def req_spawn.pre(-o: S.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match pre:
    case Some{+e}:
      reply_err(o, cx, token, e)
    case None{}:
      req_spawn.fwd(o, K.opt_n(J.get(q, "fwd")), String.is_empty(K.get_s(q, "brief_parts")), cx, token, q)

def req_interrupt(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +agent: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no agent named ", agent))
    case Some{+n}:
      LZ.pick(X.Cx<o>, is_busy(X.Rt.run(rt(o, cx, n))),
        u => reply(o, stopped(o, interrupt.fx(o, cx, n), n), token, ok_body(Nil{})),
        u => reply_err(o, cx, token, "no turn in progress"))

def req_stop(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +agent: String, +reason: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no agent named ", agent))
    case Some{+n}:
      LZ.pick(X.Cx<o>, String.eq(n, M.MAIN()), u => reply_err(o, cx, token, String.append("no agent named ", agent)),
        u => reply(o, feed(o, stop_noted(o, cx, n, S.LStopped{}, reason), M.MAIN(), FD.info(cat(["main → @", n, " stopped: ", reason]))), token, ok_body(Nil{})))

def req_drop.res(-o: S.St, p: X.Cx<o> & DropR, +token: Nat, +agent: String) -> X.Cx<o>:
  match p:
    case (+cx, DDone{}):
      reply(o, cx, token, ok_body([J.kv("dropped", J.JBool{True{}})]))
    case (+cx, DAsk{+q}):
      +n = Maybe.default(&2, String, M.resolve(st(o, cx), agent), agent)
      +id = S.St.next_card(st(o, cx))
      +cx2 = open_card(o, cx, "drop", n, PL.drop_card(q), None{})
      reply(o, cx2, token, ok_body([J.kv("dropped", J.JBool{False{}}), J.kv("card", J.JNum{id}),
        J.kv("reason", J.JStr{"the user must confirm: a notification for it is open in the inbox"})]))
    case (+cx, DErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, DHalt{}):
      cx

def req_card.settle(-o: S.St, m: Maybe<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+id}:
      emit(o, cx, M.ESettled{id}, Unit{}, Unit{}, Unit{})

# a card main opens for a task's message (`sb card --for m_<n>`) shows in
# that task's own feed too (a registered agent of this hub, not main) (bar A.3, architect m_10203), with the line
# main's feed gets: a view; open or answered still comes from the cards
def is_task_of(-o: S.St, +cx: X.Cx<o>, +agent: String) -> Bool:
  has_agent(o, cx, agent) && Bool.not(String.eq(agent, M.MAIN()))

def card_to_asker(-o: S.St, +cx: X.Cx<o>, +id: Nat, +agent: String, +text: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, is_task_of(o, cx, agent),
    u => feed(o, cx, agent, FD.card(id, "question", agent, text)),
    u => cx)

# `about`: what the card is about, the shell's (its `--page|--artifact`
# resolved), kept on the card as is
def req_card.go(-o: S.St, +cx: X.Cx<o>, +token: Nat, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>, +about: Maybe<&2, J.Json>) -> X.Cx<o>:
  +id = S.St.next_card(st(o, cx))
  +cx2 = card_to_asker(o, open_card_about(o, cx, "question", agent, text, for_msg, about), id, agent, text)
  # main hands the question over: no automatic reply
  +cx3 = req_card.settle(o, for_msg, cx2)
  reply(o, cx3, token, ok_body([J.kv("card", J.JNum{id})]))

def req_card.msg(-o: S.St, q: Maybe<&2, S.Msg>, +cx: X.Cx<o>, +token: Nat, +text: String, +id: Nat, +about: Maybe<&2, J.Json>) -> X.Cx<o>:
  match q:
    case None{}:
      reply_err(o, cx, token, String.append("message inconnu : ", mid(id)))
    case Some{+m}:
      req_card.go(o, cx, token, S.Msg.from(m), text, Some{id}, about)

def req_card(-o: S.St, for_msg: Maybe<&2, Nat>, +cx: X.Cx<o>, +token: Nat, +text: String, +about: Maybe<&2, J.Json>) -> X.Cx<o>:
  match for_msg:
    case None{}:
      req_card.go(o, cx, token, M.MAIN(), text, None{}, about)
    case Some{+id}:
      req_card.msg(o, M.msg_of(st(o, cx), id), cx, token, text, id, about)

# `sb close N ["note"]`: a card in the user's inbox is the user's to
# answer or close (BISE-299): refused; an older journal's card of another
# kind (closed at boot anyway) closes with a short resolution
def req_close(-o: S.St, m: Maybe<&2, S.Card>, +cx: X.Cx<o>, +token: Nat, +id: Nat, +note: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no open notification #", ns(id)))
    case Some{+c}:
      +res = Bool.pick(String, String.is_empty(T.one_line(note)), "closed by main", String.append("main: ", T.one_line(note)))
      LZ.pick(X.Cx<o>, CK.user_kind(S.Card.kind(c)),
        u => reply_err(o, cx, token, cat(["notification #", ns(id), " is in the user's inbox: only the user answers or closes it (your own question, now moot: sb notify --withdraw ", ns(id), " \"why\")"])),
        u => reply(o, close_card(o, cx, id, res), token, ok_body([J.kv("card", J.JNum{id})])))

# main's withdraw of card `c`: closed without him, and journaled as
# withdrawn so his answer already on its way still reaches main
# (answer.late, hub/gone.bend)
def withdraw(-o: S.St, +cx: X.Cx<o>, +c: S.Card, +why: String) -> X.Cx<o>:
  +cx2 = close_card(o, cx, S.Card.id(c), String.append("withdrawn by main: ", why))
  emit(o, cx2, M.ECardWithdrawn{S.Card.id(c), S.Card.text(c)}, Unit{}, Unit{}, Unit{})

# `sb card --withdraw N "why"` (BISE-299): main takes back its own
# escalation (a question or a drop, never a confirmation) when it is moot;
# the card leaves his inbox, and the question it held is main's again;
# his answer already on its way reaches main as a message (withdraw,
# answer.late)
def req_withdraw(-o: S.St, m: Maybe<&2, S.Card>, +cx: X.Cx<o>, +token: Nat, +id: Nat, +why: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no open notification #", ns(id)))
    case Some{+c}:
      +k = S.Card.kind(c)
      LZ.pick(X.Cx<o>, String.eq(k, "question") || String.eq(k, "drop"),
        u => LZ.pick(X.Cx<o>, String.is_empty(T.one_line(why)),
          u => reply_err(o, cx, token, "say why: sb notify --withdraw N \"why\" (the user reads it)"),
          u => reply(o, withdraw(o, cx, c, T.one_line(why)), token, ok_body([J.kv("card", J.JNum{id})]))),
        u => reply_err(o, cx, token, cat(["notification #", ns(id), " is a ", k, ": only the user answers or closes it"])))

# `sb rename <task> <new-name>`: the same rules as the user's /rename
def req_rename.go(-o: S.St, ok: Bool, +cx: X.Cx<o>, +token: Nat, +old: String, +new: String) -> X.Cx<o>:
  match ok:
    case False{}:
      reply_err(o, cx, token, String.append("invalid or taken name: ", new))
    case True{}:
      reply(o, rename.apply(o, cx, old, new), token, ok_body([J.kv("name", J.JStr{new})]))

def req_rename(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +name: String, +new: String, +valid: Bool) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no agent named ", name))
    case Some{+old}:
      LZ.pick(X.Cx<o>, String.eq(old, M.MAIN()), u => reply_err(o, cx, token, String.append("no agent named ", name)),
        u => req_rename.go(o, valid && Bool.not(M.name_taken(st(o, cx), new)), cx, token, old, new))

# `sb restore` / `sb isolate`: the user or main (main only on the
# user's explicit request: its prompt says so)
def req_task.res(-o: S.St, p: X.Cx<o> & TaskR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      reply(o, cx, token, ok_body([J.kv("name", J.JStr{n})]))
    case (+cx, TErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, THalt{}):
      cx

def cmd_tag(+c: String) -> Cmd:
  Bool.pick(Cmd, String.eq(c, "send"), CSend{},
  Bool.pick(Cmd, String.eq(c, "wait"), CWait{},
  Bool.pick(Cmd, String.eq(c, "ask"), CAsk{},
  Bool.pick(Cmd, String.eq(c, "status"), CStatus{},
  Bool.pick(Cmd, String.eq(c, "report"), CReport{},
  Bool.pick(Cmd, String.eq(c, "follow"), CFollow{},
  Bool.pick(Cmd, String.eq(c, "spawn"), CSpawn{},
  Bool.pick(Cmd, String.eq(c, "interrupt"), CInterrupt{},
  Bool.pick(Cmd, String.eq(c, "stop"), CStop{},
  Bool.pick(Cmd, String.eq(c, "drop"), CDrop{},
  Bool.pick(Cmd, String.eq(c, "close"), CClose{},
  Bool.pick(Cmd, String.eq(c, "rename"), CRename{},
  Bool.pick(Cmd, String.eq(c, "restore"), CRestore{},
  Bool.pick(Cmd, String.eq(c, "isolate"), CIsolate{},
  Bool.pick(Cmd, String.eq(c, "move"), CMove{},
  Bool.pick(Cmd, String.eq(c, "withdraw"), CWithdraw{},
  Bool.pick(Cmd, String.eq(c, "project_send"), CProjectSend{},
  Bool.pick(Cmd, String.eq(c, "project_ask"), CProjectAsk{}, CCard{}))))))))))))))))))

def req.main.go(-o: S.St, c: Cmd, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match c:
    case CSpawn{}:
      req_spawn.pre(o, K.opt_s(J.get(q, "pre_err")), cx, token, q)
    case CInterrupt{}:
      req_interrupt(o, M.resolve(st(o, cx), K.get_s(q, "agent")), cx, token, K.get_s(q, "agent"))
    case CStop{}:
      req_stop(o, M.resolve(st(o, cx), K.get_s(q, "agent")), cx, token, K.get_s(q, "agent"), K.get_s(q, "reason"))
    case CDrop{}:
      req_drop.res(o, drop_task(o, cx, K.get_s(q, "agent"), False{}), token, K.get_s(q, "agent"))
    case CClose{}:
      req_close(o, M.card_of(S.St.cards(st(o, cx)), K.get_n(q, "card")), cx, token, K.get_n(q, "card"), K.get_s(q, "note"))
    case CRename{}:
      req_rename(o, M.resolve(st(o, cx), K.get_s(q, "agent")), cx, token, K.get_s(q, "agent"), K.get_s(q, "new_name"), K.get_b(q, "valid"))
    case CRestore{}:
      req_task.res(o, restore_task(o, cx, K.get_s(q, "agent")), token)
    case CIsolate{}:
      req_task.res(o, isolate_task(o, cx, K.get_s(q, "agent")), token)
    case CMove{}:
      req_task.res(o, move_task(o, K.opt_s(J.get(q, "pre_err")), cx, K.get_s(q, "agent"), J.get(q, "ws")), token)
    case CWithdraw{}:
      req_withdraw(o, M.card_of(S.St.cards(st(o, cx)), K.get_n(q, "card")), cx, token, K.get_n(q, "card"), K.get_s(q, "why"))
    case CProjectSend{}:
      req_xsend(o, K.opt_s(J.get(q, "pre_err")), cx, token, q)
    case CProjectAsk{}:
      req_xask(o, K.opt_s(J.get(q, "pre_err")), cx, token, q)
    case _:
      req_card(o, K.opt_n(J.get(q, "for")), cx, token, K.get_s(q, "text"), J.get(q, "about"))

def req.main(-o: S.St, is_main: Bool, +cx: X.Cx<o>, +token: Nat, +c: Cmd, +q: J.Json) -> X.Cx<o>:
  match is_main:
    case False{}:
      main_only(o, cx, token)
    case True{}:
      req.main.go(o, c, cx, token, q)

def req.go(-o: S.St, c: Cmd, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  match c:
    case CSend{}:
      req_send(o, cx, token, from, q)
    case CWait{}:
      wait(o, cx, token, from, K.get_n(q, "msg"), K.get_n(q, "timeout_s"))
    case CAsk{}:
      req_ask.res(o, send(o, cx, msg(from, K.get_s(q, "to"), K.get_s(q, "text"), True{}, None{}, False{}, False{})), token, from, K.get_n(q, "timeout_s"))
    case CStatus{}:
      req_status(o, cx, token, from, String.eq(from, M.MAIN()), q)
    case CReport{}:
      req_report.go(o, String.eq(from, M.MAIN()), cx, token, from, q)
    case CFollow{}:
      req_follow(o, cx, token, from, q)
    case other:
      req.main(o, String.eq(from, M.MAIN()), cx, token, other, q)

def req.from(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +raw: String, +q: J.Json) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("agent inconnu : ", raw))
    case Some{+from}:
      req.go(o, cmd_tag(K.get_s(q, "cmd")), cx, token, from, q)

def agent_req(-o: S.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  req.from(o, M.resolve(st(o, cx), from), cx, token, from, q)

# issue 37 #2 (peer.bend): a request runs only with its sender's ok, else
# fx spoke alone (law req_refused_says_only_why)
def agent_req.by(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  match m:
    case None{}:
      agent_req(o, cx, token, from, q)
    case Some{+why}:
      out(o, cx, PR.spoke(token, Some{why}))

# ---- the step ----

def Hub.d(x: Hub) -> S.St:
  match x:
    case Hub{+d, _, _}:
      d

def Hub.r(x: Hub) -> X.R:
  match x:
    case Hub{_, +r, _}:
      r

def hub_new(+workspace: String) -> Hub:
  Hub{M.init(workspace), X.R{Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, workspace, False{}, 0n, False{}, Nil{}, None{}, None{}, Nil{}, Nil{}}, True{}}

# the context of one input over the hub
def start(+h: Hub, +i: J.Json) -> X.Cx<Hub.d(h)>:
  +r = Hub.r(h)
  +r2 = X.R.with_answered(X.R.with_need(X.R.with_ans(X.R.with_git(X.R.with_now(X.R.with_dirty(r, False{}), K.get_n(i, "now")), K.get_b(i, "git")),
    Maybe.default(&2, List<&2, J.Json>, J.get_arr(i, "ans"), Nil{})), None{}), None{})
  X.Cx{X.core_start(Hub.d(h)), r2}

def agent_decl(m: Maybe<&2, S.Agent>) -> Maybe<&2, S.Decl>:
  match m:
    case None{}:
      None{}
    case Some{+a}:
      S.Agent.declared(a)

def turn.resume(-o: S.St, go: Bool, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  match go:
    case True{}:
      emit(o, cx, M.EDeclared{agent, None{}, ""}, Unit{}, Unit{}, Unit{})
    case False{}:
      cx

def decl_resumes(d: Maybe<&2, S.Decl>) -> Bool:
  match d:
    case Some{S.DDone{}}:
      True{}
    case Some{S.DBlocked{}}:
      True{}
    case _:
      False{}

def turn_started(-o: S.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  +decl = agent_decl(M.get_agent(st(o, cx), agent))
  +cx2 = dirty(o, upd_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(rt(o, cx, agent), S.RBusy{}), Some{now(o, cx)})))
  turn.resume(o, decl_resumes(decl), cx2, agent)

def steer_count(-o: S.St, +cx: X.Cx<o>, +agent: String, +injected: Bool) -> X.Cx<o>:
  +r = rr(o, cx)
  +c = KL.cnt_get(X.R.counts(r), agent)
  +c2 = Bool.pick(X.Cnt, injected, X.Cnt{agent, X.Cnt.recv(c), 1n+X.Cnt.inj(c)}, X.Cnt{agent, 1n+X.Cnt.recv(c), X.Cnt.inj(c)})
  set_r(o, cx, X.R.with_counts(r, M.snoc(X.Cnt, KL.cnt_del(X.R.counts(r), agent), c2)))

# an agent's steered list with `seen` set to `k` (the list itself kept)
def steer_mark(xs: List<&2, X.Steer>, +n: String, +k: Nat) -> List<&2, X.Steer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(X.Steer, String.eq(X.Steer.agent(h), n), X.Steer{n, X.Steer.ids(h), k, X.Steer.rx_seen(h)}, h) <> steer_mark(t, n, k)

# an agent's steered list with `rx_seen` set to `k` (the list and `seen`
# kept)
def steer_rx_mark(xs: List<&2, X.Steer>, +n: String, +k: Nat) -> List<&2, X.Steer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(X.Steer, String.eq(X.Steer.agent(h), n), X.Steer{n, X.Steer.ids(h), X.Steer.seen(h), k}, h) <> steer_rx_mark(t, n, k)

# the receipt's ids: the steer-mode deliveries since the last receipt
# (none: a second receipt with no new steer marks nothing)
def steer_receipt.go(-o: S.St, new: List<&2, Nat>, +cx: X.Cx<o>, +agent: String, +total: Nat) -> X.Cx<o>:
  match new:
    case Nil{}:
      cx
    case Con{+h, +t}:
      +r = rr(o, cx)
      +cx2 = set_r(o, cx, X.R.with_steered(r, steer_mark(X.R.steered(r), agent, total)))
      feed(o, cx2, agent, FD.steered(h <> t))

# the runtime read the steering (ISteered): the feed line naming the
# messages it read, at this point of the turn ('sb steered : m_3 m_4');
# R.steered stays whole for the turn's end (repl_idle reads it)
def steer_receipt(-o: S.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  +xs = X.R.steered(rr(o, cx))
  +ids = KL.steer_get(xs, agent)
  steer_receipt.go(o, List.drop(&2, Nat, ids, KL.steer_seen(xs, agent)), cx, agent, List.length(&2, Nat, ids))

# the received receipt's ids: the steer-mode deliveries since the last
# received receipt (none: nothing marked, nothing written)
def steer_rx_receipt.go(-o: S.St, new: List<&2, Nat>, +cx: X.Cx<o>, +agent: String, +total: Nat) -> X.Cx<o>:
  match new:
    case Nil{}:
      cx
    case Con{+h, +t}:
      +r = rr(o, cx)
      +cx2 = set_r(o, cx, X.R.with_steered(r, steer_rx_mark(X.R.steered(r), agent, total)))
      feed(o, cx2, agent, FD.steer_rx(h <> t))

# the runtime received the steering (ISteerRx, before it reads it): the
# feed line naming those messages ('sb steer-rx : m_3 m_4', the clients'
# ✓); its own cursor, rx_seen, apart from steered's
def steer_rx_receipt(-o: S.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  +xs = X.R.steered(rr(o, cx))
  +ids = KL.steer_get(xs, agent)
  steer_rx_receipt.go(o, List.drop(&2, Nat, ids, KL.steer_rx_seen(xs, agent)), cx, agent, List.length(&2, Nat, ids))

def assistant(-o: S.St, +cx: X.Cx<o>, +agent: String, +text: String) -> X.Cx<o>:
  +r = rr(o, cx)
  set_r(o, cx, X.R.with_last(r, M.snoc(X.Kv, KL.kv_del(X.R.last(r), agent), X.Kv{agent, text})))

# ---- land-claims: a claim ends when its file has no change left ----
# IClaimsClean: the shell's `git status` of a place (after a land in the
# shared folder, and at boot) found no change on `paths`; each live agent
# of that place that claims one of them drops it (one EClaimDropped each),
# so a file two agents once changed can be landed again. A later edit
# claims it again (EFileTouched). The window left: an agent that rewrites
# a claimed file between the shell's `git status` and this step loses
# that claim (its land then names the file instead of taking it).

def claims_clean.one(-o: S.St, hit: Bool, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  match hit:
    case True{}:
      emit(o, cx, M.EClaimDropped{agent, path}, Unit{}, Unit{}, Unit{})
    case False{}:
      cx

def claims_clean.paths(-o: S.St, paths: List<&2, String>, +cx: X.Cx<o>, +agent: String, +files: List<&2, String>) -> X.Cx<o>:
  match paths:
    case Nil{}:
      cx
    case Con{+p, +t}:
      claims_clean.paths(o, t, claims_clean.one(o, M.str_in(files, p), cx, agent, p), agent, files)

def claims_clean.at(-o: S.St, here: Bool, +cx: X.Cx<o>, +agent: String, +files: List<&2, String>, +paths: List<&2, String>) -> X.Cx<o>:
  match here:
    case True{}:
      claims_clean.paths(o, paths, cx, agent, files)
    case False{}:
      cx

def claims_clean.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +place: String, +paths: List<&2, String>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      claims_clean.at(o, String.eq(PL.Agent.place(a), place), cx, S.Agent.name(a), S.Agent.files(a), paths)

def claims_clean.go(-o: S.St, names: List<&2, String>, +cx: X.Cx<o>, +place: String, +paths: List<&2, String>) -> X.Cx<o>:
  match names:
    case Nil{}:
      cx
    case Con{+n, +t}:
      claims_clean.go(o, t, claims_clean.agent(o, M.get_agent(st(o, cx), n), cx, place, paths), place, paths)

def claims_clean(-o: S.St, +cx: X.Cx<o>, +place: String, +paths: List<&2, String>) -> X.Cx<o>:
  claims_clean.go(o, M.live_names(st(o, cx)), cx, place, paths)

# ---- file overlap (RFC 0001 §10.3) ----

def touch.others(-o: S.St, others: List<&2, String>, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  match others:
    case Nil{}:
      cx
    case Con{+h, +t}:
      +text = cat([path, " is changed by @", agent, " and by @", String.join({h <> t : List<&2, String>}, ", @")])
      note_main(o, cx, String.append("file overlap: ", text))

# an overlap is within one place (dev-flow §3.1): the shared folder, or
# one worktree its agents share
def overlap_hit(m: Maybe<&2, S.Agent>, +agent: String, +path: String, +place: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      Bool.not(S.Agent.is_main(a)) && Bool.not(String.eq(S.Agent.name(a), agent))
        && String.eq(PL.Agent.place(a), place) && is_active(S.Agent.life(a))
        && M.str_in(S.Agent.files(a), path)

def overlap_others(xs: List<&2, String>, +s: S.St, +agent: String, +path: String, +place: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = overlap_others(t, s, agent, path, place)
      Bool.pick(List<&2, String>, overlap_hit(M.get_agent(s, n), agent, path, place), n <> rest, rest)

def touch.new(-o: S.St, known: Bool, +cx: X.Cx<o>, +agent: String, +path: String, +place: String) -> X.Cx<o>:
  match known:
    case True{}:
      cx
    case False{}:
      +cx2 = emit(o, cx, M.EFileTouched{agent, path}, Unit{}, Unit{}, Unit{})
      touch.others(o, overlap_others(M.live_names(st(o, cx2)), st(o, cx2), agent, path, place), cx2, agent, path)

def touch.task(-o: S.St, task: Bool, +cx: X.Cx<o>, +agent: String, +path: String, +known: Bool, +place: String) -> X.Cx<o>:
  match task:
    case True{}:
      touch.new(o, known, cx, agent, path, place)
    case False{}:
      cx

def touch.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      # every task, in the shared folder or a worktree: `sb land` commits
      # an agent's own files, two agents may share a worktree
      touch.task(o, Bool.not(S.Agent.is_main(a)), cx, agent, path, M.str_in(S.Agent.files(a), path), PL.Agent.place(a))

def touch(-o: S.St, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  touch.agent(o, M.get_agent(st(o, cx), agent), cx, agent, path)

def repl_ready(-o: S.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  +cx2 = upd_rt(o, cx, X.Rt.with_waiting(X.Rt.with_run(rt(o, cx, agent), S.RIdle{}), False{}))
  pump(o, dirty(o, cx2), agent)

def crash.restart(-o: S.St, +cx: X.Cx<o>, +agent: String, +n: Nat, +reason: String) -> X.Cx<o>:
  +cx2 = set_run(o, cx, agent, S.RStarting{})
  +cx3 = out(o, cx2, F.spawn(agent, True{}, Some{reason}, False{}))
  +cx4 = feed(o, cx3, M.MAIN(), FD.warn(cat(["the session of @", agent, " crashed (", reason, ") — restart ", ns(n), "/", ns(MAX_CRASHES())])))
  LZ.pick(X.Cx<o>, String.eq(agent, M.MAIN()), u => cx4,
    u => notify_main(o, cx4, cat(["agent @", agent, " crashed (", reason, "). Its session restarts on its saved checkpoint (attempt ", ns(n), "/", ns(MAX_CRASHES()), "); the turn it was running is interrupted and it may need a new message to continue."])))

def crash.fail(-o: S.St, +cx: X.Cx<o>, +agent: String, +n: Nat, +reason: String) -> X.Cx<o>:
  +why = cat(["the REPL crashed ", ns(n), " times: ", reason])
  +cx2 = job_end(o, emit(o, cx, M.ELifecycle{agent, S.LFailed{}, Some{why}}, Unit{}, Unit{}, Unit{}), agent, "failed", why)
  +cx3 = feed(o, cx2, M.MAIN(), FD.warn(cat(["@", agent, " failed: ", why])))
  LZ.pick(X.Cx<o>, String.eq(agent, M.MAIN()), u => cx3,
    u => notify_main(o, cx3, cat(["agent @", agent, " failed: ", why, ". It is not restarted anymore. A new message from you or the user restarts it; otherwise tell the user."])))

def crash.pick(-o: S.St, again: Bool, +cx: X.Cx<o>, +agent: String, +n: Nat, +reason: String) -> X.Cx<o>:
  match again:
    case True{}:
      crash.restart(o, cx, agent, n, reason)
    case False{}:
      crash.fail(o, cx, agent, n, reason)

def crash.count(-o: S.St, go: Bool, +cx: X.Cx<o>, +agent: String, +reason: String) -> X.Cx<o>:
  match go:
    case False{}:
      cx
    case True{}:
      +r = rr(o, cx)
      +n = {1n+KL.kn_get(X.R.crashes(r), agent) : Nat}
      +cx2 = set_r(o, cx, X.R.with_crashes(r, M.snoc(X.Kn, KL.kn_del(X.R.crashes(r), agent), X.Kn{agent, n})))
      crash.pick(o, Nat.is_le(n, MAX_CRASHES()), cx2, agent, n, reason)

# a stalled start under load (hub/stall.bend): it starts again, not
# counted; `wait`: its K()-th in a row, the spawn waits for a tick under
# load x2 and its row shows it waiting
def crash.again.line(wait: Bool, +agent: String, +reason: String) -> J.Json:
  match wait:
    case True{}:
      FD.warn(cat(["@", agent, " waits for the machine: ", reason, " — it starts when the load is under x2"]))
    case False{}:
      FD.info(cat(["@", agent, " starts again: ", reason, " (the machine is slow: not a crash)"]))

def crash.again(-o: S.St, +wait: Bool, +cx: X.Cx<o>, +agent: String, +reason: String) -> X.Cx<o>:
  +cx2 = upd_rt(o, cx, X.Rt.with_waiting(X.Rt.with_run(rt(o, cx, agent), S.RStarting{}), wait))
  +cx3 = out(o, cx2, F.spawn(agent, True{}, None{}, wait))
  feed(o, cx3, M.MAIN(), crash.again.line(wait, agent, reason))

# the one line an agent cut mid-turn by a stop gets (stop-not-crash,
# architect m_48209): what happened, not who did it
def STOP_LINE(+reason: String) -> String:
  cat(["[bise] your process was stopped mid-turn (", reason, ") and started again from its last checkpoint: the turn was cut, so check what it had done, then go on."])

def crash.stop.tell(-o: S.St, busy: Bool, +cx: X.Cx<o>, +agent: String, +reason: String) -> X.Cx<o>:
  match busy:
    case True{}:
      fst(o, send(o, cx, msg(M.HUB(), agent, STOP_LINE(reason), False{}, None{}, False{}, False{})))
    case False{}:
      cx

# asked to stop (hub/stall.bend Stop): it starts again on its checkpoint,
# no crash counted, nothing to main; `busy` (it was mid-turn): it gets
# STOP_LINE once
def crash.stop(-o: S.St, busy: Bool, +cx: X.Cx<o>, +agent: String, +reason: String) -> X.Cx<o>:
  +cx2 = set_run(o, cx, agent, S.RStarting{})
  crash.stop.tell(o, busy, out(o, cx2, F.spawn(agent, True{}, None{}, False{})), agent, reason)

def crash.do(-o: S.St, d: SL.Do, busy: Bool, +cx: X.Cx<o>, +agent: String, +reason: String) -> X.Cx<o>:
  match d:
    case SL.Count{}:
      crash.count(o, True{}, cx, agent, reason)
    case SL.Again{}:
      crash.again(o, False{}, cx, agent, reason)
    case SL.SWait{}:
      crash.again(o, True{}, cx, agent, reason)
    case SL.Stop{}:
      crash.stop(o, busy, cx, agent, reason)

def crash.go(-o: S.St, go: Bool, busy: Bool, +cx: X.Cx<o>, +agent: String, d: SL.Do, +reason: String) -> X.Cx<o>:
  match go:
    case False{}:
      cx
    case True{}:
      crash.do(o, d, busy, cx, agent, reason)

# `busy`: it was mid-turn when its REPL went (read before the exit marks
# it down)
def exited.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +agent: String, +crashed: Bool, d: SL.Do, +reason: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      +busy = is_busy(X.Rt.run(rt(o, cx, agent)))
      +cx2 = set_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(rt(o, cx, agent), S.RDown{}), None{}))
      crash.go(o, crashed && is_active(S.Agent.life(a)), busy, dirty(o, cx2), agent, d, reason)

# `stalled`, `loaded`, `row`: the shell's facts on a crashed start
# (hub/stall.bend picks what it does to the crash count)
def repl_exited(-o: S.St, +cx: X.Cx<o>, +agent: String, +crashed: Bool, d: SL.Do, +reason: String) -> X.Cx<o>:
  +cx2 = fail_waiters(o, cx, agent, "interrupted_by_restart")
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_steered(r, KL.steer_del(X.R.steered(r), agent)))
  exited.agent(o, M.get_agent(st(o, cx3), agent), cx3, agent, crashed, d, reason)

def boot.live(-o: S.St, live: Bool, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  match live:
    case True{}:
      +cx2 = set_run(o, cx, n, S.RStarting{})
      out(o, cx2, F.spawn(n, True{}, None{}, False{}))
    case False{}:
      cx

# issue 32: a task a stopped hub was still placing gets its fx again (a
# joiner waits for its place's): its worktree_create, or its restore's
# worktree_restore; its REPL starts at its `placed`
def boot.place(-o: S.St, r: Maybe<&2, J.Json>, +cx: X.Cx<o>, +a: S.Agent) -> X.Cx<o>:
  match r:
    case None{}:
      place.ask(o, PL.makes(S.Agent.ws(a)), cx, S.Agent.name(a), S.Agent.ws(a))
    case Some{+from}:
      out(o, cx, F.worktree_restore(S.Agent.name(a), from, S.Agent.snap(a)))

def boot.placing(-o: S.St, placing: Bool, +cx: X.Cx<o>, +a: S.Agent) -> X.Cx<o>:
  match placing:
    case True{}:
      boot.place(o, PL.restore_of(S.Agent.ws(a)), cx, a)
    case False{}:
      boot.live(o, is_active(S.Agent.life(a)), cx, S.Agent.name(a))

def boot.one(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      boot.placing(o, M.ws_placing(S.Agent.ws(a)), cx, a)

# issue 32 3b: an archived task a stopped hub was still dropping: its
# worktree_drop again, with what to save (it rode with its ws)
def boot.drop(-o: S.St, dropping: Bool, +cx: X.Cx<o>, +a: S.Agent) -> X.Cx<o>:
  match dropping:
    case True{}:
      +ws = S.Agent.ws(a)
      out(o, cx, F.worktree_drop(S.Agent.name(a), ws, J.get_num_or(ws, "drop_dirty", 0n), J.get_num_or(ws, "drop_unpushed", 0n)))
    case False{}:
      cx

def boot.drops(-o: S.St, xs: List<&2, S.Agent>, +cx: X.Cx<o>) -> X.Cx<o>:
  match xs:
    case Nil{}:
      cx
    case Con{+a, +t}:
      boot.drops(o, t, boot.drop(o, M.ws_dropping(S.Agent.ws(a)), cx, a))

def boot.each(-o: S.St, names: List<&2, String>, +cx: X.Cx<o>) -> X.Cx<o>:
  match names:
    case Nil{}:
      dirty(o, cx)
    case Con{+n, +rest}:
      boot.each(o, rest, boot.one(o, M.get_agent(st(o, cx), n), cx, n))

# the daemon started: spawn every live agent. An older journal's cards
# that are not the user's (done, blocked, failed, overlap: the agents'
# traffic, main's since BISE-299) close first; their reports and notes
# are in main's thread already
def boot(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  +cx1 = view_ends(o, S.St.cards(st(o, cx)), cx)
  +cx2 = close_ids(o, CK.legacy_cards(S.St.cards(st(o, cx1))), cx1, "moved to main's inbox")
  +cx3 = boot.each(o, M.live_names(st(o, cx2)), cx2)
  boot.drops(o, S.St.arch(st(o, cx3)), cx3)

# the asked message, still with no reply watch and no reply, of an active
# agent: one more pause, from the ask (laws ask_timeout_pauses, ask_once)
def reply_pause.need(-o: S.St, +cx: X.Cx<o>, +agent: String, +orig: S.Msg) -> Bool:
  Bool.not(Maybe.is_some(&2, S.Watch, WK.reply_for(S.Wakes.live(wks(o, cx)), agent, S.Msg.id(orig))))
    && Bool.not(Maybe.is_some(&2, S.Msg, CO.reply_at(st(o, cx), S.Msg.id(orig), S.Msg.to(orig)))) && R.agent_active(st(o, cx), agent)

def reply_pause.go(-o: S.St, need: Bool, +cx: X.Cx<o>, +agent: String, +orig: S.Msg) -> X.Cx<o>:
  match need:
    case False{}:
      cx
    case True{}:
      emit(o, cx, M.EWake{WK.WSet{WK.make_reply(wks(o, cx), agent, S.Msg.to(orig), S.Msg.id(orig), S.Msg.created(orig), now(o, cx))}}, Unit{}, Unit{}, Unit{})

def reply_pause.orig(-o: S.St, m: Maybe<&2, S.Msg>, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+orig}:
      reply_pause.go(o, reply_pause.need(o, cx, agent, orig), cx, agent, orig)

# a wait that timed out (tick.each)
def reply_pause(-o: S.St, +cx: X.Cx<o>, +w: X.Waiter) -> X.Cx<o>:
  reply_pause.orig(o, M.msg_of(st(o, cx), X.Waiter.msg(w)), cx, X.Waiter.agent(w))

def tick.each(-o: S.St, late: List<&2, X.Waiter>, +cx: X.Cx<o>) -> X.Cx<o>:
  match late:
    case Nil{}:
      cx
    case Con{+w, +rest}:
      +cx2 = reply(o, cx, X.Waiter.token(w), J.obj([J.kv("ok", J.JBool{False{}}), J.kv("error", J.JStr{"timeout"}),
        J.kv("hint", J.JStr{cat(["no reply to ", mid(X.Waiter.msg(w)), " yet. It stays valid, and a pause on your row until the reply wakes you: end your turn, or continue with an explicit assumption (sb wake --stop <its id> when you move on)."])})]))
      +still = X.any_waiter(X.R.waiters(rr(o, cx2)), X.Waiter.agent(w))
      +cx3 = dirty(o, upd_rt(o, cx2, X.Rt.with_waiting(rt(o, cx2, X.Waiter.agent(w)), still)))
      tick.each(o, rest, pump(o, reply_pause(o, cx3, w), X.Waiter.agent(w)))

def tick.keep(-o: S.St, +cx: X.Cx<o>) -> List<&2, X.Waiter>:
  X.waiters_keep(X.R.waiters(rr(o, cx)), X.R.now(rr(o, cx)))

def tick.go(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  tick.each(o, X.waiters_late(X.R.waiters(rr(o, cx)), X.R.now(rr(o, cx))), set_waiters(o, cx, tick.keep(o, cx)))

# the late waits get their timeout; the others stay (law
# tick_clears_late_waiters; replies and pumps never add a wait)
def tick(-o: S.St, +cx: X.Cx<o>) -> X.Cx<o>:
  set_waiters(o, tick.go(o, cx), tick.keep(o, cx))

def requeue(-o: S.St, ids: List<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      requeue(o, rest, emit(o, cx, M.EMsgState{id, S.Queued{"steer_leftover"}}, Unit{}, Unit{}, {==}))

def skip_steered(xs: List<&2, S.Msg>, +skip: Bool, +ids: List<&2, Nat>) -> List<&2, S.Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = skip_steered(t, skip, ids)
      Bool.pick(List<&2, S.Msg>, skip && M.nat_in(ids, S.Msg.id(h)), rest, h <> rest)

# RFC 0003 §5.1: the end-of-turn answer to `@task message` typed in
# another view goes back to that view, and main gets it as a note
def auto_reply.local(-o: S.St, via: Maybe<&2, String>, +cx: X.Cx<o>, +agent: String, +m: S.Msg, +body: String) -> X.Cx<o>:
  match via:
    case None{}:
      fst(o, send(o, cx, msg(agent, S.Msg.from(m), body, False{}, Some{S.Msg.id(m)}, False{}, True{})))
    case Some{+view}:
      +cx2 = emit(o, cx, M.ESettled{S.Msg.id(m)}, Unit{}, Unit{}, Unit{})
      # an agent writing to the user (C2 `msg-you`, level 2)
      +cx3 = feed(o, cx2, view, FD.msg_you(agent, body))
      note_main(o, cx3, cat(["@", agent, " answered the user (asked from @", view, "'s view: \"", T.clip(T.one_line(S.Msg.text(m)), 500n),
        "\"): \"", T.clip_tail(T.one_line(body), 2000n), "\""]))

# a message bise sent this hub (S2) is answered back to bise's hub
def auto_reply.x(-o: S.St, r: Maybe<&2, S.XIn>, via: Maybe<&2, String>, +cx: X.Cx<o>, +agent: String, +m: S.Msg, +body: String) -> X.Cx<o>:
  match r:
    case Some{+i}:
      xreplied(o, cx, i, body)
    case None{}:
      auto_reply.local(o, via, cx, agent, m, body)

def auto_reply(-o: S.St, via: Maybe<&2, String>, +cx: X.Cx<o>, +agent: String, +m: S.Msg, +body: String) -> X.Cx<o>:
  auto_reply.x(o, XB.reply_due(xst(o, cx), S.Msg.id(m)), via, cx, agent, m, body)

# RFC 0003 §7: a question gets an answer, the end of the turn
def auto_replies(-o: S.St, qs: List<&2, S.Msg>, +cx: X.Cx<o>, +agent: String, +body: String) -> X.Cx<o>:
  match qs:
    case Nil{}:
      cx
    case Con{+m, +rest}:
      auto_replies(o, rest, auto_reply(o, Bool.pick(Maybe<&2, String>, String.eq(S.Msg.from(m), M.USER()), S.Msg.via(m), None{}), cx, agent, m, body), agent, body)

def idle.report(-o: S.St, go: Bool, +cx: X.Cx<o>, +agent: String, +text: String) -> X.Cx<o>:
  match go:
    case True{}:
      emit(o, cx, M.EReported{agent, J.obj([J.kv("at_ms", J.JNum{now(o, cx)}), J.kv("kind", J.JStr{"turn"}),
        J.kv("summary", J.JStr{T.clip(text, 1000n)}), J.kv("decisions", J.JArr{Nil{}}), J.kv("auto", J.JBool{True{}})])}, Unit{}, Unit{}, Unit{})
    case False{}:
      cx

def idle.nudge(-o: S.St, go: Bool, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  match go:
    case True{}:
      +cx2 = upd_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(rt(o, cx, agent), S.RBusy{}), Some{now(o, cx)}))
      out(o, cx2, F.say(agent, "[bise] The message(s) just above arrived while you were writing your last answer: you have not answered them yet. Answer them now."))
    case False{}:
      cx

# held (expired-ux): the turn stopped on the expired ChatGPT sign-in and
# goes on once the user signs in again: no automatic reply nor report (a
# `(no written reply…)` woke the parent, on the same plan, into a turn
# that failed too); the questions stay unanswered for the turn to come
def idle.agent(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +agent: String, +leftover: Bool, +unread: Bool, +steered: List<&2, Nat>, +held: Bool) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      +cx2 = dirty(o, set_rt(o, cx, X.Rt{agent, S.RIdle{}, False{}, None{}, 0n}))
      +r = rr(o, cx2)
      +text = KL.kv_get(X.R.last(r), agent)
      +cx3 = set_r(o, cx2, X.R.with_last(r, KL.kv_del(X.R.last(r), agent)))
      +qs = skip_steered(M.unanswered_for(st(o, cx3), agent), leftover || unread || held, steered)
      +body = Bool.pick(String, M.trim_is_empty(text), "(no written reply at the end of the turn)", text)
      +cx4 = LZ.pick(X.Cx<o>, held, u => cx3, u => auto_replies(o, qs, cx3, agent, body))
      # RFC 0001 §7.2: the automatic report (board only)
      +cx5 = idle.report(o, Bool.not(S.Agent.is_main(a)) && Bool.not(M.trim_is_empty(text)) && Bool.not(held), cx4, agent, text)
      +cx6 = idle.nudge(o, unread && List.is_empty(&2, S.Msg, M.queued_for(st(o, cx5), agent)) && Bool.not(P.is_paused(st(o, cx5))), cx5, agent)
      pump(o, cx6, agent)

def repl_idle(-o: S.St, +cx: X.Cx<o>, +agent: String, +leftover: Bool, +held: Bool) -> X.Cx<o>:
  +cx1 = fail_waiters(o, cx, agent, "turn_ended")
  +r = rr(o, cx1)
  +steered = KL.steer_get(X.R.steered(r), agent)
  +c = KL.cnt_get(X.R.counts(r), agent)
  +cx2 = set_r(o, cx1, X.R.with_counts(X.R.with_steered(r, KL.steer_del(X.R.steered(r), agent)), KL.cnt_del(X.R.counts(r), agent)))
  # steering committed after the last model call: the model never read
  # it; a new turn makes it answer
  +unread = Bool.not(leftover) && Nat.is_gt(X.Cnt.recv(c), X.Cnt.inj(c)) && Bool.not(List.is_empty(&2, Nat, steered))
  # leftover: the runtime never read them: they go again, as a new turn
  +cx3 = LZ.pick(X.Cx<o>, leftover, u => requeue(o, steered, cx2), u => cx2)
  idle.agent(o, M.get_agent(st(o, cx3), agent), cx3, agent, leftover, unread, steered, held)

# ---- waits (RFC 0003 §6.2) ----

# `/close N`: the user files a card without answering it
def close_by_user(-o: S.St, m: Maybe<&2, S.Card>, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      notice(o, cx, String.append("no open notification #", ns(id)))
    case Some{+c}:
      note_main(o, close_card(o, cx, id, "closed"), cat(["the user closed notification #", ns(id), " (", S.Card.kind(c), " @", S.Card.agent(c), ": \"", T.one_line(S.Card.text(c)), "\") without answering"]))

# the gate's card closes without an answer (its agents were
# interrupted): no card, nothing to do
def confirm_close(-o: S.St, m: Maybe<&2, S.Card>, +cx: X.Cx<o>, +id: Nat, +res: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{_}:
      close_card(o, cx, id, res)

# {"t":"page_answered"}: a page question answered (`to`: the agent its
# pick went to); its card closes, and its asker hears it when that is
# another agent (law page_answer_tells_asker)
def page_answered.tell(-o: S.St, tell: Bool, +cx: X.Cx<o>, +c: S.Card, +to: String, +page: String, +text: String) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, tell, u => fst(o, send(o, cx, msg(M.HUB(), S.Card.agent(c), R.answer_words(S.Card.id(c), page, to, text), False{}, None{}, False{}, False{}))), u => cx)

def page_answered(-o: S.St, m: Maybe<&2, S.Card>, +cx: X.Cx<o>, +to: String, +page: String, +text: String, +block: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+c}:
      +cx2 = close_card_ans(o, cx, S.Card.id(c), "answered", A.page_ans(text, page, block))
      page_answered.tell(o, R.tells_asker(M.resolve(st(o, cx2), S.Card.agent(c)), M.resolve(st(o, cx2), to)), cx2, c, to, page, text)

# {"t":"page_card"}: a page's own card (plan-ok): asked by the first live
# agent of `agents`, else main (reach.bend page_asker), in its feed too;
# a kind that is no page card's opens nothing. `key` is the hub's record;
# `about`, the page as an artifact (the shell's), is kept on the card.
def page_card.open(-o: S.St, +cx: X.Cx<o>, +kind: String, +who: String, +text: String, +about: Maybe<&2, J.Json>) -> X.Cx<o>:
  card_to_asker(o, open_card_about(o, cx, kind, who, text, None{}, about), S.St.next_card(st(o, cx)), who, text)

def page_card(-o: S.St, +cx: X.Cx<o>, +agents: List<&2, String>, +kind: String, +text: String, +about: Maybe<&2, J.Json>) -> X.Cx<o>:
  LZ.pick(X.Cx<o>, CK.page_kind(kind),
    u => page_card.open(o, cx, kind, R.page_asker(st(o, cx), agents), text, about),
    u => notice(o, cx, String.append("not a page card: ", kind)))

def force_run_of(+r: String) -> S.Run:
  Bool.pick(S.Run, String.eq(r, "starting"), S.RStarting{},
    Bool.pick(S.Run, String.eq(r, "idle"), S.RIdle{},
      Bool.pick(S.Run, String.eq(r, "busy"), S.RBusy{}, S.RDown{})))

def placed.tell(-o: S.St, user: Bool, +cx: X.Cx<o>, +parent: String, +text: String) -> X.Cx<o>:
  match user:
    case True{}:
      feed(o, cx, M.MAIN(), FD.warn(text))
    case False{}:
      fst(o, send(o, cx, msg(M.HUB(), parent, text, False{}, None{}, False{}, False{})))

# the shell's `placed` (place.bend's placed_of decided it)
def placed(-o: S.St, d: PL.Do, +cx: X.Cx<o>) -> X.Cx<o>:
  match d:
    case PL.POk{+n, +ws}:
      +cx2 = feed(o, emit(o, cx, M.EPlaced{n, ws, ""}, Unit{}, Unit{}, Unit{}), M.MAIN(), FD.info(PL.ready_text(n, ws)))
      spawn(o, cx2, n, False{})
    case PL.PErr{+n, +e, +parent}:
      +cx2 = stop_noted(o, emit(o, cx, M.EPlaced{n, J.JNull{}, e}, Unit{}, Unit{}, Unit{}), n, S.LFailed{}, e)
      placed.tell(o, String.eq(parent, M.USER()), cx2, parent, PL.err_text(n, e))
    case PL.POrphan{+n, +ws}:
      out(o, cx, F.worktree_drop(n, ws, 0n, 0n))
    case PL.PNone{}:
      cx
    case PL.PRestored{+n, +ws}:
      +cx2 = emit(o, emit(o, cx, M.EPlaced{n, ws, ""}, Unit{}, Unit{}, Unit{}), M.ESnapshot{n, None{}}, Unit{}, Unit{}, Unit{})
      spawn(o, feed(o, cx2, M.MAIN(), FD.info(PL.ready_text(n, ws))), n, True{})
    case PL.PUnrestored{+n, +e, +from}:
      +cx2 = emit(o, emit(o, cx, M.EPlaced{n, J.JNull{}, e}, Unit{}, Unit{}, Unit{}), M.EWorkspace{n, from}, Unit{}, Unit{}, Unit{})
      feed(o, stop_noted(o, cx2, n, S.LArchived{}, e), M.MAIN(), FD.warn(PL.unrestored_text(n, e)))

# then its joiners (`sb spawn --place` while it was placing): the same
def placed.join(-o: S.St, ns: List<&2, String>, +cx: X.Cx<o>, +d: PL.Do, +err: String) -> X.Cx<o>:
  match ns:
    case Nil{}:
      cx
    case Con{+b, +t}:
      placed.join(o, t, placed(o, PL.placed_of(st(o, cx), b, PL.join.ws(d), PL.join.err(d, err)), cx), d, err)

def placed.in(-o: S.St, +cx: X.Cx<o>, +name: String, +ws: J.Json, +err: String) -> X.Cx<o>:
  +d = PL.placed_of(st(o, cx), name, ws, err)
  placed.join(o, PL.joiners(st(o, cx), name), placed(o, d, cx), d, err)

# one input, decoded by the generated codec (gen/in.bend, from
# bise_proto::core::In): an unknown tag is INone, a missing field its
# default
def dispatch(-o: S.St, +cx: X.Cx<o>, +i: GI.In) -> X.Cx<o>:
  match i:
    case GI.IReq{+token, +from, +req, +who, +dir, +sock}:
      agent_req.by(o, PR.req_ok(st(o, cx), who, dir, sock, from), cx, token, from, req)
    case GI.ISpeaks{+token, +from, _, +who, +dir, +sock}:
      out(o, cx, PR.spoke(token, PR.req_ok(st(o, cx), who, dir, sock, from)))
    case GI.ITick{}:
      replies_tick(o, wakes_tick(o, acts_tick(o, timers_tick(o, tick(o, cx)))))
    case GI.IIdle{+agent, +leftover, +held}:
      repl_idle(o, cx, agent, leftover, held)
    case GI.ITurnStarted{+agent}:
      turn_started(o, cx, agent)
    case GI.ISteerRx{+agent}:
      steer_rx_receipt(o, steer_count(o, cx, agent, False{}), agent)
    case GI.ISteered{+agent}:
      steer_receipt(o, steer_count(o, cx, agent, True{}), agent)
    case GI.IAssistant{+agent, +text}:
      assistant(o, cx, agent, text)
    case GI.ITouch{+agent, +path}:
      touch(o, cx, agent, path)
    case GI.IClaimsClean{+place, +paths}:
      claims_clean(o, cx, place, paths)
    case GI.IReady{+agent}:
      repl_ready(o, cx, agent)
    case GI.IExited{+agent, +crashed, +reason, +stalled, +loaded, +row, +stopped}:
      repl_exited(o, cx, agent, crashed, SL.exit(stopped, stalled, loaded, row), reason)
    case GI.IBoot{}:
      xboot(o, boot(o, cx))
    case GI.ISay{+focus, +text, +queued, +parts, +from}:
      says_ctx(o, cx, focus, text, queued, parts, from)
    case GI.IRoute{+target, +text, +focus, +queued, +parts}:
      LZ.pick(X.Cx<o>, String.eq(target, M.MAIN()),
        u => says_ctx(o, cx, M.MAIN(), text, queued, parts, ""),
        u => route_ctx(o, M.resolve(st(o, cx), target), cx, target, text, focus, queued, parts))
    case GI.INew{+base, +brief, +brief_text, +objective, +worktree, +with_changes}:
      user_new.res(o, create_task(o, cx, NewTask{M.USER(), base, brief, brief_text, objective, worktree, with_changes, "", None{}}))
    case GI.IDrop{+name, +force}:
      user_drop.res(o, drop_task(o, cx, name, force), name)
    case GI.IRestore{+name}:
      user_restore.res(o, restore_task(o, cx, name))
    case GI.IIsolate{+name}:
      user_isolate.res(o, isolate_task(o, cx, name))
    case GI.IPlaced{+name, +ws, +err}:
      placed.in(o, cx, name, ws, err)
    case GI.IUnplaced{+name, +snapshot, +err}:
      unplaced.in(o, cx, name, snapshot, err)
    case GI.IFeatureRetire{+name, +kept, +main}:
      feature_retire(o, cx, name, kept, main)
    case GI.IRename{+name, +new_name, +valid}:
      rename.old(o, M.resolve(st(o, cx), name), cx, name, new_name, valid)
    case GI.IAnswer{+card, +text, +parts}:
      answer_card(o, cx, card, text, parts, "inbox")
    case GI.IAnswerPos{+card, +agent, +pos}:
      A.answer_pos(o, cx, card, agent, pos)
    case GI.IInterrupt{+agent}:
      interrupt.go(o, M.resolve(st(o, cx), agent), cx)
    case GI.IUnqueue{+id}:
      unqueue(o, cx, id)
    case GI.ISteerQueued{+id}:
      steer_queued(o, cx, id)
    case GI.IPassthrough{+focus, +line}:
      passthrough(o, cx, focus, line)
    case GI.IFocus{+to, +note, +direct}:
      focus(o, cx, to, note, direct)
    case GI.IClose{+card}:
      close_by_user(o, M.card_of(S.St.cards(st(o, cx)), card), cx, card)
    case GI.IForceRun{+agent, +run}:
      # tests only: put an agent's REPL in a given state (then deliver, like
      # any change of run: law L4)
      pump(o, set_run(o, cx, agent, force_run_of(run)), agent)
    case GI.IConfirmOpen{+agent, +text}:
      open_confirm(o, cx, agent, text)
    case GI.IConfirmClose{+card, +res}:
      confirm_close(o, M.card_of(S.St.cards(st(o, cx)), card), cx, card, res)
    case GI.IViewConfirmOpen{+agent, +call, +text, +ask}:
      open_view_confirm(o, cx, agent, call, text, ask)
    case GI.IViewMessage{+agent, +plugin, +text}:
      view_message(o, cx, agent, plugin, text)
    case GI.IViewContext{+agent, +call, +text, +part}:
      view_context(o, cx, agent, call, text, part)
    case GI.IPageAnswered{+card, +agent, +page, +text, +block}:
      page_answered(o, M.card_of(S.St.cards(st(o, cx)), card), cx, agent, page, text, block)
    case GI.IForge{+to, +text}:
      fst(o, send(o, cx, msg(M.GITHUB(), to, text, False{}, None{}, False{}, False{})))
    case GI.IPageCard{+agents, +kind, +text, +key, +about}:
      page_card(o, cx, agents, kind, text, about)
    case GI.IForgeCard{+agent, +text}:
      open_card(o, cx, "question", agent, text, None{})
    case GI.ICardOpen{+kind, +agent, +text, +place, +pr}:
      hub_card(o, cx, kind, agent, text, place, pr)
    case GI.IHubMsg{+to, +text}:
      fst(o, send(o, cx, msg(M.HUB(), to, text, False{}, None{}, False{}, False{})))
    case GI.IEverySet{+agent, +by, +text, +next_ms, +name, +every_ms, +daily_min, +until_ms, +times, +page}:
      timer_set(o, cx, agent, every_ms, daily_min, GI.IEverySet{agent, by, text, next_ms, name, every_ms, daily_min, until_ms, times, page})
    case GI.IEveryStop{+id, +why, +wake}:
      timer_stop.found(o, TM.find(live(o, cx), id), cx, id, why, wake)
    case GI.IEveryRun{+id, +text}:
      timer_run.found(o, TM.find(live(o, cx), id), cx, text)
    case GI.IEveryName{+id, +name}:
      timer_name.found(o, TM.find(live(o, cx), id), cx, id, name)
    case GI.IXIn{+token, +hub, +xid, +kind, +text, +name, +parts}:
      xin(o, XB.seen(xst(o, cx), hub, xid), cx, token, hub, xid, SC.XKind.of_word(kind), text, name, parts)
    case GI.IXAck{+xid}:
      xack(o, XB.pending(xst(o, cx), xid), cx, xid)
    case GI.IXFail{+xid}:
      xfail(o, XB.pending(xst(o, cx), xid), cx, xid)
    case GI.IXReply{+xid, +name, +text}:
      xanswer(o, XB.open(xst(o, cx), xid), cx, xid, name, text)
    case GI.IRouteAsk{+focus, +text, +via, +queued, +context, +parts, +projects}:
      route_ask(o, XB.addressed(XB.projs(projects), st(o, cx), queued, focus, text, via), cx, text, via, context, parts)
    case GI.IRouteCorrect{+rid, +to, +name}:
      route_correct(o, cx, rid, to, name)
    case GI.IRouteCancel{+rid}:
      route_cancel(o, cx, rid)
    case GI.IFollow{+agent, +on}:
      follow_in(o, follow_err(st(o, cx), agent), cx, agent, on)
    case GI.IWakeSet{+agent, +spec, +max_text, +max_ms, +quiet, +then}:
      wake_set(o, M.resolve(st(o, cx), agent), cx, spec, max_text, max_ms, quiet, then)
    case GI.IWakeStop{+id, +why, +tell}:
      wake_stop.found(o, WK.hit(wks(o, cx), id), cx, why, tell)
    case GI.IWakeHit{+id, +text}:
      wake_hit(o, cx, id, text)
    case GI.IPause{}:
      pause(o, cx)
    case GI.IResume{}:
      resume(o, cx)
    case GI.INone{}:
      cx

# one input: the context after it (its state is the replay of its
# journal over the hub's state: law replay_reproduces)
def step(+h: Hub, +i: J.Json) -> X.Cx<Hub.d(h)>:
  dispatch(Hub.d(h), start(h, i), GI.of(i))

# ---- committing a step (what sb-core keeps) ----

def commit.go(-o: S.St, need: Maybe<&2, J.Json>, +c: X.Core<o>, +r: X.R, +h: Hub) -> Turn:
  match need:
    case Some{+q}:
      Turn{h, Nil{}, Some{q}}
    case None{}:
      Turn{Hub{X.Core.d(o, c), X.R.with_ans(r, Nil{}), False{}}, X.Core.fx(o, c), None{}}

def commit.cx(-o: S.St, cx: X.Cx<o>, +h: Hub) -> Turn:
  match cx:
    case X.Cx{+c, +r}:
      commit.go(o, X.R.need(r), c, r, h)

def commit(+h: Hub, +i: J.Json) -> Turn:
  commit.cx(Hub.d(h), step(h, i), h)

def RunR.hub(x: RunR) -> Hub:
  match x:
    case RunR{+hub, _}:
      hub

def Turn.fx(x: Turn) -> List<&2, X.Fx>:
  match x:
    case Turn{_, +fx, _}:
      fx

def RunR.journal(x: RunR) -> List<&2, M.Ev>:
  match x:
    case RunR{_, +journal}:
      journal

def run.cat(+o1: Turn, +rr: RunR) -> RunR:
  RunR{RunR.hub(rr), List.append(&2, M.Ev, X.journal(Turn.fx(o1)), RunR.journal(rr))}

def Turn.hub(x: Turn) -> Hub:
  match x:
    case Turn{+hub, _, _}:
      hub

def run(ins: List<&2, J.Json>, +h: Hub) -> RunR:
  match ins:
    case Nil{}:
      RunR{h, Nil{}}
    case Con{+i, +rest}:
      run.cat(commit(h, i), run(rest, Turn.hub(commit(h, i))))

def Turn.need(x: Turn) -> Maybe<&2, J.Json>:
  match x:
    case Turn{_, _, +need}:
      need

# every waiter's deadline is after `now`
def Later(ws: List<&2, X.Waiter>, +now: Nat) -> Data:
  match ws:
    case Nil{}:
      Unit
    case Con{+w, +t}:
      X.Both<{Nat.is_le(X.Waiter.deadline(w), now) == False{} : Bool}, Later(t, now)>

# no waiter belongs to `n`
def NoneOf(ws: List<&2, X.Waiter>, +n: String) -> Data:
  match ws:
    case Nil{}:
      Unit
    case Con{+w, +t}:
      X.Both<{String.eq(X.Waiter.agent(w), n) == False{} : Bool}, NoneOf(t, n)>

def waiters_of_cx(-o: S.St, +cx: X.Cx<o>) -> List<&2, X.Waiter>:
  X.R.waiters(rr(o, cx))
