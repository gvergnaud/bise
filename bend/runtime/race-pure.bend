# runtime/race-pure.bend — the decisions of a call raced against the
# interrupt flag (runtime/race.bend runs the effects; issue 13).
#
# A call that can block for seconds (a model attempt, a remote tool)
# runs beside a ticker. Every tick the caller reads the interrupt flag
# (the hub writes who asked; an empty file is a cleared flag, BR-007)
# and decides here: keep waiting, or stop now and leave the call
# behind. A result that is in wins, whatever the flag says: the
# boundary drain after it applies the interrupt anyway. The ticker
# sends a tick, then goes on only while the channel takes it: the
# caller closes the channel once it has its answer, so the ticker and a
# late result both end in a closed channel and are dropped there.
import Base
import ./provider-pure.bend as Pvp

type Verdict is Data:
  VWait{}
  VWon{}
  VStop{who: String}

# the ticker's period: a stop lands within one tick
def tick_ms() -> U32:
  100

def decide.flag(on: Bool, +c: String) -> Verdict:
  match on:
    case True{}:
      VStop{Pvp.interrupter(c)}
    case False{}:
      VWait{}

def decide.tick(f: Maybe<&2, String>) -> Verdict:
  match f:
    case None{}:
      VWait{}
    case Some{+c}:
      decide.flag(Pvp.interrupt_requested(c), c)

# won: the call's result is the message read; f: the flag, read on a tick
def decide(won: Bool, f: Maybe<&2, String>) -> Verdict:
  match won:
    case True{}:
      VWon{}
    case False{}:
      decide.tick(f)

# the ticker goes on only while its tick was taken (Chan.send answers
# False on a closed channel)
def tick_again(sent: Bool) -> Bool:
  sent

# ---- the abandoned model attempt's cancel file ----
# A stopped attempt is left running; it must stop costing tokens. The
# race writes the attempt's request id to run/bend-cancel-<port>.txt;
# the attempt's stream reads it between pieces and closes when the id
# is its own, so a stale file never cuts a later call.
# how long the attempt's stream waits in silence before it looks at the
# cancel file again (Http.stream.poll keeps the stream open)
def poll_ms() -> U32:
  200

def cancel_path_of(+dir: String, +port: String) -> String:
  String.append(dir, String.append("/bend-cancel-", String.append(port, ".txt")))

def cancelled(m: Maybe<&2, String>, +id: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+c}:
      String.eq(String.trim(c), id)
