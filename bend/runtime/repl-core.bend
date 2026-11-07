# runtime/repl-core.bend — the REPL line server, shared by both entries.
#
# runtime/repl-live.bend (real provider, real tools) and runtime/repl.bend
# (scripted replies, no API) are thin entries over this module: they
# build the session config and call serve(live, live, ...). The mode reaches
# the runtime loop through the explicit `live` parameter.
#
# Bend has no stdin effect, so the REPL is a line server: the session
# persists across lines and across connections (until the process dies).
#
# The provider call and the bash tool run INSIDE this process: the hub
# HTTP client does the TLS call to api.mistral.ai (core/api.bend maps
# the wire request to the API JSON and the reply back), and Base's
# Process.run (runtime/proc.bend) runs /bin/sh. No bridge process anymore.
#
# Commands (one per line):
#   say <text>        start a turn. "say tools ..." makes the model call
#                     search_tool_functions; "say prog: <program>"
#                     makes it call run_typescript (D1).
#   steer <text>      steering input (at idle the Core starts a turn with it)
#   notify <text>     a notification
#   compact           manual between-turn compaction
#   interrupt         interrupt (an accepted no-op while idle)
#   reload            restart the process with the latest code: only
#                     accepted between turns (reload_allowed); the parent
#                     recompiles, respawns on the same port and restores
#                     the checkpointed session (BEND_CONTINUE=1)
#   quit              close the connection; the session survives
#
# The whole loop follows the echo-server pattern from the demos: the
# per-line helpers take the loop continuation as a closure, and the
# accept loop is @unsafe (it runs until killed).
import Base
import ../core/types.bend as T
import ../core/text.bend as Tx
import ../core/commands.bend as Cm
import ../core/api.bend as Api
import ../core/session.bend as S
import ./persist.bend as P
import ./model.bend as M
import ./main.bend as R
import ./drain.bend as Dr
import ./main-pure.bend as Rp
import ../runtime/provider.bend as Pv
import ../runtime/provider-pure.bend as Pvp
import ../runtime/selftools.bend as Sf
import ../runtime/selftools-pure.bend as Sfp
import ./repl-core-pure.bend as Rcp
import ../core/ev.bend as Ev
import ../core/utf8.bend as U8
import ../core/gen/obs.bend as OG

# what the loops return: keep serving, or stop. The reload path
# unwinds to main so the process exits 0 — the parent then recompiles
# the latest source and respawns on the SAME session (--continue).
# Go keeps the calls' memory too (Pvp.CallMem): the next connection
# (a hub that reconnects) goes on with it; it is never saved.
type Flow is Data:
  Go{sess: T.Session, mem: Pvp.CallMem}
  Stop{}

def repl_help() -> List<&2, String>:
  Con{"bend-harness REPL. Commands:",
   Con{"  say <text>       start a turn (say tools ... / say prog: const s = search_tool_functions(best_match: catalog); return s;)",
   Con{"  steer <text>     steering input",
   Con{"  notify <text>    a notification",
   Con{"  compact          manual compaction",
   Con{"  interrupt        interrupt",
   Con{"  reload           restart with the latest code (between turns only)",
   Con{"  quit             close (the session survives)",
   Nil{}}}}}}}}}

# ---- command dispatch ----

def plan_of(cl: Cm.CmdLine) -> Rcp.Plan:
  match cl:
    case Cm.CL{+op, +rest}:
      Rcp.plan_say(String.eq(op, "say"), op, rest)

# ---- the connection loop ----
# Each helper takes the loop continuation as a curried closure (the
# echo-server pattern: closures cannot match, so extraction happens in
# ordinary defs, and the closure only re-enters the loop with the new
# socket and session). Everything returns Flow: Go keeps serving,
# Stop unwinds to main (the reload path — the process exits 0 and the
# parent respawns the latest code on the same session).

