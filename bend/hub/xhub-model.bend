# hub/xhub-model.bend — the hub's cross-hub message state (bise desktop S2):
# the home hub's outbox to the project hubs and a project hub's record of
# what it received from the home hub. A field of model.bend's St (`x`),
# owned by sb-core and rebuilt from the journal like the rest.
#
# Empty for now: S2's step 1 fills it (outbox, inbox) with its journal
# kinds and laws. Imported by model.bend, so it imports neither model nor
# core (Bend has no import cycles), like timers.bend.
import Base

import ../vendor/json.bend as J

# his own words forwarded by reference (his authority), or bise's own
# question (none)
type XKind is Data:
  KInput{}
  KAsk{}
  # J: a task this project's hub follows for bise ended (project -> bise's
  # home hub): `text` is its JSON {agent, key, state, label, summary, name}
  KJobEnd{}

type OutSt is Data:
  OPending{}
  OAcked{}
  OSettled{}

# `msg`: the home hub's message it forwards (kind input) or 0; `text`: the
# words sent (copied by sb-core from that message, or the question)
type Out is Data:
  Out{xid: Nat, project: String, kind: XKind, msg: Nat, text: String, st: OutSt, tries: Nat, context: String}

# `msg`: the message the project hub made of it (its id there)
type XIn is Data:
  XIn{hub: String, xid: Nat, msg: Nat, kind: XKind, replied: Bool}

type RSt is Data:
  RHeld{}
  RSent{}
  RCancelled{}

# his words on their way to a project (step 2's hold): held until `due`
# (its act `act` on timers.bend's clock), then sent or cancelled. `to`:
# the project's hub id, `name` its name (the note and the event), `via`
# the view he typed in ("" none)
type Route is Data:
  Route{rid: Nat, text: String, via: String, to: String, name: String, due: Nat, act: Nat, why: String, st: RSt, context: String}

type Xs is Data:
  Xs{outbox: List<&2, Out>, next_xid: Nat, xin: List<&2, XIn>, routes: List<&2, Route>, next_rid: Nat}

def empty() -> Xs:
  Xs{Nil{}, 1n, Nil{}, Nil{}, 1n}

# ---- fields ----

def Out.xid(x: Out) -> Nat:
  match x:
    case Out{+xid, _, _, _, _, _, _, _}:
      xid

def Out.project(x: Out) -> String:
  match x:
    case Out{_, +p, _, _, _, _, _, _}:
      p

def Out.kind(x: Out) -> XKind:
  match x:
    case Out{_, _, +k, _, _, _, _, _}:
      k

def Out.msg(x: Out) -> Nat:
  match x:
    case Out{_, _, _, +m, _, _, _, _}:
      m

def Out.text(x: Out) -> String:
  match x:
    case Out{_, _, _, _, +t, _, _, _}:
      t

def Out.st(x: Out) -> OutSt:
  match x:
    case Out{_, _, _, _, _, +s, _, _}:
      s

def Out.tries(x: Out) -> Nat:
  match x:
    case Out{_, _, _, _, _, _, +n, _}:
      n

# the context of his routed words (Route.context), "" for the rest
def Out.context(x: Out) -> String:
  match x:
    case Out{_, _, _, _, _, _, _, +v}:
      v

def XIn.hub(x: XIn) -> String:
  match x:
    case XIn{+h, _, _, _, _}:
      h

def XIn.xid(x: XIn) -> Nat:
  match x:
    case XIn{_, +xid, _, _, _}:
      xid

def XIn.msg(x: XIn) -> Nat:
  match x:
    case XIn{_, _, +m, _, _}:
      m

def XIn.kind(x: XIn) -> XKind:
  match x:
    case XIn{_, _, _, +k, _}:
      k

def XIn.replied(x: XIn) -> Bool:
  match x:
    case XIn{_, _, _, _, +r}:
      r

def Xs.outbox(x: Xs) -> List<&2, Out>:
  match x:
    case Xs{+o, _, _, _, _}:
      o

def Xs.next_xid(x: Xs) -> Nat:
  match x:
    case Xs{_, +n, _, _, _}:
      n

def Xs.xin(x: Xs) -> List<&2, XIn>:
  match x:
    case Xs{_, _, +i, _, _}:
      i

def Xs.routes(x: Xs) -> List<&2, Route>:
  match x:
    case Xs{_, _, _, +r, _}:
      r

def Xs.next_rid(x: Xs) -> Nat:
  match x:
    case Xs{_, _, _, _, +n}:
      n

def Route.rid(x: Route) -> Nat:
  match x:
    case Route{+v, _, _, _, _, _, _, _, _, _}:
      v

def Route.text(x: Route) -> String:
  match x:
    case Route{_, +v, _, _, _, _, _, _, _, _}:
      v

def Route.via(x: Route) -> String:
  match x:
    case Route{_, _, +v, _, _, _, _, _, _, _}:
      v

def Route.to(x: Route) -> String:
  match x:
    case Route{_, _, _, +v, _, _, _, _, _, _}:
      v

def Route.name(x: Route) -> String:
  match x:
    case Route{_, _, _, _, +v, _, _, _, _, _}:
      v

