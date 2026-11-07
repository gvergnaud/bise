# bend/proofs/lib/hub-clean.bend: no agent, live or archived, holds a
# never_agent name (HI.NoReserved, law no_reserved_names,
# c4b-core-step-flat): every event keeps it, a task_created's and a
# rename's name by FreshEv (never name_held, which holds every reserved
# name). The same shape as hub-agents.bend's split (lv_*).
import Base
import ../../vendor/json.bend as J
import ../../hub/model.bend as HM
import ../../hub/gen/state.bend as HS
import ../../hub/gen/state-laws.bend as HSL
import ../../hub/cx.bend as HX
import ../../hub/inv.bend as HI
import ../../hub/names.bend as HN
import ../../hub/agents.bend as HAG
import ../../hub/lazy.bend as HLZ
import ./hub-1.bend as PL1
import ./absurd.bend as AB
import ./hub-2.bend as PL2
import ./hub-names.bend as PLN
import ./hub-agents.bend as PLA

# ---- never_in on a list of names ----

def ni_true(+h: String, +t: List<&2, String>, ec: {HM.never_agent(h) == True{} : Bool}) -> {True{} == HLZ.or(HM.never_agent(h), u => HM.never_in(t)) : Bool}:
  %Equal.sym(Bool, HM.never_agent(h), True{}, ec) : {True{} == HLZ.or(_, u => HM.never_in(t)) : Bool}
  {==}

def ni_false(+h: String, +t: List<&2, String>, ec: {HM.never_agent(h) == False{} : Bool}) -> {HM.never_in(t) == HLZ.or(HM.never_agent(h), u => HM.never_in(t)) : Bool}:
  %Equal.sym(Bool, HM.never_agent(h), False{}, ec) : {HM.never_in(t) == HLZ.or(_, u => HM.never_in(t)) : Bool}
  {==}

def ni_head.of(c: Bool, +h: String, +t: List<&2, String>, +ec: {HM.never_agent(h) == c : Bool}, e: {HM.never_in(h <> t) == False{} : Bool})
    -> {HM.never_agent(h) == False{} : Bool}:
  match c:
    case True{}:
      PLA.ag_new_absurd({HM.never_agent(h) == False{} : Bool}, Equal.trans(Bool, True{}, HM.never_in(h <> t), False{}, ni_true(h, t, ec), e))
    case False{}:
      ec

# a clean list's head is clean...
def ni_head(+h: String, +t: List<&2, String>, e: {HM.never_in(h <> t) == False{} : Bool}) -> {HM.never_agent(h) == False{} : Bool}:
  ni_head.of(HM.never_agent(h), h, t, {==}, e)

def ni_tail.of(c: Bool, +h: String, +t: List<&2, String>, +ec: {HM.never_agent(h) == c : Bool}, e: {HM.never_in(h <> t) == False{} : Bool})
    -> {HM.never_in(t) == False{} : Bool}:
  match c:
    case True{}:
      PLA.ag_new_absurd({HM.never_in(t) == False{} : Bool}, Equal.trans(Bool, True{}, HM.never_in(h <> t), False{}, ni_true(h, t, ec), e))
    case False{}:
      Equal.trans(Bool, HM.never_in(t), HM.never_in(h <> t), False{}, ni_false(h, t, ec), e)

# ...and so is its tail
def ni_tail(+h: String, +t: List<&2, String>, e: {HM.never_in(h <> t) == False{} : Bool}) -> {HM.never_in(t) == False{} : Bool}:
  ni_tail.of(HM.never_agent(h), h, t, {==}, e)

def ni_cons(+h: String, +t: List<&2, String>, eh: {HM.never_agent(h) == False{} : Bool}, et: {HM.never_in(t) == False{} : Bool}) -> {HM.never_in(h <> t) == False{} : Bool}:
  %Equal.sym(Bool, HM.never_agent(h), False{}, eh) : {HLZ.or(_, u => HM.never_in(t)) == False{} : Bool}
  et

def ni_drop.keep(d: Bool, +h: String, +rest: List<&2, String>, eh: {HM.never_agent(h) == False{} : Bool}, er: {HM.never_in(rest) == False{} : Bool})
    -> {HM.never_in(HN.drop_str.keep(d, h, rest)) == False{} : Bool}:
  match d:
    case True{}:
      er
    case False{}:
      ni_cons(h, rest, eh, er)

