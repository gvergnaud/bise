# bend/proofs/lib/hub-agents.bend: the agents split (issue 21 sha 2,
# hub/agents.bend): St.agents the agents not archived, St.arch the
# archived ones. names_apart reads both, as one list (HI.NamesApart:
# St.agents then arch); a lifecycle moves an agent from one side to the
# other, a slide of one entry of that list, and NamesOK is kept by a
# slide (Apart is symmetric). Layered after hub-1, hub-2 and hub-names;
# hub-3's hub_apply_names calls it.
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
import ../../hub/cold.bend as CO
import ./hub-1.bend as PL1
import ./absurd.bend as AB
import ./hub-2.bend as PL2
import ./hub-names.bend as PLN

# ---- lists ----

def ag_assoc(-A: Data, +a: List<&2, A>, +b: List<&2, A>, +c: List<&2, A>)
    -> {List.append(&2, A, List.append(&2, A, a, b), c) == List.append(&2, A, a, List.append(&2, A, b, c)) : List<&2, A>}:
  match a:
    case Nil{}:
      {==}
    case Con{x, t}:
      Equal.cong(List<&2, A>, List<&2, A>, y => x <> y, List.append(&2, A, List.append(&2, A, t, b), c),
        List.append(&2, A, t, List.append(&2, A, b, c)), ag_assoc(A, t, b, c))

def ag_nil(-A: Data, +a: List<&2, A>) -> {List.append(&2, A, a, Nil{}) == a : List<&2, A>}:
  match a:
    case Nil{}:
      {==}
    case Con{x, t}:
      Equal.cong(List<&2, A>, List<&2, A>, y => x <> y, List.append(&2, A, t, Nil{}), t, ag_nil(A, t))

def ag_snoc(-A: Data, +a: List<&2, A>, +x: A) -> {HM.snoc(A, a, x) == List.append(&2, A, a, [x]) : List<&2, A>}:
  match a:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.cong(List<&2, A>, List<&2, A>, y => h <> y, HM.snoc(A, t, x), List.append(&2, A, t, [x]), ag_snoc(A, t, x))

def ag_take_drop(-A: Data, +xs: List<&2, A>, +k: Nat)
    -> {List.append(&2, A, List.take(&2, A, xs, k), List.drop(&2, A, xs, k)) == xs : List<&2, A>}:
  match xs k:
    case Nil{} _:
      {==}
    case Con{h, t} 0n:
      {==}
    case Con{h, t} 1n+p:
      Equal.cong(List<&2, A>, List<&2, A>, y => h <> y, List.append(&2, A, List.take(&2, A, t, p), List.drop(&2, A, t, p)), t, ag_take_drop(A, t, p))

def ag_tr(-x: List<&2, List<&2, String>>, -y: List<&2, List<&2, String>>, e: {x == y : List<&2, List<&2, String>>}, d: HI.NamesOK(y)) -> HI.NamesOK(x):
  %Equal.sym(List<&2, List<&2, String>>, x, y, e) : HI.NamesOK(_)
  d

# the names of two lists, one after the other
def ag_prof_app(+a: List<&2, HS.Agent>, +b: List<&2, HS.Agent>)
    -> {HI.profile(List.append(&2, HS.Agent, a, b)) == List.append(&2, List<&2, String>, HI.profile(a), HI.profile(b)) : List<&2, List<&2, String>>}:
  match a:
    case Nil{}:
      {==}
    case Con{x, t}:
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, y => HI.names(x) <> y, HI.profile(List.append(&2, HS.Agent, t, b)),
        List.append(&2, List<&2, String>, HI.profile(t), HI.profile(b)), ag_prof_app(t, b))

def ag_strip_app(+a: List<&2, HS.Agent>, +b: List<&2, HS.Agent>, +n: String)
    -> {HM.strip(List.append(&2, HS.Agent, a, b), n) == List.append(&2, HS.Agent, HM.strip(a, n), HM.strip(b, n)) : List<&2, HS.Agent>}:
  match a:
    case Nil{}:
      {==}
    case Con{x, t}:
      Equal.cong(List<&2, HS.Agent>, List<&2, HS.Agent>, y => HM.Agent.strip(x, n) <> y, HM.strip(List.append(&2, HS.Agent, t, b), n),
        List.append(&2, HS.Agent, HM.strip(t, n), HM.strip(b, n)), ag_strip_app(t, b, n))

# a cut gives back its list
def ag_cut_whole.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String, ih: {HAG.whole(HAG.pre(t, n), HAG.hit(t, n), HAG.post(t, n)) == t : List<&2, HS.Agent>})
    -> {HAG.whole(HLZ.pick(List<&2, HS.Agent>, c, u => Nil{}, u => h <> HAG.pre(t, n)), HLZ.pick(Maybe<&2, HS.Agent>, c, u => Some{h}, u => HAG.hit(t, n)),
        HLZ.pick(List<&2, HS.Agent>, c, u => t, u => HAG.post(t, n))) == h <> t : List<&2, HS.Agent>}:
  match c:
    case True{}:
      {==}
    case False{}:
      Equal.cong(List<&2, HS.Agent>, List<&2, HS.Agent>, y => h <> y, HAG.whole(HAG.pre(t, n), HAG.hit(t, n), HAG.post(t, n)), t, ih)

