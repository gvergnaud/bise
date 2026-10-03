# core/oai-resp.bend — the OpenAI Responses family (BISE-147), pure Bend.
#
# OpenAI's reasoning models (the gpt-6 family) take function tools with
# reasoning on the Responses API only (/v1/responses); Chat Completions
# refuses tools with reasoning_effort. The mapping follows what OpenCode
# (the AI SDK's openai.responses()) and Codex send, stateless:
# - store false, include ["reasoning.encrypted_content"]: nothing is kept
#   on OpenAI's side, every request replays the whole conversation, the
#   reasoning items included (their encrypted_content, no id);
# - reasoning {effort: the call's effort, summary: "auto"} for a model
#   the catalog marks as reasoning;
# - input items: system -> developer message; user/context -> user
#   message (input_text / input_image parts when it carries images);
#   assistant -> its reasoning items (the think spans signed "oair:",
#   core/wire.bend resp_sig), its text as an output_text message, its
#   calls as function_call items; tool -> function_call_output, paired
#   FIFO with the calls (the ids "call_<n>", as core/oai-chat.bend);
# - tools: {type: function, name (W.wire_name), description,
#   parameters, strict: false} (the API's strict default would refuse
#   bise's schemas), tool_choice auto.
# The reply (whole, or the stream's response.completed) becomes the
# OK/CALL/END reply the Core parses: a reasoning item is a think span
# signed "oair:<encrypted_content>" (its summary as the text), the
# message texts follow, each function_call is a CALL line.

import Base
import ./image.bend as Im
import ./wire.bend as W
import ./oai-chat.bend as Oai
import ./text.bend as Tx
import ../vendor/json.bend as J

# ---- tools ----

def tool_json(t: W.WTool) -> J.Json:
  match t:
    case W.WTool{+name, +desc}:
      J.obj(Con{J.kv("type", J.str("function")),
        Con{J.kv("name", J.str(W.wire_name(name))),
        Con{J.kv("description", J.str(desc)),
        Con{J.kv("parameters", W.tool_schema(name)),
        Con{J.kv("strict", J.JBool{False{}}), Nil{}}}}}})

def tools_json(ts: List<&2, W.WTool>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case Con{+t, rest}:
      Con{tool_json(t), tools_json(rest)}

# ---- the ChatGPT plan route (shape = "chatgpt-plan", docs/subscriptions-design.md) ----
# OpenAI's plan route takes function tools only inside a namespace
# (OpenAI's function-calling docs, "Defining namespaces"; Codex's
# create_tools_json_for_responses_lite): every tool goes in one
# {"type":"namespace","name":"bise",...}. A call comes back as a
# function_call with the tool's own name and "namespace":"bise" (Codex's
# ResponseItem::FunctionCall), so the CALL line reads `name` as before;
# a replayed call carries the namespace back, as Codex replays it.

def PLAN_NS() -> String:
  "bise"

def plan_tools_json(+ts: List<&2, W.WTool>) -> List<&2, J.Json>:
  [J.obj(Con{J.kv("type", J.str("namespace")),
    Con{J.kv("name", J.str(PLAN_NS())),
    Con{J.kv("description", J.str("bise's tools")),
    Con{J.kv("tools", J.arr(tools_json(ts))), Nil{}}}}})]

def tools_for(plan: Bool, +ts: List<&2, W.WTool>) -> List<&2, J.Json>:
  match plan:
    case True{}:
      plan_tools_json(ts)
    case False{}:
      tools_json(ts)

# a replayed function_call item gets "namespace":"bise" last; any other
# item is unchanged
def plan_item.call(is_call: Bool, +o: J.Json) -> J.Json:
  match is_call:
    case True{}:
      match o:
        case J.JObj{+kvs}:
          J.JObj{List.append(&2, Sigma<&2, &2, String, _ => J.Json>, kvs,
            Con{J.kv("namespace", J.str(PLAN_NS())), Nil{}})}
        case _:
          o
    case False{}:
      o

def plan_items(os: List<&2, J.Json>) -> List<&2, J.Json>:
  match os:
    case Nil{}:
      Nil{}
    case Con{+o, rest}:
      Con{plan_item.call(String.eq(J.get_str_or(o, "type", ""), "function_call"), o),
        plan_items(rest)}

def items_for(plan: Bool, +os: List<&2, J.Json>) -> List<&2, J.Json>:
  match plan:
    case True{}:
      plan_items(os)
    case False{}:
      os

# ---- input items ----

def item_msg(+role: String, +content: J.Json) -> J.Json:
  J.obj(Con{J.kv("role", J.str(role)),
    Con{J.kv("content", content), Nil{}}})

def item_asst_text(+text: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("message")),
    Con{J.kv("role", J.str("assistant")),
    Con{J.kv("content", J.arr(Con{J.obj(Con{J.kv("type", J.str("output_text")),
      Con{J.kv("text", J.str(text)), Nil{}}}), Nil{}})), Nil{}}}})