# an old name taken out keeps a list clean
def ni_drop(+xs: List<&2, String>, +n: String, +e: {HM.never_in(xs) == False{} : Bool}) -> {HM.never_in(HN.drop_str(xs, n)) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      ni_drop.keep(String.eq(h, n), h, HN.drop_str(t, n), ni_head(h, t, e), ni_drop(t, n, ni_tail(h, t, e)))

# a clean name added at the end keeps it clean
def ni_snoc(+xs: List<&2, String>, +x: String, +e: {HM.never_in(xs) == False{} : Bool}, +ex: {HM.never_agent(x) == False{} : Bool})
    -> {HM.never_in(HM.snoc(String, xs, x)) == False{} : Bool}:
  match xs:
    case Nil{}:
      ni_cons(x, Nil{}, ex, {==})
    case Con{h, t}:
      ni_cons(h, HM.snoc(String, t, x), ni_head(h, t, e), ni_snoc(t, x, ni_tail(h, t, e), ex))

# ---- the agents ----

def cl_eq(-x: List<&2, HS.Agent>, -y: List<&2, HS.Agent>, e: {x == y : List<&2, HS.Agent>}, d: HI.Clean(y)) -> HI.Clean(x):
  %Equal.sym(List<&2, HS.Agent>, x, y, e) : HI.Clean(_)
  d

def cl_split.add(-a: HS.Agent, -t: List<&2, HS.Agent>, -r: List<&2, HS.Agent>, a1: {HM.never_in(HI.names(a)) == False{} : Bool}, b: HX.Both<HI.Clean(t), HI.Clean(r)>)
    -> HX.Both<HI.Clean(a <> t), HI.Clean(r)>:
  match b:
    case HX.Both{b1, b2}:
      HX.Both{HX.Both{a1, b1}, b2}

def cl_split(+q: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, l: HI.Clean(List.append(&2, HS.Agent, q, r))) -> HX.Both<HI.Clean(q), HI.Clean(r)>:
  match q:
    case Nil{}:
      HX.Both{Unit{}, l}
    case Con{a, t}:
      match l:
        case HX.Both{l1, l2}:
          cl_split.add(a, t, r, l1, cl_split(t, r, l2))

def cl_join(+q: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, a: HI.Clean(q), b: HI.Clean(r)) -> HI.Clean(List.append(&2, HS.Agent, q, r)):
  match q:
    case Nil{}:
      b
    case Con{x, t}:
      match a:
        case HX.Both{a1, a2}:
          HX.Both{a1, cl_join(t, r, a2, b)}

# an operation that is not a rename keeps an agent's names
def cl_op(+a: HS.Agent, +op: HM.AgentOp, +nr: {PL2.hub_is_rename(op) == False{} : Bool}, l1: {HM.never_in(HI.names(a)) == False{} : Bool})
    -> {HM.never_in(HI.names(HM.agent_op(a, op))) == False{} : Bool}:
  %Equal.sym(List<&2, String>, HI.names(HM.agent_op(a, op)), HI.names(a), PL2.hub_names_op(a, op, nr)) : {HM.never_in(_) == False{} : Bool}
  l1

# a rename to a clean name: the old name joins the old names
def cl_ren_one(+a: HS.Agent, +nn: String, kn: {HM.never_agent(nn) == False{} : Bool}, +l1: {HM.never_in(HI.names(a)) == False{} : Bool})
    -> {HM.never_in(HI.names(HM.agent_op(a, HM.ORename{nn}))) == False{} : Bool}:
  %HSL.Agent.name.eta(a, nn) : {HM.never_in(HI.names(HS.Agent.with_aliases(_, HM.snoc(String, HS.Agent.aliases(a), HS.Agent.name(a))))) == False{} : Bool}
  ni_cons(nn, HM.snoc(String, HS.Agent.aliases(a), HS.Agent.name(a)), kn,
    ni_snoc(HS.Agent.aliases(a), HS.Agent.name(a), ni_tail(HS.Agent.name(a), HS.Agent.aliases(a), l1), ni_head(HS.Agent.name(a), HS.Agent.aliases(a), l1)))

def cl_map.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, +nr: {PL2.hub_is_rename(op) == False{} : Bool},
    l1: {HM.never_in(HI.names(h)) == False{} : Bool}, l2: HI.Clean(t), ih: HI.Clean(HM.map_agent(t, n, op))) -> HI.Clean(HM.map_agent.go(h <> t, c, n, op)):
  match c:
    case True{}:
      HX.Both{cl_op(h, op, nr, l1), l2}
    case False{}:
      HX.Both{l1, ih}

