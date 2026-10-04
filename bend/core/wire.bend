# core/wire.bend — the harness wire, shared by every provider family.
#
# The Core never calls a provider: runtime/remote.bend prints a line
# request (MODEL/TOOL/MSG/CALL/END) and parses back an OK/CALL/END reply.
# This module is the part of that detour every family shares: the
# single-line escapes, the request parser (parse_wire -> WReq), the call
# ids, the tool schemas, the think-span markers of the history, and the
# JSON / reply helpers. One module per family maps a WReq to its JSON
# body and its reply back (core/oai-chat.bend, core/anthropic.bend);
# core/api.bend dispatches on the family. All pure, pinned by LAWS.bend.

import Base
import ./compaction.bend as C
import ./text.bend as Tx
import ./image.bend as Im
import ../vendor/json.bend as J
import ./edit.bend as Ed

# ---- single-line flattening (every reply line is one line) ----

# ---- newline escaping: the wire is single-line, markdown is multi-line ----
# Real newlines in a provider text travel as the two chars backslash-n;
# the TUI unescapes for rendering, and parse_wire unescapes message
# contents so the API JSON carries real newlines again. One
# self-recursive def per direction (state in Data, fuel first).

type ENl is Data:
  EGo{s: String}
  EStep{is_nl: Bool, c: U32, t: String}

def escape_nl.go(fuel: Nat, st: ENl, acc: String) -> String:
  match fuel:
    case 0n:
      String.reverse(acc)
    case 1n+k:
      match st:
        case EGo{SNil{}}:
          String.reverse(acc)
        case EGo{SCon{Chr{+c}, t}}:
          escape_nl.go(k, EStep{U32.is_eq(c, 10), c, t}, acc)
        case EStep{True{}, _, t}:
          escape_nl.go(k, EGo{t}, SCon{Chr{110}, SCon{Chr{92}, acc}})
        case EStep{False{}, +c, t}:
          escape_nl.go(k, EGo{t}, SCon{Chr{c}, acc})

def escape_nl(+s: String) -> String:
  escape_nl.go(Nat.add(Nat.mul(String.length(s), 2n), 16n), EGo{s}, SNil{})

type UNl is Data:
  UGo{s: String}
  UStep{is_bs: Bool, c: U32, t: String}
  UNl{t: String}
  UPair{is_n: Bool, c2: U32, t2: String}

def unescape_nl.go(fuel: Nat, st: UNl, acc: String) -> String:
  match fuel:
    case 0n:
      String.reverse(acc)
    case 1n+k:
      match st:
        case UGo{SNil{}}:
          String.reverse(acc)
        case UGo{SCon{Chr{+c}, t}}:
          unescape_nl.go(k, UStep{U32.is_eq(c, 92), c, t}, acc)
        case UStep{True{}, _, t}:
          unescape_nl.go(k, UNl{t}, acc)
        case UStep{False{}, +c, t}:
          unescape_nl.go(k, UGo{t}, SCon{Chr{c}, acc})
        case UNl{SCon{Chr{+c2}, t2}}:
          unescape_nl.go(k, UPair{U32.is_eq(c2, 110), c2, t2}, acc)
        case UNl{SNil{}}:
          unescape_nl.go(k, UGo{SNil{}}, SCon{Chr{92}, acc})
        case UPair{True{}, _, t2}:
          unescape_nl.go(k, UGo{t2}, SCon{Chr{10}, acc})
        case UPair{False{}, +c2, t2}:
          unescape_nl.go(k, UGo{t2}, SCon{Chr{c2}, SCon{Chr{92}, acc}})

def unescape_nl(+s: String) -> String:
  unescape_nl.go(Nat.add(Nat.mul(String.length(s), 2n), 16n), UGo{s}, SNil{})

# ---- wire arg encoding: reversible, unlike escape_nl ----
# escape_nl is for markdown text; a literal backslash-n in that text is
# indistinguishable from an encoded newline after a round-trip. Tool
# args are commands and JSON text full of backslashes, so the arg
# encoding must decode to exactly what was encoded: LF becomes
# backslash capital N, CR backslash capital R, backslash doubled.
# A literal sentinel sequence encodes with a doubled backslash and
# decodes back. The capital letters keep legacy wire content (which
# never carries them) intact through the decode.

type WEnc is Data:
  WEGo{s: String}
  WEPickLf{is_lf: Bool, c: U32, t: String}
  WEPickBs{is_bs: Bool, c: U32, t: String}
  WEPickCr{is_cr: Bool, c: U32, t: String}
  WEBS{t: String}
  WELF{t: String}
  WECR{t: String}
  WEOther{c: U32, t: String}

def wire_encode.go(fuel: Nat, st: WEnc, acc: String) -> String:
  match fuel:
    case 0n:
      String.reverse(acc)
    case 1n+k:
      match st:
        case WEGo{SNil{}}:
          String.reverse(acc)
        case WEGo{SCon{Chr{+c}, t}}:
          wire_encode.go(k, WEPickLf{U32.is_eq(c, 10), c, t}, acc)
        case WEPickLf{True{}, _, t}:
          wire_encode.go(k, WELF{t}, acc)
        case WEPickLf{False{}, +c, t}:
          wire_encode.go(k, WEPickBs{U32.is_eq(c, 92), c, t}, acc)
        case WEPickBs{True{}, _, t}:
          wire_encode.go(k, WEBS{t}, acc)
        case WEPickBs{False{}, +c, t}:
          wire_encode.go(k, WEPickCr{U32.is_eq(c, 13), c, t}, acc)
        case WEPickCr{True{}, _, t}:
          wire_encode.go(k, WECR{t}, acc)
        case WEPickCr{False{}, +c, t}:
          wire_encode.go(k, WEOther{c, t}, acc)
        case WEBS{t}:
          wire_encode.go(k, WEGo{t}, SCon{Chr{92}, SCon{Chr{92}, acc}})
        case WELF{t}:
          wire_encode.go(k, WEGo{t}, SCon{Chr{78}, SCon{Chr{92}, acc}})
        case WECR{t}:
          wire_encode.go(k, WEGo{t}, SCon{Chr{82}, SCon{Chr{92}, acc}})
        case WEOther{+c, t}:
          wire_encode.go(k, WEGo{t}, SCon{Chr{c}, acc})

