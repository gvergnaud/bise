# runtime/live-pure.bend — the answer as it streams in: the pure half of
# the live reader (runtime/sse-read.bend feeds it each raw piece of a
# streamed model attempt; runtime/race.bend carries its lines to the
# caller, who writes them on the wire).
#
# A piece is raw bytes. Only the piece is scanned for newlines (a byte
# that never sits inside a UTF-8 sequence): what follows its last
# newline is held as the tail (a list of pieces, newest first), and a
# line is joined, decoded and parsed once, when its newline comes. So the
# cost stays linear in the reply, however small the pieces.
#
# Each "data:" line is read by the family's own event shape (below) for
# the ANSWER's text only: no thinking, no tool-call JSON. The text since
# the last emit is kept as a list of parts (newest first) and joined
# once per emit. A new text block (another index, item or content part;
# in Chat Completions, text after a thinking block) starts again at
# offset 0, and its unsent text is dropped: the hub replaces its text.
#
# The wire line, one per emit (at most every every_ms()):
#   partial #<call>.<attempt> +<offset> : <escaped text since the last>
# offset counts the ESCAPED characters already sent in this block, so
# the hub, which keeps the escaped text and unescapes it whole, checks a
# delta by its own length. The final reply stays the truth: it is decoded
# once, whole, as before (provider.bend sse.text).
import Base
import ../core/api.bend as Api
import ../core/wire.bend as W
import ../core/text.bend as Tx
import ../vendor/json.bend as J
import ../vendor/http/encoding/utf8.bend as U8

# at most one partial per this many ms (designer m_17504: 400 ms, under
# main m_12376's 250 ms ceiling; each partial costs the REPL a wire.log
# write, so the rate is part of the R40 CPU bar)
def every_ms() -> Nat:
  400n

# one attempt's live state. on: False for a call whose text is not the
# agent's answer (a compaction summary, a one-shot): feed does nothing.
type Lv is Data:
  Lv{on: Bool, id: String, fam: Api.Style, tail: List<&2, String>,
    key: String, pend: List<&2, String>, off: Nat, at: Nat}

def lv0(on: Bool, +id: String, fam: Api.Style) -> Lv:
  Lv{on, id, fam, Nil{}, "", Nil{}, 0n, 0n}

# ---- one event: the answer's text it carries ----

# ETxt: text for the block named key ("" = the block in progress);
# EBrk: a thinking block, after which text starts a new block
type Ex is Data:
  ETxt{key: String, text: String}
  EBrk{}

def one(+key: String, +text: String) -> List<&2, Ex>:
  Con{ETxt{key, text}, Nil{}}

# Messages API: content_block_delta / text_delta, its block by index
def anth.delta(is_text: Bool, +d: J.Json, +key: String) -> List<&2, Ex>:
  match is_text:
    case True{}:
      one(key, J.get_str_or(d, "text", ""))
    case False{}:
      Nil{}

def anth.pick(hit: Bool, +e: J.Json) -> List<&2, Ex>:
  match hit:
    case True{}:
      anth.delta(
        String.eq(J.get_str_or(W.field_or(e, "delta", J.JNull{}), "type", ""),
          "text_delta"),
        W.field_or(e, "delta", J.JNull{}),
        J.stringify(W.field_or(e, "index", J.JNull{})))
    case False{}:
      Nil{}

def anth(+e: J.Json) -> List<&2, Ex>:
  anth.pick(String.eq(J.get_str_or(e, "type", ""), "content_block_delta"), e)

# Chat Completions: choices[0].delta.content, a string or (Mistral's
# reasoning models) blocks of text and thinking, as Os.content_piece
def chat.block(is_think: Bool, +b: J.Json) -> Ex:
  match is_think:
    case True{}:
      EBrk{}
    case False{}:
      ETxt{"", J.get_str_or(b, "text", "")}

def chat.blocks(bs: List<&2, J.Json>) -> List<&2, Ex>:
  match bs:
    case Nil{}:
      Nil{}
    case Con{+b, rest}:
      Con{chat.block(String.eq(J.get_str_or(b, "type", ""), "thinking"), b),
        chat.blocks(rest)}