def summary_json.pick(empty: Bool, +text: String) -> J.Json:
  match empty:
    case True{}:
      J.arr(Nil{})
    case False{}:
      J.arr(Con{J.obj(Con{J.kv("type", J.str("summary_text")),
        Con{J.kv("text", J.str(text)), Nil{}}}), Nil{}})

def item_reasoning(+summary: String, +enc: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("reasoning")),
    Con{J.kv("summary", summary_json.pick(String.is_empty(summary), summary)),
    Con{J.kv("encrypted_content", J.str(enc)), Nil{}}}})

def item_call(c: W.WCall) -> J.Json:
  match c:
    case W.WCall{+id, +name, +args}:
      J.obj(Con{J.kv("type", J.str("function_call")),
        Con{J.kv("call_id", J.str(W.call_id_of(id))),
        Con{J.kv("name", J.str(W.wire_name(name))),
        Con{J.kv("arguments", J.str(Oai.args_json(name, args))), Nil{}}}}})

def items_calls(cs: List<&2, W.WCall>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, rest}:
      Con{item_call(c), items_calls(rest)}

def item_output(+id: String, +content: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("function_call_output")),
    Con{J.kv("call_id", J.str(id)),
    Con{J.kv("output", W.img_content_of("tool", Im.IResp{}, content)), Nil{}}}})

# the plain text between two spans: a whitespace-only gap is no item
def seg_text.pick(empty: Bool, +t: String) -> List<&2, J.Json>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      Con{item_asst_text(t), Nil{}}

def seg_text(+t: String) -> List<&2, J.Json>:
  seg_text.pick(String.is_empty(String.trim(t)), String.trim(t))

# a span this family signed replays as its reasoning item; any other
# span (unsigned, or Anthropic's) is dropped: the model re-thinks
def seg_think.pick(own: Bool, +reason: String, +sig: String)
    -> List<&2, J.Json>:
  match own:
    case True{}:
      Con{item_reasoning(reason, W.resp_enc(sig)), Nil{}}
    case False{}:
      Nil{}

def seg_think(tm: W.ThinkMark) -> List<&2, J.Json>:
  match tm:
    case W.TM{+reason, +sig}:
      seg_think.pick(W.is_resp_sig(sig), reason, sig)

# every span of the message, in order (core/anthropic.bend's walk)
def asst_items.go(fuel: Nat, pr: Maybe<&2, W.ThinkSpan>, +content: String,
    +calls: List<&2, W.WCall>) -> List<&2, J.Json>:
  match fuel:
    case 0n:
      List.append(&2, J.Json, seg_text(W.asst_think.visible(content)),
        items_calls(calls))
    case 1n+k:
      match pr:
        case None{}:
          List.append(&2, J.Json, seg_text(W.asst_think.visible(content)),
            items_calls(calls))
        case Some{W.TSpan{+n0, +n1}}:
          +rest = W.asst_think.raw_visible(content, n1)
          List.append(&2, J.Json, seg_text(String.take(content, n0)),
            List.append(&2, J.Json,
              seg_think(W.think_split(W.asst_think.raw_span(content, n0, n1))),
              asst_items.go(k, W.asst_think.find(rest), rest, calls)))

def asst_items(+content: String, +calls: List<&2, W.WCall>)
    -> List<&2, J.Json>:
  asst_items.go(64n, W.asst_think.find(content), content, calls)

# one message's items, and the call ids still waiting for their result
type RP is Data:
  RP{items: List<&2, J.Json>, ids: List<&2, String>}

def msg_items.tool(ids: List<&2, String>, +content: String) -> RP:
  match ids:
    case Con{+id, rest}:
      RP{Con{item_output(id, content), Nil{}}, rest}
    case Nil{}:
      RP{Con{item_output("call_0", content), Nil{}}, Nil{}}

