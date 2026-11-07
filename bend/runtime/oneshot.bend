# runtime/oneshot.bend — one provider call, then exit (BISE-126).
#
#   BISE_ONESHOT=<request file> BISE_MODEL=<model> ./repl-live
#
# The hub asks a small model for short labels (a task's role line) with
# the same provider code as the agents (families, URLs, headers, keys,
# retries: runtime/provider.bend), instead of an HTTP client of its own.
# The file holds one wire request (runtime/remote.bend's format: MSG
# lines, no TOOL line). The body goes out light (runtime/oneshot-pure.bend).
# stdout ends with one line:
#   ONESHOT_OK <the reply text, newlines escaped>
#   ONESHOT_ERR <why>
# (lines before it are the provider notes: usage, retries). No listener,
# no session, no plugins, no tools.
import Base
import ../core/wire.bend as W
import ./persist.bend as P
import ./remote.bend as Rem
import ./provider.bend as Pv

def answer(r: Pv.Out & Rem.RemoteRes) -> IO(Unit):
  match r:
    case (_, Rem.RROk{+text, _}):
      IO.print(String.append("ONESHOT_OK ", W.escape_nl(text)))
    case (_, Rem.RRErr{+why}):
      IO.print(String.append("ONESHOT_ERR ", W.escape_nl(why)))

@unsafe
def call(m: Maybe<&2, String>) -> IO(Unit):
  match m:
    case Some{+request}:
      do IO<Unit>:
        r : Pv.Out & Rem.RemoteRes <- Pv.model_call(Pv.std0(), request)
        answer(r)
    case None{}:
      IO.print("ONESHOT_ERR the request file is unreadable")

@unsafe
def run(+path: String) -> IO(Unit):
  do IO<Unit>:
    m : Maybe<&2, String> <- P.read_file(path)
    call(m)
