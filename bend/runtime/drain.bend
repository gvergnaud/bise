# runtime/drain.bend — the loop's output and the turn boundary's side
# channels: emit (one batch of lines to the REPL socket or stdout), then
# the steering file and the interrupt flag, drained at the safe
# boundaries (after a model call, after a tool) by runtime/main.bend.
# Moved out of main.bend as is (no change inside the moved defs).
import Base
import ../core/types.bend as T
import ../core/session.bend as S
import ../core/text.bend as Tx
import ./model.bend as M
import ./persist.bend as P
import ./provider.bend as Pv
import ./main-pure.bend as Rtp
import ./debug.bend as Dbg

def print_lines(lines: List<&2, String>) -> IO(Unit):
  match lines:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case Con{l, rest}:
      do IO<Unit>:
        IO.print(l)
        print_lines(rest)

# send one batch of lines to the loop's output; a socket send hands the
# socket back, so the new Pv.Out is returned for the next hop.
# every batch also leaves its breadcrumbs in the session debug log;
# the wire log: Pv.wire_log (the provider's in-flight notes share it)
def emit(out: Pv.Out, +lines: List<&2, String>) -> IO(Pv.Out):
  match out:
    case Pv.OStd{}:
      do IO<Pv.Out>:
        d : Unit <- Dbg.sent(lines)
        print_lines(lines)
        IO.pure(Pv.Out, Pv.OStd{})
    case Pv.ONet{sock}:
      do IO<Pv.Out>:
        d : Unit <- Dbg.sent(lines)
        +text : String <- IO.pure(String,
          String.append(String.join(lines, "\n"), "\n"))
        Pv.wire_log(text)
        sm : Socket & Result<&1, &1, U32 & String, Unit> <- TCP.send(sock, text)
        s2 : Socket <- IO.pure(Socket, Pv.out.pair(sm))
        IO.pure(Pv.Out, Pv.ONet{s2})


def steer_path() -> IO(String):
  do IO<String>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    +d : String <- P.side_dir()
    IO.pure(String, Rtp.steer_path_of(d, p))

# apply the drained lines one by one. Mid-turn Steer produces no acts
# (apply.user.busy), so the continuation work in `after` is untouched.
# Structural recursion on the shrinking line list; out and rt are
# merely threaded.
# apply the drained lines one by one. Mid-turn Steer produces no acts
# (apply.user.busy), so the continuation work in `after` is untouched.
# Structural recursion on the shrinking line list; out and the session
# fields are merely threaded.
def drain_steer.one(steers: List<&2, String>, out: Pv.Out, +sess: T.Session,
    +script: List<&2, M.Reply>, +cmds: List<&2, T.Cmd>, live: Bool)
    -> IO(Pv.Out & Rtp.Rt):
  match steers:
    case Nil{}:
      do IO<Pv.Out & Rtp.Rt>:
        IO.pure(Pv.Out & Rtp.Rt, (out, Rtp.Rt{sess, script, cmds, live}))
    case Con{+l, +rest}:
      do IO<Pv.Out & Rtp.Rt>:
        +res : T.Res <- IO.pure(T.Res,
          S.apply(sess, Rtp.steer_cmd(l)))
        out2 : Pv.Out <- emit(out, Rtp.drain_steer.emit_of(res))
        drain_steer.one(rest, out2, Rtp.drain_steer.next_of(sess, res),
          script, cmds, live)

# destructure the runtime state once, then fold the steering lines in
def drain_steer.step(out: Pv.Out, +rt: Rtp.Rt, +steers: List<&2, String>)
    -> IO(Pv.Out & Rtp.Rt):
  match rt:
    case Rtp.Rt{+sess, +script, +cmds, +live}:
      drain_steer.one(steers, out, sess, script, cmds, live)

# read-then-truncate: content that arrives after the read stays in the
# file for the NEXT boundary, never lost, never double-applied
def drain_steer.some(empty: Bool, +path: String, out: Pv.Out, +rt: Rtp.Rt,
    +c: String) -> IO(Pv.Out & Rtp.Rt):
  match empty:
    case True{}:
      do IO<Pv.Out & Rtp.Rt>:
        IO.pure(Pv.Out & Rtp.Rt, (out, rt))
    case False{}:
      do IO<Pv.Out & Rtp.Rt>:
        u : Unit <- P.write_file(path, "")
        drain_steer.step(out, rt, Rtp.drain_steer.skip_go(String.split(c, Tx.newline())))

