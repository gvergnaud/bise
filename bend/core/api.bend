# core/api.bend — the provider families: every per-family fact, pure
# Bend, in one place.
#
# runtime/remote.bend prints the harness wire request (MODEL/TOOL/MSG/
# CALL lines, pinned by LAWS.bend); core/wire.bend parses it; the
# family of the model maps it to its JSON body and maps the provider
# response back to the OK/CALL/END reply the Core parses:
#   core/wire.bend       the wire, shared by every family
#   core/oai-chat.bend   OpenAI Chat Completions (and compatibles)
#   core/oai-stream.bend OpenAI Chat, streamed
#   core/anthropic.bend  Anthropic Messages (+ core/anth-stream.bend)
#
# The family is one of the two registries of the provider arc (the
# other one, the providers, is runtime/provider-pure.bend: a provider
# names its family by `api = "<name>"`). Everything the runtime needs to
# know about a family is answered here, so runtime/provider.bend and
# runtime/usage-pure.bend never match on it:
#   style_of_api / api_name   the `api` name <-> the family
#   endpoint                  the path appended to a provider's base_url
#   headers                   the HTTP headers (the key, when there is one)
#   streams / wire_body       whether the call streams (SSE), its body
#   MFacts                    what the catalog says of the model
#   api_body_for              the request body
#   reply_of_for              a whole reply -> OK/CALL/END
#   stream_reply              a streamed reply -> OK/CALL/END
#   usage_of / stream_usage   the token usage of a reply
# A new family (BISE-147 Responses: done; 148 Gemini) is one more constructor,
# one more case in each function below, and its wire module.

import Base
import ./wire.bend as W
import ./oai-chat.bend as Oai
import ./anthropic.bend as Anth
import ./anth-stream.bend as As
import ./oai-stream.bend as Os
import ./oai-resp.bend as Rs
import ../vendor/json.bend as J

type Style is Data:
  OpenAI{}
  Anthropic{}
  OpenAIResp{}

# ---- the family's name (`api = ...` in a provider table) ----

def style_of_api.resp(is_resp: Bool) -> Maybe<&2, Style>:
  match is_resp:
    case True{}:
      Some{OpenAIResp{}}
    case False{}:
      None{}

def style_of_api.chat(is_chat: Bool, is_anth: Bool, is_resp: Bool)
    -> Maybe<&2, Style>:
  match is_chat:
    case True{}:
      Some{OpenAI{}}
    case False{}:
      match is_anth:
        case True{}:
          Some{Anthropic{}}
        case False{}:
          style_of_api.resp(is_resp)

# None: a family bise does not speak (yet): gemini, bedrock-converse,
# a typo (openai-responses: BISE-147)
def style_of_api(+api: String) -> Maybe<&2, Style>:
  style_of_api.chat(String.eq(api, "openai-chat"), String.eq(api, "anthropic"),
    String.eq(api, "openai-responses"))

def api_name(st: Style) -> String:
  match st:
    case OpenAI{}:
      "openai-chat"
    case Anthropic{}:
      "anthropic"
    case OpenAIResp{}:
      "openai-responses"

def is_anthropic(st: Style) -> Bool:
  match st:
    case OpenAI{}:
      False{}
    case Anthropic{}:
      True{}
    case OpenAIResp{}:
      False{}

# ---- the endpoint: base_url + this path ----

def endpoint(st: Style) -> String:
  match st:
    case OpenAI{}:
      "/chat/completions"
    case Anthropic{}:
      "/messages"
    case OpenAIResp{}:
      "/responses"

# ---- the headers, in the order they are set ----
# No key (a provider with key_env = "": a local server) sends no auth
# header.

type Hdr is Data:
  Hdr{name: String, val: String}

def headers.auth(no_key: Bool, +h: Hdr, +rest: List<&2, Hdr>)
    -> List<&2, Hdr>:
  match no_key:
    case True{}:
      rest
    case False{}:
      Con{h, rest}

# the anthropic-beta flags: the model's (catalog `betas`, BISE-146: the
# direct API takes no 1M-context flag), else the foundry proxy's list
def anth_betas.pick(empty: Bool, +betas: String) -> String:
  match empty:
    case True{}:
      "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14,prompt-caching-2024-07-31,context-1m-2025-08-07"
    case False{}:
      betas

def anth_betas(+betas: String) -> String:
  anth_betas.pick(String.is_empty(betas), betas)

def headers(st: Style, +betas: String, +key: String) -> List<&2, Hdr>:
  match st:
    case OpenAI{}:
      headers.auth(String.is_empty(key),
        Hdr{"Authorization", String.append("Bearer ", key)},
        [Hdr{"Content-Type", "application/json"}])
    case OpenAIResp{}:
      headers.auth(String.is_empty(key),
        Hdr{"Authorization", String.append("Bearer ", key)},
        [Hdr{"Content-Type", "application/json"}])
    case Anthropic{}:
      headers.auth(String.is_empty(key), Hdr{"x-api-key", key},
        [Hdr{"anthropic-version", "2023-06-01"},
          Hdr{"anthropic-beta", anth_betas(betas)},
          Hdr{"Content-Type", "application/json"}])

