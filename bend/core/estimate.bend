# core/estimate.bend — the deterministic model-input budget (ADR 0011).
# One approximate token per four bytes of the projected input, plus per-item
# overhead. Pure and total: the same input always estimates the same.
import Base
import ./types.bend as T

def text_tokens(s: String) -> Nat:
  Nat.div(String.length(s), 4n)

def call_tokens(c: T.Call) -> Nat:
  match c:
    case T.Call{_, name, args}:
      Nat.add(1n, Nat.add(text_tokens(name), text_tokens(args)))

def calls_tokens(cs: List<&2, T.Call>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case Con{c, rest}:
      Nat.add(call_tokens(c), calls_tokens(rest))

def msg_tokens(m: T.Msg) -> Nat:
  match m:
    case T.Msg{_, +text, calls, _, _}:
      Nat.add(1n, Nat.add(text_tokens(text), calls_tokens(calls)))

def msgs_tokens(ms: List<&2, T.Msg>) -> Nat:
  match ms:
    case Nil{}:
      0n
    case Con{m, rest}:
      Nat.add(msg_tokens(m), msgs_tokens(rest))

def tool_tokens(ts: List<&2, T.ToolDef>) -> Nat:
  match ts:
    case Nil{}:
      0n
    case Con{t, rest}:
      match t:
        case T.ToolDef{name, desc}:
          Nat.add(2n, Nat.add(Nat.add(text_tokens(name), text_tokens(desc)),
            tool_tokens(rest)))

# estimated size of the complete model input for one session state:
# system message + tool catalog + projected history
def input_tokens(cfg: T.Cfg, history: List<&2, T.Msg>) -> Nat:
  match cfg:
    case T.Cfg{_, _, _, +system, +tools}:
      Nat.add(Nat.add(text_tokens(system), tool_tokens(tools)),
        Nat.add(4n, msgs_tokens(history)))