def msg_items.plain(is_sys: Bool, +role: String, +content: String,
    ids: List<&2, String>) -> RP:
  match is_sys:
    case True{}:
      RP{Con{item_msg("developer", J.str(content)), Nil{}}, ids}
    case False{}:
      RP{Con{item_msg(W.role_out(role), W.img_content_of(role, Im.IResp{}, content)),
        Nil{}}, ids}

def msg_items.pick(is_tool: Bool, is_asst: Bool, +role: String,
    +content: String, +calls: List<&2, W.WCall>, ids: List<&2, String>)
    -> RP:
  match is_tool:
    case True{}:
      msg_items.tool(ids, content)
    case False{}:
      match is_asst:
        case True{}:
          RP{asst_items(content, calls),
            List.append(&2, String, ids, W.ids_of(calls))}
        case False{}:
          msg_items.plain(String.eq(role, "system"), role, content, ids)

def msg_items(m: W.WMsg, ids: List<&2, String>) -> RP:
  match m:
    case W.WMsg{+role, +content, +calls}:
      msg_items.pick(String.eq(role, "tool"), String.eq(role, "assistant"),
        role, content, calls, ids)

type RWalk is Data:
  RNext{todo: List<&2, W.WMsg>, ids: List<&2, String>, acc: List<&2, J.Json>}
  RPair{p: RP, todo: List<&2, W.WMsg>, acc: List<&2, J.Json>}

def items_go(fuel: Nat, w: RWalk) -> List<&2, J.Json>:
  match fuel:
    case 0n:
      Nil{}
    case 1n+k:
      match w:
        case RNext{Nil{}, _, +acc}:
          List.reverse(&2, J.Json, acc)
        case RNext{Con{+m, +rest}, +ids, +acc}:
          items_go(k, RPair{msg_items(m, ids), rest, acc})
        case RPair{+p, +todo, +acc}:
          match p:
            case RP{+items, +ids2}:
              items_go(k, RNext{todo, ids2,
                List.append(&2, J.Json, List.reverse(&2, J.Json, items), acc)})

def input_items(+msgs: List<&2, W.WMsg>) -> List<&2, J.Json>:
  items_go(Nat.add(Nat.mul(List.length(&2, W.WMsg, msgs), 2n), 16n),
    RNext{msgs, Nil{}, Nil{}})

# ---- the body ----

# what the catalog says of the model (core/api.bend MFacts): whether it
# reasons, its output cap (0n: none sent), the call's effort ("": the
# API's default), whether the provider is a ChatGPT plan (shape =
# "chatgpt-plan": no output cap ever, the tools in one namespace)
type Opts is Data:
  Opts{reasoning: Bool, output: Nat, effort: String, plan: Bool}

def reasoning_json.effort(empty: Bool, +effort: String) -> J.Json:
  match empty:
    case True{}:
      J.obj(Con{J.kv("summary", J.str("auto")), Nil{}})
    case False{}:
      J.obj(Con{J.kv("effort", J.str(effort)),
        Con{J.kv("summary", J.str("auto")), Nil{}}})