def ag_cut_whole(+xs: List<&2, HS.Agent>, +n: String) -> {HAG.whole(HAG.pre(xs, n), HAG.hit(xs, n), HAG.post(xs, n)) == xs : List<&2, HS.Agent>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      ag_cut_whole.pick(String.eq(HS.Agent.name(h), n), h, t, n, ag_cut_whole(t, n))

# ---- NamesOK is kept by a slide of one entry ----

def ag_aa_split.add(-p: List<&2, String>, -y: List<&2, String>, -t: List<&2, List<&2, String>>, -r: List<&2, List<&2, String>>, a1: HI.Apart(p, y), b: HX.Both<HI.ApartAll(p, t), HI.ApartAll(p, r)>)
    -> HX.Both<HI.ApartAll(p, y <> t), HI.ApartAll(p, r)>:
  match b:
    case HX.Both{b1, b2}:
      HX.Both{HX.Both{a1, b1}, b2}

def ag_aa_split(+p: List<&2, String>, +q: List<&2, List<&2, String>>, +r: List<&2, List<&2, String>>, a: HI.ApartAll(p, List.append(&2, List<&2, String>, q, r)))
    -> HX.Both<HI.ApartAll(p, q), HI.ApartAll(p, r)>:
  match q:
    case Nil{}:
      HX.Both{Unit{}, a}
    case Con{y, t}:
      match a:
        case HX.Both{a1, a2}:
          ag_aa_split.add(p, y, t, r, a1, ag_aa_split(p, t, r, a2))

def ag_aa_join(+p: List<&2, String>, +q: List<&2, List<&2, String>>, +r: List<&2, List<&2, String>>, a: HI.ApartAll(p, q), b: HI.ApartAll(p, r))
    -> HI.ApartAll(p, List.append(&2, List<&2, String>, q, r)):
  match q:
    case Nil{}:
      b
    case Con{y, t}:
      match a:
        case HX.Both{a1, a2}:
          HX.Both{a1, ag_aa_join(p, t, r, a2, b)}

# Apart is symmetric: no name of one is a name of the other
def ag_apart_sym(+p: List<&2, String>, +q: List<&2, String>, a: HI.Apart(p, q)) -> HI.Apart(q, p):
  match p q:
    case Nil{} Nil{}:
      Unit{}
    case Nil{} Con{m, bl}:
      Unit{}
    case Con{n, al} Nil{}:
      Unit{}
    case Con{n, al} Con{m, bl}:
      match a:
        case HX.Both{+a1, a2}:
          HX.Both{PL1.hub_lor_false(String.eq(n, m), HM.str_in(al, m),
              PL2.hub_str_eq_sym_false(m, n, PL1.hub_lor_false_l(String.eq(m, n), HM.str_in(bl, n), a1)), a2),
            PL1.hub_lor_false_r(String.eq(m, n), HM.str_in(bl, n), a1)}

def ag_aa_ins(+p: List<&2, String>, +q: List<&2, List<&2, String>>, +x: List<&2, String>, +r: List<&2, List<&2, String>>,
    a: HI.ApartAll(p, List.append(&2, List<&2, String>, q, r)), b: HI.Apart(p, x)) -> HI.ApartAll(p, List.append(&2, List<&2, String>, q, x <> r)):
  match q:
    case Nil{}:
      HX.Both{b, a}
    case Con{y, t}:
      match a:
        case HX.Both{a1, a2}:
          HX.Both{a1, ag_aa_ins(p, t, x, r, a2, b)}

def ag_aa_out.add(-p: List<&2, String>, -y: List<&2, String>, -t: List<&2, List<&2, String>>, -x: List<&2, String>, -r: List<&2, List<&2, String>>, a1: HI.Apart(p, y),
    c: HX.Both<HI.ApartAll(p, List.append(&2, List<&2, String>, t, r)), HI.Apart(p, x)>) -> HX.Both<HI.ApartAll(p, y <> List.append(&2, List<&2, String>, t, r)), HI.Apart(p, x)>:
  match c:
    case HX.Both{c1, c2}:
      HX.Both{HX.Both{a1, c1}, c2}

def ag_aa_out(+p: List<&2, String>, +q: List<&2, List<&2, String>>, +x: List<&2, String>, +r: List<&2, List<&2, String>>,
    a: HI.ApartAll(p, List.append(&2, List<&2, String>, q, x <> r))) -> HX.Both<HI.ApartAll(p, List.append(&2, List<&2, String>, q, r)), HI.Apart(p, x)>:
  match q:
    case Nil{}:
      match a:
        case HX.Both{b, c}:
          HX.Both{c, b}
    case Con{y, t}:
      match a:
        case HX.Both{a1, a2}:
          ag_aa_out.add(p, y, t, x, r, a1, ag_aa_out(p, t, x, r, a2))

# x slides right past q
def ag_slide_r0(+x: List<&2, String>, +q: List<&2, List<&2, String>>, +r: List<&2, List<&2, String>>, k: HI.NamesOK(x <> List.append(&2, List<&2, String>, q, r)))
    -> HI.NamesOK(List.append(&2, List<&2, String>, q, x <> r)):
  match q:
    case Nil{}:
      k
    case Con{y, t}:
      match k:
        case HX.Both{kx, kr}:
          match kx kr:
            case HX.Both{xy, xr} HX.Both{ya, yn}:
              HX.Both{ag_aa_ins(y, t, x, r, ya, ag_apart_sym(x, y, xy)), ag_slide_r0(x, t, r, HX.Both{xr, yn})}

# x slides left past q
def ag_slide_l0.put(+x: List<&2, String>, +y: List<&2, String>, -t: List<&2, List<&2, String>>, -r: List<&2, List<&2, String>>,
    o: HX.Both<HI.ApartAll(y, List.append(&2, List<&2, String>, t, r)), HI.Apart(y, x)>, k: HI.NamesOK(x <> List.append(&2, List<&2, String>, t, r)))
    -> HI.NamesOK(x <> y <> List.append(&2, List<&2, String>, t, r)):
  match o k:
    case HX.Both{yr, yx} HX.Both{xr, yn}:
      HX.Both{HX.Both{ag_apart_sym(y, x, yx), xr}, HX.Both{yr, yn}}

def ag_slide_l0(+x: List<&2, String>, +q: List<&2, List<&2, String>>, +r: List<&2, List<&2, String>>, k: HI.NamesOK(List.append(&2, List<&2, String>, q, x <> r)))
    -> HI.NamesOK(x <> List.append(&2, List<&2, String>, q, r)):
  match q:
    case Nil{}:
      k
    case Con{y, t}:
      match k:
        case HX.Both{ya, kn}:
          ag_slide_l0.put(x, y, t, r, ag_aa_out(y, t, x, r, ya), ag_slide_l0(x, t, r, kn))

# the same after a prefix p
def ag_slide_r.head(+y: List<&2, String>, +t: List<&2, List<&2, String>>, +x: List<&2, String>, +q: List<&2, List<&2, String>>, +r: List<&2, List<&2, String>>,
    b: HX.Both<HI.ApartAll(y, t), HI.ApartAll(y, x <> List.append(&2, List<&2, String>, q, r))>) -> HI.ApartAll(y, List.append(&2, List<&2, String>, t, List.append(&2, List<&2, String>, q, x <> r))):
  match b:
    case HX.Both{yt, yx}:
      match yx:
        case HX.Both{yxx, yqr}:
          ag_aa_join(y, t, List.append(&2, List<&2, String>, q, x <> r), yt, ag_aa_ins(y, q, x, r, yqr, yxx))

def ag_slide_l.head2(+y: List<&2, String>, +t: List<&2, List<&2, String>>, +x: List<&2, String>, +q: List<&2, List<&2, String>>, +r: List<&2, List<&2, String>>,
    yt: HI.ApartAll(y, t), o: HX.Both<HI.ApartAll(y, List.append(&2, List<&2, String>, q, r)), HI.Apart(y, x)>) -> HI.ApartAll(y, List.append(&2, List<&2, String>, t, x <> List.append(&2, List<&2, String>, q, r))):
  match o:
    case HX.Both{yqr, yxx}:
      ag_aa_join(y, t, x <> List.append(&2, List<&2, String>, q, r), yt, HX.Both{yxx, yqr})

def ag_slide_l.head(+y: List<&2, String>, +t: List<&2, List<&2, String>>, +x: List<&2, String>, +q: List<&2, List<&2, String>>, +r: List<&2, List<&2, String>>,
    b: HX.Both<HI.ApartAll(y, t), HI.ApartAll(y, List.append(&2, List<&2, String>, q, x <> r))>) -> HI.ApartAll(y, List.append(&2, List<&2, String>, t, x <> List.append(&2, List<&2, String>, q, r))):
  match b:
    case HX.Both{yt, yq}:
      ag_slide_l.head2(y, t, x, q, r, yt, ag_aa_out(y, q, x, r, yq))

def ag_slide_r(+p: List<&2, List<&2, String>>, +x: List<&2, String>, +q: List<&2, List<&2, String>>, +r: List<&2, List<&2, String>>,
    k: HI.NamesOK(List.append(&2, List<&2, String>, p, x <> List.append(&2, List<&2, String>, q, r))))
    -> HI.NamesOK(List.append(&2, List<&2, String>, p, List.append(&2, List<&2, String>, q, x <> r))):
  match p:
    case Nil{}:
      ag_slide_r0(x, q, r, k)
    case Con{y, t}:
      match k:
        case HX.Both{ya, kn}:
          HX.Both{ag_slide_r.head(y, t, x, q, r, ag_aa_split(y, t, x <> List.append(&2, List<&2, String>, q, r), ya)), ag_slide_r(t, x, q, r, kn)}

def ag_slide_l(+p: List<&2, List<&2, String>>, +x: List<&2, String>, +q: List<&2, List<&2, String>>, +r: List<&2, List<&2, String>>,
    k: HI.NamesOK(List.append(&2, List<&2, String>, p, List.append(&2, List<&2, String>, q, x <> r))))
    -> HI.NamesOK(List.append(&2, List<&2, String>, p, x <> List.append(&2, List<&2, String>, q, r))):
  match p:
    case Nil{}:
      ag_slide_l0(x, q, r, k)
    case Con{y, t}:
      match k:
        case HX.Both{ya, kn}:
          HX.Both{ag_slide_l.head(y, t, x, q, r, ag_aa_split(y, t, List.append(&2, List<&2, String>, q, x <> r), ya)), ag_slide_l(t, x, q, r, kn)}

# ---- the agents' names through the split ----

def ag_pf3(+a: List<&2, HS.Agent>, +b: List<&2, HS.Agent>, +c: List<&2, HS.Agent>)
    -> {HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, a, b), c))
        == List.append(&2, List<&2, String>, HI.profile(a), List.append(&2, List<&2, String>, HI.profile(b), HI.profile(c))) : List<&2, List<&2, String>>}:
  Equal.trans(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, a, b), c)),
    HI.profile(List.append(&2, HS.Agent, a, List.append(&2, HS.Agent, b, c))),
    List.append(&2, List<&2, String>, HI.profile(a), List.append(&2, List<&2, String>, HI.profile(b), HI.profile(c))),
    Equal.cong(List<&2, HS.Agent>, List<&2, List<&2, String>>, z => HI.profile(z), List.append(&2, HS.Agent, List.append(&2, HS.Agent, a, b), c),
      List.append(&2, HS.Agent, a, List.append(&2, HS.Agent, b, c)), ag_assoc(HS.Agent, a, b, c)),
    Equal.trans(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, a, List.append(&2, HS.Agent, b, c))),
      List.append(&2, List<&2, String>, HI.profile(a), HI.profile(List.append(&2, HS.Agent, b, c))),
      List.append(&2, List<&2, String>, HI.profile(a), List.append(&2, List<&2, String>, HI.profile(b), HI.profile(c))),
      ag_prof_app(a, List.append(&2, HS.Agent, b, c)),
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, HI.profile(a), z),
        HI.profile(List.append(&2, HS.Agent, b, c)), List.append(&2, List<&2, String>, HI.profile(b), HI.profile(c)), ag_prof_app(b, c))))

# an agent with the same names in the same place: the same profile
def ag_swap(+p: List<&2, HS.Agent>, +q: List<&2, HS.Agent>, +a: HS.Agent, +a2: HS.Agent, en: {HI.names(a2) == HI.names(a) : List<&2, String>})
    -> {HI.profile(List.append(&2, HS.Agent, p, a2 <> q)) == HI.profile(List.append(&2, HS.Agent, p, a <> q)) : List<&2, List<&2, String>>}:
  match p:
    case Nil{}:
      Equal.cong(List<&2, String>, List<&2, List<&2, String>>, z => z <> HI.profile(q), HI.names(a2), HI.names(a), en)
    case Con{h, t}:
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => HI.names(h) <> z,
        HI.profile(List.append(&2, HS.Agent, t, a2 <> q)), HI.profile(List.append(&2, HS.Agent, t, a <> q)), ag_swap(t, q, a, a2, en))

def ag_ok_eq(-x: List<&2, HS.Agent>, -y: List<&2, HS.Agent>, e: {x == y : List<&2, HS.Agent>}, d: HI.NamesOK(HI.profile(y))) -> HI.NamesOK(HI.profile(x)):
  %Equal.sym(List<&2, HS.Agent>, x, y, e) : HI.NamesOK(HI.profile(_))
  d