def wire_encode(+s: String) -> String:
  wire_encode.go(Nat.add(Nat.mul(String.length(s), 8n), 64n), WEGo{s}, SNil{})

type WDec is Data:
  WDGo{s: String}
  WDPick{is_bs: Bool, c: U32, t: String}
  WDBS{s: String}
  WDBSPair{is_bs2: Bool, c2: U32, t2: String}
  WDBSN{is_n: Bool, c2: U32, t2: String}
  WDBSR{is_r: Bool, c2: U32, t2: String}
  WDEmit{c: U32, t: String}
  WDLeg{c2: U32, t2: String}

def wire_decode.go(fuel: Nat, st: WDec, acc: String) -> String:
  match fuel:
    case 0n:
      String.reverse(acc)
    case 1n+k:
      match st:
        case WDGo{SNil{}}:
          String.reverse(acc)
        case WDGo{SCon{Chr{+c}, t}}:
          wire_decode.go(k, WDPick{U32.is_eq(c, 92), c, t}, acc)
        case WDPick{True{}, _, t}:
          wire_decode.go(k, WDBS{t}, acc)
        case WDPick{False{}, +c, t}:
          wire_decode.go(k, WDEmit{c, t}, acc)
        case WDBS{SNil{}}:
          String.reverse(SCon{Chr{92}, acc})
        case WDBS{SCon{Chr{+c2}, t2}}:
          wire_decode.go(k, WDBSPair{U32.is_eq(c2, 92), c2, t2}, acc)
        case WDBSPair{True{}, _, t2}:
          wire_decode.go(k, WDEmit{92, t2}, acc)
        case WDBSPair{False{}, +c2, t2}:
          wire_decode.go(k, WDBSN{U32.is_eq(c2, 78), c2, t2}, acc)
        case WDBSN{True{}, _, t2}:
          wire_decode.go(k, WDEmit{10, t2}, acc)
        case WDBSN{False{}, +c2, t2}:
          wire_decode.go(k, WDBSR{U32.is_eq(c2, 82), c2, t2}, acc)
        case WDBSR{True{}, _, t2}:
          wire_decode.go(k, WDEmit{13, t2}, acc)
        case WDBSR{False{}, +c2, t2}:
          wire_decode.go(k, WDLeg{c2, t2}, acc)
        case WDLeg{+c2, t2}:
          wire_decode.go(k, WDGo{t2}, SCon{Chr{c2}, SCon{Chr{92}, acc}})
        case WDEmit{+c, t}:
          wire_decode.go(k, WDGo{t}, SCon{Chr{c}, acc})

def wire_decode(+s: String) -> String:
  wire_decode.go(Nat.add(Nat.mul(String.length(s), 8n), 64n), WDGo{s}, SNil{})

# ---- the parsed wire request ----

type WCall is Data:
  WCall{id: String, name: String, args: String}

type WMsg is Data:
  WMsg{role: String, content: String, calls: List<&2, WCall>}

type WTool is Data:
  WTool{name: String, desc: String}

# cur is the message whose CALL sub-lines are still arriving
type WReq is Data:
  WReq{tools: List<&2, WTool>, msgs: List<&2, WMsg>, cur: Maybe<&2, WMsg>}

# ---- wire request parsing (the mirror of remote.bend's serialization) ----

# the role a family sends: the ephemeral context (`MSG context`, runtime/
# remote.bend with_context) is a user message on every API; a family
# that caches by breakpoints (Anthropic) knows it by its wire role
def role_out.pick(is_ctx: Bool, +role: String) -> String:
  match is_ctx:
    case True{}:
      "user"
    case False{}:
      role

def role_out(+role: String) -> String:
  role_out.pick(String.eq(role, "context"), role)

def parse_tool.cut(sp: Maybe<&2, Nat>, +rest: String) -> WTool:
  match sp:
    case Some{+p}:
      WTool{String.take(rest, p), String.drop(rest, Nat.add(p, 3n))}
    case None{}:
      WTool{rest, ""}

def parse_tool(+rest: String) -> WTool:
  parse_tool.cut(Tx.find(rest, " : ", 0n), rest)

def parse_call.name(sp: Maybe<&2, Nat>, +id: String, +tail: String) -> WCall:
  match sp:
    case Some{+q}:
      WCall{id, String.take(tail, q), wire_decode(String.drop(tail, Nat.add(q, 3n)))}
    case None{}:
      WCall{id, tail, ""}

def parse_call.id(sp: Maybe<&2, Nat>, +line: String) -> WCall:
  match sp:
    case Some{+p}:
      parse_call.name(Tx.find(String.drop(line, Nat.add(p, 1n)), " : ", 0n),
        String.take(line, p), String.drop(line, Nat.add(p, 1n)))
    case None{}:
      WCall{"0", line, ""}

