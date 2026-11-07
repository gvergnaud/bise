# runtime/demo.bend — the scripted scenario runner (harness-demo).
#
#   bend runtime/demo.bend -o harness-demo && ./harness-demo
#
# Each scenario drives the Core through the Step Protocol with scripted
# model replies (ADR 0006); the output is deterministic.
import Base
import ../core/types.bend as T
import ../core/checkpoint.bend as K
import ./model.bend as M
import ./main.bend as R
import ./provider.bend as Pv
import ./main-pure.bend as Rp
def say(text: String) -> T.Cmd:
  T.UserMsg{T.Queue{}, text}

def reply(text: String) -> M.Reply:
  M.ROk{text, Nil{}}

def call(id: U32, name: String, args: String) -> M.Reply:
  M.ROk{"", Con{T.Call{id, name, args}, Nil{}}}

def scenario(name: String, +c: T.Cfg, +cmds: List<&2, T.Cmd>,
    +script: List<&2, M.Reply>) -> IO(Unit):
  do IO<Unit>:
    IO.print(String.append("\n=== ", String.append(name, " ===")))
    e : R.RtEnd <- R.run(500n, Pv.std0(),
      Rp.Rt{T.new_session(c), script, cmds, False{}},
      Rp.JIdle{},
      Rp.transition(Rp.Rt{T.new_session(c), script, cmds, False{}}, Rp.JIdle{}))
    IO.pure(Unit, Unit{})

def s_conversation() -> IO(Unit):
  scenario("conversation",
    R.cfg(600n, 3n),
    Con{say("Hello!"), Nil{}},
    Con{reply("Hi! How can I help?"), Nil{}})

def s_tools() -> IO(Unit):
  scenario("tools + discovery + a failing tool",
    R.cfg(600n, 3n),
    Con{say("Find the search tool, search the catalog again, and also try a bogus tool."), Nil{}},
    Con{call(1, "search_tool_functions", "best_match: catalog"),
      Con{call(2, "search_tool_functions", "details: search_tool_functions"),
      Con{call(3, "bogus_tool", "anything"),
      Con{reply("Searched twice and reported the bogus tool."), Nil{}}}}})

def s_program() -> IO(Unit):
  scenario("programmatic tool calling (D1, V8: bend-jsrt)",
    R.cfg(600n, 3n),
    Con{say("Orchestrate: search the catalog and look up a tool's details, then report."), Nil{}},
    Con{call(1, "run_typescript",
        "{\"code\": \"const s = await search_tool_functions({query: 'catalog'}); const d = await search_tool_functions({mode: 'details', functions: ['bash']}); return s;\"}"),
      Con{reply("The program searched the catalog and looked up the details."), Nil{}}})

def s_program_fail() -> IO(Unit):
  scenario("a failing program call fails the program",
    R.cfg(600n, 3n),
    Con{say("Run a program that calls a missing tool."), Nil{}},
    Con{call(1, "run_typescript", "{\"code\": \"const s = await no_such_tool({}); return s;\"}"),
      Con{reply("The program failed on the missing tool as expected."), Nil{}}})

def s_steering() -> IO(Unit):
  scenario("steering mid-turn (ADR 0005)",
    R.cfg(600n, 3n),
    Con{say("Do something."),
      Con{T.UserMsg{T.Steer{}, "Also mention penguins."}, Nil{}}},
    Con{call(1, "search_tool_functions", "best_match: catalog"),
      Con{reply("Done, and penguins are lovely."), Nil{}}})

def s_auto_compaction() -> IO(Unit):
  scenario("automatic in-turn compaction (ADR 0011)",
    R.cfg(304n, 3n),
    Con{say("First: let us talk about the history of the harbour, the cranes, and the ferries that cross every morning."),
      Con{say("Second: the warehouses along the quay stored grain, timber, and later container freight for decades."),
        Con{say("Third: the fishermen mended nets on the pier and told the same stories every single evening there."),
          Con{say("Fourth: now summarize everything about the harbour we discussed so far."), Nil{}}}}},
    Con{reply("The harbour has cranes, ferries, warehouses, and fishermen."),
      Con{reply("Grain and container freight shaped the warehouses."),
        Con{reply("The fishermen's stories repeated every evening."),
          Con{reply("<summary>The user asked about the harbour: cranes, ferries, warehouses with grain and freight, and fishermen telling stories.</summary>"),
            Con{reply("The harbour summary: cranes and ferries, warehouses, fishermen."), Nil{}}}}}})

def s_retry() -> IO(Unit):
  scenario("malformed summary retry, then success (ADR 0011)",
    R.cfg(289n, 3n),
    Con{say("First: the observatory on the hill opened its dome at dusk and tracked the moving planets all night."),
      Con{say("Second: the astronomers logged each transit in leather notebooks, arguing gently about the numbers."),
        Con{say("Third: now compact the observatory conversation."), Nil{}}}},
    Con{reply("The observatory tracked the moving planets from dusk until dawn, and the dome closed only when the sky paled at dawn."),
      Con{reply("The astronomers logged each transit carefully in leather notebooks, and they argued gently about the numbers until dawn."),
        Con{reply(""),
          Con{reply("<summary>The user asked about the hill observatory: it opened at dusk, tracked planets, and the astronomers logged transits while arguing gently.</summary>"),
            Con{reply("The observatory summary is ready."), Nil{}}}}}})

def s_manual_compaction() -> IO(Unit):
  scenario("manual between-turn compaction (ADR 0011)",
    R.cfg(600n, 3n),
    Con{say("Tell me about the lighthouse on the eastern cliff, its lamp, and its keeper who climbed every evening."),
      Con{T.Compact{"Keep the keeper's name if mentioned."}, Nil{}}},
    Con{reply("The lighthouse lamp turned slowly and the keeper climbed each evening."),
      Con{reply("<summary>The user asked about the eastern cliff lighthouse: its rotating lamp and the keeper's evening climbs.</summary>"),
      Nil{}}})

def s_null_limit() -> IO(Unit):
  scenario("null iteration limit (ADR 0015)",
    R.cfg(600n, 1n),
    Con{say("Say something."), Nil{}},
    Con{reply(""), Con{reply(""), Nil{}}})

def s_provider_failure() -> IO(Unit):
  scenario("provider failure fails the turn",
    R.cfg(600n, 3n),
    Con{say("Hello?"), Nil{}},
    Con{M.RFail{}, Nil{}})

def s_interrupt_noop() -> IO(Unit):
  scenario("interrupt mid-turn; the late completion is rejected",
    R.cfg(600n, 3n),
    Con{say("Hi."),
      Con{T.Interrupt{}, Nil{}}},
    Con{reply("Hello!"), Nil{}})

def s_sleep() -> IO(Unit):
  scenario("bounded sleep tool (D9 bis)",
    R.cfg(600n, 3n),
    Con{say("Sleep a moment, then tell me the time you slept."), Nil{}},
    Con{call(1, "self.sleep", "1"),
      Con{reply("I slept one second."), Nil{}}})

def s_checkpoint() -> IO(Unit):
  do IO<Unit>:
    IO.print("\n=== checkpoint roundtrip ===")
    IO.print(String.append("restore(export(s)) is the session by proof; version = ",
      U32.show(K.version(K.export(T.new_session(R.cfg(600n, 3n)))))))

def main() -> IO(Unit):
  do IO<Unit>:
    s_conversation()
    s_tools()
    s_program()
    s_program_fail()
    s_steering()
    s_auto_compaction()
    s_retry()
    s_manual_compaction()
    s_null_limit()
    s_provider_failure()
    s_interrupt_noop()
    s_sleep()
    s_checkpoint()