# a profile-keeping change of one side: the same profile
def ag_prof_l(+x: List<&2, HS.Agent>, +x2: List<&2, HS.Agent>, +y: List<&2, HS.Agent>, e: {HI.profile(x2) == HI.profile(x) : List<&2, List<&2, String>>})
    -> {HI.profile(List.append(&2, HS.Agent, x2, y)) == HI.profile(List.append(&2, HS.Agent, x, y)) : List<&2, List<&2, String>>}:
  Equal.trans(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, x2, y)), List.append(&2, List<&2, String>, HI.profile(x2), HI.profile(y)), HI.profile(List.append(&2, HS.Agent, x, y)),
    ag_prof_app(x2, y),
    Equal.trans(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, HI.profile(x2), HI.profile(y)), List.append(&2, List<&2, String>, HI.profile(x), HI.profile(y)), HI.profile(List.append(&2, HS.Agent, x, y)),
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, z, HI.profile(y)), HI.profile(x2), HI.profile(x), e),
      Equal.sym(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, x, y)), List.append(&2, List<&2, String>, HI.profile(x), HI.profile(y)), ag_prof_app(x, y))))

def ag_prof_r(+x: List<&2, HS.Agent>, +y: List<&2, HS.Agent>, +y2: List<&2, HS.Agent>, e: {HI.profile(y2) == HI.profile(y) : List<&2, List<&2, String>>})
    -> {HI.profile(List.append(&2, HS.Agent, x, y2)) == HI.profile(List.append(&2, HS.Agent, x, y)) : List<&2, List<&2, String>>}:
  match x:
    case Nil{}:
      e
    case Con{h, t}:
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => HI.names(h) <> z, HI.profile(List.append(&2, HS.Agent, t, y2)), HI.profile(List.append(&2, HS.Agent, t, y)), ag_prof_r(t, y, y2, e))

# ---- an agent event on its side is the same as on both lists ----

def ag_map_hit.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp,
    e: {HLZ.or(c, u => HAG.has(t, n)) == True{} : Bool},
    ih: @p: {HAG.has(t, n) == True{} : Bool} -> {HM.map_agent(List.append(&2, HS.Agent, t, r), n, op) == List.append(&2, HS.Agent, HM.map_agent(t, n, op), r) : List<&2, HS.Agent>})
    -> {HM.map_agent.go(h <> List.append(&2, HS.Agent, t, r), c, n, op) == List.append(&2, HS.Agent, HM.map_agent.go(h <> t, c, n, op), r) : List<&2, HS.Agent>}:
  match c:
    case True{}:
      {==}
    case False{}:
      Equal.cong(List<&2, HS.Agent>, List<&2, HS.Agent>, z => h <> z, HM.map_agent(List.append(&2, HS.Agent, t, r), n, op),
        List.append(&2, HS.Agent, HM.map_agent(t, n, op), r), ih(e))

# the agent is in the first list: the second is untouched
def ag_map_hit(+xs: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, e: {HAG.has(xs, n) == True{} : Bool})
    -> {HM.map_agent(List.append(&2, HS.Agent, xs, r), n, op) == List.append(&2, HS.Agent, HM.map_agent(xs, n, op), r) : List<&2, HS.Agent>}:
  match xs:
    case Nil{}:
      AB.absurd_eq(AB.tf_absurd(Equal.sym(Bool, False{}, True{}, e)), List<&2, HS.Agent>, HM.map_agent(r, n, op), r)
    case Con{h, t}:
      ag_map_hit.pick(String.eq(HS.Agent.name(h), n), h, t, r, n, op, e, p => ag_map_hit(t, r, n, op, p))

def ag_map_miss.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp,
    e: {HLZ.or(c, u => HAG.has(t, n)) == False{} : Bool},
    ih: @p: {HAG.has(t, n) == False{} : Bool} -> {HM.map_agent(List.append(&2, HS.Agent, t, r), n, op) == List.append(&2, HS.Agent, t, HM.map_agent(r, n, op)) : List<&2, HS.Agent>})
    -> {HM.map_agent.go(h <> List.append(&2, HS.Agent, t, r), c, n, op) == List.append(&2, HS.Agent, h <> t, HM.map_agent(r, n, op)) : List<&2, HS.Agent>}:
  match c:
    case True{}:
      AB.absurd_eq(AB.tf_absurd(e), List<&2, HS.Agent>, HM.map_agent.go(h <> List.append(&2, HS.Agent, t, r), True{}, n, op), List.append(&2, HS.Agent, h <> t, HM.map_agent(r, n, op)))
    case False{}:
      Equal.cong(List<&2, HS.Agent>, List<&2, HS.Agent>, z => h <> z, HM.map_agent(List.append(&2, HS.Agent, t, r), n, op),
        List.append(&2, HS.Agent, t, HM.map_agent(r, n, op)), ih(e))

# the agent is not in the first list: the second one changes
def ag_map_miss(+xs: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, e: {HAG.has(xs, n) == False{} : Bool})
    -> {HM.map_agent(List.append(&2, HS.Agent, xs, r), n, op) == List.append(&2, HS.Agent, xs, HM.map_agent(r, n, op)) : List<&2, HS.Agent>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      ag_map_miss.pick(String.eq(HS.Agent.name(h), n), h, t, r, n, op, e, p => ag_map_miss(t, r, n, op, p))

# ---- a name no agent has ----

def ag_has_none.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String,
    e: {HLZ.pick(Maybe<&2, HS.Agent>, c, u => Some{h}, u => HM.agent_named(t, n)) == None{} : Maybe<&2, HS.Agent>},
    ih: @p: {HM.agent_named(t, n) == None{} : Maybe<&2, HS.Agent>} -> {HAG.has(t, n) == False{} : Bool})
    -> {HLZ.or(c, u => HAG.has(t, n)) == False{} : Bool}:
  match c:
    case True{}:
      AB.absurd_eq(PL2.hub_some_none(h, e), Bool, True{}, False{})
    case False{}:
      ih(e)

def ag_has_none(+xs: List<&2, HS.Agent>, +n: String, e: {HM.agent_named(xs, n) == None{} : Maybe<&2, HS.Agent>}) -> {HAG.has(xs, n) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      ag_has_none.pick(String.eq(HS.Agent.name(h), n), h, t, n, e, p => ag_has_none(t, n, p))

def ag_none_r.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String,
    e: {HLZ.pick(Maybe<&2, HS.Agent>, c, u => Some{h}, u => HM.agent_named(List.append(&2, HS.Agent, t, r), n)) == None{} : Maybe<&2, HS.Agent>},
    ih: @p: {HM.agent_named(List.append(&2, HS.Agent, t, r), n) == None{} : Maybe<&2, HS.Agent>} -> {HM.agent_named(r, n) == None{} : Maybe<&2, HS.Agent>})
    -> {HM.agent_named(r, n) == None{} : Maybe<&2, HS.Agent>}:
  match c:
    case True{}:
      AB.absurd_eq(PL2.hub_some_none(h, e), Maybe<&2, HS.Agent>, HM.agent_named(r, n), None{})
    case False{}:
      ih(e)

# no agent of both lists has it: none of the second
def ag_none_r(+xs: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, e: {HM.agent_named(List.append(&2, HS.Agent, xs, r), n) == None{} : Maybe<&2, HS.Agent>})
    -> {HM.agent_named(r, n) == None{} : Maybe<&2, HS.Agent>}:
  match xs:
    case Nil{}:
      e
    case Con{h, t}:
      ag_none_r.pick(String.eq(HS.Agent.name(h), n), h, t, r, n, e, p => ag_none_r(t, r, n, p))

def ag_free_l(+p: List<&2, List<&2, String>>, +q: List<&2, List<&2, String>>, +n: String, f: PL2.hub_free(List.append(&2, List<&2, String>, p, q), n)) -> PL2.hub_free(p, n):
  match p:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match f:
        case HX.Both{f1, f2}:
          HX.Both{f1, ag_free_l(t, q, n, f2)}

# ---- each agent event keeps the names apart ----

# an operation that is not a rename, on its side
def ag_on_ok(c: Bool, +a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, +nr: {PL2.hub_is_rename(op) == False{} : Bool}, u: HI.NamesOK(HI.profile(List.append(&2, HS.Agent, a, r))))
    -> HI.NamesOK(HI.profile(List.append(&2, HS.Agent, HM.on_agent.go(c, True{}, a, r, n, op), HM.on_agent.go(c, False{}, a, r, n, op)))):
  match c:
    case True{}:
      ag_tr(HI.profile(List.append(&2, HS.Agent, HM.map_agent(a, n, op), r)), HI.profile(List.append(&2, HS.Agent, a, r)), ag_prof_l(a, HM.map_agent(a, n, op), r, PL2.hub_prof_map(a, n, op, nr)), u)
    case False{}:
      ag_tr(HI.profile(List.append(&2, HS.Agent, a, HM.map_agent(r, n, op))), HI.profile(List.append(&2, HS.Agent, a, r)), ag_prof_r(a, r, HM.map_agent(r, n, op), PL2.hub_prof_map(r, n, op, nr)), u)

# hub-loop: a placed task's line (its ws, or nothing)
def ag_placed_ok(ok: Bool, +a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +ws: J.Json, u: HI.NamesOK(HI.profile(List.append(&2, HS.Agent, a, r))))
    -> HI.NamesOK(HI.profile(List.append(&2, HS.Agent, HM.placed.go(ok, True{}, a, r, n, ws), HM.placed.go(ok, False{}, a, r, n, ws)))):
  match ok:
    case True{}:
      ag_on_ok(HAG.has(a, n), a, r, n, HM.OWs{ws}, {==}, u)
    case False{}:
      u