def parse_call.body(+line: String) -> WCall:
  parse_call.id(Tx.find(line, " ", 0n), line)

def parse_call(+line: String) -> WCall:
  parse_call.body(String.drop(line, 7n))

def add_call(cur: Maybe<&2, WMsg>, +line: String) -> Maybe<&2, WMsg>:
  match cur:
    case Some{+m}:
      match m:
        case WMsg{+role, +content, +calls}:
          # appended, not prepended: the calls keep the order of the
          # CALL lines, which is the order the tool results follow
          # (the results pair with the calls by position)
          Some{WMsg{role, content,
            List.append(&2, WCall, calls, Con{parse_call(line), Nil{}})}}
    case None{}:
      cur

def push_msg(cur: Maybe<&2, WMsg>, +msgs: List<&2, WMsg>) -> List<&2, WMsg>:
  match cur:
    case Some{+m}:
      Con{m, msgs}
    case None{}:
      msgs

def start_msg.cut(sp: Maybe<&2, Nat>, req: WReq, cur: Maybe<&2, WMsg>,
    +rest: String) -> WReq:
  match sp:
    case Some{+p}:
      match req:
        case WReq{+tools, +msgs, _}:
          WReq{tools, push_msg(cur, msgs),
            Some{WMsg{String.take(rest, p),
              unescape_nl(String.drop(rest, Nat.add(p, 3n))),
              Nil{}}}}
    case None{}:
      req

def start_msg(req: WReq, cur: Maybe<&2, WMsg>, +rest: String) -> WReq:
  start_msg.cut(Tx.find(rest, " : ", 0n), req, cur, rest)

def cur_of(req: WReq) -> Maybe<&2, WMsg>:
  match req:
    case WReq{_, _, +cur}:
      cur

def add_tool(req: WReq, +line: String) -> WReq:
  match req:
    case WReq{+tools, +msgs, +cur}:
      WReq{Con{parse_tool(String.drop(line, 5n)), tools}, msgs, cur}

def step_tool(is_tool: Bool, +req: WReq, +line: String) -> WReq:
  match is_tool:
    case True{}:
      add_tool(req, line)
    case False{}:
      req
def step_msg(is_msg: Bool, +req: WReq, +line: String) -> WReq:
  match is_msg:
    case True{}:
      start_msg(req, cur_of(req), String.drop(line, 4n))
    case False{}:
      step_tool(String.starts_with(line, "TOOL "), req, line)
def step_call(is_call: Bool, +req: WReq, +line: String) -> WReq:
  match is_call:
    case True{}:
      match req:
        case WReq{+tools, +msgs, +cur}:
          WReq{tools, msgs, add_call(cur, line)}
    case False{}:
      step_msg(String.starts_with(line, "MSG "), req, line)
def step(+req: WReq, +line: String) -> WReq:
  step_call(String.starts_with(line, "  CALL "), req, line)


def parse_finish(req: WReq) -> WReq:
  match req:
    case WReq{+tools, +msgs, +cur}:
      WReq{tools, List.reverse(&2, WMsg, push_msg(cur, msgs)), None{}}

def parse_go(lines: List<&2, String>, +req: WReq) -> WReq:
  match lines:
    case Nil{}:
      parse_finish(req)
    case Con{+line, rest}:
      parse_go(rest, step(req, line))

# the tools a request offers (BISE-293: the reply's names map back
# through them)
def req_tools(+r: WReq) -> List<&2, WTool>:
  match r:
    case WReq{+tools, _, _}:
      tools

def parse_wire(+request: String) -> WReq:
  parse_go(String.split(request, Tx.newline()), WReq{Nil{}, Nil{}, None{}})

# ---- shared by the families: the wire call id, the tool schemas ----

def call_id_of(+id: String) -> String:
  String.append("call_", id)

def arg_schema() -> J.Json:
  J.obj(Con{J.kv("type", J.str("object")),
    Con{J.kv("properties",
      J.obj(Con{J.kv("arg", J.obj(Con{J.kv("type", J.str("string")), Nil{}})),
        Nil{}})),
      Con{J.kv("required", J.arr(Con{J.str("arg"), Nil{}})), Nil{}}}})

# the optional "description" of bash and run_typescript (BISE-223):
# optional in the schema (old models keep working), asked for in the
# tool texts; the TUI shows it in place of the code
def intent_prop() -> Sigma<&2, &2, String, _ => J.Json>:
  J.kv("description",
    J.obj(Con{J.kv("type", J.str("string")),
      Con{J.kv("description", J.str("One short line shown to the user while this runs: what you are doing, in the user's language. 8 words at most, starting lowercase, no final period. E.g. 'weighing the hero image', 'je lance les tests du signup'.")),
        Nil{}}}))

def bash_schema() -> J.Json:
  J.obj(Con{J.kv("type", J.str("object")),
    Con{J.kv("properties",
      J.obj(Con{J.kv("arg", J.obj(Con{J.kv("type", J.str("string")), Nil{}})),
        Con{intent_prop(), Nil{}}})),
      Con{J.kv("required", J.arr(Con{J.str("arg"), Nil{}})), Nil{}}}})

# ---- per-tool parameter schemas (exact vibe_sdk schemas) ----
# run_typescript: {"code": string} (deny_unknown_fields)
# search_tool_functions: the SearchRequest schema (mode/query/
# functions/functionNames/connectors)