def cl_map(+xs: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, +nr: {PL2.hub_is_rename(op) == False{} : Bool}, l: HI.Clean(xs)) -> HI.Clean(HM.map_agent(xs, n, op)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match l:
        case HX.Both{l1, +l2}:
          cl_map.pick(String.eq(HS.Agent.name(h), n), h, t, n, op, nr, l1, l2, cl_map(t, n, op, nr, l2))

def cl_map_ren.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String, +nn: String, +kn: {HM.never_agent(nn) == False{} : Bool},
    l1: {HM.never_in(HI.names(h)) == False{} : Bool}, l2: HI.Clean(t), ih: HI.Clean(HM.map_agent(t, n, HM.ORename{nn}))) -> HI.Clean(HM.map_agent.go(h <> t, c, n, HM.ORename{nn})):
  match c:
    case True{}:
      HX.Both{cl_ren_one(h, nn, kn, l1), l2}
    case False{}:
      HX.Both{l1, ih}

def cl_map_ren(+xs: List<&2, HS.Agent>, +n: String, +nn: String, +kn: {HM.never_agent(nn) == False{} : Bool}, l: HI.Clean(xs)) -> HI.Clean(HM.map_agent(xs, n, HM.ORename{nn})):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match l:
        case HX.Both{l1, +l2}:
          cl_map_ren.pick(String.eq(HS.Agent.name(h), n), h, t, n, nn, kn, l1, l2, cl_map_ren(t, n, nn, kn, l2))

def cl_strip(+xs: List<&2, HS.Agent>, +n: String, l: HI.Clean(xs)) -> HI.Clean(HM.strip(xs, n)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match l:
        case HX.Both{+l1, l2}:
          %HSL.Agent.aliases.eta(h, HN.drop_str(HS.Agent.aliases(h), n)) : HX.Both<{HM.never_in(HI.names(_)) == False{} : Bool}, HI.Clean(HM.strip(t, n))>
          HX.Both{ni_cons(HS.Agent.name(h), HN.drop_str(HS.Agent.aliases(h), n), ni_head(HS.Agent.name(h), HS.Agent.aliases(h), l1),
            ni_drop(HS.Agent.aliases(h), n, ni_tail(HS.Agent.name(h), HS.Agent.aliases(h), l1))), cl_strip(t, n, l2)}

def cl_put.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String, +a: HS.Agent, la: {HM.never_in(HI.names(a)) == False{} : Bool},
    l1: {HM.never_in(HI.names(h)) == False{} : Bool}, l2: HI.Clean(t), ih: HI.Clean(HM.put_agent(t, n, a)))
    -> HI.Clean(HLZ.pick(List<&2, HS.Agent>, c, u => a <> t, u => h <> HM.put_agent(t, n, a))):
  match c:
    case True{}:
      HX.Both{la, l2}
    case False{}:
      HX.Both{l1, ih}

def cl_put(+xs: List<&2, HS.Agent>, +n: String, +a: HS.Agent, +la: {HM.never_in(HI.names(a)) == False{} : Bool}, l: HI.Clean(xs)) -> HI.Clean(HM.put_agent(xs, n, a)):
  match xs:
    case Nil{}:
      HX.Both{la, Unit{}}
    case Con{h, t}:
      match l:
        case HX.Both{l1, +l2}:
          cl_put.pick(String.eq(HS.Agent.name(h), n), h, t, n, a, la, l1, l2, cl_put(t, n, a, la, l2))

def cl_ins.go(-xs: List<&2, HS.Agent>, +t: List<&2, HS.Agent>, +d: List<&2, HS.Agent>, +a: HS.Agent, la: {HM.never_in(HI.names(a)) == False{} : Bool}, b: HX.Both<HI.Clean(t), HI.Clean(d)>)
    -> HI.Clean(List.append(&2, HS.Agent, t, a <> d)):
  match b:
    case HX.Both{b1, b2}:
      cl_join(t, a <> d, b1, HX.Both{la, b2})

