# hub/said.bend - his message a step made (A1, architect m_21450): the
# first message from the user in a step's journal. His input makes it
# when it arrives, queued too. main.bend answers its id with the step
# (AStep.msg), the shell hands it to the asker (turn/send's msg_id): the
# id his you-id line carries when it is delivered (laws said_is_his,
# said_you_id in bend/laws/threads.bend).
import Base

import ./model.bend as M
import ./gen/state.bend as S

def his_msg(e: M.Ev) -> Maybe<&2, S.Msg>:
  match e:
    case M.EMsgSent{+s}:
      +m = M.msg_of_sent(s)
      Bool.pick(Maybe<&2, S.Msg>, String.eq(S.Msg.from(m), M.USER()), Some{m}, None{})
    case _:
      None{}

def said.of(h: Maybe<&2, S.Msg>, +rest: Maybe<&2, S.Msg>) -> Maybe<&2, S.Msg>:
  match h:
    case None{}:
      rest
    case Some{+m}:
      Some{m}

def said(evs: List<&2, M.Ev>) -> Maybe<&2, S.Msg>:
  match evs:
    case Nil{}:
      None{}
    case Con{+e, +t}:
      said.of(his_msg(e), said(t))

def said_id(m: Maybe<&2, S.Msg>) -> Maybe<&2, Nat>:
  match m:
    case None{}:
      None{}
    case Some{+x}:
      Some{S.Msg.id(x)}
