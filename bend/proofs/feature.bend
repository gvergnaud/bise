# bend/proofs/feature.bend: the proofs of bend/laws/feature.bend (moved
# from bend/proofs/hub.bend). Each holds by unfolding the definitions
# involved; the checker verifies the equalities.
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/feature.bend as HFT
import ./lib/hub-1.bend as PL1
import ./lib/absurd.bend as AB
import ./lib/hub-2.bend as PL2
import ../laws/feature.bend as Laws

# ---- `sb feature retire` (hub/feature.bend) ----

# a workspace's entries after the retire hold no `feature`
def hub_unfeature_none.pick(c: Bool, +k: String, +v: J.Json, +t: List<&2, Sigma<&2, &2, String, _ => J.Json>>,
    +e: {String.eq(k, "feature") == c : Bool},
    ih: {J.get_kv(HFT.unfeature.kvs(t), "feature") == None{} : Maybe<&2, J.Json>})
    -> {J.get_kv(Bool.pick(List<&2, Sigma<&2, &2, String, _ => J.Json>>, String.eq(k, "feature"), HFT.unfeature.kvs(t), Tuple{k, v} <> HFT.unfeature.kvs(t)), "feature") == None{} : Maybe<&2, J.Json>}:
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(k, "feature"), True{}, e) : {J.get_kv(Bool.pick(List<&2, Sigma<&2, &2, String, _ => J.Json>>, _, HFT.unfeature.kvs(t), Tuple{k, v} <> HFT.unfeature.kvs(t)), "feature") == None{} : Maybe<&2, J.Json>}
      ih
    case False{}:
      %Equal.sym(Bool, String.eq(k, "feature"), False{}, e) : {J.get_kv(Bool.pick(List<&2, Sigma<&2, &2, String, _ => J.Json>>, _, HFT.unfeature.kvs(t), Tuple{k, v} <> HFT.unfeature.kvs(t)), "feature") == None{} : Maybe<&2, J.Json>}
      %Equal.sym(Bool, String.eq(k, "feature"), False{}, e) : {Bool.pick(Maybe<&2, J.Json>, _, Some{v}, J.get_kv(HFT.unfeature.kvs(t), "feature")) == None{} : Maybe<&2, J.Json>}
      ih