def cl_ins(+ord: List<&2, String>, +xs: List<&2, HS.Agent>, +a: HS.Agent, la: {HM.never_in(HI.names(a)) == False{} : Bool}, l: HI.Clean(xs)) -> HI.Clean(HAG.ins(ord, xs, a)):
  cl_ins.go(xs, List.take(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), List.drop(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), a, la,
    cl_split(List.take(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), List.drop(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), cl_eq(List.append(&2, HS.Agent, List.take(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), List.drop(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a)))), xs, PLA.ag_take_drop(HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), l)))

def cl_drop_of.m(m: Maybe<&2, HS.Agent>, +p: List<&2, HS.Agent>, +q: List<&2, HS.Agent>, b: HX.Both<HI.Clean(p), HI.Clean(HAG.whole.of(m, q))>) -> HI.Clean(List.append(&2, HS.Agent, p, q)):
  match m:
    case None{}:
      match b:
        case HX.Both{b1, b2}:
          cl_join(p, q, b1, b2)
    case Some{x}:
      match b:
        case HX.Both{b1, b2}:
          match b2:
            case HX.Both{c1, c2}:
              cl_join(p, q, b1, c2)

def cl_drop(+xs: List<&2, HS.Agent>, +n: String, l: HI.Clean(xs)) -> HI.Clean(HAG.drop(xs, n)):
  cl_drop_of.m(HAG.hit(xs, n), HAG.pre(xs, n), HAG.post(xs, n),
    cl_split(HAG.pre(xs, n), HAG.whole.of(HAG.hit(xs, n), HAG.post(xs, n)), cl_eq(HAG.whole(HAG.pre(xs, n), HAG.hit(xs, n), HAG.post(xs, n)), xs, PLA.ag_cut_whole(xs, n), l)))

# ---- each event keeps them clean ----

def cl_on(c: Bool, +a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, +nr: {PL2.hub_is_rename(op) == False{} : Bool}, la: HI.Clean(a), lr: HI.Clean(r))
    -> HX.Both<HI.Clean(HM.on_agent.go(c, True{}, a, r, n, op)), HI.Clean(HM.on_agent.go(c, False{}, a, r, n, op))>:
  match c:
    case True{}:
      HX.Both{cl_map(a, n, op, nr, la), lr}
    case False{}:
      HX.Both{la, cl_map(r, n, op, nr, lr)}

def cl_on_ren(c: Bool, +a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +nn: String, +kn: {HM.never_agent(nn) == False{} : Bool}, la: HI.Clean(a), lr: HI.Clean(r))
    -> HX.Both<HI.Clean(HM.on_agent.go(c, True{}, a, r, n, HM.ORename{nn})), HI.Clean(HM.on_agent.go(c, False{}, a, r, n, HM.ORename{nn}))>:
  match c:
    case True{}:
      HX.Both{cl_map_ren(a, n, nn, kn, la), lr}
    case False{}:
      HX.Both{la, cl_map_ren(r, n, nn, kn, lr)}

def cl_placed(ok: Bool, +a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +ws: J.Json, la: HI.Clean(a), lr: HI.Clean(r))
    -> HX.Both<HI.Clean(HM.placed.go(ok, True{}, a, r, n, ws)), HI.Clean(HM.placed.go(ok, False{}, a, r, n, ws))>:
  match ok:
    case True{}:
      cl_on(HAG.has(a, n), a, r, n, HM.OWs{ws}, {==}, la, lr)
    case False{}:
      HX.Both{la, lr}

def cl_live_to(stay: Bool, +a2: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, hl: {HM.never_in(HI.names(a2)) == False{} : Bool},
    lp: HI.Clean(pre), lq: HI.Clean(post), lr: HI.Clean(r)) -> HX.Both<HI.Clean(HM.life.live.to(stay, True{}, a2, pre, post, r)), HI.Clean(HM.life.live.to(stay, False{}, a2, pre, post, r))>:
  match stay:
    case True{}:
      HX.Both{cl_join(pre, a2 <> post, lp, HX.Both{hl, lq}), lr}
    case False{}:
      HX.Both{cl_join(pre, post, lp, lq), HX.Both{hl, lr}}

def cl_arch_to(back: Bool, +a2: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +xs: List<&2, HS.Agent>, +ord: List<&2, String>, hl: {HM.never_in(HI.names(a2)) == False{} : Bool},
    lp: HI.Clean(pre), lq: HI.Clean(post), la: HI.Clean(xs)) -> HX.Both<HI.Clean(HM.life.arch.to(back, True{}, a2, pre, post, xs, ord)), HI.Clean(HM.life.arch.to(back, False{}, a2, pre, post, xs, ord))>:
  match back:
    case True{}:
      HX.Both{cl_ins(ord, xs, a2, hl, la), cl_join(pre, post, lp, lq)}
    case False{}:
      HX.Both{la, cl_join(pre, a2 <> post, lp, HX.Both{hl, lq})}