def code_schema() -> J.Json:
  J.obj(
    Con{J.kv("type",
        J.str("object")),
      Con{J.kv("additionalProperties",
          J.JBool{False{}}),
        Con{J.kv("properties",
            J.obj(
              Con{J.kv("code",
                  J.obj(
                    Con{J.kv("type",
                        J.str("string")),
                      Nil{}})),
                Con{intent_prop(), Nil{}}})),
          Con{J.kv("required",
              J.arr(
                Con{
                  J.str("code"),
                  Nil{}})),
            Nil{}}}}})

def search_schema() -> J.Json:
  J.obj(
    Con{J.kv("type",
        J.str("object")),
      Con{J.kv("additionalProperties",
          J.JBool{False{}}),
        Con{J.kv("properties",
            J.obj(
              Con{J.kv("mode",
                  J.obj(
                    Con{J.kv("type",
                        J.str("string")),
                      Con{J.kv("enum",
                          J.arr(
                            Con{
                              J.str("best_match"),
                              Con{
                                J.str("details"),
                                Con{
                                  J.str("all_connector_capabilities"),
                                  Nil{}}}})),
                        Con{J.kv("description",
                            J.str("best_match returns ranked names; details returns declarations for exact functions; all_connector_capabilities lists exact connector inventories.")),
                          Con{J.kv("default",
                              J.str("best_match")),
                            Nil{}}}}})),
                Con{J.kv("query",
                    J.obj(
                      Con{J.kv("type",
                          J.str("string")),
                        Con{J.kv("description",
                            J.str("Rewrite the needed operation into short tool keywords, e.g. \"list issues\"; do not pass the full user request.")),
                          Nil{}}})),
                  Con{J.kv("functions",
                      J.obj(
                        Con{J.kv("type",
                            J.str("array")),
                          Con{J.kv("items",
                              J.obj(
                                Con{J.kv("type",
                                    J.str("string")),
                                  Con{J.kv("minLength",
                                      J.JNum{1n}),
                                    Nil{}}})),
                            Con{J.kv("description",
                                J.str("Exact {connector}.{function} names for details; required in details mode.")),
                              Con{J.kv("maxItems",
                                  J.JNum{10n}),
                                Con{J.kv("default",
                                    J.arr(
                                      Nil{})),
                                  Nil{}}}}}})),
                    Con{J.kv("functionNames",
                        J.obj(
                          Con{J.kv("type",
                              J.str("array")),
                            Con{J.kv("items",
                                J.obj(
                                  Con{J.kv("type",
                                      J.str("string")),
                                    Con{J.kv("minLength",
                                        J.JNum{1n}),
                                      Nil{}}})),
                              Con{J.kv("description",
                                  J.str("Deprecated alias for functions.")),
                                Con{J.kv("maxItems",
                                    J.JNum{10n}),
                                  Nil{}}}}})),
                      Con{J.kv("connectors",
                          J.obj(
                            Con{J.kv("type",
                                J.str("array")),
                              Con{J.kv("items",
                                  J.obj(
                                    Con{J.kv("type",
                                        J.str("string")),
                                      Con{J.kv("minLength",
                                          J.JNum{1n}),
                                        Nil{}}})),
                                Con{J.kv("description",
                                    J.str("Exact connector names for best_match/all_connector_capabilities; unused in details.")),
                                  Con{J.kv("maxItems",
                                      J.JNum{10n}),
                                    Con{J.kv("default",
                                        J.arr(
                                          Nil{})),
                                      Nil{}}}}}})),
                        Nil{}}}}}})),
          Nil{}}}})

# Vibe's edit and write_file (core/edit.bend); every other flat tool
# (apply_patch) takes {"arg": string}
def tool_schema.write(is_write: Bool) -> J.Json:
  match is_write:
    case True{}:
      Ed.write_schema()
    case False{}:
      arg_schema()

def tool_schema.edit(is_edit: Bool, +name: String) -> J.Json:
  match is_edit:
    case True{}:
      Ed.edit_schema()
    case False{}:
      tool_schema.write(String.eq(name, "write_file"))

def tool_schema.bash(is_bash: Bool, +name: String) -> J.Json:
  match is_bash:
    case True{}:
      bash_schema()
    case False{}:
      tool_schema.edit(String.eq(name, "edit"), name)

def tool_schema.rt(is_rt: Bool, +name: String) -> J.Json:
  match is_rt:
    case True{}:
      code_schema()
    case False{}:
      tool_schema.bash(String.eq(name, "bash"), name)

def tool_schema.search(is_search: Bool, +name: String) -> J.Json:
  match is_search:
    case True{}:
      search_schema()
    case False{}:
      tool_schema.rt(String.eq(name, "run_typescript"), name)

def tool_schema(+name: String) -> J.Json:
  tool_schema.search(String.eq(name, "search_tool_functions"), name)

def ids_of(calls: List<&2, WCall>) -> List<&2, String>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{+c, rest}:
      match c:
        case WCall{+id, _, _}:
          Con{call_id_of(id), ids_of(rest)}

# a user or tool content with image markers (core/image.bend) becomes
# content parts; any other content is the plain string, as before
def img_content.pick(has: Bool, +st: Im.IStyle, +content: String) -> J.Json:
  match has:
    case True{}:
      J.arr(Im.content_blocks(st, content))
    case False{}:
      J.str(content)

def img_content(+st: Im.IStyle, +content: String) -> J.Json:
  img_content.pick(Im.has_image(content), st, content)

# where a marker is an image (docs/images.md): the user's own input
# (user, context) and a run_typescript result (its image blocks). Never in
# another tool's result (bash output, a file read, sb inspect: a marker
# there is text the agent printed, 2026-10-02), the system or the assistant.
def img_scope(+role: String, +content: String) -> Bool:
  Bool.or(String.eq(role, "user"), Bool.or(String.eq(role, "context"),
    Bool.and(String.eq(role, "tool"),
      String.starts_with(content, "tool run_typescript "))))

