# core/oai-chat.bend — the OpenAI Chat Completions family, pure Bend.
#
# Converts the parsed wire request (core/wire.bend WReq) into an
# OpenAI-compatible chat-completions JSON body, and a provider response
# back into the OK/CALL/END reply the Core parses (runtime/remote.bend
# parse_reply). This is the port of what the Rust bridge used to do: the
# mapping lives in pure Bend and is pinned by bend/laws/.
#
# Tool results pair with assistant calls FIFO on the ids ("call_<n>"),
# exactly as the bridge did; GLM content arrives as an array of blocks
# whose text fields are joined.

import Base
import ./image.bend as Im
import ./wire.bend as W
import ../vendor/json.bend as J

# ---- wire request -> OpenAI Chat JSON body ----

def tool_fn_json(+name: String, +desc: String) -> J.Json:
  J.obj(Con{J.kv("name", J.str(W.wire_name(name))),
    Con{J.kv("description", J.str(desc)),
      Con{J.kv("parameters", W.tool_schema(name)), Nil{}}}})

def tool_json(t: W.WTool) -> J.Json:
  match t:
    case W.WTool{+name, +desc}:
      J.obj(Con{J.kv("type", J.str("function")),
        Con{J.kv("function", tool_fn_json(name, desc)), Nil{}}})

def tools_json.link(rest_j: J.Json, +t: W.WTool) -> J.Json:
  match rest_j:
    case J.JArr{+vals}:
      J.JArr{Con{tool_json(t), vals}}
    case _:
      rest_j

def tools_json(ts: List<&2, W.WTool>) -> J.Json:
  match ts:
    case Nil{}:
      J.JArr{Nil{}}
    case Con{+t, rest}:
      tools_json.link(tools_json(rest), t)

def args_json.structured(is_structured: Bool, +args: String) -> String:
  match is_structured:
    case True{}:
      args
    case False{}:
      J.stringify(W.flat_out(args))

def args_json(+name: String, +args: String) -> String:
  args_json.structured(W.is_structured(name), args)

def call_json(c: W.WCall) -> J.Json:
  match c:
    case W.WCall{+id, +name, +args}:
      J.obj(Con{J.kv("id", J.str(W.call_id_of(id))),
        Con{J.kv("type", J.str("function")),
          Con{J.kv("function",
            J.obj(Con{J.kv("name", J.str(W.wire_name(name))),
              Con{J.kv("arguments", J.str(args_json(name, args))), Nil{}}})),
            Nil{}}}})

def calls_json.link(rest_j: J.Json, +c: W.WCall) -> J.Json:
  match rest_j:
    case J.JArr{+vals}:
      J.JArr{Con{call_json(c), vals}}
    case _:
      rest_j

def calls_json(calls: List<&2, W.WCall>) -> J.Json:
  match calls:
    case Nil{}:
      J.JArr{Nil{}}
    case Con{+c, rest}:
      calls_json.link(calls_json(rest), c)

# tool results pair with assistant calls FIFO: each tool message takes
# the oldest unmatched id
def msg_json.tool(ids: List<&2, String>, +content: String)
    -> J.Json & List<&2, String>:
  match ids:
    case Con{+id, rest}:
      (J.obj(Con{J.kv("role", J.str("tool")),
        Con{J.kv("content", W.img_content_of("tool", Im.IOai{},
          W.tool_text(content))),
          Con{J.kv("tool_call_id", J.str(id)), Nil{}}}}), rest)
    case Nil{}:
      (J.obj(Con{J.kv("role", J.str("tool")),
        Con{J.kv("content", W.img_content_of("tool", Im.IOai{},
          W.tool_text(content))),
          Con{J.kv("tool_call_id", J.str("call_0")), Nil{}}}}), Nil{})

def content_json.pick(empty: Bool, +text: String) -> J.Json:
  match empty:
    case True{}:
      J.JNull{}
    case False{}:
      J.str(text)

# blank: null, which goes only with tool_calls (W.sendable drops a
# blank assistant message without any)
def content_json(+text: String) -> J.Json:
  content_json.pick(W.is_blank(text), text)

# the exact vibe ThinkChunk shape: the thinking block first, the text
# block second (a thinking-only assistant message carries no text block)
def thinking_parts(+reasoning: String) -> J.Json:
  J.arr(Con{J.obj(Con{J.kv("type", J.str("text")),
    Con{J.kv("text", J.str(reasoning)), Nil{}}}), Nil{}})