# a rename on its side: the rename of both lists as one
def ag_ren_ok(c: Bool, +a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +nn: String, e: {HAG.has(a, n) == c : Bool},
    k: HI.NamesOK(HI.profile(HM.map_agent(List.append(&2, HS.Agent, a, r), n, HM.ORename{nn}))))
    -> HI.NamesOK(HI.profile(List.append(&2, HS.Agent, HM.on_agent.go(c, True{}, a, r, n, HM.ORename{nn}), HM.on_agent.go(c, False{}, a, r, n, HM.ORename{nn})))):
  match c:
    case True{}:
      ag_ok_eq(List.append(&2, HS.Agent, HM.map_agent(a, n, HM.ORename{nn}), r), HM.map_agent(List.append(&2, HS.Agent, a, r), n, HM.ORename{nn}),
        Equal.sym(List<&2, HS.Agent>, HM.map_agent(List.append(&2, HS.Agent, a, r), n, HM.ORename{nn}), List.append(&2, HS.Agent, HM.map_agent(a, n, HM.ORename{nn}), r), ag_map_hit(a, r, n, HM.ORename{nn}, e)), k)
    case False{}:
      ag_ok_eq(List.append(&2, HS.Agent, a, HM.map_agent(r, n, HM.ORename{nn})), HM.map_agent(List.append(&2, HS.Agent, a, r), n, HM.ORename{nn}),
        Equal.sym(List<&2, HS.Agent>, HM.map_agent(List.append(&2, HS.Agent, a, r), n, HM.ORename{nn}), List.append(&2, HS.Agent, a, HM.map_agent(r, n, HM.ORename{nn})), ag_map_miss(a, r, n, HM.ORename{nn}, e)), k)

# a lifecycle of a live agent: it stays (its names unchanged), or it is
# archived (it slides to arch's head)
def ag_live_to_ok(stay: Bool, +a: HS.Agent, +a2: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, en: {HI.names(a2) == HI.names(a) : List<&2, String>},
    u: HI.NamesOK(HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, pre, a <> post), r))))
    -> HI.NamesOK(HI.profile(List.append(&2, HS.Agent, HM.life.live.to(stay, True{}, a2, pre, post, r), HM.life.live.to(stay, False{}, a2, pre, post, r)))):
  match stay:
    case True{}:
      ag_tr(HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, pre, a2 <> post), r)), HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, pre, a <> post), r)), ag_prof_l(List.append(&2, HS.Agent, pre, a <> post), List.append(&2, HS.Agent, pre, a2 <> post), r, ag_swap(pre, post, a, a2, en)), u)
    case False{}:
      ag_tr(HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, pre, post), a2 <> r)), List.append(&2, List<&2, String>, HI.profile(pre), List.append(&2, List<&2, String>, HI.profile(post), HI.names(a) <> HI.profile(r))),
        Equal.trans(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, pre, post), a2 <> r)), List.append(&2, List<&2, String>, HI.profile(pre), List.append(&2, List<&2, String>, HI.profile(post), HI.names(a2) <> HI.profile(r))), List.append(&2, List<&2, String>, HI.profile(pre), List.append(&2, List<&2, String>, HI.profile(post), HI.names(a) <> HI.profile(r))),
      ag_pf3(pre, post, a2 <> r),
      Equal.cong(List<&2, String>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, HI.profile(pre), List.append(&2, List<&2, String>, HI.profile(post), z <> HI.profile(r))), HI.names(a2), HI.names(a), en)),
        ag_slide_r(HI.profile(pre), HI.names(a), HI.profile(post), HI.profile(r), ag_tr(List.append(&2, List<&2, String>, HI.profile(pre), List.append(&2, List<&2, String>, HI.profile(a <> post), HI.profile(r))), HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, pre, a <> post), r)), Equal.sym(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, pre, a <> post), r)), List.append(&2, List<&2, String>, HI.profile(pre), List.append(&2, List<&2, String>, HI.profile(a <> post), HI.profile(r))), ag_pf3(pre, a <> post, r)), u)))

# a lifecycle of an archived agent: it stays (its names unchanged), or it
# is brought back (it slides left to its creation place in St.agents)
def ag_arch_to_ok(back: Bool, +a: HS.Agent, +a2: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +a0: List<&2, HS.Agent>, +ord: List<&2, String>, en: {HI.names(a2) == HI.names(a) : List<&2, String>},
    u: HI.NamesOK(HI.profile(List.append(&2, HS.Agent, a0, List.append(&2, HS.Agent, pre, a <> post)))))
    -> HI.NamesOK(HI.profile(List.append(&2, HS.Agent, HM.life.arch.to(back, True{}, a2, pre, post, a0, ord), HM.life.arch.to(back, False{}, a2, pre, post, a0, ord)))):
  match back:
    case False{}:
      ag_tr(HI.profile(List.append(&2, HS.Agent, a0, List.append(&2, HS.Agent, pre, a2 <> post))), HI.profile(List.append(&2, HS.Agent, a0, List.append(&2, HS.Agent, pre, a <> post))), ag_prof_r(a0, List.append(&2, HS.Agent, pre, a <> post), List.append(&2, HS.Agent, pre, a2 <> post), ag_swap(pre, post, a, a2, en)), u)
    case True{}:
      ag_tr(HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), a2 <> List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, HS.Agent, pre, post))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a) <> List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.profile(post))),
        Equal.trans(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), a2 <> List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, HS.Agent, pre, post))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a2) <> List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(List.append(&2, HS.Agent, pre, post)))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a) <> List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.profile(post))),
      ag_pf3(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), a2 <> List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), List.append(&2, HS.Agent, pre, post)),
      Equal.trans(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a2) <> List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(List.append(&2, HS.Agent, pre, post)))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a2) <> List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(pre), HI.profile(post)))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a) <> List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.profile(post))),
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a2) <> List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), z)), HI.profile(List.append(&2, HS.Agent, pre, post)), List.append(&2, List<&2, String>, HI.profile(pre), HI.profile(post)), ag_prof_app(pre, post)),
      Equal.trans(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a2) <> List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(pre), HI.profile(post)))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a2) <> List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.profile(post))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a) <> List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.profile(post))),
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a2) <> z), List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(pre), HI.profile(post))), List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.profile(post)), Equal.sym(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.profile(post)), List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(pre), HI.profile(post))), ag_assoc(List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre), HI.profile(post)))),
      Equal.cong(List<&2, String>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), z <> List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.profile(post))), HI.names(a2), HI.names(a), en)))),
        ag_slide_l(HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.names(a), List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.profile(post), ag_tr(List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.names(a) <> HI.profile(post))), HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, HS.Agent, pre, a <> post))), Equal.sym(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, HS.Agent, pre, a <> post))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.names(a) <> HI.profile(post))), Equal.trans(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, List.append(&2, HS.Agent, List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, HS.Agent, pre, a <> post))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(List.append(&2, HS.Agent, pre, a <> post)))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.names(a) <> HI.profile(post))),
      ag_pf3(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), List.append(&2, HS.Agent, pre, a <> post)),
      Equal.trans(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(List.append(&2, HS.Agent, pre, a <> post)))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(pre), HI.names(a) <> HI.profile(post)))), List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.names(a) <> HI.profile(post))),
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), z)), HI.profile(List.append(&2, HS.Agent, pre, a <> post)), List.append(&2, List<&2, String>, HI.profile(pre), HI.names(a) <> HI.profile(post)), ag_prof_app(pre, a <> post)),
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, HI.profile(List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), z), List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(pre), HI.names(a) <> HI.profile(post))), List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.names(a) <> HI.profile(post)), Equal.sym(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre)), HI.names(a) <> HI.profile(post)), List.append(&2, List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, List<&2, String>, HI.profile(pre), HI.names(a) <> HI.profile(post))), ag_assoc(List<&2, String>, HI.profile(List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), HI.profile(pre), HI.names(a) <> HI.profile(post))))))), ag_ok_eq(List.append(&2, HS.Agent, List.append(&2, HS.Agent, List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), List.append(&2, HS.Agent, pre, a <> post)), List.append(&2, HS.Agent, a0, List.append(&2, HS.Agent, pre, a <> post)), Equal.cong(List<&2, HS.Agent>, List<&2, HS.Agent>, z => List.append(&2, HS.Agent, z, List.append(&2, HS.Agent, pre, a <> post)), List.append(&2, HS.Agent, List.take(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2))), List.drop(&2, HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), a0, ag_take_drop(HS.Agent, a0, HAG.place(ord, a0, HS.Agent.name(a2)))), u))))

def ag_arch_ok(m: Maybe<&2, HS.Agent>, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +a0: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +ord: List<&2, String>, +l: HS.Life, +why: Maybe<&2, String>,
    w: {HAG.whole(pre, m, post) == r : List<&2, HS.Agent>}, u: HI.NamesOK(HI.profile(List.append(&2, HS.Agent, a0, r))))
    -> HI.NamesOK(HI.profile(List.append(&2, HS.Agent, HM.life.arch(m, True{}, pre, post, a0, r, ord, l, why), HM.life.arch(m, False{}, pre, post, a0, r, ord, l, why)))):
  match m:
    case None{}:
      u
    case Some{+a}:
      ag_arch_to_ok(HM.live_life(l), a, HM.agent_op(a, HM.OLife{l, why}), pre, post, a0, ord, PL2.hub_names_op(a, HM.OLife{l, why}, {==}),
        ag_ok_eq(List.append(&2, HS.Agent, a0, List.append(&2, HS.Agent, pre, a <> post)), List.append(&2, HS.Agent, a0, r), Equal.cong(List<&2, HS.Agent>, List<&2, HS.Agent>, z => List.append(&2, HS.Agent, a0, z), List.append(&2, HS.Agent, pre, a <> post), r, w), u))

