# runtime/race.bend — a blocking call raced against the interrupt flag
# (issue 13: a stop ends the turn in under 1 s, not when the call
# returns). The decisions are in runtime/race-pure.bend.
#
# until_stop forks the call onto a channel (the worker sends its
# result), spawns a ticker beside it (a tick every Rp.tick_ms), and
# reads the channel: on a tick it reads the flag, and a set flag closes
# the channel and answers Stopped{who}; the result answers Won. The
# flag is only read here, never consumed: the turn's boundary drain
# (runtime/drain.bend) stays its one consumer. Once the channel is
# closed, the ticker's next send answers False and it ends; a result
# that comes later is sent into the closed channel and dropped. The
# call itself is left running: the caller makes it stop (the model
# attempt's cancel file, the bash wrapper's own check of the flag).
# Lives only while one call is in flight.
import Base
import ./persist.bend as P
import ./race-pure.bend as Rp

type Step<-A: Type> is Type:
  STick{}
  SWon{value: A}

type Raced<-A: Type> is Type:
  Won{value: A}
  Stopped{who: String}

@unsafe
def ticker.next(-A: Type, go: Bool, +c: Chan(Step<A>)) -> IO(Unit):
  match go:
    case True{}:
      ticker(A, c)
    case False{}:
      IO.pure(Unit, Unit{})

@unsafe
def ticker(-A: Type, +c: Chan(Step<A>)) -> IO(Unit):
  do IO<Unit>:
    s : Unit <- IO.sleep(Rp.tick_ms())
    sent : Bool <- Chan.send(Step<A>, c, STick{})
    ticker.next(A, Rp.tick_again(sent), c)

def wait.close(-A: Type, +c: Chan(Step<A>), r: Raced<A>) -> IO(Raced<A>):
  do IO<Raced<A>>:
    u : Unit <- Chan.close(Step<A>, c)
    IO.pure(Raced<A>, r)

@unsafe
def wait.verdict(-A: Type, v: Rp.Verdict, +c: Chan(Step<A>), +ipath: String)
    -> IO(Raced<A>):
  match v:
    case Rp.VStop{+who}:
      wait.close(A, c, Stopped{who})
    case _:
      wait(A, c, ipath)

@unsafe
def wait.got(-A: Type, m: Maybe<&1, Step<A>>, +c: Chan(Step<A>), +ipath: String)
    -> IO(Raced<A>):
  match m:
    case None{}:
      # nothing closes the channel but this reader: unreachable
      IO.pure(Raced<A>, Stopped{"the runtime"})
    case Some{st}:
      match st:
        case SWon{a}:
          wait.close(A, c, Won{a})
        case STick{}:
          do IO<Raced<A>>:
            f : Maybe<&2, String> <- P.read_file(ipath)
            wait.verdict(A, Rp.decide(False{}, f), c, ipath)

@unsafe
def wait(-A: Type, +c: Chan(Step<A>), +ipath: String) -> IO(Raced<A>):
  do IO<Raced<A>>:
    m : Maybe<&1, Step<A>> <- Chan.recv(Step<A>, c)
    wait.got(A, m, c, ipath)

def worker(-A: Type, +c: Chan(Step<A>), x: A) -> IO(Bool):
  Chan.send(Step<A>, c, SWon{x})

# ipath: the interrupt flag (Pv.interrupt_path)
@unsafe
def until_stop(-A: Type, +ipath: String, act: IO(A)) -> IO(Raced<A>):
  do IO<Raced<A>>:
    +c : Chan(Step<A>) <- Chan.new(Step<A>, 1)
    w : Unit <- IO.spawn(Bool, IO.bind(A, Bool, act, worker(A, c)))
    t : Unit <- IO.spawn(Unit, ticker(A, c))
    wait(A, c, ipath)
