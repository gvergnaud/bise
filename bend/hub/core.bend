# hub/core.bend — the hub's decisions (RFC 0001, RFC 0003): a pure step
# from (state, input) to (state, effects). Ported from
# rust/switchboard/src/core.rs; the Rust daemon runs the effects.
import Base

import ../vendor/json.bend as J

import ./model.bend as M

import ./cx.bend as X

import ./codec.bend as C

import ./timers.bend as TM

import ./xhub-model.bend as XH

import ./xhub.bend as XB

type Snd is Data:
  Snd{from: String, to: String, text: String, expect: Bool,
    reply_to: Maybe<&2, Nat>, plain: Bool, auto: Bool, queued: Bool,
    via: Maybe<&2, String>}

type SendR is Data:
  SOk{m: M.Msg}
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
    objective: String, worktree: Bool, with_changes: Bool}

type Hub is Data:
  Hub{d: M.St, r: X.R}

type In is Data:
  IReq{}
  ITick{}
  IIdle{}
  ITurnStarted{}
  ISteerRx{}
  ISteered{}
  IAssistant{}
  ITouch{}
  IReady{}
  IExited{}
  IBoot{}
  ISay{}
  IRoute{}
  INew{}
  IDrop{}
  IConfirmDrop{}
  IRestore{}
  IIsolate{}
  IRename{}
  IAnswer{}
  IInterrupt{}
  IPassthrough{}
  IFocus{}
  IClose{}
  IForceRun{}
  IConfirmOpen{}
  IConfirmClose{}
  IForge{}
  IForgeCard{}
  ICardOpen{}
  IHubMsg{}
  IEverySet{}
  IEveryStop{}
  IEveryRun{}
  IEveryName{}
  IXIn{}
  IXAck{}
  IXFail{}
  IXReply{}
  IRouteHold{}
  IRouteCorrect{}
  IRouteCancel{}
  IRoutePick{}
  IFollow{}
  INone{}

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
type Out is Data:
  Out{hub: Hub, fx: List<&2, X.Fx>, need: Maybe<&2, J.Json>}

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

# Bool.pick computes both of its arguments; pick_lazy computes only the
# branch it takes (for branches that are whole step paths)
def pick_lazy(-A: Type, c: Bool, a: Unit -> A, b: Unit -> A) -> A:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def run_str(r: M.Run) -> String:
  match r:
    case M.RDown{}:
      "down"
    case M.RStarting{}:
      "starting"
    case M.RIdle{}:
      "idle"
    case M.RBusy{}:
      "busy"

def is_busy(r: M.Run) -> Bool:
  match r:
    case M.RBusy{}:
      True{}
    case _:
      False{}

def is_idle(r: M.Run) -> Bool:
  match r:
    case M.RIdle{}:
      True{}
    case _:
      False{}

def is_down(r: M.Run) -> Bool:
  match r:
    case M.RDown{}:
      True{}
    case _:
      False{}

def is_active(l: M.Life) -> Bool:
  match l:
    case M.LActive{}:
      True{}
    case _:
      False{}

# ---- the runtime lists ----

def rt_find(xs: List<&2, X.Rt>, +n: String) -> X.Rt:
  match xs:
    case Nil{}:
      X.Rt{n, M.RDown{}, False{}, None{}}
    case Con{+h, +t}:
      Bool.pick(X.Rt, String.eq(X.Rt.name(h), n), h, rt_find(t, n))

def rt_put(xs: List<&2, X.Rt>, +x: X.Rt) -> List<&2, X.Rt>:
  match xs:
    case Nil{}:
      [x]
    case Con{+h, +t}:
      Bool.pick(List<&2, X.Rt>, String.eq(X.Rt.name(h), X.Rt.name(x)), x <> t, h <> rt_put(t, x))

def rt_rename(xs: List<&2, X.Rt>, +old: String, +new: String) -> List<&2, X.Rt>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(X.Rt, String.eq(X.Rt.name(h), old), X.Rt.with_name(h, new), h) <> rt_rename(t, old, new)

def any_waiter(xs: List<&2, X.Waiter>, +n: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      String.eq(X.Waiter.agent(h), n) || any_waiter(t, n)

def waiters_of(xs: List<&2, X.Waiter>, +n: String) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_of(t, n)
      Bool.pick(List<&2, X.Waiter>, String.eq(X.Waiter.agent(h), n), h <> rest, rest)

def waiters_not(xs: List<&2, X.Waiter>, +n: String) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_not(t, n)
      Bool.pick(List<&2, X.Waiter>, String.eq(X.Waiter.agent(h), n), rest, h <> rest)

def waiters_late(xs: List<&2, X.Waiter>, +now: Nat) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_late(t, now)
      Bool.pick(List<&2, X.Waiter>, Nat.is_le(X.Waiter.deadline(h), now), h <> rest, rest)

def waiters_keep(xs: List<&2, X.Waiter>, +now: Nat) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_keep(t, now)
      Bool.pick(List<&2, X.Waiter>, Nat.is_le(X.Waiter.deadline(h), now), rest, h <> rest)

def waiters_rename(xs: List<&2, X.Waiter>, +old: String, +new: String) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(X.Waiter, String.eq(X.Waiter.agent(h), old), X.Waiter.with_agent(h, new), h) <> waiters_rename(t, old, new)

# the first waiter a new message releases (RFC 0003 §6.2): the agent's
# wait for this reply, or any wait when the message asks a question
def waiter_hit(+w: X.Waiter, +to: String, +reply_to: Maybe<&2, Nat>, +asks: Bool) -> Bool:
  String.eq(X.Waiter.agent(w), to) && (M.maybe_nat_is(reply_to, X.Waiter.msg(w)) || asks)

def waiter_pick(xs: List<&2, X.Waiter>, +to: String, +reply_to: Maybe<&2, Nat>, +asks: Bool) -> Maybe<&2, X.Waiter>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, X.Waiter>, waiter_hit(h, to, reply_to, asks), Some{h}, waiter_pick(t, to, reply_to, asks))

def waiter_drop(xs: List<&2, X.Waiter>, +tok: Nat) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, X.Waiter>, Nat.is_eq(X.Waiter.token(h), tok), t, h <> waiter_drop(t, tok))

# ---- keyed lists (a list of entries, at most one per key) ----

# the entries whose key is not `n`
def del_key(~A: Data, ~key: A -> String, xs: List<&2, A>, +n: String) -> List<&2, A>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = del_key(~A, ~key, t, n)
      Bool.pick(List<&2, A>, String.eq(key(h), n), rest, h <> rest)

# the value of the first entry whose key is `n`, or `dflt`
def get_key(~A: Data, ~B: Data, ~key: A -> String, ~val: A -> B, xs: List<&2, A>, +n: String, +dflt: B) -> B:
  match xs:
    case Nil{}:
      dflt
    case Con{+h, +t}:
      Bool.pick(B, String.eq(key(h), n), val(h), get_key(~A, ~B, ~key, ~val, t, n, dflt))

def steer_get(xs: List<&2, X.Steer>, +n: String) -> List<&2, Nat>:
  get_key(~X.Steer, ~List<&2, Nat>, ~X.Steer.agent, ~X.Steer.ids, xs, n, Nil{})

def cnt_get(xs: List<&2, X.Cnt>, +n: String) -> X.Cnt:
  get_key(~X.Cnt, ~X.Cnt, ~X.Cnt.agent, ~(c => c), xs, n, X.Cnt{n, 0n, 0n})

def cnt_del(xs: List<&2, X.Cnt>, +n: String) -> List<&2, X.Cnt>:
  del_key(~X.Cnt, ~X.Cnt.agent, xs, n)

def kv_get(xs: List<&2, X.Kv>, +n: String) -> String:
  get_key(~X.Kv, ~String, ~X.Kv.key, ~X.Kv.val, xs, n, "")

def kv_del(xs: List<&2, X.Kv>, +n: String) -> List<&2, X.Kv>:
  del_key(~X.Kv, ~X.Kv.key, xs, n)

def kn_get(xs: List<&2, X.Kn>, +n: String) -> Nat:
  get_key(~X.Kn, ~Nat, ~X.Kn.key, ~X.Kn.n, xs, n, 0n)

def kn_del(xs: List<&2, X.Kn>, +n: String) -> List<&2, X.Kn>:
  del_key(~X.Kn, ~X.Kn.key, xs, n)

def steer_del(xs: List<&2, X.Steer>, +n: String) -> List<&2, X.Steer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = steer_del(t, n)
      Bool.pick(List<&2, X.Steer>, String.eq(X.Steer.agent(h), n), rest, h <> rest)

def steer_add(+xs: List<&2, X.Steer>, +n: String, +ids: List<&2, Nat>) -> List<&2, X.Steer>:
  M.snoc(X.Steer, steer_del(xs, n), X.Steer{n, List.append(&2, Nat, steer_get(xs, n), ids)})

def steer_rename(xs: List<&2, X.Steer>, +old: String, +new: String) -> List<&2, X.Steer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(X.Steer, String.eq(X.Steer.agent(h), old), X.Steer{new, X.Steer.ids(h)}, h) <> steer_rename(t, old, new)

def ids_of(xs: List<&2, M.Msg>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      M.Msg.id(h) <> ids_of(t)

# ---- context shortcuts ----

def st(-o: M.St, +cx: X.Cx<o>) -> M.St:
  X.d(o, cx)

def rr(-o: M.St, +cx: X.Cx<o>) -> X.R:
  X.Cx.r(o, cx)

def set_r(-o: M.St, +cx: X.Cx<o>, +r: X.R) -> X.Cx<o>:
  X.with_r(o, cx, r)

def out(-o: M.St, +cx: X.Cx<o>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> X.Cx<o>:
  X.out(o, cx, J.obj(kvs))

def line(-o: M.St, +cx: X.Cx<o>, +agent: String, +kind: String, +text: String) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"line"}), J.kv("agent", J.JStr{agent}), J.kv("kind", J.JStr{kind}), J.kv("text", J.JStr{text})])

def reply(-o: M.St, +cx: X.Cx<o>, +token: Nat, +body: J.Json) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"reply"}), J.kv("token", J.JNum{token}), J.kv("body", body)])

def reply_err(-o: M.St, +cx: X.Cx<o>, +token: Nat, +e: String) -> X.Cx<o>:
  reply(o, cx, token, err_body(e))

def notice(-o: M.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"notice"}), J.kv("text", J.JStr{text})])

def rt(-o: M.St, +cx: X.Cx<o>, +n: String) -> X.Rt:
  rt_find(X.R.rts(rr(o, cx)), n)

def jturn(m: Maybe<&2, Nat>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+t}:
      J.JNum{t}

# set the runtime state of an agent (the Rust mirror follows)
def set_rt(-o: M.St, +cx: X.Cx<o>, +x: X.Rt) -> X.Cx<o>:
  +cx2 = set_r(o, cx, X.R.with_rts(rr(o, cx), rt_put(X.R.rts(rr(o, cx)), x)))
  out(o, cx2, [J.kv("fx", J.JStr{"rt"}), J.kv("agent", J.JStr{X.Rt.name(x)}),
    J.kv("run", J.JStr{run_str(X.Rt.run(x))}), J.kv("waiting", J.JBool{X.Rt.waiting(x)}),
    J.kv("turn_ms", jturn(X.Rt.turn(x)))])

def set_run(-o: M.St, +cx: X.Cx<o>, +n: String, +run: M.Run) -> X.Cx<o>:
  set_rt(o, cx, X.Rt.with_run(rt(o, cx, n), run))

def has_agent(-o: M.St, +cx: X.Cx<o>, +n: String) -> Bool:
  Maybe.is_some(&2, M.Agent, M.get_agent(st(o, cx), n))

# the runtime change applies only to a known agent (like the Rust
# `if let Some(a) = self.st.agents.get_mut(..)`)
def if_agent(-o: M.St, known: Bool, +cx: X.Cx<o>, +x: X.Rt) -> X.Cx<o>:
  match known:
    case True{}:
      set_rt(o, cx, x)
    case False{}:
      cx

def upd_rt(-o: M.St, +cx: X.Cx<o>, +x: X.Rt) -> X.Cx<o>:
  if_agent(o, has_agent(o, cx, X.Rt.name(x)), cx, x)

