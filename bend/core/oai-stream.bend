# core/oai-stream.bend — OpenAI Chat Completions, streamed (BISE-144).
#
# Every OpenAI-compatible provider (OpenAI, Mistral, OpenRouter, Groq,
# DeepSeek, xAI, Together, Fireworks, Cerebras, Ollama, LM Studio...)
# streams the same Server-Sent Events: one `data: {chat.completion.chunk}`
# line per event, `data: [DONE]` last. Like core/anth-stream.bend for
# the Messages API, this module is the pure half: the request body with
# "stream": true, and the fold that rebuilds the message a whole reply
# carries (choices[0].message), so Oai.reply_ok maps it to the same
# OK/CALL/END lines as a non-streaming reply.
#
# A chunk (platform.openai.com/docs/api-reference/chat-streaming):
#   choices[0].delta.content            a text piece (string); Mistral's
#                                       reasoning models send an array
#                                       of blocks (text / thinking)
#   choices[0].delta.reasoning_content  a reasoning piece (DeepSeek,
#                                       Qwen, Kimi; `reasoning`: OpenRouter,
#                                       Groq, Ollama)
#   choices[0].delta.tool_calls         pieces of tool calls BY INDEX: the
#                                       first piece of an index has id +
#                                       name, the next ones argument text;
#                                       indexes may interleave; Mistral
#                                       sends a whole call in one piece
#   choices[0].finish_reason            how the message ended
#   usage                               the token usage: a last chunk with
#                                       choices [] when the request asks
#                                       stream_options.include_usage
#                                       (Mistral: in the finish chunk;
#                                       Groq also x_groq.usage)
#   error                               a mid-stream error (OpenRouter):
#                                       the reply is that error
import Base
import ./wire.bend as W
import ./oai-chat.bend as Oai
import ./text.bend as Tx
import ../vendor/json.bend as J

# the request body with "stream": true and the usage asked for (the body
# is always a non-empty JSON object: its fields follow the new ones)
def stream_body(+body: String) -> String:
  String.append("{\"stream\":true,\"stream_options\":{\"include_usage\":true},",
    String.drop(body, 1n))

# ---- the fold state ----

# a content block being rebuilt, its pieces newest first
type Blk is Data:
  BText{parts: List<&2, String>}
  BThink{parts: List<&2, String>, sig: List<&2, String>}

# a tool call being rebuilt: its index, its id, its name and argument
# pieces (newest first); ec: its extra_content (Gemini's signature), the
# first non-null one of its pieces
type Tc is Data:
  Tc{idx: Nat, id: String, name: String, args: List<&2, String>, ec: J.Json}

# blocks and calls newest first; reason: the reasoning pieces; fin: the
# finish_reason; usage: the last usage object; err: the error, if one
# came; ec: the message's extra_content, the last non-null delta's
type Acc is Data:
  Acc{blocks: List<&2, Blk>, reason: List<&2, String>, calls: List<&2, Tc>,
    fin: String, usage: J.Json, err: Maybe<&2, J.Json>, ec: J.Json}

def acc0() -> Acc:
  Acc{Nil{}, Nil{}, Nil{}, "", J.JNull{}, None{}, J.JNull{}}

# a non-null extra_content wins over the null one
def ec_or(+new: J.Json, +cur: J.Json) -> J.Json:
  Bool.pick(J.Json, J.is_null(new), cur, new)

def joined(+parts: List<&2, String>) -> String:
  String.join(List.reverse(&2, String, parts), "")

def str_or_empty(j: J.Json) -> String:
  match j:
    case J.JStr{+s}:
      s
    case _:
      ""

# ---- content pieces: a piece joins the newest block of its kind ----

def add_text(+p: String, +blocks: List<&2, Blk>) -> List<&2, Blk>:
  match blocks:
    case Con{BText{+parts}, +rest}:
      Con{BText{Con{p, parts}}, rest}
    case _:
      Con{BText{Con{p, Nil{}}}, blocks}

