# bend/laws/streams.bend: refusals, the config file, harness-info, --resume, the Anthropic and OpenAI Chat streams, switchboard hooks.
# Open claims, each one closed by bend/proofs/streams.bend (moved from
# LAWS.bend). The gate checks every bend/proofs/*.bend: a law here
# with no proof there fails it.
import Base
import ../core/types.bend as T
import ../core/config.bend as Cf
import ../core/api.bend as Api
import ../core/wire.bend as W
import ../core/oai-chat.bend as Oai
import ../core/anth-stream.bend as As
import ../core/oai-stream.bend as Os
import ../runtime/remote.bend as Rm
import ../runtime/main-pure.bend as Rt
import ../runtime/bash-pure.bend as Sh
import ../runtime/provider-pure.bend as Pv
import ../runtime/settings-pure.bend as St
import ../runtime/repl-core-pure.bend as Rp
import ../runtime/debug-pure.bend as Dg
import ../runtime/usage-pure.bend as Us
import ../core/ev.bend as Ev

# ---- a refused request, in bise's words and the provider's (BISE-293) ----
# The feed showed the provider's raw JSON twice; now one line: who
# refused, the status, what to check, then the provider's own message
# (its error.message), one line, ~200 chars, the key masked.

law refusal_quotes_the_provider:
  {Pv.refusal("OpenAI", 400,
      "{\n  \"error\": {\n    \"message\": \"Invalid 'tools[0].function.name': string does not match pattern.\",\n    \"type\": \"invalid_request_error\"\n  }\n}", "")
    == "OpenAI refused the request (400). OpenAI said: \"Invalid 'tools[0].function.name': string does not match pattern.\""
    : String}

law refusal_of_a_key_masks_it:
  {Pv.refusal("OpenAI", 401,
      "{\"error\":{\"message\":\"Incorrect API key provided: sk-abcdefghijklmnop.\"}}",
      "sk-abcdefghijklmnop")
    == "OpenAI refused the key (401). check it with /setup. OpenAI said: \"Incorrect API key provided: …mnop.\""
    : String}

law refusal_without_a_name_or_json:
  {Pv.refusal("", 404, "no  such\n model", "")
    == "the provider refused the request (404). check the model name and the provider URL. the provider said: \"no such model\""
    : String}

law refusal_without_words:
  {Pv.refusal("Mistral", 400, "", "")
    == "Mistral refused the request (400)." : String}

# LAW (big-request): a refusal for size says so, in the words the Core
# reads to compact: the foundry proxy's 400, a 413; another 400 does not.
law refusal_for_size:
  {Pv.refusal("Anthropic", 400,
      "{\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"Request content length exceeded 32 MB limit.\"}}", "")
    == "Anthropic refused the request (400). the request is too large. Anthropic said: \"Request content length exceeded 32 MB limit.\""
    : String}

law size_refused_cases:
  {Con{W.size_refused(400, "Request content length exceeded 32 MB limit."),
    Con{W.size_refused(413, ""),
      Con{W.size_refused(400, "{\"error\":{\"type\":\"request_too_large\"}}"),
        Con{W.size_refused(400, "invalid base64 data"),
          Con{W.size_refused(500, "request too large"), Nil{}}}}}}
    == Con{True{}, Con{True{}, Con{True{}, Con{False{}, Con{False{}, Nil{}}}}}}
    : List<&2, Bool>}

# LAW: the attempt the Core reads: a refusal in those words, a reply's
# calls named back, a transport failure (status 0) as it was.
law attempt_for_names_the_calls:
  {Pv.attempt_for([W.WTool{"a.b", ""}], "OpenAI", "", 200, "",
      Pv.AOk{"OK \nCALL 1 a_b : x\nEND\n"})
    == Pv.AOk{"OK \nCALL 1 a.b : x\nEND\n"} : Pv.Attempt}

law attempt_for_words_a_refusal:
  {Pv.attempt_for(Nil{}, "OpenAI", "", 400, "{\"error\":{\"message\":\"bad\"}}",
      Pv.AFail{"provider 400: bad"})
    == Pv.AFail{"OpenAI refused the request (400). OpenAI said: \"bad\""}
    : Pv.Attempt}

law attempt_for_keeps_a_transport_failure:
  {Pv.attempt_for(Nil{}, "OpenAI", "", 0, "", Pv.AFail{"no route to api.openai.com"})
    == Pv.AFail{"no route to api.openai.com"} : Pv.Attempt}

# ---- the config file (core/config.bend) ----
# A pragmatic TOML subset: key = value lines, [section] headers, #
# comments. Precedence everywhere: env var > config > default.

