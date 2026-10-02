# runtime/gate-pure.bend — the approvals gate's pure part
# (approvals-design.md §10, approvals.md §3). The runtime asks, the hub
# decides: in `auto`, before a gated call, the runtime prints
# `gate <n> {"tool","args","nonce"}`, then waits on its gate file (the hub
# appends `<n> <nonce> allow` or `<n> <nonce> deny <reason>`) and on its
# interrupt file; it prints `gate-done <n> <how>` when it goes on. The
# mode file next to them says `yolo` or `auto`: in `yolo` nothing asks.
import Base
import ../vendor/json.bend as J
import ./provider-pure.bend as Pvp

type Verdict is Data:
  # flags: what follows `allow` (`net`: the sandbox opens the network,
  # approvals-sandbox), "" for a plain allow
  VAllow{flags: String}
  VDeny{reason: String}
  # ctrl+c on the agent, `sb interrupt`: the call does not run; who:
  # who asked (Pvp.interrupter: "the user", "main")
  VStop{who: String}

# never gated (design §3): the run_typescript wrapper (its calls are gated
# one by one), search_tool_functions, skill, self.*
def gated(+name: String) -> Bool:
  Bool.not(String.eq(name, "run_typescript") || String.eq(name, "search_tool_functions")
    || String.eq(name, "skill") || String.starts_with(name, "self."))

# the mode file's text asks the hub only when it says `auto`
def asks(+mode: String) -> Bool:
  String.eq(String.trim(mode), "auto")

def mode_path(+dir: String) -> String:
  String.append(dir, "/approvals-mode")

def gate_path(+dir: String, +port: String) -> String:
  String.append(dir, String.append("/bend-gate-", String.append(port, ".txt")))

# the line the hub reads, with more JSON fields (`extra` starts with a
# comma, or is "")
def ask_line_x(+n: String, +tool: String, +args: String, +nonce: String,
    +extra: String) -> String:
  String.concat(["gate ", n, " {\"tool\":", J.quote_string(tool), ",\"args\":",
    J.quote_string(args), ",\"nonce\":", J.quote_string(nonce), extra, "}"])

# the line the hub reads (the arguments as the model wrote them)
def ask_line(+n: String, +tool: String, +args: String, +nonce: String) -> String:
  ask_line_x(n, tool, args, nonce, "")

# ---- the sandbox's rerun (approvals-sandbox, approvals-design.md §6.3) ----
# A bash call the sandbox stopped asks the gate a second time, with the
# first run's output (its last 4000 chars: the hub reads the path the
# sandbox refused); an allow runs it again without the sandbox, a no keeps
# the first output and adds why.

def denied_tail(+out: String) -> String:
  String.drop(out, Nat.sub(String.length(out), 4000n))

def denied_extra(+out: String) -> String:
  String.append(",\"denied\":", J.quote_string(denied_tail(out)))

def stopped_result(+out: String, +why: String) -> String:
  String.concat([String.trim(out), "\n\n", why])

def done_line(+n: String, +how: String) -> String:
  String.concat(["gate-done ", n, " ", how])

# what follows `deny` (its reason); a bare `deny` says the user's no
def deny_of(+rest: String) -> Verdict:
  +r = String.trim(String.drop(rest, 4n))
  VDeny{Bool.pick(String, String.is_empty(r), "the user said no.", r)}

def verdict_rest(+rest: String) -> Maybe<&2, Verdict>:
  Bool.pick(Maybe<&2, Verdict>, String.starts_with(rest, "allow"), Some{VAllow{String.trim(String.drop(rest, 5n))}},
    Bool.pick(Maybe<&2, Verdict>, String.starts_with(rest, "deny"), Some{deny_of(rest)}, None{}))

def verdict_lines(ls: List<&2, String>, +pre: String) -> Maybe<&2, Verdict>:
  match ls:
    case Nil{}:
      None{}
    case Con{+l, +t}:
      Bool.pick(Maybe<&2, Verdict>, String.starts_with(l, pre),
        verdict_rest(String.drop(l, String.length(pre))), verdict_lines(t, pre))

# the hub's answer to call `n` in the gate file's text: a line with this
# call's number and nonce (a line with another nonce is not the hub's)
def verdict(+text: String, +n: String, +nonce: String) -> Maybe<&2, Verdict>:
  verdict_lines(String.lines(text), String.concat([n, " ", nonce, " "]))

# the poll: every 20 ms for the first 200 ms, then every 100 ms
def poll_ms(+k: Nat) -> U32:
  Bool.pick(U32, Nat.is_lt(k, 10n), 20, 100)

# the nonce: two random words, never shown to the model
def nonce_of(+a: U32, +b: U32) -> String:
  String.concat(["n", U32.show(a), "x", U32.show(b)])

# an interrupted call's result: "interrupted by main" (or "by the user")
def interrupted(+who: String) -> String:
  Pvp.interrupted_by(who)

# the mode file (None: no hub wrote one, nothing asks)
def mode_asks(m: Maybe<&2, String>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+t}:
      asks(t)

# an interrupt flag with content (Pvp.interrupt_flag: the cleared flag is
# an empty file)
def stopped(m: Maybe<&2, String>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+t}:
      Bool.not(String.is_empty(String.trim(t)))

def found.int(stop: Bool, +who: String) -> Maybe<&2, Verdict>:
  match stop:
    case True{}:
      Some{VStop{who}}
    case False{}:
      None{}

def found.pick(v: Maybe<&2, Verdict>, stop: Bool, +who: String) -> Maybe<&2, Verdict>:
  match v:
    case Some{+x}:
      Some{x}
    case None{}:
      found.int(stop, who)

# one poll: the hub's verdict first, else an interrupt, else nothing yet
def found(g: Maybe<&2, String>, +i: Maybe<&2, String>, +n: String, +nonce: String) -> Maybe<&2, Verdict>:
  match g:
    case None{}:
      found.int(stopped(i), Pvp.interrupter_in(i))
    case Some{+t}:
      found.pick(verdict(t, n, nonce), stopped(i), Pvp.interrupter_in(i))

def how_of(v: Verdict) -> String:
  match v:
    case VAllow{_}:
      "allow"
    case VDeny{_}:
      "deny"
    case VStop{_}:
      "interrupted"

# None: the call runs; Some: its failed result
def denial(v: Verdict) -> Maybe<&2, String>:
  match v:
    case VAllow{_}:
      None{}
    case VDeny{+r}:
      Some{r}
    case VStop{+w}:
      Some{interrupted(w)}

# the flags of an allow (`net`), "" otherwise
def flags(v: Verdict) -> String:
  match v:
    case VAllow{+f}:
      f
    case _:
      ""