def chat.content(+c: J.Json) -> List<&2, Ex>:
  match c:
    case J.JStr{+s}:
      one("", s)
    case J.JArr{+bs}:
      chat.blocks(bs)
    case _:
      Nil{}

def chat.choice(m: Maybe<&2, J.Json>) -> List<&2, Ex>:
  match m:
    case Some{+ch}:
      chat.content(W.field_or(W.field_or(ch, "delta", J.JNull{}), "content",
        J.JNull{}))
    case None{}:
      Nil{}

def chat(+e: J.Json) -> List<&2, Ex>:
  chat.choice(J.arr_get(W.field_or(e, "choices", J.JNull{}), 0n))

# Responses API: response.output_text.delta, its block by item and part
def resp.pick(hit: Bool, +e: J.Json) -> List<&2, Ex>:
  match hit:
    case True{}:
      one(String.concat([J.get_str_or(e, "item_id", ""), "/",
          J.stringify(W.field_or(e, "content_index", J.JNull{}))]),
        J.get_str_or(e, "delta", ""))
    case False{}:
      Nil{}

def resp(+e: J.Json) -> List<&2, Ex>:
  resp.pick(String.eq(J.get_str_or(e, "type", ""), "response.output_text.delta"),
    e)

def of(fam: Api.Style, +e: J.Json) -> List<&2, Ex>:
  match fam:
    case Api.Anthropic{}:
      anth(e)
    case Api.OpenAI{}:
      chat(e)
    case Api.DeepSeek{}:
      chat(e)
    case Api.OpenAIResp{}:
      resp(e)

def parsed(r: Result<&2, &2, String, J.Json>, fam: Api.Style) -> List<&2, Ex>:
  match r:
    case Done{+e}:
      of(fam, e)
    case Fail{_}:
      Nil{}

# ---- the fold ----

def key_of(+lv: Lv) -> String:
  match lv:
    case Lv{_, _, _, _, +key, _, _, _}:
      key

def fam_of(+lv: Lv) -> Api.Style:
  match lv:
    case Lv{_, _, +fam, _, _, _, _, _}:
      fam

def on_of(+lv: Lv) -> Bool:
  match lv:
    case Lv{+on, _, _, _, _, _, _, _}:
      on

def apply.txt(same: Bool, +k: String, +t: String, +lv: Lv) -> Lv:
  match same:
    case True{}:
      match lv:
        case Lv{+on, +id, +fam, +tail, +key, +pend, +off, +at}:
          Lv{on, id, fam, tail, key, Con{t, pend}, off, at}
    case False{}:
      match lv:
        case Lv{+on, +id, +fam, +tail, _, _, _, +at}:
          Lv{on, id, fam, tail, k, Con{t, Nil{}}, 0n, at}

def apply.some(empty: Bool, +k: String, +t: String, +lv: Lv) -> Lv:
  match empty:
    case True{}:
      lv
    case False{}:
      apply.txt(Bool.or(String.is_empty(k), String.eq(k, key_of(lv))), k, t,
        lv)

def apply(x: Ex, +lv: Lv) -> Lv:
  match x:
    case ETxt{+k, +t}:
      apply.some(String.is_empty(t), k, t, lv)
    case EBrk{}:
      match lv:
        case Lv{+on, +id, +fam, +tail, +key, _, _, +at}:
          Lv{on, id, fam, tail, String.append(key, "+"), Nil{}, 0n, at}

def applies(xs: List<&2, Ex>, +lv: Lv) -> Lv:
  match xs:
    case Nil{}:
      lv
    case Con{+x, rest}:
      applies(rest, apply(x, lv))

# ---- the fast path: a Messages API text delta without J.parse ----
#
# Nearly every line of an answer is a text delta, and decoding and
# parsing each whole line was most of the live reader's cost (R40). The
# Messages API writes them in one exact shape:
#   data: {"type":"content_block_delta","index":N,"delta":{"type":"text_delta","text":"..."}}
# fast reads that shape only: the index's digits as the block key, then
# the text's string literal, decoded and unescaped alone (J's own string
# reader), then "}}" and nothing else. Any other line, or any doubt (a
# leading zero, a field out of order, a space, a bad escape), is None and
# J.parse decides (slow). For every line fast reads, it reads what slow
# reads (the laws, and bend/tests/live-fast.bend over the fixtures).