# LAW: flat keys parse byte-for-byte - strings quoted, numbers bare.
law config_parse_flat:
  {Cf.parse_toml("model = \"opus-5.5\"\ncompaction_threshold = 800000\n")
    == Con{Cf.KV{"model", "opus-5.5"},
      Con{Cf.KV{"compaction_threshold", "800000"},
      Nil{}}} : List<&2, Cf.KV>}

# LAW: comments and blank lines never reach the pairs.
law config_skips_comments_blanks:
  {Cf.parse_toml("# a comment\n\n  \nmodel = \"m\"\n")
    == Con{Cf.KV{"model", "m"}, Nil{}} : List<&2, Cf.KV>}

# LAW: a key inside a section carries the section prefix.
law config_sections_prefix:
  {Cf.parse_toml("[harness]\nbg_after = 5\n")
    == Con{Cf.KV{"harness.bg_after", "5"}, Nil{}} : List<&2, Cf.KV>}

# LAW: a quoted value may carry '=' itself (the pair splits on the
# FIRST one only).
law config_value_keeps_equals:
  {Cf.parse_toml("cmd = \"a=b\"\n")
    == Con{Cf.KV{"cmd", "a=b"}, Nil{}} : List<&2, Cf.KV>}

# LAW: the lookups answer the exact key - Some when present, the
# default when not.
law config_lookup_found:
  {Cf.cfg_get("model = \"opus-5.5\"\n", "model") == Some{"opus-5.5"}
    : Maybe<&2, String>}

law config_lookup_missing:
  {Cf.cfg_or(Cf.cfg_get("model = \"m\"\n", "compaction_threshold"), "800000")
    == "800000" : String}

# LAW: numbers ride as U32.
law config_u32_parses:
  {Cf.cfg_u32("bg_after = 45\n", "bg_after", 30) == 45 : U32}

law config_u32_defaults:
  {Cf.cfg_u32("model = \"m\"\n", "bg_after", 30) == 30 : U32}

# LAW: the precedence - the environment variable wins, then the
# config file, then the default.
law config_env_wins:
  {Cf.env_cfg_or("zai-glm-5-3",
      Cf.cfg_get("model = \"opus-5.5\"\n", "model"), "claude-opus-5-5")
    == "zai-glm-5-3" : String}

law config_env_empty_falls_to_file:
  {Cf.env_cfg_or("",
      Cf.cfg_get("model = \"opus-5.5\"\n", "model"), "claude-opus-5-5")
    == "opus-5.5" : String}

law config_env_empty_no_file:
  {Cf.env_cfg_or("", Cf.cfg_get("", "model"), "claude-opus-5-5")
    == "claude-opus-5-5" : String}

# LAW: the template the runtime writes parses - its own keys are
# what the lookups find.
law config_template_parses:
  {Cf.cfg_get(St.CONFIG_TEMPLATE(), "model") == None{}
    : Maybe<&2, String>}

law config_template_threshold:
  {Cf.cfg_u32(St.CONFIG_TEMPLATE(), "compaction_threshold",
      800000) == 800000
    : U32}

# ---- the harness-info line: the single source of truth for clients ----
# The TUI (through the bend-harness parent) and bend_client.py read
# this line instead of recomputing the model, the threshold or the
# side-channel paths. Its format is a contract with both parsers.
law info_line_format:
  {Rt.info_line("claude-opus-5-5", "800000", "/tmp/bend-steer-7.txt",
      "/tmp/bend-interrupt-7.txt")
    == "harness-info model=claude-opus-5-5 threshold=800000 steer=/tmp/bend-steer-7.txt interrupt=/tmp/bend-interrupt-7.txt"
    : String}

# LAW: the announced side-channel paths are the ones the runtime
# drains - the same builders, per port.
law info_paths_are_the_drained_ones:
  {Bool.and(String.eq(Rt.steer_path_of("/r", "7"), "/r/bend-steer-7.txt"),
     String.eq(Pv.interrupt_path_of("/r", "7"), "/r/bend-interrupt-7.txt"))
    == True{} : Bool}

# ---- --resume: the restored history replays as live wire lines ----

