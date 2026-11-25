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

# ---- wt-disk: the gate of a new checkout (architect m_67917) ----

# LAW: no reading yet (or the reading off): a new checkout goes, as before.
law unknown_disk_goes:
  {DK.gate(None{}) == DK.GGo{} : DK.Gate}

# ---- one bar at 10 GB (his word m_68020, architect m_68023): nothing waits

# LAW: under GATE() GB a new checkout is refused.
law refused_under_10:
  {DK.gate.of(9n) == DK.GRefuse{9n} : DK.Gate}

law refused_at_0:
  {DK.gate.of(0n) == DK.GRefuse{0n} : DK.Gate}

# LAW: at 10 GB it goes (under the old bars it waited at 10 and was refused).
law goes_at_10:
  {DK.gate.of(10n) == DK.GGo{} : DK.Gate}

# LAW: at GATE() GB or more it goes, whatever more.
law goes_at_gate:
  for +p: Nat
  {DK.gate.of(Nat.add(DK.GATE(), p)) == DK.GGo{} : DK.Gate}