def dirty(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  X.dirty(o, cx)

def now(-o: M.St, +cx: X.Cx<o>) -> Nat:
  X.now(o, cx)

# every event keeps the journal's invariants: a caller that is not sending
# a message proves them with `Unit{}, Unit{}`
def emit(-o: M.St, +cx: X.Cx<o>, +e: M.Ev, k: X.FreshEv(X.d(o, cx), e), kq: X.QuietEv(e), kv: X.DelivEv(X.d(o, cx), e)) -> X.Cx<o>:
  X.emit(o, cx, e, k, kq, kv)

# ---- env queries (replayed answers, see hub/main.bend) ----

def ask.go(-o: M.St, ans: List<&2, J.Json>, +cx: X.Cx<o>, +q: J.Json) -> X.Cx<o> & Maybe<&2, J.Json>:
  match ans:
    case Con{+a, +rest}:
      (set_r(o, cx, X.R.with_ans(rr(o, cx), rest)), Some{a})
    case Nil{}:
      (set_r(o, cx, X.R.with_need(rr(o, cx), Maybe.or(&2, J.Json, X.R.need(rr(o, cx)), Some{q}))), None{})

# the answer to an env query, or None: the step halts and the daemon
# replays the input with the answer
def ask(-o: M.St, +cx: X.Cx<o>, +q: J.Json) -> X.Cx<o> & Maybe<&2, J.Json>:
  ask.go(o, X.R.ans(rr(o, cx)), cx, q)

# ---- status and relation ----

def status_of_decl(d: Maybe<&2, M.Decl>) -> String:
  match d:
    case Some{M.DDone{}}:
      "done"
    case Some{M.DBlocked{}}:
      "blocked"
    case _:
      "idle"

def status_of_run(+a: M.Agent, x: X.Rt) -> String:
  match x:
    case X.Rt{_, run, +w, _}:
      match run:
        case M.RDown{}:
          "starting"
        case M.RStarting{}:
          "starting"
        case M.RBusy{}:
          Bool.pick(String, w, "waiting", "working")
        case M.RIdle{}:
          status_of_decl(M.Agent.declared(a))

# the status every view shows (model.rs Agent::status)
def status_str(a: M.Agent, +x: X.Rt) -> String:
  match a:
    case M.Agent{+n, +im, +pa, life, +de, +al, +fi, +ws, +di, +br, +cr, +fa, +no, +rp, +sn, +fo}:
      match life:
        case M.LArchived{}:
          "archived"
        case M.LStopped{}:
          "stopped"
        case M.LFailed{}:
          "failed"
        case M.LActive{}:
          status_of_run(M.Agent{n, im, pa, M.LActive{}, de, al, fi, ws, di, br, cr, fa, no, rp, sn, fo}, x)

# the relation of the sender to the recipient, as the recipient reads it
# (prompts.rs relation)
def relation(+from: String, +to: String, +fp: Maybe<&2, String>, +tp: Maybe<&2, String>) -> String:
  Bool.pick(String, String.eq(from, M.USER()), "user",
    Bool.pick(String, String.eq(from, M.HUB()), "hub",
      Bool.pick(String, String.eq(from, M.GITHUB()), "peer",
      Bool.pick(String, M.maybe_str_is(tp, from) || (String.eq(from, M.MAIN()) && Bool.not(String.eq(to, M.MAIN()))), "parent",
        Bool.pick(String, M.maybe_str_is(fp, to) || String.eq(to, M.MAIN()), "child", "peer")))))

def parent_of(m: Maybe<&2, M.Agent>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+a}:
      M.Agent.parent(a)

# ---- notes and cards ----

def note_main(-o: M.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  emit(o, cx, M.EMainNote{text, now(o, cx)}, Unit{}, Unit{}, Unit{})

# opens card number next_card (the caller reads it first when it needs it)
# in the user's inbox: `kind` is a user kind (M.user_kind), never an
# agent's traffic (BISE-299)
#
# open_card_at: the same, about a place (and its PR): the hub's own items
def open_card_at(-o: M.St, +cx: X.Cx<o>, +kind: String, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>, +place: Maybe<&2, String>, +pr: Maybe<&2, Nat>) -> X.Cx<o>:
  +id = M.St.next_card(st(o, cx))
  +cx2 = emit(o, cx, M.ECardOpened{M.Card{id, kind, agent, text, for_msg, now(o, cx), place, pr}}, {==}, Unit{}, Unit{})
  line(o, cx2, M.MAIN(), "card", cat(["#", ns(id), " ", kind, " @", agent, " : ", text]))

def open_card(-o: M.St, +cx: X.Cx<o>, +kind: String, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>) -> X.Cx<o>:
  open_card_at(o, cx, kind, agent, text, for_msg, None{}, None{})

# the hub's Rust side opens one of its items (`card_open`: a PR ready to
# merge, pr-design §6.3); a kind outside the user's inbox opens nothing
# (BISE-299)
def hub_card(-o: M.St, +cx: X.Cx<o>, +kind: String, +agent: String, +text: String, +place: Maybe<&2, String>, +pr: Maybe<&2, Nat>) -> X.Cx<o>:
  pick_lazy(X.Cx<o>, M.choice_kind(kind),
    u => open_card_at(o, cx, kind, agent, text, None{}, place, pr),
    u => notice(o, cx, String.append("not a hub item: ", kind)))

# BISE-299, the hook for tool-call confirmations: a tool call that needs
# the user opens a `confirm` card for `agent` straight in the user's
# inbox, whoever runs it (the approvals gate, approvals-design.md §9);
# the user's answer goes to the gate (answer.kind: a `confirm` effect).
def open_confirm(-o: M.St, +cx: X.Cx<o>, +agent: String, +text: String) -> X.Cx<o>:
  open_card(o, cx, "confirm", agent, text, None{})

def close_ids(-o: M.St, ids: List<&2, Nat>, +cx: X.Cx<o>, +res: String) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      +cx2 = emit(o, cx, M.ECardClosed{id, res}, Unit{}, Unit{}, Unit{})
      close_ids(o, rest, line(o, cx2, M.MAIN(), "card-closed", cat(["#", ns(id), " ", res])), res)

# the cards of `agent` whose kind is in `kinds` (Nil: every kind)
def card_ids(cs: List<&2, M.Card>, +agent: String, +kinds: List<&2, String>) -> List<&2, Nat>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      +rest = card_ids(t, agent, kinds)
      +hit = String.eq(M.Card.agent(c), agent) && (List.is_empty(&2, String, kinds) || M.str_in(kinds, M.Card.kind(c)))
      Bool.pick(List<&2, Nat>, hit, M.Card.id(c) <> rest, rest)

def close_cards(-o: M.St, +cx: X.Cx<o>, +agent: String, +kinds: List<&2, String>, +res: String) -> X.Cx<o>:
  close_ids(o, card_ids(M.St.cards(st(o, cx)), agent, kinds), cx, res)

# ---- REPL lifecycle ----

def spawn.if(-o: M.St, down: Bool, +cx: X.Cx<o>, +n: String, +resume: Bool) -> X.Cx<o>:
  match down:
    case True{}:
      +cx2 = set_run(o, cx, n, M.RStarting{})
      dirty(o, out(o, cx2, [J.kv("fx", J.JStr{"spawn"}), J.kv("agent", J.JStr{n}),
        J.kv("resume", J.JBool{resume}), J.kv("crash_note", J.JNull{})]))
    case False{}:
      cx

def spawn(-o: M.St, +cx: X.Cx<o>, +n: String, +resume: Bool) -> X.Cx<o>:
  spawn.if(o, has_agent(o, cx, n) && is_down(X.Rt.run(rt(o, cx, n))), cx, n, resume)

# ---- waiters ----

# a wait gets its message (the message is not marked delivered here)
def waiter_reply(-o: M.St, +cx: X.Cx<o>, +w: X.Waiter, +m: M.Msg, +kind: String) -> X.Cx<o>:
  +cx3 = reply(o, cx, X.Waiter.token(w), ok_body([
    J.kv("type", J.JStr{kind}), J.kv("from", J.JStr{M.Msg.from(m)}),
    J.kv("message_id", J.JStr{mid(M.Msg.id(m))}),
    J.kv("thread", J.JStr{String.append("t_", ns(M.Msg.thread(m)))}),
    J.kv("expects_reply", J.JBool{M.Msg.expect(m)}), J.kv("auto", J.JBool{M.Msg.auto(m)}),
    J.kv("asked", J.JStr{mid(X.Waiter.msg(w))}),
    J.kv("message", J.JStr{M.Msg.text(m)})]))
  +cx4 = line(o, cx3, X.Waiter.agent(w), "msg-in", cat([M.Msg.from(m), " ", mid(M.Msg.id(m)), " : ", M.Msg.text(m)]))
  +still = any_waiter(X.R.waiters(rr(o, cx4)), X.Waiter.agent(w))
  +x = X.Rt.with_waiting(rt(o, cx4, X.Waiter.agent(w)), still)
  dirty(o, upd_rt(o, cx4, x))

def mark_delivered.go(-o: M.St, +cx: X.Cx<o>, +id: Nat, c: Bool, +e: {X.queued_at(X.d(o, cx), id) == c : Bool}) -> X.Cx<o>:
  match c:
    case True{}:
      emit(o, cx, M.EMsgState{id, M.Delivered{}}, Unit{}, Unit{}, e)
    case False{}:
      cx

# a message becomes delivered only while it is queued (law
# delivered_once): the check never fails in a state the hub reached
def mark_delivered(-o: M.St, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  mark_delivered.go(o, cx, id, X.queued_at(X.d(o, cx), id), {==})

def deliver_to_waiter(-o: M.St, +cx: X.Cx<o>, +w: X.Waiter, +m: M.Msg, +kind: String) -> X.Cx<o>:
  waiter_reply(o, mark_delivered(o, cx, M.Msg.id(m)), w, m, kind)

def reply_all(-o: M.St, ws: List<&2, X.Waiter>, +cx: X.Cx<o>, +why: String) -> X.Cx<o>:
  match ws:
    case Nil{}:
      cx
    case Con{+w, +rest}:
      reply_all(o, rest, reply_err(o, cx, X.Waiter.token(w), why), why)

def fail_waiters(-o: M.St, +cx: X.Cx<o>, +agent: String, +why: String) -> X.Cx<o>:
  +ws = X.R.waiters(rr(o, cx))
  +cx2 = set_r(o, cx, X.R.with_waiters(rr(o, cx), waiters_not(ws, agent)))
  +cx3 = reply_all(o, waiters_of(ws, agent), cx2, why)
  upd_rt(o, cx3, X.Rt.with_waiting(rt(o, cx3, agent), False{}))

# ---- delivery (RFC 0003 §6.1, RFC 0001 §10.1) ----

def not_queued_mode(xs: List<&2, M.Msg>) -> List<&2, M.Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = not_queued_mode(t)
      Bool.pick(List<&2, M.Msg>, M.Msg.queued(h), rest, h <> rest)

def any_from_user(xs: List<&2, M.Msg>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      String.eq(M.Msg.from(h), M.USER()) || any_from_user(t)

def leftover_at.go(m: Maybe<&2, M.MSt>) -> Bool:
  match m:
    case Some{M.Queued{+r}}:
      String.eq(r, "steer_leftover")
    case _:
      False{}

# a steer_leftover was steered in a turn that never read it: its feed line
# was written then (qa-explore bug A)
def leftover_at(+s: M.St, +id: Nat) -> Bool:
  leftover_at.go(M.state_of(s, id))

# the feed line of a delivered message: once, when it is first handed over
def deliver.line(-o: M.St, again: Bool, you: Bool, +cx: X.Cx<o>, +name: String, +m: M.Msg) -> X.Cx<o>:
  match again you:
    case True{} _:
      cx
    case False{} True{}:
      line(o, cx, name, "you", M.Msg.text(m))
    case False{} False{}:
      line(o, cx, name, "msg-in", cat([M.Msg.from(m), " ", mid(M.Msg.id(m)), " : ", M.Msg.text(m)]))

def deliver_each(-o: M.St, batch: List<&2, M.Msg>, +cx: X.Cx<o>, +name: String, +acc: List<&2, J.Json>) -> X.Cx<o> & List<&2, J.Json>:
  match batch:
    case Nil{}:
      (cx, acc)
    case Con{+m, +rest}:
      +s = st(o, cx)
      +rel = relation(M.Msg.from(m), M.Msg.to(m), parent_of(M.get_agent(s, M.Msg.from(m))), parent_of(M.get_agent(s, M.Msg.to(m))))
      +cx2 = mark_delivered(o, cx, M.Msg.id(m))
      +cx3 = deliver.line(o, leftover_at(s, M.Msg.id(m)), M.Msg.plain(m) || Maybe.is_some(&2, String, M.Msg.via(m)), cx2, name, m)
      deliver_each(o, rest, cx3, name, M.snoc(J.Json, acc, J.obj([J.kv("id", J.JNum{M.Msg.id(m)}), J.kv("rel", J.JStr{rel})])))

def pump.flush(-o: M.St, flush: Bool, +cx: X.Cx<o>) -> X.Cx<o>:
  match flush:
    case True{}:
      emit(o, cx, M.ENotesFlushed{}, Unit{}, Unit{}, Unit{})
    case False{}:
      cx

def pump.send.go(-o: M.St, busy: Bool, +cx: X.Cx<o>, +name: String, +ids: List<&2, Nat>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> X.Cx<o>:
  match busy:
    case True{}:
      +r = rr(o, cx)
      +cx2 = set_r(o, cx, X.R.with_steered(r, steer_add(X.R.steered(r), name, ids)))
      dirty(o, out(o, cx2, J.kv("fx", J.JStr{"deliver"}) <> J.kv("mode", J.JStr{"steer"}) <> kvs))
    case False{}:
      # the turn starts now: nothing else may `say` meanwhile
      +cx2 = upd_rt(o, cx, X.Rt{name, M.RBusy{}, X.Rt.waiting(rt(o, cx, name)), Some{now(o, cx)}})
      dirty(o, out(o, cx2, J.kv("fx", J.JStr{"deliver"}) <> J.kv("mode", J.JStr{"say"}) <> kvs))

def pump.send(-o: M.St, busy: Bool, p: X.Cx<o> & List<&2, J.Json>, +name: String, +ids: List<&2, Nat>, +kvs0: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> X.Cx<o>:
  match p:
    case (+cx, +msgs):
      pump.send.go(o, busy, cx, name, ids, M.snoc(Sigma<&2, &2, String, _ => J.Json>, kvs0, J.kv("msgs", J.JArr{msgs})))

def jstrs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.JStr{h} <> jstrs(t)

def pump.batch(-o: M.St, batch: List<&2, M.Msg>, +cx: X.Cx<o>, +name: String, +busy: Bool, +is_main: Bool) -> X.Cx<o>:
  match batch:
    case Nil{}:
      cx
    case Con{+h, +t}:
      +b = {h <> t : List<&2, M.Msg>}
      +notes = Bool.pick(List<&2, String>, is_main, M.St.notes(st(o, cx)), Nil{})
      +cx2 = pump.flush(o, Bool.not(List.is_empty(&2, String, notes)), cx)
      +status = is_main && any_from_user(b)
      pump.send(o, busy, deliver_each(o, b, cx2, name, Nil{}), name, ids_of(b), [
        J.kv("agent", J.JStr{name}),
        J.kv("notes", J.JArr{jstrs(notes)}),
        J.kv("status", J.JBool{status})])

def pump.run(-o: M.St, run: M.Run, +cx: X.Cx<o>, +name: String, +queued: List<&2, M.Msg>, +is_main: Bool) -> X.Cx<o>:
  match run:
    case M.RDown{}:
      cx
    case M.RStarting{}:
      cx
    case M.RBusy{}:
      # a queued-mode message waits for the end of the turn
      pump.batch(o, not_queued_mode(queued), cx, name, True{}, is_main)
    case M.RIdle{}:
      pump.batch(o, queued, cx, name, False{}, is_main)

def pump.active(-o: M.St, active: Bool, +cx: X.Cx<o>, +name: String, +is_main: Bool) -> X.Cx<o>:
  match active:
    case True{}:
      pump.run(o, X.Rt.run(rt(o, cx, name)), cx, name, M.queued_for(st(o, cx), name), is_main)
    case False{}:
      cx

def pump.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      pump.active(o, is_active(M.Agent.life(a)), cx, name, M.Agent.is_main(a))

# deliver what is queued for `name`, when its state allows it
def pump(-o: M.St, +cx: X.Cx<o>, +name: String) -> X.Cx<o>:
  pump.agent(o, M.get_agent(st(o, cx), name), cx, name)

# ---- messages (RFC 0003) ----

def serr(-o: M.St, +cx: X.Cx<o>, +e: String) -> X.Cx<o> & SendR:
  (cx, SErr{e})

def send.waiter(-o: M.St, mw: Maybe<&2, X.Waiter>, +cx: X.Cx<o>, +m: M.Msg) -> X.Cx<o> & SendR:
  match mw:
    case Some{+w}:
      # a waiting recipient gets it as the result of its `sb wait`
      +r = rr(o, cx)
      +cx2 = set_r(o, cx, X.R.with_waiters(r, waiter_drop(X.R.waiters(r), X.Waiter.token(w))))
      +kind = Bool.pick(String, M.maybe_nat_is(M.Msg.reply_to(m), X.Waiter.msg(w)), "reply", "incoming_request")
      (deliver_to_waiter(o, cx2, w, m, kind), SOk{m})
    case None{}:
      (pump(o, cx, M.Msg.to(m)), SOk{m})

def quiet_ok(a: Bool, e: Bool) -> {Bool.and(a, Bool.and(e, Bool.not(a))) == False{} : Bool}:
  match a e:
    case True{} True{}:
      {==}
    case True{} False{}:
      {==}
    case False{} _:
      {==}

def send.emit(-o: M.St, +cx: X.Cx<o>, +s: Snd, +thread: Nat) -> X.Cx<o> & SendR:
  match s:
    case Snd{+from, +to, +text, +expect, +reply_to, +plain, +auto, +queued, +via}:
      # an automatic reply never asks (law auto_never_asks); the id is the
      # next free one (law msg_ids_fresh)
      +m = {M.Msg{M.St.next_msg(st(o, cx)), thread, from, to, reply_to, Bool.and(expect, Bool.not(auto)), auto, text, now(o, cx), plain, queued, via} : M.Msg}
      # a card is closed by the user's answer only (answer_card): no reply
      # here touches the user's inbox (BISE-299)
      +cx2 = emit(o, cx, M.EMsgSent{m}, {==}, quiet_ok(auto, expect), Unit{})
      +w = Bool.pick(Maybe<&2, X.Waiter>, queued, None{},
        waiter_pick(X.R.waiters(rr(o, cx2)), to, reply_to, expect && Bool.not(plain) && Bool.not(String.eq(from, M.USER()))))
      send.waiter(o, w, cx2, m)

def send.orig(-o: M.St, m: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +s: Snd, +r: Nat) -> X.Cx<o> & SendR:
  match m:
    case None{}:
      serr(o, cx, String.append("reply_to inconnu : ", mid(r)))
    case Some{+orig}:
      send.emit(o, cx, s, M.Msg.thread(orig))

def send.thread(-o: M.St, reply_to: Maybe<&2, Nat>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match reply_to:
    case None{}:
      send.emit(o, cx, s, M.St.next_msg(st(o, cx)))
    case Some{+r}:
      send.orig(o, M.msg_of(st(o, cx), r), cx, s, r)

def Snd.to(x: Snd) -> String:
  match x:
    case Snd{_, +to, _, _, _, _, _, _, _}:
      to

def Snd.reply_to(x: Snd) -> Maybe<&2, Nat>:
  match x:
    case Snd{_, _, _, _, +reply_to, _, _, _, _}:
      reply_to

def send.revive(-o: M.St, ok: Bool, +cx: X.Cx<o>, +s: Snd, +why: String) -> X.Cx<o> & SendR:
  match ok:
    case False{}:
      serr(o, cx, why)
    case True{}:
      +to = Snd.to(s)
      +cx2 = emit(o, cx, M.ELifecycle{to, M.LActive{}, None{}}, Unit{}, Unit{}, Unit{})
      +r = rr(o, cx2)
      +cx3 = set_r(o, cx2, X.R.with_crashes(r, kn_del(X.R.crashes(r), to)))
      send.thread(o, Snd.reply_to(s), spawn(o, cx3, to, True{}), s)

def dropped_wt(+a: M.Agent) -> Bool:
  M.ws_worktree(M.Agent.ws(a)) && M.ws_dropped(M.Agent.ws(a))

def unavailable(-o: M.St, +cx: X.Cx<o>, +s: Snd, +a: M.Agent) -> String:
  cat(["recipient_unavailable: @", Snd.to(s), " is ", status_str(a, rt(o, cx, Snd.to(s))),
    Bool.pick(String, dropped_wt(a), " (its worktree was deleted: /restore)", "")])

def create.sent(-o: M.St, p: X.Cx<o> & SendR, +name: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, SOk{_}):
      (cx, TOk{name})
    case (+cx, SErr{+e}):
      (cx, TErr{e})

def NewTask.parent(x: NewTask) -> String:
  match x:
    case NewTask{+parent, _, _, _, _, _, _}:
      parent

def NewTask.objective(x: NewTask) -> String:
  match x:
    case NewTask{_, _, _, _, +objective, _, _}:
      objective

def NewTask.brief(x: NewTask) -> J.Json:
  match x:
    case NewTask{_, _, +brief, _, _, _, _}:
      brief

def NewTask.brief_text(x: NewTask) -> String:
  match x:
    case NewTask{_, _, _, +brief_text, _, _, _}:
      brief_text

def branch_label(b: J.Json) -> String:
  match b:
    case J.JStr{+s}:
      cat([" (worktree ", s, ")"])
    case _:
      ""

def Snd.from(x: Snd) -> String:
  match x:
    case Snd{+from, _, _, _, _, _, _, _, _}:
      from

def send.life(-o: M.St, life: M.Life, +cx: X.Cx<o>, +s: Snd, +a: M.Agent) -> X.Cx<o> & SendR:
  match life:
    case M.LActive{}:
      send.thread(o, Snd.reply_to(s), cx, s)
    case M.LFailed{}:
      send.revive(o, String.eq(Snd.from(s), M.USER()) || M.maybe_str_is(M.Agent.parent(a), Snd.from(s)), cx, s, unavailable(o, cx, s, a))
    case M.LStopped{}:
      send.revive(o, String.eq(Snd.from(s), M.USER()) && Bool.not(dropped_wt(a)), cx, s, unavailable(o, cx, s, a))
    case M.LArchived{}:
      send.revive(o, String.eq(Snd.from(s), M.USER()) && Bool.not(dropped_wt(a)), cx, s, unavailable(o, cx, s, a))

def send.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match m:
    case None{}:
      serr(o, cx, String.append("recipient_unknown: no agent named ", Snd.to(s)))
    case Some{+a}:
      send.life(o, M.Agent.life(a), cx, s, a)

def send.checked(-o: M.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match pre:
    case Some{+e}:
      serr(o, cx, e)
    case None{}:
      send.agent(o, M.get_agent(st(o, cx), Snd.to(s)), cx, s)

def Snd.text(x: Snd) -> String:
  match x:
    case Snd{_, _, +text, _, _, _, _, _, _}:
      text

def Snd.with_to(x: Snd, +v: String) -> Snd:
  match x:
    case Snd{+from, _, +text, +expect, +reply_to, +plain, +auto, +queued, +via}:
      Snd{from, v, text, expect, reply_to, plain, auto, queued, via}

def send.known(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match m:
    case None{}:
      serr(o, cx, String.append("recipient_unknown: no agent named ", Snd.to(s)))
    case Some{+to}:
      +pre = Bool.pick(Maybe<&2, String>, String.eq(to, Snd.from(s)), Some{"invalid_recipient: an agent cannot write to itself"},
        Bool.pick(Maybe<&2, String>, M.trim_is_empty(Snd.text(s)), Some{"message vide"}, None{}))
      send.checked(o, pre, cx, Snd.with_to(s, to))

def send.user(-o: M.St, user: Bool, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match user:
    case True{}:
      serr(o, cx, "invalid_recipient: to talk to the user, go through main (sb card)")
    case False{}:
      send.known(o, M.resolve(st(o, cx), Snd.to(s)), cx, s)

# ---- main's feed (BISE-04, hub line protocol v2, contract C2) ----

# a hub line whose text is several fields joined by " : " (the daemon
# escapes " : " inside each field, see core.rs)
def line_fields(-o: M.St, +cx: X.Cx<o>, +agent: String, +kind: String, +fields: List<&2, String>) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"line"}), J.kv("agent", J.JStr{agent}), J.kv("kind", J.JStr{kind}),
    J.kv("fields", J.JArr{jstrs(fields)})])

# C2 amendment (BISE-86): a message from the user that cannot reach its
# agent (stopped, dropped, archived) says so in the feed where the user
# wrote it: `undelivered : {name} : {text}` (the TUI marks the line ✗)
def undelivered.if(-o: M.St, gone: Bool, +cx: X.Cx<o>, +feed: String, +name: String, +text: String) -> X.Cx<o>:
  match gone:
    case True{}:
      line_fields(o, cx, feed, "undelivered", [name, text])
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
def undelivered_msgs(-o: M.St, xs: List<&2, M.Msg>, +cx: X.Cx<o>) -> X.Cx<o>:
  match xs:
    case Nil{}:
      cx
    case Con{+m, +rest}:
      +to = M.Msg.to(m)
      undelivered_msgs(o, rest, undelivered.if(o, String.eq(M.Msg.from(m), M.USER()), cx,
        undelivered.feed(M.Msg.via(m), to), to, M.Msg.text(m)))

def unavailable_err(+e: String) -> Bool:
  String.starts_with(e, "recipient_unavailable")

# the message `m` points to (a card's, a reply's)
def for_msg_of(+s: M.St, m: Maybe<&2, Nat>) -> Maybe<&2, M.Msg>:
  match m:
    case None{}:
      None{}
    case Some{+id}:
      M.msg_of(s, id)

# the question `to` asked main, that `m` answers
def asked_main(q: Maybe<&2, M.Msg>, +to: String) -> Bool:
  match q:
    case None{}:
      False{}
    case Some{+x}:
      String.eq(M.Msg.from(x), to) && String.eq(M.Msg.to(x), M.MAIN()) && M.Msg.expect(x)

def question_text(q: Maybe<&2, M.Msg>) -> String:
  match q:
    case None{}:
      ""
    case Some{+x}:
      M.Msg.text(x)

# main's feed shows every message between agents (including between two
# tasks), so what happened stays readable: `msg : from → to m_<n> : text`
# (the id, BISE-110: the TUI hides main's `sb send` box when its message
# shows; an older TUI reads `to m_<n>` as the receiver's name).
# What main receives already shows as `msg-in` when delivered; main
# answering a task's question shows as `answered` (level 2) instead.
def feed_main(-o: M.St, +cx: X.Cx<o>, +m: M.Msg, +why: String) -> X.Cx<o>:
  +from = M.Msg.from(m)
  +to = M.Msg.to(m)
  +q = for_msg_of(st(o, cx), M.Msg.reply_to(m))
  +answered = String.eq(from, M.MAIN()) && asked_main(q, to)
  +agents = Bool.not(String.eq(from, M.USER())) && Bool.not(String.eq(to, M.MAIN()))
  pick_lazy(X.Cx<o>, answered,
    u => line_fields(o, cx, M.MAIN(), "answered", [to, question_text(q), M.Msg.text(m), why]),
    u => pick_lazy(X.Cx<o>, agents,
      v => line(o, cx, M.MAIN(), "msg", cat([from, " → ", to, " ", mid(M.Msg.id(m)), " : ", M.Msg.text(m)])),
      v => cx))

# What a task sent is a fact of its own thread (desktop bar A.3, S16;
# architect m_10203): a message from a task (not main, whose feed has
# `msg` for every message, not the user) to another agent writes one
# line in the task's own feed, `sent : {to} : m_<n> : {ask} : {text}`
# (ask 1: it waits for a reply, --expect-reply or sb ask). A message to
# the user is `msg-you` already. A view only: no state changes.
# The sender must be an agent of this hub (architect m_10280): '@bise'
# (a cross-hub delegation) or the hub's own name has no feed here.
def is_task_send(-o: M.St, +cx: X.Cx<o>, +from: String, +to: String) -> Bool:
  has_agent(o, cx, from) && Bool.not(String.eq(from, M.MAIN())) && Bool.not(String.eq(to, M.USER()))

def feed_sender(-o: M.St, +cx: X.Cx<o>, +m: M.Msg) -> X.Cx<o>:
  +from = M.Msg.from(m)
  +to = M.Msg.to(m)
  pick_lazy(X.Cx<o>, is_task_send(o, cx, from, to),
    u => line_fields(o, cx, from, "sent", [to, mid(M.Msg.id(m)), Bool.pick(String, M.Msg.expect(m), "1", "0"), M.Msg.text(m)]),
    u => cx)

def send.fed(-o: M.St, p: X.Cx<o> & SendR, +why: String) -> X.Cx<o> & SendR:
  match p:
    case (+cx, SOk{+m}):
      (feed_main(o, feed_sender(o, cx, m), m, why), SOk{m})
    case (+cx, SErr{+e}):
      (cx, SErr{e})

# send a message, with main's reason when it answers for the user
# (`sb send --why`); answers it, or why it was refused
def send_why(-o: M.St, +cx: X.Cx<o>, +s: Snd, +why: String) -> X.Cx<o> & SendR:
  send.fed(o, send.user(o, String.eq(Snd.to(s), M.USER()), cx, s), why)

# send a message; answers it, or why it was refused
def send(-o: M.St, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  send_why(o, cx, s, "")

def msg(+from: String, +to: String, +text: String, +expect: Bool, +reply_to: Maybe<&2, Nat>, +plain: Bool, +auto: Bool) -> Snd:
  Snd{from, to, text, expect, reply_to, plain, auto, False{}, None{}}

def fst(-o: M.St, p: X.Cx<o> & SendR) -> X.Cx<o>:
  match p:
    case (+cx, _):
      cx

# a notification of the hub to main: it wakes main like any message (a
# main note waits for main's next turn instead)
def notify_main(-o: M.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  fst(o, send(o, cx, msg(M.HUB(), M.MAIN(), text, False{}, None{}, False{}, False{})))

def create.go.new(-o: M.St, +cx: X.Cx<o>, +t: NewTask, +name: String, +ws: J.Json, +e: {M.name_taken(X.d(o, cx), name) == False{} : Bool}) -> X.Cx<o> & TaskR:
  +parent = NewTask.parent(t)
  +label = branch_label(M.ws_branch(ws))
  +obj = M.one_line(NewTask.objective(t))
  +cx2 = emit(o, cx, M.ETaskCreated{name, parent, NewTask.brief(t), ws, now(o, cx)}, e, Unit{}, Unit{})
  +who = Bool.pick(String, String.eq(parent, M.USER()), "you", parent)
  +cx3 = line(o, cx2, M.MAIN(), "spawn", cat([who, " → new agent @", name, label, " : ", obj]))
  +cx4 = pick_lazy(X.Cx<o>, String.eq(parent, M.USER()), u => note_main(o, cx3, cat(["the user created the agent @", name, label, ": ", obj])), u => cx3)
  +cx5 = spawn(o, cx4, name, False{})
  create.sent(o, send(o, cx5, msg(parent, name, cat(["# Task `", name, "`\n\n", NewTask.brief_text(t)]), String.eq(parent, M.MAIN()), None{}, String.eq(parent, M.USER()), False{})), name)

def create.go.free(-o: M.St, taken: Bool, +cx: X.Cx<o>, +t: NewTask, +name: String, +ws: J.Json, +e: {M.name_taken(X.d(o, cx), name) == taken : Bool}) -> X.Cx<o> & TaskR:
  match taken:
    case True{}:
      (cx, TErr{String.append("name already taken: ", name)})
    case False{}:
      create.go.new(o, cx, t, name, ws, e)

# the name is still free (unique_name found it; it can only be taken if
# its search ran out)
def create.go(-o: M.St, +cx: X.Cx<o>, +t: NewTask, +name: String, +ws: J.Json) -> X.Cx<o> & TaskR:
  create.go.free(o, M.name_taken(st(o, cx), name), cx, t, name, ws, {==})

def ans_ok.go(m: Maybe<&2, J.Json>, +j: J.Json) -> Result<&2, &2, String, J.Json>:
  match m:
    case Some{+v}:
      Done{v}
    case None{}:
      Fail{J.get_str_or(j, "err", "git error")}

def ans_ok(+j: J.Json) -> Result<&2, &2, String, J.Json>:
  ans_ok.go(J.get(j, "ok"), j)

def create.ws.res(-o: M.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match r:
    case Fail{+e}:
      (cx, TErr{e})
    case Done{+ws}:
      create.go(o, cx, t, name, ws)

def create.ws(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, None{}):
      (cx, THalt{})
    case (+cx, Some{+a}):
      create.ws.res(o, ans_ok(a), cx, t, name)

def NewTask.with_changes(x: NewTask) -> Bool:
  match x:
    case NewTask{_, _, _, _, _, _, +with_changes}:
      with_changes

def create.git(-o: M.St, git: Bool, +cx: X.Cx<o>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match git:
    case False{}:
      (cx, TErr{"the workspace is not a git repository: no worktree possible"})
    case True{}:
      create.ws(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_create"}), J.kv("name", J.JStr{name}),
        J.kv("with_changes", J.JBool{NewTask.with_changes(t)})])), t, name)

def shared_ws(+path: String) -> J.Json:
  J.obj([J.kv("mode", J.JStr{"shared"}), J.kv("path", J.JStr{path}), J.kv("branch", J.JNull{}),
    J.kv("base_commit", J.JNull{}), J.kv("dropped", J.JBool{False{}})])

def create.wt(-o: M.St, wt: Bool, +cx: X.Cx<o>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match wt:
    case False{}:
      create.go(o, cx, t, name, shared_ws(X.R.workspace(rr(o, cx))))
    case True{}:
      create.git(o, X.R.git(rr(o, cx)), cx, t, name)

def NewTask.worktree(x: NewTask) -> Bool:
  match x:
    case NewTask{_, _, _, _, _, +worktree, _}:
      worktree

def NewTask.base(x: NewTask) -> String:
  match x:
    case NewTask{_, +base, _, _, _, _, _}:
      base

def strip_dash.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{+c, +t}:
      Bool.pick(String, Char.is_eq(c, '-'), strip_dash.go(t), SCon{c, t})

def strip_dash(+s: String) -> String:
  String.reverse(strip_dash.go(String.reverse(s)))

def candidate(+base: String, +i: Nat) -> String:
  +suffix = String.append("-", ns(i))
  String.append(strip_dash(String.take(base, Nat.sub(24n, String.length(suffix)))), suffix)

def unique_name.go(fuel: Nat, +s: M.St, +base: String, +i: Nat) -> String:
  match fuel:
    case 0n:
      candidate(base, i)
    case 1n+f:
      Bool.pick(String, M.name_taken(s, candidate(base, i)), unique_name.go(f, s, base, 1n+i), candidate(base, i))

def unique_name(+s: M.St, +base: String) -> String:
  Bool.pick(String, M.name_taken(s, base), unique_name.go(1000n, s, base, 2n), base)

# RFC 0001 §7.1 and RFC 0002 §3-4 (the name was validated by the daemon)
def create_task(-o: M.St, +cx: X.Cx<o>, +t: NewTask) -> X.Cx<o> & TaskR:
  create.wt(o, NewTask.worktree(t), cx, t, unique_name(st(o, cx), NewTask.base(t)))

# `sb spawn --place <agent>|<branch>` (dev-flow §3.1): the new task joins
# that place (`join`: its ws, found by the daemon); else as create_task
def create_in(-o: M.St, +cx: X.Cx<o>, +t: NewTask, j: Maybe<&2, J.Json>) -> X.Cx<o> & TaskR:
  match j:
    case Some{J.JObj{+kvs}}:
      create.go(o, cx, t, unique_name(st(o, cx), NewTask.base(t)), J.JObj{kvs})
    case _:
      create_task(o, cx, t)

# ---- timers (`sb every`, hub/timers.bend) ----
# The steps of the timers, wired around timers.bend's pure decisions.
# They live here, not in timers.bend: they call send, pump, emit and ask,
# which are core's, and timers.bend (imported by model.bend) cannot
# import core (Bend has no import cycles).

def tms(-o: M.St, +cx: X.Cx<o>) -> TM.Tms:
  M.St.tm(st(o, cx))

def live(-o: M.St, +cx: X.Cx<o>) -> List<&2, TM.Timer>:
  TM.Tms.live(tms(o, cx))

def from_bise(xs: List<&2, M.Msg>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      String.eq(M.Msg.from(h), M.HUB()) || from_bise(t)

# a message from bise is queued for `n` (a wake not delivered yet)
def bise_queued(+s: M.St, +n: String) -> Bool:
  from_bise(M.queued_for(s, n))

def timer_agent.go(+s: M.St, m: Maybe<&2, String>) -> Maybe<&2, M.Agent>:
  match m:
    case None{}:
      None{}
    case Some{+n}:
      M.get_agent(s, n)

# the agent of a timer (by its name or an old one)
def timer_agent(+s: M.St, +n: String) -> Maybe<&2, M.Agent>:
  timer_agent.go(s, M.resolve(s, n))

def life_gone(l: M.Life) -> Bool:
  match l:
    case M.LArchived{}:
      True{}
    case _:
      False{}

def ready.go(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      is_active(M.Agent.life(a)) && is_idle(X.Rt.run(rt(o, cx, M.Agent.name(a))))

# active and idle: a wake reaches it now
def timer_ready(-o: M.St, +cx: X.Cx<o>, +n: String) -> Bool:
  ready.go(o, timer_agent(st(o, cx), n), cx)

def gone.go(m: Maybe<&2, M.Agent>) -> Bool:
  match m:
    case None{}:
      True{}
    case Some{+a}:
      life_gone(M.Agent.life(a))

# unknown or archived: its timers end
def timer_gone(+s: M.St, +n: String) -> Bool:
  gone.go(timer_agent(s, n))

def timer_may(-o: M.St, +cx: X.Cx<o>, +t: TM.Timer, +due: Bool) -> Bool:
  TM.may_fire(bise_queued(st(o, cx), TM.Timer.agent(t)), due, t, timer_ready(o, cx, TM.Timer.agent(t)), now(o, cx))

def timer_spent(-o: M.St, spent: Bool, +cx: X.Cx<o>, +id: Nat, +at: Nat) -> X.Cx<o>:
  match spent:
    case True{}:
      emit(o, cx, M.ETimerStop{id, "it ran its times", at}, Unit{}, Unit{}, Unit{})
    case False{}:
      cx

# the wake reached the queue: a due one counts (and ends the timer when it
# ran its times, law times_bound), a run now is only a run
def timer_count(-o: M.St, due: Bool, +cx: X.Cx<o>, +t: TM.Timer, +next: Nat) -> X.Cx<o>:
  match due:
    case False{}:
      emit(o, cx, M.ETimerRun{TM.Timer.id(t), now(o, cx)}, Unit{}, Unit{}, Unit{})
    case True{}:
      timer_spent(o, TM.spent(t),
        emit(o, cx, M.ETimerFired{TM.Timer.id(t), 1n+TM.Timer.fired(t), next, now(o, cx), False{}}, Unit{}, Unit{}, Unit{}),
        TM.Timer.id(t), now(o, cx))

# a fire counts only when its message was queued (law fire_is_a_message,
# amb-tools m_5822: a one-shot timer was spent on a lost wake)
# the wake was refused (unreachable today, a safety net never driven live:
# timer_may wants it idle and active, and send then always queues): a due
# one is tried again in a minute, nothing counted, one line that says so
# (every_fired undelivered, the hub's warn); a run now is just not done
def timer_retry(-o: M.St, due: Bool, +cx: X.Cx<o>, +t: TM.Timer) -> X.Cx<o>:
  match due:
    case False{}:
      cx
    case True{}:
      emit(o, cx, M.ETimerFired{TM.Timer.id(t), TM.Timer.fired(t), Nat.add(now(o, cx), TM.MIN_MS()), TM.Timer.last(t), True{}},
        Unit{}, Unit{}, Unit{})

def timer_fire.sent(-o: M.St, p: X.Cx<o> & SendR, +t: TM.Timer, +due: Bool, +next: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SOk{_}):
      timer_count(o, due, cx, t, next)
    case (+cx, SErr{_}):
      timer_retry(o, due, cx, t)

def timer_fire.go(-o: M.St, ok: Bool, +cx: X.Cx<o>, +t: TM.Timer, +text: String, +due: Bool, +next: Nat) -> X.Cx<o>:
  match ok:
    case False{}:
      cx
    case True{}:
      timer_fire.sent(o, send(o, cx, msg(M.HUB(), TM.Timer.agent(t), text, False{}, None{}, False{}, False{})), t, due, next)

# the only way a timer wakes its agent: a due wake (`due`, counted, its
# next time `next`) from the tick, or /scheduled's run now; never while a
# wake from bise is still queued for it (law wake_never_stacked)
def timer_fire(-o: M.St, +cx: X.Cx<o>, +t: TM.Timer, +text: String, +due: Bool, +next: Nat) -> X.Cx<o>:
  timer_fire.go(o, timer_may(o, cx, t, due), cx, t, text, due, next)

# {"t":"every_set"}: the hub checked the request and computed `next_ms`
# (its clock); a timer only for an active agent
def timer_set.new(-o: M.St, s: Maybe<&2, TM.Sched>, +cx: X.Cx<o>, +agent: String, +i: J.Json) -> X.Cx<o>:
  match s:
    case None{}:
      cx
    case Some{+sc}:
      +t = {TM.Timer{1n+TM.Tms.last_id(tms(o, cx)), agent, C.get_s(i, "by"), C.get_s(i, "text"), sc,
        C.get_nat(i, "next_ms"), C.opt_nat(J.get(i, "until_ms")), C.opt_nat(J.get(i, "times")), 0n,
        C.opt_str(J.get(i, "page")), 0n, Nil{}, C.get_s(i, "name"), String.is_empty(C.get_s(i, "name"))} : TM.Timer}
      emit(o, cx, M.ETimerSet{t, now(o, cx)}, Unit{}, Unit{}, Unit{})

def timer_set.active(-o: M.St, ok: Bool, +cx: X.Cx<o>, +agent: String, +i: J.Json) -> X.Cx<o>:
  match ok:
    case False{}:
      cx
    case True{}:
      timer_set.new(o, C.sched_of(C.opt_nat(J.get(i, "every_ms")), C.opt_nat(J.get(i, "daily_min"))), cx, agent, i)

def timer_set.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +i: J.Json) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      timer_set.active(o, is_active(M.Agent.life(a)), cx, M.Agent.name(a), i)

def timer_set(-o: M.St, +cx: X.Cx<o>, +i: J.Json) -> X.Cx<o>:
  timer_set.agent(o, timer_agent(st(o, cx), C.get_s(i, "agent")), cx, i)

# {"t":"every_stop"}: `sb every --stop`, /scheduled's x; `wake` (the
# user's stop): what bise tells its agent, in the same step
def timer_stop.found(-o: M.St, m: Maybe<&2, TM.Timer>, +cx: X.Cx<o>, +id: Nat, +why: String, +wake: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+t}:
      +cx2 = emit(o, cx, M.ETimerStop{id, why, now(o, cx)}, Unit{}, Unit{}, Unit{})
      pick_lazy(X.Cx<o>, String.is_empty(wake), u => cx2,
        u => fst(o, send(o, cx2, msg(M.HUB(), TM.Timer.agent(t), wake, False{}, None{}, False{}, False{}))))

def timer_stop(-o: M.St, +cx: X.Cx<o>, +i: J.Json) -> X.Cx<o>:
  timer_stop.found(o, TM.find(live(o, cx), C.get_nat(i, "id")), cx, C.get_nat(i, "id"), C.get_s(i, "why"), C.get_s(i, "wake"))

# {"t":"every_run"}: /scheduled's run now, through timer_fire
def timer_run.found(-o: M.St, m: Maybe<&2, TM.Timer>, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+t}:
      timer_fire(o, cx, t, text, False{}, 0n)

def timer_run(-o: M.St, +cx: X.Cx<o>, +i: J.Json) -> X.Cx<o>:
  timer_run.found(o, TM.find(live(o, cx), C.get_nat(i, "id")), cx, C.get_s(i, "text"))

# {"t":"every_name"}: the hub's name for a live timer (its small model's,
# else its plain fallback); an ended or unknown one, or no name: nothing.
# A timer set without a name is held: its ◷ set line waits for this step,
# which announces it (architect m_15603: in sb-core, so a restart between
# set and name still writes it, once; law named_announces_once)
def timer_name.found(-o: M.St, m: Maybe<&2, TM.Timer>, +cx: X.Cx<o>, +id: Nat, +name: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+t}:
      pick_lazy(X.Cx<o>, String.is_empty(name), u => cx,
        u => emit(o, cx, M.ETimerName{id, name, TM.Timer.held(t), now(o, cx)}, Unit{}, Unit{}, Unit{}))

def timer_name(-o: M.St, +cx: X.Cx<o>, +i: J.Json) -> X.Cx<o>:
  timer_name.found(o, TM.find(live(o, cx), C.get_nat(i, "id")), cx, C.get_nat(i, "id"), C.get_s(i, "name"))

def timers_gone.each(-o: M.St, ids: List<&2, Nat>, +cx: X.Cx<o>, +why: String) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      timers_gone.each(o, rest, emit(o, cx, M.ETimerStop{id, why, now(o, cx)}, Unit{}, Unit{}, Unit{}), why)

def timers_gone.if(-o: M.St, archived: Bool, +cx: X.Cx<o>, +n: String, +ns: List<&2, String>) -> X.Cx<o>:
  match archived:
    case False{}:
      cx
    case True{}:
      timers_gone.each(o, TM.ids_in(live(o, cx), ns), cx, cat(["@", n, " is gone"]))

# an archived (dropped) agent's timers end with it, in the same step (law
# stop_leaves_no_timer); a stopped one's wait for it, as before
def timers_gone(-o: M.St, +life: M.Life, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  timers_gone.if(o, life_gone(life), cx, n, M.names_of(st(o, cx), n))

# ---- the timers at a tick ----

def wake_q.go(ok: Bool, +t: TM.Timer, +now: Nat) -> List<&2, J.Json>:
  match ok:
    case False{}:
      Nil{}
    case True{}:
      [J.obj(List.append(&2, Sigma<&2, &2, String, _ => J.Json>,
        [J.kv("id", J.JNum{TM.Timer.id(t)}), J.kv("fired", J.JNum{1n+TM.Timer.fired(t)}),
          J.kv("waited_ms", J.JNum{Nat.sub(now, TM.Timer.next(t))})],
        Bool.pick(List<&2, Sigma<&2, &2, String, _ => J.Json>>, TM.is_daily(TM.Timer.sched(t)), [TM.sched_kv(TM.Timer.sched(t))], Nil{})))]

# the wakes this tick may send (their texts and daily next times come
# from the hub: one {need} for them all, answered by id)
def wake_qs(-o: M.St, ts: List<&2, TM.Timer>, +cx: X.Cx<o>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case Con{+t, +rest}:
      +ok = Maybe.is_none(&2, String, TM.end_why(t, timer_gone(st(o, cx), TM.Timer.agent(t)), now(o, cx))) && timer_may(o, cx, t, True{})
      List.append(&2, J.Json, wake_q.go(ok, t, now(o, cx)), wake_qs(o, rest, cx))

def wake_ans(xs: List<&2, J.Json>, +id: Nat) -> Maybe<&2, J.Json>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, J.Json>, Nat.is_eq(C.get_nat(h, "id"), id), Some{h}, wake_ans(t, id))

def timers_tick.wake(-o: M.St, a: Maybe<&2, J.Json>, +cx: X.Cx<o>, +t: TM.Timer) -> X.Cx<o>:
  match a:
    case None{}:
      cx
    case Some{+w}:
      timer_fire(o, cx, t, C.get_s(w, "text"), True{},
        TM.next_of(TM.Timer.sched(t), TM.Timer.next(t), now(o, cx), C.get_nat(w, "next_ms")))

def timers_tick.end(-o: M.St, w: Maybe<&2, String>, +cx: X.Cx<o>, +t: TM.Timer, +ans: List<&2, J.Json>) -> X.Cx<o>:
  match w:
    case Some{+why}:
      emit(o, cx, M.ETimerStop{TM.Timer.id(t), why, now(o, cx)}, Unit{}, Unit{}, Unit{})
    case None{}:
      timers_tick.wake(o, wake_ans(ans, TM.Timer.id(t)), cx, t)

def timers_tick.each(-o: M.St, ts: List<&2, TM.Timer>, +cx: X.Cx<o>, +ans: List<&2, J.Json>) -> X.Cx<o>:
  match ts:
    case Nil{}:
      cx
    case Con{+t, +rest}:
      +cx2 = timers_tick.end(o, TM.end_why(t, timer_gone(st(o, cx), TM.Timer.agent(t)), now(o, cx)), cx, t, ans)
      timers_tick.each(o, rest, cx2, ans)

def json_arr(m: Maybe<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case Some{J.JArr{+vals}}:
      vals
    case _:
      Nil{}

def timers_tick.asked(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +ts: List<&2, TM.Timer>) -> X.Cx<o>:
  match p:
    case (+cx, None{}):
      # the step halts: the hub answers, then runs the tick again (same now)
      cx
    case (+cx, Some{+a}):
      timers_tick.each(o, ts, cx, json_arr(J.get(a, "wakes")))

def timers_tick.ask(-o: M.St, qs: List<&2, J.Json>, +cx: X.Cx<o>, +ts: List<&2, TM.Timer>) -> X.Cx<o>:
  match qs:
    case Nil{}:
      timers_tick.each(o, ts, cx, Nil{})
    case Con{+q, +rest}:
      timers_tick.asked(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"every_wake"}), J.kv("wakes", J.JArr{q <> rest})])), ts)

def timers_tick.go(-o: M.St, work: Bool, +cx: X.Cx<o>) -> X.Cx<o>:
  match work:
    case False{}:
      cx
    case True{}:
      timers_tick.ask(o, wake_qs(o, live(o, cx), cx), cx, live(o, cx))

# the timers due now wake their idle agents (a busy one: at its next
# idle), the ended ones stop. A tick with none due or past its end does
# nothing (idle-cpu)
def timers_tick(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  timers_tick.go(o, TM.any_work(live(o, cx), now(o, cx)), cx)

def reject_ids(-o: M.St, ids: List<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      reject_ids(o, rest, emit(o, cx, M.EMsgState{id, M.Rejected{"recipient_unavailable"}}, Unit{}, Unit{}, Unit{}))

def set_waiters(-o: M.St, +cx: X.Cx<o>, +ws: List<&2, X.Waiter>) -> X.Cx<o>:
  set_r(o, cx, X.R.with_waiters(rr(o, cx), ws))

def stop_task.go(-o: M.St, +cx: X.Cx<o>, +name: String, +life: M.Life, +reason: String) -> X.Cx<o>:
  +cx1 = out(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{name})])
  +cx2 = fail_waiters(o, cx1, name, "stopped")
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_steered(r, steer_del(X.R.steered(r), name)))
  +cx4 = upd_rt(o, cx3, X.Rt.with_turn(X.Rt.with_run(rt(o, cx3, name), M.RDown{}), None{}))
  +cx5 = emit(o, cx4, M.ELifecycle{name, life, Bool.pick(Maybe<&2, String>, String.is_empty(reason), None{}, Some{reason})}, Unit{}, Unit{}, Unit{})
  +cx6 = reject_ids(o, ids_of(M.queued_for(st(o, cx5), name)), cx5)
  close_cards(o, cx6, name, Nil{}, "agent stopped")

# no wait of the stopped task survives (law stop_leaves_no_waiter)
def stop_task(-o: M.St, +cx: X.Cx<o>, +name: String, +life: M.Life, +reason: String) -> X.Cx<o>:
  set_waiters(o, timers_gone(o, life, stop_task.go(o, cx, name, life, reason), name), waiters_not(X.R.waiters(rr(o, cx)), name))

# stop_task, after saying which of the user's queued messages will never
# arrive (BISE-86)
def stop_noted(-o: M.St, +cx: X.Cx<o>, +name: String, +life: M.Life, +reason: String) -> X.Cx<o>:
  stop_task(o, undelivered_msgs(o, M.queued_for(st(o, cx), name), cx), name, life, reason)

def plural(+n: Nat) -> String:
  Bool.pick(String, Nat.is_gt(n, 1n), "s", "")

def loss_text(+dirty: Nat, +unpushed: Nat) -> String:
  cat([ns(dirty), " changed file", plural(dirty), " and ", ns(unpushed),
    " unpushed commit", plural(unpushed), " will be saved (/restore)"])

def drop.label(r: Result<&2, &2, String, J.Json>) -> String:
  match r:
    case Fail{+e}:
      String.append(" — worktree NOT deleted: ", e)
    case Done{J.JStr{_}}:
      " — work saved (/restore)"
    case Done{_}:
      " · its worktree removed"

def drop.snap(j: J.Json) -> Maybe<&2, String>:
  match j:
    case J.JStr{+s}:
      Some{s}
    case _:
      None{}

# ---- places (dev-flow §3.1): a worktree shared by agents ----

def is_gone(l: M.Life) -> Bool:
  match l:
    case M.LArchived{}:
      True{}
    case _:
      False{}

# `b` is another agent of `place`'s live folder: still in it (`live`), or
# archived with its ws still on that folder (it left before the others)
def mate.hit(m: Maybe<&2, M.Agent>, +name: String, +place: String, +live: Bool) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+b}:
      +gone = is_gone(M.Agent.life(b))
      Bool.not(String.eq(M.Agent.name(b), name)) && M.ws_worktree(M.Agent.ws(b)) && Bool.not(M.ws_dropped(M.Agent.ws(b)))
        && String.eq(M.Agent.place(b), place) && Bool.pick(Bool, live, Bool.not(gone), gone)

def mates(xs: List<&2, String>, +s: M.St, +name: String, +place: String, +live: Bool) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = mates(t, s, name, place, live)
      Bool.pick(List<&2, String>, mate.hit(M.get_agent(s, n), name, place, live), n <> rest, rest)

# the other agents of a's worktree (none for the shared folder)
def mates_of(+s: M.St, +a: M.Agent, +live: Bool) -> List<&2, String>:
  mates(M.St.order(s), s, M.Agent.name(a), M.Agent.place(a), live)

def first_agent(+s: M.St, ns: List<&2, String>) -> Maybe<&2, M.Agent>:
  match ns:
    case Nil{}:
      None{}
    case Con{+n, _}:
      M.get_agent(s, n)

def drop.stays(ms: List<&2, String>) -> String:
  match ms:
    case Nil{}:
      ""
    case Con{+h, +t}:
      cat([" — its worktree stays with @", String.join({h <> t : List<&2, String>}, ", @")])

def drop.left.ws(m: Maybe<&2, M.Agent>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+b}:
      J.set(M.Agent.ws(b), "dropped", J.JBool{True{}})

# the place's folder is gone: so is it for the agents that left it
# before (archived), with the same saved work (RFC 0002 §5, per place)
def drop.left(-o: M.St, ns: List<&2, String>, +cx: X.Cx<o>, +snap: Maybe<&2, String>) -> X.Cx<o>:
  match ns:
    case Nil{}:
      cx
    case Con{+n, +t}:
      +cx2 = emit(o, cx, M.EWorkspace{n, drop.left.ws(M.get_agent(st(o, cx), n))}, Unit{}, Unit{}, Unit{})
      drop.left(o, t, emit(o, cx2, M.ESnapshot{n, snap}, Unit{}, Unit{}, Unit{}), snap)

def drop.ws(-o: M.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o>:
  match r:
    case Fail{_}:
      cx
    case Done{+snap}:
      +left = mates_of(st(o, cx), a, False{})
      +sn = drop.snap(snap)
      +cx2 = emit(o, cx, M.EWorkspace{M.Agent.name(a), J.set(M.Agent.ws(a), "dropped", J.JBool{True{}})}, Unit{}, Unit{}, Unit{})
      drop.left(o, left, emit(o, cx2, M.ESnapshot{M.Agent.name(a), sn}, Unit{}, Unit{}, Unit{}), sn)

def drop.finish(-o: M.St, +cx: X.Cx<o>, +a: M.Agent, +label: String) -> X.Cx<o> & DropR:
  +cx2 = line(o, cx, M.MAIN(), "info", cat(["@", M.Agent.name(a), " archived", label]))
  # a client looking at it goes back to main
  (out(o, cx2, [J.kv("fx", J.JStr{"focus_main"}), J.kv("name", J.JStr{M.Agent.name(a)})]), DDone{})

def drop.git(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +a: M.Agent) -> X.Cx<o> & DropR:
  match p:
    case (+cx, None{}):
      (cx, DHalt{})
    case (+cx, Some{+ans}):
      drop.finish(o, drop.ws(o, ans_ok(ans), cx, a), a, drop.label(ans_ok(ans)))

def drop.now(-o: M.St, has_wt: Bool, +cx: X.Cx<o>, +a: M.Agent, +loss: J.Json) -> X.Cx<o> & DropR:
  match has_wt:
    case False{}:
      # a worktree others still work in stays (it goes with its last agent)
      +label = drop.stays(mates_of(st(o, cx), a, True{}))
      drop.finish(o, stop_noted(o, cx, M.Agent.name(a), M.LArchived{}, "drop"), a, label)
    case True{}:
      +cx2 = stop_noted(o, cx, M.Agent.name(a), M.LArchived{}, "drop")
      drop.git(o, ask(o, cx2, J.obj([J.kv("q", J.JStr{"worktree_drop"}), J.kv("name", J.JStr{M.Agent.name(a)}), J.kv("ws", M.Agent.ws(a)),
        J.kv("dirty", J.JNum{J.get_num_or(loss, "dirty", 0n)}), J.kv("unpushed", J.JNum{J.get_num_or(loss, "unpushed", 0n)})])), a)

def drop.decide(-o: M.St, confirm: Bool, +cx: X.Cx<o>, +a: M.Agent, +has_wt: Bool, +loss: J.Json, +busy: Bool) -> X.Cx<o> & DropR:
  match confirm:
    case True{}:
      +dirty = J.get_num_or(loss, "dirty", 0n)
      +unpushed = J.get_num_or(loss, "unpushed", 0n)
      +parts = List.append(&2, String, Bool.pick(List<&2, String>, busy, ["the agent is mid-turn"], Nil{}),
        Bool.pick(List<&2, String>, Nat.is_gt(Nat.add(dirty, unpushed), 0n), [loss_text(dirty, unpushed)], Nil{}))
      (cx, DAsk{cat(["archive @", M.Agent.name(a), "? ", String.join(parts, " ; "), ". [y/N]"])})
    case False{}:
      drop.now(o, has_wt, cx, a, loss)

def drop.loss(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +a: M.Agent, +force: Bool, +has_wt: Bool) -> X.Cx<o> & DropR:
  match p:
    case (+cx, None{}):
      (cx, DHalt{})
    case (+cx, Some{+loss}):
      +busy = is_busy(X.Rt.run(rt(o, cx, M.Agent.name(a))))
      +any = Nat.is_gt(Nat.add(J.get_num_or(loss, "dirty", 0n), J.get_num_or(loss, "unpushed", 0n)), 0n)
      drop.decide(o, Bool.not(force) && (busy || any), cx, a, has_wt, loss, busy)

def drop.wt(-o: M.St, has_wt: Bool, +cx: X.Cx<o>, +a: M.Agent, +force: Bool) -> X.Cx<o> & DropR:
  match has_wt:
    case True{}:
      drop.loss(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_loss"}), J.kv("name", J.JStr{M.Agent.name(a)}), J.kv("ws", M.Agent.ws(a))])), a, force, True{})
    case False{}:
      drop.loss(o, (cx, Some{J.obj([J.kv("dirty", J.JNum{0n}), J.kv("unpushed", J.JNum{0n})])}), a, force, False{})

def drop.checks(-o: M.St, is_main: Bool, life: M.Life, +cx: X.Cx<o>, +a: M.Agent, +force: Bool) -> X.Cx<o> & DropR:
  match is_main:
    case True{}:
      (cx, DErr{"main cannot be dropped"})
    case False{}:
      match life:
        case M.LArchived{}:
          (cx, DErr{cat(["@", M.Agent.name(a), " is already archived"])})
        case _:
          # the folder goes with the place's last agent only
          +last = List.is_empty(&2, String, mates_of(st(o, cx), a, True{}))
          drop.wt(o, M.ws_worktree(M.Agent.ws(a)) && Bool.not(M.ws_dropped(M.Agent.ws(a))) && last, cx, a, force)

def drop.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String, +force: Bool) -> X.Cx<o> & DropR:
  match m:
    case None{}:
      (cx, DErr{String.append("no agent named ", name)})
    case Some{+a}:
      drop.checks(o, M.Agent.is_main(a), M.Agent.life(a), cx, a, force)

def agent_of(+s: M.St, m: Maybe<&2, String>) -> Maybe<&2, M.Agent>:
  match m:
    case None{}:
      None{}
    case Some{+n}:
      M.get_agent(s, n)

# RFC 0001 §9.4 + RFC 0002 §5
def drop_task(-o: M.St, +cx: X.Cx<o>, +name: String, +force: Bool) -> X.Cx<o> & DropR:
  drop.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name, force)

def restore.go(-o: M.St, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o> & TaskR:
  +n = M.Agent.name(a)
  +cx2 = emit(o, cx, M.ELifecycle{n, M.LActive{}, None{}}, Unit{}, Unit{}, Unit{})
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_crashes(r, kn_del(X.R.crashes(r), n)))
  +cx4 = spawn(o, cx3, n, True{})
  # an idle task back from failed gets the mail it kept (law L4)
  (line(o, pump(o, cx4, n), M.MAIN(), "info", cat(["@", n, " restored"])), TOk{n})

def restore.res(-o: M.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o> & TaskR:
  match r:
    case Fail{+e}:
      (cx, TErr{e})
    case Done{+ws}:
      +cx2 = emit(o, cx, M.EWorkspace{M.Agent.name(a), ws}, Unit{}, Unit{}, Unit{})
      restore.go(o, emit(o, cx2, M.ESnapshot{M.Agent.name(a), None{}}, Unit{}, Unit{}, Unit{}), a)

def restore.ans(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +a: M.Agent) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, None{}):
      (cx, THalt{})
    case (+cx, Some{+ans}):
      restore.res(o, ans_ok(ans), cx, a)

def restore.wt(-o: M.St, dropped: Bool, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o> & TaskR:
  match dropped:
    case True{}:
      restore.ans(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_restore"}), J.kv("name", J.JStr{M.Agent.name(a)}), J.kv("ws", M.Agent.ws(a)),
        J.kv("snapshot_ref", C.jmaybe_str(M.Agent.snap(a)))])), a)
    case False{}:
      restore.go(o, cx, a)

# its place is live again (a mate restored it): it rejoins the folder
def restore.join(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      restore.wt(o, True{}, cx, a)
    case Some{+b}:
      +n = M.Agent.name(a)
      +cx2 = emit(o, cx, M.EWorkspace{n, M.Agent.ws(b)}, Unit{}, Unit{}, Unit{})
      restore.go(o, emit(o, cx2, M.ESnapshot{n, None{}}, Unit{}, Unit{}, Unit{}), a)

def restore.dropped(-o: M.St, dropped: Bool, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o> & TaskR:
  match dropped:
    case True{}:
      restore.join(o, first_agent(st(o, cx), mates_of(st(o, cx), a, True{})), cx, a)
    case False{}:
      restore.go(o, cx, a)

def restore.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      (cx, TErr{String.append("no agent named ", name)})
    case Some{+a}:
      pick_lazy(X.Cx<o> & TaskR, is_active(M.Agent.life(a)), u => (cx, TErr{cat(["@", M.Agent.name(a), " is active"])}),
        u => restore.dropped(o, M.ws_worktree(M.Agent.ws(a)) && M.ws_dropped(M.Agent.ws(a)), cx, a))

def restore_task(-o: M.St, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  restore.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name)

def isolate.sent(-o: M.St, p: X.Cx<o> & SendR, +n: String, +branch: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, SErr{+e}):
      (cx, TErr{e})
    case (+cx, SOk{_}):
      (line(o, cx, M.MAIN(), "info", cat(["@", n, " now works in the worktree ", branch])), TOk{n})

def isolate.res(-o: M.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +n: String) -> X.Cx<o> & TaskR:
  match r:
    case Fail{+e}:
      (cx, TErr{e})
    case Done{+ws}:
      +path = M.ws_path(ws)
      +branch = J.get_str_or(ws, "branch", "")
      +cx2 = emit(o, cx, M.EWorkspace{n, ws}, Unit{}, Unit{}, Unit{})
      # the REPL takes its new BEND_WORKDIR at the next spawn
      +cx3 = out(o, cx2, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})])
      +cx4 = spawn(o, upd_rt(o, cx3, X.Rt.with_run(rt(o, cx3, n), M.RDown{})), n, True{})
      +text = cat(["[bise] Your working directory is now the isolated git worktree `", path, "` (branch `", branch, "`). Work only there from now on."])
      isolate.sent(o, send(o, cx4, msg(M.USER(), n, text, False{}, None{}, True{}, False{})), n, branch)

def isolate.ans(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +n: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, None{}):
      (cx, THalt{})
    case (+cx, Some{+ans}):
      isolate.res(o, ans_ok(ans), cx, n)

def isolate.pre(-o: M.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +n: String) -> X.Cx<o> & TaskR:
  match pre:
    case Some{+e}:
      (cx, TErr{e})
    case None{}:
      isolate.ans(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_create"}), J.kv("name", J.JStr{n}), J.kv("with_changes", J.JBool{False{}})])), n)

def isolate.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      (cx, TErr{String.append("no agent named ", name)})
    case Some{+a}:
      +n = M.Agent.name(a)
      +pre = Bool.pick(Maybe<&2, String>, M.Agent.is_main(a) || M.ws_worktree(M.Agent.ws(a)), Some{cat(["@", n, " already has its own folder"])},
        Bool.pick(Maybe<&2, String>, Bool.not(List.is_empty(&2, String, M.Agent.files(a))),
          Some{cat(["@", n, " already changed files of the workspace (", String.join(List.take(&2, String, M.Agent.files(a), 3n), ", "), "): its work cannot be separated from yours"])},
          Bool.pick(Maybe<&2, String>, is_busy(X.Rt.run(rt(o, cx, n))), Some{cat(["@", n, " is mid-turn: wait for the end of its turn"])},
            Bool.pick(Maybe<&2, String>, X.R.git(rr(o, cx)), None{}, Some{"the workspace is not a git repository"}))))
      isolate.pre(o, pre, cx, n)

def isolate_task(-o: M.St, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  isolate.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name)

# ---- `sb move <agent> <place>` (dev-flow §3.1, /isolate generalized) ----

def move.sent(-o: M.St, p: X.Cx<o> & SendR, +n: String, +dest: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, SErr{+e}):
      (cx, TErr{e})
    case (+cx, SOk{_}):
      (line(o, cx, M.MAIN(), "info", cat(["@", n, " moved to ", dest])), TOk{n})

def move.res(-o: M.St, +cx: X.Cx<o>, +n: String, +ws: J.Json) -> X.Cx<o> & TaskR:
  +path = M.ws_path(ws)
  +branch = J.get_str_or(ws, "branch", "")
  +wt = M.ws_worktree(ws)
  +cx2 = emit(o, cx, M.EWorkspace{n, ws}, Unit{}, Unit{}, Unit{})
  # the REPL takes its new BEND_WORKDIR at the next spawn
  +cx3 = out(o, cx2, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})])
  +cx4 = spawn(o, upd_rt(o, cx3, X.Rt.with_run(rt(o, cx3, n), M.RDown{})), n, True{})
  +text = Bool.pick(String, wt,
    cat(["[bise] Your working directory is now the git worktree `", path, "` (branch `", branch, "`), shared with other agents: change and commit only your own files there (`sb land --here`). Work only there from now on."]),
    cat(["[bise] Your working directory is now the shared folder `", path, "`. Work only there from now on; do not revert changes you did not make."]))
  +dest = Bool.pick(String, wt, cat(["the worktree ", branch]), "the shared folder")
  move.sent(o, send(o, cx4, msg(M.USER(), n, text, False{}, None{}, True{}, False{})), n, dest)

