# runtime/read-tool.bend - the `read_file` tool's IO half: this module
# reads the file and asks the disk what a path is; every check, string
# and transformation lives in core/read.bend (law-pinned). Like
# runtime/edit-tool.bend beside core/edit.bend: defs bottom-up, a def
# only calls defs defined ABOVE it.
import Base
import ../core/read.bend as Rd
import ../core/edit.bend as Ed
import ./edit-tool.bend as Et
import ./tools-pure.bend as Xp
import ./persist.bend as P

def fail(+msg: String) -> Xp.ExecRes:
  Xp.Ex{False{}, msg}

def ok(+text: String) -> Xp.ExecRes:
  Xp.Ex{True{}, text}

# ---- what the disk says ----

def read.out(r: Rd.ReadOut) -> Xp.ExecRes:
  match r:
    case Rd.ROk{+text}:
      ok(text)
    case Rd.RFail{+why}:
      fail(why)

def read.bad_text(m: Maybe<&2, Nat>, +path: String) -> Xp.ExecRes:
  match m:
    case Some{+byte}:
      fail(Rd.err_not_text(path, byte))
    case None{}:
      fail(Rd.err_not_text(path, 0n))

# the text with its bytes: a U+FFFD the read put in says which byte is
# not UTF-8 (edit's stance: refuse, never mangle)
def read.text_bytes.pick(bad: Bool, +text: String, bs: List<&2, U32>,
    +path: String, +start: Nat, +limit: Nat) -> Xp.ExecRes:
  match bad:
    case True{}:
      read.bad_text(Ed.utf8_bad(bs), path)
    case False{}:
      read.out(Rd.read_out(text, path, start, limit))

def read.text_bytes(+text: String, bs: List<&2, U32>, +path: String,
    +start: Nat, +limit: Nat) -> Xp.ExecRes:
  read.text_bytes.pick(Ed.has_replacement(text), text, bs, path, start, limit)

def read.text(m: Maybe<&2, String>, +path: String, +start: Nat, +limit: Nat)
    -> IO(Xp.ExecRes):
  match m:
    case None{}:
      # kind said the file is there: an unreadable one
      IO.pure(Xp.ExecRes, fail(Rd.err_reading(path, "Permission denied")))
    case Some{+text}:
      do IO<Xp.ExecRes>:
        bs : List<&2, U32> <- Et.read_bytes(path)
        IO.pure(Xp.ExecRes, read.text_bytes(text, bs, path, start, limit))

def read.kind(+k: String, +path: String, +start: Nat, +limit: Nat)
    -> IO(Xp.ExecRes):
  Bool.pick(IO(Xp.ExecRes), String.eq(k, "none"),
    IO.pure(Xp.ExecRes, fail(Rd.err_not_found(path))),
    Bool.pick(IO(Xp.ExecRes), String.eq(k, "dir"),
      IO.pure(Xp.ExecRes, fail(Rd.err_is_dir(path))),
      do IO<Xp.ExecRes>:
        m : Maybe<&2, String> <- P.read_file(path)
        read.text(m, path, start, limit)))

# the window of a parsed call: its line limit
def limit_of(a: Rd.ReadArgs) -> Nat:
  match a:
    case Rd.ReadArgs{_, _, +l}:
      l

# the window of a parsed call: its line limit
def read.call(+p: String, +o: Maybe<&2, Nat>, +l: Nat) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    +path : String <- Et.resolved(p)
    +k : String <- Et.kind(path)
    read.kind(k, path, Rd.start_of.m(o), l)

def read.args(bad: Maybe<&2, String>, +p: String, +o: Maybe<&2, Nat>,
    +l: Nat) -> IO(Xp.ExecRes):
  match bad:
    case Some{+msg}:
      IO.pure(Xp.ExecRes, fail(msg))
    case None{}:
      read.call(p, o, l)

def read.entry(a: Rd.ReadArgs) -> IO(Xp.ExecRes):
  match a:
    case Rd.ReadArgs{+p, +o, +l}:
      read.args(Rd.check_args(p), p, o, l)

def scripted() -> Xp.ExecRes:
  fail("read_file needs a live session")

# the tool entry: the args as the model wrote them
def exec_read(+args: String, live: Bool) -> IO(Xp.ExecRes):
  match live:
    case True{}:
      read.entry(Rd.read_args(args))
    case False{}:
      IO.pure(Xp.ExecRes, scripted())
