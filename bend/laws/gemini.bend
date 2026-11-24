# bend/laws/gemini.bend: Gemini's thought signatures on the OpenAI Chat
# family (Gauthier's 400 of 2026-11-24, task gemini-sig).
# Gemini (its own OpenAI-compatible endpoint, a gateway's) signs its tool
# calls: tool_calls[i].extra_content.google.thought_signature, and may
# sign the message (extra_content). The next request of the turn must
# carry each back unchanged, on the call it came with, or Gemini answers
# 400 "Function call is missing a thought_signature in functionCall
# parts". The laws follow one tool turn the whole way the runtime takes
# it: the provider's reply -> the OK/CALL/END lines -> the stamp (whose
# it is) -> the Core's message -> the wire request -> the next body; and
# the checkpoint, for a resumed session. No network, no key: the replies
# are fixtures shaped like Gemini's.
import Base
import ../core/types.bend as T
import ../core/api.bend as Api
import ../core/wire.bend as W
import ../core/oai-chat.bend as Oai
import ../core/oai-stream.bend as Os
import ../core/checkpoint.bend as K
import ../runtime/remote.bend as Rm

# a Gemini reply: two calls with the provider's ids, the first signed,
# and a signed message
def reply_body() -> String:
  "{\"object\":\"chat.completion\",\"model\":\"gemini-4-neon\",\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":null,\"extra_content\":{\"google\":{\"thought_signature\":\"MSG_SIG\"}},\"tool_calls\":[{\"id\":\"function-call-7\",\"type\":\"function\",\"extra_content\":{\"google\":{\"thought_signature\":\"SIG_A\"}},\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"pwd\\\"}\"}},{\"id\":\"function-call-8\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"ls\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}"

# the same calls from a provider that signs nothing
def plain_body() -> String:
  "{\"object\":\"chat.completion\",\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":null,\"tool_calls\":[{\"id\":\"function-call-7\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"pwd\\\"}\"}},{\"id\":\"function-call-8\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"ls\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}"

# the signed reply, streamed: the call's signature rides its first
# piece, the message's a delta
def reply_sse() -> String:
  "data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"extra_content\":{\"google\":{\"thought_signature\":\"MSG_SIG\"}},\"tool_calls\":[{\"index\":0,\"id\":\"function-call-7\",\"type\":\"function\",\"extra_content\":{\"google\":{\"thought_signature\":\"SIG_A\"}},\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"pwd\\\"}\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"function-call-8\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"ls\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}

data: [DONE]

"

# the Core's assistant message of a reply, as the runtime makes it: the
# reply read and stamped with the provider and model it came from
def asst_of.cmd(c: T.Cmd) -> T.Msg:
  match c:
    case T.CompletionOk{_, +text, +calls, +extra}:
      T.asst_x(text, calls, extra)
    case _:
      T.asst("", Nil{})

def asst_of(+lines: String) -> T.Msg:
  asst_of.cmd(Rm.remote_cmd(Rm.stamp(Rm.parse_reply(lines), "google",
    "gemini-4-neon"), 1))

def law_cfg() -> T.Cfg:
  T.Cfg{4000n, 25n, 3n, "sys", Nil{}}

def turn(+lines: String) -> List<&2, T.Msg>:
  [T.user("where am i"), asst_of(lines), T.tool_res("/home"),
    T.tool_res("a b")]

# the next request of the turn, to that provider and model, built as the
# runtime builds it (runtime/provider.bend request_body)
def next_body_to(+prov: String, +model: String, +ms: List<&2, T.Msg>)
    -> String:
  Api.api_body_for(Api.OpenAI{}, model, Api.facts0(),
    Oai.own_extras(prov, model,
      W.pair_wire(W.parse_wire(Rm.build_request(law_cfg(), ms)))))

def next_body(+lines: String) -> String:
  next_body_to("google", "gemini-4-neon", turn(lines))