def cl_arch.go(+a: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +xs: List<&2, HS.Agent>, +ord: List<&2, String>, +l: HS.Life, +why: Maybe<&2, String>,
    b: HX.Both<HI.Clean(pre), HI.Clean(a <> post)>, la: HI.Clean(xs))
    -> HX.Both<HI.Clean(HM.life.arch.to(HM.live_life(l), True{}, HM.agent_op(a, HM.OLife{l, why}), pre, post, xs, ord)), HI.Clean(HM.life.arch.to(HM.live_life(l), False{}, HM.agent_op(a, HM.OLife{l, why}), pre, post, xs, ord))>:
  match b:
    case HX.Both{lp, b2}:
      match b2:
        case HX.Both{l1, lq}:
          cl_arch_to(HM.live_life(l), HM.agent_op(a, HM.OLife{l, why}), pre, post, xs, ord, cl_op(a, HM.OLife{l, why}, {==}, l1), lp, lq, la)

def cl_arch(m: Maybe<&2, HS.Agent>, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +xs: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +ord: List<&2, String>, +l: HS.Life, +why: Maybe<&2, String>, w: {HAG.whole(pre, m, post) == r : List<&2, HS.Agent>},
    la: HI.Clean(xs), lr: HI.Clean(r)) -> HX.Both<HI.Clean(HM.life.arch(m, True{}, pre, post, xs, r, ord, l, why)), HI.Clean(HM.life.arch(m, False{}, pre, post, xs, r, ord, l, why))>:
  match m:
    case None{}:
      HX.Both{la, lr}
    case Some{+a}:
      cl_arch.go(a, pre, post, xs, ord, l, why, cl_split(pre, a <> post, cl_eq(List.append(&2, HS.Agent, pre, a <> post), r, w, lr)), la)

def cl_live.go(+a: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +l: HS.Life, +why: Maybe<&2, String>,
    b: HX.Both<HI.Clean(pre), HI.Clean(a <> post)>, lr: HI.Clean(r))
    -> HX.Both<HI.Clean(HM.life.live.to(HM.live_life(l), True{}, HM.agent_op(a, HM.OLife{l, why}), pre, post, r)), HI.Clean(HM.life.live.to(HM.live_life(l), False{}, HM.agent_op(a, HM.OLife{l, why}), pre, post, r))>:
  match b:
    case HX.Both{lp, b2}:
      match b2:
        case HX.Both{l1, lq}:
          cl_live_to(HM.live_life(l), HM.agent_op(a, HM.OLife{l, why}), pre, post, r, cl_op(a, HM.OLife{l, why}, {==}, l1), lp, lq, lr)

def cl_life(m: Maybe<&2, HS.Agent>, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +xs: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +ord: List<&2, String>, +n: String, +l: HS.Life, +why: Maybe<&2, String>, w: {HAG.whole(pre, m, post) == xs : List<&2, HS.Agent>},
    la: HI.Clean(xs), lr: HI.Clean(r)) -> HX.Both<HI.Clean(HM.life.live(m, True{}, pre, post, xs, r, ord, n, l, why)), HI.Clean(HM.life.live(m, False{}, pre, post, xs, r, ord, n, l, why))>:
  match m:
    case None{}:
      cl_arch(HAG.hit(r, n), HAG.pre(r, n), HAG.post(r, n), xs, r, ord, l, why, PLA.ag_cut_whole(r, n), la, lr)
    case Some{+a}:
      cl_live.go(a, pre, post, r, l, why, cl_split(pre, a <> post, cl_eq(List.append(&2, HS.Agent, pre, a <> post), xs, w, la)), lr)