def ag_life_ok(m: Maybe<&2, HS.Agent>, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +a0: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +ord: List<&2, String>, +n: String, +l: HS.Life, +why: Maybe<&2, String>,
    w: {HAG.whole(pre, m, post) == a0 : List<&2, HS.Agent>}, u: HI.NamesOK(HI.profile(List.append(&2, HS.Agent, a0, r))))
    -> HI.NamesOK(HI.profile(List.append(&2, HS.Agent, HM.life.live(m, True{}, pre, post, a0, r, ord, n, l, why), HM.life.live(m, False{}, pre, post, a0, r, ord, n, l, why)))):
  match m:
    case None{}:
      ag_arch_ok(HAG.hit(r, n), HAG.pre(r, n), HAG.post(r, n), a0, r, ord, l, why, ag_cut_whole(r, n), u)
    case Some{+a}:
      ag_live_to_ok(HM.live_life(l), a, HM.agent_op(a, HM.OLife{l, why}), pre, post, r, PL2.hub_names_op(a, HM.OLife{l, why}, {==}),
        ag_ok_eq(List.append(&2, HS.Agent, List.append(&2, HS.Agent, pre, a <> post), r), List.append(&2, HS.Agent, a0, r), Equal.cong(List<&2, HS.Agent>, List<&2, HS.Agent>, z => List.append(&2, HS.Agent, z, r), List.append(&2, HS.Agent, pre, a <> post), a0, w), u))

def ag_free_eq(-p: List<&2, List<&2, String>>, -q: List<&2, List<&2, String>>, -n: String, e: {p == q : List<&2, List<&2, String>>}, f: PL2.hub_free(q, n)) -> PL2.hub_free(p, n):
  %Equal.sym(List<&2, List<&2, String>>, p, q, e) : PL2.hub_free(_, n)
  f

def ag_new_eqs(+a0: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String) -> {HI.profile(HM.strip(List.append(&2, HS.Agent, a0, r), n)) == List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))) : List<&2, List<&2, String>>}:
  Equal.trans(List<&2, List<&2, String>>, HI.profile(HM.strip(List.append(&2, HS.Agent, a0, r), n)), HI.profile(List.append(&2, HS.Agent, HM.strip(a0, n), HM.strip(r, n))), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))),
      Equal.cong(List<&2, HS.Agent>, List<&2, List<&2, String>>, z => HI.profile(z), HM.strip(List.append(&2, HS.Agent, a0, r), n), List.append(&2, HS.Agent, HM.strip(a0, n), HM.strip(r, n)), ag_strip_app(a0, r, n)),
      ag_prof_app(HM.strip(a0, n), HM.strip(r, n)))

def ag_new_absurd.of(-g: Type, x: Empty) -> g:
  match x:

def ag_new_absurd(-g: Type, e: {True{} == False{} : Bool}) -> g:
  ag_new_absurd.of(g, AB.tf_absurd(e))

# the new agent goes last of St.agents: it slides there from the end of
# both lists
def ag_new_ok(c: Bool, +a0: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +ord: List<&2, String>, +n: String, +x: HS.Agent, e: {HAG.has(r, n) == c : Bool},
    en: {HI.names(x) == [n] : List<&2, String>}, +hn: {HM.agent_named(List.append(&2, HS.Agent, a0, r), n) == None{} : Maybe<&2, HS.Agent>}, u: HI.NamesOK(HI.profile(List.append(&2, HS.Agent, a0, r))))
    -> HI.NamesOK(HI.profile(List.append(&2, HS.Agent, HM.new_task.go(c, True{}, a0, r, ord, n, x), HM.new_task.go(c, False{}, a0, r, ord, n, x)))):
  match c:
    case True{}:
      ag_new_absurd(HI.NamesOK(HI.profile(List.append(&2, HS.Agent, HM.new_task.go(True{}, True{}, a0, r, ord, n, x), HM.new_task.go(True{}, False{}, a0, r, ord, n, x)))),
        Equal.trans(Bool, True{}, HAG.has(r, n), False{}, Equal.sym(Bool, HAG.has(r, n), True{}, e), ag_has_none(r, n, ag_none_r(a0, r, n, hn))))
    case False{}:
      ag_tr(HI.profile(List.append(&2, HS.Agent, HM.put_agent(HM.strip(a0, n), n, x), HM.strip(r, n))), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), [n] <> List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{})),
        Equal.trans(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, HM.put_agent(HM.strip(a0, n), n, x), HM.strip(r, n))), List.append(&2, List<&2, String>, HI.profile(HM.put_agent(HM.strip(a0, n), n, x)), HI.profile(HM.strip(r, n))), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), [n] <> List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{})),
      ag_prof_app(HM.put_agent(HM.strip(a0, n), n, x), HM.strip(r, n)),
      Equal.trans(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, HI.profile(HM.put_agent(HM.strip(a0, n), n, x)), HI.profile(HM.strip(r, n))), List.append(&2, List<&2, String>, HM.snoc(List<&2, String>, HI.profile(HM.strip(a0, n)), HI.names(x)), HI.profile(HM.strip(r, n))), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), [n] <> List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{})),
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, z, HI.profile(HM.strip(r, n))), HI.profile(HM.put_agent(HM.strip(a0, n), n, x)), HM.snoc(List<&2, String>, HI.profile(HM.strip(a0, n)), HI.names(x)), PL2.hub_put_absent(HM.strip(a0, n), n, x, ag_free_l(HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n)), n, ag_free_eq(List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), HI.profile(HM.strip(List.append(&2, HS.Agent, a0, r), n)), n, Equal.sym(List<&2, List<&2, String>>, HI.profile(HM.strip(List.append(&2, HS.Agent, a0, r), n)), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), ag_new_eqs(a0, r, n)), PLN.nm_free(List.append(&2, HS.Agent, a0, r), n, hn))))),
      Equal.trans(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, HM.snoc(List<&2, String>, HI.profile(HM.strip(a0, n)), HI.names(x)), HI.profile(HM.strip(r, n))), List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), [HI.names(x)]), HI.profile(HM.strip(r, n))), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), [n] <> List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{})),
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, z, HI.profile(HM.strip(r, n))), HM.snoc(List<&2, String>, HI.profile(HM.strip(a0, n)), HI.names(x)), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), [HI.names(x)]), ag_snoc(List<&2, String>, HI.profile(HM.strip(a0, n)), HI.names(x))),
      Equal.trans(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), [HI.names(x)]), HI.profile(HM.strip(r, n))), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.names(x) <> HI.profile(HM.strip(r, n))), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), [n] <> List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{})),
      ag_assoc(List<&2, String>, HI.profile(HM.strip(a0, n)), [HI.names(x)], HI.profile(HM.strip(r, n))),
      Equal.trans(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.names(x) <> HI.profile(HM.strip(r, n))), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.names(x) <> List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{})), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), [n] <> List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{})),
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.names(x) <> z), HI.profile(HM.strip(r, n)), List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{}), Equal.sym(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{}), HI.profile(HM.strip(r, n)), ag_nil(List<&2, String>, HI.profile(HM.strip(r, n))))),
      Equal.cong(List<&2, String>, List<&2, List<&2, String>>, z => List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), z <> List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), Nil{})), HI.names(x), [n], en)))))),
        ag_slide_l(HI.profile(HM.strip(a0, n)), [n], HI.profile(HM.strip(r, n)), Nil{}, ag_tr(List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), [[n]])), HM.snoc(List<&2, String>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), [n]), Equal.trans(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), [[n]])), List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), [[n]]), HM.snoc(List<&2, String>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), [n]),
      Equal.sym(List<&2, List<&2, String>>, List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), [[n]]), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), List.append(&2, List<&2, String>, HI.profile(HM.strip(r, n)), [[n]])), ag_assoc(List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n)), [[n]])),
      Equal.sym(List<&2, List<&2, String>>, HM.snoc(List<&2, String>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), [n]), List.append(&2, List<&2, String>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), [[n]]), ag_snoc(List<&2, String>, List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), [n]))), PL2.hub_nok_snoc(List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), [n], ag_tr(List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), HI.profile(HM.strip(List.append(&2, HS.Agent, a0, r), n)), Equal.sym(List<&2, List<&2, String>>, HI.profile(HM.strip(List.append(&2, HS.Agent, a0, r), n)), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), ag_new_eqs(a0, r, n)), PLN.nm_ok(List.append(&2, HS.Agent, a0, r), n, u)), PL2.hub_allapart_new(List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), n, ag_free_eq(List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), HI.profile(HM.strip(List.append(&2, HS.Agent, a0, r), n)), n, Equal.sym(List<&2, List<&2, String>>, HI.profile(HM.strip(List.append(&2, HS.Agent, a0, r), n)), List.append(&2, List<&2, String>, HI.profile(HM.strip(a0, n)), HI.profile(HM.strip(r, n))), ag_new_eqs(a0, r, n)), PLN.nm_free(List.append(&2, HS.Agent, a0, r), n, hn)))))))

