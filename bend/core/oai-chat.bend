# core/oai-chat.bend — the OpenAI Chat Completions family, pure Bend.
#
# Converts the parsed wire request (core/wire.bend WReq) into an
# OpenAI-compatible chat-completions JSON body, and a provider response
# back into the OK/CALL/END reply the Core parses (runtime/remote.bend
# parse_reply). This is the port of what the Rust bridge used to do: the
# mapping lives in pure Bend and is pinned by bend/laws/.
#
# Tool results pair with assistant calls FIFO on the ids ("call_<n>"),
# exactly as the bridge did, or the provider's own ids when its extra
# came back (Gemini's thought signatures, gemini-sig: extra_of, stamp,
# own_extras); GLM content arrives as an array of blocks whose text
# fields are joined.

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

# ---- the sibling-field replay of marked reasoning (glm-5-history) ----
# For the providers whose backend 400s the ThinkChunk parts (databricks'
# glm-5-3 'unsupported content item type: "thinking"'; google's and
# baseten's, probed): the visible text as the content, the reasoning as
# a reasoning_content field — the shape deepseek.bend already sends, so
# one extraction (W.think_hist) serves both dialects. A SIGNED span's
# reasoning stays out with its signature (W.own_reason: another
# transport's, never replayed unsigned).

# the visible text as the field family's content: null only when calls
# came (a blank no-calls assistant never reaches the walk:
# field_sendable drops it, deepseek's rule)
def field_content.pick(null_ok: Bool, +visible: String) -> J.Json:
  match null_ok:
    case True{}:
      J.JNull{}
    case False{}:
      J.str(visible)

