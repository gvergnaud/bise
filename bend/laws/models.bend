# bend/laws/models.bend: images, Anthropic per model, model and effort, paired calls, the prompt cache, gateway headers.
# Open claims, each one closed by bend/proofs/models.bend (moved from
# LAWS.bend). The gate checks every bend/proofs/*.bend: a law here
# with no proof there fails it.
import Base
import ../core/types.bend as T
import ../core/history.bend as H
import ../core/session.bend as S
import ../core/api.bend as Api
import ../core/wire.bend as W
import ../core/oai-chat.bend as Oai
import ../core/anthropic.bend as Anth
import ../core/image.bend as Im
import ../core/text.bend as Tx
import ../runtime/remote.bend as Rm
import ../runtime/main-pure.bend as Rt
import ../runtime/provider-pure.bend as Pv

# ---- images (docs/images.md, core/image.bend) ----

# LAW: a text splits into its text parts and its image markers, in order.
law image_split_marker:
  {Im.img_split("a <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\"> b")
    == Con{Im.IText{"a "}, Con{Im.IImg{"[Image #1]", "p.png", "image/png", "/s/x.b64", ""},
      Con{Im.IText{" b"}, Nil{}}}}
    : List<&2, Im.IPart>}

# LAW: a caption, the marker's optional last attribute (architect
# m_22000), is read apart from the b64 file.
law image_split_caption:
  {Im.img_split("<image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\" caption=\"Safari · bench\">")
    == Con{Im.IImg{"[Image #1]", "p.png", "image/png", "/s/x.b64", "Safari · bench"}, Nil{}}
    : List<&2, Im.IPart>}

# LAW: the model's frame of a captioned image says its caption, after
# its name; an image without one keeps the frame it had.
law image_frame_caption:
  {Im.img_open_text("[Image #1]", "Safari · bench", "p.png")
    == "<image name=[Image #1] caption=\"Safari · bench\" path=\"p.png\">" : String}

# LAW: a marker that does not name a .b64 file is no image.
law image_split_refuses_bad_marker:
  {Im.img_paths("<image name=\"x\" path=\"p\" mime=\"image/png\" b64=\"/etc/passwd\">")
    == Nil{} : List<&2, String>}

# LAW: OpenAI style - a user message with a marker becomes content
# parts, the Codex frame around a data-url placeholder.
law api_body_image_user_parts:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(), W.parse_wire("MODEL default
MSG user : see <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\">
END
"))
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"see \"},{\"type\":\"text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},\"@@BENDPART:o|image/png|/s/x.b64|[Image #1]@@\",{\"type\":\"text\",\"text\":\"</image>\"}]}]}"
    : String}

# LAW: Anthropic - a tool result with a marker carries image blocks.
law api_body_image_anth_tool_result:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(), W.parse_wire("MODEL default
MSG user : go
MSG assistant : 
  CALL 1 run_typescript : {}
MSG tool : tool run_typescript ok: <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\">
END
"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"go\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"run_typescript\",\"input\":{},\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_1\",\"content\":[{\"type\":\"text\",\"text\":\"tool run_typescript ok: \"},{\"type\":\"text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},\"@@BENDPART:a|image/png|/s/x.b64|[Image #1]@@\",{\"type\":\"text\",\"text\":\"</image>\"}],\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: only the last `cap` loaded images are kept.
law image_keep_last:
  {Im.img_keep(Con{True{}, Con{False{}, Con{True{}, Con{True{}, Nil{}}}}}, 2n)
    == Con{False{}, Con{False{}, Con{True{}, Con{True{}, Nil{}}}}}
    : List<&2, Bool>}

# LAW: a marker that is not kept becomes plain text; the others stay.
law image_defang_unkept:
  {Im.img_defang("<image name=\"a\" path=\"p\" mime=\"image/png\" b64=\"/s/1.b64\"> <image name=\"b\" path=\"q\" mime=\"image/png\" b64=\"/s/2.b64\">",
      Con{False{}, Con{True{}, Nil{}}})
    == "<image-unavailable name=\"a\" path=\"p\" mime=\"image/png\" b64=\"/s/1.b64\"> <image name=\"b\" path=\"q\" mime=\"image/png\" b64=\"/s/2.b64\">"
    : String}

# LAW: the splice puts each image's data at its placeholder (an unknown
# one becomes empty).
law image_splice_data:
  {Im.img_splice("{\"d\":\"@@BENDIMG:/s/1.b64@@\",\"e\":\"@@BENDIMG:/s/9.b64@@\"}",
      Con{Im.IL{"/s/1.b64", "QUJD"}, Nil{}})
    == "{\"d\":\"QUJD\",\"e\":\"\"}" : String}

# ---- Anthropic per model: prompt caching, thinking, max_tokens (BISE-146) ----

# LAW: prompt caching - four cache_control breakpoints, the API's
# maximum: the system block, the last tool, and the last block of each
# of the two newest messages; the older messages carry none.
law anth_cache_breakpoints:
  {Anth.api_body_anth_for("m", 0n, "", True{}, "high", W.parse_wire("MODEL default
MSG system : S
TOOL echo : Returns text.
TOOL bash : Runs.
MSG user : go
MSG assistant : x
  CALL 7 echo : hi
MSG tool : out1
END
"))
    == "{\"system\":[{\"type\":\"text\",\"text\":\"S\",\"cache_control\":{\"type\":\"ephemeral\"}}],\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"go\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"x\"},{\"type\":\"tool_use\",\"id\":\"call_7\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"},\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"out1\",\"cache_control\":{\"type\":\"ephemeral\"}}]}],\"tools\":[{\"name\":\"bash\",\"description\":\"Runs.\",\"input_schema\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"},\"description\":{\"type\":\"string\",\"description\":\"One short line shown to the user while this runs: what you are doing, in the user's language. 8 words at most, starting lowercase, no final period. E.g. 'weighing the hero image', 'je lance les tests du signup'.\"}},\"required\":[\"arg\"]}},{\"name\":\"echo\",\"description\":\"Returns text.\",\"input_schema\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"}},\"required\":[\"arg\"]},\"cache_control\":{\"type\":\"ephemeral\"}}],\"tool_choice\":{\"type\":\"auto\"}}"
    : String}