# ---- the split: no archived agent in St.agents, only archived ones in arch ----

def lv_eq(-x: List<&2, HS.Agent>, -y: List<&2, HS.Agent>, -v: Bool, e: {x == y : List<&2, HS.Agent>}, d: HI.Lives(y, v)) -> HI.Lives(x, v):
  %Equal.sym(List<&2, HS.Agent>, x, y, e) : HI.Lives(_, v)
  d

def lv_split.add(-a: HS.Agent, -t: List<&2, HS.Agent>, -r: List<&2, HS.Agent>, -v: Bool, a1: {HM.live_life(HS.Agent.life(a)) == v : Bool}, b: HX.Both<HI.Lives(t, v), HI.Lives(r, v)>)
    -> HX.Both<HI.Lives(a <> t, v), HI.Lives(r, v)>:
  match b:
    case HX.Both{b1, b2}:
      HX.Both{HX.Both{a1, b1}, b2}

def lv_split(+q: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +v: Bool, l: HI.Lives(List.append(&2, HS.Agent, q, r), v)) -> HX.Both<HI.Lives(q, v), HI.Lives(r, v)>:
  match q:
    case Nil{}:
      HX.Both{Unit{}, l}
    case Con{a, t}:
      match l:
        case HX.Both{l1, l2}:
          lv_split.add(a, t, r, v, l1, lv_split(t, r, v, l2))

def lv_join(+q: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +v: Bool, a: HI.Lives(q, v), b: HI.Lives(r, v)) -> HI.Lives(List.append(&2, HS.Agent, q, r), v):
  match q:
    case Nil{}:
      b
    case Con{x, t}:
      match a:
        case HX.Both{a1, a2}:
          HX.Both{a1, lv_join(t, r, v, a2, b)}

def lv_is_life(op: HM.AgentOp) -> Bool:
  match op:
    case HM.OLife{l, why}:
      True{}
    case _:
      False{}

# an operation that is not a lifecycle keeps the agent's life
def lv_life_op(+h: HS.Agent, op: HM.AgentOp, nl: {lv_is_life(op) == False{} : Bool}) -> {HS.Agent.life(HM.agent_op(h, op)) == HS.Agent.life(h) : HS.Life}:
  match op:
    case HM.OLife{l, why}:
      AB.absurd_eq(AB.tf_absurd(nl), HS.Life, HS.Agent.life(HM.agent_op(h, HM.OLife{l, why})), HS.Agent.life(h))
    case HM.OWs{w}:
      %HSL.Agent.ws.eta(h, w) : {HS.Agent.life(_) == HS.Agent.life(h) : HS.Life}
      {==}
    case HM.ODeclared{d, n}:
      %HSL.Agent.declared.eta(h, d) : {HS.Agent.life(HS.Agent.with_note(_, n)) == HS.Agent.life(h) : HS.Life}
      {==}
    case HM.OFile{+p}:
      %HSL.Agent.files.eta(h, HM.agent_op.file(HM.str_in(HS.Agent.files(h), p), HS.Agent.files(h), p)) : {HS.Agent.life(_) == HS.Agent.life(h) : HS.Life}
      {==}
    case HM.OUnclaim{+p}:
      %HSL.Agent.files.eta(h, HN.drop_str(HS.Agent.files(h), p)) : {HS.Agent.life(_) == HS.Agent.life(h) : HS.Life}
      {==}
    case HM.ORename{nn}:
      %HSL.Agent.name.eta(h, nn) : {HS.Agent.life(HS.Agent.with_aliases(_, HM.snoc(String, HS.Agent.aliases(h), HS.Agent.name(h)))) == HS.Agent.life(h) : HS.Life}
      {==}
    case HM.OSnap{x}:
      %HSL.Agent.snap.eta(h, x) : {HS.Agent.life(_) == HS.Agent.life(h) : HS.Life}
      {==}
    case HM.OReport{x}:
      %HSL.Agent.report.eta(h, x) : {HS.Agent.life(_) == HS.Agent.life(h) : HS.Life}
      {==}
    case HM.OFollow{x}:
      %HSL.Agent.follow.eta(h, x) : {HS.Agent.life(_) == HS.Agent.life(h) : HS.Life}
      {==}

# a lifecycle gives the agent its life
def lv_life_set(+h: HS.Agent, +l: HS.Life, +why: Maybe<&2, String>) -> {HM.live_life(HS.Agent.life(HM.agent_op(h, HM.OLife{l, why}))) == HM.live_life(l) : Bool}:
  %HSL.Agent.life.eta(h, l) : {HM.live_life(HS.Agent.life(HS.Agent.with_failure(HS.Agent.with_declared(_, HM.agent_op.decl(l, HS.Agent.declared(h))), why))) == HM.live_life(l) : Bool}
  {==}

def lv_map.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, +v: Bool, +nl: {lv_is_life(op) == False{} : Bool},
    l1: {HM.live_life(HS.Agent.life(h)) == v : Bool}, l2: HI.Lives(t, v), ih: HI.Lives(HM.map_agent(t, n, op), v)) -> HI.Lives(HM.map_agent.go(h <> t, c, n, op), v):
  match c:
    case True{}:
      %Equal.sym(HS.Life, HS.Agent.life(HM.agent_op(h, op)), HS.Agent.life(h), lv_life_op(h, op, nl)) : HX.Both<{HM.live_life(_) == v : Bool}, HI.Lives(t, v)>
      HX.Both{l1, l2}
    case False{}:
      HX.Both{l1, ih}

def lv_map(+xs: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, +v: Bool, +nl: {lv_is_life(op) == False{} : Bool}, l: HI.Lives(xs, v)) -> HI.Lives(HM.map_agent(xs, n, op), v):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match l:
        case HX.Both{l1, +l2}:
          lv_map.pick(String.eq(HS.Agent.name(h), n), h, t, n, op, v, nl, l1, l2, lv_map(t, n, op, v, nl, l2))

def lv_strip(+xs: List<&2, HS.Agent>, +n: String, +v: Bool, l: HI.Lives(xs, v)) -> HI.Lives(HM.strip(xs, n), v):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match l:
        case HX.Both{l1, l2}:
          %HSL.Agent.aliases.eta(h, HN.drop_str(HS.Agent.aliases(h), n)) : HX.Both<{HM.live_life(HS.Agent.life(_)) == v : Bool}, HI.Lives(HM.strip(t, n), v)>
          HX.Both{l1, lv_strip(t, n, v, l2)}

def lv_put.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String, +a: HS.Agent, +v: Bool, la: {HM.live_life(HS.Agent.life(a)) == v : Bool},
    l1: {HM.live_life(HS.Agent.life(h)) == v : Bool}, l2: HI.Lives(t, v), ih: HI.Lives(HM.put_agent(t, n, a), v))
    -> HI.Lives(HLZ.pick(List<&2, HS.Agent>, c, u => a <> t, u => h <> HM.put_agent(t, n, a)), v):
  match c:
    case True{}:
      HX.Both{la, l2}
    case False{}:
      HX.Both{l1, ih}

# a new agent (replaced or added) of the side's life
def lv_put(+xs: List<&2, HS.Agent>, +n: String, +a: HS.Agent, +v: Bool, +la: {HM.live_life(HS.Agent.life(a)) == v : Bool}, l: HI.Lives(xs, v)) -> HI.Lives(HM.put_agent(xs, n, a), v):
  match xs:
    case Nil{}:
      HX.Both{la, Unit{}}
    case Con{h, t}:
      match l:
        case HX.Both{l1, +l2}:
          lv_put.pick(String.eq(HS.Agent.name(h), n), h, t, n, a, v, la, l1, l2, lv_put(t, n, a, v, la, l2))

def lv_ins.go(-xs: List<&2, HS.Agent>, +t: List<&2, HS.Agent>, +d: List<&2, HS.Agent>, +a: HS.Agent, +v: Bool, la: {HM.live_life(HS.Agent.life(a)) == v : Bool}, b: HX.Both<HI.Lives(t, v), HI.Lives(d, v)>)
    -> HI.Lives(List.append(&2, HS.Agent, t, a <> d), v):
  match b:
    case HX.Both{b1, b2}:
      lv_join(t, a <> d, v, b1, HX.Both{la, b2})

# an agent in its creation place among agents of the same life
def lv_ins(+ord: List<&2, String>, +xs: List<&2, HS.Agent>, +a: HS.Agent, +v: Bool, la: {HM.live_life(HS.Agent.life(a)) == v : Bool}, l: HI.Lives(xs, v)) -> HI.Lives(HAG.ins(ord, xs, a), v):
  lv_ins.go(xs, List.take(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), List.drop(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), a, v, la,
    lv_split(List.take(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), List.drop(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), v, lv_eq(List.append(&2, HS.Agent, List.take(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), List.drop(&2, HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a)))), xs, v, ag_take_drop(HS.Agent, xs, HAG.place(ord, xs, HS.Agent.name(a))), l)))

def lv_drop_of.m(m: Maybe<&2, HS.Agent>, +p: List<&2, HS.Agent>, +q: List<&2, HS.Agent>, +v: Bool, b: HX.Both<HI.Lives(p, v), HI.Lives(HAG.whole.of(m, q), v)>) -> HI.Lives(List.append(&2, HS.Agent, p, q), v):
  match m:
    case None{}:
      match b:
        case HX.Both{b1, b2}:
          lv_join(p, q, v, b1, b2)
    case Some{x}:
      match b:
        case HX.Both{b1, b2}:
          match b2:
            case HX.Both{c1, c2}:
              lv_join(p, q, v, b1, c2)