def img_content_of(+role: String, +st: Im.IStyle, +content: String) -> J.Json:
  img_content.pick(Bool.and(img_scope(role, content), Im.has_image(content)),
    st, content)

# ---- the way back: marked history reasoning becomes a thinking block ----
# The harness history stores the assistant reasoning as MARKED TEXT.
# The signed part of the provider block rides INSIDE the span, on its
# last line, prefixed "BENDSIG::" - so the signature survives the wire,
# the history, the checkpoint and the reload, and the request replays
# the thinking block verbatim WITH its signature (the Messages API
# rejects an unsigned thinking block: probed
# "thinking.signature: Field required"). A span WITHOUT a signature
# line (GLM, or an older session) is still STRIPPED on the way back -
# the probed fallback; the model re-thinks only when the reasoning is
# genuinely not replayable. The span readers below are shared; each
# family builds its own block from them (oai-chat.bend asst_content_json,
# anthropic.bend asst_blocks_signed).
#
# The vibe CLI sends the assistant's own reasoning back as a structured
# thinking block ahead of the text block (mistral.py prepare_message ->
# ThinkChunk). The think markers are the transport INSIDE the harness
# wire/history; they are NEVER sent literally to the API. An assistant
# message without markers is sent as a plain string, exactly as before.

type ThinkSpan is Data:
  TSpan{n0: Nat, n1: Nat}

# the search ran on the tail past the start marker (C.find takes a
# position accumulator, not an offset), so the hit is RELATIVE: rebase
# it to the absolute end-marker position
def asst_think.end(m: Maybe<&2, Nat>, +content: String, +n0: Nat)
    -> Maybe<&2, ThinkSpan>:
  match m:
    case Some{+r}:
      Some{TSpan{n0, Nat.add(Nat.add(n0, 7n), r)}}
    case None{}:
      None{}

def asst_think.start(m: Maybe<&2, Nat>, +content: String)
    -> Maybe<&2, ThinkSpan>:
  match m:
    case Some{+n0}:
      asst_think.end(Tx.find(String.drop(content, Nat.add(n0, 7n)), "</think>", 0n), content, n0)
    case None{}:
      None{}

# the marker span: (start, end) of "<think>R</think>" in the text
def asst_think.find(+content: String) -> Maybe<&2, ThinkSpan>:
  asst_think.start(Tx.find(content, "<think>", 0n), content)

# the span text verbatim, no trim: a replayed thinking must be exactly
# what the provider signed
def asst_think.raw_span(+content: String, +n0: Nat, +n1: Nat) -> String:
  String.take(String.drop(content, Nat.add(n0, 7n)),
    Nat.sub(n1, Nat.add(n0, 7n)))

def asst_think.reasoning(+content: String, +n0: Nat, +n1: Nat) -> String:
  String.trim(asst_think.raw_span(content, n0, n1))

# ---- the signed part of one think span ----
# The reply parsers (block_think, anth_block.think) append the
# signature as the span's LAST line: "BENDSIG::<sig>". The split below
# peels it back off. Only the LAST line counts, so reasoning text that
# mentions the prefix stays reasoning.

def lines_last(xs: List<&2, String>) -> Maybe<&2, String>:
  match xs:
    case Nil{}:
      None{}
    case Con{+l, Nil{}}:
      Some{l}
    case Con{_, rest}:
      lines_last(rest)

def lines_init(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{_, Nil{}}:
      Nil{}
    case Con{+l, rest}:
      Con{l, lines_init(rest)}

def think_sig.line(is_sig: Bool, +line: String) -> Maybe<&2, String>:
  match is_sig:
    case True{}:
      Some{String.drop(line, 9n)}
    case False{}:
      None{}

def think_sig(+line: String) -> Maybe<&2, String>:
  think_sig.line(String.starts_with(line, "BENDSIG::"), line)

# (reasoning verbatim, signature) of one think span
type ThinkMark is Data:
  TM{reason: String, sig: String}

def nl_str() -> String:
  String.from_list(Con{Tx.newline(), Nil{}})

def think_split.plain(+span: String) -> ThinkMark:
  TM{span, ""}

def think_split.signed(+span: String, +sig: String) -> ThinkMark:
  TM{String.join(lines_init(String.split(span, Tx.newline())), nl_str()), sig}

def think_split.sig_of(m: Maybe<&2, String>, +span: String) -> ThinkMark:
  match m:
    case Some{+s}:
      think_split.signed(span, s)
    case None{}:
      think_split.plain(span)

def think_split.line(m: Maybe<&2, String>, +span: String) -> ThinkMark:
  match m:
    case Some{+l}:
      think_split.sig_of(think_sig(l), span)
    case None{}:
      think_split.plain(span)

def think_split(+span: String) -> ThinkMark:
  think_split.line(lines_last(String.split(span, Tx.newline())), span)

# the marked span, with the signature riding on its last line when the
# provider sent one: byte-identical to the unsigned form when sig is "".
# The BENDSIG line gets its own newline UNLESS the text already ends
# with one (think_texts joins parts with a trailing LF - the old
# pinned form keeps it, so the separator must not double it)
def think_marked.sep(is_nl: Bool) -> String:
  match is_nl:
    case True{}:
      ""
    case False{}:
      nl_str()

def think_marked.pick(is_sig: Bool, +text: String, +sig: String) -> String:
  match is_sig:
    case True{}:
      String.append("<think>",
        String.append(text,
          String.append(
            think_marked.sep(
              String.starts_with(String.reverse(text), nl_str())),
            String.append("BENDSIG::", String.append(sig, "</think>")))))
    case False{}:
      String.append("<think>", String.append(text, "</think>"))

def think_marked(+text: String, +sig: String) -> String:
  think_marked.pick(Bool.not(String.is_empty(sig)), text, sig)

# ---- whose signature is it (BISE-147) ----
# A think span's signature is the provider's opaque proof of its
# reasoning, and only that provider takes it back. Anthropic's
# signature rides bare (its format before BISE-147); OpenAI Responses'
# encrypted_content rides as "oair:<encrypted_content>". Each family
# replays only its own: another family's span is unsigned to it.
def resp_sig(+enc: String) -> String:
  String.append("oair:", enc)

def is_resp_sig(+sig: String) -> Bool:
  String.starts_with(sig, "oair:")

def resp_enc(+sig: String) -> String:
  String.drop(sig, 5n)

# the signature a family other than OpenAI Responses may replay
def own_sig.pick(foreign: Bool, +sig: String) -> String:
  Bool.pick(String, foreign, "", sig)

def bare_sig(+sig: String) -> String:
  own_sig.pick(is_resp_sig(sig), sig)

def asst_think.visible.strip(is_nl: Bool, +rest: String) -> String:
  match is_nl:
    case True{}:
      String.drop(rest, 1n)
    case False{}:
      rest

def asst_think.visible(+rest: String) -> String:
  asst_think.visible.strip(String.starts_with(rest, "\n"), rest)

def text_block_json(+text: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("text")),
    Con{J.kv("text", J.str(text)), Nil{}}})