# reasoning_content only when some reasoning goes, before `rest`
def field_kvs.reason(empty: Bool, +reason: String,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match empty:
    case True{}:
      rest
    case False{}:
      Con{J.kv("reasoning_content", J.str(reason)), rest}

# the content and reasoning fields of a marked assistant message:
# content first (null only with calls), then reasoning_content when
# the history holds reasoning
def field_kvs.of(h: W.ThinkHist, has_calls: Bool,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match h:
    case W.TH{+visible, +reason}:
      Con{J.kv("content",
          field_content.pick(Bool.and(has_calls, W.is_blank(visible)),
            visible)),
        field_kvs.reason(String.is_empty(reason), reason, rest)}

def field_kvs(+content: String, has_calls: Bool,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  field_kvs.of(W.think_hist(content), has_calls, rest)

# the field family's sendable: an assistant counts by its VISIBLE text
# (a thinking-only turn — blank outside its spans — is left out, its
# content would be null without calls; deepseek's rule); every other
# role by today's unsendable
def is_marked_blank.pick(asst: Bool, +text: String) -> Bool:
  match asst:
    case True{}:
      W.is_blank(W.think_visible(text))
    case False{}:
      W.is_blank(text)

def is_marked_blank(+role: String, +text: String) -> Bool:
  is_marked_blank.pick(String.eq(role, "assistant"), text)

def field_unsendable(+role: String, +text: String, has_calls: Bool) -> Bool:
  match has_calls:
    case True{}:
      False{}
    case False{}:
      Bool.and(Bool.not(String.eq(role, "tool")),
        is_marked_blank(role, text))

def field_sendable(msgs: List<&2, W.WMsg>) -> List<&2, W.WMsg>:
  match msgs:
    case Nil{}:
      Nil{}
    case Con{+m, rest}:
      match m:
        case W.WMsg{+role, +content, +calls, +extra}:
          W.keep_msg(field_unsendable(role, content, W.has_calls(calls)),
            W.WMsg{role, content, calls, extra}, field_sendable(rest))

# ---- the provider's extra: Gemini's thought signatures (gemini-sig) ----
# Gemini signs its tool calls (tool_calls[i].extra_content.google.
# thought_signature) and may sign the message (extra_content); the next
# request must carry each back unchanged where it came, or Gemini answers
# 400 "Function call is missing a thought_signature". The reply's are
# kept as one opaque blob, T.Msg's extra, which only this module writes
# (extra_of, stamp) and reads (own_extras, msg_json.asst):
#   {"extra_content": <the message's>, "calls": [<part>, ...],
#    "by": {"provider": "<p>", "model": "<m>"}}
# one part per call, by position: {"id": "<the provider's id>",
# "extra_content": <the call's, when it had one>}. "by" is stamped when
# the reply is read (runtime/remote.bend stamp: the call's provider and
# model). A blob replays only to that same provider and model
# (own_extras: a signature is valid for the model that wrote it, and a
# strict server 400s an unknown key), and only when its parts align with
# the message's calls (aligned: never a signature on another call);
# else the message goes as it would without it: call_<n> ids, no
# extra_content.

def opt_kv(none: Bool, +k: String, +v: J.Json,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match none:
    case True{}:
      rest
    case False{}:
      Con{J.kv(k, v), rest}

def ec_of(+j: J.Json) -> J.Json:
  W.field_or(j, "extra_content", J.JNull{})

# the extra_content of `j`, when it has one, before `rest`
def ec_kv(+j: J.Json, +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  opt_kv(J.is_null(ec_of(j)), "extra_content", ec_of(j), rest)

# the reply side: a call's part, the blob of a reply's message ("" when
# nothing in it is signed: the reply lines stay today's)
def call_part(+tc: J.Json) -> J.Json:
  J.obj(Con{J.kv("id", J.str(J.get_str_or(tc, "id", ""))), ec_kv(tc, Nil{})})

def call_parts(tcs: List<&2, J.Json>) -> List<&2, J.Json>:
  match tcs:
    case Nil{}:
      Nil{}
    case Con{+tc, rest}:
      Con{call_part(tc), call_parts(rest)}

def any_ec(tcs: List<&2, J.Json>) -> Bool:
  match tcs:
    case Nil{}:
      False{}
    case Con{+tc, rest}:
      Bool.or(Bool.not(J.is_null(ec_of(tc))), any_ec(rest))

def extra_of.pick(none: Bool, +msg: J.Json) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      J.stringify(J.obj(ec_kv(msg,
        Con{J.kv("calls", J.arr(call_parts(W.arr_field_or(msg, "tool_calls")))),
          Nil{}})))

def extra_of(+msg: J.Json) -> String:
  extra_of.pick(Bool.and(J.is_null(ec_of(msg)),
    Bool.not(any_ec(W.arr_field_or(msg, "tool_calls")))), msg)

# whose it is: the provider and model the call went to
def by_json(+prov: String, +model: String) -> J.Json:
  J.obj(Con{J.kv("provider", J.str(prov)),
    Con{J.kv("model", J.str(model)), Nil{}}})

def stamp.parsed(r: Result<&2, &2, String, J.Json>, +prov: String,
    +model: String) -> String:
  match r:
    case Done{+j}:
      J.stringify(J.set(j, "by", by_json(prov, model)))
    case Fail{_}:
      ""

def stamp.pick(empty: Bool, +extra: String, +prov: String, +model: String)
    -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      stamp.parsed(J.parse(extra), prov, model)

# the blob stamped with the provider and model that wrote it
def stamp(+extra: String, +prov: String, +model: String) -> String:
  stamp.pick(String.is_empty(extra), extra, prov, model)

def by_of.parsed(r: Result<&2, &2, String, J.Json>) -> String:
  match r:
    case Done{+j}:
      J.stringify(W.field_or(j, "by", J.JNull{}))
    case Fail{_}:
      ""

# the blob, when that provider and model wrote it; else ""
def own_extra(+extra: String, +prov: String, +model: String) -> String:
  Bool.pick(String,
    String.eq(by_of.parsed(J.parse(extra)), J.stringify(by_json(prov, model))),
    extra, "")

def own_msgs(ms: List<&2, W.WMsg>, +prov: String, +model: String)
    -> List<&2, W.WMsg>:
  match ms:
    case Nil{}:
      Nil{}
    case Con{+m, rest}:
      match m:
        case W.WMsg{+role, +content, +calls, +extra}:
          Con{W.WMsg{role, content, calls, own_extra(extra, prov, model)},
            own_msgs(rest, prov, model)}

# the request with only the extras that provider and model wrote
def own_extras(+prov: String, +model: String, +req: W.WReq) -> W.WReq:
  match req:
    case W.WReq{+tools, +msgs, +cur}:
      W.WReq{tools, own_msgs(msgs, prov, model), cur}

# the replay side: the blob when its parts align with the `n` calls
def aligned.pick(ok: Bool, +j: J.Json) -> Maybe<&2, J.Json>:
  match ok:
    case True{}:
      Some{j}
    case False{}:
      None{}

def aligned.parsed(r: Result<&2, &2, String, J.Json>, +n: Nat)
    -> Maybe<&2, J.Json>:
  match r:
    case Done{+j}:
      aligned.pick(Bool.and(J.is_obj(j),
        Nat.is_eq(List.length(&2, J.Json, W.arr_field_or(j, "calls")), n)), j)
    case Fail{_}:
      None{}

def aligned.empty(empty: Bool, +extra: String, +n: Nat) -> Maybe<&2, J.Json>:
  match empty:
    case True{}:
      None{}
    case False{}:
      aligned.parsed(J.parse(extra), n)

def aligned(+extra: String, +n: Nat) -> Maybe<&2, J.Json>:
  aligned.empty(String.is_empty(extra), extra, n)

# a call's id: the provider's, kept in its part; else call_<n>
def part_id.pick(empty: Bool, +pid: String, +id: String) -> String:
  match empty:
    case True{}:
      W.call_id_of(id)
    case False{}:
      pid

def part_id(+p: J.Json, +id: String) -> String:
  part_id.pick(String.is_empty(J.get_str_or(p, "id", "")),
    J.get_str_or(p, "id", ""), id)

def call_json_x(c: W.WCall, +p: J.Json) -> J.Json:
  match c:
    case W.WCall{+id, +name, +args}:
      J.obj(Con{J.kv("id", J.str(part_id(p, id))),
        Con{J.kv("type", J.str("function")),
          Con{J.kv("function",
            J.obj(Con{J.kv("name", J.str(W.wire_name(name))),
              Con{J.kv("arguments", J.str(args_json(name, args))), Nil{}}})),
            ec_kv(p, Nil{})}}})

def calls_x(calls: List<&2, W.WCall>, ps: List<&2, J.Json>)
    -> List<&2, J.Json>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{+c, rest}:
      match ps:
        case Con{+p, prest}:
          Con{call_json_x(c, p), calls_x(rest, prest)}
        case Nil{}:
          Con{call_json(c), calls_x(rest, Nil{})}

def ids_x(calls: List<&2, W.WCall>, ps: List<&2, J.Json>) -> List<&2, String> :
  match calls:
    case Nil{}:
      Nil{}
    case Con{+c, rest}:
      match c:
        case W.WCall{+id, _, _}:
          match ps:
            case Con{+p, prest}:
              Con{part_id(p, id), ids_x(rest, prest)}
            case Nil{}:
              Con{W.call_id_of(id), ids_x(rest, Nil{})}

# the content kvs of an assistant message, by its replay shape: the
# ThinkChunk parts (blocks) or the visible text with a reasoning_content
# sibling (field); `has_calls` only the field shape reads (its content
# goes null only when calls came)
def asst_kvs.pick(blocks: Bool, +content: String, has_calls: Bool)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match blocks:
    case True{}:
      Con{J.kv("content", asst_content_json(content)), Nil{}}
    case False{}:
      field_kvs(content, has_calls, Nil{})

def asst_kvs(blocks: Bool, +content: String, has_calls: Bool)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  asst_kvs.pick(blocks, content, has_calls)

# the kvs of an assistant message: role, its content kvs, then `rest`
# (the tool_calls and the provider's extra, when either came)
def asst_obj_kvs(blocks: Bool, +content: String, has_calls: Bool,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  Con{J.kv("role", J.str("assistant")),
    List.append(&2, Sigma<&2, &2, String, _ => J.Json>,
      asst_kvs(blocks, content, has_calls), rest)}

# an assistant message without its provider's extra (today's bytes)
def msg_json.asst.plain(blocks: Bool, +content: String,
    +calls: List<&2, W.WCall>, ids: List<&2, String>)
    -> J.Json & List<&2, String>:
  match calls:
    case Nil{}:
      (J.obj(asst_obj_kvs(blocks, content, False{}, Nil{})), ids)
    case Con{_, _}:
      (J.obj(asst_obj_kvs(blocks, content, True{},
          Con{J.kv("tool_calls", calls_json(calls)), Nil{}})),
        List.append(&2, String, ids, W.ids_of(calls)))

def msg_json.asst.signed(blocks: Bool, +j: J.Json, +content: String,
    +calls: List<&2, W.WCall>, ids: List<&2, String>)
    -> J.Json & List<&2, String>:
  match calls:
    case Nil{}:
      (J.obj(asst_obj_kvs(blocks, content, False{}, ec_kv(j, Nil{}))),
        ids)
    case Con{_, _}:
      (J.obj(asst_obj_kvs(blocks, content, True{},
          Con{J.kv("tool_calls",
            J.arr(calls_x(calls, W.arr_field_or(j, "calls")))),
            ec_kv(j, Nil{})})),
        List.append(&2, String, ids, ids_x(calls, W.arr_field_or(j, "calls"))))

def msg_json.asst.x(blocks: Bool, m: Maybe<&2, J.Json>, +content: String,
    +calls: List<&2, W.WCall>, ids: List<&2, String>)
    -> J.Json & List<&2, String>:
  match m:
    case Some{+j}:
      msg_json.asst.signed(blocks, j, content, calls, ids)
    case None{}:
      msg_json.asst.plain(blocks, content, calls, ids)

def msg_json.asst(blocks: Bool, +content: String, +calls: List<&2, W.WCall>,
    +extra: String, ids: List<&2, String>) -> J.Json & List<&2, String>:
  msg_json.asst.x(blocks, aligned(extra, List.length(&2, W.WCall, calls)),
    content, calls, ids)

def msg_json.pick(blocks: Bool, is_tool: Bool, is_asst: Bool, +role: String,
    +content: String, +calls: List<&2, W.WCall>, +extra: String,
    ids: List<&2, String>) -> J.Json & List<&2, String>:
  match is_tool:
    case True{}:
      msg_json.tool(ids, content)
    case False{}:
      match is_asst:
        case True{}:
          msg_json.asst(blocks, content, calls, extra, ids)
        case False{}:
          (J.obj(Con{J.kv("role", J.str(W.role_out(role))),
            Con{J.kv("content", W.img_content_of(role, Im.IOai{}, content)), Nil{}}}), ids)

def msg_json(blocks: Bool, m: W.WMsg, ids: List<&2, String>)
    -> J.Json & List<&2, String>:
  match m:
    case W.WMsg{+role, +content, +calls, +extra}:
      msg_json.pick(blocks, String.eq(role, "tool"),
        String.eq(role, "assistant"), role, content, calls, extra, ids)

# the message walk is one self-recursive def over a Data state (the
# json package's parser pattern): computed values travel inside
# constructors and are matched on the next hop, as parameters
type MsgPair is Data:
  MP{j: J.Json, ids2: List<&2, String>}

type MWalk is Data:
  MNext{blocks: Bool, todo: List<&2, W.WMsg>, ids: List<&2, String>,
    acc: List<&2, J.Json>}
  MPair{blocks: Bool, pair: MsgPair, todo: List<&2, W.WMsg>,
    acc: List<&2, J.Json>}

def pair_of.wrap(t: J.Json & List<&2, String>) -> MsgPair:
  match t:
    case (j, ids2):
      MP{j, ids2}

def pair_of(blocks: Bool, m: W.WMsg, ids: List<&2, String>) -> MsgPair:
  pair_of.wrap(msg_json(blocks, m, ids))

def msgs_go(fuel: Nat, w: MWalk) -> List<&2, J.Json>:
  match fuel:
    case 0n:
      Nil{}
    case 1n+k:
      match w:
        case MNext{_, Nil{}, _, +acc}:
          List.reverse(&2, J.Json, acc)
        case MNext{+blocks, Con{+m, +rest}, +ids, +acc}:
          msgs_go(k, MPair{blocks, pair_of(blocks, m, ids), rest, acc})
        case MPair{+blocks, +pair, +todo, +acc}:
          match pair:
            case MP{+j, +ids2}:
              msgs_go(k, MNext{blocks, todo, ids2, Con{j, acc}})

# a message with empty content never leaves (W.sendable); the field
# shape counts an assistant by its visible text (field_sendable)
def msgs_json(+blocks: Bool, +msgs: List<&2, W.WMsg>) -> List<&2, J.Json>:
  msgs_go(Nat.add(Nat.mul(List.length(&2, W.WMsg, msgs), 2n), 16n),
    MNext{blocks,
      Bool.pick(List<&2, W.WMsg>, blocks, W.sendable(msgs),
        field_sendable(msgs)), Nil{}, Nil{}})

# what the catalog says of the model (core/api.bend MFacts): whether it
# reasons, its output cap (0n: none sent) and the cap's field name;
# think: how marked reasoning replays — "blocks" the ThinkChunk parts
# (Mistral's shape, the catalog pins it there), anything else a sibling
# reasoning_content field (glm-5-history: the backends behind a gateway
# 400 the parts, databricks' glm-5-3 'unsupported content item type:
# "thinking"'; google's and baseten's, probed)
type Opts is Data:
  Opts{reasoning: Bool, output: Nat, max_key: String, effort: String,
    think: String}

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
    case Opts{+reasoning, +output, +max_key, +effort, _}:
      body.effort(reasoning, effort,
        body.cap(Nat.is_eq(output, 0n), output, max_key, rest))

# the think word of the Opts: "blocks" the ThinkChunk parts (Mistral's),
# anything else the sibling field (the walk's flag)
def thinks_blocks(+think: String) -> Bool:
  String.eq(think, "blocks")

def opts_blocks(o: Opts) -> Bool:
  match o:
    case Opts{_, _, _, _, +think}:
      thinks_blocks(think)

def api_body.pick(empty_tools: Bool, +model: String, +o: Opts,
    +tools: List<&2, W.WTool>, +msgs: List<&2, W.WMsg>) -> String:
  match empty_tools:
    case True{}:
      # a compaction call offers no tools at all
      J.stringify(J.obj(Con{J.kv("model", J.str(model)),
        body.head(o,
          Con{J.kv("messages",
            J.arr(msgs_json(opts_blocks(o), msgs))), Nil{}})}))
    case False{}:
      J.stringify(J.obj(Con{J.kv("model", J.str(model)),
        body.head(o,
          Con{J.kv("messages",
            J.arr(msgs_json(opts_blocks(o), msgs))),
            Con{J.kv("tools", tools_json(tools)),
              Con{J.kv("tool_choice", J.str("auto")), Nil{}}}})}))

def api_body_with(+model: String, +o: Opts, +req: W.WReq) -> String:
  match req:
    case W.WReq{+tools, +msgs, _}:
      api_body.pick(W.is_empty_tools(tools), model, o, tools, msgs)

# no catalog facts: reasoning_effort on, no output cap, Mistral's
# blocks (the shape of a model the file does not know: today's body)
def api_body(+model: String, +req: W.WReq) -> String:
  api_body_with(model, Opts{True{}, 0n, "max_tokens", "high", "blocks"}, req)

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

# the reply's EXTRA line (runtime/remote.bend parse_reply): the message's
# blob (extra_of), wire-encoded; none when nothing in it is signed
def extra_reply_lines.pick(empty: Bool, +extra: String,
    +rest: List<&2, String>) -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      Con{String.append("EXTRA ", W.wire_encode(extra)), rest}

def extra_reply_lines(+extra: String, +rest: List<&2, String>)
    -> List<&2, String>:
  extra_reply_lines.pick(String.is_empty(extra), extra, rest)

def reply_ok(+msg: J.Json) -> String:
  +content = content_of(W.field_or(msg, "content", J.JNull{}))
  +calls = tc_lines(W.arr_field_or(msg, "tool_calls"))
  W.join_lines(Con{String.append("OK ", W.escape_nl(content)),
    List.append(&2, String, calls,
      extra_reply_lines(extra_of(msg), Con{"END", Nil{}}))})

def reply_of.check(ok: Bool, +status: U32, +body: String, +j: J.Json) -> String:
  match ok:
    case True{}:
      reply_ok(choice_msg(j))
    case False{}:
      W.reply_err(status, body)

def reply_of(+status: U32, +body: String) -> String:
  reply_of.check(U32.is_eq(status, 200), status, body, W.parse_json(body))