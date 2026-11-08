# hub/disk.bend — when the hub tells main the disk is low (disk-hygiene,
# point 8 of the disk audit, architect m_55867 / m_56051). The shell reads
# the data volume's free GB on its tick and sends IDiskFree when the whole
# GB changed; the band (low or not) is a sensor fact, runtime only (the
# core's R.last, key KEY()), never journaled: a hub restart while low notes
# main once more. Under LOW() GB it notes main once; it is low again only
# after the disk climbed back over BACK() GB (hysteresis: a disk hovering
# at 40 GB says it once). Pure; its laws are in bend/laws/disk.bend.
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

# what main reads
def text(+gb: Nat) -> String:
  "disk: " ++ Nat.show(gb) ++ " GB free, under " ++ Nat.show(LOW()) ++ " GB: heavy-run trims and reclaims idle cargo targets (scripts/disk_reclaim.py); a big agent tmp or a build folder may need a look"