def anth.pre() -> String:
  "data: {\"type\":\"content_block_delta\",\"index\":"

def anth.mid() -> String:
  ",\"delta\":{\"type\":\"text_delta\",\"text\":\""

def is_digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

# the leading digits of a string, and what follows them
type Dg is Data:
  Dg{ds: String, rest: String}

type DgSt is Data:
  DgNext{}
  DgChar{dig: Bool, c: U32}

def digits.go(fuel: Nat, st: DgSt, s: String, acc: String) -> Dg:
  match fuel:
    case 0n:
      Dg{String.reverse(acc), s}
    case 1n+p:
      match st:
        case DgNext{}:
          match s:
            case SNil{}:
              Dg{String.reverse(acc), ""}
            case SCon{Chr{+c}, +t}:
              digits.go(p, DgChar{is_digit(c), c}, t, acc)
        case DgChar{dig, +c}:
          match dig:
            case True{}:
              digits.go(p, DgNext{}, s, SCon{Chr{c}, acc})
            case False{}:
              Dg{String.reverse(acc), SCon{Chr{c}, s}}

# at most 10 digits (an index past that is a doubt: slow reads it)
def digits(+s: String) -> Dg:
  digits.go(20n, DgNext{}, s, "")

# the digits J.stringify writes for the same number: one or more, no
# leading zero but "0" itself
def anth.key_ok(+ds: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(ds)),
    Bool.or(String.eq(ds, "0"), Bool.not(String.starts_with(ds, "0"))))

# the text's literal, from just past its opening quote; the rest after
# its closing quote must be "}}" (and the blanks slow trims)
def anth.text(r: J.StrRes, +key: String) -> Maybe<&2, List<&2, Ex>>:
  match r:
    case J.SOk{+t, +rest}:
      Bool.pick(Maybe<&2, List<&2, Ex>>, String.eq(String.trim(rest), "}}"),
        Some{one(key, t)}, None{})
    case J.SErr{_}:
      None{}

def anth.body(+key: String, +body: String) -> Maybe<&2, List<&2, Ex>>:
  +s = U8.utf8.decode(body)
  anth.text(J.parse_str.loop(Nat.add(Nat.mul(String.length(s), 8n), 100n),
    J.StrNormal{}, s, Nil{}), key)

def anth.mid_ok(ok: Bool, +key: String, +rest: String)
    -> Maybe<&2, List<&2, Ex>>:
  match ok:
    case True{}:
      anth.body(key, String.drop(rest, String.length(anth.mid())))
    case False{}:
      None{}

def anth.digits(d: Dg) -> Maybe<&2, List<&2, Ex>>:
  match d:
    case Dg{+ds, +rest}:
      anth.mid_ok(Bool.and(anth.key_ok(ds), String.starts_with(rest, anth.mid())),
        ds, rest)

def anth.fast(hit: Bool, +l: String) -> Maybe<&2, List<&2, Ex>>:
  match hit:
    case True{}:
      anth.digits(digits(String.drop(l, String.length(anth.pre()))))
    case False{}:
      None{}

# a raw data line read without J.parse, when it is the family's exact
# text-delta shape; else None (slow decides)
def fast(fam: Api.Style, +l: String) -> Maybe<&2, List<&2, Ex>>:
  match fam:
    case Api.Anthropic{}:
      anth.fast(String.starts_with(l, anth.pre()), l)
    case _:
      None{}

# a raw data line decoded whole and read by J.parse
def slow(fam: Api.Style, +l: String) -> List<&2, Ex>:
  parsed(J.parse(String.trim(String.drop(U8.utf8.decode(l), 5n))), fam)