def asst_think.raw_visible(+content: String, +n1: Nat) -> String:
  String.drop(content, Nat.add(n1, 8n))

def is_empty_tools(ts: List<&2, WTool>) -> Bool:
  match ts:
    case Nil{}:
      True{}
    case Con{_, _}:
      False{}

# ---- reading a provider response (JSON helpers, OK/CALL/END lines) ----

def parsed_or(r: Result<&2, &2, String, J.Json>) -> J.Json:
  match r:
    case Done{+j}:
      j
    case Fail{_}:
      J.JNull{}

def parse_json(+body: String) -> J.Json:
  parsed_or(J.parse(body))

def field_or.found(m: Maybe<&2, J.Json>, dflt: J.Json) -> J.Json:
  match m:
    case Some{+v}:
      v
    case None{}:
      dflt

def field_or(j: J.Json, +key: String, dflt: J.Json) -> J.Json:
  field_or.found(J.get(j, key), dflt)

def arr_field_or.arr(v: J.Json) -> List<&2, J.Json>:
  match v:
    case J.JArr{+vals}:
      vals
    case _:
      Nil{}

def arr_field_or.found(m: Maybe<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case Some{+v}:
      arr_field_or.arr(v)
    case None{}:
      Nil{}

def arr_field_or(j: J.Json, +key: String) -> List<&2, J.Json>:
  arr_field_or.found(J.get(j, key))

# extract "code" from structured JSON tool arguments (run_typescript);
# pass through if the args are not JSON or have no code field
def call_code.res(r: Result<&2, &2, String, J.Json>, +raw: String) -> String:
  match r:
    case Done{+j}:
      J.get_str_or(j, "code", raw)
    case Fail{_}:
      raw

def call_code(+args: String) -> String:
  call_code.res(J.parse(args), args)

# ---- the intent of a call (BISE-223) ----
# bash and run_typescript take an optional "description": one line the
# model writes for the user, shown in place of the code. run_typescript
# args are JSON already; a bash call keeps its JSON args
# {"arg": <cmd>, "description": <d>} only when it carries one, else its
# args stay the bare command (old sessions, other models, scenarios).

def json_field.res(r: Result<&2, &2, String, J.Json>, +key: String,
    +dflt: String) -> String:
  match r:
    case Done{+j}:
      J.get_str_or(j, key, dflt)
    case Fail{_}:
      dflt

# the command of a flat call: the "arg" of JSON args, else the args
def flat_cmd(+args: String) -> String:
  json_field.res(J.parse(args), "arg", args)

# one line, capped: what a model sent may hold newlines
def intent_line(+s: String) -> String:
  String.take(String.trim(Tx.flatten(s)), 200n)

# the description of a call's JSON args, one line; "" when none
def call_intent(+args: String) -> String:
  intent_line(json_field.res(J.parse(args), "description", ""))

def flat_in.pick(keep: Bool, +j: J.Json, +d: String, +dflt: String) -> String:
  match keep:
    case True{}:
      J.stringify(J.obj(Con{J.kv("arg", J.str(J.get_str_or(j, "arg", ""))),
        Con{J.kv("description", J.str(d)), Nil{}}}))
    case False{}:
      J.get_str_or(j, "arg", dflt)

# a flat call's input as the adapters read it: the "arg" string (dflt
# when it has none), or for a bash call with a description, the
# canonical {"arg", "description"} JSON
def flat_in(+name: String, +j: J.Json, +dflt: String) -> String:
  +d = intent_line(J.get_str_or(j, "description", ""))
  flat_in.pick(Bool.and(String.eq(name, "bash"),
    Bool.not(String.is_empty(d))), j, d, dflt)

def flat_out.found(m: Maybe<&2, String>, +j: J.Json, +args: String) -> J.Json:
  match m:
    case Some{_}:
      j
    case None{}:
      J.obj(Con{J.kv("arg", J.str(args)), Nil{}})

def flat_out.res(r: Result<&2, &2, String, J.Json>, +args: String) -> J.Json:
  match r:
    case Done{+j}:
      flat_out.found(J.get_str(j, "arg"), j, args)
    case Fail{_}:
      J.obj(Con{J.kv("arg", J.str(args)), Nil{}})

# a flat call's args back to a provider: JSON args with an "arg" ride
# as they are, a bare command gets the {"arg": ...} wrapper
def flat_out(+args: String) -> J.Json:
  flat_out.res(J.parse(args), args)

def join_lines(lines: List<&2, String>) -> String:
  match lines:
    case Nil{}:
      ""
    case Con{+l, rest}:
      String.append(l, String.append("\n", join_lines(rest)))

# ---- tool names on the wire (BISE-293) ----
# Every family bise speaks takes a tool name matching
# ^[a-zA-Z0-9_-]{1,64}$ (OpenAI: ^[a-zA-Z0-9_-]+$ and 64 chars;
# Anthropic: 128), the strictest wins. A harness name that does not
# (a dotted MCP-style "group.fn", a space, a non-ASCII char) rides
# with each other char as "_", cut to 64 chars; an empty one as "_".
# The request carries the wire name, in tools[] AND in the history's
# calls (an old session's top-level "self.compact" call replays as
# self_compact, never a 400). The reply side maps a returned name
# back through the request's own tools (name_in): the first tool
# whose wire name it is, else the name unchanged.

def name_char_ok(+c: U32) -> Bool:
  Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)),
    Bool.or(Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)),
      Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)),
        Bool.or(U32.is_eq(c, 95), U32.is_eq(c, 45)))))

