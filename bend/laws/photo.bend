# bend/laws/photo.bend: the hub's photo (hub/photo.bend, issue 36 chunk
# 3). A boot may start from a photo and replay only the events after it:
# these laws say it gives the state a full replay gives. Closed by
# bend/proofs/photo.bend; the gate checks every bend/proofs/*.bend.
import Base
import ../vendor/json.bend as J
import ../hub/photo.bend as P
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/gen/state-codec.bend as HSC
import ../hub/cx.bend as HX
import ../hub/core.bend as HC

# LAW: with no env query pending, the photo is of the durable state (and
# only the durable state: the runtime is not in it).
law photo_is_the_state:
  for +d: HS.St
  for +r: HX.R
  for +f: HC.Phase
  for q: {HX.R.need(r) == None{} : Maybe<&2, J.Json>}
  {P.photo(HC.Hub{d, r, f}) == Some{d} : Maybe<&2, HS.St>}

# LAW: a hub that applied nothing yet, restored from the photo of a
# state, holds that state, its runtime fresh in the same workspace
# (restore_photo(photo(h)) keeps Hub.d), Restored: only restore_more
# follows.
law restore_keeps_the_state:
  for +d: HS.St
  for +x: HS.St
  for +r: HX.R
  {P.restore(HC.Hub{x, r, HC.Fresh{}}, HSC.St.photo_json(d)) == Some{HC.Hub{d, P.fresh(HX.R.workspace(r)), HC.Restored{}}} : Maybe<&2, HC.Hub>}

# LAW: a hub that applied something (a step, a replay) is never restored:
# a photo never replaces a live state (architect m_34907).
law restore_only_fresh:
  for +x: HS.St
  for +r: HX.R
  for +j: J.Json
  {P.restore(HC.Hub{x, r, HC.Live{}}, j) == None{} : Maybe<&2, HC.Hub>}

# LAW: nor restored twice: a second photo never replaces the first.
law restore_once:
  for +x: HS.St
  for +r: HX.R
  for +j: J.Json
  {P.restore(HC.Hub{x, r, HC.Restored{}}, j) == None{} : Maybe<&2, HC.Hub>}

# LAW: restore_more puts a slice's records before the state's (its
# scalars), the runtime kept, still Restored.
law restore_more_prepends:
  for +a: HS.St
  for +x: HS.St
  for +r: HX.R
  {P.restore_more(HC.Hub{x, r, HC.Restored{}}, HSC.St.photo_json(a)) == Some{HC.Hub{HSC.St.slice_join(a, x), r, HC.Restored{}}} : Maybe<&2, HC.Hub>}

# LAW: restore_more only after a restore: never on a fresh hub, never on
# a live one.
law restore_more_not_fresh:
  for +x: HS.St
  for +r: HX.R
  for +j: J.Json
  {P.restore_more(HC.Hub{x, r, HC.Fresh{}}, j) == None{} : Maybe<&2, HC.Hub>}

law restore_more_not_live:
  for +x: HS.St
  for +r: HX.R
  for +j: J.Json
  {P.restore_more(HC.Hub{x, r, HC.Live{}}, j) == None{} : Maybe<&2, HC.Hub>}

# LAW: a state in two slices, the rest restored first, then its first
# k records' photo: the state whole (St.slice_round), so the photo's
# lines restored last first give the photo taken.
law restore_in_slices:
  for +k: Nat
  for +st: HS.St
  for +r: HX.R
  {P.restore_more(HC.Hub{HSC.St.slice_drop(k, st), r, HC.Restored{}}, HSC.St.photo_json(HSC.St.slice_take(k, st))) == Some{HC.Hub{st, r, HC.Restored{}}} : Maybe<&2, HC.Hub>}

# LAW: replaying a journal whole gives the state of replaying its tail
# from the photo of its prefix's state: a boot from a photo taken after
# `p` and a full replay agree, wherever the photo was taken.
law replay_from_photo:
  for +p: List<&2, Maybe<&2, HM.Ev>>
  for +t: List<&2, Maybe<&2, HM.Ev>>
  for +st: HS.St
  {P.replayed(List.append(&2, Maybe<&2, HM.Ev>, p, t), st) == P.replayed(t, HSC.St.photo_of(HSC.St.photo_json(P.replayed(p, st)))) : HS.St}