def think_block_json.pick(sig: Maybe<&2, String>, +reasoning: String) -> J.Json:
  match sig:
    case Some{+s}:
      J.obj(Con{J.kv("type", J.str("thinking")),
        Con{J.kv("thinking", thinking_parts(reasoning)),
        Con{J.kv("signature", J.str(s)), Nil{}}}})
    case None{}:
      J.obj(Con{J.kv("type", J.str("thinking")),
        Con{J.kv("thinking", thinking_parts(reasoning)), Nil{}}})

def asst_content.marked.pick(empty_vis: Bool, +reasoning: String,
    +visible: String, sig: Maybe<&2, String>) -> J.Json:
  match empty_vis:
    case True{}:
      J.arr(Con{think_block_json.pick(sig, reasoning), Nil{}})
    case False{}:
      J.arr(Con{think_block_json.pick(sig, reasoning),
        Con{W.text_block_json(visible), Nil{}}})

def asst_content.pick_signed.signed(tm: W.ThinkMark, +visible: String)
    -> J.Json:
  match tm:
    case W.TM{+reason, +sig}:
      asst_content.marked.pick(W.is_blank(visible), reason,
        visible, Some{sig})

def asst_content.pick_signed(is_unsigned: Bool, tm: W.ThinkMark,
    +reasoning: String, +visible: String) -> J.Json:
  match is_unsigned:
    case True{}:
      asst_content.marked.pick(W.is_blank(visible), reasoning,
        visible, None{})
    case False{}:
      asst_content.pick_signed.signed(tm, visible)

def asst_content.split(tm: W.ThinkMark, +content: String, +n0: Nat,
    +n1: Nat) -> J.Json:
  match tm:
    case W.TM{+reason, +sig}:
      asst_content.pick_signed(String.is_empty(W.bare_sig(sig)),
        W.TM{reason, sig},
        W.asst_think.reasoning(content, n0, n1),
        W.asst_think.visible(W.asst_think.raw_visible(content, n1)))

def asst_content.found(pr: W.ThinkSpan, +content: String) -> J.Json:
  match pr:
    case W.TSpan{+n0, +n1}:
      asst_content.split(W.think_split(W.asst_think.raw_span(content, n0, n1)),
        content, n0, n1)

def asst_content.pick(parts: Maybe<&2, W.ThinkSpan>, +content: String) -> J.Json:
  match parts:
    case None{}:
      content_json(content)
    case Some{+pr}:
      asst_content.found(pr, content)

def asst_content_json(+content: String) -> J.Json:
  asst_content.pick(W.asst_think.find(content), content)

def msg_json.asst(+content: String, +calls: List<&2, W.WCall>,
    ids: List<&2, String>) -> J.Json & List<&2, String>:
  match calls:
    case Nil{}:
      (J.obj(Con{J.kv("role", J.str("assistant")),
        Con{J.kv("content", asst_content_json(content)), Nil{}}}), ids)
    case Con{_, _}:
      (J.obj(Con{J.kv("role", J.str("assistant")),
        Con{J.kv("content", asst_content_json(content)),
          Con{J.kv("tool_calls", calls_json(calls)), Nil{}}}}),
        List.append(&2, String, ids, W.ids_of(calls)))

def msg_json.pick(is_tool: Bool, is_asst: Bool, +role: String,
    +content: String, +calls: List<&2, W.WCall>, ids: List<&2, String>)
    -> J.Json & List<&2, String>:
  match is_tool:
    case True{}:
      msg_json.tool(ids, content)
    case False{}:
      match is_asst:
        case True{}:
          msg_json.asst(content, calls, ids)
        case False{}:
          (J.obj(Con{J.kv("role", J.str(W.role_out(role))),
            Con{J.kv("content", W.img_content_of(role, Im.IOai{}, content)), Nil{}}}), ids)

def msg_json(m: W.WMsg, ids: List<&2, String>) -> J.Json & List<&2, String>:
  match m:
    case W.WMsg{+role, +content, +calls}:
      msg_json.pick(String.eq(role, "tool"), String.eq(role, "assistant"),
        role, content, calls, ids)

# the message walk is one self-recursive def over a Data state (the
# json package's parser pattern): computed values travel inside
# constructors and are matched on the next hop, as parameters
type MsgPair is Data:
  MP{j: J.Json, ids2: List<&2, String>}

