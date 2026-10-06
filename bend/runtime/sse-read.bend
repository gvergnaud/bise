# runtime/sse-read.bend — the model attempt's stream reader (moved out of
# provider.bend, which calls it from sse.head and sse.done; issue 13).
import Base
import ./persist.bend as P
import ../vendor/http/http.bend as Http
import ./race-pure.bend as Rcp

# ---- the streamed call (every family): Server-Sent Events ----
# The body arrives in pieces (raw bytes; decoded once, whole). The
# attempt runs inside a race (call.raced, runtime/race.bend): the race
# reads the interrupt flag every 100 ms and leaves the attempt behind
# on a stop. The attempt then stops on its own: it reads the cancel
# file (race-pure.bend cancelled) between two pieces and in every quiet
# slice of Rcp.poll_ms (Http.stream.poll keeps a silent stream open),
# and closes the connection when the file names its token, so the
# provider stops generating, and charging, at once. Not covered: the
# wait for the response headers (inside Http.open.with) - a streamed
# reply sends them before it generates. A read that waits longer than
# the stream's wait in total is a dead connection (a timeout the policy
# retries).

type Rd is Type:
  RdEnd{st: Http.Stream, parts: List<&2, String>}
  RdErr{st: Http.Stream, parts: List<&2, String>, e: Http.Err}
  RdStop{st: Http.Stream, parts: List<&2, String>}

# what the reader checks: the cancel file, its own token, and how long
# it may wait in silence (Pvp.wait_ms of the stream)
type Ck is Data:
  Ck{cpath: String, tok: String, limit: U32}

@unsafe
def sse.check.go(stop: Bool, st: Http.Stream, +parts: List<&2, String>,
    +ck: Ck, +quiet: U32) -> IO(Rd):
  match stop:
    case True{}:
      IO.pure(Rd, RdStop{st, parts})
    case False{}:
      sse.read(st, parts, ck, quiet)

@unsafe
def sse.check(st: Http.Stream, +parts: List<&2, String>, +ck: Ck, +quiet: U32)
    -> IO(Rd):
  match ck:
    case Ck{+cpath, +tok, _}:
      do IO<Rd>:
        m : Maybe<&2, String> <- P.read_file(cpath)
        sse.check.go(Rcp.cancelled(m, tok), st, parts, ck, quiet)

# a quiet slice: past the stream's wait it is a dead connection
@unsafe
def sse.quiet(over: Bool, st: Http.Stream, +parts: List<&2, String>,
    +ck: Ck, +quiet: U32) -> IO(Rd):
  match over:
    case True{}:
      IO.pure(Rd, RdErr{st, parts, Http.ErrTimeout{}})
    case False{}:
      sse.check(st, parts, ck, quiet)

def sse.over(+ck: Ck, +quiet: U32) -> Bool:
  match ck:
    case Ck{_, _, +limit}:
      U32.is_le(limit, quiet)

@unsafe
def sse.piece(m: Maybe<&2, Maybe<&2, String>>, st: Http.Stream,
    +parts: List<&2, String>, +ck: Ck, +quiet: U32) -> IO(Rd):
  match m:
    case None{}:
      sse.quiet(sse.over(ck, U32.add(quiet, Rcp.poll_ms())), st, parts, ck,
        U32.add(quiet, Rcp.poll_ms()))
    case Some{+mp}:
      sse.piece.some(mp, st, parts, ck)

@unsafe
def sse.piece.some(mp: Maybe<&2, String>, st: Http.Stream,
    +parts: List<&2, String>, +ck: Ck) -> IO(Rd):
  match mp:
    case None{}:
      IO.pure(Rd, RdEnd{st, parts})
    case Some{+p}:
      sse.check(st, Con{p, parts}, ck, 0)

@unsafe
def sse.got(r: Http.Stream & Result<&2, &2, Http.Err, Maybe<&2, Maybe<&2, String>>>,
    +parts: List<&2, String>, +ck: Ck, +quiet: U32) -> IO(Rd):
  match r:
    case (st, res):
      match res:
        case Fail{+e}:
          IO.pure(Rd, RdErr{st, parts, e})
        case Done{+m}:
          sse.piece(m, st, parts, ck, quiet)

@unsafe
def sse.read(st: Http.Stream, +parts: List<&2, String>, +ck: Ck, +quiet: U32)
    -> IO(Rd):
  do IO<Rd>:
    r : Http.Stream & Result<&2, &2, Http.Err, Maybe<&2, Maybe<&2, String>>> <- Http.stream.poll(st, Rcp.poll_ms())
    sse.got(r, parts, ck, quiet)