# LAW: a restored history replays as the lines a live turn prints, each
# prefixed "history ": the compaction summary (its preamble skipped), the
# user message, each tool call with its code and its result (paired in
# call order, no id in the result), the assistant text on ONE wire line
# (a real newline escaped as backslash-n, chars 92 110, as live replies
# arrive); a call left without a result (interrupt, crash) replays as
# failed.
law resume_replays_history:
  {Rt.replay(Con{T.injected_user("The earlier conversation was compacted. A summary replaces it; continue from the preserved user messages."),
      Con{T.injected_user("Summary of the earlier conversation:
<summary>court</summary>"),
      Con{T.user("salut"),
      Con{T.asst("", Con{T.Call{7, "bash", "ls -la"}, Nil{}}),
      Con{T.tool_res("tool bash failed: boom"),
      Con{T.asst("<think>hmm</think>
fini", Nil{}),
      Con{T.asst("", Con{T.Call{9, "bash", "pwd"}, Nil{}}),
      Nil{}}}}}}}})
    == Con{"history obs compaction_done :: {\"summary\":\"court\"}",
      Con{"history you : salut",
      Con{"history obs tool_started :: {\"call\":7}",
      Con{"history tool #7 bash : ls -la",
      Con{"history tool_code #7 : ls -la",
      Con{"history tool_result #7 fail : boom",
      Con{"history obs tool_finished :: {\"call\":7,\"ok\":false}",
      Con{String.append("history obs assistant :: {\"text\":\"<think>hmm</think>",
        SCon{Chr{92}, SCon{Chr{92}, SCon{Chr{110}, "fini\"}"}}}),
      Con{"history obs tool_started :: {\"call\":9}",
      Con{"history tool #9 bash : pwd",
      Con{"history tool_code #9 : pwd",
      Con{"history obs tool_finished :: {\"call\":9,\"ok\":false}",
      Nil{}}}}}}}}}}}}}
    : List<&2, String>}

# ---- the Anthropic stream (core/anth-stream.bend) ----

# LAW: a streamed request is the same body with "stream": true first.
law anth_stream_body:
  {As.stream_body("{\"model\":\"m\"}") == "{\"stream\":true,\"model\":\"m\"}" : String}

# LAW: text and tool input stream in pieces; the reply is rebuilt whole -
# pings, padding and the empty first input piece change nothing.
law anth_sse_tool_call:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}      \n\nevent: ping\ndata: {\"type\": \"ping\"}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"I'll list\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" /tmp.\"}}   \n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"bash\",\"input\":{},\"caller\":{\"type\":\"direct\"}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"arg\\\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\": \\\"ls -la /tmp\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\" | head\\\"}\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":72}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n")
    == "OK I'll list /tmp.\nCALL 1 bash : ls -la /tmp | head\nEND\n" : String}

# LAW: a streamed reply maps to exactly what the same message sent whole
# maps to - signed thinking included (the signature streams as its own
# delta and must survive for the replay).
law anth_sse_matches_whole_reply:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"2+2\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\" = 4\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"4\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n")
    == Api.reply_of_for(Api.Anthropic{}, 200, "{\"type\":\"message\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"2+2 = 4\",\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}],\"stop_reason\":\"end_turn\"}") : String}

# LAW: an error event mid-stream is the reply: the 200 error body the
# retry policy already retries.
law anth_sse_error_event:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: ping\ndata: {\"type\":\"ping\"}\n\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n")
    == "ERROR provider 200: Overloaded\nEND\n" : String}

# LAW: a streamed reply prints the usage a whole reply would: the input
# side from message_start, output_tokens from the last message_delta.
law anth_sse_usage_line:
  {Us.usage_line(Api.Anthropic{}, "m", 200, "event: message_start
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"content\":[],\"usage\":{\"input_tokens\":10,\"cache_creation_input_tokens\":2,\"cache_read_input_tokens\":30,\"output_tokens\":1}}}

event: message_delta
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":72}}

")
    == "obs usage :: {\"cache_read\":30,\"cache_write\":2,\"input\":42,\"model\":\"m\",\"output\":72}" : String}

# LAW: a stream with no message_start (an error event) prints no usage.
law anth_sse_no_usage:
  {Us.usage_line(Api.Anthropic{}, "m", 200, "event: error
data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}

") == "" : String}

# LAW: a streamed refusal surfaces like a whole one.
law anth_sse_refusal:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"refusal\",\"stop_details\":{\"type\":\"refusal\",\"explanation\":\"blocked\"}}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n")
    == "ERROR provider refusal: blocked\nEND\n" : String}

# LAW: a 200 that is a whole JSON error body (the proxy's overload
# answer, even to a streamed request) still maps to the provider error
# the retry policy retries.
law anth_stream_json_error_body:
  {As.reply_any("{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}")
    == "ERROR provider 200: Overloaded\nEND\n" : String}