# a list without one of its agents
# a list without its first agent named `n`
def lv_drop(+xs: List<&2, HS.Agent>, +n: String, +v: Bool, l: HI.Lives(xs, v)) -> HI.Lives(HAG.drop(xs, n), v):
  lv_drop_of.m(HAG.hit(xs, n), HAG.pre(xs, n), HAG.post(xs, n), v,
    lv_split(HAG.pre(xs, n), HAG.whole.of(HAG.hit(xs, n), HAG.post(xs, n)), v, lv_eq(HAG.whole(HAG.pre(xs, n), HAG.hit(xs, n), HAG.post(xs, n)), xs, v, ag_cut_whole(xs, n), l)))

# ---- each event keeps the split ----

def lv_on(c: Bool, +a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, +nl: {lv_is_life(op) == False{} : Bool}, la: HI.Lives(a, True{}), lr: HI.Lives(r, False{}))
    -> HX.Both<HI.Lives(HM.on_agent.go(c, True{}, a, r, n, op), True{}), HI.Lives(HM.on_agent.go(c, False{}, a, r, n, op), False{})>:
  match c:
    case True{}:
      HX.Both{lv_map(a, n, op, True{}, nl, la), lr}
    case False{}:
      HX.Both{la, lv_map(r, n, op, False{}, nl, lr)}

def lv_placed(ok: Bool, +a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +ws: J.Json, la: HI.Lives(a, True{}), lr: HI.Lives(r, False{}))
    -> HX.Both<HI.Lives(HM.placed.go(ok, True{}, a, r, n, ws), True{}), HI.Lives(HM.placed.go(ok, False{}, a, r, n, ws), False{})>:
  match ok:
    case True{}:
      lv_on(HAG.has(a, n), a, r, n, HM.OWs{ws}, {==}, la, lr)
    case False{}:
      HX.Both{la, lr}

def lv_live_to(stay: Bool, +a2: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, hl: {HM.live_life(HS.Agent.life(a2)) == stay : Bool},
    lp: HI.Lives(pre, True{}), lq: HI.Lives(post, True{}), lr: HI.Lives(r, False{})) -> HX.Both<HI.Lives(HM.life.live.to(stay, True{}, a2, pre, post, r), True{}), HI.Lives(HM.life.live.to(stay, False{}, a2, pre, post, r), False{})>:
  match stay:
    case True{}:
      HX.Both{lv_join(pre, a2 <> post, True{}, lp, HX.Both{hl, lq}), lr}
    case False{}:
      HX.Both{lv_join(pre, post, True{}, lp, lq), HX.Both{hl, lr}}

def lv_arch_to(back: Bool, +a2: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +xs: List<&2, HS.Agent>, +ord: List<&2, String>, hl: {HM.live_life(HS.Agent.life(a2)) == back : Bool},
    lp: HI.Lives(pre, False{}), lq: HI.Lives(post, False{}), la: HI.Lives(xs, True{})) -> HX.Both<HI.Lives(HM.life.arch.to(back, True{}, a2, pre, post, xs, ord), True{}), HI.Lives(HM.life.arch.to(back, False{}, a2, pre, post, xs, ord), False{})>:
  match back:
    case True{}:
      HX.Both{lv_ins(ord, xs, a2, True{}, hl, la), lv_join(pre, post, False{}, lp, lq)}
    case False{}:
      HX.Both{la, lv_join(pre, a2 <> post, False{}, lp, HX.Both{hl, lq})}

def lv_cut3(-pre: List<&2, HS.Agent>, -a: HS.Agent, -post: List<&2, HS.Agent>, -v: Bool, b: HX.Both<HI.Lives(pre, v), HI.Lives(a <> post, v)>) -> HX.Both<HI.Lives(pre, v), HI.Lives(post, v)>:
  match b:
    case HX.Both{b1, b2}:
      match b2:
        case HX.Both{c1, c2}:
          HX.Both{b1, c2}

def lv_arch.go(+a2: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +xs: List<&2, HS.Agent>, +ord: List<&2, String>, +l: HS.Life, hl: {HM.live_life(HS.Agent.life(a2)) == HM.live_life(l) : Bool},
    b: HX.Both<HI.Lives(pre, False{}), HI.Lives(post, False{})>, la: HI.Lives(xs, True{})) -> HX.Both<HI.Lives(HM.life.arch.to(HM.live_life(l), True{}, a2, pre, post, xs, ord), True{}), HI.Lives(HM.life.arch.to(HM.live_life(l), False{}, a2, pre, post, xs, ord), False{})>:
  match b:
    case HX.Both{lp, lq}:
      lv_arch_to(HM.live_life(l), a2, pre, post, xs, ord, hl, lp, lq, la)

def lv_arch(m: Maybe<&2, HS.Agent>, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +xs: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +ord: List<&2, String>, +l: HS.Life, +why: Maybe<&2, String>, w: {HAG.whole(pre, m, post) == r : List<&2, HS.Agent>},
    la: HI.Lives(xs, True{}), lr: HI.Lives(r, False{})) -> HX.Both<HI.Lives(HM.life.arch(m, True{}, pre, post, xs, r, ord, l, why), True{}), HI.Lives(HM.life.arch(m, False{}, pre, post, xs, r, ord, l, why), False{})>:
  match m:
    case None{}:
      HX.Both{la, lr}
    case Some{+a}:
      lv_arch.go(HM.agent_op(a, HM.OLife{l, why}), pre, post, xs, ord, l, lv_life_set(a, l, why),
        lv_cut3(pre, a, post, False{}, lv_split(pre, a <> post, False{}, lv_eq(List.append(&2, HS.Agent, pre, a <> post), r, False{}, w, lr))), la)

def lv_live.go(+a2: HS.Agent, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +l: HS.Life, hl: {HM.live_life(HS.Agent.life(a2)) == HM.live_life(l) : Bool},
    b: HX.Both<HI.Lives(pre, True{}), HI.Lives(post, True{})>, lr: HI.Lives(r, False{})) -> HX.Both<HI.Lives(HM.life.live.to(HM.live_life(l), True{}, a2, pre, post, r), True{}), HI.Lives(HM.life.live.to(HM.live_life(l), False{}, a2, pre, post, r), False{})>:
  match b:
    case HX.Both{lp, lq}:
      lv_live_to(HM.live_life(l), a2, pre, post, r, hl, lp, lq, lr)

def lv_life(m: Maybe<&2, HS.Agent>, +pre: List<&2, HS.Agent>, +post: List<&2, HS.Agent>, +xs: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +ord: List<&2, String>, +n: String, +l: HS.Life, +why: Maybe<&2, String>, w: {HAG.whole(pre, m, post) == xs : List<&2, HS.Agent>},
    la: HI.Lives(xs, True{}), lr: HI.Lives(r, False{})) -> HX.Both<HI.Lives(HM.life.live(m, True{}, pre, post, xs, r, ord, n, l, why), True{}), HI.Lives(HM.life.live(m, False{}, pre, post, xs, r, ord, n, l, why), False{})>:
  match m:
    case None{}:
      lv_arch(HAG.hit(r, n), HAG.pre(r, n), HAG.post(r, n), xs, r, ord, l, why, ag_cut_whole(r, n), la, lr)
    case Some{+a}:
      lv_live.go(HM.agent_op(a, HM.OLife{l, why}), pre, post, r, l, lv_life_set(a, l, why),
        lv_cut3(pre, a, post, True{}, lv_split(pre, a <> post, True{}, lv_eq(List.append(&2, HS.Agent, pre, a <> post), xs, True{}, w, la))), lr)


# a new task (a live agent): last of St.agents, or in the place of an
# archived agent of the same name (an older journal)
def lv_new(c: Bool, +xs: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +ord: List<&2, String>, +n: String, +a: HS.Agent, +ha: {HM.live_life(HS.Agent.life(a)) == True{} : Bool},
    la: HI.Lives(xs, True{}), lr: HI.Lives(r, False{})) -> HX.Both<HI.Lives(HM.new_task.go(c, True{}, xs, r, ord, n, a), True{}), HI.Lives(HM.new_task.go(c, False{}, xs, r, ord, n, a), False{})>:
  match c:
    case False{}:
      HX.Both{lv_put(HM.strip(xs, n), n, a, True{}, ha, lv_strip(xs, n, True{}, la)), lv_strip(r, n, False{}, lr)}
    case True{}:
      HX.Both{lv_ins(ord, HM.strip(xs, n), a, True{}, ha, lv_strip(xs, n, True{}, la)), lv_drop(HM.strip(r, n), n, False{}, lv_strip(r, n, False{}, lr))}

