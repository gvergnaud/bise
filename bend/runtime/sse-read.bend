# runtime/sse-read.bend — the model attempt's stream reader (moved out of
# provider.bend, which calls it from sse.head and sse.done; issue 13).
import Base
import ./persist.bend as P
import ../vendor/http/http.bend as Http
import ./race-pure.bend as Rcp
import ./race.bend as Race
import ./live-pure.bend as Lp
import ../vendor/http/encoding/encoding.bend as Enc

# ---- the streamed call (every family): Server-Sent Events ----
# The body arrives in pieces (raw bytes; decoded once, whole). The
# attempt runs inside a race (call.raced, runtime/race.bend): the race
# reads the interrupt flag every 100 ms and leaves the attempt behind
# on a stop. The attempt then stops on its own: it reads the cancel
# file (race-pure.bend cancelled) between two pieces and in every quiet
# slice of Rcp.poll_ms (Http.stream.poll keeps a silent stream open),
# and closes the connection when the file names its token, so the
# provider stops generating, and charging, at once. Not covered: the
# wait for the response headers (inside Http.open.with, bounded by the
# attempt's head limit, Pvp.head_of) - a streamed reply sends them
# before it generates. A read that waits longer than the stream's gap
# (Ck.limit = Pvp.gap_ms) in total is a dead connection (a timeout the
# policy retries).
#
# The answer as it streams in: each piece also feeds the live reader
# (runtime/live-pure.bend, lv) beside the raw parts; at most every
# Lp.every_ms() the text since the last partial goes to the race's caller
# on the race channel c (Race.SPart: this worker never writes the wire).
# After a stop the channel is closed and the send is dropped. The parts
# stay the truth: the reply is decoded whole from them, as before.

type Rd is Type:
  RdEnd{st: Http.Stream, parts: List<&2, String>}
  RdErr{st: Http.Stream, parts: List<&2, String>, e: Http.Err}
  RdStop{st: Http.Stream, parts: List<&2, String>}

# what the reader checks: the cancel file, its own token, and how long
# it may wait in silence (Pvp.wait_ms of the stream)
type Ck is Data:
  Ck{cpath: String, tok: String, limit: U32}

# ---- the live reader (the pure half: runtime/live-pure.bend) ----

# a piece's complete raw lines through the fold (it decodes what it
# reads: a text delta's literal alone, any other data line whole)
def live.cut(c: Lp.Cut) -> Lp.Lv:
  match c:
    case Lp.Cut{+ls, +lv}:
      Lp.read(ls, lv)

def live.send(-A: Type, +c: Chan(Race.Step<A>), e: Lp.Emit) -> IO(Lp.Lv):
  match e:
    case Lp.Em{+line, +lv}:
      do IO<Lp.Lv>:
        sent : Result<&1, &1, Race.Step<A>, Unit> <- Chan.send(Race.Step<A>, c, Race.SPart{line})
        IO.pure(Lp.Lv, lv)

def live.due(due: Bool, -A: Type, +c: Chan(Race.Step<A>), +lv: Lp.Lv,
    +now: Nat) -> IO(Lp.Lv):
  match due:
    case True{}:
      live.send(A, c, Lp.take(lv, now))
    case False{}:
      IO.pure(Lp.Lv, lv)

def live.now(has: Bool, -A: Type, +c: Chan(Race.Step<A>), +lv: Lp.Lv)
    -> IO(Lp.Lv):
  match has:
    case True{}:
      do IO<Lp.Lv>:
        +now : Nat <- IO.now()
        live.due(Lp.due(lv, now), A, c, lv, now)
    case False{}:
      IO.pure(Lp.Lv, lv)

# text waiting and the last partial every_ms() old: send the next one
def live(-A: Type, +c: Chan(Race.Step<A>), +lv: Lp.Lv) -> IO(Lp.Lv):
  live.now(Lp.has(lv), A, c, lv)