def hub_unfeature_none(+kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> {J.get_kv(HFT.unfeature.kvs(kvs), "feature") == None{} : Maybe<&2, J.Json>}:
  match kvs:
    case Nil{}:
      {==}
    case Tuple{k, v} <> t:
      hub_unfeature_none.pick(String.eq(k, "feature"), k, v, t, {==}, hub_unfeature_none(t))

# no `feature`: the workspace lands on no feature
def hub_feature_is_none(+m: Maybe<&2, J.Json>, -rest: Bool, +name: String, e: {m == None{} : Maybe<&2, J.Json>})
    -> {Bool.and(HFT.feature_is(m, name), rest) == False{} : Bool}:
  %Equal.sym(Maybe<&2, J.Json>, m, None{}, e) : {Bool.and(HFT.feature_is(_, name), rest) == False{} : Bool}
  {==}

def Laws.feature_retire_clears(ws, name):
  match ws:
    case J.JObj{kvs}:
      +w = {J.JObj{HFT.unfeature.kvs(kvs)} : J.Json}
      hub_feature_is_none(J.get_kv(HFT.unfeature.kvs(kvs), "feature"), Bool.and(HM.ws_worktree(w), Bool.not(HM.ws_dropped(w))), name, hub_unfeature_none(kvs))
    case J.JNull{}:
      {==}
    case J.JBool{b}:
      {==}
    case J.JNum{n}:
      {==}
    case J.JNeg{n}:
      {==}
    case J.JStr{s}:
      {==}
    case J.JArr{xs}:
      {==}
    case J.JRaw{r}:
      {==}

# the other keys read the same after the retire
def hub_keeps_ws.pick(c: Bool, +kk: String, +v: J.Json, +t: List<&2, Sigma<&2, &2, String, _ => J.Json>>, +k: String,
    +ec: {String.eq(kk, "feature") == c : Bool}, e: {String.eq(k, "feature") == False{} : Bool},
    ih: {J.get_kv(HFT.unfeature.kvs(t), k) == J.get_kv(t, k) : Maybe<&2, J.Json>})
    -> {J.get_kv(Bool.pick(List<&2, Sigma<&2, &2, String, _ => J.Json>>, String.eq(kk, "feature"), HFT.unfeature.kvs(t), Tuple{kk, v} <> HFT.unfeature.kvs(t)), k)
        == Bool.pick(Maybe<&2, J.Json>, String.eq(kk, k), Some{v}, J.get_kv(t, k)) : Maybe<&2, J.Json>}:
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(kk, "feature"), True{}, ec) : {J.get_kv(Bool.pick(List<&2, Sigma<&2, &2, String, _ => J.Json>>, _, HFT.unfeature.kvs(t), Tuple{kk, v} <> HFT.unfeature.kvs(t)), k) == Bool.pick(Maybe<&2, J.Json>, String.eq(kk, k), Some{v}, J.get_kv(t, k)) : Maybe<&2, J.Json>}
      %Equal.sym(String, kk, "feature", PL2.hub_str_eq_sound(kk, "feature", ec)) : {J.get_kv(HFT.unfeature.kvs(t), k) == Bool.pick(Maybe<&2, J.Json>, String.eq(_, k), Some{v}, J.get_kv(t, k)) : Maybe<&2, J.Json>}
      %Equal.sym(Bool, String.eq("feature", k), False{}, PL2.hub_str_eq_sym_false(k, "feature", e)) : {J.get_kv(HFT.unfeature.kvs(t), k) == Bool.pick(Maybe<&2, J.Json>, _, Some{v}, J.get_kv(t, k)) : Maybe<&2, J.Json>}
      ih
    case False{}:
      %Equal.sym(Bool, String.eq(kk, "feature"), False{}, ec) : {J.get_kv(Bool.pick(List<&2, Sigma<&2, &2, String, _ => J.Json>>, _, HFT.unfeature.kvs(t), Tuple{kk, v} <> HFT.unfeature.kvs(t)), k) == Bool.pick(Maybe<&2, J.Json>, String.eq(kk, k), Some{v}, J.get_kv(t, k)) : Maybe<&2, J.Json>}
      %Equal.sym(Maybe<&2, J.Json>, J.get_kv(HFT.unfeature.kvs(t), k), J.get_kv(t, k), ih) : {Bool.pick(Maybe<&2, J.Json>, String.eq(kk, k), Some{v}, _) == Bool.pick(Maybe<&2, J.Json>, String.eq(kk, k), Some{v}, J.get_kv(t, k)) : Maybe<&2, J.Json>}
      {==}

def hub_keeps_ws(+kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>, +k: String, +e: {String.eq(k, "feature") == False{} : Bool})
    -> {J.get_kv(HFT.unfeature.kvs(kvs), k) == J.get_kv(kvs, k) : Maybe<&2, J.Json>}:
  match kvs:
    case Nil{}:
      {==}
    case Tuple{kk, v} <> t:
      hub_keeps_ws.pick(String.eq(kk, "feature"), kk, v, t, k, {==}, e, hub_keeps_ws(t, k, e))

def Laws.feature_retire_keeps_ws(kvs, k, e):
  hub_keeps_ws(kvs, k, e)

def Laws.feature_retire_archives_none(ws, name):
  {==}

def Laws.feature_retire_one(o, cx, a, text):
  {==}

def Laws.feature_retire_stops_nothing(o, cx, life, n, text, e):
  match life:
    case HS.LActive{}:
      AB.absurd_eq(AB.tf_absurd(e), HX.Cx<o>, HC.feature_retire.tell(o, HS.LActive{}, cx, n, text), cx)
    case HS.LStopped{}:
      {==}
    case HS.LArchived{}:
      {==}
    case HS.LFailed{}:
      {==}

def Laws.feature_retire_tells_once(o, cx, n, text):
  {==}