# LAW: the call's signature and the message's go back unchanged, each
# where it came; the calls keep the provider's ids and the results
# answer those ids.
law gemini_signatures_ride_the_next_request:
  {next_body(Oai.reply_of(200, reply_body()))
    == "{\"model\":\"gemini-4-neon\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"where am i\"},{\"role\":\"assistant\",\"content\":null,\"tool_calls\":[{\"id\":\"function-call-7\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"pwd\\\"}\"},\"extra_content\":{\"google\":{\"thought_signature\":\"SIG_A\"}}},{\"id\":\"function-call-8\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"ls\\\"}\"}}],\"extra_content\":{\"google\":{\"thought_signature\":\"MSG_SIG\"}}},{\"role\":\"tool\",\"content\":\"/home\",\"tool_call_id\":\"function-call-7\"},{\"role\":\"tool\",\"content\":\"a b\",\"tool_call_id\":\"function-call-8\"}]}" : String}

# LAW: a streamed reply replays the same as a whole one
law gemini_stream_signatures_ride_the_next_request:
  {next_body(Os.reply(reply_sse()))
    == next_body(Oai.reply_of(200, reply_body())) : String}

# LAW: a reply with nothing signed changes nothing: no EXTRA line, and
# the next body is today's (call_<n> ids, no extra_content)
law no_extra_body_is_unchanged:
  {(Oai.reply_of(200, plain_body()), next_body(Oai.reply_of(200, plain_body())))
    == ("OK 
CALL 1 bash : pwd
CALL 2 bash : ls
END
", "{\"model\":\"gemini-4-neon\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"where am i\"},{\"role\":\"assistant\",\"content\":null,\"tool_calls\":[{\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"pwd\\\"}\"}},{\"id\":\"call_2\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"ls\\\"}\"}}]},{\"role\":\"tool\",\"content\":\"/home\",\"tool_call_id\":\"call_1\"},{\"role\":\"tool\",\"content\":\"a b\",\"tool_call_id\":\"call_2\"}]}") : String & String}

# LAW: the signatures go back only to the provider and model that wrote
# them: another model (a /model switch) or another provider (the same
# model through a gateway) gets the message as if nothing were signed.
law extra_replays_only_to_its_model:
  {[next_body_to("google", "gemini-4-pro", turn(Oai.reply_of(200, reply_body()))),
      next_body_to("gateway", "gemini-4-neon", turn(Oai.reply_of(200, reply_body())))]
    == [next_body_to("google", "gemini-4-pro", turn(Oai.reply_of(200, plain_body()))),
      next_body_to("gateway", "gemini-4-neon", turn(Oai.reply_of(200, plain_body())))]
      : List<&2, String>}

# the reply's message with one call less (compaction, an edit, a bug):
# its extra still holds two parts
def shifted(+m: T.Msg) -> T.Msg:
  match m:
    case T.Msg{+role, +text, Con{_, +rest}, +inj, +extra}:
      T.Msg{role, text, rest, inj, extra}
    case _:
      m

# LAW: the parts go back only when they align with the message's calls:
# a message whose calls changed sends no signature at all, never one
# call's signature on another call
law extra_replays_only_when_calls_align:
  {next_body_to("google", "gemini-4-neon",
      [T.user("where am i"), shifted(asst_of(Oai.reply_of(200, reply_body()))),
        T.tool_res("a b")])
    == next_body_to("google", "gemini-4-neon",
      [T.user("where am i"), shifted(asst_of(Oai.reply_of(200, plain_body()))),
        T.tool_res("a b")]) : String}

def sess_of(+ms: List<&2, T.Msg>) -> T.Session:
  T.Sess{law_cfg(), ms, 3n, 7, None{}, None{}, Nil{}, Nil{}}

# LAW: a resumed session keeps them: the checkpoint's text form (an
# EXTRA sub-line under the message) parses back to the same message
law checkpoint_keeps_the_extra:
  {K.from_text(K.to_text(K.export(sess_of(turn(Oai.reply_of(200, reply_body()))))))
    == Some{K.export(sess_of(turn(Oai.reply_of(200, reply_body()))))}
      : Maybe<&2, K.Checkpoint>}

# LAW: a binary older than the EXTRA line (a rolled-back switch) skips
# it as any line it does not know (its step.pick, known: False): it
# reads the session and only loses the signatures
law old_reader_skips_the_extra_line:
  for +st: K.FS
  {K.step.pick(False{}, st, "  EXTRA {}") == st : K.FS}