@unsafe
def sse.check.go(stop: Bool, -A: Type, +c: Chan(Race.Step<A>), +lv: Lp.Lv,
    st: Http.Stream, +parts: List<&2, String>, +ck: Ck, +quiet: U32) -> IO(Rd):
  match stop:
    case True{}:
      IO.pure(Rd, RdStop{st, parts})
    case False{}:
      sse.read(A, c, lv, st, parts, ck, quiet)

@unsafe
def sse.check(-A: Type, +c: Chan(Race.Step<A>), +lv: Lp.Lv, st: Http.Stream,
    +parts: List<&2, String>, +ck: Ck, +quiet: U32) -> IO(Rd):
  match ck:
    case Ck{+cpath, +tok, _}:
      do IO<Rd>:
        m : Maybe<&2, String> <- P.read_file(cpath)
        sse.check.go(Rcp.cancelled(m, tok), A, c, lv, st, parts, ck, quiet)

# a quiet slice: past the stream's wait it is a dead connection
@unsafe
def sse.quiet(over: Bool, -A: Type, +c: Chan(Race.Step<A>), +lv: Lp.Lv,
    st: Http.Stream, +parts: List<&2, String>, +ck: Ck, +quiet: U32) -> IO(Rd):
  match over:
    case True{}:
      IO.pure(Rd, RdErr{st, parts, Http.ErrTimeout{}})
    case False{}:
      sse.check(A, c, lv, st, parts, ck, quiet)

def sse.over(+ck: Ck, +quiet: U32) -> Bool:
  match ck:
    case Ck{_, _, +limit}:
      U32.is_le(limit, quiet)

@unsafe
def sse.piece(m: Maybe<&2, Maybe<&2, String>>, -A: Type,
    +c: Chan(Race.Step<A>), +lv: Lp.Lv, st: Http.Stream,
    +parts: List<&2, String>, +ck: Ck, +quiet: U32) -> IO(Rd):
  match m:
    case None{}:
      sse.quiet(sse.over(ck, U32.add(quiet, Rcp.poll_ms())), A, c, lv, st,
        parts, ck, U32.add(quiet, Rcp.poll_ms()))
    case Some{+mp}:
      sse.piece.some(mp, A, c, lv, st, parts, ck)

@unsafe
def sse.piece.some(mp: Maybe<&2, String>, -A: Type, +c: Chan(Race.Step<A>),
    +lv: Lp.Lv, st: Http.Stream, +parts: List<&2, String>, +ck: Ck) -> IO(Rd):
  match mp:
    case None{}:
      IO.pure(Rd, RdEnd{st, parts})
    case Some{+p}:
      do IO<Rd>:
        +lv2 : Lp.Lv <- live(A, c, live.cut(Lp.cut(lv, p)))
        sse.check(A, c, lv2, st, Con{p, parts}, ck, 0)

@unsafe
def sse.got(r: Http.Stream & Result<&2, &2, Http.Err, Maybe<&2, Maybe<&2, String>>>,
    -A: Type, +c: Chan(Race.Step<A>), +lv: Lp.Lv, +parts: List<&2, String>,
    +ck: Ck, +quiet: U32) -> IO(Rd):
  match r:
    case (st, res):
      match res:
        case Fail{+e}:
          IO.pure(Rd, RdErr{st, parts, e})
        case Done{+m}:
          sse.piece(m, A, c, lv, st, parts, ck, quiet)

@unsafe
def sse.read(-A: Type, +c: Chan(Race.Step<A>), +lv: Lp.Lv, st: Http.Stream,
    +parts: List<&2, String>, +ck: Ck, +quiet: U32) -> IO(Rd):
  do IO<Rd>:
    r : Http.Stream & Result<&2, &2, Http.Err, Maybe<&2, Maybe<&2, String>>> <- Http.stream.poll(st, Rcp.poll_ms())
    sse.got(r, A, c, lv, parts, ck, quiet)