def Route.due(x: Route) -> Nat:
  match x:
    case Route{_, _, _, _, _, +v, _, _, _, _}:
      v

def Route.act(x: Route) -> Nat:
  match x:
    case Route{_, _, _, _, _, _, +v, _, _, _}:
      v

def Route.why(x: Route) -> String:
  match x:
    case Route{_, _, _, _, _, _, _, +v, _, _}:
      v

def Route.st(x: Route) -> RSt:
  match x:
    case Route{_, _, _, _, _, _, _, _, +v, _}:
      v

# his fn context (FnContext JSON text, "" none): carried to the project,
# rendered there like a direct input's (fn_context::with_context), never
# on the home hub
def Route.context(x: Route) -> String:
  match x:
    case Route{_, _, _, _, _, _, _, _, _, +v}:
      v

def rst_str(s: RSt) -> String:
  match s:
    case RHeld{}:
      "held"
    case RSent{}:
      "sent"
    case RCancelled{}:
      "cancelled"

def rst_of(+s: String) -> RSt:
  Bool.pick(RSt, String.eq(s, "sent"), RSent{}, Bool.pick(RSt, String.eq(s, "cancelled"), RCancelled{}, RHeld{}))

def route_of(xs: List<&2, Route>, +rid: Nat) -> Maybe<&2, Route>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, Route>, Nat.is_eq(Route.rid(h), rid), Some{h}, route_of(t, rid))

def Route.with_to(x: Route, +to: String, +name: String) -> Route:
  match x:
    case Route{+r, +tx, +v, _, _, +d, +a, +w, +s, +c}:
      Route{r, tx, v, to, name, d, a, w, s, c}

def Route.with_st(x: Route, +s: RSt) -> Route:
  match x:
    case Route{+r, +tx, +v, +to, +n, +d, +a, +w, _, +c}:
      Route{r, tx, v, to, n, d, a, w, s, c}

type RouteOp is Data:
  RTo{to: String, name: String}
  REnd{st: RSt}

def route_op(op: RouteOp, +r: Route) -> Route:
  match op:
    case RTo{+to, +name}:
      Route.with_to(r, to, name)
    case REnd{+s}:
      Route.with_st(r, s)

def route_set(xs: List<&2, Route>, +rid: Nat, +op: RouteOp) -> List<&2, Route>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, Route>, Nat.is_eq(Route.rid(h), rid), route_op(op, h) <> t, h <> route_set(t, rid, op))

def kind_str(k: XKind) -> String:
  match k:
    case KInput{}:
      "input"
    case KAsk{}:
      "ask"
    case KJobEnd{}:
      "job_end"

def kind_of(+s: String) -> XKind:
  Bool.pick(XKind, String.eq(s, "ask"), KAsk{}, Bool.pick(XKind, String.eq(s, "job_end"), KJobEnd{}, KInput{}))

def st_str(s: OutSt) -> String:
  match s:
    case OPending{}:
      "pending"
    case OAcked{}:
      "acked"
    case OSettled{}:
      "settled"

def is_settled(s: OutSt) -> Bool:
  match s:
    case OSettled{}:
      True{}
    case _:
      False{}

# ---- lookups ----

def out_of(xs: List<&2, Out>, +xid: Nat) -> Maybe<&2, Out>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, Out>, Nat.is_eq(Out.xid(h), xid), Some{h}, out_of(t, xid))

def xin_of(xs: List<&2, XIn>, +hub: String, +xid: Nat) -> Maybe<&2, XIn>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, XIn>, String.eq(XIn.hub(h), hub) && Nat.is_eq(XIn.xid(h), xid), Some{h}, xin_of(t, hub, xid))

# the inbox record whose message is `msg` (its answer goes back)
def xin_by_msg(xs: List<&2, XIn>, +msg: Nat) -> Maybe<&2, XIn>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, XIn>, Nat.is_eq(XIn.msg(h), msg), Some{h}, xin_by_msg(t, msg))

# the forward of his message `msg` (one per message)
def out_by_msg(xs: List<&2, Out>, +msg: Nat) -> Maybe<&2, Out>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, Out>, Nat.is_eq(Out.msg(h), msg), Some{h}, out_by_msg(t, msg))

# ---- the journal's changes ----

def Out.with_st(x: Out, +s: OutSt) -> Out:
  match x:
    case Out{+xid, +p, +k, +m, +t, _, +n, +c}:
      Out{xid, p, k, m, t, s, n, c}

# acked only from pending (a late ack never reopens a settled send)
def acked_st(s: OutSt) -> OutSt:
  match s:
    case OPending{}:
      OAcked{}
    case other:
      other

def Out.acked(x: Out) -> Out:
  match x:
    case Out{+xid, +p, +k, +m, +t, +s, +n, +c}:
      Out{xid, p, k, m, t, acked_st(s), n, c}

def Out.tried(x: Out) -> Out:
  match x:
    case Out{+xid, +p, +k, +m, +t, +s, +n, +c}:
      Out{xid, p, k, m, t, s, 1n+n, c}

