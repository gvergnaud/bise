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
