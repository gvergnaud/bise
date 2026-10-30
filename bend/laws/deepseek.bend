# Offline regression contract for the DeepSeek Chat dialect. No API key,
# account or network call: exercise the same pure adapters the runtime uses.
import Base
import ../core/api.bend as Api
import ../core/deepseek.bend as Ds
import ../core/oai-chat.bend as Oai
import ../core/wire.bend as W
import ../runtime/provider-pure.bend as Pv
import ../runtime/remote.bend as Rm
import ../vendor/json.bend as J

# The catalog names the dialect (models.toml: api = "deepseek-chat").
law deepseek_provider_resolves_its_dialect:
  {Pv.resolve_id("deepseek/deepseek-flash", Pv.registry("[providers.deepseek]
api = \"deepseek-chat\"
base_url = \"https://api.deepseek.com/v1\"
key_env = \"DEEPSEEK_API_KEY\"
reasoning = true
"), "")
    == Pv.ROk{Pv.Prov{"deepseek", "deepseek-flash", Api.DeepSeek{},
      "https://api.deepseek.com/v1/chat/completions", "DEEPSEEK_API_KEY",
      Api.MFacts{True{}, 0n, "max_tokens", "", "", "", False{}}}} : Pv.Resolved}

# The api name picks the dialect, never the provider's id: openai-chat
# stays OpenAI's for every provider, deepseek-chat round-trips.
law deepseek_dialect_is_named_by_its_api:
  {[Api.style_of_api("deepseek-chat"), Api.style_of_api("openai-chat"),
      Api.style_of_api("typo")]
    == [Some{Api.DeepSeek{}}, Some{Api.OpenAI{}}, None{}]
      : List<&2, Maybe<&2, Api.Style>>}

law deepseek_native_whitespace_is_verbatim:
  {Ds.split("<think>  native\nreason \n</think>\nanswer")
    == Ds.H{"answer", "  native\nreason \n"} : Ds.History}

law deepseek_no_marker_keeps_visible_whitespace:
  {Ds.split("\nanswer\n") == Ds.H{"\nanswer\n", ""} : Ds.History}

law deepseek_foreign_signatures_stay_out:
  {[Ds.split("<think>foreign\nBENDSIG::anthsig</think>\nanswer"),
      Ds.split("<think>foreign\nBENDSIG::oair:ENC1</think>\nanswer")]
    == [Ds.H{"answer", ""}, Ds.H{"answer", ""}] : List<&2, Ds.History>}

law deepseek_all_spans_are_separated_from_visible_text:
  {Ds.split("prefix<think>one</think>\nmiddle<think>foreign\nBENDSIG::sig</think><think>two</think>\nsuffix")
    == Ds.H{"prefixmiddlesuffix", "onetwo"} : Ds.History}

# The exact regression for the 422: only visible content and sibling
# reasoning_content. Never a thinking content part or a leaked marker.
law deepseek_normal_history_high:
  {Api.api_body_for(Api.DeepSeek{}, "deepseek-flash", Api.facts0(),
      W.parse_wire("MODEL default
MSG user : calculate
MSG assistant : <think>2+2=4</think>\\n4
MSG user : continue
END
"))
    == "{\"model\":\"deepseek-flash\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"calculate\"},{\"role\":\"assistant\",\"content\":\"4\",\"reasoning_content\":\"2+2=4\"},{\"role\":\"user\",\"content\":\"continue\"}]}" : String}

law deepseek_normal_history_none:
  {Api.api_body_for(Api.DeepSeek{}, "deepseek-flash",
      Api.MFacts{True{}, 64n, "max_tokens", "", "", "none", False{}},
      W.parse_wire("MODEL default
MSG assistant : <think>old</think>\\nanswer
END
"))
    == "{\"model\":\"deepseek-flash\",\"max_tokens\":64,\"thinking\":{\"type\":\"disabled\"},\"messages\":[{\"role\":\"assistant\",\"content\":\"answer\",\"reasoning_content\":\"old\"}]}" : String}

# An assistant turn with no call and no visible text (blank, or only
# reasoning, native or foreign) is left out: its content would be ""
# (main's 400 of 2026-10-30, "text content blocks must be non-empty").
# Its neighbours stay as they are; the turns around a visible one keep
# their reasoning (deepseek_tools_keep_all_turns_reasoning_and_ids).
def empty_body(+effort: String) -> J.Json:
  W.parse_json(Api.api_body_for(Api.DeepSeek{}, "deepseek-flash",
    Api.MFacts{True{}, 64n, "max_tokens", "", "", effort, False{}},
    W.WReq{Nil{}, [W.WMsg{"assistant", "", Nil{}},
      W.WMsg{"assistant", "<think> native\n </think>", Nil{}},
      W.WMsg{"assistant", "<think>foreign\nBENDSIG::sig</think>", Nil{}}], None{}}))

law deepseek_empty_and_reasoning_only_turns_are_left_out:
  {W.arr_field_or(empty_body("high"), "messages") == Nil{} : List<&2, J.Json>}

law deepseek_empty_turns_none_keeps_history:
  {W.arr_field_or(empty_body("none"), "messages")
    == W.arr_field_or(empty_body("high"), "messages") : List<&2, J.Json>}

# Tool continuations need all previous reasoning, including non-tool
# assistant turns. Empty tool-call content is null; ids pair FIFO unchanged.
def tool_req() -> W.WReq:
  W.parse_wire("MODEL default
TOOL echo : Returns text.
MSG user : go
MSG assistant : <think>first</think>\\nready
MSG user : again
MSG assistant : <think>next</think>
  CALL 7 echo : hi
  CALL 8 echo : bye
MSG tool : out1
MSG tool : out2
END
")

def tool_body(+effort: String) -> J.Json:
  W.parse_json(Api.api_body_for(Api.DeepSeek{}, "deepseek-flash",
    Api.MFacts{True{}, 64n, "max_tokens", "", "", effort, False{}}, tool_req()))

law deepseek_tools_keep_all_turns_reasoning_and_ids:
  {W.arr_field_or(tool_body("high"), "messages")
    == W.arr_field_or(W.parse_json("{\"messages\":[{\"role\":\"user\",\"content\":\"go\"},{\"role\":\"assistant\",\"content\":\"ready\",\"reasoning_content\":\"first\"},{\"role\":\"user\",\"content\":\"again\"},{\"role\":\"assistant\",\"content\":null,\"reasoning_content\":\"next\",\"tool_calls\":[{\"id\":\"call_7\",\"type\":\"function\",\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\":\\\"hi\\\"}\"}},{\"id\":\"call_8\",\"type\":\"function\",\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\":\\\"bye\\\"}\"}}]},{\"role\":\"tool\",\"content\":\"out1\",\"tool_call_id\":\"call_7\"},{\"role\":\"tool\",\"content\":\"out2\",\"tool_call_id\":\"call_8\"}]}"), "messages") : List<&2, J.Json>}

law deepseek_tools_none_keeps_history:
  {W.arr_field_or(tool_body("none"), "messages")
    == W.arr_field_or(tool_body("high"), "messages") : List<&2, J.Json>}

law deepseek_tools_none_disables_thinking:
  {(J.get_str_or(W.field_or(tool_body("none"), "thinking", J.JNull{}), "type", ""),
      J.get_str_or(tool_body("none"), "reasoning_effort", ""))
    == ("disabled", "") : String & String}

def signed_tool_body(+effort: String) -> J.Json:
  W.parse_json(Api.api_body_for(Api.DeepSeek{}, "deepseek-flash",
    Api.MFacts{True{}, 64n, "max_tokens", "", "", effort, False{}},
    W.parse_wire("MODEL default
TOOL echo : Returns text.
MSG assistant : <think>foreign\\nBENDSIG::oair:ENC1</think>
  CALL 7 echo : hi
MSG tool : out
END
")))

law deepseek_foreign_signed_tool_history_is_sanitized:
  {W.arr_field_or(signed_tool_body("high"), "messages")
    == W.arr_field_or(W.parse_json("{\"messages\":[{\"role\":\"assistant\",\"content\":null,\"reasoning_content\":\"\",\"tool_calls\":[{\"id\":\"call_7\",\"type\":\"function\",\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\":\\\"hi\\\"}\"}}]},{\"role\":\"tool\",\"content\":\"out\",\"tool_call_id\":\"call_7\"}]}"), "messages") : List<&2, J.Json>}

law deepseek_foreign_signed_tool_history_none_is_sanitized:
  {W.arr_field_or(signed_tool_body("none"), "messages")
    == W.arr_field_or(signed_tool_body("high"), "messages") : List<&2, J.Json>}

# Mistral/vibe keeps its signed and unsigned ThinkChunk shapes byte-for-byte.
law mistral_history_is_unchanged:
  {Oai.asst_content_json("<think>native</think>\nanswer")
    == W.parse_json("[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"native\"}]},{\"type\":\"text\",\"text\":\"answer\"}]") : J.Json}

law mistral_signed_history_is_unchanged:
  {Oai.asst_content_json("<think>native\nBENDSIG::sig</think>\nanswer")
    == W.parse_json("[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"native\"}],\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"answer\"}]") : J.Json}

# Whole and streamed native reasoning must reach the wire too: dropping
# it would fix the 422 only to break the next tool continuation with 400.
law deepseek_whole_reply_keeps_reasoning:
  {Api.reply_of_for(Api.DeepSeek{}, 200,
      "{\"choices\":[{\"message\":{\"content\":\"answer\",\"reasoning_content\":\"  native \\n\"}}]}")
    == "OK <think>  native \\n</think>\\nanswer
END
" : String}

law deepseek_stream_reply_keeps_reasoning_and_calls:
  {Api.stream_reply(Api.DeepSeek{}, "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"native\"}}]}

data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"fake\",\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\":\\\"hi\\\"}\"}}]}}]}

data: [DONE]

")
    == "OK <think>native</think>\\n
CALL 1 echo : hi
END
" : String}

law deepseek_stream_errors_stay_errors:
  {String.starts_with(Api.stream_reply(Api.DeepSeek{},
      "data: {\"error\":{\"message\":\"fake failure\",\"type\":\"server_error\"}}

"), "ERROR provider 200:") == True{} : Bool}

law deepseek_http_errors_stay_errors:
  {Api.reply_of_for(Api.DeepSeek{}, 422, "fake invalid content")
    == W.reply_err(422, "fake invalid content") : String}

# The round trip the runtime makes between two calls: a reply's OK line
# is parsed (runtime/remote.bend), kept as the assistant's text, and
# sent back as its MSG line in the next request.
def asst_line(r: Rm.RemoteRes) -> String:
  match r:
    case Rm.RROk{+text, _}:
      String.append("MSG assistant : ", text)
    case Rm.RRErr{+why}:
      String.append("MSG user : error ", why)

def next_body(+reply: String) -> String:
  Api.api_body_for(Api.DeepSeek{}, "deepseek-flash", Api.facts0(),
    W.parse_wire(String.concat(["MODEL default
MSG user : go
", asst_line(Rm.parse_reply(reply)), "
MSG user : next
END
"])))

# DeepSeek's own streamed reasoning comes back as reasoning_content, the
# answer as content (the 400 of a tool continuation without it).
law deepseek_reasoning_round_trips:
  {next_body(Api.stream_reply(Api.DeepSeek{}, "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\" native\\n \",\"content\":\"answer\"}}]}

data: [DONE]

"))
    == "{\"model\":\"deepseek-flash\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"go\"},{\"role\":\"assistant\",\"content\":\"answer\",\"reasoning_content\":\" native\\n \"},{\"role\":\"user\",\"content\":\"next\"}]}" : String}

# A Mistral/vibe reply with a signed thinking block, then the thread
# moves to DeepSeek: its text goes back, the thinking never does (the 422).
law deepseek_foreign_thinking_round_trips_out:
  {next_body(Api.stream_reply(Api.OpenAI{}, "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"foreign thought\"}],\"signature\":\"foreign-sig\"},{\"type\":\"text\",\"text\":\"foreign answer\"}]}}]}

data: [DONE]

"))
    == "{\"model\":\"deepseek-flash\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"go\"},{\"role\":\"assistant\",\"content\":\"foreign answer\",\"reasoning_content\":\"\"},{\"role\":\"user\",\"content\":\"next\"}]}" : String}