# ---- the OpenAI Chat stream (core/oai-stream.bend, BISE-144) ----

# LAW: a streamed OpenAI Chat request is the same body with "stream":
# true and the usage asked for (stream_options.include_usage) first.
law oai_stream_body:
  {Os.stream_body("{\"model\":\"m\"}")
    == "{\"stream\":true,\"stream_options\":{\"include_usage\":true},\"model\":\"m\"}" : String}

# LAW: text split over many chunks is rebuilt whole: the streamed reply
# maps to what the same message sent whole maps to (reasoning_content
# rides along as the whole message carries it).
law oai_stream_text_split:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":null,\"reasoning_content\":\"Think\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo wor\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ld\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":2}}}

data: [DONE]

")
    == Oai.reply_of(200, "{\"object\":\"chat.completion\",\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":\"Hello world\",\"reasoning_content\":\"Think\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":2}}}") : String}

law oai_stream_text_split_is:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":null,\"reasoning_content\":\"Think\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo wor\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ld\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":2}}}

data: [DONE]

")
    == "OK Hello world
END
" : String}

# LAW: tool_calls deltas are gathered by index, even interleaved: the
# first piece of an index has id + name, the next ones argument text.
law oai_stream_interleaved_tools:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"a\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"b\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\"\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"arg\\\": \\\"l\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"function\":{\"arguments\":\": \\\"pwd\\\"}\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"s\\\"}\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}

data: [DONE]

")
    == "OK 
CALL 1 bash : ls
CALL 2 bash : pwd
END
" : String}

# LAW: the usage comes in the last chunk (choices []): the usage line a
# whole reply prints (prompt_tokens counts the cached ones).
law oai_stream_usage_line:
  {Us.usage_line(Api.OpenAI{}, "m", 200, "data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":null,\"reasoning_content\":\"Think\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo wor\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ld\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":2}}}

data: [DONE]

")
    == "obs usage :: {\"cache_read\":2,\"cache_write\":0,\"input\":10,\"model\":\"m\",\"output\":5}" : String}

# LAW: Mistral sends a whole tool call in one delta, in the chunk that
# also carries finish_reason and the usage.
law oai_stream_mistral_one_delta:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"p\":\"abc\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"wBvFpU1Qf\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\": \\\"echo ok\\\"}\"},\"index\":0}]},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":115,\"total_tokens\":133,\"completion_tokens\":18,\"prompt_tokens_details\":{\"cached_tokens\":0}}}

data: [DONE]

")
    == "OK 
CALL 1 bash : echo ok
END
" : String}