def move.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String, +ws: J.Json) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      (cx, TErr{String.append("no agent named ", name)})
    case Some{+a}:
      +n = M.Agent.name(a)
      +pre = Bool.pick(Maybe<&2, String>, M.Agent.is_main(a), Some{"main works in the shared folder"},
        Bool.pick(Maybe<&2, String>, String.eq(M.Agent.place(a), M.ws_place(ws, M.Agent.dir(a))), Some{cat(["@", n, " is already there"])},
          Bool.pick(Maybe<&2, String>, Bool.not(List.is_empty(&2, String, M.Agent.files(a))),
            Some{cat(["@", n, " already changed files (", String.join(List.take(&2, String, M.Agent.files(a), 3n), ", "), "): only an agent that changed nothing yet moves"])},
            Bool.pick(Maybe<&2, String>, is_busy(X.Rt.run(rt(o, cx, n))), Some{cat(["@", n, " is mid-turn: wait for the end of its turn"])}, None{}))))
      pick_lazy(X.Cx<o> & TaskR, Maybe.is_some(&2, String, pre), u => (cx, TErr{Maybe.default(&2, String, pre, "")}), u => move.res(o, cx, n, ws))

def move.to(-o: M.St, j: Maybe<&2, J.Json>, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  match j:
    case Some{J.JObj{+kvs}}:
      move.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name, J.JObj{kvs})
    case _:
      isolate_task(o, cx, name)

