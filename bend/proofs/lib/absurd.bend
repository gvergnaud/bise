# bend/proofs/lib/absurd.bend: the absurd cases every proofs file shares (from lib/hub-1.bend).
# It imports nothing of the hub, so a proofs file that only needs these (models.bend) has no
# hub file in its imports' closure, and a hub change never re-checks it (tests/proof_run.py,
# c3-quick-90s).
import Base

def bdisc(b: Bool) -> Type:
  match b:
    case True{}:
      Unit
    case False{}:
      Empty

def ft_absurd(e: {False{} == True{} : Bool}) -> Empty:
  %Equal.sym(Bool, False{}, True{}, e) : bdisc(_)
  Unit{}

def absurd_eq(x: Empty, -A: Type, -a: A, -b: A) -> {a == b : A}:
  match x:

def tf_absurd(e: {True{} == False{} : Bool}) -> Empty:
  %e : bdisc(_)
  Unit{}