def drain_steer.pick(m: Maybe<&2, String>, +path: String, out: Pv.Out, +rt: Rtp.Rt)
    -> IO(Pv.Out & Rtp.Rt):
  match m:
    case None{}:
      do IO<Pv.Out & Rtp.Rt>:
        IO.pure(Pv.Out & Rtp.Rt, (out, rt))
    case Some{+c}:
      drain_steer.some(String.is_empty(c), path, out, rt, c)

# ---- the interrupt side-channel (Ctrl+C in the TUI) ----
# Same shape as the steering side-channel: the socket is only read
# between turns, so a mid-turn interrupt would sit in the kernel buffer
# until the turn ends. The TUI writes a flag file instead; the runtime
# drains it right after the steering file at the same safe boundaries.
# The Core kills the turn (CandidateDiscarded + TurnDone interrupted),
# commits any steering the turn was holding, and the in-flight result
# is then rejected cleanly ("no pending completion").

def drain_interrupt.some(empty: Bool, +ipath: String, out: Pv.Out, +rt: Rtp.Rt,
    +sess: T.Session, +script: List<&2, M.Reply>, +cmds: List<&2, T.Cmd>,
    +live: Bool) -> IO(Pv.Out & Rtp.Rt):
  match empty:
    case True{}:
      do IO<Pv.Out & Rtp.Rt>:
        IO.pure(Pv.Out & Rtp.Rt, (out, rt))
    case False{}:
      do IO<Pv.Out & Rtp.Rt>:
        u : Unit <- P.write_file(ipath, "")
        +res : T.Res <- IO.pure(T.Res, S.apply(sess, T.Interrupt{}))
        out2 : Pv.Out <- emit(out, Rtp.drain_steer.emit_of(res))
        IO.pure(Pv.Out & Rtp.Rt,
          (out2, Rtp.Rt{Rtp.drain_steer.next_of(sess, res), script, cmds, live}))

def drain_interrupt.pick(m: Maybe<&2, String>, +ipath: String, out: Pv.Out,
    +rt: Rtp.Rt, +sess: T.Session, +script: List<&2, M.Reply>,
    +cmds: List<&2, T.Cmd>, live: Bool) -> IO(Pv.Out & Rtp.Rt):
  match m:
    case None{}:
      do IO<Pv.Out & Rtp.Rt>:
        IO.pure(Pv.Out & Rtp.Rt, (out, rt))
    case Some{+c}:
      drain_interrupt.some(String.is_empty(c), ipath, out, rt, sess,
        script, cmds, live)

# consumes the drained pair once and runs the interrupt check
def drain_interrupt(pr: Pv.Out & Rtp.Rt) -> IO(Pv.Out & Rtp.Rt):
  match pr:
    case (out2, +rt2):
      match rt2:
        case Rtp.Rt{+sess, +script, +cmds, +live}:
          do IO<Pv.Out & Rtp.Rt>:
            +ipath : String <- Pv.interrupt_path()
            m : Maybe<&2, String> <- P.read_file(ipath)
            drain_interrupt.pick(m, ipath, out2, rt2, sess, script, cmds,
              live)

def drain_steer.file(+path: String, out: Pv.Out, +rt: Rtp.Rt) -> IO(Pv.Out & Rtp.Rt):
  do IO<Pv.Out & Rtp.Rt>:
    m : Maybe<&2, String> <- P.read_file(path)
    pr : Pv.Out & Rtp.Rt <- drain_steer.pick(m, path, out, rt)
    drain_interrupt(pr)

# the entry point: scripted runs (live=False) never touch the file, so
# the scenarios and harness-demo stay hermetic
def drain_steer(out: Pv.Out, +rt: Rtp.Rt) -> IO(Pv.Out & Rtp.Rt):
  match rt:
    case Rtp.Rt{_, _, _, +live}:
      match live:
        case True{}:
          do IO<Pv.Out & Rtp.Rt>:
            path : String <- steer_path()
            drain_steer.file(path, out, rt)
        case False{}:
          do IO<Pv.Out & Rtp.Rt>:
            IO.pure(Pv.Out & Rtp.Rt, (out, rt))