# ---- streaming: every family streams (Server-Sent Events) ----

def streams(st: Style) -> Bool:
  match st:
    case OpenAI{}:
      True{}
    case Anthropic{}:
      True{}
    case OpenAIResp{}:
      True{}

def wire_body(st: Style, +body: String) -> String:
  match st:
    case OpenAI{}:
      Os.stream_body(body)
    case Anthropic{}:
      As.stream_body(body)
    case OpenAIResp{}:
      Rs.stream_body(body)

# ---- what the catalog says of the model ----
# runtime/provider-pure.bend fills it from the models file (a key of
# models."p/id", else of providers.p): reasoning = `reasoning = true`;
# output = `max_output` (0n: not set); max_key = the name of the output
# cap in an OpenAI Chat body ("max_completion_tokens" for OpenAI itself,
# whose reasoning models refuse max_tokens; "max_tokens" elsewhere);
# thinking / betas: the raw keys (Anthropic, BISE-146; "" = not set);
# plan: the provider's `shape = "chatgpt-plan"` (OpenAI's ChatGPT plan
# route: core/oai-resp.bend's plan body, the plan's error lines).

# effort: the reasoning effort of the call (BISE-135: the session's
# choice, config, the model's default; "" = none sent), already checked
# against the model's words (runtime/provider-pure.bend effort_of).

type MFacts is Data:
  MFacts{reasoning: Bool, output: Nat, max_key: String, thinking: String,
    betas: String, effort: String, plan: Bool}

def facts_betas(f: MFacts) -> String:
  match f:
    case MFacts{_, _, _, _, +betas, _, _}:
      betas

def facts_effort(f: MFacts) -> String:
  match f:
    case MFacts{_, _, _, _, _, +effort, _}:
      effort

def facts_plan(f: MFacts) -> Bool:
  match f:
    case MFacts{_, _, _, _, _, _, +plan}:
      plan

# no catalog entry: today's body (reasoning_effort high, no output cap)
def facts0() -> MFacts:
  MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}

# ---- the prompt cache's routing key (BISE-268) ----
# Some providers route a request to the machine that holds its cached
# prefix when it carries a key: a body field (OpenAI, Mistral, Cerebras
# `prompt_cache_key`) or a header (xAI `x-grok-conv-id`, OpenRouter
# `x-session-id`, Fireworks `x-session-affinity`); the catalog names it
# per provider (cache_key / cache_header). The key is the session's:
# "bise-" + an FNV-1a hash of the request's first two messages (the
# system prompt and the first one after it), so every call of a session
# carries the same key, a new session (or a compacted one, whose prefix
# is new anyway) another; no secret or path goes out.

def cache_hash.go(fuel: Nat, +s: String, +h: U32) -> U32:
  match fuel:
    case 0n:
      h
    case 1n+k:
      match s:
        case SNil{}:
          h
        case SCon{Chr{+c}, t}:
          cache_hash.go(k, t, U32.mul(U32.xor(h, c), 16777619))

def cache_hash(+s: String) -> U32:
  cache_hash.go(Nat.add(String.length(s), 1n), s, 2166136261)

def cache_seed(msgs: List<&2, W.WMsg>) -> String:
  match msgs:
    case Nil{}:
      ""
    case Con{+m, +rest}:
      match m:
        case W.WMsg{_, +c1, _}:
          match rest:
            case Nil{}:
              c1
            case Con{+m2, _}:
              match m2:
                case W.WMsg{_, +c2, _}:
                  String.append(c1, String.append("\n", c2))

def session_key(+req: W.WReq) -> String:
  match req:
    case W.WReq{_, +msgs, _}:
      String.append("bise-", U32.show(cache_hash(cache_seed(msgs))))

# the body with the key's field first ("" field: unchanged)
def with_cache_key.pick(none: Bool, +field: String, +key: String,
    +body: String) -> String:
  match none:
    case True{}:
      body
    case False{}:
      String.concat(["{", J.stringify(J.str(field)), ":",
        J.stringify(J.str(key)), ",", String.drop(body, 1n)])

def with_cache_key(+field: String, +key: String, +body: String) -> String:
  with_cache_key.pick(String.is_empty(field), field, key, body)

# the headers with the key's header last ("" name: unchanged)
def with_cache_header.pick(none: Bool, +name: String, +key: String,
    +hs: List<&2, Hdr>) -> List<&2, Hdr>:
  match none:
    case True{}:
      hs
    case False{}:
      List.append(&2, Hdr, hs, [Hdr{name, key}])

def with_cache_header(+name: String, +key: String, +hs: List<&2, Hdr>)
    -> List<&2, Hdr>:
  with_cache_header.pick(String.is_empty(name), name, key, hs)

# ---- extra headers (a gateway's: the catalog's headers_env) ----
# The text of the variable headers_env names: one "Name: value" per
# line, Claude Code's ANTHROPIC_CUSTOM_HEADERS form. A line with no ':'
# or no name is skipped; the value keeps any ':' of its own.