# `ws`: the target place's (the daemon finds it; none: a new worktree,
# /isolate); `pre_err`: the daemon refused (no such place...)
def move_task(-o: M.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +name: String, +j: Maybe<&2, J.Json>) -> X.Cx<o> & TaskR:
  match pre:
    case Some{+e}:
      (cx, TErr{e})
    case None{}:
      move.to(o, j, cx, name)

# ---- the user ----

def user_sent(-o: M.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"user_sent"}), J.kv("text", J.JStr{text})])

def close_card(-o: M.St, +cx: X.Cx<o>, +id: Nat, +res: String) -> X.Cx<o>:
  +cx2 = emit(o, cx, M.ECardClosed{id, res}, Unit{}, Unit{}, Unit{})
  line(o, cx2, M.MAIN(), "card-closed", cat(["#", ns(id), " ", res]))

def answer.q.sent(-o: M.St, p: X.Cx<o> & SendR, +c: M.Card, +asker: String, +text: String) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      notice(o, cx, e)
    case (+cx, SOk{_}):
      +id = M.Card.id(c)
      +cx2 = note_main(o, cx, cat(["the user answered card #", ns(id), " (@", asker, " asked: \"", M.one_line(M.Card.text(c)), "\"): \"", M.one_line(text), "\""]))
      +cx3 = line(o, cx2, M.MAIN(), "route", cat(["you → @", asker, " (answer to card #", ns(id), ") : ", text]))
      close_card(o, cx3, id, "answered")

# main's own card: the answer wakes main with a turn that holds it, like
# a card for an agent reaches that agent (a main note would wait for the
# user's next message)

# the head of main's note when the user answers one of main's own cards
# (the clients match it to drop that note: lines.rs MAIN_ANSWER_HEAD)
def MAIN_ANSWER_HEAD() -> String:
  "the user answered card #"

#
# The same route line as an agent's card (answer.q.sent), his answer whole:
# the clients fold that line into the "you answered" row and drop the
# notify's twin (lines.rs is_main_answer_note, its head MAIN_ANSWER_HEAD)
def answer.q.main(-o: M.St, +cx: X.Cx<o>, +c: M.Card, +text: String) -> X.Cx<o>:
  +id = M.Card.id(c)
  +cx2 = notify_main(o, cx, cat([MAIN_ANSWER_HEAD(), ns(id), " (\"", M.one_line(M.Card.text(c)), "\"): \"", M.one_line(text), "\""]))
  +cx3 = line(o, cx2, M.MAIN(), "route", cat(["you → @", M.MAIN(), " (answer to card #", ns(id), ") : ", text]))
  close_card(o, cx3, id, "answered")

def answer.q(-o: M.St, m: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +c: M.Card, +text: String) -> X.Cx<o>:
  match m:
    case Some{+q}:
      answer.q.sent(o, send(o, cx, msg(M.USER(), M.Msg.from(q), text, False{}, Some{M.Msg.id(q)}, False{}, False{})), c, M.Msg.from(q), text)
    case None{}:
      # pr-news: a card the hub opened about an agent (checks that still
      # fail after two tries): the answer goes to that agent, main reads it
      +ag = M.Card.agent(c)
      pick_lazy(X.Cx<o>, Bool.not(String.eq(ag, M.MAIN())) && Maybe.is_some(&2, String, M.resolve(st(o, cx), ag)),
        u => answer.q.sent(o, send(o, cx, msg(M.USER(), ag, text, False{}, None{}, False{}, False{})), c, ag, text),
        u => answer.q.main(o, cx, c, text))

def answer.dropped(-o: M.St, p: X.Cx<o> & DropR, +c: M.Card) -> X.Cx<o>:
  match p:
    case (+cx, DErr{+e}):
      close_card(o, notice(o, cx, e), M.Card.id(c), "accepted")
    case (+cx, _):
      close_card(o, note_main(o, cx, String.append("the user accepted to drop @", M.Card.agent(c))), M.Card.id(c), "accepted")

def answer.drop(-o: M.St, yes: Bool, +cx: X.Cx<o>, +c: M.Card) -> X.Cx<o>:
  match yes:
    case True{}:
      answer.dropped(o, drop_task(o, cx, M.Card.agent(c), True{}), c)
    case False{}:
      close_card(o, note_main(o, cx, String.append("the user refused to drop @", M.Card.agent(c))), M.Card.id(c), "refused")

def answer.plain.sent(-o: M.St, p: X.Cx<o> & SendR, +id: Nat) -> X.Cx<o>:
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
        Bool.pick(CardK, M.choice_kind(k), KChoice{}, KOther{}))))

def digits(cs: List<&2, Char>) -> Bool:
  match cs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Char.is_digit(h) && digits(t)

# an option's number (`1`), not words
def is_number(+t: String) -> Bool:
  +s = String.trim(t)
  Bool.pick(Bool, String.is_empty(s), False{}, digits(String.to_list(s)))

def opt_line(+l: String, +n: String) -> Bool:
  String.starts_with(l, String.append(n, ".")) || String.starts_with(l, String.append(n, ")"))