# a reasoning model: its effort and summaries, and the encrypted
# reasoning back (the stateless replay)
def body.reasoning(reasoning: Bool, +effort: String,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match reasoning:
    case True{}:
      Con{J.kv("reasoning", reasoning_json.effort(String.is_empty(effort), effort)),
        Con{J.kv("include", J.arr(Con{J.str("reasoning.encrypted_content"), Nil{}})),
          rest}}
    case False{}:
      rest

def body.cap(none: Bool, +output: Nat,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match none:
    case True{}:
      rest
    case False{}:
      Con{J.kv("max_output_tokens", J.JNum{output}), rest}

# a compaction call offers no tools at all
def body.tools(empty: Bool, plan: Bool, +tools: List<&2, W.WTool>,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match empty:
    case True{}:
      rest
    case False{}:
      Con{J.kv("tools", J.arr(tools_for(plan, tools))),
        Con{J.kv("tool_choice", J.str("auto")), rest}}

# the plan route sends no output cap (it refuses max_output_tokens);
# the body never carries temperature, top_p, metadata, user,
# truncation, previous_response_id or prompt_cache_retention on any
# route, and the stream's "stream": true comes from stream_body
def api_body(+model: String, +o: Opts, +req: W.WReq) -> String:
  match o:
    case Opts{+reasoning, +output, +effort, +plan}:
      match req:
        case W.WReq{+tools, +msgs, _}:
          J.stringify(J.obj(Con{J.kv("model", J.str(model)),
            Con{J.kv("store", J.JBool{False{}}),
              body.reasoning(reasoning, effort,
                body.cap(Bool.or(plan, Nat.is_eq(output, 0n)), output,
                  Con{J.kv("input", J.arr(items_for(plan, input_items(msgs)))),
                    body.tools(W.is_empty_tools(tools), plan, tools, Nil{})}))}}))

# the stream's body: the same, with "stream": true first
def stream_body(+body: String) -> String:
  String.append("{\"stream\":true,", String.drop(body, 1n))

# ---- the reply: a response object -> OK/CALL/END ----

def type_of(+o: J.Json) -> String:
  J.get_str_or(o, "type", "")

# the texts of a list of parts of one type ("summary_text",
# "output_text", "refusal"), newline-joined
def part_texts.put(hit: Bool, +p: J.Json, +rest: List<&2, String>)
    -> List<&2, String>:
  match hit:
    case True{}:
      Con{String.append(J.get_str_or(p, "text", ""), J.get_str_or(p, "refusal", "")),
        rest}
    case False{}:
      rest

def part_texts(parts: List<&2, J.Json>, +ty: String) -> List<&2, String>:
  match parts:
    case Nil{}:
      Nil{}
    case Con{+p, rest}:
      part_texts.put(Bool.or(String.eq(type_of(p), ty),
          Bool.and(String.eq(ty, "output_text"), String.eq(type_of(p), "refusal"))),
        p, part_texts(rest, ty))

def sig_of.pick(empty: Bool, +enc: String) -> String:
  Bool.pick(String, empty, "", W.resp_sig(enc))

def think_of(+o: J.Json) -> String:
  +enc = J.get_str_or(o, "encrypted_content", "")
  W.think_marked(String.join(part_texts(W.arr_field_or(o, "summary"), "summary_text"), "\n\n"),
    sig_of.pick(String.is_empty(enc), enc))

def call_line(+n: Nat, +o: J.Json) -> String:
  +name = J.get_str_or(o, "name", "")
  String.concat(["CALL ", Nat.show(n), " ", name, " : ",
    String.trim(W.wire_encode(Oai.arg_of(name, J.get_str_or(o, "arguments", ""))))])

# the fold of the output items: the texts newest first, the CALL lines
# newest first, the number of calls so far
type RAcc is Data:
  RA{texts: List<&2, String>, calls: List<&2, String>, n: Nat}

def out_step.call(is_call: Bool, +o: J.Json, +a: RAcc) -> RAcc:
  match is_call:
    case True{}:
      match a:
        case RA{+ts, +cs, +n}:
          RA{ts, Con{call_line(Nat.add(n, 1n), o), cs}, Nat.add(n, 1n)}
    case False{}:
      a

def out_step.msg(is_msg: Bool, +o: J.Json, +a: RAcc) -> RAcc:
  match is_msg:
    case True{}:
      match a:
        case RA{+ts, +cs, +n}:
          RA{Con{String.join(part_texts(W.arr_field_or(o, "content"), "output_text"), ""), ts},
            cs, n}
    case False{}:
      out_step.call(String.eq(type_of(o), "function_call"), o, a)

def out_step.think(is_think: Bool, +o: J.Json, +a: RAcc) -> RAcc:
  match is_think:
    case True{}:
      match a:
        case RA{+ts, +cs, +n}:
          RA{Con{think_of(o), ts}, cs, n}
    case False{}:
      out_step.msg(String.eq(type_of(o), "message"), o, a)

def out_step(+o: J.Json, +a: RAcc) -> RAcc:
  out_step.think(String.eq(type_of(o), "reasoning"), o, a)

def outs(os: List<&2, J.Json>, +a: RAcc) -> RAcc:
  match os:
    case Nil{}:
      a
    case Con{+o, rest}:
      outs(rest, out_step(o, a))

def reply_ok.acc(+a: RAcc) -> String:
  match a:
    case RA{+ts, +cs, _}:
      W.join_lines(Con{String.append("OK ",
          W.escape_nl(String.trim(String.join(List.reverse(&2, String, ts), "\n")))),
        List.append(&2, String, List.reverse(&2, String, cs), Con{"END", Nil{}})})

def reply_ok(+r: J.Json) -> String:
  reply_ok.acc(outs(W.arr_field_or(r, "output"), RA{Nil{}, Nil{}, 0n}))

# a response whose "error" is set (status failed) is the error reply
def reply_resp.err(e: J.Json, +r: J.Json) -> String:
  match e:
    case J.JObj{_}:
      W.reply_err(200, J.stringify(r))
    case _:
      reply_ok(r)

def reply_resp(+r: J.Json) -> String:
  reply_resp.err(W.field_or(r, "error", J.JNull{}), r)

def reply_of.check(ok: Bool, +status: U32, +body: String) -> String:
  match ok:
    case True{}:
      reply_resp(W.parse_json(body))
    case False{}:
      W.reply_err(status, body)

def reply_of(+status: U32, +body: String) -> String:
  reply_of.check(U32.is_eq(status, 200), status, body)

# ---- the stream: its last response.completed / incomplete / failed ----
# Every Responses stream ends with the whole response object (output
# items, usage) in its terminal event; the deltas before it are the
# live view, the fold keeps only that object (an "error" event wraps as
# {"error": ...}). Only the data lines of those events are parsed.

def terminal(+head: String) -> Bool:
  Bool.or(String.contains(head, "\"response.completed\""),
    Bool.or(String.contains(head, "\"response.incomplete\""),
      String.contains(head, "\"response.failed\"")))

def fin.parsed(r: Result<&2, &2, String, J.Json>, is_err: Bool,
    +prev: Maybe<&2, J.Json>) -> Maybe<&2, J.Json>:
  match r:
    case Done{+e}:
      Bool.pick(Maybe<&2, J.Json>, is_err,
        Some{J.obj(Con{J.kv("error", e), Nil{}})},
        Some{W.field_or(e, "response", J.JNull{})})
    case Fail{_}:
      prev

def fin.data(+data: String, +prev: Maybe<&2, J.Json>) -> Maybe<&2, J.Json>:
  +head = String.take(data, 96n)
  Bool.pick(Maybe<&2, J.Json>,
    Bool.or(terminal(head), String.contains(head, "\"type\":\"error\"")),
    fin.parsed(J.parse(data), String.contains(head, "\"type\":\"error\""), prev),
    prev)

def fin.line(is_data: Bool, +l: String, +prev: Maybe<&2, J.Json>)
    -> Maybe<&2, J.Json>:
  match is_data:
    case True{}:
      fin.data(String.trim(String.drop(l, 5n)), prev)
    case False{}:
      prev

def fin(lines: List<&2, String>, +prev: Maybe<&2, J.Json>)
    -> Maybe<&2, J.Json>:
  match lines:
    case Nil{}:
      prev
    case Con{+l, rest}:
      fin(rest, fin.line(String.starts_with(l, "data:"), l, prev))

def final_of(+sse: String) -> Maybe<&2, J.Json>:
  fin(String.split(sse, Tx.newline()), None{})

def reply.of(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{+r}:
      reply_resp(r)
    case None{}:
      "ERROR provider 200: the stream ended before the response was complete\nEND\n"

def reply(+sse: String) -> String:
  reply.of(final_of(sse))

# a 200 is a stream, or a whole JSON (a server that ignored "stream")
def reply_any.pick(is_json: Bool, +body: String) -> String:
  match is_json:
    case True{}:
      reply_of(200, body)
    case False{}:
      reply(body)

def reply_any(+body: String) -> String:
  reply_any.pick(String.starts_with(String.trim(body), "{"), body)

# ---- the token usage (runtime/usage-pure.bend reads it) ----
def usage.of(m: Maybe<&2, J.Json>) -> J.Json:
  match m:
    case Some{+r}:
      W.field_or(r, "usage", J.JNull{})
    case None{}:
      J.JNull{}

def usage(+sse: String) -> J.Json:
  usage.of(final_of(sse))

# ---- the error code of a reply (the ChatGPT plan's errors) ----
# The code OpenAI put in the error of a reply, "" when there is none: a
# whole JSON body's error.code (a non-200 answer, or a 200 that ignored
# "stream"), else the stream's terminal event's (response.failed's
# response.error.code, or an error event's code). runtime/
# provider-pure.bend turns the plan's codes into bise's lines.
def error_code.of(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{+r}:
      J.get_str_or(W.field_or(r, "error", J.JNull{}), "code", "")
    case None{}:
      ""

def error_code.pick(is_json: Bool, +body: String) -> String:
  match is_json:
    case True{}:
      error_code.of(Some{W.parse_json(body)})
    case False{}:
      error_code.of(final_of(body))

def error_code(+body: String) -> String:
  error_code.pick(String.starts_with(String.trim(body), "{"), body)