# a new task of a clean name
def cl_new(c: Bool, +xs: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +ord: List<&2, String>, +n: String, +a: HS.Agent, +ha: {HM.never_in(HI.names(a)) == False{} : Bool},
    la: HI.Clean(xs), lr: HI.Clean(r)) -> HX.Both<HI.Clean(HM.new_task.go(c, True{}, xs, r, ord, n, a)), HI.Clean(HM.new_task.go(c, False{}, xs, r, ord, n, a))>:
  match c:
    case False{}:
      HX.Both{cl_put(HM.strip(xs, n), n, a, ha, cl_strip(xs, n, la)), cl_strip(r, n, lr)}
    case True{}:
      HX.Both{cl_ins(ord, HM.strip(xs, n), a, ha, cl_strip(xs, n, la)), cl_drop(HM.strip(r, n), n, cl_strip(r, n, lr))}

# a name FreshEv keeps free (not name_held) is not never_agent
def cl_fresh(+s: HS.St, +n: String, +k: {HM.name_held(s, n) == False{} : Bool}) -> {HM.never_agent(n) == False{} : Bool}:
  PLN.nm_held_never(n, PL1.hub_or_false_l(HM.reserved(n), Maybe.is_some(&2, HS.Agent, HM.get_agent(s, n)), k))

# every event keeps the agents clean, on the state opened by name (S.St.open)
def cl_apply.go(+e: HM.Ev, +s: HS.St, la: HI.Clean(HS.St.agents(s)), lr: HI.Clean(HS.St.arch(s)), +k: HX.FreshEv(s, e)) -> HI.NoReserved(HM.apply(HS.St.open(s), e)):
  match e:
    case HM.ETaskCreated{+n, pa, br, ws, at}:
      cl_new(HAG.has(HS.St.arch(s), n), HS.St.agents(s), HS.St.arch(s), HS.St.order(s), n,
        HM.agent_of_new(HM.ETaskCreated{n, pa, br, ws, at}, HN.free_dir(HM.dirs_of(List.append(&2, HS.Agent, HS.St.agents(s), HS.St.arch(s))), n)),
        ni_cons(n, Nil{}, cl_fresh(s, n, k), {==}), la, lr)
    case HM.ERenamed{+old, +new}:
      cl_on_ren(HAG.has(HM.strip(HS.St.agents(s), new), old), HM.strip(HS.St.agents(s), new), HM.strip(HS.St.arch(s), new), old, new, cl_fresh(s, new, k),
        cl_strip(HS.St.agents(s), new, la), cl_strip(HS.St.arch(s), new, lr))
    case HM.ELifecycle{+x, +l, +r}:
      cl_life(HAG.hit(HS.St.agents(s), x), HAG.pre(HS.St.agents(s), x), HAG.post(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), HS.St.order(s), x, l, r, PLA.ag_cut_whole(HS.St.agents(s), x), la, lr)
    case HM.EWorkspace{+x, +w}:
      cl_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OWs{w}, {==}, la, lr)
    case HM.EDeclared{+x, +d, +nt}:
      cl_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.ODeclared{d, nt}, {==}, la, lr)
    case HM.EFileTouched{+x, +p}:
      cl_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OFile{p}, {==}, la, lr)
    case HM.EClaimDropped{+x, +p}:
      cl_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OUnclaim{p}, {==}, la, lr)
    case HM.ESnapshot{+x, +sn}:
      cl_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OSnap{sn}, {==}, la, lr)
    case HM.EPlaced{+x, +w, +er}:
      cl_placed(String.is_empty(er), HS.St.agents(s), HS.St.arch(s), x, w, la, lr)
    case HM.EDropped{+x, +sn, +er}:
      cl_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.ODrop{String.is_empty(er), sn}, {==}, la, lr)
    case HM.EReported{+x, +rp}:
      cl_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OReport{rp}, {==}, la, lr)
    case HM.EFollow{+x, +f, +h}:
      cl_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OFollow{Bool.pick(Maybe<&2, String>, f, Some{h}, None{})}, {==}, la, lr)
    case HM.EJobEnd{+x, b}:
      cl_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OFollow{None{}}, {==}, la, lr)
    case HM.EMsgsMoved{a, b}:
      HX.Both{la, lr}
    case HM.EMsgSent{m}:
      HX.Both{la, lr}
    case HM.EMsgState{a, b}:
      HX.Both{la, lr}
    case HM.ESettled{a}:
      HX.Both{la, lr}
    case HM.ECardOpened{a}:
      HX.Both{la, lr}
    case HM.ECardClosed{a, b, c}:
      HX.Both{la, lr}
    case HM.ECardAnswerPos{a, b, c}:
      HX.Both{la, lr}
    case HM.ECardWithdrawn{a, b}:
      HX.Both{la, lr}
    case HM.ECardLate{a}:
      HX.Both{la, lr}
    case HM.EMainNote{a, b}:
      HX.Both{la, lr}
    case HM.ENotesFlushed{}:
      HX.Both{la, lr}
    case HM.ETimerSet{a, b}:
      HX.Both{la, lr}
    case HM.ETimerFired{a, b, c, d, f}:
      HX.Both{la, lr}
    case HM.ETimerRun{a, b}:
      HX.Both{la, lr}
    case HM.ETimerStop{a, b, c}:
      HX.Both{la, lr}
    case HM.ETimerName{a, b, c, d}:
      HX.Both{la, lr}
    case HM.EX{a}:
      HX.Both{la, lr}
    case HM.EAct{a}:
      HX.Both{la, lr}
    case HM.EWake{a}:
      HX.Both{la, lr}
    case HM.EPaused{a}:
      HX.Both{la, lr}
    case HM.EResumed{a}:
      HX.Both{la, lr}

