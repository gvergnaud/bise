# runtime/debug-pure.bend — the session debug log, pure side.
#
# Every live session keeps a debug directory (BEND_DEBUG_DIR, set by the
# parent: ~/.bise/sessions/<id>.debug/). The REPL appends one
# JSON object per line to events.jsonl: a breadcrumb for every obs and
# tool annotation it sends the client, and an incident for every
# provider attempt that failed (with the request and the raw reply saved
# beside it). This module renders those lines and decides what a
# provider attempt is worth recording; runtime/debug.bend runs the IO.
import Base
import ../vendor/json.bend as J
import ./provider-pure.bend as Pvp

# one field of an event: a string, or a number
type Field is Data:
  FStr{key: String, val: String}
  FNat{key: String, val: Nat}

def field(f: Field) -> String:
  match f:
    case FStr{+k, +v}:
      J.quote_string(k) ++ ":" ++ J.quote_string(v)
    case FNat{+k, +v}:
      J.quote_string(k) ++ ":" ++ Nat.show(v)

def fields(fs: List<&2, Field>) -> String:
  match fs:
    case Nil{}:
      ""
    case Con{f, rest}:
      "," ++ field(f) ++ fields(rest)

# one events.jsonl line (no trailing newline)
def ev_line(+ts: Nat, +kind: String, fs: List<&2, Field>) -> String:
  "{\"ts\":" ++ Nat.show(ts) ++ ",\"src\":\"repl\",\"kind\":"
    ++ J.quote_string(kind) ++ fields(fs) ++ "}"

# a text cut to max chars, with a marker saying how much was dropped
def clip.pick(long: Bool, +s: String, +max: Nat, +len: Nat) -> String:
  match long:
    case True{}:
      String.take(s, max) ++ "\n[... truncated: " ++ Nat.show(len)
        ++ " chars in total]"
    case False{}:
      s

def clip(+s: String, +max: Nat) -> String:
  clip.pick(Nat.is_lt(max, String.length(s)), s, max, String.length(s))

# the most a breadcrumb line keeps, and a raw reply artifact
def crumb_max() -> Nat:
  2000n

def reply_max() -> Nat:
  262144n

# ---- breadcrumbs: what the REPL sent the client ----

# an obs line, either form: the record (`obs <kind> :: <json>`, issue 38)
# or the text (`  obs: …`: older history lines replay as text)
def is_obs_line(+l: String) -> Bool:
  String.starts_with(l, "obs ") || String.starts_with(l, "  obs: ")

def is_crumb(+l: String) -> Bool:
  is_obs_line(l)
    || String.starts_with(l, "tool #")
    || String.starts_with(l, "tool_result #")
    || String.starts_with(l, "tool_view #")

def crumb.pick(keep: Bool, +ts: Nat, +l: String) -> String:
  match keep:
    case True{}:
      ev_line(ts, "sent", Con{FStr{"line", clip(String.trim(l), crumb_max())},
        Nil{}}) ++ "\n"
    case False{}:
      ""

# the events.jsonl text for one batch of client lines ("" when none
# of them is a breadcrumb)
def crumbs(lines: List<&2, String>, +ts: Nat) -> String:
  match lines:
    case Nil{}:
      ""
    case Con{+l, rest}:
      crumb.pick(is_crumb(l), ts, l) ++ crumbs(rest, ts)

# ---- provider attempts: which ones are incidents ----

type Plan is Data:
  PSkip{}
  PRecovered{}
  PFailed{verdict: String, why: String, after_ms: U32}

# a 200 whose reply the mapper turned into an error (a refusal, a
# malformed body) is an incident too: the turn fails on it
def plan.ok(err: Bool, +tries: Nat, +reply: String) -> Plan:
  match err:
    case True{}:
      PFailed{"error_reply", String.take(reply, 500n), 0}
    case False{}:
      match tries:
        case 0n:
          PSkip{}
        case 1n+_:
          PRecovered{}