def name_char(ok: Bool, c: U32) -> U32:
  match ok:
    case True{}:
      c
    case False{}:
      95

def name_chars(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Chr{name_char(name_char_ok(c), c)}, name_chars(t)}

def wire_name.pick(empty: Bool, +s: String) -> String:
  match empty:
    case True{}:
      "_"
    case False{}:
      s

def wire_name(+name: String) -> String:
  wire_name.pick(String.is_empty(name), String.take(name_chars(name), 64n))

# the pattern itself, the laws' check: 1 to 64 chars, each allowed
def name_ok.chars(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(name_char_ok(c), name_ok.chars(t))

def name_ok(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)),
    Bool.and(Nat.is_le(String.length(s), 64n), name_ok.chars(s)))

def wtool_name(+t: WTool) -> String:
  match t:
    case WTool{+name, _}:
      name

# a name the provider sent back, as the harness knows it
def name_in(tools: List<&2, WTool>, +wire: String) -> String:
  match tools:
    case Nil{}:
      wire
    case Con{+t, rest}:
      Bool.pick(String, String.eq(wire_name(wtool_name(t)), wire),
        wtool_name(t), name_in(rest, wire))

# one reply line: a "CALL <id> <name> : <args>" line gets its name
# back; any other line stays
def call_line_in.name(sp: Maybe<&2, Nat>, +tools: List<&2, WTool>,
    +head: String, +tail: String) -> String:
  match sp:
    case Some{+q}:
      String.append(head, String.append(name_in(tools, String.take(tail, q)),
        String.drop(tail, q)))
    case None{}:
      String.append(head, name_in(tools, tail))

def call_line_in.id(sp: Maybe<&2, Nat>, +tools: List<&2, WTool>,
    +rest: String) -> String:
  match sp:
    case Some{+p}:
      call_line_in.name(Tx.find(String.drop(rest, Nat.add(p, 1n)), " : ", 0n),
        tools, String.append("CALL ", String.take(rest, Nat.add(p, 1n))),
        String.drop(rest, Nat.add(p, 1n)))
    case None{}:
      String.append("CALL ", rest)

def call_line_in(is_call: Bool, +tools: List<&2, WTool>, +line: String)
    -> String:
  match is_call:
    case True{}:
      call_line_in.id(Tx.find(String.drop(line, 5n), " ", 0n), tools,
        String.drop(line, 5n))
    case False{}:
      line

def reply_lines_in(+tools: List<&2, WTool>, lines: List<&2, String>)
    -> List<&2, String>:
  match lines:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      Con{call_line_in(String.starts_with(l, "CALL "), tools, l),
        reply_lines_in(tools, rest)}

# a whole OK/CALL/END reply, every call named as the harness knows it
def reply_names_in(+tools: List<&2, WTool>, +reply: String) -> String:
  String.join(reply_lines_in(tools, String.split(reply, Tx.newline())),
    "\n")

# ---- a provider's error body, in its own words (BISE-293) ----
# The message of its JSON (OpenAI / Anthropic error.message, Mistral
# message, a bare error string, a FastAPI detail), else the body
# itself; on one line, runs of spaces as one. The full body stays in
# the debug log.
def err_text.first(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(a)), b, a)

def err_text(+body: String) -> String:
  +j = parse_json(body)
  err_text.first(J.get_str_or(field_or(j, "error", J.JNull{}), "message", ""),
    err_text.first(J.get_str_or(j, "message", ""),
      err_text.first(J.get_str_or(j, "error", ""),
        err_text.first(J.get_str_or(j, "detail", ""), body))))

def is_space(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 32), Bool.or(U32.is_eq(c, 9),
    Bool.or(U32.is_eq(c, 10), U32.is_eq(c, 13))))

