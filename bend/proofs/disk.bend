# bend/proofs/disk.bend: the proofs of bend/laws/disk.bend. Each side
# computes, but for BACK() + p, which is not under BACK() whatever p is.
import Base
import ../hub/disk.bend as DK
import ../laws/disk.bend as Laws

def Laws.low_never_tells_again(gb):
  {==}

def Laws.tells_on_the_crossing(gb):
  {==}

def Laws.back_over_ends_low(p):
  match p:
    case 0n:
      {==}
    case 1n+q:
      {==}

def Laws.hovering_stays_low():
  {==}

def Laws.under_low_tells():
  {==}

def Laws.at_low_is_silent():
  {==}

def Laws.unknown_disk_goes():
  {==}

def Laws.refused_under_20():
  {==}

def Laws.refused_at_0():
  {==}

def Laws.waits_at_20():
  {==}

def Laws.waits_under_40():
  {==}

def Laws.goes_at_40(p):
  match p:
    case 0n:
      {==}
    case 1n+q:
      {==}