def add_think(+p: String, +s: String, +blocks: List<&2, Blk>)
    -> List<&2, Blk>:
  match blocks:
    case Con{BThink{+parts, +sig}, +rest}:
      Con{BThink{Con{p, parts}, Con{s, sig}}, rest}
    case _:
      Con{BThink{Con{p, Nil{}}, Con{s, Nil{}}}, blocks}

def text_piece.pick(empty: Bool, +p: String, +blocks: List<&2, Blk>)
    -> List<&2, Blk>:
  match empty:
    case True{}:
      blocks
    case False{}:
      add_text(p, blocks)

def text_piece(+p: String, +blocks: List<&2, Blk>) -> List<&2, Blk>:
  text_piece.pick(String.is_empty(p), p, blocks)

# the texts of a thinking block's parts, joined
def think_parts(parts: List<&2, J.Json>) -> String:
  match parts:
    case Nil{}:
      ""
    case Con{+p, rest}:
      String.append(J.get_str_or(p, "text", ""), think_parts(rest))

def think_text(+t: J.Json) -> String:
  match t:
    case J.JStr{+s}:
      s
    case J.JArr{+parts}:
      think_parts(parts)
    case _:
      ""

def block_piece.pick(is_think: Bool, +b: J.Json, +blocks: List<&2, Blk>)
    -> List<&2, Blk>:
  match is_think:
    case True{}:
      add_think(think_text(W.field_or(b, "thinking", J.JNull{})),
        J.get_str_or(b, "signature", ""), blocks)
    case False{}:
      text_piece(J.get_str_or(b, "text", ""), blocks)

def block_pieces(bs: List<&2, J.Json>, +blocks: List<&2, Blk>)
    -> List<&2, Blk>:
  match bs:
    case Nil{}:
      blocks
    case Con{+b, rest}:
      block_pieces(rest,
        block_piece.pick(String.eq(J.get_str_or(b, "type", ""), "thinking"),
          b, blocks))

def content_piece(+c: J.Json, +blocks: List<&2, Blk>) -> List<&2, Blk>:
  match c:
    case J.JStr{+s}:
      text_piece(s, blocks)
    case J.JArr{+bs}:
      block_pieces(bs, blocks)
    case _:
      blocks

# ---- reasoning pieces: reasoning_content, else reasoning ----

def reason_piece.pick(empty: Bool, +r: String, +d: J.Json) -> String:
  match empty:
    case True{}:
      str_or_empty(W.field_or(d, "reasoning", J.JNull{}))
    case False{}:
      r

def reason_piece(+d: J.Json) -> String:
  +r = str_or_empty(W.field_or(d, "reasoning_content", J.JNull{}))
  reason_piece.pick(String.is_empty(r), r, d)

def add_reason.pick(empty: Bool, +p: String, +rs: List<&2, String>)
    -> List<&2, String>:
  match empty:
    case True{}:
      rs
    case False{}:
      Con{p, rs}

def add_reason(+p: String, +rs: List<&2, String>) -> List<&2, String>:
  add_reason.pick(String.is_empty(p), p, rs)

# ---- tool call pieces, by index ----

# the arguments piece: a string (the spec); an object (a lax server)
# counts as its JSON text
def args_piece(+a: J.Json) -> String:
  match a:
    case J.JStr{+s}:
      s
    case J.JObj{+kvs}:
      J.stringify(J.JObj{kvs})
    case _:
      ""

def keep_or.pick(empty: Bool, +cur: String, +new: String) -> String:
  match empty:
    case True{}:
      new
    case False{}:
      cur

# the first non-empty value stays (a server repeating the id or the
# name on every piece does not double it)
def keep_or(+cur: String, +new: String) -> String:
  keep_or.pick(String.is_empty(cur), cur, new)

def tc_add(+c: Tc, +id: String, +name: String, +arg: String, +ec: J.Json)
    -> Tc:
  match c:
    case Tc{+idx, +cid, +cname, +args, +cec}:
      Tc{idx, keep_or(cid, id), keep_or(cname, name), Con{arg, args},
        ec_or(cec, ec)}

def tc_idx(c: Tc) -> Nat:
  match c:
    case Tc{+idx, _, _, _, _}:
      idx