def line.fast(m: Maybe<&2, List<&2, Ex>>, +l: String, +lv: Lv) -> Lv:
  match m:
    case Some{+xs}:
      applies(xs, lv)
    case None{}:
      applies(slow(fam_of(lv), l), lv)

# one complete raw line (its newline taken off)
def line(is_data: Bool, +l: String, +lv: Lv) -> Lv:
  match is_data:
    case True{}:
      line.fast(fast(fam_of(lv), l), l, lv)
    case False{}:
      lv

# ---- for the laws: fast agrees with slow ----

def ex.eq(+a: Ex, +b: Ex) -> Bool:
  match a:
    case ETxt{+ka, +ta}:
      match b:
        case ETxt{+kb, +tb}:
          Bool.and(String.eq(ka, kb), String.eq(ta, tb))
        case EBrk{}:
          False{}
    case EBrk{}:
      match b:
        case ETxt{_, _}:
          False{}
        case EBrk{}:
          True{}

def exs.eq(xs: List<&2, Ex>, ys: List<&2, Ex>) -> Bool:
  match xs:
    case Nil{}:
      match ys:
        case Nil{}:
          True{}
        case Con{_, _}:
          False{}
    case Con{+x, xr}:
      match ys:
        case Nil{}:
          False{}
        case Con{+y, yr}:
          Bool.and(ex.eq(x, y), exs.eq(xr, yr))

def agree.m(m: Maybe<&2, List<&2, Ex>>, fam: Api.Style, +l: String) -> Bool:
  match m:
    case Some{+xs}:
      exs.eq(xs, slow(fam, l))
    case None{}:
      True{}

# fast reads l as slow does, or leaves it to slow
def agree(+fam: Api.Style, +l: String) -> Bool:
  agree.m(fast(fam, l), fam, l)