def squeeze.pick(is_sp: Bool, prev_sp: Bool, c: U32, +rest: String)
    -> String:
  match is_sp:
    case True{}:
      Bool.pick(String, prev_sp, rest, SCon{Chr{32}, rest})
    case False{}:
      SCon{Chr{c}, rest}

def squeeze.go(s: String, sp: Bool) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      squeeze.pick(is_space(c), sp, c, squeeze.go(t, is_space(c)))

# whitespace runs (newlines included) become one space, trimmed
def squeeze(+s: String) -> String:
  String.trim(squeeze.go(s, True{}))

def err_said(+body: String) -> String:
  squeeze(err_text(body))

# ---- a request refused for its size (big-request, 2026-10-04) ----
# A 413, or a 400 whose words say the body is too big: Anthropic through
# the foundry proxy answers 400 "Request content length exceeded 32 MB
# limit"; Anthropic's own API 413 request_too_large; gateways "payload
# too large" / "request entity too large".
def size_words(+s: String) -> Bool:
  Bool.or(String.contains(s, "content length exceeded"),
    Bool.or(String.contains(s, "request_too_large"),
      Bool.or(String.contains(s, "request too large"),
        Bool.or(String.contains(s, "payload too large"),
          Bool.or(String.contains(s, "request entity too large"),
            String.contains(s, "request is too large"))))))

def size_refused(+status: U32, +body: String) -> Bool:
  Bool.or(U32.is_eq(status, 413),
    Bool.and(U32.is_eq(status, 400), size_words(String.to_lower(body))))

# the words a refusal for size carries in bise's line
# (runtime/provider-pure.bend refusal.head); the Core reads them on a
# failed call and compacts (core/session.bend)
def too_large_mark() -> String:
  "the request is too large"

def too_large(+error: String) -> Bool:
  String.contains(error, too_large_mark())

def reply_err(+status: U32, +body: String) -> String:
  String.append("ERROR provider ",
    String.append(U32.show(status),
      String.append(": ",
        String.append(String.take(err_said(body), 300n), "\nEND\n"))))

# the structured tools: their args ride as JSON (OpenAI arguments string,
# Anthropic input object); the others wrap a flat {"arg": ...}

def is_structured(+name: String) -> Bool:
  Bool.or(String.eq(name, "run_typescript"),
    Bool.or(String.eq(name, "search_tool_functions"),
      Bool.or(String.eq(name, "edit"), String.eq(name, "write_file"))))
# ---- the last guard before a provider request (BISE-242) ----
# Every family (core/api.bend) builds its body from this WReq: a call
# with no tool message right after its assistant message gets a failed
# one, and a tool message no call waits for is dropped, so the provider
# never sees an unpaired tool call (Anthropic answers 400 to it). The
# Core already pairs its input (H.pair_calls); this covers the rest
# (compaction requests, the context file, a runtime bug).

type WTag is Data:
  WTag{tool: Bool, m: WMsg}

def wtag(+m: WMsg) -> WTag:
  match m:
    case WMsg{+role, +content, +calls}:
      WTag{String.eq(role, "tool"), WMsg{role, content, calls}}

def wtags(ms: List<&2, WMsg>) -> List<&2, WTag>:
  match ms:
    case Nil{}:
      Nil{}
    case Con{+m, rest}:
      Con{wtag(m), wtags(rest)}

def wcalls(+m: WMsg) -> List<&2, WCall>:
  match m:
    case WMsg{_, _, +calls}:
      calls

def wno_result(+name: String) -> WMsg:
  WMsg{"tool", String.append("tool ", String.append(name,
    " failed: no result: bise restarted while this ran")), Nil{}}

def wcloses(owed: List<&2, WCall>) -> List<&2, WMsg>:
  match owed:
    case Nil{}:
      Nil{}
    case Con{c, rest}:
      match c:
        case WCall{_, +name, _}:
          Con{wno_result(name), wcloses(rest)}

def pair_tagged(ts: List<&2, WTag>, owed: List<&2, WCall>) -> List<&2, WMsg>:
  match ts:
    case Nil{}:
      wcloses(owed)
    case Con{t, rest}:
      match t:
        case WTag{tool, +m}:
          match tool:
            case True{}:
              match owed:
                case Nil{}:
                  pair_tagged(rest, Nil{})
                case Con{_, ow}:
                  Con{m, pair_tagged(rest, ow)}
            case False{}:
              List.append(&2, WMsg, wcloses(owed),
                Con{m, pair_tagged(rest, wcalls(m))})

def wowes(owed: List<&2, WCall>) -> Bool:
  match owed:
    case Nil{}:
      False{}
    case Con{_, _}:
      True{}

def paired_tagged(ts: List<&2, WTag>, owed: List<&2, WCall>) -> Bool:
  match ts:
    case Nil{}:
      Bool.not(wowes(owed))
    case Con{t, rest}:
      match t:
        case WTag{tool, +m}:
          match tool:
            case True{}:
              match owed:
                case Nil{}:
                  False{}
                case Con{_, ow}:
                  paired_tagged(rest, ow)
            case False{}:
              Bool.and(Bool.not(wowes(owed)), paired_tagged(rest, wcalls(m)))

# the request with every call answered
def pair_wire(req: WReq) -> WReq:
  match req:
    case WReq{+tools, +msgs, +cur}:
      WReq{tools, pair_tagged(wtags(msgs), Nil{}), cur}

# whether pair_wire changes the request (the runtime logs it)
def wire_unpaired(req: WReq) -> Bool:
  match req:
    case WReq{_, +msgs, _}:
      Bool.not(paired_tagged(wtags(msgs), Nil{}))
