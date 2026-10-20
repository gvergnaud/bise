# Offline regression contract for the DeepSeek Chat dialect. No API key,
# account or network call: exercise the same pure adapters the runtime uses.
import Base
import ../core/api.bend as Api
import ../core/deepseek.bend as Ds
import ../core/oai-chat.bend as Oai
import ../core/wire.bend as W
import ../runtime/provider-pure.bend as Pv
import ../vendor/json.bend as J

# Use the public family in the registry, as the built-in catalog does.
law deepseek_provider_resolves_its_dialect:
  {Pv.resolve_id("deepseek/deepseek-flash", Pv.registry("[providers.deepseek]
api = \"openai-chat\"
base_url = \"https://api.deepseek.com/v1\"
key_env = \"DEEPSEEK_API_KEY\"
reasoning = true
"), "")
    == Pv.ROk{Pv.Prov{"deepseek", "deepseek-flash", Api.DeepSeek{},
      "https://api.deepseek.com/v1/chat/completions", "DEEPSEEK_API_KEY",
      Api.MFacts{True{}, 0n, "max_tokens", "", "", "", False{}}}} : Pv.Resolved}

law deepseek_dialect_is_provider_and_family_specific:
  {[Api.style_for_provider("deepseek", "openai-chat"),
      Api.style_for_provider("mistral", "openai-chat"),
      Api.style_for_provider("gateway", "openai-chat"),
      Api.style_for_provider("deepseek", "anthropic"),
      Api.style_for_provider("deepseek", "typo")]
    == [Some{Api.DeepSeek{}}, Some{Api.OpenAI{}}, Some{Api.OpenAI{}},
      Some{Api.Anthropic{}}, None{}] : List<&2, Maybe<&2, Api.Style>>}

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

# Tool continuations need all previous reasoning, including non-tool
# assistant turns. Empty visible content is null; ids pair FIFO unchanged.
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