law oai_stream_mistral_usage:
  {Us.usage_line(Api.OpenAI{}, "m", 200, "data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"p\":\"abc\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"wBvFpU1Qf\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\": \\\"echo ok\\\"}\"},\"index\":0}]},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":115,\"total_tokens\":133,\"completion_tokens\":18,\"prompt_tokens_details\":{\"cached_tokens\":0}}}

data: [DONE]

")
    == "obs usage :: {\"cache_read\":0,\"cache_write\":0,\"input\":115,\"model\":\"m\",\"output\":18}" : String}

# LAW: Mistral's reasoning models stream content as blocks (thinking,
# text): rebuilt as the whole reply's blocks, pieces joined.
law oai_stream_thinking_blocks:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2\"}]}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\" = 4\"}]}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":[{\"type\":\"text\",\"text\":\"4\"}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"!\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}

data: [DONE]

")
    == Oai.reply_of(200, "{\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}]},{\"type\":\"text\",\"text\":\"4!\"}]},\"finish_reason\":\"stop\"}]}") : String}

# LAW: an error chunk mid-stream (OpenRouter) is the reply: a 200 error
# body, which the retry policy retries (its message only, BISE-293).
law oai_stream_error_chunk:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"error\":{\"code\":\"server_error\",\"message\":\"Internal server error\"},\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\"},\"finish_reason\":\"error\"}]}

")
    == "ERROR provider 200: Internal server error
END
" : String}

# LAW: a 200 that is one JSON body (a server ignoring "stream") maps as
# a whole reply.
law oai_stream_json_body:
  {Api.stream_reply(Api.OpenAI{}, "{\"choices\":[{\"message\":{\"content\":\"hi\"}}]}")
    == Oai.reply_of(200, "{\"choices\":[{\"message\":{\"content\":\"hi\"}}]}") : String}

# LAW: the catalog drives the body (BISE-145): a model that does not
# reason gets no reasoning_effort; its max_output is the cap, named
# max_completion_tokens on OpenAI, max_tokens elsewhere.
law oai_body_no_reasoning_cap:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.MFacts{False{}, 4096n, "max_tokens", "", "", "", False{}},
      W.parse_wire("MODEL default
MSG user : hi
END
"))
    == "{\"model\":\"m\",\"max_tokens\":4096,\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}" : String}

law oai_body_openai_cap:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.MFacts{True{}, 128000n, "max_completion_tokens", "", "", "high", False{}},
      W.parse_wire("MODEL default
MSG user : hi
END
"))
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"max_completion_tokens\":128000,\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}" : String}

# LAW: the facts come from the models file, a model's key over its
# provider's; the OpenAI provider names its cap max_completion_tokens.
# (a small max_output: a Nat literal past 256n is too deep to check)
law provider_facts_from_file:
  {Pv.resolve("", "", "", "", "model = \"openai/gpt-4.1\"",
      "[providers.openai]
api = \"openai-chat\"
base_url = \"https://api.openai.com/v1\"
key_env = \"OPENAI_API_KEY\"
max_output = 128000
reasoning = true
[models.\"openai/gpt-4.1\"]
max_output = 200
reasoning = false",
      "")
    == Pv.ROk{Pv.Prov{"openai", "gpt-4.1", Api.OpenAI{},
      "https://api.openai.com/v1/chat/completions", "OPENAI_API_KEY",
      Api.MFacts{False{}, 200n, "max_completion_tokens", "", "", "", False{}}}}
    : Pv.Resolved}

# LAW: only a non-empty flag is a Ctrl+C - the runtime clears the flag to
# "" at every turn end, and that must never read as an interrupt.
law interrupt_flag_semantics:
  {Bool.and(Pv.interrupt_requested("1"),
      Bool.and(Bool.not(Pv.interrupt_requested("")),
        Bool.not(Pv.interrupt_requested("\n"))))
    == True{} : Bool}

# LAW: a streamed read waits 90 s at most (events come every few seconds).
law provider_stream_step_timeout:
  {Pv.stream_step_ms() == 90000 : U32}

# LAW (c8-gateway-hang gap_timeout_unchanged): the gap of a body read is
# the wait of before the head limit existed: 90 s with no
# idle_timeout_sec (or 0, or not a number), as before the key existed:
# nothing changes for who sets nothing. Retries keep it
# (gap_timeout_unchanged_per_try).
law gap_timeout_unchanged:
  {Bool.and(U32.is_eq(Pv.gap_ms(Pv.stream_wait("", "fake", "fake/m")), 90000),
      Bool.and(U32.is_eq(Pv.gap_ms(Pv.stream_wait(
          "[providers.fake]\nidle_timeout_sec = 0\n", "fake", "fake/m")), 90000),
        U32.is_eq(Pv.gap_ms(Pv.stream_wait(
          "[providers.fake]\nidle_timeout_sec = \"soon\"\n", "fake", "fake/m")), 90000)))
    == True{} : Bool}

# LAW: idle_timeout_sec under [providers.<id>] sets the wait of its
# models, a model's own table wins, another provider keeps 90 s, and a
# huge value is capped at a day.
law provider_idle_timeout_set:
  {Bool.and(U32.is_eq(Pv.gap_ms(Pv.stream_wait(
        "[providers.fake]\nidle_timeout_sec = 600\n", "fake", "fake/m")), 600000),
      Bool.and(U32.is_eq(Pv.gap_ms(Pv.stream_wait(
          "[providers.fake]\nidle_timeout_sec = 600\n[models.\"fake/m\"]\nidle_timeout_sec = 5\n",
          "fake", "fake/m")), 5000),
        Bool.and(U32.is_eq(Pv.gap_ms(Pv.stream_wait(
            "[providers.fake]\nidle_timeout_sec = 600\n", "mistral", "mistral/m")), 90000),
          U32.is_eq(Pv.gap_ms(Pv.stream_wait(
            "[providers.fake]\nidle_timeout_sec = 99999999\n", "fake", "fake/m")), 86400000))))
    == True{} : Bool}

# LAW (c8-gateway-hang): the gap stays 90 s on every retry.
law gap_timeout_unchanged_per_try:
  {Bool.and(U32.is_eq(Pv.gap_ms(Pv.wait_for_try(
        Pv.stream_wait("", "fake", "fake/m"), 0n)), 90000),
      U32.is_eq(Pv.gap_ms(Pv.wait_for_try(
        Pv.stream_wait("", "fake", "fake/m"), 7n)), 90000))
    == True{} : Bool}

# LAW (c8-gateway-hang head_grows_per_attempt): with no idle_timeout_sec
# the first attempt waits head_ms (30 s) for the response head, a retry
# doubles it: 30 s, 60 s, then the gap.
law head_grows_per_attempt:
  {Bool.and(U32.is_eq(Pv.head_ms(), 30000),
      Bool.and(U32.is_eq(Pv.head_of(Pv.wait_for_try(
          Pv.stream_wait("", "fake", "fake/m"), 0n)), 30000),
        Bool.and(U32.is_eq(Pv.head_of(Pv.wait_for_try(
            Pv.stream_wait("", "fake", "fake/m"), 1n)), 60000),
          U32.is_eq(Pv.head_of(Pv.wait_for_try(
            Pv.stream_wait("", "fake", "fake/m"), 2n)), 90000))))
    == True{} : Bool}

# LAW (c8-gateway-hang head_capped_at_gap): the head never waits past the
# gap, however many retries (a slow but alive head gets today's 90 s
# from the third attempt on: no turn lost that the old limit saved), and
# a day-long wait never overflows.
law head_capped_at_gap:
  {Bool.and(U32.is_eq(Pv.head_of(Pv.wait_for_try(
        Pv.stream_wait("", "fake", "fake/m"), 9n)), 90000),
      Bool.and(U32.is_eq(Pv.head_of(Pv.wait_for_try(
          Pv.ProvWait{"x", 30000, 20000}, 0n)), 20000),
        U32.is_eq(Pv.head_of(Pv.wait_for_try(
          Pv.ProvWait{"x", 86400000, 86400000}, 9n)), 86400000)))
    == True{} : Bool}

# LAW (c8-gateway-hang): a provider with idle_timeout_sec keeps head ==
# gap, its own value, on every attempt (a local model reading a long
# prompt before its head is not cut at 30 s).
law idle_timeout_head_is_gap:
  {Bool.and(U32.is_eq(Pv.head_of(Pv.wait_for_try(
        Pv.stream_wait.of("fake", 600), 0n)), 600000),
      Bool.and(U32.is_eq(Pv.head_of(Pv.wait_for_try(
          Pv.stream_wait.of("fake", 600), 3n)), 600000),
        U32.is_eq(Pv.head_of(Pv.wait_for_try(
          Pv.stream_wait.of("fake", 5), 0n)), 5000)))
    == True{} : Bool}

# LAW: a streamed call that times out still retries, and its text names
# the host, the limit, which one ran out and the key to raise, under its
# provider's table; the attempt carries the limit and its ms, typed.
law attempt_timeout_names_the_key:
  {Pv.attempt_err_in("127.0.0.1:8080", Pv.ProvWait{"ollama", 3000, 5000},
      Pv.LHead{}, Pv.NTimeout{})
    == Pv.ATimeout{Pv.LHead{}, 3000, "no answer from 127.0.0.1:8080 in 3 s (timeout, no response head) — a slow model or gateway? raise idle_timeout_sec under [providers.ollama] in ~/.bise/config.toml"}
    : Pv.Attempt}

law attempt_timeout_names_the_gap:
  {Pv.attempt_err_in("127.0.0.1:8080", Pv.ProvWait{"ollama", 3000, 5000},
      Pv.LGap{}, Pv.NTimeout{})
    == Pv.ATimeout{Pv.LGap{}, 5000, "no answer from 127.0.0.1:8080 in 5 s (timeout, the reply stalled) — a slow model or gateway? raise idle_timeout_sec under [providers.ollama] in ~/.bise/config.toml"}
    : Pv.Attempt}

# LAW: the other transport failures of a streamed call read as before.
law attempt_err_in_others:
  {Pv.attempt_err_in("api.example.com", Pv.ProvWait{"x", 5000, 5000},
      Pv.LHead{}, Pv.NDns{"no address", ""})
    == Pv.attempt_err("api.example.com", Pv.NDns{"no address", ""}) : Pv.Attempt}

# LAW (c8-gateway-hang): a head timeout spends a status attempt with the
# growing backoff (1 s, 2 s, 4 s...), as any retry without a server
# hint: a shorter head limit never hammers a gateway in an episode.
law head_timeout_backoff_grows:
  {[Pv.next_of(Pv.ATimeout{Pv.LHead{}, 30000, "w"}, Pv.Budget{9n, 0}, 0n),
      Pv.next_of(Pv.ATimeout{Pv.LHead{}, 60000, "w"}, Pv.Budget{8n, 0}, 1n),
      Pv.next_of(Pv.ATimeout{Pv.LHead{}, 90000, "w"}, Pv.Budget{7n, 0}, 2n)]
    == [Pv.XRetry{1000, "w", Pv.Budget{8n, 0}}, Pv.XRetry{2000, "w", Pv.Budget{7n, 0}},
      Pv.XRetry{4000, "w", Pv.Budget{6n, 0}}] : List<&2, Pv.Next>}

# LAW (c8-gateway-hang): a timeout's request_failed event says kind
# "timeout", the limit and its ms, typed (scripts/call_times.py counts
# head hangs without reading the message); any other retry keeps
# "other" and no limit.
law retry_event_types_the_timeout:
  {Ev.retry(4, 1n, Pv.retry_kind(Pv.ATimeout{Pv.LHead{}, 30000, "w"}),
      Pv.retry_limit(Pv.ATimeout{Pv.LHead{}, 30000, "w"}),
      Pv.retry_waited(Pv.ATimeout{Pv.LHead{}, 30000, "w"}), "w", 1000)
    == "  ev: {\"type\":\"request_failed\",\"v\":1,\"data\":{\"req\":4,\"attempt\":1,\"error\":{\"kind\":\"timeout\",\"message\":\"w\"},\"retry_in_ms\":1000,\"limit\":\"head\",\"waited_ms\":30000}}"
    : String}

law retry_event_other_unchanged:
  {Ev.retry(4, 2n, Pv.retry_kind(Pv.ARetry{0, "x"}),
      Pv.retry_limit(Pv.ARetry{0, "x"}), Pv.retry_waited(Pv.ARetry{0, "x"}),
      "x", 2000)
    == "  ev: {\"type\":\"request_failed\",\"v\":1,\"data\":{\"req\":4,\"attempt\":2,\"error\":{\"kind\":\"other\",\"message\":\"x\"},\"retry_in_ms\":2000}}"
    : String}

# LAW: a debug event is one JSON object on one line: ts, src, kind, then
# the fields in order; strings are JSON-escaped (a newline stays \\n).
law debug_event_line:
  {Dg.ev_line(7n, "a\"b", Con{Dg.FStr{"k", "x\ny"}, Con{Dg.FNat{"n", 3n}, Nil{}}})
    == "{\"ts\":7,\"src\":\"repl\",\"kind\":\"a\\\"b\",\"k\":\"x\\ny\",\"n\":3}" : String}

# LAW: only obs and tool annotations become breadcrumbs, one line each.
law debug_crumbs_filter:
  {Dg.crumbs(Con{"  obs: turn_started", Con{"hello", Con{"tool #3 bash : ls", Nil{}}}}, 5n)
    == "{\"ts\":5,\"src\":\"repl\",\"kind\":\"sent\",\"line\":\"obs: turn_started\"}\n{\"ts\":5,\"src\":\"repl\",\"kind\":\"sent\",\"line\":\"tool #3 bash : ls\"}\n" : String}

# LAW: a record obs line (issue 38) is a breadcrumb too.
law debug_crumbs_record:
  {Dg.crumbs(Con{"obs turn_started :: {}", Nil{}}, 5n)
    == "{\"ts\":5,\"src\":\"repl\",\"kind\":\"sent\",\"line\":\"obs turn_started :: {}\"}
" : String}

# LAW: a first attempt that works is not logged; a later one that works
# is a recovery; a failure, a retry or a 200 carrying an ERROR reply is
# an incident.
law debug_plan_ok_first:
  {Dg.plan(Pv.AOk{"OK\nEND\n"}, 0n) == Dg.PSkip{} : Dg.Plan}

law debug_plan_recovered:
  {Dg.plan(Pv.AOk{"OK\nEND\n"}, 2n) == Dg.PRecovered{} : Dg.Plan}

law debug_plan_error_reply:
  {Dg.plan(Pv.AOk{"ERROR provider refusal: x"}, 0n)
    == Dg.PFailed{"error_reply", "ERROR provider refusal: x", 0} : Dg.Plan}

law debug_plan_retry:
  {Dg.plan(Pv.ARetry{5000, "provider 529 (transient)"}, 1n)
    == Dg.PFailed{"retry", "provider 529 (transient)", 5000} : Dg.Plan}

# LAW: an artifact is cut at the limit, and says how long it was.
law debug_clip:
  {Bool.and(String.eq(Dg.clip("abcdef", 3n), "abc\n[... truncated: 6 chars in total]"),
      String.eq(Dg.clip("abc", 3n), "abc"))
    == True{} : Bool}

# LAW: the reply artifact names the attempt 1-based, like the retry notes.
law debug_reply_name:
  {Dg.reply_name(12n, 0n) == "reply-12-1.txt" : String}

# LAW: the clock probe answers "<status>\n<epoch ms>": the offset is
# epoch - mono; an unreadable answer gives no offset (0).
law debug_clock_anchor:
  {Dg.anchor(Con{"0", Con{"1500", Nil{}}}, 500n) == 1000n : Nat}

law debug_clock_anchor_failed:
  {Dg.anchor(Con{"127", Con{"sh: perl: not found", Nil{}}}, 500n) == 0n : Nat}

# LAW: an event is stamped mono + the stored offset, mono without one.
law debug_stamp:
  {Bool.and(Nat.is_eq(Dg.stamp(Some{"1000\n"}, 5n), 1005n),
      Nat.is_eq(Dg.stamp(None{}, 5n), 5n))
    == True{} : Bool}

# ---- switchboard hooks (BEND_WORKDIR, BEND_CONTEXT_FILE, BEND_EXTRA_PROMPT) ----

# LAW: without BEND_WORKDIR the command is unchanged.
law workdir_empty_keeps_command:
  {Sh.with_workdir("", "ls -la") == "ls -la" : String}

# LAW: with BEND_WORKDIR the command first enters the (quoted) directory,
# and fails instead of running elsewhere when it cannot.
law workdir_cds_first:
  {Sh.with_workdir("/tmp/a b", "ls") == "cd '/tmp/a b' || exit 1\nls" : String}

# LAW: an empty context changes nothing.
law context_empty_keeps_request:
  {Rm.with_context("", "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nEND\n")
    == "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nEND\n" : String}

# LAW: a context becomes the LAST message of an agent request (role
# `context`: a user message on the wire's families, never a cache
# breakpoint, BISE-268), newlines escaped like every wire message.
law context_is_last_user_message:
  {Rm.with_context("board\nline 2", "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nEND\n")
    == "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nMSG context : board\\nline 2\nEND\n" : String}

# LAW: a compaction request (no tool catalog) never carries the context.
law context_skips_compaction:
  {Rm.with_context("board", "MODEL default\nMSG user : hi\n\nEND\n")
    == "MODEL default\nMSG user : hi\n\nEND\n" : String}

# LAW: nor does the first attempt of a compaction, which carries the
# tools to share the agent calls' prefix (BISE-268): its COMPACT line.
law context_skips_compaction_with_tools:
  {Rm.with_context("board", "MODEL default\nCOMPACT\nTOOL bash : run\n\nMSG user : hi\n\nEND\n")
    == "MODEL default\nCOMPACT\nTOOL bash : run\n\nMSG user : hi\n\nEND\n" : String}

# LAW: the extra prompt ends the system prompt; an empty one adds nothing.
law extra_prompt_appends:
  {Rp.prompt_extra("base", "role") == "base\n\nrole" : String}

law extra_prompt_empty_keeps:
  {Rp.prompt_extra("base", "  ") == "base" : String}

# LAW: a session without a role (solo) gets the tone block; one with a
# role does not (the role carries the same block): never twice.
law solo_tone_without_role:
  {Rp.solo_tone("  ", "tone") == "tone" : String}

law solo_tone_not_with_role:
  {Rp.solo_tone("role", "tone") == "" : String}

# LAW: a drained steering line carries escaped newlines back as real ones.
law steer_line_unescapes:
  {Rt.steer_cmd("a\\nb") == T.UserMsg{T.Steer{}, "a\nb"} : T.Cmd}

# LAW (c2-turn-survives d): a call's usage event carries its times: the
# body's build, the checkpoint, the response's head after the send, the
# attempt's whole time and which attempt; no head: ttfb 0.
law usage_carries_timing:
  {Us.usage_ev(Api.OpenAI{}, "m", 200,
      "{\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}", 7,
      Us.timing(1000n, 1040n, 1900n, 2500n, 12n, 1n))
    == "  ev: {\"type\":\"usage\",\"v\":1,\"data\":{\"req\":7,\"model\":\"m\",\"input\":10,\"output\":2,\"cache_read\":0,\"cache_write\":0,\"build_ms\":40,\"save_ms\":12,\"ttfb_ms\":860,\"total_ms\":1500,\"attempt\":2}}"
    : String}

law timing_without_head:
  {Us.timing(1000n, 1040n, 0n, 2500n, 12n, 0n)
    == Ev.Timing{40n, 12n, 0n, 1500n, 1n} : Ev.Timing}