def tc_last(cs: List<&2, Tc>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case Con{+c, _}:
      tc_idx(c)

def has_idx(cs: List<&2, Tc>, +idx: Nat) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Con{+c, rest}:
      Bool.or(Nat.is_eq(tc_idx(c), idx), has_idx(rest, idx))

def tc_upd.one(hit: Bool, +c: Tc, +id: String, +name: String, +arg: String,
    +ec: J.Json) -> Tc:
  match hit:
    case True{}:
      tc_add(c, id, name, arg, ec)
    case False{}:
      c

def tc_upd(cs: List<&2, Tc>, +idx: Nat, +id: String, +name: String,
    +arg: String, +ec: J.Json) -> List<&2, Tc>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, rest}:
      Con{tc_upd.one(Nat.is_eq(tc_idx(c), idx), c, id, name, arg, ec),
        tc_upd(rest, idx, id, name, arg, ec)}

def tc_put.pick(hit: Bool, +cs: List<&2, Tc>, +idx: Nat, +id: String,
    +name: String, +arg: String, +ec: J.Json) -> List<&2, Tc>:
  match hit:
    case True{}:
      tc_upd(cs, idx, id, name, arg, ec)
    case False{}:
      Con{Tc{idx, id, name, Con{arg, Nil{}}, ec}, cs}

# the piece joins the call of its index; a new index is a new call
def tc_put(+cs: List<&2, Tc>, +idx: Nat, +id: String, +name: String,
    +arg: String, +ec: J.Json) -> List<&2, Tc>:
  tc_put.pick(has_idx(cs, idx), cs, idx, id, name, arg, ec)

# no index (a lax server): a piece with an id or a name opens a new
# call, one without continues the newest
def tc_index.none(opens: Bool, +cs: List<&2, Tc>) -> Nat:
  match opens:
    case True{}:
      List.length(&2, Tc, cs)
    case False{}:
      tc_last(cs)

def tc_index(m: Maybe<&2, J.Json>, opens: Bool, +cs: List<&2, Tc>) -> Nat:
  match m:
    case Some{J.JNum{+n}}:
      n
    case _:
      tc_index.none(opens, cs)

def tc_piece(+t: J.Json, +cs: List<&2, Tc>) -> List<&2, Tc>:
  +fn = W.field_or(t, "function", J.JNull{})
  +id = J.get_str_or(t, "id", "")
  +name = J.get_str_or(fn, "name", "")
  tc_put(cs,
    tc_index(J.get(t, "index"),
      Bool.not(Bool.and(String.is_empty(id), String.is_empty(name))), cs),
    id, name, args_piece(W.field_or(fn, "arguments", J.JNull{})),
    W.field_or(t, "extra_content", J.JNull{}))

def tc_pieces(ts: List<&2, J.Json>, +cs: List<&2, Tc>) -> List<&2, Tc>:
  match ts:
    case Nil{}:
      cs
    case Con{+t, rest}:
      tc_pieces(rest, tc_piece(t, cs))

# ---- one chunk ----

def usage_obj(+u: J.Json, +prev: J.Json) -> J.Json:
  match u:
    case J.JObj{+kvs}:
      J.JObj{kvs}
    case _:
      prev

# the usage a chunk carries: "usage", else Groq's x_groq.usage
def chunk_usage(+e: J.Json, +prev: J.Json) -> J.Json:
  usage_obj(W.field_or(e, "usage", J.JNull{}),
    usage_obj(W.field_or(W.field_or(e, "x_groq", J.JNull{}), "usage",
      J.JNull{}), prev))

def fin_of(+ch: J.Json, +prev: String) -> String:
  keep_or.pick(String.is_empty(J.get_str_or(ch, "finish_reason", "")),
    J.get_str_or(ch, "finish_reason", ""), prev)

def err_of(m: Maybe<&2, J.Json>, +prev: Maybe<&2, J.Json>)
    -> Maybe<&2, J.Json>:
  match m:
    case Some{J.JObj{+kvs}}:
      Some{J.obj(Con{J.kv("error", J.JObj{kvs}), Nil{}})}
    case _:
      prev

