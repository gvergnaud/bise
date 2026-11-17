# runtime/repl.bend — the scripted REPL entry (no API, no real tools).
#
#   bend runtime/repl.bend -o repl-scripted
#   BISE_REPL_TOKEN=t ./repl-scripted & { echo "hello t"; cat; } | nc 127.0.0.1 7700
#
# Model replies come from the script each user line selects
# (Rc.script_for_entry). The line server is runtime/repl-core.bend.
import Base
import ../core/types.bend as T
import ./persist.bend as P
import ./main.bend as R
import ./repl-core.bend as Rc
import ./debug.bend as Dbg

def main() -> IO(Unit):
  do IO<Unit>:
    dbg : Unit <- Dbg.start()
    +p : U32 <- Rc.repl_port(7700)
    # no token, no say port: it exits here (Rc.say_token)
    +token : String <- Rc.say_token()
    l : Listener <- IO.try(Listener, TCP.listen("127.0.0.1", p))
    +ps : String <- IO.pure(String, U32.show(p))
    # the same info line as the live REPL: no provider here
    +info : String <- R.harness_info("scripted", "400")
    ui : Unit <- IO.print(info)
    IO.print(String.append("bend-harness REPL on 127.0.0.1:",
      String.append(ps,
        String.append(" — connect with: nc 127.0.0.1 ",
          ps))))
    +ld : P.Load <- P.load(T.new_session(R.cfg(400n, 3n)))
    f : Rc.Flow <- Rc.serve(False{}, token, l, Rc.ld_greet(ld), Rc.ld_sess(ld))
    # Stop (a /reload) lands here: main returns, the process exits 0,
    # and the parent respawns the latest code on the same session.
    # The marker line lets the parent tell a reload from a crash.
    u : Unit <- Rc.flow_note(f)
    IO.pure(Unit, Unit{})
