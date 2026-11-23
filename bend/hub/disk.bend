# hub/disk.bend — when the hub tells main the disk is low (disk-hygiene,
# point 8 of the disk audit, architect m_55867 / m_56051). The shell reads
# the data volume's free GB on its tick and sends IDiskFree when the whole
# GB changed; the band (low or not) is a sensor fact, runtime only (the
# core's R.last, key KEY()), never journaled: a hub restart while low notes
# main once more. Under LOW() GB it notes main once; it is low again only
# after the disk climbed back over BACK() GB (hysteresis: a disk hovering
# at 40 GB says it once). The last reading also gates a new checkout (a
# worktree, a feature's, a restore's, an isolate's: [`gate`], wt-disk):
# under LOW() GB it waits, under REFUSE() GB it is refused. Pure; its laws
# are in bend/laws/disk.bend (the held ws's in bend/laws/place.bend).
import Base

# the GB under which main is told
def LOW() -> Nat:
  40n

# the GB the disk must reach again before a new crossing tells main
def BACK() -> Nat:
  45n

# the runtime key of the band in the core's R.last (no agent name has a '/')
def KEY() -> String:
  "disk/low"

# low after a reading of gb GB free, from was_low
def low(was_low: Bool, +gb: Nat) -> Bool:
  match was_low:
    case True{}:
      Nat.is_lt(gb, BACK())
    case False{}:
      Nat.is_lt(gb, LOW())

# whether this reading tells main: only the crossing down from not low
def tells(was_low: Bool, +gb: Nat) -> Bool:
  match was_low:
    case True{}:
      False{}
    case False{}:
      Nat.is_lt(gb, LOW())

# ---- the guard on a new checkout (wt-disk, architect m_67917): a worktree
# create, a feature create, a restore or an isolate checks out a whole tree
# (1.3 GB for the dashboard). Under LOW() GB it waits (its ws held,
# place.bend's ws_hold, no fx), under REFUSE() GB it is refused; the last
# reading is kept like the band (runtime, GB_KEY()); no reading yet (or the
# reading off): it goes, as before. A held one starts at a reading at LOW()
# or more, one per reading in creation order; a reading under REFUSE()
# refuses every held one.

# the GB under which a new checkout is refused
def REFUSE() -> Nat:
  20n

# the runtime key of the last reading's whole GB in the core's R.last
def GB_KEY() -> String:
  "disk/gb"

# what a new checkout does at the last reading
type Gate is Data:
  GGo{}
  GWait{gb: Nat}
  GRefuse{gb: Nat}

def gate.wait(low: Bool, +gb: Nat) -> Gate:
  match low:
    case True{}:
      GWait{gb}
    case False{}:
      GGo{}

def gate.refuse(under: Bool, +gb: Nat) -> Gate:
  match under:
    case True{}:
      GRefuse{gb}
    case False{}:
      gate.wait(Nat.is_lt(gb, LOW()), gb)

def gate.of(+gb: Nat) -> Gate:
  gate.refuse(Nat.is_lt(gb, REFUSE()), gb)

# no reading: it goes
def gate(m: Maybe<&2, Nat>) -> Gate:
  match m:
    case None{}:
      GGo{}
    case Some{+gb}:
      gate.of(gb)

# the refusal the spawner (or the /restore, the /isolate) reads
def refused_text(+gb: Nat) -> String:
  "refused: " ++ Nat.show(gb) ++ " GB free on the disk, under " ++ Nat.show(REFUSE()) ++ " GB: a new worktree checks out a whole tree; free some disk (scripts/disk_reclaim.py, idle worktrees), then try again"

# main's one note for a held checkout
def wait_text(+n: String, +gb: Nat) -> String:
  "@" ++ n ++ " waits for disk: " ++ Nat.show(gb) ++ " GB free, under " ++ Nat.show(LOW()) ++ " GB; its worktree is made at the first reading at " ++ Nat.show(LOW()) ++ " GB or more (one held worktree per reading), refused under " ++ Nat.show(REFUSE()) ++ " GB"

# what main reads
def text(+gb: Nat) -> String:
  "disk: " ++ Nat.show(gb) ++ " GB free, under " ++ Nat.show(LOW()) ++ " GB: heavy-run trims and reclaims idle cargo targets (scripts/disk_reclaim.py); a big agent tmp or a build folder may need a look"
