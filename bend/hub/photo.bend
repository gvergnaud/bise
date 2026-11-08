# hub/photo.bend: the hub's photo (issue 36). Its durable state as one
# JSON value (gen/state-codec.bend's St.photo_json, lossless: the law
# St.photo_round), and the hub rebuilt from one with a fresh runtime, so a
# boot starts from the photo and replays only the events after it.
# main.bend answers `photo` and `restore_photo` with these; their laws are
# in bend/laws/photo.bend.
import Base

import ../vendor/json.bend as J

import ./gen/state.bend as S
import ./gen/state-codec.bend as SC

import ./model.bend as M
import ./cold.bend as CO
import ./cx.bend as X
import ./core.bend as H

# the durable state a photo is taken of (main.bend sends it in slices of
# SLICE records); none while an env query is pending (a step halted on
# its need: the hub sends the same input again)
def photo.go(need: Maybe<&2, J.Json>, +d: S.St) -> Maybe<&2, S.St>:
  match need:
    case Some{_}:
      None{}
    case None{}:
      Some{d}

def photo(+h: H.Hub) -> Maybe<&2, S.St>:
  photo.go(X.R.need(H.Hub.r(h)), H.Hub.d(h))

# the runtime of a new hub in that workspace (R is never journaled)
def fresh(+workspace: String) -> X.R:
  H.Hub.r(H.hub_new(workspace))

# where the hub is in its boot
def phase(+h: H.Hub) -> H.Phase:
  match h:
    case H.Hub{_, _, +p}:
      p

# the hub from a photo (its last slice, c6 (c)): its durable state, the
# runtime fresh in the same workspace, Restored: only restore_more may
# follow. None, the hub as it is, when it is not Fresh (a restore never
# replaces a live state: architect m_34907) or when the photo is not an
# object
def restore.go(p: H.Phase, +h: H.Hub, +j: J.Json) -> Maybe<&2, H.Hub>:
  match p j:
    case H.Fresh{} J.JObj{+kvs}:
      Some{H.Hub{SC.St.photo_of(J.JObj{kvs}), fresh(X.R.workspace(H.Hub.r(h))), H.Restored{}}}
    case _ _:
      None{}

def restore(+h: H.Hub, +j: J.Json) -> Maybe<&2, H.Hub>:
  restore.go(phase(h), h, j)

# the slice before the ones restored: its records before the state's,
# its scalars. The shell sends the slices last first, so each one is
# prepended: the work is the slice's size, never the state's, and the
# whole restore stays linear (appending would walk the state each time).
# None, the hub as it is, unless Restored, or when the slice is not an
# object
def restore_more.go(p: H.Phase, +h: H.Hub, +j: J.Json) -> Maybe<&2, H.Hub>:
  match p j:
    case H.Restored{} J.JObj{+kvs}:
      Some{H.Hub{SC.St.slice_join(SC.St.photo_of(J.JObj{kvs}), H.Hub.d(h)), H.Hub.r(h), H.Restored{}}}
    case _ _:
      None{}

def restore_more(+h: H.Hub, +j: J.Json) -> Maybe<&2, H.Hub>:
  restore_more.go(phase(h), h, j)

# how many records one photo line holds: under json.bend's 256, where an
# array prints in 64 parallel parts (c-1: +27% instructions for lines
# that pay that split). Arch agents are the biggest (~4.3 KB): ~1.1 MB a
# line at most
def SLICE() -> Nat:
  255n

# a photo line: the photo of st's first SLICE records (st's scalars)
def slice_json(+st: S.St) -> J.Json:
  SC.St.photo_json(SC.St.slice_take(SLICE(), st))

# what is left after that line; all sent once no record is left
def slice_rest(+st: S.St) -> S.St:
  SC.St.slice_drop(SLICE(), st)

def all_sent(+st: S.St) -> Bool:
  SC.St.slice_none(st)

# the durable state a batch of journal events leaves (the boot's fold,
# CO.replay_batch, without its skipped indexes)
def st_of(p: S.St & List<&2, Nat>) -> S.St:
  match p:
    case (d, _):
      d

def replayed(ms: List<&2, Maybe<&2, M.Ev>>, +st: S.St) -> S.St:
  st_of(CO.replay_batch(ms, st, 0n, Nil{}))