def choice.delta(+d: J.Json, +ch: J.Json, +a: Acc) -> Acc:
  match a:
    case Acc{+blocks, +reason, +calls, +fin, +usage, +err, +ec}:
      Acc{content_piece(W.field_or(d, "content", J.JNull{}), blocks),
        add_reason(reason_piece(d), reason),
        tc_pieces(W.arr_field_or(d, "tool_calls"), calls),
        fin_of(ch, fin), usage, err,
        ec_or(W.field_or(d, "extra_content", J.JNull{}), ec)}

def choice(m: Maybe<&2, J.Json>, +a: Acc) -> Acc:
  match m:
    case Some{+ch}:
      choice.delta(W.field_or(ch, "delta", J.JNull{}), ch, a)
    case None{}:
      a

def step.meta(+e: J.Json, +a: Acc) -> Acc:
  match a:
    case Acc{+blocks, +reason, +calls, +fin, +usage, +err, +ec}:
      Acc{blocks, reason, calls, fin, chunk_usage(e, usage),
        err_of(J.get(e, "error"), err), ec}

def step(+e: J.Json, +a: Acc) -> Acc:
  choice(J.arr_get(W.field_or(e, "choices", J.JNull{}), 0n), step.meta(e, a))

# ---- the SSE body: every "data:" line is one chunk ([DONE] is not JSON) ----

def event.parsed(r: Result<&2, &2, String, J.Json>, +a: Acc) -> Acc:
  match r:
    case Done{+e}:
      step(e, a)
    case Fail{_}:
      a

def event.line(is_data: Bool, +l: String, +a: Acc) -> Acc:
  match is_data:
    case True{}:
      event.parsed(J.parse(String.trim(String.drop(l, 5n))), a)
    case False{}:
      a

def fold(lines: List<&2, String>, +a: Acc) -> Acc:
  match lines:
    case Nil{}:
      a
    case Con{+l, rest}:
      fold(rest, event.line(String.starts_with(l, "data:"), l, a))

def fold_sse(+sse: String) -> Acc:
  fold(String.split(sse, Tx.newline()), acc0())

# ---- the message a whole reply would carry ----

def has_think(bs: List<&2, Blk>) -> Bool:
  match bs:
    case Nil{}:
      False{}
    case Con{BThink{_, _}, _}:
      True{}
    case Con{_, +rest}:
      has_think(rest)

def texts(bs: List<&2, Blk>) -> String:
  match bs:
    case Nil{}:
      ""
    case Con{BText{+parts}, +rest}:
      String.append(texts(rest), joined(parts))
    case Con{_, +rest}:
      texts(rest)

def sig_kv.pick(empty: Bool, +sig: String)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      Con{J.kv("signature", J.str(sig)), Nil{}}

def sig_kv(+sig: String) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  sig_kv.pick(String.is_empty(sig), sig)

# a block as a whole Mistral reply writes it (Oai.content_of reads it)
def block_json(+b: Blk) -> J.Json:
  match b:
    case BText{+parts}:
      J.obj(Con{J.kv("type", J.str("text")),
        Con{J.kv("text", J.str(joined(parts))), Nil{}}})
    case BThink{+parts, +sig}:
      J.obj(Con{J.kv("type", J.str("thinking")),
        Con{J.kv("thinking", J.arr(Con{J.obj(Con{J.kv("type", J.str("text")),
          Con{J.kv("text", J.str(joined(parts))), Nil{}}}), Nil{}})),
        sig_kv(joined(sig))}})

def blocks_json(bs: List<&2, Blk>) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      Nil{}
    case Con{+b, rest}:
      Con{block_json(b), blocks_json(rest)}

# text only: one string (a whole reply's content); with thinking: the
# blocks, oldest first
def content_json.pick(think: Bool, +bs: List<&2, Blk>) -> J.Json:
  match think:
    case True{}:
      J.arr(blocks_json(List.reverse(&2, Blk, bs)))
    case False{}:
      J.str(texts(bs))

