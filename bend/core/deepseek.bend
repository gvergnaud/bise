# DeepSeek Chat Completions never accepts Mistral/vibe thinking content
# blocks. Replay unsigned reasoning as a sibling reasoning_content field,
# not visible text; signatures from another API must not cross providers.
# https://api-docs.deepseek.com/guides/thinking_mode (2026-10-20): with
# tools, replay reasoning from ALL previous turns, even non-tool turns.
# Without tools the server ignores it. Keep the wire's exact whitespace.
# The split itself is shared with the OpenAI Chat field shape
# (W.think_hist, glm-5-history): one extraction serves both dialects.
import Base
import ./wire.bend as W
import ./oai-chat.bend as Oai
import ./oai-stream.bend as Os
import ../vendor/json.bend as J

type History is Data:
  H{visible: String, reason: String}

def unsigned(tm: W.ThinkMark) -> String:
  W.own_reason(tm)

def split.of(h: W.ThinkHist) -> History:
  match h:
    case W.TH{+visible, +reason}:
      H{visible, reason}

def split(+content: String) -> History:
  split.of(W.think_hist(content))

# Null is valid only with tool calls. A blank or reasoning-only assistant
# without calls is left out (sendable): its content would be "" (main's
# 400 of 2026-10-30, "text content blocks must be non-empty").
def content.pick(null_ok: Bool, +text: String) -> J.Json:
  match null_ok:
    case True{}:
      J.JNull{}
    case False{}:
      J.str(text)

# Always include reasoning_content, including an empty string for foreign
# or non-reasoning history. Never re-label another provider's signature.
def fields.of(h: History, +has_calls: Bool)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match h:
    case H{+visible, +reason}:
      [J.kv("content", content.pick(
          Bool.and(has_calls, String.is_empty(visible)), visible)),
        J.kv("reasoning_content", J.str(reason))]

def fields(+content: String, +has_calls: Bool)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  fields.of(split(content), has_calls)