def Out.settled(x: Out) -> Out:
  Out.with_st(x, OSettled{})

type OutOp is Data:
  OpAcked{}
  OpTried{}
  OpSettled{}

def out_op(op: OutOp, +x: Out) -> Out:
  match op:
    case OpAcked{}:
      Out.acked(x)
    case OpTried{}:
      Out.tried(x)
    case OpSettled{}:
      Out.settled(x)

def out_set(xs: List<&2, Out>, +xid: Nat, +op: OutOp) -> List<&2, Out>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, Out>, Nat.is_eq(Out.xid(h), xid), out_op(op, h) <> t, h <> out_set(t, xid, op))

def XIn.done(x: XIn) -> XIn:
  match x:
    case XIn{+h, +xid, +m, +k, _}:
      XIn{h, xid, m, k, True{}}

def xin_set(xs: List<&2, XIn>, +hub: String, +xid: Nat) -> List<&2, XIn>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, XIn>, String.eq(XIn.hub(h), hub) && Nat.is_eq(XIn.xid(h), xid), XIn.done(h) <> t, h <> xin_set(t, hub, xid))

def x_out(+x: Xs, +o: Out) -> Xs:
  match x:
    case Xs{+ob, +n, +xi, +rs, +nr}:
      Xs{List.append(&2, Out, ob, [o]), Nat.max(n, 1n+Out.xid(o)), xi, rs, nr}

def x_op(+x: Xs, +xid: Nat, +op: OutOp) -> Xs:
  match x:
    case Xs{+ob, +n, +xi, +rs, +nr}:
      Xs{out_set(ob, xid, op), n, xi, rs, nr}

def x_in(+x: Xs, +i: XIn) -> Xs:
  match x:
    case Xs{+ob, +n, +xi, +rs, +nr}:
      Xs{ob, n, List.append(&2, XIn, xi, [i]), rs, nr}

def x_replied(+x: Xs, +hub: String, +xid: Nat) -> Xs:
  match x:
    case Xs{+ob, +n, +xi, +rs, +nr}:
      Xs{ob, n, xin_set(xi, hub, xid), rs, nr}

def x_route_held(+x: Xs, +r: Route) -> Xs:
  match x:
    case Xs{+ob, +n, +xi, +rs, +nr}:
      Xs{ob, n, xi, List.append(&2, Route, rs, [r]), Nat.max(nr, 1n+Route.rid(r))}

def x_route(+x: Xs, +rid: Nat, +op: RouteOp) -> Xs:
  match x:
    case Xs{+ob, +n, +xi, +rs, +nr}:
      Xs{ob, n, xi, route_set(rs, rid, op), nr}

# ---- JSON (the journal lines' parts, the snapshot) ----

def out_json(+o: Out) -> J.Json:
  J.obj([J.kv("xid", J.JNum{Out.xid(o)}), J.kv("project", J.JStr{Out.project(o)}),
    J.kv("kind", J.JStr{kind_str(Out.kind(o))}), J.kv("msg", J.JNum{Out.msg(o)}),
    J.kv("text", J.JStr{Out.text(o)}), J.kv("state", J.JStr{st_str(Out.st(o))}), J.kv("tries", J.JNum{Out.tries(o)}),
    J.kv("context", J.JStr{Out.context(o)})])

def xin_json(+i: XIn) -> J.Json:
  J.obj([J.kv("hub", J.JStr{XIn.hub(i)}), J.kv("xid", J.JNum{XIn.xid(i)}), J.kv("msg", J.JNum{XIn.msg(i)}),
    J.kv("kind", J.JStr{kind_str(XIn.kind(i))}), J.kv("replied", J.JBool{XIn.replied(i)})])

def route_json(+r: Route) -> J.Json:
  J.obj([J.kv("rid", J.JNum{Route.rid(r)}), J.kv("text", J.JStr{Route.text(r)}), J.kv("via", J.JStr{Route.via(r)}),
    J.kv("to", J.JStr{Route.to(r)}), J.kv("name", J.JStr{Route.name(r)}), J.kv("due_ms", J.JNum{Route.due(r)}),
    J.kv("act", J.JNum{Route.act(r)}), J.kv("why", J.JStr{Route.why(r)}), J.kv("state", J.JStr{rst_str(Route.st(r))}),
    J.kv("context", J.JStr{Route.context(r)})])

# ---- the journal's kinds of this state (model.bend's Ev EX) ----

type XEv is Data:
  XOut{o: Out}
  XOp{xid: Nat, op: OutOp}
  XGot{i: XIn}
  XReplied{hub: String, xid: Nat}
  XRouteHeld{r: Route}
  XRoute{rid: Nat, op: RouteOp}

def apply(+x: Xs, e: XEv) -> Xs:
  match e:
    case XOut{+o}:
      x_out(x, o)
    case XOp{+xid, +op}:
      x_op(x, xid, op)
    case XGot{+i}:
      x_in(x, i)
    case XReplied{+h, +xid}:
      x_replied(x, h, xid)
    case XRouteHeld{+r}:
      x_route_held(x, r)
    case XRoute{+rid, +op}:
      x_route(x, rid, op)