def cl_apply(+s: HS.St, e: HM.Ev, u: HI.NoReserved(s), +k: HX.FreshEv(s, e)) -> HI.NoReserved(HM.apply(s, e)):
  match u:
    case HX.Both{la, lr}:
      %HSL.St.open_eq(s) : HI.NoReserved(HM.apply(_, e))
      cl_apply.go(e, s, la, lr, k)

def cl_apply_all(+evs: List<&2, HM.Ev>, +s: HS.St, u: HI.NoReserved(s), f: HX.Fresh(evs, s)) -> HI.NoReserved(HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      u
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          cl_apply_all(t, HM.apply(s, e), cl_apply(s, e, u, k), ft)

def cl_tr(-x: HS.St, -y: HS.St, e: {x == y : HS.St}, u: HI.NoReserved(y)) -> HI.NoReserved(x):
  %Equal.sym(HS.St, x, y, e) : HI.NoReserved(_)
  u

# ---- the walk a lookup skips finds nothing ----

def cl_named.true(+h: HS.Agent, +n: String, ec: {String.eq(HS.Agent.name(h), n) == True{} : Bool}, k: {HM.never_agent(n) == True{} : Bool})
    -> {True{} == HM.never_agent(HS.Agent.name(h)) : Bool}:
  %Equal.sym(String, HS.Agent.name(h), n, PL2.hub_str_eq_sound(HS.Agent.name(h), n, ec)) : {True{} == HM.never_agent(_) : Bool}
  Equal.sym(Bool, HM.never_agent(n), True{}, k)

def cl_named.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String, +ec: {String.eq(HS.Agent.name(h), n) == c : Bool}, k: {HM.never_agent(n) == True{} : Bool},
    l1: {HM.never_in(HI.names(h)) == False{} : Bool}, ih: {HM.agent_named(t, n) == None{} : Maybe<&2, HS.Agent>})
    -> {HLZ.pick(Maybe<&2, HS.Agent>, String.eq(HS.Agent.name(h), n), u => Some{h}, u => HM.agent_named(t, n)) == None{} : Maybe<&2, HS.Agent>}:
  match c:
    case True{}:
      PLA.ag_new_absurd({HLZ.pick(Maybe<&2, HS.Agent>, String.eq(HS.Agent.name(h), n), u => Some{h}, u => HM.agent_named(t, n)) == None{} : Maybe<&2, HS.Agent>},
        Equal.trans(Bool, True{}, HM.never_agent(HS.Agent.name(h)), False{}, cl_named.true(h, n, ec, k), ni_head(HS.Agent.name(h), HS.Agent.aliases(h), l1)))
    case False{}:
      %Equal.sym(Bool, String.eq(HS.Agent.name(h), n), False{}, ec) : {HLZ.pick(Maybe<&2, HS.Agent>, _, u => Some{h}, u => HM.agent_named(t, n)) == None{} : Maybe<&2, HS.Agent>}
      ih

# no agent of a clean list is named a never_agent name
def cl_named(+xs: List<&2, HS.Agent>, +n: String, +k: {HM.never_agent(n) == True{} : Bool}, l: HI.Clean(xs)) -> {HM.agent_named(xs, n) == None{} : Maybe<&2, HS.Agent>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match l:
        case HX.Both{l1, l2}:
          cl_named.pick(String.eq(HS.Agent.name(h), n), h, t, n, {==}, k, l1, cl_named(t, n, k, l2))