def call_fields(calls: List<&2, W.WCall>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{_, _}:
      [J.kv("tool_calls", Oai.calls_json(calls))]

def msg_json.pick(asst: Bool, +m: W.WMsg, ids: List<&2, String>)
    -> J.Json & List<&2, String>:
  match asst:
    case True{}:
      match m:
        case W.WMsg{_, +text, +calls, _}:
          (J.obj(Con{J.kv("role", J.str("assistant")),
            List.append(&2, Sigma<&2, &2, String, _ => J.Json>,
              fields(text, Bool.not(List.is_empty(&2, W.WCall, calls))),
              call_fields(calls))}),
            List.append(&2, String, ids, W.ids_of(calls)))
    case False{}:
      Oai.msg_json(False{}, m, ids)

def pair_of(+m: W.WMsg, ids: List<&2, String>) -> Oai.MsgPair:
  match m:
    case W.WMsg{+role, _, _, _}:
      Oai.pair_of.wrap(msg_json.pick(String.eq(role, "assistant"), m, ids))

def msgs_go(fuel: Nat, w: Oai.MWalk) -> List<&2, J.Json>:
  match fuel:
    case 0n:
      Nil{}
    case 1n+k:
      match w:
        case Oai.MNext{_, Nil{}, _, +acc}:
          List.reverse(&2, J.Json, acc)
        case Oai.MNext{+blocks, Con{+m, +rest}, +ids, +acc}:
          msgs_go(k, Oai.MPair{blocks, pair_of(m, ids), rest, acc})
        case Oai.MPair{+blocks, +pair, +todo, +acc}:
          match pair:
            case Oai.MP{+j, +ids2}:
              msgs_go(k, Oai.MNext{blocks, todo, ids2, Con{j, acc}})

# the text DeepSeek sends of a message: an assistant's visible part (its
# reasoning rides reasoning_content), any other's content
def visible_of(h: History) -> String:
  match h:
    case H{+visible, _}:
      visible

def sent_text(is_asst: Bool, +content: String) -> String:
  match is_asst:
    case True{}:
      visible_of(split(content))
    case False{}:
      content

# a message with empty content never leaves (W.unsendable)
def sendable(msgs: List<&2, W.WMsg>) -> List<&2, W.WMsg>:
  match msgs:
    case Nil{}:
      Nil{}
    case Con{+m, rest}:
      match m:
        case W.WMsg{+role, +content, +calls, +extra}:
          W.keep_msg(W.unsendable(role,
              sent_text(String.eq(role, "assistant"), content),
              W.has_calls(calls)),
            W.WMsg{role, content, calls, extra}, sendable(rest))

def msgs_json(+msgs: List<&2, W.WMsg>) -> List<&2, J.Json>:
  msgs_go(Nat.add(Nat.mul(List.length(&2, W.WMsg, msgs), 2n), 16n),
    Oai.MNext{False{}, sendable(msgs), Nil{}, Nil{}})

# In DeepSeek's Chat API, reasoning_effort none does NOT disable
# thinking. Send its documented thinking toggle and omit that effort.
def head.pick(off: Bool, +o: Oai.Opts,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match off:
    case True{}:
      match o:
        case Oai.Opts{_, +output, +key, _, _}:
          Oai.body.head(Oai.Opts{False{}, output, key, "", ""},
            Con{J.kv("thinking", J.obj([J.kv("type", J.str("disabled"))])), rest})
    case False{}:
      Oai.body.head(o, rest)

def head(+o: Oai.Opts, +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match o:
    case Oai.Opts{+reasoning, _, _, +effort, _}:
      head.pick(Bool.or(Bool.not(reasoning), String.eq(effort, "none")), o, rest)

def tool_fields(tools: List<&2, W.WTool>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match tools:
    case Nil{}:
      Nil{}
    case Con{_, _}:
      [J.kv("tools", Oai.tools_json(tools)), J.kv("tool_choice", J.str("auto"))]

def api_body(+model: String, +o: Oai.Opts, +req: W.WReq) -> String:
  match req:
    case W.WReq{+tools, +msgs, _}:
      J.stringify(J.obj(Con{J.kv("model", J.str(model)),
        head(o, Con{J.kv("messages", J.arr(msgs_json(msgs))), tool_fields(tools)})}))

# The shared Chat reply reader historically ignores reasoning_content.
# Preserve it here, for whole and streamed replies, so tool continuations
# can replay native reasoning instead of losing it after the first call.
def reply_text.pick(empty: Bool, +reason: String, +visible: String) -> String:
  match empty:
    case True{}:
      visible
    case False{}:
      String.concat([W.think_marked(reason, ""), "\n", visible])

def reply_ok(+msg: J.Json) -> String:
  +reason = J.get_str_or(msg, "reasoning_content", "")
  +text = reply_text.pick(String.is_empty(reason), reason,
    Oai.content_of(W.field_or(msg, "content", J.JNull{})))
  W.join_lines(Con{String.append("OK ", W.escape_nl(text)),
    List.append(&2, String, Oai.tc_lines(W.arr_field_or(msg, "tool_calls")),
      Con{"END", Nil{}})})

def reply_of.pick(ok: Bool, +status: U32, +body: String) -> String:
  match ok:
    case True{}:
      reply_ok(Oai.choice_msg(W.parse_json(body)))
    case False{}:
      W.reply_err(status, body)

def reply_of(+status: U32, +body: String) -> String:
  reply_of.pick(U32.is_eq(status, 200), status, body)

def stream_reply.acc(+a: Os.Acc) -> String:
  match a:
    case Os.Acc{_, _, _, _, _, Some{+e}, _}:
      W.reply_err(200, J.stringify(e))
    case Os.Acc{_, _, _, _, _, None{}, _}:
      reply_ok(Os.message(a))

def stream_reply.pick(is_json: Bool, +body: String) -> String:
  match is_json:
    case True{}:
      reply_of(200, body)
    case False{}:
      stream_reply.acc(Os.fold_sse(body))

def stream_reply(+body: String) -> String:
  stream_reply.pick(String.starts_with(String.trim(body), "{"), body)