def content_json(+bs: List<&2, Blk>) -> J.Json:
  content_json.pick(has_think(bs), bs)

def call_json(+c: Tc) -> J.Json:
  match c:
    case Tc{_, +id, +name, +args, +ec}:
      J.obj(Con{J.kv("id", J.str(id)),
        Con{J.kv("type", J.str("function")),
        Con{J.kv("function", J.obj(Con{J.kv("name", J.str(name)),
          Con{J.kv("arguments", J.str(joined(args))), Nil{}}})),
          Oai.opt_kv(J.is_null(ec), "extra_content", ec, Nil{})}}})

def calls_json(cs: List<&2, Tc>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, rest}:
      Con{call_json(c), calls_json(rest)}

def opt_kv.pick(none: Bool, +k: String, +v: J.Json,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match none:
    case True{}:
      rest
    case False{}:
      Con{J.kv(k, v), rest}

# choices[0].message: role, content, reasoning_content (when some came),
# tool_calls (when some came), in first-piece order, extra_content (when
# one came)
def message.of(+bs: List<&2, Blk>, +reason: String, +cs: List<&2, Tc>,
    +ec: J.Json) -> J.Json:
  J.obj(Con{J.kv("role", J.str("assistant")),
    Con{J.kv("content", content_json(bs)),
      opt_kv.pick(String.is_empty(reason), "reasoning_content", J.str(reason),
        opt_kv.pick(List.is_empty(&2, Tc, cs), "tool_calls",
          J.arr(calls_json(List.reverse(&2, Tc, cs))),
          opt_kv.pick(J.is_null(ec), "extra_content", ec, Nil{})))}})

def message(+a: Acc) -> J.Json:
  match a:
    case Acc{+blocks, +reason, +calls, _, _, _, +ec}:
      message.of(blocks, joined(reason), calls, ec)

# the whole reply body a non-streaming call would have answered
def whole(+a: Acc) -> J.Json:
  match a:
    case Acc{+blocks, +reason, +calls, +fin, +usage, +err, +ec}:
      match err:
        case Some{+e}:
          e
        case None{}:
          J.obj(Con{J.kv("object", J.str("chat.completion")),
            Con{J.kv("choices", J.arr(Con{J.obj(Con{J.kv("index", J.JNum{0n}),
              Con{J.kv("message", message.of(blocks, joined(reason), calls, ec)),
              Con{J.kv("finish_reason", J.str(fin)), Nil{}}}}), Nil{}})),
            Con{J.kv("usage", usage), Nil{}}}})

# ---- the reply ----

def reply.of(err: Maybe<&2, J.Json>, +a: Acc) -> String:
  match err:
    case Some{+e}:
      W.reply_err(200, J.stringify(e))
    case None{}:
      Oai.reply_ok(message(a))

def reply.acc(+a: Acc) -> String:
  match a:
    case Acc{+blocks, +reason, +calls, +fin, +usage, +err, +ec}:
      reply.of(err, Acc{blocks, reason, calls, fin, usage, None{}, ec})

# the whole SSE body -> the OK/CALL/END reply (Oai.reply_ok, as for a
# whole body); an error chunk IS the reply: "ERROR provider 200: ..."
# (a 200 error body: the retry policy retries it)
def reply(+sse: String) -> String:
  reply.acc(fold_sse(sse))

# a 200 is not always a stream: a server that ignores "stream" (or a
# proxy's JSON error) answers one JSON body: the whole-reply mapping
def reply_any.pick(is_json: Bool, +body: String) -> String:
  match is_json:
    case True{}:
      Oai.reply_of(200, body)
    case False{}:
      reply(body)

def reply_any(+body: String) -> String:
  reply_any.pick(String.starts_with(String.trim(body), "{"), body)

# ---- the token usage (runtime/usage-pure.bend reads it) ----
# the last usage object of the stream (null: none came)
def usage.of(+a: Acc) -> J.Json:
  match a:
    case Acc{_, _, _, _, +u, _, _}:
      u

def usage(+sse: String) -> J.Json:
  usage.of(fold_sse(sse))