# LAW: a thinking block never carries a breakpoint (the API refuses
# it): a newest message ending in one gets none. And `thinking = none`
# sends no thinking field, even for a model that reasons; max_tokens is
# the catalog's max output.
law anth_thinking_none_and_no_cache_on_thinking:
  {Anth.api_body_anth_for("m", 64000n, "none", True{}, "high", W.parse_wire("MODEL default
MSG user : a
MSG assistant : <think>t\\nBENDSIG::s1</think>
MSG user : b
END
"))
    == "{\"model\":\"m\",\"max_tokens\":64000,\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"a\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"t\",\"signature\":\"s1\"}]},{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"b\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: no empty text block ever leaves (the Messages API refuses one:
# "text content blocks must be non-empty", main's 400 of 2026-10-30). An
# assistant turn that holds only an UNSIGNED think span (cut before its
# text and calls) replays as no message at all, never as content "" (the
# API reads "" as an empty text block); an empty or whitespace-only
# user message is dropped, an empty system prompt sends no system field,
# and an empty tool result omits its content.
law anth_no_empty_text_block:
  {Anth.api_body_anth_for("m", 64000n, "none", True{}, "high", W.parse_wire("MODEL default
MSG system : 
MSG user : a
MSG assistant : <think>t\\n</think>
MSG user : 
MSG user :  \\n 
MSG assistant : x
  CALL 7 echo : hi
MSG tool : 
MSG user : b
END
"))
    == "{\"model\":\"m\",\"max_tokens\":64000,\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"a\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"x\"},{\"type\":\"tool_use\",\"id\":\"call_7\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"b\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: the same for the OpenAI-style families (W.sendable): a blank
# system or user message and an assistant with no call and no text are
# left out (never content "" or an assistant with content null and no
# tool_calls); a blank tool result reads (no output); a user message of
# only an image keeps its frame text, never an empty text part.
def no_empty_req() -> W.WReq:
  W.parse_wire("MODEL default
MSG system : 
MSG user : a
MSG assistant : 
MSG user :  \\n 
MSG assistant :  \\n 
MSG assistant : x
  CALL 7 echo : hi
MSG tool : 
MSG user : <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\">
END
")

law oai_no_empty_content:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(), no_empty_req())
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"a\"},{\"role\":\"assistant\",\"content\":\"x\",\"tool_calls\":[{\"id\":\"call_7\",\"type\":\"function\",\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\":\\\"hi\\\"}\"}}]},{\"role\":\"tool\",\"content\":\"(no output)\",\"tool_call_id\":\"call_7\"},{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},\"@@BENDPART:o|image/png|/s/x.b64|[Image #1]@@\",{\"type\":\"text\",\"text\":\"</image>\"}]}]}"
    : String}

law resp_no_empty_content:
  {Api.api_body_for(Api.OpenAIResp{}, "m", Api.facts0(), no_empty_req())
    == "{\"model\":\"m\",\"store\":false,\"reasoning\":{\"effort\":\"high\",\"summary\":\"auto\"},\"include\":[\"reasoning.encrypted_content\"],\"input\":[{\"role\":\"user\",\"content\":\"a\"},{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"x\"}]},{\"type\":\"function_call\",\"call_id\":\"call_7\",\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\":\\\"hi\\\"}\"},{\"type\":\"function_call_output\",\"call_id\":\"call_7\",\"output\":\"(no output)\"},{\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},\"@@BENDPART:r|image/png|/s/x.b64|[Image #1]@@\",{\"type\":\"input_text\",\"text\":\"</image>\"}]}]}"
    : String}

law deepseek_no_empty_content:
  {Api.api_body_for(Api.DeepSeek{}, "m", Api.facts0(), no_empty_req())
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"a\"},{\"role\":\"assistant\",\"content\":\"x\",\"reasoning_content\":\"\",\"tool_calls\":[{\"id\":\"call_7\",\"type\":\"function\",\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\":\\\"hi\\\"}\"}}]},{\"role\":\"tool\",\"content\":\"(no output)\",\"tool_call_id\":\"call_7\"},{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},\"@@BENDPART:o|image/png|/s/x.b64|[Image #1]@@\",{\"type\":\"text\",\"text\":\"</image>\"}]}]}"
    : String}

law anth_image_only_user_has_no_empty_text:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(), no_empty_req())
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"a\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"x\"},{\"type\":\"tool_use\",\"id\":\"call_7\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},\"@@BENDPART:a|image/png|/s/x.b64|[Image #1]@@\",{\"type\":\"text\",\"text\":\"</image>\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: `thinking = budget` (Haiku 4.5 refuses adaptive thinking):
# extended thinking with budget_tokens = half the max output, at most
# 16000; no output_config.
law anth_thinking_budget:
  {Anth.api_body_anth_for("claude-haiku-4-5", 64000n, "budget", True{}, "high",
      W.parse_wire("MODEL default
MSG user : salut
END
"))
    == "{\"model\":\"claude-haiku-4-5\",\"max_tokens\":64000,\"thinking\":{\"type\":\"enabled\",\"budget_tokens\":16000},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"salut\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: a budget under the API's 1024 minimum: no thinking at all.
law anth_thinking_budget_too_small:
  {String.contains(Anth.api_body_anth_for("m", 1500n, "budget", True{}, "high",
      W.parse_wire("MODEL default
MSG user : salut
END
")), "thinking")
    == False{} : Bool}

# LAW: no thinking word: a model that does not reason thinks not; one
# that reasons gets adaptive thinking (the foundry body).
law anth_thinking_default_follows_reasoning:
  {Anth.api_body_anth_for("m", 8192n, "", False{}, "", W.parse_wire("MODEL default
MSG user : salut
END
"))
    == "{\"model\":\"m\",\"max_tokens\":8192,\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"salut\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

law anth_thinking_default_reasons_adaptive:
  {Anth.thinking_mode("", True{}) == Anth.TAdaptive{} : Anth.TMode}

law anth_thinking_words:
  {[Anth.thinking_mode("adaptive", False{}), Anth.thinking_mode("budget", False{}),
      Anth.thinking_mode("none", True{}), Anth.thinking_mode("high", False{})]
    == [Anth.TAdaptive{}, Anth.TBudget{}, Anth.TNone{}, Anth.TNone{}]
    : List<&2, Anth.TMode>}

# LAW: a model's catalog `betas` is the anthropic-beta header (the
# direct API: no 1M-context flag); unset, the foundry proxy's list
# (family_headers_anthropic).
law family_headers_anthropic_betas:
  {Api.headers(Api.Anthropic{}, "interleaved-thinking-2025-05-14", "k")
    == [Api.Hdr{"x-api-key", "k"},
      Api.Hdr{"anthropic-version", "2023-06-01"},
      Api.Hdr{"anthropic-beta", "interleaved-thinking-2025-05-14"},
      Api.Hdr{"Content-Type", "application/json"}] : List<&2, Api.Hdr>}

# LAW: the Anthropic family's body follows the model's catalog facts
# (the dispatch passes max output, thinking word and reasoning through).
law api_body_anth_follows_facts:
  {Api.api_body_for(Api.Anthropic{}, "m",
      Api.MFacts{False{}, 200n, "max_tokens", "", "", "", False{}},
      W.parse_wire("MODEL default
MSG user : salut
END
"))
    == Anth.api_body_anth_for("m", 200n, "", False{}, "", W.parse_wire("MODEL default
MSG user : salut
END
")) : String}

# ---- the model and the effort in use, and the session's choice (BISE-135) ----
# /model and /reasoning write the session's choice file
# (BISE_SESSION_CHOICE); Pv.resolve_chosen(choice, BISE_MODEL,
# BEND_MODEL, BISE_AGENT_MODEL, BISE_ROLE, config.toml, models file,
# BEND_PROVIDER_URL). The same order as rust/catalog's Setup::in_use.

# LAW: the session's model comes first, over every env var and config
# key, for main and for a sub-agent; an alias of the choice resolves.
law choice_model_first:
  {(Pv.shown_model(Pv.resolve_chosen("model = \"mistral/s\"", "mistral/a",
        "mistral/b", "mistral/e", "main", "model = \"mistral/c\"", "", "")),
      Pv.shown_model(Pv.resolve_chosen("model = \"mistral/s\"", "mistral/a",
        "", "mistral/e", "agent", "agent_model = \"mistral/d\"", "", "")),
      Pv.shown_model(Pv.resolve_chosen("model = \"opus-5.5\"", "mistral/a",
        "", "", "main", "", "", "")))
    == ("mistral/s", "mistral/s", "foundry/claude-opus-5-5")
    : String & String & String}

# LAW: no choice (or a choice of the effort alone): the role's order,
# as before.
law choice_empty_keeps_the_role_order:
  {(Pv.shown_model(Pv.resolve_chosen("", "", "", "mistral/e", "agent",
        "model = \"mistral/c\"", "", "")),
      Pv.shown_model(Pv.resolve_chosen("reasoning_effort = \"low\"", "", "",
        "", "main", "model = \"mistral/c\"", "", "")))
    == ("mistral/e", "mistral/c") : String & String}

# LAW: the effort order: the session's choice > agent_reasoning_effort
# (sub-agents) > reasoning_effort (config.toml) > the model's default
# (high, today's).
law effort_order:
  {(Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"max\"", "", "",
        "", "agent", "model = \"opus-5.5\"
reasoning_effort = \"low\"
agent_reasoning_effort = \"medium\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"
efforts = \"none,low,medium,high,xhigh,max\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "agent",
        "model = \"opus-5.5\"
reasoning_effort = \"low\"
agent_reasoning_effort = \"medium\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"
efforts = \"none,low,medium,high,xhigh,max\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "main",
        "model = \"opus-5.5\"
reasoning_effort = \"low\"
agent_reasoning_effort = \"medium\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"
efforts = \"none,low,medium,high,xhigh,max\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "main", "model = \"opus-5.5\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"
efforts = \"none,low,medium,high,xhigh,max\"", "")))
    == ("max", "medium", "low", "high") : String & String & String & String}

# LAW: an effort the model does not take falls to its default: max is
# an Anthropic word, not a chat one; a model's own list and default
# (efforts, effort keys of the models file) win over its family's.
law effort_outside_the_list_falls_to_the_default:
  {(Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"max\"", "",
        "zai-glm-5-3", "", "", "", "[providers.mistral]
efforts = \"none,low,medium,high\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"low\"", "",
        "", "", "", "model = \"work/m\"", "[providers.work]
base_url = \"http://x\"
reasoning = true
efforts = \"none,high\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "",
        "model = \"work/m\"", "[providers.work]
base_url = \"http://x\"
reasoning = true
efforts = \"minimal,low\"
effort = \"minimal\"", "")))
    == ("high", "high", "minimal") : String & String & String}

# LAW (effort-table): no models file, no effort list: the runtime keeps
# no list of its own (Rust owns them, rust/catalog/src/efforts.rs), so a
# model it only knows from builtin() gets none, whatever was asked, and
# the body's default goes.
law effort_without_models_file_is_none:
  {(Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"high\"", "",
        "zai-glm-5-3", "", "", "", "", "")),
      Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"xhigh\"", "",
        "", "", "", "model = \"opus-5.5\"", "", "")))
    == ("", "") : String & String}

# LAW: a model that does not reason (or an Anthropic one whose thinking
# is none) has no effort: none is sent, whatever was asked.
law effort_none_without_reasoning:
  {(Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"high\"", "", "",
        "", "", "model = \"work/m\"", "[providers.work]
base_url = \"http://x\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"high\"", "", "",
        "", "", "model = \"work/m\"", "[providers.work]
base_url = \"http://x\"
api = \"anthropic\"
reasoning = true
thinking = \"none\"", "")))
    == ("", "") : String & String}

# LAW: the window (so the compaction threshold) follows the session's
# model.
law window_follows_the_choice:
  {Pv.window_chosen("model = \"zai-glm-5-3\"", "", "", "", "", "", "")
    == 128000 : U32}

# LAW: the chat body sends the call's effort word as is.
law oai_body_sends_the_effort:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.MFacts{True{}, 0n, "max_tokens", "", "", "low", False{}},
      W.parse_wire("MODEL default
MSG user : hi
END
"))
    == "{\"model\":\"m\",\"reasoning_effort\":\"low\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}" : String}

# LAW: Anthropic adaptive thinking carries the effort in
# output_config; effort none turns thinking off (no thinking, no
# output_config).
law anth_adaptive_effort:
  {(String.contains(Anth.api_body_anth_for("m", 0n, "adaptive", True{}, "low",
        W.parse_wire("MODEL default
MSG user : salut
END
")), "\"output_config\":{\"effort\":\"low\"}"),
      String.contains(Anth.api_body_anth_for("m", 0n, "adaptive", True{}, "none",
        W.parse_wire("MODEL default
MSG user : salut
END
")), "thinking"))
    == (True{}, False{}) : Bool & Bool}

# LAW: a thinking budget follows the effort: low 4000, medium 8000,
# high 16000, max half the output cap (each at most half of it).
law anth_budget_follows_the_effort:
# (shown: a Nat compared past 256n is too deep to check)
  {[Nat.show(Anth.budget_for("low", 64000n)), Nat.show(Anth.budget_for("medium", 64000n)),
      Nat.show(Anth.budget_for("high", 64000n)), Nat.show(Anth.budget_for("max", 64000n)),
      Nat.show(Anth.budget_for("", 64000n)), Nat.show(Anth.budget_for("low", 6000n))]
    == ["4000", "8000", "16000", "32000", "16000", "3000"] : List<&2, String>}

# ---- calls and results stay paired (BISE-242) ----

# LAW: whatever the history (a call cut by a restart, a lost result, a
# stray result), the guard answers every call before any other message.
law pair_calls_paired:
  for +ms: List<&2, T.Msg>
  {H.paired(H.pair_calls(ms), Nil{}) == True{} : Bool}

# LAW: so every model input the Core sends is paired: the provider never
# sees a tool call without its result right after it.
law model_input_paired:
  for +cfg: T.Cfg
  for +h: List<&2, T.Msg>
  {H.paired(S.model_input(cfg, h), Nil{}) == True{} : Bool}

# LAW: on a history that is already paired, the guard changes nothing.
law pair_calls_keeps_paired:
  for +ms: List<&2, T.Msg>
  for p: {H.paired(ms, Nil{}) == True{} : Bool}
  {H.pair_calls(ms) == ms : List<&2, T.Msg>}

# LAW: the wire guard: a call cut before its result gets a failed one,
# right after its assistant message.
law pair_wire_answers_the_cut:
  {W.pair_wire(W.WReq{Nil{}, [W.WMsg{"user", "q", Nil{}},
      W.WMsg{"assistant", "", [W.WCall{"call_7", "bash", "{}"}]},
      W.WMsg{"user", "next", Nil{}}], None{}})
    == W.WReq{Nil{}, [W.WMsg{"user", "q", Nil{}},
      W.WMsg{"assistant", "", [W.WCall{"call_7", "bash", "{}"}]},
      W.WMsg{"tool", "tool bash failed: no result: bise restarted while this ran", Nil{}},
      W.WMsg{"user", "next", Nil{}}], None{}} : W.WReq}

# ---- the prompt cache (BISE-268): the start of every request stays
# byte-identical from one call of a session to the next ----

# LAW: past the cap, the oldest images go in steps of half the cap: the
# dropped set stays the same while cap/2 new images come (a slide of
# one would change the history at the oldest image on every call).
law image_keep_stepped:
  {Im.img_keep([True{}, True{}, True{}, True{}, True{}], 4n)
    == [False{}, False{}, True{}, True{}, True{}] : List<&2, Bool>}

law image_keep_stepped_same_drop:
  {Im.img_keep([True{}, True{}, True{}, True{}, True{}, True{}], 4n)
    == [False{}, False{}, True{}, True{}, True{}, True{}] : List<&2, Bool>}

# LAW: the next step drops the next half at once (7 images, cap 4: 3 kept).
law image_keep_next_step:
  {Im.img_keep([True{}, True{}, True{}, True{}, True{}, True{}, True{}], 4n)
    == [False{}, False{}, False{}, False{}, True{}, True{}, True{}]
    : List<&2, Bool>}

# three calls of one session, then its compaction: a tool round trip,
# then a new user turn; each agent call ends with its own context
def pc_req1() -> String:
  "MODEL default\nTOOL bash : Runs.\n\nMSG system : S\nMSG user : go\n\nEND\n"

def pc_req2() -> String:
  "MODEL default\nTOOL bash : Runs.\n\nMSG system : S\nMSG user : go\nMSG assistant : x\n  CALL 7 bash : ls\nMSG tool : out1\n\nEND\n"

def pc_req3() -> String:
  "MODEL default\nTOOL bash : Runs.\n\nMSG system : S\nMSG user : go\nMSG assistant : x\n  CALL 7 bash : ls\nMSG tool : out1\nMSG assistant : done\nMSG user : next\n\nEND\n"

def pc_compact() -> String:
  "MODEL default\nCOMPACT\nTOOL bash : Runs.\n\nMSG system : S\nMSG user : go\nMSG assistant : x\n  CALL 7 bash : ls\nMSG tool : out1\nMSG assistant : done\nMSG user : next\nMSG user : Summarize.\n\nEND\n"

# a request's body as the provider gets it, without the moving cache
# marks (Anthropic's breakpoints follow the tail by design: they are not
# part of the cached prefix)
def pc_body(+st: Api.Style, +ctx: String, +req: String) -> String:
  Tx.drop_all(Api.api_body_for(st, "m", Api.facts0(),
      W.pair_wire(W.parse_wire(Rm.with_context(ctx, req)))),
    ",\"cache_control\":{\"type\":\"ephemeral\"}")

# `b1` up to its context message (found by `needle`; not found: False,
# the law is never vacuous) starts `b2`
def pc_stable.at(at: Maybe<&2, Nat>, +b1: String, +b2: String) -> Bool:
  match at:
    case None{}:
      False{}
    case Some{+n}:
      Bool.and(Nat.is_gt(n, 100n), String.starts_with(b2, String.take(b1, n)))

def pc_stable(+b1: String, +b2: String, +needle: String) -> Bool:
  pc_stable.at(Tx.find(b1, needle, 0n), b1, b2)

def pc_session(+st: Api.Style, +needle: String) -> Bool:
  Bool.and(pc_stable(pc_body(st, "board v1", pc_req1()),
      pc_body(st, "board v2", pc_req2()), needle),
    Bool.and(pc_stable(pc_body(st, "board v2", pc_req2()),
        pc_body(st, "board v3", pc_req3()), needle),
      pc_stable(pc_body(st, "board v3", pc_req3()),
        pc_body(st, "board v4", pc_compact()), needle)))

# LAW: prefix stability, OpenAI Chat family: each request of a session
# starts with the whole previous one up to its context message (the
# only thing dropped), across a tool round trip, a new user turn and
# the compaction call.
law prefix_stable_oai:
  {pc_session(Api.OpenAI{}, "{\"role\":\"user\",\"content\":\"board v") == True{} : Bool}

# LAW: the same for the Anthropic family, its cache marks aside.
law prefix_stable_anth:
  {pc_session(Api.Anthropic{}, "{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"board v") == True{} : Bool}

# LAW: the context never carries a cache breakpoint; the two newest
# history messages do (the newest writes, the one before reads).
law anth_context_not_a_breakpoint:
  {String.contains(Anth.api_body_anth_for("m", 0n, "", True{}, "high",
        W.parse_wire(Rm.with_context("board", pc_req2()))),
      "\"input\":{\"arg\":\"ls\"},\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"out1\",\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"board\"}]}],")
    == True{} : Bool}

# LAW: on the OpenAI Chat family the context is a plain user message.
law oai_context_is_user:
  {String.contains(Oai.api_body_with("m", Oai.Opts{False{}, 0n, "max_tokens", ""},
      W.parse_wire(Rm.with_context("board", pc_req1()))),
    "{\"role\":\"user\",\"content\":\"board\"}]") == True{} : Bool}

# LAW: the routing key of a session is the same on every call (its
# first two messages do not change) and says nothing but a hash.
law session_key_stable:
  {Api.session_key(W.parse_wire(pc_req3()))
    == Api.session_key(W.parse_wire(pc_req1())) : String}

law session_key_is_a_hash:
  {Api.session_key(W.parse_wire(pc_req1())) == "bise-2869019270" : String}

# LAW: the key rides as the body field the catalog names, first; no
# field named, the body is unchanged.
law cache_key_field:
  {Api.with_cache_key("prompt_cache_key", "bise-1", "{\"model\":\"m\"}")
    == "{\"prompt_cache_key\":\"bise-1\",\"model\":\"m\"}" : String}

law cache_key_field_none:
  {Api.with_cache_key("", "bise-1", "{\"model\":\"m\"}")
    == "{\"model\":\"m\"}" : String}

# LAW: or as the header the catalog names, after the family's; none
# named, the headers are unchanged.
law cache_key_header:
  {Api.with_cache_header("x-grok-conv-id", "bise-1", [Api.Hdr{"a", "b"}])
    == [Api.Hdr{"a", "b"}, Api.Hdr{"x-grok-conv-id", "bise-1"}]
    : List<&2, Api.Hdr>}

law cache_key_header_none:
  {Api.with_cache_header("", "bise-1", [Api.Hdr{"a", "b"}])
    == [Api.Hdr{"a", "b"}] : List<&2, Api.Hdr>}

# ---- a gateway's extra headers (catalog headers_env) ----
# LAW: one "Name: value" per line (Claude Code's
# ANTHROPIC_CUSTOM_HEADERS), trimmed, after the family's; a value keeps
# its own ':'; a line with no ':' or no name is skipped.
law extra_headers_lines:
  {Api.with_extra_headers("source: bise\norg-id : 2 \nurl: a:b",
      [Api.Hdr{"x-api-key", "k"}])
    == [Api.Hdr{"x-api-key", "k"}, Api.Hdr{"source", "bise"},
      Api.Hdr{"org-id", "2"}, Api.Hdr{"url", "a:b"}]
    : List<&2, Api.Hdr>}

law extra_headers_skipped:
  {Api.extra_headers("no colon\n: no name\n\n") == [] : List<&2, Api.Hdr>}

law extra_headers_none:
  {Api.with_extra_headers("", [Api.Hdr{"a", "b"}])
    == [Api.Hdr{"a", "b"}] : List<&2, Api.Hdr>}

# LAW: a key_command's key also rides as a bearer token, after the
# family's headers (Claude Code's apiKeyHelper); else nothing is added.
law key_command_bearer:
  {Api.with_bearer(True{}, "k", [Api.Hdr{"x-api-key", "k"}])
    == [Api.Hdr{"x-api-key", "k"}, Api.Hdr{"Authorization", "Bearer k"}]
    : List<&2, Api.Hdr>}

law key_command_bearer_off:
  {Api.with_bearer(False{}, "k", [Api.Hdr{"x-api-key", "k"}])
    == [Api.Hdr{"x-api-key", "k"}] : List<&2, Api.Hdr>}

# LAW: a compaction's first attempt shares the prefix (tools included,
# the COMPACT line first); its retry goes bare (no tool to call).
law compact_first_attempt_shares:
  {Rt.compact_first(Some{T.PComp{1, 1n, 1, "", True{}}}) == True{} : Bool}

law compact_retry_is_bare:
  {Rt.compact_first(Some{T.PComp{1, 2n, 2, "", True{}}}) == False{} : Bool}

# LAW (image-tag, 2026-10-02): a marker in a tool's result is an image only
# for run_typescript (its image blocks); bash output, a file read or sb
# inspect printing the user's marker is text.
law image_scope_tool_text:
  {W.img_scope("tool", "tool bash ok: x") == False{} : Bool}

law image_scope_run_typescript:
  {W.img_scope("tool", "tool run_typescript ok: x") == True{} : Bool}

# LAW: a marker value holding a newline is no marker (the wire escapes it,
# so the wire scan and the message parse must agree it is text).
law image_marker_newline_is_text:
  {Im.img_paths(String.append(Im.img_open(),
    "a\" path=\"p\nq\" mime=\"image/png\" b64=\"/s/1.b64\">"))
    == Nil{} : List<&2, String>}

# LAW: an image part the IO step could not fill becomes a text part with
# its name and reason; a filled one gets its data.
law image_part_unavailable_is_text:
  {Im.body_fill(Im.body_segs("[{\"a\":1},\"@@BENDPART:a|image/png|/s/1.b64|[Image #1]@@\"]"),
    Con{Im.IRBad{"file missing"}, Nil{}})
    == "[{\"a\":1},{\"type\":\"text\",\"text\":\"[image unavailable: [Image #1] (file missing)]\"}]"
    : String}

law image_part_filled:
  {Im.body_fill(Im.body_segs("[{\"a\":1},\"@@BENDPART:o|image/png|/s/1.b64|n@@\"]"),
    Con{Im.IROk{"QUJD", 4n}, Nil{}})
    == "[{\"a\":1},{\"type\":\"image_url\",\"image_url\":{\"url\":\"data:image/png;base64,QUJD\"}}]"
    : String}

# LAW: the 20 most recent good images go; an older one becomes text.
law image_part_cap:
  {Im.res_cap(Con{Im.IROk{"a", 1n}, Con{Im.IRBad{"x"}, Con{Im.IROk{"b", 1n}, Nil{}}}}, 1n)
    == Con{Im.IRBad{"older than the 1 most recent images"}, Con{Im.IRBad{"x"},
      Con{Im.IROk{"b", 1n}, Nil{}}}} : List<&2, Im.IRes>}

# LAW (big-request): the images that go fit the request's byte limit.
# Four images of 1 MiB under a 3 MiB limit: the count cap halves (20,
# 10, 5, 2) until the kept ones fit, the oldest become text that says
# why.
law image_part_fit_bytes:
  {Im.res_fit(Con{Im.IROk{"a", 1048576n}, Con{Im.IROk{"b", 1048576n},
      Con{Im.IROk{"c", 1048576n}, Con{Im.IROk{"d", 1048576n}, Nil{}}}}},
      20n, 5n, 3145728n)
    == Con{Im.IRBad{"removed to keep the request under 3 MB"},
      Con{Im.IRBad{"removed to keep the request under 3 MB"},
        Con{Im.IROk{"c", 1048576n}, Con{Im.IROk{"d", 1048576n}, Nil{}}}}}
    : List<&2, Im.IRes>}

# LAW (big-request): the count cap and the byte limit together: the ones
# over the count say so, the next ones dropped for the bytes say that, a
# bad one keeps its own reason, the newest goes.
law image_part_fit_count_then_bytes:
  {Im.res_fit(Con{Im.IROk{"a", 1048576n}, Con{Im.IRBad{"x"},
      Con{Im.IROk{"b", 1048576n}, Con{Im.IROk{"c", 1048576n},
        Con{Im.IROk{"d", 1048576n}, Nil{}}}}}}, 2n, 5n, 1572864n)
    == Con{Im.IRBad{"older than the 2 most recent images"}, Con{Im.IRBad{"x"},
      Con{Im.IRBad{"older than the 2 most recent images"},
        Con{Im.IRBad{"removed to keep the request under 1 MB"},
          Con{Im.IROk{"d", 1048576n}, Nil{}}}}}}
    : List<&2, Im.IRes>}

# LAW (big-request): under the limit nothing changes; a limit no image
# fits sends none.
law image_part_fit_under_limit:
  {Im.res_fit(Con{Im.IROk{"a", 10n}, Con{Im.IROk{"b", 10n}, Nil{}}}, 20n, 5n, 100n)
    == Con{Im.IROk{"a", 10n}, Con{Im.IROk{"b", 10n}, Nil{}}} : List<&2, Im.IRes>}

law image_part_fit_none:
  {Im.res_fit(Con{Im.IROk{"a", 10n}, Nil{}}, 20n, 50n, 40n)
    == Con{Im.IRBad{"removed to keep the request under 0 KB"}, Nil{}}
    : List<&2, Im.IRes>}

# LAW (big-request): a filled body says what it weighs: its text plus the
# images that went.
law image_fill_fit_weighs:
  {Im.fill_fit(Im.body_segs("[{\"a\":1},\"@@BENDPART:a|image/png|/s/1.b64|n@@\"]"),
      Con{Im.IROk{"QUJD", 4n}, Nil{}}, 20n, 1000n)
    == Im.IFill{"[{\"a\":1},{\"type\":\"image\",\"source\":{\"type\":\"base64\",\"media_type\":\"image/png\",\"data\":\"QUJD\"}}]",
      14n, 4n} : Im.IFill}

# LAW: a .b64 file is sent only if non-empty, a multiple of 4 bytes, base64
law image_why_checks:
  {Con{Im.img_why(0, ""), Con{Im.img_why(5, "QUJDR"), Con{Im.img_why(4, "da:a"),
    Con{Im.img_why(4, "QUJD"), Nil{}}}}}
    == Con{"empty file", Con{"invalid base64", Con{"invalid base64", Con{"", Nil{}}}}}
    : List<&2, String>}