def extra_header.named(empty: Bool, +n: String, +v: String) -> List<&2, Hdr>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      [Hdr{n, v}]

def extra_header(parts: List<&2, String>) -> List<&2, Hdr>:
  match parts:
    case Nil{}:
      Nil{}
    case Con{_, Nil{}}:
      Nil{}
    case Con{+n, +rest}:
      extra_header.named(String.is_empty(String.trim(n)), String.trim(n),
        String.trim(String.join(rest, ":")))

def extra_headers.go(lines: List<&2, String>) -> List<&2, Hdr>:
  match lines:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      List.append(&2, Hdr, extra_header(String.split(l, ':')),
        extra_headers.go(rest))

def extra_headers(+text: String) -> List<&2, Hdr>:
  extra_headers.go(String.split(text, Char.from_u32(10)))

# the headers with the extra ones last: they win over the family's
def with_extra_headers(+text: String, +hs: List<&2, Hdr>) -> List<&2, Hdr>:
  List.append(&2, Hdr, hs, extra_headers(text))

# a key from key_command also goes as a bearer token, like Claude Code's
# apiKeyHelper (X-Api-Key and Authorization): a gateway in front of
# Anthropic may read only the second
def with_bearer(on: Bool, +key: String, +hs: List<&2, Hdr>) -> List<&2, Hdr>:
  match on:
    case True{}:
      List.append(&2, Hdr, hs, [Hdr{"Authorization", String.append("Bearer ", key)}])
    case False{}:
      hs

# ---- the body and the reply ----

def api_body_for(st: Style, +model: String, +facts: MFacts, +req: W.WReq)
    -> String:
  match st:
    case OpenAI{}:
      match facts:
        case MFacts{+reasoning, +output, +max_key, _, _, +effort, _}:
          Oai.api_body_with(model, Oai.Opts{reasoning, output, max_key, effort},
            req)
    case Anthropic{}:
      match facts:
        case MFacts{+reasoning, +output, _, +thinking, _, +effort, _}:
          Anth.api_body_anth_for(model, output, thinking, reasoning, effort,
            req)
    case OpenAIResp{}:
      match facts:
        case MFacts{+reasoning, +output, _, _, _, +effort, +plan}:
          Rs.api_body(model, Rs.Opts{reasoning, output, effort, plan}, req)

def reply_of_for(st: Style, +status: U32, +body: String) -> String:
  match st:
    case OpenAI{}:
      Oai.reply_of(status, body)
    case Anthropic{}:
      Anth.reply_anth_of(status, body)
    case OpenAIResp{}:
      Rs.reply_of(status, body)

# a 200 of a streaming family (its events, or a whole JSON error body)
def stream_reply(st: Style, +text: String) -> String:
  match st:
    case OpenAI{}:
      Os.reply_any(text)
    case Anthropic{}:
      As.reply_any(text)
    case OpenAIResp{}:
      Rs.reply_any(text)

# the code in a reply's error ("" none): the Responses family reads
# error.code (whole JSON, or the stream's terminal event); the others
# say nothing (no family but the plan route has a code bise maps yet)
def error_code(st: Style, +body: String) -> String:
  match st:
    case OpenAI{}:
      ""
    case Anthropic{}:
      ""
    case OpenAIResp{}:
      Rs.error_code(body)

# ---- the token usage ----
# in: every input token of the call (cached ones included); out: the
# tokens the model wrote; cr/cw: the cache reads and writes.

type Usage is Data:
  Usage{inp: Nat, out: Nat, cr: Nat, cw: Nat}

# Anthropic Messages: input_tokens counts only the uncached part;
# OpenAI-compatible: prompt_tokens already counts the cached part
def usage_of(st: Style, +u: J.Json) -> Usage:
  match st:
    case OpenAI{}:
      Usage{J.get_num_or(u, "prompt_tokens", 0n),
        J.get_num_or(u, "completion_tokens", 0n),
        J.get_num_or(W.field_or(u, "prompt_tokens_details", J.JNull{}),
          "cached_tokens", 0n),
        0n}
    case Anthropic{}:
      +cr = J.get_num_or(u, "cache_read_input_tokens", 0n)
      +cw = J.get_num_or(u, "cache_creation_input_tokens", 0n)
      Usage{Nat.add(J.get_num_or(u, "input_tokens", 0n), Nat.add(cr, cw)),
        J.get_num_or(u, "output_tokens", 0n), cr, cw}
    # Responses: input_tokens counts the cached part too (BISE-147)
    case OpenAIResp{}:
      Usage{J.get_num_or(u, "input_tokens", 0n),
        J.get_num_or(u, "output_tokens", 0n),
        J.get_num_or(W.field_or(u, "input_tokens_details", J.JNull{}),
          "cached_tokens", 0n),
        0n}

# the "usage" object a streamed body spreads over its events
def stream_usage(st: Style, +text: String) -> J.Json:
  match st:
    case OpenAI{}:
      Os.usage(text)
    case Anthropic{}:
      As.usage(text)
    case OpenAIResp{}:
      Rs.usage(text)