# fast reads l (the laws: the fixtures' text deltas do take the fast path)
def fires.m(m: Maybe<&2, List<&2, Ex>>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def fires(fam: Api.Style, +l: String) -> Bool:
  fires.m(fast(fam, l))

# the complete raw lines of the pieces (oldest first). A line holds
# whole UTF-8 sequences (a newline never sits inside one): fast decodes
# only the text's literal, slow the whole line
def read(ls: List<&2, String>, +lv: Lv) -> Lv:
  match ls:
    case Nil{}:
      lv
    case Con{+l, rest}:
      read(rest, line(String.starts_with(l, "data:"), l, lv))

# what a piece gives: its complete raw lines (oldest first) and the
# state that holds the new tail
type Cut is Data:
  Cut{lines: List<&2, String>, lv: Lv}

def with_tail(+tail: List<&2, String>, +lv: Lv) -> Lv:
  match lv:
    case Lv{+on, +id, +fam, _, +key, +pend, +off, +at}:
      Lv{on, id, fam, tail, key, pend, off, at}

def tail_of(+lv: Lv) -> List<&2, String>:
  match lv:
    case Lv{_, _, _, +tail, _, _, _, _}:
      tail

# the segments after the piece's first newline: whole lines, then the
# new tail (the last segment, "" when the piece ends on a newline)
def cut.rest(segs: List<&2, String>, +acc: List<&2, String>, +lv: Lv) -> Cut:
  match segs:
    case Nil{}:
      Cut{List.reverse(&2, String, acc), lv}
    case Con{+s, rest}:
      match rest:
        case Nil{}:
          Cut{List.reverse(&2, String, acc), with_tail(Con{s, Nil{}}, lv)}
        case Con{+h, +t}:
          cut.rest(Con{h, t}, Con{s, acc}, lv)

# the piece's first segment ends the held tail's line, when the piece
# has a newline; else it joins the tail
def cut.segs(segs: List<&2, String>, +lv: Lv) -> Cut:
  match segs:
    case Nil{}:
      Cut{Nil{}, lv}
    case Con{+first, rest}:
      match rest:
        case Nil{}:
          Cut{Nil{}, with_tail(Con{first, tail_of(lv)}, lv)}
        case Con{+h, +t}:
          cut.rest(Con{h, t},
            Con{String.join(List.reverse(&2, String, Con{first, tail_of(lv)}),
              ""), Nil{}},
            with_tail(Nil{}, lv))

def cut.on(on: Bool, +lv: Lv, +piece: String) -> Cut:
  match on:
    case True{}:
      cut.segs(String.split(piece, Tx.newline()), lv)
    case False{}:
      Cut{Nil{}, lv}

# a raw piece of the body: only the piece is scanned
def cut(+lv: Lv, +piece: String) -> Cut:
  cut.on(on_of(lv), lv, piece)

# ---- the emit ----

def has.parts(+ps: List<&2, String>) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{_, _}:
      True{}

# text waits to be sent
def has(+lv: Lv) -> Bool:
  match lv:
    case Lv{_, _, _, _, _, +pend, _, _}:
      has.parts(pend)

# text waits, and the last partial is every_ms() old (now: IO.now, ms)
def due(+lv: Lv, +now: Nat) -> Bool:
  match lv:
    case Lv{_, _, _, _, _, +pend, _, +at}:
      Bool.and(has.parts(pend), Nat.is_le(Nat.add(at, every_ms()), now))

def line_of(+id: String, +off: Nat, +d: String) -> String:
  String.concat(["partial #", id, " +", Nat.show(off), " : ", d])

type Emit is Data:
  Em{line: String, lv: Lv}

def take.d(+d: String, +lv: Lv) -> Emit:
  match lv:
    case Lv{+on, +id, +fam, +tail, +key, +pend, +off, +at}:
      Em{line_of(id, off, d),
        Lv{on, id, fam, tail, key, pend, Nat.add(off, String.length(d)), at}}

# the wire line of the text since the last one, and the state after it
def take(+lv: Lv, +now: Nat) -> Emit:
  match lv:
    case Lv{+on, +id, +fam, +tail, +key, +pend, +off, _}:
      take.d(W.escape_nl(String.join(List.reverse(&2, String, pend), "")),
        Lv{on, id, fam, tail, key, Nil{}, off, now})

# ---- for the laws: every piece fed, then everything taken ----

def feed.cut(c: Cut) -> Lv:
  match c:
    case Cut{+ls, +lv}:
      read(ls, lv)

# an ASCII piece fed whole (the laws: no decoding needed)
def feed(+lv: Lv, +piece: String) -> Lv:
  feed.cut(cut(lv, piece))

def feeds(ps: List<&2, String>, +lv: Lv) -> Lv:
  match ps:
    case Nil{}:
      lv
    case Con{+p, rest}:
      feeds(rest, feed(lv, p))

def emit_line(e: Emit) -> String:
  match e:
    case Em{+l, _}:
      l

def emit_lv(e: Emit) -> Lv:
  match e:
    case Em{_, +lv}:
      lv

# ---- for the laws: a stream fed in groups of pieces, a take after each
# group when text waits (as the live reader does when due) ----

type Rn is Data:
  Rn{lines: List<&2, String>, lv: Lv}

def runs.em(e: Emit, +ls: List<&2, String>) -> Rn:
  match e:
    case Em{+l, +lv}:
      Rn{Con{l, ls}, lv}

def runs.flush(has: Bool, +lv: Lv, +ls: List<&2, String>) -> Rn:
  match has:
    case True{}:
      runs.em(take(lv, 0n), ls)
    case False{}:
      Rn{ls, lv}

def runs.group(+lv: Lv, +ls: List<&2, String>) -> Rn:
  runs.flush(has(lv), lv, ls)

def runs.go(gs: List<&2, List<&2, String>>, +r: Rn) -> Rn:
  match gs:
    case Nil{}:
      r
    case Con{+g, rest}:
      match r:
        case Rn{+ls, +lv}:
          runs.go(rest, runs.group(feeds(g, lv), ls))

def runs.lines(r: Rn) -> List<&2, String>:
  match r:
    case Rn{+ls, _}:
      List.reverse(&2, String, ls)

# the partial lines sent, oldest first
def runs(gs: List<&2, List<&2, String>>, +lv: Lv) -> List<&2, String>:
  runs.lines(runs.go(gs, Rn{Nil{}, lv}))