def any_opt(xs: List<&2, String>, +n: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      opt_line(String.trim(h), n) || any_opt(t, n)

# one of the card's options: a number with its line (`2.` or `2)`) in the
# card's text
def is_option(+card: String, +t: String) -> Bool:
  is_number(t) && any_opt(String.lines(card), String.trim(t))

# a deny alias (lead m_10900): `no` or `no: <why>`, any case, on the
# lowered trimmed answer; it can only refuse (`nope`, `now` are words)
def is_no(+l: String) -> Bool:
  String.eq(l, "no") || String.starts_with(l, "no:")

# a deny alias is an answer; else the options and the TUI's `deny:`
def confirm_pick(no: Bool, +s: String) -> Bool:
  match no:
    case True{}:
      True{}
    case False{}:
      String.eq(s, "1") || String.eq(s, "2") || String.eq(s, "3") || String.starts_with(String.to_lower(s), "deny:")

# an approval card's answer (the composer rule, lead m_8777): its options
# 1 allow, 2 always, 3 no, the TUI's explicit deny action (`deny: <why>`,
# a refusal with its note) or its aliases `no` and `no: <why>`; typed
# words never answer it
def confirm_ok(+t: String) -> Bool:
  +s = String.trim(t)
  confirm_pick(is_no(String.to_lower(s)), s)

# what a confirm card waits for, said when words are refused
def confirm_opts() -> String:
  "1 allow · 2 always · 3 no · or no: <why>"

# typed words on an approval card: the card stays open, nothing wakes
def words_refused(-o: M.St, +cx: X.Cx<o>, +c: M.Card, +opts: String) -> X.Cx<o>:
  notice(o, cx, cat(["card #", ns(M.Card.id(c)), " waits for ", opts, " — typed words don't answer it"]))

def answer.confirm(-o: M.St, ok: Bool, +cx: X.Cx<o>, +c: M.Card, +text: String) -> X.Cx<o>:
  match ok:
    case True{}:
      # the approvals gate (approvals-design.md §9): the answer is the
      # verdict of the waiting call, written to its gate by the hub's
      # Rust side (the `confirm` effect), never a message to the agent
      +cx2 = out(o, cx, [J.kv("fx", J.JStr{"confirm"}), J.kv("card", J.JNum{M.Card.id(c)}),
        J.kv("agent", J.JStr{M.Card.agent(c)}), J.kv("text", J.JStr{text})])
      close_card(o, cx2, M.Card.id(c), "answered")
    case False{}:
      words_refused(o, cx, c, confirm_opts())

# a hub item's answer (M.choice_kind): one of its numbers goes to the
# hub's Rust side (the `card_choice` effect), which acts and closes the
# item (a merge runs `gh pr merge`: the item stays open meanwhile); typed
# words don't answer it (the composer rule: the window sends them to the
# agent as a message)
def answer.choice(-o: M.St, num: Bool, +cx: X.Cx<o>, +c: M.Card, +text: String) -> X.Cx<o>:
  match num:
    case True{}:
      out(o, cx, [J.kv("fx", J.JStr{"card_choice"}), J.kv("card", J.JNum{M.Card.id(c)}),
        J.kv("kind", J.JStr{M.Card.kind(c)}), J.kv("agent", J.JStr{M.Card.agent(c)}),
        J.kv("place", C.jmaybe_str(M.Card.place(c))), J.kv("pr", C.jmaybe_nat(M.Card.pr(c))),
        J.kv("text", J.JStr{String.trim(text)})])
    case False{}:
      words_refused(o, cx, c, "one of its numbers")

def answer.kind(-o: M.St, k: CardK, +cx: X.Cx<o>, +c: M.Card, +text: String) -> X.Cx<o>:
  match k:
    case KQuestion{}:
      answer.q(o, for_msg_of(st(o, cx), M.Card.for_msg(c)), cx, c, text)
    case KDrop{}:
      answer.drop(o, is_yes(text), cx, c)
    case KConfirm{}:
      answer.confirm(o, confirm_ok(text), cx, c, text)
    case KChoice{}:
      answer.choice(o, is_option(M.Card.text(c), text), cx, c, text)
    case KOther{}:
      close_card(o, cx, M.Card.id(c), "vue")

def answer.card(-o: M.St, m: Maybe<&2, M.Card>, +cx: X.Cx<o>, +id: Nat, +text: String) -> X.Cx<o>:
  match m:
    case None{}:
      notice(o, cx, String.append("no open card #", ns(id)))
    case Some{+c}:
      answer.kind(o, card_kind(M.Card.kind(c)), cx, c, text)

def answer_card(-o: M.St, +cx: X.Cx<o>, +id: Nat, +text: String) -> X.Cx<o>:
  answer.card(o, M.card_of(M.St.cards(st(o, cx)), id), cx, id, text)

def asked_by(m: Maybe<&2, M.Msg>, +to: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+q}:
      String.eq(M.Msg.from(q), to)

# RFC 0001 §7.3: answering a task that waits on the user, from its own
# view, answers its question
def question_card(cs: List<&2, M.Card>, +s: M.St, +to: String) -> Maybe<&2, Nat>:
  match cs:
    case Nil{}:
      None{}
    case Con{+c, +t}:
      Bool.pick(Maybe<&2, Nat>, String.eq(M.Card.kind(c), "question") && asked_by(for_msg_of(s, M.Card.for_msg(c)), to),
        Some{M.Card.id(c)}, question_card(t, s, to))

def says.sent(-o: M.St, p: X.Cx<o> & SendR, +to: String, +text: String) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      undelivered.if(o, unavailable_err(e), notice(o, cx, e), to, to, text)
    case (+cx, SOk{_}):
      pick_lazy(X.Cx<o>, String.eq(to, M.MAIN()), u => cx, u => user_sent(o, cx, text))

def says.card(-o: M.St, card: Maybe<&2, Nat>, +cx: X.Cx<o>, +to: String, +text: String, +queued: Bool) -> X.Cx<o>:
  match card:
    case Some{+id}:
      answer_card(o, user_sent(o, cx, text), id, text)
    case None{}:
      says.sent(o, send(o, cx, Snd{M.USER(), to, text, False{}, None{}, True{}, False{}, queued, None{}}), to, text)

# plain text from the user to the agent in focus. `queued` (the window's
# "send queued"): a queued-mode message, held while the agent is busy and
# said at its turn's end (law user_queued_waits_for_turn_end); it never
# answers a question card (law user_queued_never_answers_a_card)
def user_says(-o: M.St, +cx: X.Cx<o>, +to: String, +text: String, +queued: Bool) -> X.Cx<o>:
  says.card(o, Bool.pick(Maybe<&2, Nat>, queued || String.eq(to, M.MAIN()), None{}, question_card(M.St.cards(st(o, cx)), st(o, cx), to)), cx, to, text, queued)

def close_names(xs: List<&2, String>, +target: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = close_names(t, target)
      +first = String.take(target, 1n)
      Bool.pick(List<&2, String>, Bool.not(String.eq(n, M.MAIN())) && (String.starts_with(n, Bool.pick(String, String.is_empty(first), "?", first)) || String.contains(n, target)),
        String.append("@", n) <> rest, rest)

def route.sent(-o: M.St, p: X.Cx<o> & SendR, +name: String, +text: String, +focus: String) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      undelivered.if(o, unavailable_err(e), notice(o, cx, e), focus, name, text)
    case (+cx, SOk{+m}):
      +cx2 = line(o, cx, M.MAIN(), "route", cat(["you → @", name, " : ", text]))
      +cx3 = pick_lazy(X.Cx<o>, Bool.not(String.eq(focus, M.MAIN())) || Bool.not(String.eq(name, focus)),
        u => note_main(o, cx2, cat(["the user wrote directly to @", name, ": \"", M.one_line(text), "\""])), u => cx2)
      cx3

def route.target(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +target: String, +text: String, +focus: String, +queued: Bool) -> X.Cx<o>:
  match m:
    case None{}:
      +close = close_names(M.St.order(st(o, cx)), target)
      +hint = Bool.pick(String, List.is_empty(&2, String, close), "", cat([" — did you mean ", String.join(close, ", "), "?"]))
      notice(o, cx, cat(["no agent named @", target, hint]))
    case Some{+name}:
      # RFC 0003 §5.1: from another view, the task reads it tagged and
      # its answer comes back to that view
      +elsewhere = Bool.not(String.eq(name, focus))
      route.sent(o, send(o, cx, Snd{M.USER(), name, text, elsewhere, None{}, Bool.not(elsewhere), False{}, queued,
        Bool.pick(Maybe<&2, String>, elsewhere, Some{focus}, None{})}), name, text, focus)

# `@task text`: a direct message, without main; `queued` as user_says
def route(-o: M.St, +cx: X.Cx<o>, +target: String, +text: String, +focus: String, +queued: Bool) -> X.Cx<o>:
  pick_lazy(X.Cx<o>, String.eq(target, M.MAIN()), u => user_says(o, cx, M.MAIN(), text, queued),
    u => route.target(o, M.resolve(st(o, cx), target), cx, target, text, focus, queued))

def user_new.res(-o: M.St, p: X.Cx<o> & TaskR) -> X.Cx<o>:
  match p:
    case (+cx, TErr{+e}):
      notice(o, cx, e)
    case (+cx, _):
      cx

def user_drop.res(-o: M.St, p: X.Cx<o> & DropR, +name: String) -> X.Cx<o>:
  match p:
    case (+cx, DDone{}):
      note_main(o, cx, String.append("the user dropped the agent @", name))
    case (+cx, DAsk{+q}):
      +canon = Maybe.default(&2, String, M.resolve(st(o, cx), name), name)
      out(o, cx, [J.kv("fx", J.JStr{"confirm"}), J.kv("name", J.JStr{canon}), J.kv("text", J.JStr{q})])
    case (+cx, DErr{+e}):
      notice(o, cx, e)
    case (+cx, DHalt{}):
      cx

def user_confirm.res(-o: M.St, p: X.Cx<o> & DropR, +name: String) -> X.Cx<o>:
  match p:
    case (+cx, DErr{+e}):
      notice(o, cx, e)
    case (+cx, _):
      note_main(o, cx, String.append("the user dropped the agent @", name))

def user_restore.res(-o: M.St, p: X.Cx<o> & TaskR) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      note_main(o, cx, String.append("the user restored the agent @", n))
    case (+cx, TErr{+e}):
      notice(o, cx, e)
    case (+cx, THalt{}):
      cx

def user_isolate.res(-o: M.St, p: X.Cx<o> & TaskR) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      note_main(o, cx, cat(["the user moved @", n, " into its own git worktree"]))
    case (+cx, TErr{+e}):
      notice(o, cx, e)
    case (+cx, THalt{}):
      cx

def rename.apply.new(-o: M.St, +cx: X.Cx<o>, +old: String, +new: String, +e: {M.name_taken(X.d(o, cx), new) == False{} : Bool}) -> X.Cx<o>:
  +cx2 = emit(o, cx, M.ERenamed{old, new}, e, Unit{}, Unit{})
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_rts(X.R.with_waiters(X.R.with_steered(r, steer_rename(X.R.steered(r), old, new)),
    waiters_rename(X.R.waiters(r), old, new)), rt_rename(X.R.rts(r), old, new)))
  +cx4 = out(o, cx3, [J.kv("fx", J.JStr{"renamed"}), J.kv("old", J.JStr{old}), J.kv("new", J.JStr{new})])
  line(o, cx4, M.MAIN(), "info", cat(["@", old, " s'appelle maintenant @", new]))

def rename.apply.free(-o: M.St, taken: Bool, +cx: X.Cx<o>, +old: String, +new: String, +e: {M.name_taken(X.d(o, cx), new) == taken : Bool}) -> X.Cx<o>:
  match taken:
    case True{}:
      notice(o, cx, String.append("invalid or taken name: ", new))
    case False{}:
      rename.apply.new(o, cx, old, new, e)

# the rename itself (the user's /rename and main's sb rename)
# the new name is free (the callers checked it on this state)
def rename.apply(-o: M.St, +cx: X.Cx<o>, +old: String, +new: String) -> X.Cx<o>:
  rename.apply.free(o, M.name_taken(st(o, cx), new), cx, old, new, {==})

def rename.go(-o: M.St, ok: Bool, +cx: X.Cx<o>, +old: String, +new: String) -> X.Cx<o>:
  match ok:
    case False{}:
      notice(o, cx, String.append("invalid or taken name: ", new))
    case True{}:
      note_main(o, rename.apply(o, cx, old, new), cat(["the user renamed @", old, " to @", new, " (the old name still works)"]))

def rename.old(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +name: String, +new: String, +valid: Bool) -> X.Cx<o>:
  match m:
    case None{}:
      notice(o, cx, String.append("no agent named @", name))
    case Some{+old}:
      pick_lazy(X.Cx<o>, String.eq(old, M.MAIN()), u => notice(o, cx, String.append("no agent named @", name)),
        u => rename.go(o, valid && Bool.not(M.name_taken(st(o, cx), new)), cx, old, new))

# the typed fact of a stop (desktop reset, architect m_9650 D): in the same
# step as the interrupt, a 'stopped' line in that agent's feed; the thread's
# fold turns it into an entry of kind 'stopped' at its position
def stopped(-o: M.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  line(o, cx, agent, "stopped", "stopped")

# the interrupt's effect for that agent (the shell flags its REPL)
def interrupt.fx(-o: M.St, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"interrupt"}), J.kv("agent", J.JStr{n})])

def interrupt.go(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case Some{+n}:
      pick_lazy(X.Cx<o>, is_busy(X.Rt.run(rt(o, cx, n))),
        u => stopped(o, interrupt.fx(o, cx, n), n),
        u => notice(o, cx, "no turn in progress to interrupt"))
    case None{}:
      notice(o, cx, "no turn in progress to interrupt")

def passthrough(-o: M.St, +cx: X.Cx<o>, +focus: String, +l: String) -> X.Cx<o>:
  pick_lazy(X.Cx<o>, has_agent(o, cx, focus) && is_idle(X.Rt.run(rt(o, cx, focus))),
    u => out(o, dirty(o, set_run(o, cx, focus, M.RBusy{})), [J.kv("fx", J.JStr{"passthrough"}), J.kv("agent", J.JStr{focus}), J.kv("line", J.JStr{l})]),
    u => notice(o, cx, "the agent is not idle: retry at the end of its turn"))

# the daemon computed what main learns about a direct exchange (RFC
# 0001 §7.4)
def focus(-o: M.St, +cx: X.Cx<o>, +f: String, note: Maybe<&2, String>, +direct: String) -> X.Cx<o>:
  match note:
    case Some{+n}:
      line(o, note_main(o, cx, n), M.MAIN(), "direct", direct)
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

def xst(-o: M.St, +cx: X.Cx<o>) -> XH.Xs:
  M.St.x(st(o, cx))

def xemit(-o: M.St, +cx: X.Cx<o>, +e: XH.XEv) -> X.Cx<o>:
  emit(o, cx, M.EX{e}, Unit{}, Unit{}, Unit{})

def aemit(-o: M.St, +cx: X.Cx<o>, +e: TM.ActEv) -> X.Cx<o>:
  emit(o, cx, M.EAct{e}, Unit{}, Unit{}, Unit{})

def xdeliver(-o: M.St, +cx: X.Cx<o>, +ob: XH.Out) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"xdeliver"}), J.kv("xid", J.JNum{XH.Out.xid(ob)}),
    J.kv("project", J.JStr{XH.Out.project(ob)}), J.kv("kind", J.JStr{XH.kind_str(XH.Out.kind(ob))}),
    J.kv("msg", J.JNum{XH.Out.msg(ob)}), J.kv("text", J.JStr{XH.Out.text(ob)}), J.kv("context", J.JStr{XH.Out.context(ob)})])

# home: a new outbox entry, journaled, then its first delivery
def xsend.new(-o: M.St, +cx: X.Cx<o>, +token: Nat, +ob: XH.Out) -> X.Cx<o>:
  +cx2 = xdeliver(o, xemit(o, cx, XH.XOut{ob}), ob)
  reply(o, cx2, token, ok_body([J.kv("xid", J.JNum{XH.Out.xid(ob)}), J.kv("project", J.JStr{XH.Out.project(ob)})]))

def xsend.fwd(-o: M.St, f: XB.Fwd, +cx: X.Cx<o>, +token: Nat, +project: String, +id: Nat) -> X.Cx<o>:
  match f:
    case XB.FwdErr{+e}:
      reply_err(o, cx, token, e)
    case XB.FwdOk{+text}:
      xsend.new(o, cx, token, XB.new_out(xst(o, cx), project, XH.KInput{}, id, text, ""))

# `sb project send <p> --input m_<n>` (main only, req.main): `pre_err`,
# the hub's (an unknown project, this hub itself)
def req_xsend(-o: M.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match pre:
    case Some{+e}:
      reply_err(o, cx, token, e)
    case None{}:
      xsend.fwd(o, XB.forward(st(o, cx), M.MAIN(), C.get_nat(q, "msg")), cx, token, C.get_s(q, "project"), C.get_nat(q, "msg"))

def req_xask.go(-o: M.St, err: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match err:
    case Some{+e}:
      reply_err(o, cx, token, e)
    case None{}:
      xsend.new(o, cx, token, XB.new_out(xst(o, cx), C.get_s(q, "project"), XH.KAsk{}, 0n, C.get_s(q, "text"), ""))

# `sb project ask <p> "<question>"` (main only): bise's own words
def req_xask(-o: M.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  req_xask.go(o, Maybe.or(&2, String, pre, XB.ask_ok(C.get_s(q, "text"))), cx, token, q)

# home: the project hub has it (a late ack of a settled send: nothing)
def xack(-o: M.St, m: Maybe<&2, XH.Out>, +cx: X.Cx<o>, +xid: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{_}:
      xemit(o, cx, XH.XOp{xid, XH.OpAcked{}})

# home: a delivery failed: one more try, and the next one's act
def xfail(-o: M.St, m: Maybe<&2, XH.Out>, +cx: X.Cx<o>, +xid: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+ob}:
      +cx2 = xemit(o, cx, XH.XOp{xid, XH.OpTried{}})
      aemit(o, cx2, TM.ActSet{XB.retry_act(tms(o, cx2), xid, XH.Out.tries(ob), now(o, cx2))})

def xanswer.kind(k: XH.XKind, +ob: XH.Out, +text: String) -> String:
  match k:
    case XH.KInput{}:
      cat(["(answer to the user's ", mid(XH.Out.msg(ob)), " you forwarded) ", text])
    case XH.KAsk{}:
      cat(["(answer to your question \"", M.clip(M.one_line(XH.Out.text(ob)), 200n), "\") ", text])
    case XH.KJobEnd{}:
      text

def xanswer.text(+ob: XH.Out, +text: String) -> String:
  xanswer.kind(XH.Out.kind(ob), ob, text)

# home: the project's answer settles its send, once (law
# xhub_outbox_settles_once) and reaches main from `@<project>`
def xanswer(-o: M.St, m: Maybe<&2, XH.Out>, +cx: X.Cx<o>, +xid: Nat, +name: String, +text: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+ob}:
      +cx2 = xemit(o, cx, XH.XOp{xid, XH.OpSettled{}})
      fst(o, send(o, cx2, msg(String.append("@", name), M.MAIN(), xanswer.text(ob, text), False{}, None{}, False{}, False{})))

def xretry(-o: M.St, m: Maybe<&2, XH.Out>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+ob}:
      xdeliver(o, cx, ob)

# ---- the routing hold (S2 step 2) ----
# His words to bise's main that the shell's route.rs sends to a project
# (input route_hold) wait HOLD_MS on the one clock (a RouteSend act): the
# window shows them (fx route) and he may correct the target or cancel.
# At the fire they become his message to main (an id, recorded, never a
# turn: law route_fire_makes_no_turn) forwarded by the outbox, and main
# gets a note with the id. A cancel says them to main as today.

def route_ev(-o: M.St, +cx: X.Cx<o>, +r: XH.Route) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"route"}), J.kv("rid", J.JNum{XH.Route.rid(r)}),
    J.kv("to", J.JStr{XH.Route.to(r)}), J.kv("name", J.JStr{XH.Route.name(r)}),
    J.kv("text", J.JStr{XH.Route.text(r)}), J.kv("due_ms", J.JNum{XH.Route.due(r)}),
    J.kv("why", J.JStr{XH.Route.why(r)})])

def route_done(-o: M.St, +cx: X.Cx<o>, +rid: Nat, +state: String, +to: String, +xid: J.Json) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"route_done"}), J.kv("rid", J.JNum{rid}), J.kv("state", J.JStr{state}),
    J.kv("to", J.JStr{to}), J.kv("xid", xid)])

# home: the input route_hold (the shell's route.rs picked `to`, a project
# hub id, `name` its name): held, its act set, the window told
def route_hold.to(-o: M.St, +cx: X.Cx<o>, +text: String, +via: String, +to: String, +name: String, +why: String, +context: String) -> X.Cx<o>:
  +r = XB.new_route(xst(o, cx), tms(o, cx), text, via, to, name, why, now(o, cx), context)
  +cx2 = xemit(o, cx, XH.XRouteHeld{r})
  route_ev(o, aemit(o, cx2, TM.ActSet{XB.route_act(r)}), r)

# S2 step 5: his unclear words (no `to`: the shell asks BISE_ROUTE_MODEL):
# held PICK_MS with no target, the shell asked for a pick (fx route_ask),
# the window told nothing until a project is picked
def route_unclear(-o: M.St, +cx: X.Cx<o>, +text: String, +via: String, +context: String) -> X.Cx<o>:
  +r = XB.new_route_at(xst(o, cx), tms(o, cx), text, via, "", "", "", Nat.add(now(o, cx), XB.PICK_MS()), context)
  +cx2 = aemit(o, xemit(o, cx, XH.XRouteHeld{r}), TM.ActSet{XB.route_act(r)})
  out(o, cx2, [J.kv("fx", J.JStr{"route_ask"}), J.kv("rid", J.JNum{XH.Route.rid(r)}), J.kv("text", J.JStr{text})])

def route_hold(-o: M.St, +cx: X.Cx<o>, +text: String, +via: String, +to: String, +name: String, +why: String, +context: String) -> X.Cx<o>:
  pick_lazy(X.Cx<o>, String.is_empty(to),
    u => route_unclear(o, cx, text, via, context),
    u => route_hold.to(o, cx, text, via, to, name, why, context))

# his plain words to main behind an unclear route: held with no target,
# due `d` (the latest unclear due), never asked about: at its fire it goes
# plain to main like the unclear one before it
def route_behind(-o: M.St, +cx: X.Cx<o>, +text: String, +d: Nat) -> X.Cx<o>:
  +r = XB.new_route_at(xst(o, cx), tms(o, cx), text, "", "", "", "", d, "")
  aemit(o, xemit(o, cx, XH.XRouteHeld{r}), TM.ActSet{XB.route_act(r)})

def user_input.behind(-o: M.St, m: Maybe<&2, Nat>, +cx: X.Cx<o>, +to: String, +text: String, +queued: Bool) -> X.Cx<o>:
  match m:
    case None{}:
      user_says(o, cx, to, text, queued)
    case Some{+d}:
      route_behind(o, cx, text, d)

# his plain words (inputs say, and route to main): to bise's main while an
# unclear route of his waits for the model's pick, held behind it, due at
# the latest unclear due, so both reach main in order (law
# route_plain_waits_behind_unclear); else user_says. A queued input waits
# for main's turn end already: never held here.
def user_input(-o: M.St, +cx: X.Cx<o>, +to: String, +text: String, +queued: Bool) -> X.Cx<o>:
  user_input.behind(o, Bool.pick(Maybe<&2, Nat>, Bool.not(queued) && String.eq(to, M.MAIN()), XB.unclear_due(XH.Xs.routes(xst(o, cx))), None{}), cx, to, text, queued)

# an unclear route's act fired with no target: it ends, and his words
# reach bise's main as a plain message (the cancel path, no route_done:
# the window never saw it)
def route_plain(-o: M.St, +cx: X.Cx<o>, +r: XH.Route) -> X.Cx<o>:
  +cx2 = xemit(o, cx, XH.XRoute{XH.Route.rid(r), XH.REnd{XH.RCancelled{}}})
  says.sent(o, send(o, cx2, Snd{M.USER(), M.MAIN(), XH.Route.text(r), False{}, None{}, True{}, False{}, False{}, XB.via_of(XH.Route.via(r))}), M.MAIN(), XH.Route.text(r))

def route_fire.sent(-o: M.St, +cx: X.Cx<o>, +r: XH.Route, +id: Nat) -> X.Cx<o>:
  +cx2 = emit(o, mark_delivered(o, cx, id), M.ESettled{id}, Unit{}, Unit{}, Unit{})
  +ob = XB.new_out(xst(o, cx2), XH.Route.to(r), XH.KInput{}, id, XH.Route.text(r), XH.Route.context(r))
  +cx3 = xdeliver(o, xemit(o, cx2, XH.XOut{ob}), ob)
  +cx4 = xemit(o, cx3, XH.XRoute{XH.Route.rid(r), XH.REnd{XH.RSent{}}})
  +cx5 = route_done(o, cx4, XH.Route.rid(r), "sent", XH.Route.to(r), J.JNum{XH.Out.xid(ob)})
  note_main(o, cx5, cat(["the user's words went to the project ", XH.Route.name(r), " (", mid(id), ", routed by bise): \"",
    M.clip(M.one_line(XH.Route.text(r)), 300n), "\"; its answer comes back to you from @", XH.Route.name(r)]))

# his routed words as his message to main, journaled only: no send (a
# send would pump main, a turn), no waiter, no card (law
# route_fire_makes_no_turn)
def route_said(-o: M.St, +cx: X.Cx<o>, +r: XH.Route) -> X.Cx<o>:
  +msg = {M.Msg{M.St.next_msg(st(o, cx)), M.St.next_msg(st(o, cx)), M.USER(), M.MAIN(), None{}, False{}, False{}, XH.Route.text(r), now(o, cx), True{}, False{}, XB.via_of(XH.Route.via(r))} : M.Msg}
  emit(o, cx, M.EMsgSent{msg}, {==}, quiet_ok(False{}, False{}), Unit{})

# home: a held route's act fired: his message to main is created now (an
# id for the history and `--input`), delivered and settled at once with
# no turn (law route_fire_makes_no_turn), then forwarded once
def route_fire(-o: M.St, m: Maybe<&2, XH.Route>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+r}:
      route_fire.sent(o, route_said(o, cx, r), r, M.St.next_msg(st(o, cx)))

def route_gone(-o: M.St, +cx: X.Cx<o>, +rid: Nat) -> X.Cx<o>:
  notice(o, cx, cat(["route ", Nat.show(rid), " already went or was cancelled"]))