type MWalk is Data:
  MNext{todo: List<&2, W.WMsg>, ids: List<&2, String>, acc: List<&2, J.Json>}
  MPair{pair: MsgPair, todo: List<&2, W.WMsg>, acc: List<&2, J.Json>}

def pair_of.wrap(t: J.Json & List<&2, String>) -> MsgPair:
  match t:
    case (j, ids2):
      MP{j, ids2}

def pair_of(m: W.WMsg, ids: List<&2, String>) -> MsgPair:
  pair_of.wrap(msg_json(m, ids))

def msgs_go(fuel: Nat, w: MWalk) -> List<&2, J.Json>:
  match fuel:
    case 0n:
      Nil{}
    case 1n+k:
      match w:
        case MNext{Nil{}, _, +acc}:
          List.reverse(&2, J.Json, acc)
        case MNext{Con{+m, +rest}, +ids, +acc}:
          msgs_go(k, MPair{pair_of(m, ids), rest, acc})
        case MPair{+pair, +todo, +acc}:
          match pair:
            case MP{+j, +ids2}:
              msgs_go(k, MNext{todo, ids2, Con{j, acc}})

# a message with empty content never leaves (W.sendable)
def msgs_json(+msgs: List<&2, W.WMsg>) -> List<&2, J.Json>:
  msgs_go(Nat.add(Nat.mul(List.length(&2, W.WMsg, msgs), 2n), 16n),
    MNext{W.sendable(msgs), Nil{}, Nil{}})

# what the catalog says of the model (core/api.bend MFacts): whether it
# reasons, its output cap (0n: none sent) and the cap's field name
type Opts is Data:
  Opts{reasoning: Bool, output: Nat, max_key: String, effort: String}