def repl_out(out: Pv.Out, +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match out:
    case Pv.OStd{+mem}:
      IO.pure(Flow, Go{sess, mem})
    case Pv.ONet{s2, +mem}:
      next(s2)(mem)(sess)

# ---- /reload and the deferred self.reload: gated, then a clean exit ----

# refused: turn/pending/queued/notifs would be lost — the checkpoint
# only serializes turn boundaries (ADR 0002); say so, keep serving
def repl_reload.busy(out: Pv.Out, +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  do IO<Flow>:
    out2 : Pv.Out <- Dr.emit(out,
      Con{OG.reload(True{}, "not between turns — finish or interrupt the pending turn first"),
        Con{"--- idle", Nil{}}})
    repl_out(out2, sess, next)

# accepted: ack, checkpoint, close — Stop unwinds so the process exits
# 0; the parent recompiles the latest source and respawns with
# BEND_CONTINUE=1 (same port, same session file). Background command
# slots live in BEND_BG_ROOT on disk: they keep running across the
# reload and stay reachable from the new process.
def repl_reload.halt(out: Pv.Out, +sess: T.Session) -> IO(Flow):
  match out:
    case Pv.OStd{_}:
      do IO<Flow>:
        P.save(sess)
        IO.pure(Flow, Stop{})
    case Pv.ONet{s2, _}:
      do IO<Flow>:
        Socket.close(s2)
        P.save(sess)
        IO.pure(Flow, Stop{})

def repl_reload(out: Pv.Out, ok: Bool, +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match ok:
    case True{}:
      do IO<Flow>:
        out2 : Pv.Out <- Dr.emit(out,
          Con{OG.reload(False{}, "restarting the harness with the latest code"),
            Con{OG.reload(False{}, "the session is preserved — background commands keep running"),
              Con{"--- idle", Nil{}}}})
        repl_reload.halt(out2, sess)
    case False{}:
      repl_reload.busy(out, sess, next)

# ---- deferred self.reload (the agent's own request) ----
# A tool result acked "reload: scheduled" is a request the agent filed
# mid-turn. It executes HERE, at the turn boundary, through the same
# gated path as the /reload command. (self.compact is no longer
# deferred: the Core compacts inside the turn, which goes on.)

def repl_deferred.reload(+sess: T.Session, out: Pv.Out,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  do IO<Flow>:
    +ok : Bool <- IO.pure(Bool, S.reload_allowed(sess))
    f : Flow <- repl_reload(out, ok, sess, next)
    IO.pure(Flow, f)

def repl_deferred.reload.only(reload: Bool, +sess2: T.Session, out2: Pv.Out,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match reload:
    case True{}:
      repl_deferred.reload(sess2, out2, next)
    case False{}:
      repl_out(out2, sess2, next)

# one run of the runtime loop, from idle. 1M hops: effectively unbounded
# for a terminal session (the old 100 died mid-turn around ~20 model
# rounds, silently). The script only feeds scripted mode; live mode
# calls the provider and ignores it.
def repl_run(+live: Bool, out: Pv.Out, +sess: T.Session,
    +script: List<&2, M.Reply>, +cmds: List<&2, T.Cmd>) -> IO(R.RtEnd):
  +rt = {Rp.Rt{sess, script, cmds, live} : Rp.Rt}
  R.run(1000000n, out, rt, Rp.JIdle{}, Rp.transition(rt, Rp.JIdle{}))

def repl_deferred(d: Sfp.Def, +sess2: T.Session, out2: Pv.Out,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match d:
    case Sfp.Def{_, +reload}:
      repl_deferred.reload.only(reload, sess2, out2, next)

def repl_end_save.then(+live: Bool, +sess2: T.Session, out2: Pv.Out,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  do IO<Flow>:
    # BR-002: the turn is over - a stale interrupt flag would kill the
    # NEXT turn at its first boundary. The runtime is the authority on
    # turn boundaries: clear the flag now; a flag written later
    # belongs to a later turn (or to no turn at all).
    +ipath : String <- Pv.interrupt_path()
    uc : Unit <- P.write_file(ipath, "")
    P.save(sess2)
    # the turn asked for nothing: keep serving; else run its requests
    +d : Sfp.Def <- IO.pure(Sfp.Def, Sfp.deferred_of(P.history_of(sess2)))
    f : Flow <- repl_deferred(d, sess2, out2, next)
    IO.pure(Flow, f)

def repl_end_save(+live: Bool, e: R.RtEnd, next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow))
    -> IO(Flow):
  match e:
    case R.REnd{sess2, out2}:
      repl_end_save.then(live, sess2, out2, next)

# a stalled session (fuel exhausted mid-turn) must say so, never die
# silently: the turn is still pending and the user must know
def turn_active(s: T.Session) -> Bool:
  match s:
    case T.Sess{_, _, _, _, +turn, _, _, _}:
      match turn:
        case None{}:
          False{}
        case Some{_}:
          True{}

def repl_end_stalled.stalled(+live: Bool, active: Bool, e: R.RtEnd,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match active:
    case True{}:
      match e:
        case R.REnd{+sess2, out2}:
          do IO<Flow>:
            out3 : Pv.Out <- Dr.emit(out2,
              Con{OG.turn_stalled("execution budget exhausted"),
                Con{"--- idle", Nil{}}})
            P.save(sess2)
            repl_out(out3, sess2, next)
    case False{}:
      repl_end_save(live, e, next)

def repl_end_stalled(+live: Bool, e: R.RtEnd,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match e:
    case R.REnd{+sess2, _}:
      repl_end_stalled.stalled(live, turn_active(sess2), e, next)

# the scripted REPL's replies from BEND_SCRIPT_FILE (Rcp.script_lookup),
# when set; live: the plan's own (unused) script
def script_file.read(none: Bool, +path: String, +key: String,
    +dflt: List<&2, M.Reply>) -> IO(List<&2, M.Reply>):
  match none:
    case True{}:
      IO.pure(List<&2, M.Reply>, dflt)
    case False{}:
      do IO<List<&2, M.Reply>>:
        +body : String <- P.read_or(path, "")
        IO.pure(List<&2, M.Reply>, Rcp.script_lookup(body, key, dflt))

def script_file(live: Bool, +cmds: List<&2, T.Cmd>, +dflt: List<&2, M.Reply>)
    -> IO(List<&2, M.Reply>):
  match live:
    case True{}:
      IO.pure(List<&2, M.Reply>, dflt)
    case False{}:
      do IO<List<&2, M.Reply>>:
        r : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_SCRIPT_FILE")
        +path : String <- IO.pure(String, P.env_or(r, ""))
        script_file.read(String.is_empty(path), path, Rcp.script_key(cmds), dflt)

def repl_plan(+live: Bool, s: Socket, +mem: Pvp.CallMem, plan: Rcp.Plan,
    +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match plan:
    case Rcp.PQuit{}:
      do IO<Flow>:
        Socket.close(s)
        IO.pure(Flow, Go{sess, mem})
    case Rcp.PHelp{}:
      do IO<Flow>:
        out2 : Pv.Out <- Dr.emit(Pv.ONet{s, mem}, repl_help())
        repl_out(out2, sess, next)
    case Rcp.PReload{}:
      do IO<Flow>:
        +ok : Bool <- IO.pure(Bool, S.reload_allowed(sess))
        f : Flow <- repl_reload(Pv.ONet{s, mem}, ok, sess, next)
        IO.pure(Flow, f)
    case Rcp.PRun{+cmds, +script0}:
      do IO<Flow>:
        +script : List<&2, M.Reply> <- script_file(live, cmds, script0)
        e : R.RtEnd <- repl_run(live, Pv.ONet{s, mem}, sess, script, cmds)
        repl_end_stalled(live, e, next)

def repl_interpret(+live: Bool, s: Socket, +mem: Pvp.CallMem,
    i: Cm.Interpretation, +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match i:
    case Cm.Send{+line}:
      repl_plan(live, s, mem, plan_of(Cm.op_of(line)), sess, next)
    case Cm.Note{+warning}:
      # a local warning: no protocol effect, the session is untouched;
      # the idle marker keeps the client contract (batch + idle)
      do IO<Flow>:
        out2 : Pv.Out <- Dr.emit(Pv.ONet{s, mem},
          Con{String.append("  obs: ", warning), Con{"--- idle", Nil{}}})
        repl_out(out2, sess, next)

def repl_cmd(+live: Bool, s: Socket, +mem: Pvp.CallMem, +line: String,
    +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  repl_interpret(live, s, mem, Cm.interpret(line), sess, next)

# @unsafe: the continuation closure self-references the batch loop
@unsafe
def repl_lines.go(+live: Bool, +lines: List<&2, String>, s: Socket,
    +mem: Pvp.CallMem, +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match lines:
    case Nil{}:
      next(s)(mem)(sess)
    case Con{+l, +rest}:
      do IO<Flow>:
        f : Flow <- repl_cmd(live, s, mem, l, sess,
          s2 => +mem2 => sess2 => repl_lines.go(live, rest, s2, mem2, sess2, next))
        IO.pure(Flow, f)

# `lines`: the commands this chunk's newlines end (after the line the
# last chunks left unfinished: Rcp.feed); they run, then `next` reads
# the next chunk (a message longer than one recv stays one message,
# BISE-240)
def repl_data(+live: Bool, closed: Bool, s: Socket, +mem: Pvp.CallMem,
    +lines: List<&2, String>, +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match closed:
    case True{}:
      do IO<Flow>:
        Socket.close(s)
        IO.pure(Flow, Go{sess, mem})
    case False{}:
      repl_lines.go(live, lines, s, mem, sess, next)

# one recv's bytes, after the bytes of the char the last recv cut
# (core/utf8.bend: a char a recv boundary cuts reaches its line whole,
# issue 36); no bytes: the hub closed the connection. `carry`: the
# pieces of the unfinished line, newest first (Rcp.feed)
@unsafe
def repl_got(+live: Bool, s: Socket, +mem: Pvp.CallMem, +bs: List<&2, U32>, +carry: List<&2, String>, +pend: List<&2, U32>, +sess: T.Session) -> IO(Flow):
  repl_decoded(live, List.is_empty(&2, U32, bs), s, mem, U8.decode(pend, bs), carry, sess)

@unsafe
def repl_fed(+live: Bool, closed: Bool, s: Socket, +mem: Pvp.CallMem,
    f: List<&2, String> & List<&2, String>,
    +pend2: List<&2, U32>, +sess: T.Session) -> IO(Flow):
  match f:
    case (+lines, +carry2):
      repl_data(live, closed, s, mem, lines, sess,
        s2 => +mem2 => sess2 => repl_conn(live, s2, mem2, carry2, pend2, sess2))

@unsafe
def repl_decoded(+live: Bool, closed: Bool, s: Socket, +mem: Pvp.CallMem, r: String & List<&2, U32>, +carry: List<&2, String>, +sess: T.Session) -> IO(Flow):
  match r:
    case (+text, +pend2):
      repl_fed(live, closed, s, mem, Rcp.feed(carry, text), pend2, sess)

@unsafe
def repl_after(+live: Bool, rm: Socket & Result<&1, &1, U32 & String, Maybe<&1, List<&2, U32>>>,
    +mem: Pvp.CallMem, +carry: List<&2, String>, +pend: List<&2, U32>,
    +sess: T.Session) -> IO(Flow):
  match rm:
    case (s, Fail{_}):
      do IO<Flow>:
        Socket.close(s)
        IO.pure(Flow, Go{sess, mem})
    case (s, Done{m}):
      repl_got(live, s, mem, Rcp.recv_bytes(m), carry, pend, sess)

@unsafe
def repl_conn(+live: Bool, sock: Socket, +mem: Pvp.CallMem, +carry: List<&2, String>, +pend: List<&2, U32>, sess: T.Session) -> IO(Flow):
  do IO<Flow>:
    rm : Socket & Result<&1, &1, U32 & String, Maybe<&1, List<&2, U32>>> <- TCP.recv_bytes(sock, 4096)
    repl_after(live, rm, mem, carry, pend, sess)

# ---- the accept loop: one connection at a time, session survives ----

# the --continue greeting: sent on the first connection of a restored
# session (a --resume, or the respawn after a /reload), then never
# again. The restored history replays first ("history " lines, see
# R.replay), so the client's fresh feed shows the conversation so far;
# the session_restored line closes the replay.
def greet_lines(+n: Nat, +history: List<&2, T.Msg>) -> List<&2, String>:
  # no idle marker: the greeting is unsolicited, not a turn's end — an
  # idle here would make clients give up waiting for the first turn
  List.append(&2, String, Rp.replay(history),
    Con{OG.session_restored(n), Nil{}})

# the parent respawned this process after a crash (BEND_CRASH_NOTE =
# what the dead process said): the user must see it, right after the
# replayed history - the turn it was running is gone, the history up
# to its last provider call is back
def crash_lines(empty: Bool, +note: String) -> List<&2, String>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      Con{OG.harness_restarted(note), Nil{}}

# `pre`: the session-log configuration facts (core/ev.bend), first on
# every connection (the writer skips a fact equal to its state)
def greet_out(greet: Maybe<&2, Nat>, s: Socket, +mem: Pvp.CallMem,
    +sess: T.Session, +pre: List<&2, String>) -> IO(Pv.Out):
  match greet:
    case Some{+n}:
      do IO<Pv.Out>:
        cr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_CRASH_NOTE")
        +note : String <- IO.pure(String, P.env_or(cr, ""))
        Dr.emit(Pv.ONet{s, mem}, List.concat(&2, String, [pre,
          greet_lines(n, P.history_of(sess)),
          crash_lines(String.is_empty(note), note)]))
    case None{}:
      Dr.emit(Pv.ONet{s, mem}, pre)

# the model the session-log facts name: the resolved one (live), or
# "scripted"
def ev_model(live: Bool) -> IO(String):
  match live:
    case True{}:
      Pv.resolved_model()
    case False{}:
      IO.pure(String, "scripted")

# the restored queue restarts the turn chain by itself (checkpoint v2):
# the runtime idle boundary pops the queued head into a fresh turn,
# so the message the user queued before the reload runs without anyone
# re-typing it; further queued messages chain the same way inside the
# one run loop entry. Turn/pending are still boundary-empty at a reload,
# so only the queue and the held notifications resume here.
def resume_queued.run(+live: Bool, s: Socket, +mem: Pvp.CallMem,
    +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  do IO<Flow>:
    e : R.RtEnd <- repl_run(live, Pv.ONet{s, mem}, sess, Nil{}, Nil{})
    f : Flow <- repl_end_stalled(live, e, next)
    IO.pure(Flow, f)

def resume_queued(+live: Bool, empty: Bool, s: Socket, +mem: Pvp.CallMem,
    +sess: T.Session,
    next: Socket -> Pvp.CallMem -> T.Session -> IO(Flow)) -> IO(Flow):
  match empty:
    case True{}:
      next(s)(mem)(sess)
    case False{}:
      resume_queued.run(live, s, mem, sess, next)

def greet_conn(+live: Bool, out: Pv.Out, +sess: T.Session) -> IO(Flow):
  match out:
    case Pv.OStd{+mem}:
      IO.pure(Flow, Go{sess, mem})
    case Pv.ONet{s2, +mem}:
      resume_queued(live, List.is_empty(&2, String, S.sess_queued(sess)), s2,
        mem, sess,
        s3 => +mem3 => sess3 => repl_conn(live, s3, mem3, Nil{}, Nil{}, sess3))



# the connection loop ended: Go keeps serving (checkpoint + accept the
# next client, with the calls' memory), Stop unwinds to main (the
# reload exit)
def serve_after.cont(f: Flow, l2: Listener,
    next: Listener -> Maybe<&2, Nat> -> T.Session -> Pvp.CallMem -> IO(Flow))
    -> IO(Flow):
  match f:
    case Go{+sess2, +mem2}:
      do IO<Flow>:
        P.save(sess2)
        next(l2)(None{})(sess2)(mem2)
    case Stop{}:
      IO.pure(Flow, Stop{})

def serve_after(+live: Bool, am: Listener & Result<&1, &1, U32 & String, Socket>,
    +greet: Maybe<&2, Nat>, +sess: T.Session, +mem: Pvp.CallMem,
    next: Listener -> Maybe<&2, Nat> -> T.Session -> Pvp.CallMem -> IO(Flow))
    -> IO(Flow):
  match am:
    case (l2, Fail{_}):
      do IO<Flow>:
        u : Unit <- IO.print("accept failed")
        IO.pure(Flow, Stop{})
    case (l2, Done{s}):
      do IO<Flow>:
        +model : String <- ev_model(live)
        out2 : Pv.Out <- greet_out(greet, s, mem, sess,
          Ev.sess_evs(sess, model, "config"))
        f : Flow <- greet_conn(live, out2, sess)
        serve_after.cont(f, l2, next)

@unsafe
def serve.go(+live: Bool, l: Listener, greet: Maybe<&2, Nat>, sess: T.Session,
    +mem: Pvp.CallMem) -> IO(Flow):
  do IO<Flow>:
    am : Listener & Result<&1, &1, U32 & String, Socket> <- TCP.accept(l)
    serve_after(live, am, greet, sess, mem,
      l2 => g2 => s2 => +m2 => serve.go(live, l2, g2, s2, m2))

# the accept loop, from an empty memory of calls (a new process)
@unsafe
def serve(+live: Bool, l: Listener, greet: Maybe<&2, Nat>, sess: T.Session) -> IO(Flow):
  serve.go(live, l, greet, sess, Pvp.mem0())

# the REPL listen port: BEND_REPL_PORT env (default: the entry's) so several
# harness instances can run side by side
def repl_port.pick(r: Result<&1, &1, U32 & String, String>, +dflt: U32)
    -> U32:
  match r:
    case Fail{_}:
      dflt
    case Done{+s}:
      Maybe.default(&2, U32, U32.read(s), dflt)

def repl_port(+dflt: U32) -> IO(U32):
  do IO<U32>:
    env : Result<&1, &1, U32 & String, String> <- IO.get_env(
      "BEND_REPL_PORT")
    IO.pure(U32, repl_port.pick(env, dflt))

def ld_sess(ld: P.Load) -> T.Session:
  match ld:
    case P.LD{+sess, _}:
      sess

def ld_greet(ld: P.Load) -> Maybe<&2, Nat>:
  match ld:
    case P.LD{_, +greet}:
      greet

def flow_note.body(stopped: Bool) -> IO(Unit):
  match stopped:
    case True{}:
      do IO<Unit>:
        u2 : Unit <- IO.print("reload-exit")
        IO.pure(Unit, Unit{})
    case False{}:
      IO.pure(Unit, Unit{})

def flow_note(f: Flow) -> IO(Unit):
  match f:
    case Go{_, _}:
      flow_note.body(False{})
    case Stop{}:
      flow_note.body(True{})