# home: the input route_cancel: a held route ends, its act with it, and
# his words reach main as a plain message (today's path)
def route_cancel.go(-o: M.St, m: Maybe<&2, XH.Route>, +cx: X.Cx<o>, +rid: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      route_gone(o, cx, rid)
    case Some{+r}:
      +cx2 = aemit(o, cx, TM.ActDone{XH.Route.act(r)})
      +cx3 = xemit(o, cx2, XH.XRoute{rid, XH.REnd{XH.RCancelled{}}})
      +cx4 = route_done(o, cx3, rid, "cancelled", XH.Route.to(r), J.JNull{})
      says.sent(o, send(o, cx4, Snd{M.USER(), M.MAIN(), XH.Route.text(r), False{}, None{}, True{}, False{}, False{}, XB.via_of(XH.Route.via(r))}), M.MAIN(), XH.Route.text(r))

def route_cancel(-o: M.St, +cx: X.Cx<o>, +rid: Nat) -> X.Cx<o>:
  route_cancel.go(o, XB.held(xst(o, cx), rid), cx, rid)

def route_correct.go(-o: M.St, m: Maybe<&2, XH.Route>, +cx: X.Cx<o>, +rid: Nat, +to: String, +name: String) -> X.Cx<o>:
  match m:
    case None{}:
      route_gone(o, cx, rid)
    case Some{+r}:
      route_ev(o, xemit(o, cx, XH.XRoute{rid, XH.RTo{to, name}}), XH.Route.with_to(r, to, name))

# home: the input route_correct (the shell checked `to` in the registry):
# a held route gets its new project, same due time; `bise` keeps his
# words here (a cancel); a route not held: a notice (law
# route_correct_only_held)
def route_correct(-o: M.St, +cx: X.Cx<o>, +rid: Nat, +to: String, +name: String) -> X.Cx<o>:
  pick_lazy(X.Cx<o>, String.eq(to, XB.VIA_BISE()),
    u => route_cancel(o, cx, rid),
    u => route_correct.go(o, XB.held(xst(o, cx), rid), cx, rid, to, name))

# a held route's act fired: with no target, plain to main (law
# route_unclear_goes_plain); else forwarded (route_fire)
def route_go(-o: M.St, m: Maybe<&2, XH.Route>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+r}:
      pick_lazy(X.Cx<o>, XB.is_unclear(r),
        u => route_plain(o, cx, r),
        u => route_fire(o, Some{r}, cx))

# the model's pick (the shell checked `to` in the registry) for an unclear
# route still held: it ends, its act with it, and a held route to that
# project starts (his 2 s to correct start now); a route that already
# went or got a target: nothing (law route_pick_only_unclear)
def route_pick.go(-o: M.St, m: Maybe<&2, XH.Route>, +cx: X.Cx<o>, +to: String, +name: String, +why: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+r}:
      pick_lazy(X.Cx<o>, XB.is_unclear(r) && Bool.not(String.is_empty(to)),
        u => route_hold.to(o, xemit(o, aemit(o, cx, TM.ActDone{XH.Route.act(r)}), XH.XRoute{XH.Route.rid(r), XH.REnd{XH.RCancelled{}}}),
          XH.Route.text(r), XH.Route.via(r), to, name, why, XH.Route.context(r)),
        u => cx)

def route_pick(-o: M.St, +cx: X.Cx<o>, +rid: Nat, +to: String, +name: String, +why: String) -> X.Cx<o>:
  route_pick.go(o, XB.held(xst(o, cx), rid), cx, to, name, why)

def act_fire(-o: M.St, w: TM.What, +cx: X.Cx<o>) -> X.Cx<o>:
  match w:
    case TM.Retry{+xid}:
      xretry(o, XB.pending(xst(o, cx), xid), cx)
    case TM.RouteSend{+rid}:
      route_go(o, XB.held(xst(o, cx), rid), cx)

def acts_tick.each(-o: M.St, xs: List<&2, TM.Act>, +cx: X.Cx<o>) -> X.Cx<o>:
  match xs:
    case Nil{}:
      cx
    case Con{+a, +rest}:
      acts_tick.each(o, rest, act_fire(o, TM.Act.what(a), aemit(o, cx, TM.ActDone{TM.Act.id(a)})))

# the acts due at the tick fire once each (laws act_fires_once,
# act_due_only); none due: nothing (idle-cpu)
def acts_tick(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  acts_tick.each(o, TM.acts_due(TM.Tms.acts(tms(o, cx)), now(o, cx)), cx)

def xboot.each(-o: M.St, xs: List<&2, XH.Out>, +cx: X.Cx<o>) -> X.Cx<o>:
  match xs:
    case Nil{}:
      cx
    case Con{+ob, +rest}:
      xboot.each(o, rest, xdeliver(o, cx, ob))

# home: at boot the pending sends go again (the project drops a second
# copy: law xhub_xin_delivered_once)
def xboot(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  xboot.each(o, XB.pendings(XH.Xs.outbox(xst(o, cx))), cx)

def xin.snd(k: XH.XKind, +text: String) -> Snd:
  match k:
    case XH.KInput{}:
      Snd{M.USER(), M.MAIN(), text, True{}, None{}, False{}, False{}, False{}, Some{XB.VIA_BISE()}}
    case XH.KAsk{}:
      Snd{XB.FROM_BISE(), M.MAIN(), text, True{}, None{}, False{}, False{}, False{}, None{}}
    case XH.KJobEnd{}:
      Snd{XB.FROM_BISE(), M.MAIN(), text, False{}, None{}, False{}, False{}, False{}, None{}}

def xin.sent(-o: M.St, p: X.Cx<o> & SendR, +token: Nat, +hub: String, +xid: Nat, +k: XH.XKind) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      reply(o, xemit(o, cx, XH.XGot{XH.XIn{hub, xid, M.Msg.id(m), k, False{}}}), token, ok_body(Nil{}))

# J, bise's home hub: the line a followed task's end says in bise's main,
# from `@<project>`: the same words as a local task's report to main
def xjob_end.line(+j: J.Json) -> String:
  cat(["[report: ", C.get_s(j, "state"), "] @", C.get_s(j, "agent"), " (followed): ", String.trim(C.get_s(j, "summary"))])

def xjob_end.sent(-o: M.St, p: X.Cx<o> & SendR, +token: Nat, +hub: String, +xid: Nat, +j: J.Json) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      +cx2 = xemit(o, cx, XH.XGot{XH.XIn{hub, xid, M.Msg.id(m), XH.KJobEnd{}, True{}}})
      +cx3 = out(o, cx2, [J.kv("fx", J.JStr{"followed_end"}), J.kv("project", J.JStr{hub}),
        J.kv("agent", J.JStr{C.get_s(j, "agent")}), J.kv("key", J.JNum{C.get_nat(j, "key")}),
        J.kv("state", J.JStr{C.get_s(j, "state")}), J.kv("label", J.JStr{C.get_s(j, "label")}),
        J.kv("summary", J.JStr{C.get_s(j, "summary")})])
      reply(o, cx3, token, ok_body(Nil{}))

# J, bise's home hub: a task it follows in the project `name` ended: one
# line in bise's main from `@<name>`, a message like a local task's end
# report (it wakes main by the same rule: main tells him), recorded as
# received once (law xhub_xin_delivered_once), and fx followed_end for
# the window
def xjob_end(-o: M.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +token: Nat, +hub: String, +xid: Nat, +name: String) -> X.Cx<o>:
  match r:
    case Fail{+e}:
      reply_err(o, cx, token, cat(["a job_end that isn't JSON: ", e]))
    case Done{+j}:
      xjob_end.sent(o, send(o, cx, msg(String.append("@", name), M.MAIN(), xjob_end.line(j), False{}, None{}, False{}, False{})), token, hub, xid, j)

def xin.kind(-o: M.St, k: XH.XKind, +cx: X.Cx<o>, +token: Nat, +hub: String, +xid: Nat, +text: String, +name: String) -> X.Cx<o>:
  match k:
    case XH.KJobEnd{}:
      xjob_end(o, J.parse(text), cx, token, hub, xid, name)
    case XH.KInput{}:
      xin.sent(o, send(o, cx, xin.snd(XH.KInput{}, text)), token, hub, xid, XH.KInput{})
    case XH.KAsk{}:
      xin.sent(o, send(o, cx, xin.snd(XH.KAsk{}, text)), token, hub, xid, XH.KAsk{})

# project: bise's message, said to main once per origin (hub, xid); bise's
# home hub: a followed task's end (kind job_end, `name` the project's)
def xin(-o: M.St, seen: Bool, +cx: X.Cx<o>, +token: Nat, +hub: String, +xid: Nat, +k: XH.XKind, +text: String, +name: String) -> X.Cx<o>:
  match seen:
    case True{}:
      reply(o, cx, token, ok_body([J.kv("seen", J.JBool{True{}})]))
    case False{}:
      xin.kind(o, k, cx, token, hub, xid, text, name)

# project: main answered bise's message: settled here, sent back once
# (law xhub_xin_replied_once)
def xreplied(-o: M.St, +cx: X.Cx<o>, +i: XH.XIn, +text: String) -> X.Cx<o>:
  +cx2 = emit(o, cx, M.ESettled{XH.XIn.msg(i)}, Unit{}, Unit{}, Unit{})
  +cx3 = xemit(o, cx2, XH.XReplied{XH.XIn.hub(i), XH.XIn.xid(i)})
  out(o, cx3, [J.kv("fx", J.JStr{"xreply"}), J.kv("hub", J.JStr{XH.XIn.hub(i)}),
    J.kv("xid", J.JNum{XH.XIn.xid(i)}), J.kv("text", J.JStr{text})])

def xreply_due(+s: M.St, +from: String, r: Maybe<&2, Nat>) -> Maybe<&2, XH.XIn>:
  match r:
    case None{}:
      None{}
    case Some{+id}:
      Bool.pick(Maybe<&2, XH.XIn>, String.eq(from, M.MAIN()), XB.reply_due(M.St.x(s), id), None{})

# project: `sb send @bise --reply-to m_<n> "..."`: main's answer to an
# open message of bise's, before its turn ends
def req_xreply(-o: M.St, r: Maybe<&2, XH.XIn>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match r:
    case None{}:
      reply_err(o, cx, token, "@bise takes only main's answer to an open message from bise: sb send @bise --reply-to m_<n> \"...\"")
    case Some{+i}:
      reply(o, xreplied(o, cx, i, C.get_s(q, "text")), token, ok_body([J.kv("to", J.JStr{XB.FROM_BISE()}), J.kv("delivery", J.JStr{"sent"})]))

# ---- the agents' `sb` requests (RFC 0003 §4, RFC 0001 §7.2, §7.5) ----

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

def main_only(-o: M.St, +cx: X.Cx<o>, +token: Nat) -> X.Cx<o>:
  reply_err(o, cx, token, "reserved for main (the parent of the agents)")

def req_send.delivered(m: Maybe<&2, M.MSt>) -> Bool:
  match m:
    case Some{M.Delivered{}}:
      True{}
    case _:
      False{}

def req_send.res(-o: M.St, p: X.Cx<o> & SendR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      +delivered = req_send.delivered(M.state_of(st(o, cx), M.Msg.id(m)))
      reply(o, cx, token, ok_body([J.kv("message_id", J.JStr{mid(M.Msg.id(m))}),
        J.kv("thread", J.JStr{String.append("t_", ns(M.Msg.thread(m)))}), J.kv("to", J.JStr{M.Msg.to(m)}),
        J.kv("delivery", J.JStr{Bool.pick(String, delivered, "delivered", "queued")})]))

# the oldest delivered message from `asker` to `me` that still expects
# `me`'s reply
def first_from(xs: List<&2, M.Msg>, +asker: String) -> Maybe<&2, Nat>:
  match xs:
    case Nil{}:
      None{}
    case Con{+m, +t}:
      Bool.pick(Maybe<&2, Nat>, String.eq(M.Msg.from(m), asker), Some{M.Msg.id(m)}, first_from(t, asker))

def open_question.go(m: Maybe<&2, String>, +s: M.St, +me: String) -> Maybe<&2, Nat>:
  match m:
    case None{}:
      None{}
    case Some{+asker}:
      first_from(M.unanswered_for(s, me), asker)

def open_question(+s: M.St, +asker: String, +me: String) -> Maybe<&2, Nat>:
  open_question.go(M.resolve(s, asker), s, me)

def req_send.go(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  +to = C.get_s(q, "to")
  +expect = C.get_b(q, "expect_reply")
  # a plain message to an agent that asked us something answers it
  +reply_to = Maybe.or(&2, Nat, C.opt_nat(J.get(q, "reply_to")),
    Bool.pick(Maybe<&2, Nat>, expect, None{}, open_question(st(o, cx), to, from)))
  req_send.res(o, send_why(o, cx, Snd{from, to, C.get_s(q, "text"), expect, reply_to, False{}, False{}, C.get_b(q, "queued"), None{}}, C.get_s(q, "why")), token)


# BISE-299: a question main escalated (`sb card --for`) is in the user's
# inbox: only the user answers it; an agent's reply to it is refused
def req_send.held(-o: M.St, held: Maybe<&2, Nat>, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  match held:
    case Some{+id}:
      +r = C.get_nat(q, "reply_to")
      reply_err(o, cx, token, cat([mid(r), " is in the user's inbox (card #", ns(id), "): only the user answers it",
        Bool.pick(String, String.eq(from, M.MAIN()), cat([". It is moot? `sb card --withdraw ", ns(id), " \"why\"` takes it back first"]), "")]))
    case None{}:
      req_send.go(o, cx, token, from, q)

def req_send.hold(+s: M.St, r: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match r:
    case None{}:
      None{}
    case Some{+id}:
      M.card_holding(M.St.cards(s), id)

def req_send(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  pick_lazy(X.Cx<o>, String.eq(C.get_s(q, "to"), XB.FROM_BISE()),
    u => req_xreply(o, xreply_due(st(o, cx), from, C.opt_nat(J.get(q, "reply_to"))), cx, token, q),
    u => req_send.held(o, req_send.hold(st(o, cx), C.opt_nat(J.get(q, "reply_to"))), cx, token, from, q))
def wait.park(-o: M.St, +cx: X.Cx<o>, +w: X.Waiter) -> X.Cx<o>:
  +r = rr(o, cx)
  +cx2 = set_r(o, cx, X.R.with_waiters(r, M.snoc(X.Waiter, X.R.waiters(r), w)))
  dirty(o, upd_rt(o, cx2, X.Rt.with_waiting(rt(o, cx2, X.Waiter.agent(w)), True{})))

# the reply is already there (RFC 0003 §6.1-6.2): a queued one is
# delivered to the wait, unless it is a queued-mode message (it waits for
# the next turn); one already delivered is read again, without a second
# delivery; a rejected or cancelled one never reaches the wait
def wait.state(-o: M.St, s: Maybe<&2, M.MSt>, +cx: X.Cx<o>, +w: X.Waiter, +r: M.Msg) -> X.Cx<o>:
  match s:
    case Some{M.Queued{_}}:
      pick_lazy(X.Cx<o>, M.Msg.queued(r), u => wait.park(o, cx, w), u => deliver_to_waiter(o, cx, w, r, "reply"))
    case Some{M.Delivered{}}:
      waiter_reply(o, cx, w, r, "reply")
    case _:
      wait.park(o, cx, w)

def wait.existing(-o: M.St, m: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +w: X.Waiter) -> X.Cx<o>:
  match m:
    case Some{+r}:
      wait.state(o, M.state_of(st(o, cx), M.Msg.id(r)), cx, w, r)
    case None{}:
      wait.park(o, cx, w)

def reply_of(xs: List<&2, M.MRec>, +id: Nat, +replier: String) -> Maybe<&2, M.Msg>:
  match xs:
    case Nil{}:
      None{}
    case Con{+r, +t}:
      +m = M.MRec.msg(r)
      Bool.pick(Maybe<&2, M.Msg>,
        M.maybe_nat_is(M.Msg.reply_to(m), id) && (String.eq(M.Msg.from(m), replier) || String.eq(M.Msg.from(m), M.USER())),
        Some{m}, reply_of(t, id, replier))

def wait.mine(-o: M.St, mine: Bool, +cx: X.Cx<o>, +token: Nat, +from: String, +orig: M.Msg, +timeout: Nat) -> X.Cx<o>:
  match mine:
    case False{}:
      reply_err(o, cx, token, "you can only wait for the reply to your own messages")
    case True{}:
      +w = {X.Waiter{token, from, M.Msg.id(orig), Nat.add(now(o, cx), Nat.mul(1000n, Nat.min(timeout, WAIT_CAP())))} : X.Waiter}
      wait.existing(o, reply_of(M.St.msgs(st(o, cx)), M.Msg.id(orig), M.Msg.to(orig)), cx, w)

def wait.orig(-o: M.St, m: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +token: Nat, +from: String, +id: Nat, +timeout: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("message inconnu : ", mid(id)))
    case Some{+orig}:
      wait.mine(o, String.eq(M.Msg.from(orig), from), cx, token, from, orig, timeout)

def wait(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +id: Nat, +timeout: Nat) -> X.Cx<o>:
  wait.orig(o, M.msg_of(st(o, cx), id), cx, token, from, id, timeout)

def req_ask.res(-o: M.St, p: X.Cx<o> & SendR, +token: Nat, +from: String, +timeout: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      wait(o, cx, token, from, M.Msg.id(m), timeout)

# ---- the REPLs ----

def decl_of(+s: String) -> M.Decl:
  Bool.pick(M.Decl, String.eq(s, "done"), M.DDone{}, Bool.pick(M.Decl, String.eq(s, "blocked"), M.DBlocked{}, M.DWorking{}))

def req_status(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +is_main: Bool, +q: J.Json) -> X.Cx<o>:
  +status = C.get_s(q, "status")
  +note = C.get_s(q, "note")
  +cx2 = emit(o, cx, M.EDeclared{from, Some{decl_of(status)}, note}, Unit{}, Unit{}, Unit{})
  # a blocked task is main's to handle (BISE-299): main escalates to the
  # user with `sb card` when it needs them
  +cx3 = pick_lazy(X.Cx<o>, String.eq(status, "blocked") && Bool.not(is_main),
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
def job_label(+a: M.Agent) -> String:
  M.clip(M.one_line(J.get_str_or(M.Agent.brief(a), "objective", "")), 120n)

# J: the end of a job followed for another hub (bise's home hub): to its
# outbox, journaled, delivered until that hub has it (a restart of either
# hub loses nothing: the project's boot sends its pending entries again,
# the home hub drops a second copy, law xhub_xin_delivered_once)
def job_end.back(-o: M.St, local: Bool, +cx: X.Cx<o>, +hub: String, +j: J.Json) -> X.Cx<o>:
  match local:
    case True{}:
      cx
    case False{}:
      +ob = XB.new_out(xst(o, cx), hub, XH.KJobEnd{}, 0n, J.stringify(j), "")
      xdeliver(o, xemit(o, cx, XH.XOut{ob}), ob)

# `key`: the job's (its end's time on this hub's clock), the same in the
# fx job_end and in what goes back to `hub`, so a window that hears both
# shows one line
def job_end.go(-o: M.St, f: Bool, +cx: X.Cx<o>, +name: String, +state: String, +summary: String, +label: String, +hub: String) -> X.Cx<o>:
  match f:
    case True{}:
      +cx2 = emit(o, cx, M.EJobEnd{name, state}, Unit{}, Unit{}, Unit{})
      +key = now(o, cx2)
      +cx3 = out(o, cx2, [J.kv("fx", J.JStr{"job_end"}), J.kv("agent", J.JStr{name}), J.kv("state", J.JStr{state}),
        J.kv("label", J.JStr{label}), J.kv("summary", J.JStr{summary}), J.kv("key", J.JNum{key}), J.kv("hub", J.JStr{hub})])
      job_end.back(o, String.is_empty(hub), cx3, hub, J.obj([J.kv("agent", J.JStr{name}), J.kv("key", J.JNum{key}),
        J.kv("state", J.JStr{state}), J.kv("label", J.JStr{label}), J.kv("summary", J.JStr{summary})]))
    case False{}:
      cx

def job_end.of(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String, +state: String, +summary: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      job_end.go(o, M.Agent.follow(a), cx, name, state, summary, job_label(a), M.Agent.follow_hub(a))

# a task's end (done, failed): its job_end when followed, once (laws
# follow_done_once, follow_failed_at_once)
def job_end(-o: M.St, +cx: X.Cx<o>, +name: String, +state: String, +summary: String) -> X.Cx<o>:
  job_end.of(o, M.get_agent(st(o, cx), name), cx, name, state, summary)

def is_archived_life(l: M.Life) -> Bool:
  match l:
    case M.LArchived{}:
      True{}
    case _:
      False{}

def follow_err.of(m: Maybe<&2, M.Agent>, +name: String) -> Maybe<&2, String>:
  match m:
    case None{}:
      Some{cat(["no agent named @", name])}
    case Some{+a}:
      Bool.pick(Maybe<&2, String>, M.Agent.is_main(a), Some{"main isn't a job: follow one of its tasks"},
        Bool.pick(Maybe<&2, String>, is_archived_life(M.Agent.life(a)), Some{cat(["@", name, " is archived"])}, None{}))

# who can be followed: a task of this hub, not main, not archived
def follow_err(+s: M.St, +name: String) -> Maybe<&2, String>:
  follow_err.of(M.get_agent(s, name), name)

# `hub`: who asked ("" this hub's own main or window; else bise's home
# hub, J): journaled with the follow, cleared at its job_end
def follow_set(-o: M.St, +cx: X.Cx<o>, +name: String, +on: Bool, +hub: String) -> X.Cx<o>:
  emit(o, cx, M.EFollow{name, on, hub}, Unit{}, Unit{}, Unit{})

# the input follow (the window's typed command, or bise's `sb follow
# <p>/<agent>` on this hub's socket): an error is the asker's notice
def follow_in(-o: M.St, err: Maybe<&2, String>, +cx: X.Cx<o>, +name: String, +on: Bool) -> X.Cx<o>:
  match err:
    case Some{+e}:
      notice(o, cx, e)
    case None{}:
      follow_set(o, cx, name, on, "")

def req_follow.go(-o: M.St, err: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +name: String, +on: Bool, +hub: String) -> X.Cx<o>:
  match err:
    case Some{+e}:
      reply_err(o, cx, token, e)
    case None{}:
      reply(o, follow_set(o, cx, name, on, hub), token, ok_body([J.kv("agent", J.JStr{name}), J.kv("on", J.JBool{on})]))

# `sb follow <agent> [--off]`: main only (his say-so goes through main)
def req_follow(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  +name = C.get_s(q, "agent")
  req_follow.go(o,
    Bool.pick(Maybe<&2, String>, String.eq(from, M.MAIN()), follow_err(st(o, cx), name), Some{"only main follows a task: ask main"}),
    cx, token, name, Bool.not(C.get_b(q, "off")), C.get_s(q, "hub"))

# the report's done and failed: their state change, then the job's end
def report_done(-o: M.St, +cx: X.Cx<o>, +from: String) -> X.Cx<o>:
  emit(o, cx, M.EDeclared{from, Some{M.DDone{}}, ""}, Unit{}, Unit{}, Unit{})

def report_failed(-o: M.St, +cx: X.Cx<o>, +from: String, +summary: String) -> X.Cx<o>:
  emit(o, cx, M.ELifecycle{from, M.LFailed{}, Some{summary}}, Unit{}, Unit{}, Unit{})

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

def req_report.sent(-o: M.St, p: X.Cx<o> & SendR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      reply(o, cx, token, ok_body([J.kv("message_id", J.JStr{mid(M.Msg.id(m))})]))

def req_report.kind(-o: M.St, +cx: X.Cx<o>, +from: String, +kind: String, +summary: String) -> X.Cx<o>:
  pick_lazy(X.Cx<o>, String.eq(kind, "done"),
    u => job_end(o, report_done(o, cx, from), from, "done", summary),
    u => pick_lazy(X.Cx<o>, String.eq(kind, "blocked"),
      u => emit(o, cx, M.EDeclared{from, Some{M.DBlocked{}}, summary}, Unit{}, Unit{}, Unit{}),
      u => pick_lazy(X.Cx<o>, String.eq(kind, "failed"),
        u => job_end(o, report_failed(o, cx, from, summary), from, "failed", summary),
        u => cx)))

def req_report.go(-o: M.St, is_main: Bool, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  match is_main:
    case True{}:
      reply_err(o, cx, token, "main reports to no one")
    case False{}:
      +kind = C.get_s(q, "kind")
      +summary = C.get_s(q, "summary")
      +decisions = jstr_list(Maybe.default(&2, List<&2, J.Json>, J.get_arr(q, "decisions"), Nil{}))
      # S10: a progress's --step n/m; emitter 5: its --result (proto's
      # DiffResult, checked by the CLI); null when absent
      +cx2 = emit(o, cx, M.EReported{from, J.obj([J.kv("at_ms", J.JNum{now(o, cx)}), J.kv("kind", J.JStr{kind}),
        J.kv("summary", J.JStr{summary}), J.kv("decisions", J.JArr{jstrs(decisions)}), J.kv("auto", J.JBool{False{}}),
        J.kv("step", Maybe.default(&2, J.Json, J.get(q, "step"), J.JNull{})), J.kv("of", Maybe.default(&2, J.Json, J.get(q, "of"), J.JNull{})),
        J.kv("result", Maybe.default(&2, J.Json, J.get(q, "result"), J.JNull{}))])}, Unit{}, Unit{}, Unit{})
      +cx3 = req_report.kind(o, cx2, from, kind, summary)
      +parent = Maybe.default(&2, String, parent_of(M.get_agent(st(o, cx3), from)), M.MAIN())
      +to = Bool.pick(String, String.eq(parent, M.USER()), M.MAIN(), parent)
      # the report answers the parent's open request, if any: no
      # automatic reply repeats it at the end of the turn
      +reply_to = open_question(st(o, cx3), to, from)
      req_report.sent(o, send(o, cx3, msg(from, to, report_text(kind, summary, decisions), False{}, reply_to, False{}, False{})), token)

def req_spawn.ws(m: Maybe<&2, M.Agent>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+a}:
      M.Agent.ws(a)

def req_spawn.res(-o: M.St, p: X.Cx<o> & TaskR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      +ws = req_spawn.ws(M.get_agent(st(o, cx), n))
      reply(o, cx, token, ok_body([J.kv("name", J.JStr{n}), J.kv("path", J.JStr{M.ws_path(ws)}), J.kv("branch", M.ws_branch(ws))]))
    case (+cx, TErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, THalt{}):
      cx

def new_task(+parent: String, +q: J.Json) -> NewTask:
  NewTask{parent, C.get_s(q, "base"), Maybe.default(&2, J.Json, J.get(q, "brief"), J.JNull{}), C.get_s(q, "brief_text"),
    C.get_s(q, "objective"), C.get_b(q, "worktree"), C.get_b(q, "with_changes")}

def req_spawn.pre(-o: M.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match pre:
    case Some{+e}:
      reply_err(o, cx, token, e)
    case None{}:
      req_spawn.res(o, create_in(o, cx, new_task(M.MAIN(), q), J.get(q, "join")), token)

def req_interrupt(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +agent: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no agent named ", agent))
    case Some{+n}:
      pick_lazy(X.Cx<o>, is_busy(X.Rt.run(rt(o, cx, n))),
        u => reply(o, stopped(o, interrupt.fx(o, cx, n), n), token, ok_body(Nil{})),
        u => reply_err(o, cx, token, "no turn in progress"))

def req_stop(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +agent: String, +reason: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no agent named ", agent))
    case Some{+n}:
      pick_lazy(X.Cx<o>, String.eq(n, M.MAIN()), u => reply_err(o, cx, token, String.append("no agent named ", agent)),
        u => reply(o, line(o, stop_noted(o, cx, n, M.LStopped{}, reason), M.MAIN(), "info", cat(["main → @", n, " stopped: ", reason])), token, ok_body(Nil{})))

def req_drop.res(-o: M.St, p: X.Cx<o> & DropR, +token: Nat, +agent: String) -> X.Cx<o>:
  match p:
    case (+cx, DDone{}):
      reply(o, cx, token, ok_body([J.kv("dropped", J.JBool{True{}})]))
    case (+cx, DAsk{+q}):
      +n = Maybe.default(&2, String, M.resolve(st(o, cx), agent), agent)
      +id = M.St.next_card(st(o, cx))
      +text = Bool.pick(String, String.ends_with(q, " [y/N]"), String.take(q, Nat.sub(String.length(q), 6n)), q)
      +cx2 = open_card(o, cx, "drop", n, cat(["main suggests: ", text, " (answer yes or no)"]), None{})
      reply(o, cx2, token, ok_body([J.kv("dropped", J.JBool{False{}}), J.kv("card", J.JNum{id}),
        J.kv("reason", J.JStr{"the user must confirm: an attention card is open"})]))
    case (+cx, DErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, DHalt{}):
      cx

def req_card.settle(-o: M.St, m: Maybe<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+id}:
      emit(o, cx, M.ESettled{id}, Unit{}, Unit{}, Unit{})

# a card main opens for a task's message (`sb card --for m_<n>`) shows in
# that task's own feed too (a registered agent of this hub, not main) (bar A.3, architect m_10203), with the line
# main's feed gets: a view; open or answered still comes from the cards
def is_task_of(-o: M.St, +cx: X.Cx<o>, +agent: String) -> Bool:
  has_agent(o, cx, agent) && Bool.not(String.eq(agent, M.MAIN()))

def card_to_asker(-o: M.St, +cx: X.Cx<o>, +id: Nat, +agent: String, +text: String) -> X.Cx<o>:
  pick_lazy(X.Cx<o>, is_task_of(o, cx, agent),
    u => line(o, cx, agent, "card", cat(["#", ns(id), " question @", agent, " : ", text])),
    u => cx)

def req_card.go(-o: M.St, +cx: X.Cx<o>, +token: Nat, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>) -> X.Cx<o>:
  +id = M.St.next_card(st(o, cx))
  +cx2 = card_to_asker(o, open_card(o, cx, "question", agent, text, for_msg), id, agent, text)
  # main hands the question over: no automatic reply
  +cx3 = req_card.settle(o, for_msg, cx2)
  reply(o, cx3, token, ok_body([J.kv("card", J.JNum{id})]))

def req_card.msg(-o: M.St, q: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +token: Nat, +text: String, +id: Nat) -> X.Cx<o>:
  match q:
    case None{}:
      reply_err(o, cx, token, String.append("message inconnu : ", mid(id)))
    case Some{+m}:
      req_card.go(o, cx, token, M.Msg.from(m), text, Some{id})

def req_card(-o: M.St, for_msg: Maybe<&2, Nat>, +cx: X.Cx<o>, +token: Nat, +text: String) -> X.Cx<o>:
  match for_msg:
    case None{}:
      req_card.go(o, cx, token, M.MAIN(), text, None{})
    case Some{+id}:
      req_card.msg(o, M.msg_of(st(o, cx), id), cx, token, text, id)

# `sb close N ["note"]`: a card in the user's inbox is the user's to
# answer or close (BISE-299): refused; an older journal's card of another
# kind (closed at boot anyway) closes with a short resolution
def req_close(-o: M.St, m: Maybe<&2, M.Card>, +cx: X.Cx<o>, +token: Nat, +id: Nat, +note: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no open card #", ns(id)))
    case Some{+c}:
      +res = Bool.pick(String, String.is_empty(M.one_line(note)), "closed by main", String.append("main: ", M.one_line(note)))
      pick_lazy(X.Cx<o>, M.user_kind(M.Card.kind(c)),
        u => reply_err(o, cx, token, cat(["card #", ns(id), " is in the user's inbox: only the user answers or closes it (your own question, now moot: sb card --withdraw ", ns(id), " \"why\")"])),
        u => reply(o, close_card(o, cx, id, res), token, ok_body([J.kv("card", J.JNum{id})])))

# `sb card --withdraw N "why"` (BISE-299): main takes back its own
# escalation (a question or a drop, never a confirmation) when it is moot;
# the user sees it withdrawn in main's feed, and the question it held is
# main's again
def req_withdraw(-o: M.St, m: Maybe<&2, M.Card>, +cx: X.Cx<o>, +token: Nat, +id: Nat, +why: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no open card #", ns(id)))
    case Some{+c}:
      +k = M.Card.kind(c)
      pick_lazy(X.Cx<o>, String.eq(k, "question") || String.eq(k, "drop"),
        u => pick_lazy(X.Cx<o>, String.is_empty(M.one_line(why)),
          u => reply_err(o, cx, token, "say why: sb card --withdraw N \"why\" (the user reads it)"),
          u => reply(o, close_card(o, cx, id, String.append("withdrawn by main: ", M.one_line(why))), token, ok_body([J.kv("card", J.JNum{id})]))),
        u => reply_err(o, cx, token, cat(["card #", ns(id), " is a ", k, ": only the user answers or closes it"])))

# `sb rename <task> <new-name>`: the same rules as the user's /rename
def req_rename.go(-o: M.St, ok: Bool, +cx: X.Cx<o>, +token: Nat, +old: String, +new: String) -> X.Cx<o>:
  match ok:
    case False{}:
      reply_err(o, cx, token, String.append("invalid or taken name: ", new))
    case True{}:
      reply(o, rename.apply(o, cx, old, new), token, ok_body([J.kv("name", J.JStr{new})]))

def req_rename(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +name: String, +new: String, +valid: Bool) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("no agent named ", name))
    case Some{+old}:
      pick_lazy(X.Cx<o>, String.eq(old, M.MAIN()), u => reply_err(o, cx, token, String.append("no agent named ", name)),
        u => req_rename.go(o, valid && Bool.not(M.name_taken(st(o, cx), new)), cx, token, old, new))

# `sb restore` / `sb isolate`: the user or main (main only on the
# user's explicit request: its prompt says so)
def req_task.res(-o: M.St, p: X.Cx<o> & TaskR, +token: Nat) -> X.Cx<o>:
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

def req.main.go(-o: M.St, c: Cmd, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match c:
    case CSpawn{}:
      req_spawn.pre(o, C.opt_str(J.get(q, "pre_err")), cx, token, q)
    case CInterrupt{}:
      req_interrupt(o, M.resolve(st(o, cx), C.get_s(q, "agent")), cx, token, C.get_s(q, "agent"))
    case CStop{}:
      req_stop(o, M.resolve(st(o, cx), C.get_s(q, "agent")), cx, token, C.get_s(q, "agent"), C.get_s(q, "reason"))
    case CDrop{}:
      req_drop.res(o, drop_task(o, cx, C.get_s(q, "agent"), False{}), token, C.get_s(q, "agent"))
    case CClose{}:
      req_close(o, M.card_of(M.St.cards(st(o, cx)), C.get_nat(q, "card")), cx, token, C.get_nat(q, "card"), C.get_s(q, "note"))
    case CRename{}:
      req_rename(o, M.resolve(st(o, cx), C.get_s(q, "agent")), cx, token, C.get_s(q, "agent"), C.get_s(q, "new_name"), C.get_b(q, "valid"))
    case CRestore{}:
      req_task.res(o, restore_task(o, cx, C.get_s(q, "agent")), token)
    case CIsolate{}:
      req_task.res(o, isolate_task(o, cx, C.get_s(q, "agent")), token)
    case CMove{}:
      req_task.res(o, move_task(o, C.opt_str(J.get(q, "pre_err")), cx, C.get_s(q, "agent"), J.get(q, "ws")), token)
    case CWithdraw{}:
      req_withdraw(o, M.card_of(M.St.cards(st(o, cx)), C.get_nat(q, "card")), cx, token, C.get_nat(q, "card"), C.get_s(q, "why"))
    case CProjectSend{}:
      req_xsend(o, C.opt_str(J.get(q, "pre_err")), cx, token, q)
    case CProjectAsk{}:
      req_xask(o, C.opt_str(J.get(q, "pre_err")), cx, token, q)
    case _:
      req_card(o, C.opt_nat(J.get(q, "for")), cx, token, C.get_s(q, "text"))

def req.main(-o: M.St, is_main: Bool, +cx: X.Cx<o>, +token: Nat, +c: Cmd, +q: J.Json) -> X.Cx<o>:
  match is_main:
    case False{}:
      main_only(o, cx, token)
    case True{}:
      req.main.go(o, c, cx, token, q)

def req.go(-o: M.St, c: Cmd, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  match c:
    case CSend{}:
      req_send(o, cx, token, from, q)
    case CWait{}:
      wait(o, cx, token, from, C.get_nat(q, "msg"), C.get_nat(q, "timeout_s"))
    case CAsk{}:
      req_ask.res(o, send(o, cx, msg(from, C.get_s(q, "to"), C.get_s(q, "text"), True{}, None{}, False{}, False{})), token, from, C.get_nat(q, "timeout_s"))
    case CStatus{}:
      req_status(o, cx, token, from, String.eq(from, M.MAIN()), q)
    case CReport{}:
      req_report.go(o, String.eq(from, M.MAIN()), cx, token, from, q)
    case CFollow{}:
      req_follow(o, cx, token, from, q)
    case other:
      req.main(o, String.eq(from, M.MAIN()), cx, token, other, q)

def req.from(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +raw: String, +q: J.Json) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("agent inconnu : ", raw))
    case Some{+from}:
      req.go(o, cmd_tag(C.get_s(q, "cmd")), cx, token, from, q)

def agent_req(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  req.from(o, M.resolve(st(o, cx), from), cx, token, from, q)

# ---- the step ----

def Hub.d(x: Hub) -> M.St:
  match x:
    case Hub{+d, _}:
      d

def Hub.r(x: Hub) -> X.R:
  match x:
    case Hub{_, +r}:
      r

def hub_new(+workspace: String) -> Hub:
  Hub{M.init(workspace), X.R{Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, workspace, False{}, 0n, False{}, Nil{}, None{}}}

# the context of one input over the hub
def start(+h: Hub, +i: J.Json) -> X.Cx<Hub.d(h)>:
  +r = Hub.r(h)
  +r2 = X.R.with_need(X.R.with_ans(X.R.with_git(X.R.with_now(X.R.with_dirty(r, False{}), C.get_nat(i, "now")), C.get_b(i, "git")),
    Maybe.default(&2, List<&2, J.Json>, J.get_arr(i, "ans"), Nil{})), None{})
  X.Cx{X.core_start(Hub.d(h)), r2}

def agent_decl(m: Maybe<&2, M.Agent>) -> Maybe<&2, M.Decl>:
  match m:
    case None{}:
      None{}
    case Some{+a}:
      M.Agent.declared(a)

def turn.resume(-o: M.St, go: Bool, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  match go:
    case True{}:
      emit(o, cx, M.EDeclared{agent, None{}, ""}, Unit{}, Unit{}, Unit{})
    case False{}:
      cx

def decl_resumes(d: Maybe<&2, M.Decl>) -> Bool:
  match d:
    case Some{M.DDone{}}:
      True{}
    case Some{M.DBlocked{}}:
      True{}
    case _:
      False{}

def turn_started(-o: M.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  +decl = agent_decl(M.get_agent(st(o, cx), agent))
  +cx2 = dirty(o, upd_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(rt(o, cx, agent), M.RBusy{}), Some{now(o, cx)})))
  turn.resume(o, decl_resumes(decl), cx2, agent)

def steer_count(-o: M.St, +cx: X.Cx<o>, +agent: String, +injected: Bool) -> X.Cx<o>:
  +r = rr(o, cx)
  +c = cnt_get(X.R.counts(r), agent)
  +c2 = Bool.pick(X.Cnt, injected, X.Cnt{agent, X.Cnt.recv(c), 1n+X.Cnt.inj(c)}, X.Cnt{agent, 1n+X.Cnt.recv(c), X.Cnt.inj(c)})
  set_r(o, cx, X.R.with_counts(r, M.snoc(X.Cnt, cnt_del(X.R.counts(r), agent), c2)))

def assistant(-o: M.St, +cx: X.Cx<o>, +agent: String, +text: String) -> X.Cx<o>:
  +r = rr(o, cx)
  set_r(o, cx, X.R.with_last(r, M.snoc(X.Kv, kv_del(X.R.last(r), agent), X.Kv{agent, text})))

# ---- file overlap (RFC 0001 §10.3) ----

def touch.others(-o: M.St, others: List<&2, String>, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  match others:
    case Nil{}:
      cx
    case Con{+h, +t}:
      +text = cat([path, " is changed by @", agent, " and by @", String.join({h <> t : List<&2, String>}, ", @")])
      note_main(o, cx, String.append("file overlap: ", text))

# an overlap is within one place (dev-flow §3.1): the shared folder, or
# one worktree its agents share
def overlap_hit(m: Maybe<&2, M.Agent>, +agent: String, +path: String, +place: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      Bool.not(M.Agent.is_main(a)) && Bool.not(String.eq(M.Agent.name(a), agent))
        && String.eq(M.Agent.place(a), place) && is_active(M.Agent.life(a))
        && M.str_in(M.Agent.files(a), path)

def overlap_others(xs: List<&2, String>, +s: M.St, +agent: String, +path: String, +place: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = overlap_others(t, s, agent, path, place)
      Bool.pick(List<&2, String>, overlap_hit(M.get_agent(s, n), agent, path, place), n <> rest, rest)

def touch.new(-o: M.St, known: Bool, +cx: X.Cx<o>, +agent: String, +path: String, +place: String) -> X.Cx<o>:
  match known:
    case True{}:
      cx
    case False{}:
      +cx2 = emit(o, cx, M.EFileTouched{agent, path}, Unit{}, Unit{}, Unit{})
      touch.others(o, overlap_others(M.St.order(st(o, cx2)), st(o, cx2), agent, path, place), cx2, agent, path)

def touch.task(-o: M.St, task: Bool, +cx: X.Cx<o>, +agent: String, +path: String, +known: Bool, +place: String) -> X.Cx<o>:
  match task:
    case True{}:
      touch.new(o, known, cx, agent, path, place)
    case False{}:
      cx

def touch.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      # every task, in the shared folder or a worktree: `sb land` commits
      # an agent's own files, two agents may share a worktree
      touch.task(o, Bool.not(M.Agent.is_main(a)), cx, agent, path, M.str_in(M.Agent.files(a), path), M.Agent.place(a))

def touch(-o: M.St, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  touch.agent(o, M.get_agent(st(o, cx), agent), cx, agent, path)

def repl_ready(-o: M.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  +cx2 = upd_rt(o, cx, X.Rt.with_waiting(X.Rt.with_run(rt(o, cx, agent), M.RIdle{}), False{}))
  pump(o, dirty(o, cx2), agent)

def crash.restart(-o: M.St, +cx: X.Cx<o>, +agent: String, +n: Nat, +reason: String) -> X.Cx<o>:
  +cx2 = set_run(o, cx, agent, M.RStarting{})
  +cx3 = out(o, cx2, [J.kv("fx", J.JStr{"spawn"}), J.kv("agent", J.JStr{agent}),
    J.kv("resume", J.JBool{True{}}), J.kv("crash_note", J.JStr{reason})])
  +cx4 = line(o, cx3, M.MAIN(), "warn", cat(["the session of @", agent, " crashed (", reason, ") — restart ", ns(n), "/", ns(MAX_CRASHES())]))
  pick_lazy(X.Cx<o>, String.eq(agent, M.MAIN()), u => cx4,
    u => notify_main(o, cx4, cat(["agent @", agent, " crashed (", reason, "). Its session restarts on its saved checkpoint (attempt ", ns(n), "/", ns(MAX_CRASHES()), "); the turn it was running is interrupted and it may need a new message to continue."])))

def crash.fail(-o: M.St, +cx: X.Cx<o>, +agent: String, +n: Nat, +reason: String) -> X.Cx<o>:
  +why = cat(["the REPL crashed ", ns(n), " times: ", reason])
  +cx2 = job_end(o, emit(o, cx, M.ELifecycle{agent, M.LFailed{}, Some{why}}, Unit{}, Unit{}, Unit{}), agent, "failed", why)
  +cx3 = line(o, cx2, M.MAIN(), "warn", cat(["@", agent, " failed: ", why]))
  pick_lazy(X.Cx<o>, String.eq(agent, M.MAIN()), u => cx3,
    u => notify_main(o, cx3, cat(["agent @", agent, " failed: ", why, ". It is not restarted anymore. A new message from you or the user restarts it; otherwise tell the user."])))

def crash.pick(-o: M.St, again: Bool, +cx: X.Cx<o>, +agent: String, +n: Nat, +reason: String) -> X.Cx<o>:
  match again:
    case True{}:
      crash.restart(o, cx, agent, n, reason)
    case False{}:
      crash.fail(o, cx, agent, n, reason)

def crash.count(-o: M.St, go: Bool, +cx: X.Cx<o>, +agent: String, +reason: String) -> X.Cx<o>:
  match go:
    case False{}:
      cx
    case True{}:
      +r = rr(o, cx)
      +n = {1n+kn_get(X.R.crashes(r), agent) : Nat}
      +cx2 = set_r(o, cx, X.R.with_crashes(r, M.snoc(X.Kn, kn_del(X.R.crashes(r), agent), X.Kn{agent, n})))
      crash.pick(o, Nat.is_le(n, MAX_CRASHES()), cx2, agent, n, reason)

def exited.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +agent: String, +crashed: Bool, +reason: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      +cx2 = set_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(rt(o, cx, agent), M.RDown{}), None{}))
      crash.count(o, crashed && is_active(M.Agent.life(a)), dirty(o, cx2), agent, reason)

def repl_exited(-o: M.St, +cx: X.Cx<o>, +agent: String, +crashed: Bool, +reason: String) -> X.Cx<o>:
  +cx2 = fail_waiters(o, cx, agent, "interrupted_by_restart")
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_steered(r, steer_del(X.R.steered(r), agent)))
  exited.agent(o, M.get_agent(st(o, cx3), agent), cx3, agent, crashed, reason)

def boot.live(-o: M.St, live: Bool, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  match live:
    case True{}:
      +cx2 = set_run(o, cx, n, M.RStarting{})
      out(o, cx2, [J.kv("fx", J.JStr{"spawn"}), J.kv("agent", J.JStr{n}),
        J.kv("resume", J.JBool{True{}}), J.kv("crash_note", J.JNull{})])
    case False{}:
      cx

def boot.one(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      boot.live(o, is_active(M.Agent.life(a)), cx, n)

def boot.each(-o: M.St, names: List<&2, String>, +cx: X.Cx<o>) -> X.Cx<o>:
  match names:
    case Nil{}:
      dirty(o, cx)
    case Con{+n, +rest}:
      boot.each(o, rest, boot.one(o, M.get_agent(st(o, cx), n), cx, n))

# the daemon started: spawn every live agent. An older journal's cards
# that are not the user's (done, blocked, failed, overlap: the agents'
# traffic, main's since BISE-299) close first; their reports and notes
# are in main's thread already
def boot(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  +cx2 = close_ids(o, M.legacy_cards(M.St.cards(st(o, cx))), cx, "moved to main's inbox")
  boot.each(o, M.St.order(st(o, cx2)), cx2)

def tick.each(-o: M.St, late: List<&2, X.Waiter>, +cx: X.Cx<o>) -> X.Cx<o>:
  match late:
    case Nil{}:
      cx
    case Con{+w, +rest}:
      +cx2 = reply(o, cx, X.Waiter.token(w), J.obj([J.kv("ok", J.JBool{False{}}), J.kv("error", J.JStr{"timeout"}),
        J.kv("hint", J.JStr{cat(["no reply to ", mid(X.Waiter.msg(w)), " yet. It stays valid: end your turn (the reply will wake you up) or continue with an explicit assumption."])})]))
      +still = any_waiter(X.R.waiters(rr(o, cx2)), X.Waiter.agent(w))
      +cx3 = dirty(o, upd_rt(o, cx2, X.Rt.with_waiting(rt(o, cx2, X.Waiter.agent(w)), still)))
      tick.each(o, rest, pump(o, cx3, X.Waiter.agent(w)))

def tick.keep(-o: M.St, +cx: X.Cx<o>) -> List<&2, X.Waiter>:
  waiters_keep(X.R.waiters(rr(o, cx)), X.R.now(rr(o, cx)))

def tick.go(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  tick.each(o, waiters_late(X.R.waiters(rr(o, cx)), X.R.now(rr(o, cx))), set_waiters(o, cx, tick.keep(o, cx)))

# the late waits get their timeout; the others stay (law
# tick_clears_late_waiters; replies and pumps never add a wait)
def tick(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  set_waiters(o, tick.go(o, cx), tick.keep(o, cx))

def requeue(-o: M.St, ids: List<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      requeue(o, rest, emit(o, cx, M.EMsgState{id, M.Queued{"steer_leftover"}}, Unit{}, Unit{}, {==}))

def skip_steered(xs: List<&2, M.Msg>, +skip: Bool, +ids: List<&2, Nat>) -> List<&2, M.Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = skip_steered(t, skip, ids)
      Bool.pick(List<&2, M.Msg>, skip && M.nat_in(ids, M.Msg.id(h)), rest, h <> rest)

# RFC 0003 §5.1: the end-of-turn answer to `@task message` typed in
# another view goes back to that view, and main gets it as a note
def auto_reply.local(-o: M.St, via: Maybe<&2, String>, +cx: X.Cx<o>, +agent: String, +m: M.Msg, +body: String) -> X.Cx<o>:
  match via:
    case None{}:
      fst(o, send(o, cx, msg(agent, M.Msg.from(m), body, False{}, Some{M.Msg.id(m)}, False{}, True{})))
    case Some{+view}:
      +cx2 = emit(o, cx, M.ESettled{M.Msg.id(m)}, Unit{}, Unit{}, Unit{})
      # an agent writing to the user (C2 `msg-you`, level 2)
      +cx3 = line(o, cx2, view, "msg-you", cat([agent, " : ", body]))
      note_main(o, cx3, cat(["@", agent, " answered the user (asked from @", view, "'s view: \"", M.clip(M.one_line(M.Msg.text(m)), 500n),
        "\"): \"", M.clip_tail(M.one_line(body), 2000n), "\""]))

# a message bise sent this hub (S2) is answered back to bise's hub
def auto_reply.x(-o: M.St, r: Maybe<&2, XH.XIn>, via: Maybe<&2, String>, +cx: X.Cx<o>, +agent: String, +m: M.Msg, +body: String) -> X.Cx<o>:
  match r:
    case Some{+i}:
      xreplied(o, cx, i, body)
    case None{}:
      auto_reply.local(o, via, cx, agent, m, body)

def auto_reply(-o: M.St, via: Maybe<&2, String>, +cx: X.Cx<o>, +agent: String, +m: M.Msg, +body: String) -> X.Cx<o>:
  auto_reply.x(o, XB.reply_due(xst(o, cx), M.Msg.id(m)), via, cx, agent, m, body)

# RFC 0003 §7: a question gets an answer, the end of the turn
def auto_replies(-o: M.St, qs: List<&2, M.Msg>, +cx: X.Cx<o>, +agent: String, +body: String) -> X.Cx<o>:
  match qs:
    case Nil{}:
      cx
    case Con{+m, +rest}:
      auto_replies(o, rest, auto_reply(o, Bool.pick(Maybe<&2, String>, String.eq(M.Msg.from(m), M.USER()), M.Msg.via(m), None{}), cx, agent, m, body), agent, body)

def idle.report(-o: M.St, go: Bool, +cx: X.Cx<o>, +agent: String, +text: String) -> X.Cx<o>:
  match go:
    case True{}:
      emit(o, cx, M.EReported{agent, J.obj([J.kv("at_ms", J.JNum{now(o, cx)}), J.kv("kind", J.JStr{"turn"}),
        J.kv("summary", J.JStr{M.clip(text, 1000n)}), J.kv("decisions", J.JArr{Nil{}}), J.kv("auto", J.JBool{True{}})])}, Unit{}, Unit{}, Unit{})
    case False{}:
      cx

def idle.nudge(-o: M.St, go: Bool, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  match go:
    case True{}:
      +cx2 = upd_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(rt(o, cx, agent), M.RBusy{}), Some{now(o, cx)}))
      out(o, cx2, [J.kv("fx", J.JStr{"say"}), J.kv("agent", J.JStr{agent}),
        J.kv("text", J.JStr{"[bise] The message(s) just above arrived while you were writing your last answer: you have not answered them yet. Answer them now."})])
    case False{}:
      cx

# held (expired-ux): the turn stopped on the expired ChatGPT sign-in and
# goes on once the user signs in again: no automatic reply nor report (a
# `(no written reply…)` woke the parent, on the same plan, into a turn
# that failed too); the questions stay unanswered for the turn to come
def idle.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +agent: String, +leftover: Bool, +unread: Bool, +steered: List<&2, Nat>, +held: Bool) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      +cx2 = dirty(o, set_rt(o, cx, X.Rt{agent, M.RIdle{}, False{}, None{}}))
      +r = rr(o, cx2)
      +text = kv_get(X.R.last(r), agent)
      +cx3 = set_r(o, cx2, X.R.with_last(r, kv_del(X.R.last(r), agent)))
      +qs = skip_steered(M.unanswered_for(st(o, cx3), agent), leftover || unread || held, steered)
      +body = Bool.pick(String, M.trim_is_empty(text), "(no written reply at the end of the turn)", text)
      +cx4 = pick_lazy(X.Cx<o>, held, u => cx3, u => auto_replies(o, qs, cx3, agent, body))
      # RFC 0001 §7.2: the automatic report (board only)
      +cx5 = idle.report(o, Bool.not(M.Agent.is_main(a)) && Bool.not(M.trim_is_empty(text)) && Bool.not(held), cx4, agent, text)
      +cx6 = idle.nudge(o, unread && List.is_empty(&2, M.Msg, M.queued_for(st(o, cx5), agent)), cx5, agent)
      pump(o, cx6, agent)

def repl_idle(-o: M.St, +cx: X.Cx<o>, +agent: String, +leftover: Bool, +held: Bool) -> X.Cx<o>:
  +cx1 = fail_waiters(o, cx, agent, "turn_ended")
  +r = rr(o, cx1)
  +steered = steer_get(X.R.steered(r), agent)
  +c = cnt_get(X.R.counts(r), agent)
  +cx2 = set_r(o, cx1, X.R.with_counts(X.R.with_steered(r, steer_del(X.R.steered(r), agent)), cnt_del(X.R.counts(r), agent)))
  # steering committed after the last model call: the model never read
  # it; a new turn makes it answer
  +unread = Bool.not(leftover) && Nat.is_gt(X.Cnt.recv(c), X.Cnt.inj(c)) && Bool.not(List.is_empty(&2, Nat, steered))
  # leftover: the runtime never read them: they go again, as a new turn
  +cx3 = pick_lazy(X.Cx<o>, leftover, u => requeue(o, steered, cx2), u => cx2)
  idle.agent(o, M.get_agent(st(o, cx3), agent), cx3, agent, leftover, unread, steered, held)

# ---- waits (RFC 0003 §6.2) ----

def in_tag(+t: String) -> In:
  Bool.pick(In, String.eq(t, "req"), IReq{},
  Bool.pick(In, String.eq(t, "tick"), ITick{},
  Bool.pick(In, String.eq(t, "idle"), IIdle{},
  Bool.pick(In, String.eq(t, "turn_started"), ITurnStarted{},
  Bool.pick(In, String.eq(t, "steer_rx"), ISteerRx{},
  Bool.pick(In, String.eq(t, "steered"), ISteered{},
  Bool.pick(In, String.eq(t, "assistant"), IAssistant{},
  Bool.pick(In, String.eq(t, "touch"), ITouch{},
  Bool.pick(In, String.eq(t, "ready"), IReady{},
  Bool.pick(In, String.eq(t, "exited"), IExited{},
  Bool.pick(In, String.eq(t, "boot"), IBoot{},
  Bool.pick(In, String.eq(t, "say"), ISay{},
  Bool.pick(In, String.eq(t, "route"), IRoute{},
  Bool.pick(In, String.eq(t, "new"), INew{},
  Bool.pick(In, String.eq(t, "drop"), IDrop{},
  Bool.pick(In, String.eq(t, "confirm_drop"), IConfirmDrop{},
  Bool.pick(In, String.eq(t, "restore"), IRestore{},
  Bool.pick(In, String.eq(t, "isolate"), IIsolate{},
  Bool.pick(In, String.eq(t, "rename"), IRename{},
  Bool.pick(In, String.eq(t, "answer"), IAnswer{},
  Bool.pick(In, String.eq(t, "interrupt"), IInterrupt{},
  Bool.pick(In, String.eq(t, "passthrough"), IPassthrough{},
  Bool.pick(In, String.eq(t, "focus"), IFocus{},
  Bool.pick(In, String.eq(t, "close"), IClose{},
  # force_run: the tests (core_tests.rs, Hub::force_run), and the hub when it
  # restarts a dead sb-core: the REPL states of the old one (BISE-292)
  Bool.pick(In, String.eq(t, "force_run"), IForceRun{},
  # the approvals gate (approvals-design.md §9): the hub's Rust side
  # judges a gated call; a card opens, and closes when every call it
  # holds is gone (the agents interrupted)
  Bool.pick(In, String.eq(t, "confirm_open"), IConfirmOpen{},
  Bool.pick(In, String.eq(t, "confirm_close"), IConfirmClose{},
  # pr-news (pr-design §6): GitHub's news from the hub's Rust side, a
  # message from `github` to the agent that owns the PR; a card for the
  # user when a check still fails after two tries
  Bool.pick(In, String.eq(t, "forge"), IForge{},
  Bool.pick(In, String.eq(t, "forge_card"), IForgeCard{},
  # the hub's own items about a place (pr-design §6.3): the Rust side
  # opens them and closes them (`card_close`: done, or withdrawn)
  Bool.pick(In, String.eq(t, "card_open"), ICardOpen{},
  Bool.pick(In, String.eq(t, "card_close"), IConfirmClose{},
  # a message of the hub itself (from `bise`) to any agent: the timers of
  # `sb every` (docs/ambient-roadmap.md B) wake their agent with it, and
  # the agents an expired ChatGPT sign-in stopped go on (expired-ux)
  Bool.pick(In, String.eq(t, "hub_msg"), IHubMsg{},
  # the timers of `sb every` (hub/timers.bend): set, stop, run now, name
  Bool.pick(In, String.eq(t, "every_set"), IEverySet{},
  Bool.pick(In, String.eq(t, "every_stop"), IEveryStop{},
  Bool.pick(In, String.eq(t, "every_run"), IEveryRun{},
  Bool.pick(In, String.eq(t, "every_name"), IEveryName{},
  # bise's cross-hub messages (S2, the `cross-hub` section): a project
  # hub receives one (x_in); bise's home hub hears how its delivery went
  # (x_ack, x_fail) and the project's answer (x_reply)
  Bool.pick(In, String.eq(t, "x_in"), IXIn{},
  Bool.pick(In, String.eq(t, "x_ack"), IXAck{},
  Bool.pick(In, String.eq(t, "x_fail"), IXFail{},
  Bool.pick(In, String.eq(t, "x_reply"), IXReply{},
  # the routing hold (S2 step 2, the `routing hold` section): his words
  # held for a project (route_hold), its correction or cancel
  Bool.pick(In, String.eq(t, "route_hold"), IRouteHold{},
  Bool.pick(In, String.eq(t, "route_correct"), IRouteCorrect{},
  Bool.pick(In, String.eq(t, "route_cancel"), IRouteCancel{},
  # S2 step 5: the model's pick for his unclear words
  Bool.pick(In, String.eq(t, "route_pick"), IRoutePick{},
  # S10: follow a task (the window's typed command, bise's sb follow)
  Bool.pick(In, String.eq(t, "follow"), IFollow{},
  INone{})))))))))))))))))))))))))))))))))))))))))))))

# `/close N`: the user files a card without answering it
def close_by_user(-o: M.St, m: Maybe<&2, M.Card>, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      notice(o, cx, String.append("no open card #", ns(id)))
    case Some{+c}:
      note_main(o, close_card(o, cx, id, "closed"), cat(["the user closed card #", ns(id), " (", M.Card.kind(c), " @", M.Card.agent(c), ": \"", M.one_line(M.Card.text(c)), "\") without answering"]))

# the gate's card closes without an answer (its agents were
# interrupted): no card, nothing to do
def confirm_close(-o: M.St, m: Maybe<&2, M.Card>, +cx: X.Cx<o>, +id: Nat, +res: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{_}:
      close_card(o, cx, id, res)

def force_run_of(+r: String) -> M.Run:
  Bool.pick(M.Run, String.eq(r, "starting"), M.RStarting{},
    Bool.pick(M.Run, String.eq(r, "idle"), M.RIdle{},
      Bool.pick(M.Run, String.eq(r, "busy"), M.RBusy{}, M.RDown{})))

def dispatch.go(-o: M.St, tag: In, +cx: X.Cx<o>, +i: J.Json) -> X.Cx<o>:
  match tag:
    case IReq{}:
      agent_req(o, cx, C.get_nat(i, "token"), C.get_s(i, "from"), Maybe.default(&2, J.Json, J.get(i, "req"), J.JNull{}))
    case ITick{}:
      acts_tick(o, timers_tick(o, tick(o, cx)))
    case IIdle{}:
      repl_idle(o, cx, C.get_s(i, "agent"), C.get_b(i, "leftover"), C.get_b(i, "held"))
    case ITurnStarted{}:
      turn_started(o, cx, C.get_s(i, "agent"))
    case ISteerRx{}:
      steer_count(o, cx, C.get_s(i, "agent"), False{})
    case ISteered{}:
      steer_count(o, cx, C.get_s(i, "agent"), True{})
    case IAssistant{}:
      assistant(o, cx, C.get_s(i, "agent"), C.get_s(i, "text"))
    case ITouch{}:
      touch(o, cx, C.get_s(i, "agent"), C.get_s(i, "path"))
    case IReady{}:
      repl_ready(o, cx, C.get_s(i, "agent"))
    case IExited{}:
      repl_exited(o, cx, C.get_s(i, "agent"), C.get_b(i, "crashed"), C.get_s(i, "reason"))
    case IBoot{}:
      xboot(o, boot(o, cx))
    case ISay{}:
      user_input(o, cx, C.get_s(i, "focus"), C.get_s(i, "text"), C.get_b(i, "queued"))
    case IRoute{}:
      pick_lazy(X.Cx<o>, String.eq(C.get_s(i, "target"), M.MAIN()),
        u => user_input(o, cx, M.MAIN(), C.get_s(i, "text"), C.get_b(i, "queued")),
        u => route(o, cx, C.get_s(i, "target"), C.get_s(i, "text"), C.get_s(i, "focus"), C.get_b(i, "queued")))
    case INew{}:
      user_new.res(o, create_task(o, cx, new_task(M.USER(), i)))
    case IDrop{}:
      user_drop.res(o, drop_task(o, cx, C.get_s(i, "name"), C.get_b(i, "force")), C.get_s(i, "name"))
    case IConfirmDrop{}:
      user_confirm.res(o, drop_task(o, cx, C.get_s(i, "name"), True{}), C.get_s(i, "name"))
    case IRestore{}:
      user_restore.res(o, restore_task(o, cx, C.get_s(i, "name")))
    case IIsolate{}:
      user_isolate.res(o, isolate_task(o, cx, C.get_s(i, "name")))
    case IRename{}:
      rename.old(o, M.resolve(st(o, cx), C.get_s(i, "name")), cx, C.get_s(i, "name"), C.get_s(i, "new_name"), C.get_b(i, "valid"))
    case IAnswer{}:
      answer_card(o, cx, C.get_nat(i, "card"), C.get_s(i, "text"))
    case IInterrupt{}:
      interrupt.go(o, M.resolve(st(o, cx), C.get_s(i, "agent")), cx)
    case IPassthrough{}:
      passthrough(o, cx, C.get_s(i, "focus"), C.get_s(i, "line"))
    case IFocus{}:
      focus(o, cx, C.get_s(i, "focus"), C.opt_str(J.get(i, "note")), C.get_s(i, "direct"))
    case IClose{}:
      close_by_user(o, M.card_of(M.St.cards(st(o, cx)), C.get_nat(i, "card")), cx, C.get_nat(i, "card"))
    case IForceRun{}:
      # tests only: put an agent's REPL in a given state (then deliver, like
      # any change of run: law L4)
      pump(o, set_run(o, cx, C.get_s(i, "agent"), force_run_of(C.get_s(i, "run"))), C.get_s(i, "agent"))
    case IConfirmOpen{}:
      open_confirm(o, cx, C.get_s(i, "agent"), C.get_s(i, "text"))
    case IConfirmClose{}:
      confirm_close(o, M.card_of(M.St.cards(st(o, cx)), C.get_nat(i, "card")), cx, C.get_nat(i, "card"), C.get_s(i, "res"))
    case IForge{}:
      fst(o, send(o, cx, msg(M.GITHUB(), C.get_s(i, "to"), C.get_s(i, "text"), False{}, None{}, False{}, False{})))
    case IForgeCard{}:
      open_card(o, cx, "question", C.get_s(i, "agent"), C.get_s(i, "text"), None{})
    case ICardOpen{}:
      hub_card(o, cx, C.get_s(i, "kind"), C.get_s(i, "agent"), C.get_s(i, "text"), C.opt_str(J.get(i, "place")), C.opt_nat(J.get(i, "pr")))
    case IHubMsg{}:
      fst(o, send(o, cx, msg(M.HUB(), C.get_s(i, "to"), C.get_s(i, "text"), False{}, None{}, False{}, False{})))
    case IEverySet{}:
      timer_set(o, cx, i)
    case IEveryStop{}:
      timer_stop(o, cx, i)
    case IEveryRun{}:
      timer_run(o, cx, i)
    case IEveryName{}:
      timer_name(o, cx, i)
    case IXIn{}:
      xin(o, XB.seen(xst(o, cx), C.get_s(i, "hub"), C.get_nat(i, "xid")), cx, C.get_nat(i, "token"), C.get_s(i, "hub"), C.get_nat(i, "xid"), XH.kind_of(C.get_s(i, "kind")), C.get_s(i, "text"), C.get_s(i, "name"))
    case IXAck{}:
      xack(o, XB.pending(xst(o, cx), C.get_nat(i, "xid")), cx, C.get_nat(i, "xid"))
    case IXFail{}:
      xfail(o, XB.pending(xst(o, cx), C.get_nat(i, "xid")), cx, C.get_nat(i, "xid"))
    case IXReply{}:
      xanswer(o, XB.open(xst(o, cx), C.get_nat(i, "xid")), cx, C.get_nat(i, "xid"), C.get_s(i, "name"), C.get_s(i, "text"))
    case IRouteHold{}:
      route_hold(o, cx, C.get_s(i, "text"), C.get_s(i, "via"), C.get_s(i, "to"), C.get_s(i, "name"), C.get_s(i, "why"), C.get_s(i, "context"))
    case IRouteCorrect{}:
      route_correct(o, cx, C.get_nat(i, "rid"), C.get_s(i, "to"), C.get_s(i, "name"))
    case IRouteCancel{}:
      route_cancel(o, cx, C.get_nat(i, "rid"))
    case IRoutePick{}:
      route_pick(o, cx, C.get_nat(i, "rid"), C.get_s(i, "to"), C.get_s(i, "name"), C.get_s(i, "why"))
    case IFollow{}:
      follow_in(o, follow_err(st(o, cx), C.get_s(i, "agent")), cx, C.get_s(i, "agent"), C.get_b(i, "on"))
    case INone{}:
      cx

def dispatch(-o: M.St, +cx: X.Cx<o>, +i: J.Json) -> X.Cx<o>:
  dispatch.go(o, in_tag(C.get_s(i, "t")), cx, i)

# one input: the context after it (its state is the replay of its
# journal over the hub's state: law replay_reproduces)
def step(+h: Hub, +i: J.Json) -> X.Cx<Hub.d(h)>:
  dispatch(Hub.d(h), start(h, i), i)

# ---- committing a step (what sb-core keeps) ----

def commit.go(-o: M.St, need: Maybe<&2, J.Json>, +c: X.Core<o>, +r: X.R, +h: Hub) -> Out:
  match need:
    case Some{+q}:
      Out{h, Nil{}, Some{q}}
    case None{}:
      Out{Hub{X.Core.d(o, c), X.R.with_ans(r, Nil{})}, X.Core.fx(o, c), None{}}

def commit.cx(-o: M.St, cx: X.Cx<o>, +h: Hub) -> Out:
  match cx:
    case X.Cx{+c, +r}:
      commit.go(o, X.R.need(r), c, r, h)

def commit(+h: Hub, +i: J.Json) -> Out:
  commit.cx(Hub.d(h), step(h, i), h)

def RunR.hub(x: RunR) -> Hub:
  match x:
    case RunR{+hub, _}:
      hub

def Out.fx(x: Out) -> List<&2, X.Fx>:
  match x:
    case Out{_, +fx, _}:
      fx

def RunR.journal(x: RunR) -> List<&2, M.Ev>:
  match x:
    case RunR{_, +journal}:
      journal

def run.cat(+o1: Out, +rr: RunR) -> RunR:
  RunR{RunR.hub(rr), List.append(&2, M.Ev, X.journal(Out.fx(o1)), RunR.journal(rr))}

def Out.hub(x: Out) -> Hub:
  match x:
    case Out{+hub, _, _}:
      hub

def run(ins: List<&2, J.Json>, +h: Hub) -> RunR:
  match ins:
    case Nil{}:
      RunR{h, Nil{}}
    case Con{+i, +rest}:
      run.cat(commit(h, i), run(rest, Out.hub(commit(h, i))))

def Out.need(x: Out) -> Maybe<&2, J.Json>:
  match x:
    case Out{_, _, +need}:
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

def waiters_of_cx(-o: M.St, +cx: X.Cx<o>) -> List<&2, X.Waiter>:
  X.R.waiters(rr(o, cx))