# reasoning_effort "high" rides every call of a reasoning model, the
# vibe CLI parity: the active model zai-glm-5-3 runs with thinking
# "high", and the vibe CLI maps thinking medium/high/max to
# reasoning_effort "high" on the Mistral API (core/llm/backend/
# mistral.py _THINKING_TO_REASONING_EFFORT). A model the catalog marks
# as not reasoning gets none (OpenAI gpt-4.1, Groq's llama: a 400).
# BISE-135: the word is the call's effort (the session's choice, the
# config, the model's default, runtime/provider-pure.bend effort_of),
# sent as is; "" sends none.
def body.effort.word(empty: Bool, +effort: String,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match empty:
    case True{}:
      rest
    case False{}:
      Con{J.kv("reasoning_effort", J.str(effort)), rest}

def body.effort(reasoning: Bool, +effort: String,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match reasoning:
    case True{}:
      body.effort.word(String.is_empty(effort), effort, rest)
    case False{}:
      rest

# the output cap: max_tokens, or max_completion_tokens (OpenAI's name,
# the only one its reasoning models take); none when the catalog has none
def body.cap(none: Bool, +output: Nat, +key: String,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match none:
    case True{}:
      rest
    case False{}:
      Con{J.kv(key, J.JNum{output}), rest}

def body.head(o: Opts, +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match o:
    case Opts{+reasoning, +output, +max_key, +effort}:
      body.effort(reasoning, effort,
        body.cap(Nat.is_eq(output, 0n), output, max_key, rest))

def api_body.pick(empty_tools: Bool, +model: String, +o: Opts,
    +tools: List<&2, W.WTool>, +msgs: List<&2, W.WMsg>) -> String:
  match empty_tools:
    case True{}:
      # a compaction call offers no tools at all
      J.stringify(J.obj(Con{J.kv("model", J.str(model)),
        body.head(o,
          Con{J.kv("messages", J.arr(msgs_json(msgs))), Nil{}})}))
    case False{}:
      J.stringify(J.obj(Con{J.kv("model", J.str(model)),
        body.head(o,
          Con{J.kv("messages", J.arr(msgs_json(msgs))),
            Con{J.kv("tools", tools_json(tools)),
              Con{J.kv("tool_choice", J.str("auto")), Nil{}}}})}))

def api_body_with(+model: String, +o: Opts, +req: W.WReq) -> String:
  match req:
    case W.WReq{+tools, +msgs, _}:
      api_body.pick(W.is_empty_tools(tools), model, o, tools, msgs)

# no catalog facts: reasoning_effort on, no output cap (today's body)
def api_body(+model: String, +req: W.WReq) -> String:
  api_body_with(model, Opts{True{}, 0n, "max_tokens", "high"}, req)

def fn_of(tc: J.Json) -> J.Json:
  W.field_or(tc, "function", J.JNull{})

def arg_of.parsed(r: Result<&2, &2, String, J.Json>, +name: String,
    +raw: String) -> String:
  match r:
    case Done{+j}:
      W.flat_in(name, j, raw)
    case Fail{_}:
      raw

# a bash call with a description keeps its JSON args (W.flat_in)
def arg_of(+name: String, +raw: String) -> String:
  arg_of.parsed(J.parse(raw), name, raw)

def tc_line(idx: Nat, tc: J.Json) -> String:
  +fn = fn_of(tc)
  String.append("CALL ",
    String.append(Nat.show(Nat.add(idx, 1n)),
      String.append(" ",
        String.append(J.get_str_or(fn, "name", ""),
          String.append(" : ",
            String.trim(W.wire_encode(arg_of(J.get_str_or(fn, "name", ""),
              J.get_str_or(fn, "arguments", "")))))))))

def tc_lines.go(tcs: List<&2, J.Json>, +i: Nat) -> List<&2, String>:
  match tcs:
    case Nil{}:
      Nil{}
    case Con{+tc, rest}:
      Con{tc_line(i, tc), tc_lines.go(rest, Nat.add(i, 1n))}

def tc_lines(tcs: List<&2, J.Json>) -> List<&2, String>:
  tc_lines.go(tcs, 0n)

# the texts of one thinking block parts, newline-joined
def think_texts(parts: List<&2, J.Json>) -> String:
  match parts:
    case Nil{}:
      ""
    case Con{+p, rest}:
      String.append(J.get_str_or(p, "text", ""),
        String.append("\n", think_texts(rest)))

def block_think.has(has: Bool, +b: J.Json) -> String:
  match has:
    case True{}:
      W.think_marked(think_texts(W.arr_field_or(b, "thinking")),
        J.get_str_or(b, "signature", ""))
    case False{}:
      ""

def block_think(has: Bool, +b: J.Json) -> String:
  block_think.has(has, b)

def block_text.pick(is_text: Bool, has_think: Bool, +b: J.Json) -> String:
  match is_text:
    case True{}:
      J.get_str_or(b, "text", "")
    case False{}:
      block_think(has_think, b)

# one GLM content block: a text block surfaces its text; a thinking
# block carries its parts in a nested thinking array -- they surface
# wrapped in think markers, so the model keeps its own reasoning in the
# history it sees back (GLM native reasoning convention; dropping it
# here is the bug that made every multi-tool turn reason blind)
def block_text(+b: J.Json) -> String:
  block_text.pick(Bool.not(String.is_empty(J.get_str_or(b, "text", ""))),
    Bool.not(List.is_empty(&2, J.Json, W.arr_field_or(b, "thinking"))), b)

def blocks_text(blocks: List<&2, J.Json>) -> String:
  match blocks:
    case Nil{}:
      ""
    case Con{+b, rest}:
      String.append(block_text(b),
        String.append("\n", blocks_text(rest)))

# GLM returns content as an array of blocks; the blocks are joined
def content_of(j: J.Json) -> String:
  match j:
    case J.JStr{+s}:
      s
    case J.JArr{+blocks}:
      String.trim(blocks_text(blocks))
    case _:
      ""

def choice_msg.found(m: Maybe<&2, J.Json>) -> J.Json:
  match m:
    case Some{+c0}:
      W.field_or(c0, "message", J.JNull{})
    case None{}:
      J.JNull{}

def choice_msg(j: J.Json) -> J.Json:
  choice_msg.found(J.arr_get(W.field_or(j, "choices", J.JNull{}), 0n))

def reply_ok(+msg: J.Json) -> String:
  +content = content_of(W.field_or(msg, "content", J.JNull{}))
  +calls = tc_lines(W.arr_field_or(msg, "tool_calls"))
  W.join_lines(Con{String.append("OK ", W.escape_nl(content)),
    List.append(&2, String, calls, Con{"END", Nil{}})})

def reply_of.check(ok: Bool, +status: U32, +body: String, +j: J.Json) -> String:
  match ok:
    case True{}:
      reply_ok(choice_msg(j))
    case False{}:
      W.reply_err(status, body)

def reply_of(+status: U32, +body: String) -> String:
  reply_of.check(U32.is_eq(status, 200), status, body, W.parse_json(body))