# every event keeps the split, on the state opened by name (S.St.open)
def lv_apply.go(+e: HM.Ev, +s: HS.St, la: HI.Lives(HS.St.agents(s), True{}), lr: HI.Lives(HS.St.arch(s), False{})) -> HI.AgentsSplit(HM.apply(HS.St.open(s), e)):
  match e:
    case HM.ETaskCreated{+n, pa, br, ws, at}:
      lv_new(HAG.has(HS.St.arch(s), n), HS.St.agents(s), HS.St.arch(s), HS.St.order(s), n,
        HM.agent_of_new(HM.ETaskCreated{n, pa, br, ws, at}, HN.free_dir(HM.dirs_of(List.append(&2, HS.Agent, HS.St.agents(s), HS.St.arch(s))), n)), {==}, la, lr)
    case HM.ERenamed{+old, +new}:
      lv_on(HAG.has(HM.strip(HS.St.agents(s), new), old), HM.strip(HS.St.agents(s), new), HM.strip(HS.St.arch(s), new), old, HM.ORename{new}, {==},
        lv_strip(HS.St.agents(s), new, True{}, la), lv_strip(HS.St.arch(s), new, False{}, lr))
    case HM.ELifecycle{+x, +l, +r}:
      lv_life(HAG.hit(HS.St.agents(s), x), HAG.pre(HS.St.agents(s), x), HAG.post(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), HS.St.order(s), x, l, r, ag_cut_whole(HS.St.agents(s), x), la, lr)
    case HM.EWorkspace{+x, +w}:
      lv_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OWs{w}, {==}, la, lr)
    case HM.EDeclared{+x, +d, +nt}:
      lv_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.ODeclared{d, nt}, {==}, la, lr)
    case HM.EFileTouched{+x, +p}:
      lv_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OFile{p}, {==}, la, lr)
    case HM.EClaimDropped{+x, +p}:
      lv_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OUnclaim{p}, {==}, la, lr)
    case HM.ESnapshot{+x, +sn}:
      lv_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OSnap{sn}, {==}, la, lr)
    case HM.EPlaced{+x, +w, +er}:
      lv_placed(String.is_empty(er), HS.St.agents(s), HS.St.arch(s), x, w, la, lr)
    case HM.EReported{+x, +rp}:
      lv_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OReport{rp}, {==}, la, lr)
    case HM.EFollow{+x, +f, +h}:
      lv_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OFollow{Bool.pick(Maybe<&2, String>, f, Some{h}, None{})}, {==}, la, lr)
    case HM.EJobEnd{+x, b}:
      lv_on(HAG.has(HS.St.agents(s), x), HS.St.agents(s), HS.St.arch(s), x, HM.OFollow{None{}}, {==}, la, lr)
    case HM.EMsgsMoved{a, b}:
      HX.Both{la, lr}
    case HM.EMsgSent{m}:
      HX.Both{la, lr}
    case HM.EMsgState{a, b}:
      HX.Both{la, lr}
    case HM.ESettled{a}:
      HX.Both{la, lr}
    case HM.ECardOpened{c}:
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

def lv_apply(+s: HS.St, e: HM.Ev, sp: HI.AgentsSplit(s)) -> HI.AgentsSplit(HM.apply(s, e)):
  match sp:
    case HX.Both{la, lr}:
      %HSL.St.open_eq(s) : HI.AgentsSplit(HM.apply(_, e))
      lv_apply.go(e, s, la, lr)

def lv_apply_all(+evs: List<&2, HM.Ev>, +s: HS.St, sp: HI.AgentsSplit(s)) -> HI.AgentsSplit(HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      sp
    case Con{e, t}:
      lv_apply_all(t, HM.apply(s, e), lv_apply(s, e, sp))

def lv_tr(-a: HS.St, -b: HS.St, e: {a == b : HS.St}, sp: HI.AgentsSplit(b)) -> HI.AgentsSplit(a):
  %Equal.sym(HS.St, a, b, e) : HI.AgentsSplit(_)
  sp

def lv_replay_one(ok: Bool, +st: HS.St, +e: HM.Ev, sp: HI.AgentsSplit(st)) -> HI.AgentsSplit(CO.replay_one(ok, st, e)):
  match ok:
    case True{}:
      lv_apply(st, e, sp)
    case False{}:
      sp

# a boot replay's batch keeps the split (any events: no Fresh)
def lv_replay_batch(+ms: List<&2, Maybe<&2, HM.Ev>>, +st: HS.St, +i: Nat, +sk: List<&2, Nat>, sp: HI.AgentsSplit(st))
    -> HI.AgentsSplit(Pair.fst(HS.St, List<&2, Nat>, CO.replay_batch(ms, st, i, sk))):
  match ms:
    case Nil{}:
      sp
    case Con{m, rest}:
      match m:
        case None{}:
          lv_replay_batch(rest, st, 1n+i, i <> sk, sp)
        case Some{e}:
          lv_replay_batch(rest, CO.replay_one(CO.replay_fresh(st, e), st, e), 1n+i,
            Bool.pick(List<&2, Nat>, CO.replay_fresh(st, e), sk, i <> sk),
            lv_replay_one(CO.replay_fresh(st, e), st, e, sp))

# ---- no name on both sides (law agents_apart_sides) ----

def sd_aa_eq(-p: List<&2, String>, -x: List<&2, List<&2, String>>, -y: List<&2, List<&2, String>>, e: {x == y : List<&2, List<&2, String>>}, d: HI.ApartAll(p, y)) -> HI.ApartAll(p, x):
  %Equal.sym(List<&2, List<&2, String>>, x, y, e) : HI.ApartAll(p, _)
  d

def sd_neq(+m: String, +k: String, +n: String, e: {k == n : String}, d: {String.eq(m, k) == False{} : Bool}) -> {String.eq(m, n) == False{} : Bool}:
  %e : {String.eq(m, _) == False{} : Bool}
  d

def sd_snd(-x: Data, -y: Data, b: HX.Both<x, y>) -> y:
  match b:
    case HX.Both{b1, b2}:
      b2

# an agent named `n`, apart from every agent of `r`: none of them is named `n`
def sd_aa_has(+h: HS.Agent, +r: List<&2, HS.Agent>, +n: String, +e: {HS.Agent.name(h) == n : String}, aa: HI.ApartAll(HI.names(h), HI.profile(r))) -> {HAG.has(r, n) == False{} : Bool}:
  match r:
    case Nil{}:
      {==}
    case Con{b, t}:
      match aa:
        case HX.Both{a1, a2}:
          match a1:
            case HX.Both{c1, c2}:
              PL1.hub_lor_false(String.eq(HS.Agent.name(b), n), HAG.has(t, n),
                sd_neq(HS.Agent.name(b), HS.Agent.name(h), n, e, PL1.hub_lor_false_l(String.eq(HS.Agent.name(b), HS.Agent.name(h)), HM.str_in(HS.Agent.aliases(b), HS.Agent.name(h)), c1)),
                sd_aa_has(h, t, n, e, a2))

def sd_pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, +e: {String.eq(HS.Agent.name(h), n) == c : Bool},
    aa: HI.ApartAll(HI.names(h), HI.profile(List.append(&2, HS.Agent, t, r))), k: {HLZ.or(c, u => HAG.has(t, n)) == True{} : Bool},
    ih: @p: {HAG.has(t, n) == True{} : Bool} -> {HAG.has(r, n) == False{} : Bool}) -> {HAG.has(r, n) == False{} : Bool}:
  match c:
    case True{}:
      sd_aa_has(h, r, n, PL2.hub_str_eq_sound(HS.Agent.name(h), n, e),
        sd_snd(HI.ApartAll(HI.names(h), HI.profile(t)), HI.ApartAll(HI.names(h), HI.profile(r)),
          ag_aa_split(HI.names(h), HI.profile(t), HI.profile(r),
            sd_aa_eq(HI.names(h), List.append(&2, List<&2, String>, HI.profile(t), HI.profile(r)), HI.profile(List.append(&2, HS.Agent, t, r)),
              Equal.sym(List<&2, List<&2, String>>, HI.profile(List.append(&2, HS.Agent, t, r)), List.append(&2, List<&2, String>, HI.profile(t), HI.profile(r)), ag_prof_app(t, r)), aa))))
    case False{}:
      ih(k)

# a live agent's name is no archived agent's (names apart, both sides)
def sd_has(+a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, u: HI.NamesOK(HI.profile(List.append(&2, HS.Agent, a, r))), k: {HAG.has(a, n) == True{} : Bool}) -> {HAG.has(r, n) == False{} : Bool}:
  match a:
    case Nil{}:
      AB.absurd_eq(AB.tf_absurd(Equal.sym(Bool, False{}, True{}, k)), Bool, HAG.has(r, n), False{})
    case Con{h, t}:
      match u:
        case HX.Both{u1, +u2}:
          sd_pick(String.eq(HS.Agent.name(h), n), h, t, r, n, {==}, u1, k, p => sd_has(t, r, n, u2, p))

def sd_sides.go(c: Bool, +a: List<&2, HS.Agent>, +r: List<&2, HS.Agent>, +n: String, e: {HAG.has(a, n) == c : Bool}, u: HI.NamesOK(HI.profile(List.append(&2, HS.Agent, a, r))))
    -> {Bool.and(c, HAG.has(r, n)) == False{} : Bool}:
  match c:
    case False{}:
      {==}
    case True{}:
      %Equal.sym(Bool, HAG.has(r, n), False{}, sd_has(a, r, n, u, e)) : {Bool.and(True{}, _) == False{} : Bool}
      {==}

def sd_sides(+s: HS.St, +n: String, u: HI.NamesApart(s)) -> {Bool.and(HAG.has(HS.St.agents(s), n), HAG.has(HS.St.arch(s), n)) == False{} : Bool}:
  sd_sides.go(HAG.has(HS.St.agents(s), n), HS.St.agents(s), HS.St.arch(s), n, {==}, u)