# what attempt #tries (0-based) of a call is worth recording: nothing
# when the first one worked, a recovery when a later one did, an
# incident for every failure
def plan(a: Pvp.Attempt, +tries: Nat) -> Plan:
  match a:
    case Pvp.AOk{+reply}:
      plan.ok(String.starts_with(reply, "ERROR "), tries, reply)
    case Pvp.AFail{+why}:
      PFailed{"fail", why, 0}
    case Pvp.ARetry{+after_ms, +why}:
      PFailed{"retry", why, after_ms}
    case Pvp.AWait{+why}:
      PFailed{"wait", why, 0}
    case Pvp.ATimeout{_, _, +why}:
      PFailed{"retry", why, 0}

# artifact names: the request once per call, a reply per attempt
def req_name(+call: Nat) -> String:
  "req-" ++ Nat.show(call) ++ ".json"

def reply_name(+call: Nat, +tries: Nat) -> String:
  "reply-" ++ Nat.show(call) ++ "-" ++ Nat.show(Nat.add(tries, 1n)) ++ ".txt"

# the name of the reply artifact, "" when the attempt got no body
def reply_file.pick(empty: Bool, +call: Nat, +tries: Nat) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      reply_name(call, tries)

# the incident line of one failed provider attempt
def failed_line(+ts: Nat, +call: Nat, +tries: Nat, +verdict: String,
    +why: String, +after_ms: U32, +status: U32, +model: String,
    +url: String, +req_len: Nat, +reply_len: Nat) -> String:
  ev_line(ts, "provider_attempt_failed",
    Con{FNat{"call", call},
    Con{FNat{"attempt", Nat.add(tries, 1n)},
    Con{FNat{"max_attempts", Pvp.max_attempts()},
    Con{FStr{"verdict", verdict},
    Con{FStr{"why", why},
    Con{FNat{"status", U32.to_nat(status)},
    Con{FNat{"retry_after_ms", U32.to_nat(after_ms)},
    Con{FStr{"model", model},
    Con{FStr{"url", url},
    Con{FNat{"request_chars", req_len},
    Con{FNat{"reply_chars", reply_len},
    Con{FStr{"request_file", req_name(call)},
    Con{FStr{"reply_file", reply_file.pick(Nat.is_eq(reply_len, 0n), call,
      tries)},
    Nil{}}}}}}}}}}}}}})

def recovered_line(+ts: Nat, +call: Nat, +tries: Nat, +model: String)
    -> String:
  ev_line(ts, "provider_recovered",
    Con{FNat{"call", call},
    Con{FNat{"attempt", Nat.add(tries, 1n)},
    Con{FStr{"model", model}, Nil{}}}})

# ---- the clock: IO.now() is monotonic (ms since boot), the parent logs
# wall-clock ms. At startup the REPL measures offset = epoch - mono once
# (runtime/debug.bend), keeps it in <dir>/clock, and stamps every event
# mono + offset: both sources then share one time scale. ----

def anchor.pick(m: Maybe<&2, Nat>, +mono: Nat) -> Nat:
  match m:
    case None{}:
      0n
    case Some{+epoch}:
      Nat.sub(epoch, mono)

def anchor.value(rest: List<&2, String>, +mono: Nat) -> Nat:
  match rest:
    case Nil{}:
      0n
    case Con{+v, _}:
      anchor.pick(Nat.read(String.trim(v)), mono)

# the offset from the clock probe's answer ("<status>\n<epoch ms>");
# 0 (events stamped with the monotonic clock) when the probe failed
def anchor(ls: List<&2, String>, +mono: Nat) -> Nat:
  match ls:
    case Nil{}:
      0n
    case Con{_, rest}:
      anchor.value(rest, mono)

def stamp.of(m: Maybe<&2, Nat>, +mono: Nat) -> Nat:
  match m:
    case None{}:
      mono
    case Some{+offset}:
      Nat.add(mono, offset)

# an event's ts from the stored offset (none stored: the monotonic ms)
def stamp(m: Maybe<&2, String>, +mono: Nat) -> Nat:
  match m:
    case None{}:
      mono
    case Some{+c}:
      stamp.of(Nat.read(String.trim(c)), mono)
