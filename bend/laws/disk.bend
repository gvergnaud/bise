# bend/laws/disk.bend: the hub tells main once when the disk goes under
# 40 GB, and not again until it climbed back over 45 GB (hub/disk.bend,
# disk-hygiene, architect m_56051). Closed by bend/proofs/disk.bend; the
# gate checks every bend/proofs/*.bend.
import Base
import ../hub/disk.bend as DK

# LAW: while low, no reading tells main again, whatever it reads.
law low_never_tells_again:
  for +gb: Nat
  {DK.tells(True{}, gb) == False{} : Bool}

# LAW: from not low, a reading tells main exactly when it makes the disk low.
law tells_on_the_crossing:
  for +gb: Nat
  {DK.tells(False{}, gb) == DK.low(False{}, gb) : Bool}

# LAW: once low, any reading at BACK() GB or more ends the low band.
law back_over_ends_low:
  for +p: Nat
  {DK.low(True{}, Nat.add(DK.BACK(), p)) == False{} : Bool}

# LAW: between LOW() and BACK() a low disk stays low (no second note when it hovers).
law hovering_stays_low:
  {DK.low(True{}, 42n) == True{} : Bool}

# LAW: 39 GB from not low tells; 40 GB does not.
law under_low_tells:
  {DK.tells(False{}, 39n) == True{} : Bool}

law at_low_is_silent:
  {DK.tells(False{}, 40n) == False{} : Bool}
