# DeepSeek Chat Completions never accepts Mistral/vibe thinking content
# blocks. Replay unsigned reasoning as a sibling reasoning_content field,
# not visible text; signatures from another API must not cross providers.
# https://api-docs.deepseek.com/guides/thinking_mode (2026-10-20): with
# tools, replay reasoning from ALL previous turns, even non-tool turns.
# Without tools the server ignores it. Keep the wire's exact whitespace.
import Base
import ./wire.bend as W
import ./oai-chat.bend as Oai
import ./oai-stream.bend as Os
import ../vendor/json.bend as J

type History is Data:
  H{visible: String, reason: String}

def unsigned(tm: W.ThinkMark) -> String:
  match tm:
    case W.TM{+reason, +sig}:
      Bool.pick(String, String.is_empty(sig), reason, "")

def split.go(fuel: Nat, span: Maybe<&2, W.ThinkSpan>, +content: String,
    +visible: String, +reason: String) -> History:
  match fuel:
    case 0n:
      H{visible, reason}
    case 1n+k:
      match span:
        case None{}:
          H{String.append(visible, content), reason}
        case Some{W.TSpan{+n0, +n1}}:
          +rest = W.asst_think.visible(W.asst_think.raw_visible(content, n1))
          split.go(k, W.asst_think.find(rest), rest,
            String.append(visible, String.take(content, n0)),
            String.append(reason,
              unsigned(W.think_split(W.asst_think.raw_span(content, n0, n1)))))

def split(+content: String) -> History:
  split.go(Nat.add(String.length(content), 1n), W.asst_think.find(content),
    content, "", "")

# Null is valid only with tool calls. A blank or reasoning-only assistant
# without calls must send a string; keep that turn and its reasoning intact.
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
        case W.WMsg{_, +text, +calls}:
          (J.obj(Con{J.kv("role", J.str("assistant")),
            List.append(&2, Sigma<&2, &2, String, _ => J.Json>,
              fields(text, Bool.not(List.is_empty(&2, W.WCall, calls))),
              call_fields(calls))}),
            List.append(&2, String, ids, W.ids_of(calls)))
    case False{}:
      Oai.msg_json(m, ids)

def pair_of(+m: W.WMsg, ids: List<&2, String>) -> Oai.MsgPair:
  match m:
    case W.WMsg{+role, _, _}:
      Oai.pair_of.wrap(msg_json.pick(String.eq(role, "assistant"), m, ids))

def msgs_go(fuel: Nat, w: Oai.MWalk) -> List<&2, J.Json>:
  match fuel:
    case 0n:
      Nil{}
    case 1n+k:
      match w:
        case Oai.MNext{Nil{}, _, +acc}:
          List.reverse(&2, J.Json, acc)
        case Oai.MNext{Con{+m, +rest}, +ids, +acc}:
          msgs_go(k, Oai.MPair{pair_of(m, ids), rest, acc})
        case Oai.MPair{+pair, +todo, +acc}:
          match pair:
            case Oai.MP{+j, +ids2}:
              msgs_go(k, Oai.MNext{todo, ids2, Con{j, acc}})

def msgs_json(+msgs: List<&2, W.WMsg>) -> List<&2, J.Json>:
  msgs_go(Nat.add(Nat.mul(List.length(&2, W.WMsg, msgs), 2n), 16n),
    Oai.MNext{msgs, Nil{}, Nil{}})

# In DeepSeek's Chat API, reasoning_effort none does NOT disable
# thinking. Send its documented thinking toggle and omit that effort.
def head.pick(off: Bool, +o: Oai.Opts,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match off:
    case True{}:
      match o:
        case Oai.Opts{_, +output, +key, _}:
          Oai.body.head(Oai.Opts{False{}, output, key, ""},
            Con{J.kv("thinking", J.obj([J.kv("type", J.str("disabled"))])), rest})
    case False{}:
      Oai.body.head(o, rest)

def head(+o: Oai.Opts, +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match o:
    case Oai.Opts{+reasoning, _, _, +effort}:
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
    case Os.Acc{_, _, _, _, _, Some{+e}}:
      W.reply_err(200, J.stringify(e))
    case Os.Acc{_, _, _, _, _, None{}}:
      reply_ok(Os.message(a))

def stream_reply.pick(is_json: Bool, +body: String) -> String:
  match is_json:
    case True{}:
      reply_of(200, body)
    case False{}:
      stream_reply.acc(Os.fold_sse(body))

def stream_reply(+body: String) -> String:
  stream_reply.pick(String.starts_with(String.trim(body), "{"), body)
