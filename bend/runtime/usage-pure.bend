# runtime/usage-pure.bend - the token usage of one provider reply.
#
# After each model call the REPL prints one feed line the clients read
# (the TUI shows the context fill of the agent from it):
#   `obs usage :: {"cache_read":R,"cache_write":W,"input":I,"model":M,"output":O}`
# in: every input token of the call (cached ones included) - the size
# of the context the model saw; out: the tokens it wrote. No usage in
# the reply (or an error status): no line (""). Pure: no IO.
import Base
import ../core/api.bend as Api
import ../core/wire.bend as W
import ../vendor/json.bend as J
import ../core/ev.bend as Ev
import ../core/gen/obs.bend as OG

def usage_line.fields(+inp: Nat, +out: Nat, +cr: Nat, +cw: Nat,
    +model: String) -> String:
  OG.usage(model, inp, out, cr, cw)

# the family reads its own usage object (core/api.bend, Api.usage_of)
def usage_line.of(u: Api.Usage, +model: String) -> String:
  match u:
    case Api.Usage{+inp, +out, +cr, +cw}:
      usage_line.fields(inp, out, cr, cw, model)

def usage_line.usage(u: J.Json, st: Api.Style, +model: String) -> String:
  match u:
    case J.JObj{+kvs}:
      usage_line.of(Api.usage_of(st, J.JObj{kvs}), model)
    case _:
      ""

# the body: a whole JSON reply carries "usage"; a stream (Server-Sent
# Events) spreads it over its events (the family's Api.stream_usage)
def usage_line.body(is_json: Bool, st: Api.Style, +body: String) -> J.Json:
  match is_json:
    case True{}:
      W.field_or(W.parse_json(body), "usage", J.JNull{})
    case False{}:
      Api.stream_usage(st, body)

def usage_line.ok(ok: Bool, +st: Api.Style, +model: String, +body: String)
    -> String:
  match ok:
    case True{}:
      usage_line.usage(
        usage_line.body(String.starts_with(String.trim(body), "{"), st, body),
        st, model)
    case False{}:
      ""

def usage_line(st: Api.Style, +model: String, +status: U32, +body: String)
    -> String:
  usage_line.ok(U32.is_eq(status, 200), st, model, body)

# the same usage as a session-log fact (core/ev.bend): req is the
# Core's action id of the request; no usage: ""
def usage_ev.of(u: Api.Usage, +model: String, +req: U32, +t: Ev.Timing)
    -> String:
  match u:
    case Api.Usage{+inp, +out, +cr, +cw}:
      Ev.usage(req, model, inp, out, cr, cw, t)

def usage_ev.usage(u: J.Json, st: Api.Style, +model: String, +req: U32,
    +t: Ev.Timing) -> String:
  match u:
    case J.JObj{+kvs}:
      usage_ev.of(Api.usage_of(st, J.JObj{kvs}), model, req, t)
    case _:
      ""

def usage_ev.ok(ok: Bool, +st: Api.Style, +model: String, +body: String,
    +req: U32, +t: Ev.Timing) -> String:
  match ok:
    case True{}:
      usage_ev.usage(
        usage_line.body(String.starts_with(String.trim(body), "{"), st, body),
        st, model, req, t)
    case False{}:
      ""

# t: the call's times (Ev.Timing, Us.timing)
def usage_ev(st: Api.Style, +model: String, +status: U32, +body: String,
    +req: U32, +t: Ev.Timing) -> String:
  usage_ev.ok(U32.is_eq(status, 200), st, model, body, req, t)

# the times of an attempt from its clock readings (epoch ms): start
# (before its body was built), sent (the body built, the send begins),
# head (the response's head; 0: none), end; save_ms: the checkpoint's
# time before the call; tries: the attempt (0-based). ttfb: the head
# after the send (0 with no head); every difference floors at 0.
def timing(+start: Nat, +sent: Nat, +head: Nat, +end: Nat, +save_ms: Nat,
    +tries: Nat) -> Ev.Timing:
  Ev.Timing{Nat.sub(sent, start), save_ms,
    Bool.pick(Nat, Nat.is_eq(head, 0n), 0n, Nat.sub(head, sent)),
    Nat.sub(end, start), Nat.add(tries, 1n)}
