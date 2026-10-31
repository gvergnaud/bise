# bend/laws/providers.bend: the provider adapters, tool names, OpenAI Responses, the ChatGPT plan and its errors.
# Open claims, each one closed by bend/proofs/providers.bend (moved from
# LAWS.bend). The gate checks every bend/proofs/*.bend: a law here
# with no proof there fails it.
import Base
import ../core/config.bend as Cf
import ../core/api.bend as Api
import ../core/wire.bend as W
import ../core/oai-chat.bend as Oai
import ../core/oai-resp.bend as Rs
import ../runtime/provider-pure.bend as Pv
import ../runtime/threshold-pure.bend as Th
import ../runtime/settings-pure.bend as St
import ../vendor/json.bend as J

# ---- the provider adapters (core/api.bend + runtime/provider.bend) ----
# The vibe CLI structure: one mapping per api_style, a provider table
# in the runtime. The OpenAI mapping above stays exactly as the GLM
# laws pinned it; the Anthropic mapping below is the Messages shape
# the foundry proxy serves (probed live: thinking blocks are optional,
# tool_use without history thinking is accepted, a thinking block
# without its signature is rejected). A SIGNED history reasoning
# replays its thinking block verbatim with the signature (the
# reasoning continuity across tool calls); an unsigned one stays
# stripped.

# The provider registry (runtime/provider-pure.bend, BISE-141): a model
# id is "provider/model"; the provider gives the family, the base_url
# and the key env; the models file Rust writes (BISE_MODELS_FILE) sits
# over the entries built in; a bare name is an alias, else the legacy
# guess (claude* -> foundry, else mistral). Pv.resolve(BISE_MODEL,
# BEND_MODEL, BISE_ROLE, config.toml, models file, BEND_PROVIDER_URL).

# LAW: no model anywhere (no env var, no config, no models file): no
# call, the error says to pick a provider (BISE-266: no built-in
# default). The opus-5.5 alias still goes to the foundry proxy (Messages
# API, its key), a bare non-claude name to Mistral (Chat, its key).
law provider_no_default_model:
  {Pv.resolve("", "", "", "", "", "", "")
    == Pv.RErr{"", Pv.NO_MODEL()} : Pv.Resolved}

law provider_resolves_alias:
  {Pv.resolve("", "", "", "", "model = \"opus-5.5\"",
      "[providers.foundry]\nbase_url = \"https://foundry.example.net/anthropic/v1\"
efforts = \"none,low,medium,high,xhigh,max\"", "")
    == Pv.ROk{Pv.Prov{"foundry", "claude-opus-5-5", Api.Anthropic{},
      "https://foundry.example.net/anthropic/v1/messages",
      "ANTHROPIC_FOUNDRY_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}}} : Pv.Resolved}

law provider_legacy_claude_is_foundry:
  {Pv.resolve("", "claude-opus-5-5", "", "", "",
      "[providers.foundry]\nbase_url = \"https://foundry.example.net/anthropic/v1\"
efforts = \"none,low,medium,high,xhigh,max\"", "")
    == Pv.ROk{Pv.Prov{"foundry", "claude-opus-5-5", Api.Anthropic{},
      "https://foundry.example.net/anthropic/v1/messages",
      "ANTHROPIC_FOUNDRY_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}}} : Pv.Resolved}

# LAW: bise ships no URL for the foundry proxy (a private one): without
# a base_url in config.toml (so in the models file), no call, the error
# says where to set it.
law provider_foundry_needs_a_base_url:
  {Pv.resolve("", "", "", "", "model = \"opus-5.5\"", "", "")
    == Pv.RErr{"foundry/claude-opus-5-5", "provider \"foundry\" (model \"foundry/claude-opus-5-5\") has no base_url: add it under [providers.foundry] in ~/.bise/config.toml"} : Pv.Resolved}

# LAW: the models file names the variable that would give the URL
# (base_url_env, rust/catalog: ANTHROPIC_FOUNDRY_BASE_URL): the error
# says to set it.
law provider_foundry_names_its_url_variable:
  {Pv.resolve("", "", "", "", "model = \"opus-5.5\"",
      "[providers.foundry]
base_url_env = \"ANTHROPIC_FOUNDRY_BASE_URL\"", "")
    == Pv.RErr{"foundry/claude-opus-5-5", "provider \"foundry\" (model \"foundry/claude-opus-5-5\") has no base_url: set ANTHROPIC_FOUNDRY_BASE_URL, or add it under [providers.foundry] in ~/.bise/config.toml"} : Pv.Resolved}

# LAW: base_url = "" (an older models file, Ben's first run 2026-10-01)
# is no base_url: the same error, never a call to "/messages".
law provider_empty_base_url_is_none:
  {Pv.resolve("", "", "", "", "model = \"opus-5.5\"",
      "[providers.foundry]
base_url = \"\"", "")
    == Pv.RErr{"foundry/claude-opus-5-5", "provider \"foundry\" (model \"foundry/claude-opus-5-5\") has no base_url: add it under [providers.foundry] in ~/.bise/config.toml"} : Pv.Resolved}

law provider_legacy_glm_is_mistral:
  {Pv.resolve("", "zai-glm-5-3", "", "", "", "", "")
    == Pv.ROk{Pv.Prov{"mistral", "zai-glm-5-3", Api.OpenAI{},
      "https://api.mistral.ai/v1/chat/completions", "MISTRAL_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "", False{}}}} : Pv.Resolved}

# LAW: a model the registry does not list works on its provider's
# defaults.
law provider_unlisted_model_works:
  {Pv.resolve("", "", "", "", "model = \"mistral/brand-new-7b\"", "", "")
    == Pv.ROk{Pv.Prov{"mistral", "brand-new-7b", Api.OpenAI{},
      "https://api.mistral.ai/v1/chat/completions", "MISTRAL_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "", False{}}}} : Pv.Resolved}

# LAW: the choice order (as rust/catalog's Setup): main = BISE_MODEL >
# BEND_MODEL > config model; a sub-agent (BISE_ROLE=agent) =
# BISE_AGENT_MODEL > config agent_model > the main order.
law provider_bise_model_first:
  {Pv.shown_model(Pv.resolve("mistral/a", "mistral/b", "mistral/e", "main",
      "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/a" : String}

law provider_bend_model_over_config:
  {Pv.shown_model(Pv.resolve("", "mistral/b", "mistral/e", "main",
      "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/b" : String}

law provider_agent_env_first:
  {Pv.shown_model(Pv.resolve("mistral/a", "mistral/b", "mistral/e", "agent",
      "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/e" : String}

law provider_agent_model_for_agents:
  {Pv.shown_model(Pv.resolve("mistral/a", "mistral/b", "", "agent",
      "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/d" : String}

law provider_agent_follows_main:
  {Pv.shown_model(Pv.resolve("", "mistral/b", "", "agent", "model = \"mistral/c\"", "", ""))
    == "mistral/b" : String}

law provider_agent_model_not_for_main:
  {Pv.shown_model(Pv.resolve("", "", "mistral/e", "main", "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/c" : String}

# LAW (BISE-298): the model roles table. [roles] main / agents (a line,
# or [roles.<r>] model = ...) come before the old model / agent_model;
# the efforts too ([roles.<r>] effort before (agent_)reasoning_effort).
law provider_roles_main_over_model:
  {Pv.shown_model(Pv.resolve("", "", "", "main",
      "model = \"mistral/c\"
[roles]
main = \"mistral/r\"", "", ""))
    == "mistral/r" : String}

law provider_roles_agents_table:
  {(Pv.shown_model(Pv.resolve("", "", "", "agent",
      "agent_model = \"mistral/d\"
[roles.agents]
model = \"mistral/t\"", "", "")),
    Pv.shown_model(Pv.resolve("", "", "", "main",
      "[roles]
main = \"mistral/r\"
agents = \"mistral/s\"", "", "")),
    Pv.shown_model(Pv.resolve("", "", "", "agent",
      "[roles]
main = \"mistral/r\"", "", "")))
    == ("mistral/t", "mistral/r", "mistral/r") : String & String & String}

law effort_of_the_roles:
  {(Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "agent",
        "model = \"opus-5.5\"
agent_reasoning_effort = \"low\"
[roles.agents]
effort = \"medium\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"
efforts = \"none,low,medium,high,xhigh,max\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "main",
        "model = \"opus-5.5\"
reasoning_effort = \"high\"
[roles.main]
effort = \"low\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"
efforts = \"none,low,medium,high,xhigh,max\"", "")))
    == ("medium", "low") : String & String}

# LAW (BISE-150, issue #18): no threshold ever goes past 85 % of the
# model's context window: its `context` in the models file, else its
# provider's; nothing says (an unknown provider, no models file for a
# provider the built-in table lacks): 128000, as rust/catalog's
# DEFAULT_CAPS. BEND_THRESHOLD > config compaction_threshold > the
# default (450k from a 1M window, else that 85 %).
law threshold_cap_is_85_percent_of_the_window:
  {(Th.threshold_of(1000000), Th.threshold_of(200000), Th.threshold_of(128000),
      Th.threshold_of(131072), Th.threshold_of(0))
    == (850000, 170000, 108800, 111411, 0) : U32 & U32 & U32 & U32 & U32}

# LAW (issue #18, Gabriel on page token-audit): the rows of
# rust/catalog/testdata/compaction_threshold.tsv, which rust/catalog's
# compaction_threshold holds too (its test reads this law's rows)
law threshold_rows:
  {(Th.threshold("", "", 1050000),
      Th.threshold("", "", 1000000),
      Th.threshold("", "", 999999),
      Th.threshold("", "", 200000),
      Th.threshold("", "", 128000),
      Th.threshold("", "compaction_threshold = 600000", 1050000),
      Th.threshold("", "compaction_threshold = \"45%\"", 1050000),
      Th.threshold("", "compaction_threshold = \"95%\"", 1050000),
      Th.threshold("", "compaction_threshold = 450000", 200000))
    == (450000, 450000, 849999, 170000, 108800, 600000, 472500, 892500, 170000)
    : U32 & U32 & U32 & U32 & U32 & U32 & U32 & U32 & U32}

# LAW (issue #18): nothing written, a window of 1M or more compacts at
# 450k (the Core gets 337500), whatever the model
law default_caps_at_450k_from_1m:
  {(Th.threshold("", "", 1000000), Th.threshold("", "", 2000000),
      Pv.est_units(Th.threshold("", "", 1050000)))
    == (450000, 450000, 337500) : U32 & U32 & U32}

# LAW (issue #18): a value written wins over the 450k default, from the
# config or the environment, up to 85 % of the window
law written_value_not_capped_by_450k:
  {(Th.threshold("", "compaction_threshold = 600000", 1000000),
      Th.threshold("700000", "", 1000000),
      Th.threshold("", "compaction_threshold = 900000", 1000000))
    == (600000, 700000, 850000) : U32 & U32 & U32}

# LAW (issue #18): the default never goes past 85 % of the window
law default_never_past_85:
  {(U32.is_lt(Th.threshold_of(1000000), Th.threshold("", "", 1000000)),
      U32.is_lt(Th.threshold_of(999999), Th.threshold("", "", 999999)),
      U32.is_lt(Th.threshold_of(500), Th.threshold("", "", 500)))
    == (False{}, False{}, False{}) : Bool & Bool & Bool}

law window_builtin_setups:
  {(Pv.window("", "", "", "", "model = \"opus-5.5\"", ""),
      Pv.window("", "", "", "", "model = \"zai-glm-5-3\"", ""))
    == (1000000, 128000) : U32 & U32}

law window_model_over_provider:
  {(Pv.window("", "", "", "", "model = \"work/big\"",
        "[providers.work]\nbase_url = \"http://x\"\ncontext = 32768\n\n[models.\"work/big\"]\ncontext = 262144\n"),
      Pv.window("", "", "", "", "model = \"work/small\"",
        "[providers.work]\nbase_url = \"http://x\"\ncontext = 32768\n"))
    == (262144, 32768) : U32 & U32}

law window_of_the_agent_model:
  {Pv.window("", "", "", "agent", "model = \"a/x\"\nagent_model = \"b/y\"",
      "[providers.a]\ncontext = 1000\n\n[providers.b]\ncontext = 2000\n")
    == 2000 : U32}

law window_unknown_model_default:
  {(Pv.window("nowhere/m", "", "", "", "", ""),
      Pv.window("", "", "", "", "model = \"work/m\"",
        "[providers.work]\ncontext = lots\n"))
    == (128000, 128000) : U32 & U32}

law threshold_precedence:
  {(Th.threshold("5000", "compaction_threshold = 7000", 1000000),
      Th.threshold("", "compaction_threshold = 7000", 1000000),
      Th.threshold("", "model = \"x\"", 200000),
      Th.threshold("oops", "compaction_threshold = 7000", 200000))
    == (5000, 7000, 170000, 170000) : U32 & U32 & U32 & U32}

# LAW: the Core gets the threshold in its own units, three quarters of
# the provider's tokens (Pv.est_units). Main's 450000 becomes 337500:
# its history crossed 450607 real tokens at an estimate of 343760 and
# peaked at 507700 real (estimate 396485) without compacting; both
# estimates are past 337500, so it would have compacted near 450k (an
# estimate just under 337500 still goes out).
law threshold_in_core_units:
  {(Pv.est_units(450000), Pv.est_units(160000), Pv.est_units(7),
      U32.is_lt(343760, Pv.est_units(450000)),
      U32.is_lt(337499, Pv.est_units(450000)))
    == (337500, 120000, 5, False{}, True{})
    : U32 & U32 & U32 & Bool & Bool}

# LAW (BISE-300): config `compaction_threshold` is a number of tokens or
# a share of the model's window ("45%", spaces allowed around it);
# BEND_THRESHOLD reads the same way.
law threshold_tokens_or_percent:
  {(Th.threshold("", "compaction_threshold = 450000", 1000000),
      Th.threshold("", "compaction_threshold = \"45%\"", 1000000),
      Th.threshold("", "compaction_threshold = \"45%\"", 200000),
      Th.threshold("", "compaction_threshold = \" 45 % \"", 128000),
      Th.threshold("10%", "compaction_threshold = 7000", 200000))
    == (450000, 450000, 90000, 57600, 20000)
    : U32 & U32 & U32 & U32 & U32}

# LAW (BISE-300, issue #18): the threshold never goes above 85 % of the
# model's window, whoever sets it: a number written for a 1M model on a
# 200k one, a share above 85 %, the environment.
law threshold_capped_at_85_percent:
  {(Th.threshold("", "compaction_threshold = 450000", 200000),
      Th.threshold("", "compaction_threshold = \"95%\"", 1000000),
      Th.threshold("", "compaction_threshold = \"250%\"", 200000),
      Th.threshold("900000", "", 200000),
      Th.threshold("", "compaction_threshold = 160000", 200000))
    == (170000, 850000, 170000, 170000, 160000)
    : U32 & U32 & U32 & U32 & U32}

# LAW (BISE-300): a value that is no threshold (0, 0 %, a word, a
# decimal share, a lone %) leaves the default; the old key `threshold`
# is not read any more.
law threshold_bad_values_default:
  {(Th.threshold("", "compaction_threshold = 0", 200000),
      Th.threshold("", "compaction_threshold = \"0%\"", 200000),
      Th.threshold("", "compaction_threshold = \"lots\"", 200000),
      Th.threshold("", "compaction_threshold = \"4.5%\"", 200000),
      Th.threshold("", "compaction_threshold = \"%\"", 200000),
      Th.threshold("", "threshold = 7000", 200000))
    == (170000, 170000, 170000, 170000, 170000, 170000)
    : U32 & U32 & U32 & U32 & U32 & U32}

# LAW (BISE-150): the config template leaves the threshold to the model
law config_template_threshold_unset:
  {Cf.cfg_get(St.CONFIG_TEMPLATE(), "compaction_threshold") == None{}
    : Maybe<&2, String>}

# LAW: a provider of the models file: its family, base_url (a trailing
# slash dropped) + the family's endpoint, its key env; the model id
# keeps every '/' after the first.
law provider_from_models_file:
  {Pv.resolve("", "", "", "", "model = \"work/qwen/q3-coder\"",
      "[providers.work]\napi = \"openai-chat\"\nbase_url = \"https://llm.corp.example/v1/\"\nkey_env = \"CORP_KEY\"\nneeds = \"\"",
      "")
    == Pv.ROk{Pv.Prov{"work", "qwen/q3-coder", Api.OpenAI{},
      "https://llm.corp.example/v1/chat/completions", "CORP_KEY", Api.MFacts{False{}, 0n, "max_tokens", "", "", "", False{}}}}
    : Pv.Resolved}

# LAW: a model's own key wins over its provider's (a model on another
# family), and key_env = "" (a local server) needs no key.
law provider_model_key_wins:
  {Pv.resolve("", "", "", "", "model = \"local/m\"",
      "[providers.local]\napi = \"openai-chat\"\nbase_url = \"http://localhost:1234/v1\"\nkey_env = \"\"\n[models.\"local/m\"]\napi = \"anthropic\"",
      "")
    == Pv.ROk{Pv.Prov{"local", "m", Api.Anthropic{},
      "http://localhost:1234/v1/messages", "", Api.MFacts{False{}, 0n, "max_tokens", "", "", "", False{}}}} : Pv.Resolved}

# LAW: the models file wins over the built-in entries, key by key; its
# aliases and default_model are used.
law provider_file_over_builtin:
  {Pv.resolve("", "", "", "", "",
      "default_model = \"big\"\n[aliases]\n\"big\" = \"foundry/claude-x\"\n[providers.foundry]\nbase_url = \"https://api.anthropic.com/v1\"
efforts = \"none,high\"",
      "")
    == Pv.ROk{Pv.Prov{"foundry", "claude-x", Api.Anthropic{},
      "https://api.anthropic.com/v1/messages",
      "ANTHROPIC_FOUNDRY_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}}} : Pv.Resolved}

# LAW: BEND_PROVIDER_URL (the tests' fake provider) wins over the URL.
law provider_url_env_wins:
  {Pv.resolve("", "mistral-small-latest", "", "", "", "",
      "http://127.0.0.1:9/v1/chat/completions")
    == Pv.ROk{Pv.Prov{"mistral", "mistral-small-latest", Api.OpenAI{},
      "http://127.0.0.1:9/v1/chat/completions", "MISTRAL_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "", False{}}}}
    : Pv.Resolved}

# LAW: a long file is read to its end (a pair line takes 3 steps of the
# walk: the fuel counts them; the models file has ~350 lines).
law config_reads_long_file:
  {Cf.cfg_get("[providers.p0]\nbase_url = \"u0\"\n[providers.p1]\nbase_url = \"u1\"\n[providers.p2]\nbase_url = \"u2\"\n[providers.p3]\nbase_url = \"u3\"\n[providers.p4]\nbase_url = \"u4\"\n[providers.p5]\nbase_url = \"u5\"\n[providers.p6]\nbase_url = \"u6\"\n[providers.p7]\nbase_url = \"u7\"\n[providers.p8]\nbase_url = \"u8\"\n[providers.p9]\nbase_url = \"u9\"\n[providers.p10]\nbase_url = \"u10\"\n[providers.p11]\nbase_url = \"u11\"", "providers.p11.base_url")
    == Some{"u11"} : Maybe<&2, String>}

# LAW: what does not resolve is an error at call time, with its reason.
law provider_unknown_is_error:
  {Pv.resolve("nope/m", "", "", "", "", "", "")
    == Pv.RErr{"nope/m", "unknown provider \"nope\" (model \"nope/m\"): add a [providers.nope] table (base_url, key_env, api) to config.toml"}
    : Pv.Resolved}

law provider_needs_is_error:
  {Pv.resolve("az/m", "", "", "", "",
      "[providers.az]\napi = \"openai-chat\"\nbase_url = \"https://x\"\nneeds = \"BISE-149\"", "")
    == Pv.RErr{"az/m", "model \"az/m\": provider \"az\" is not supported yet (needs BISE-149)"}
    : Pv.Resolved}

law provider_unknown_family_is_error:
  {Pv.resolve("g/m", "", "", "", "",
      "[providers.g]\napi = \"gemini\"\nbase_url = \"https://x\"", "")
    == Pv.RErr{"g/m", "model \"g/m\": api \"gemini\" of provider \"g\" is not supported yet"}
    : Pv.Resolved}

# LAW: a family's api name reads back as that family.
law family_name_roundtrip:
  for st: Api.Style
  {Api.style_of_api(Api.api_name(st)) == Some{st} : Maybe<&2, Api.Style>}

# LAW: the headers of each family, in order (the bytes of today's
# calls); no key, no auth header.
law family_headers_anthropic:
  {Api.headers(Api.Anthropic{}, "", "k")
    == [Api.Hdr{"x-api-key", "k"},
      Api.Hdr{"anthropic-version", "2023-06-01"},
      Api.Hdr{"anthropic-beta",
        "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14,prompt-caching-2024-07-31,context-1m-2025-08-07"},
      Api.Hdr{"Content-Type", "application/json"}] : List<&2, Api.Hdr>}

law family_headers_openai:
  {Api.headers(Api.OpenAI{}, "", "k")
    == [Api.Hdr{"Authorization", "Bearer k"},
      Api.Hdr{"Content-Type", "application/json"}] : List<&2, Api.Hdr>}

law family_headers_no_key:
  {Api.headers(Api.OpenAI{}, "", "")
    == [Api.Hdr{"Content-Type", "application/json"}] : List<&2, Api.Hdr>}

# LAW: every family streams: the Messages API and OpenAI Chat (BISE-144).
law family_anthropic_streams:
  {Api.streams(Api.Anthropic{}) == True{} : Bool}

law family_openai_streams:
  {Api.streams(Api.OpenAI{}) == True{} : Bool}

# LAW: the style dispatch routes the body and the reply to the right
# adapter - the OpenAI side byte-identical to api_body/reply_of.
law style_routes_body:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : hi\nEND\n"))
    == Oai.api_body("m",
      W.parse_wire("MODEL default\nMSG user : hi\nEND\n"))
    : String}

law style_routes_reply:
  {Api.reply_of_for(Api.OpenAI{}, 401, "{\"error\":\"bad key\"}")
    == Oai.reply_of(401, "{\"error\":\"bad key\"}") : String}

# LAW: an Anthropic body maps a simple wire byte-for-byte: the system
# prompt becomes the top-level system field, the user message a text
# block, the thinking config the vibe CLI sends for opus-5.5 (adaptive,
# summarized, effort high, the adaptive max_tokens budget).
law api_body_anth_maps:
  {Api.api_body_for(Api.Anthropic{}, "claude-opus-5-5", Api.facts0(),
      W.parse_wire("MODEL default\nMSG system : You are precise.\nMSG user : bonjour\nEND\n"))
    == "{\"system\":[{\"type\":\"text\",\"text\":\"You are precise.\",\"cache_control\":{\"type\":\"ephemeral\"}}],\"model\":\"claude-opus-5-5\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"bonjour\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: xhigh (Claude's "extra high") goes as is in output_config.effort.
law api_body_anth_xhigh:
  {Api.api_body_for(Api.Anthropic{}, "m",
      Api.MFacts{True{}, 0n, "max_tokens", "", "", "xhigh", False{}},
      W.parse_wire("MODEL default
MSG user : salut
END
"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"xhigh\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"salut\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: an Anthropic compaction call (no TOOL lines) omits tools and
# tool_choice - the model cannot even offer a tool call.
law api_body_anth_bare_omits_tools:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : salut\nEND\n"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"salut\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: the Anthropic tools carry the {name, description, input_schema}
# shape and tool_choice is the auto object.
law api_body_anth_tools_shape:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nTOOL echo : Returns text.\nMSG user : bonjour\nEND\n"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"bonjour\",\"cache_control\":{\"type\":\"ephemeral\"}}]}],\"tools\":[{\"name\":\"echo\",\"description\":\"Returns text.\",\"input_schema\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"}},\"required\":[\"arg\"]},\"cache_control\":{\"type\":\"ephemeral\"}}],\"tool_choice\":{\"type\":\"auto\"}}"
    : String}

# LAW: an assistant call pairs with its tool result exactly as the
# OpenAI adapter pairs them - the tool_use id and the tool_use_id of
# the result agree, and the result rides a USER message.
law anth_tool_result_pairs:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : thinking\n  CALL 7 echo : hi\nMSG tool : tool echo ok: out\nEND\n"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"go\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"thinking\"},{\"type\":\"tool_use\",\"id\":\"call_7\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"},\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"tool echo ok: out\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: consecutive tool results merge into ONE user message - the
# vibe mapper's shape (the ids pair FIFO).
law anth_tool_results_merge:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : thinking\n  CALL 7 echo : hi\n  CALL 8 bash : ls\nMSG tool : out1\nMSG tool : out2\nEND\n")),
    "\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_8\",\"content\":\"out2\"}")
    == False{} : Bool}

# LAW: parallel calls pair with their results in order - the first
# CALL line takes the first tool result (the calls of one message
# were once reversed, so each result went to the other call).
law oai_parallel_results_pair_in_order:
  {String.contains(Oai.api_body("m",
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : x\n  CALL 7 echo : hi\n  CALL 8 bash : ls\nMSG tool : out1\nMSG tool : out2\nEND\n")),
    "{\"role\":\"tool\",\"content\":\"out1\",\"tool_call_id\":\"call_7\"},{\"role\":\"tool\",\"content\":\"out2\",\"tool_call_id\":\"call_8\"}")
    == True{} : Bool}

# LAW: the same pairing on the Anthropic adapter.
law anth_parallel_results_pair_in_order:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : x\n  CALL 7 echo : hi\n  CALL 8 bash : ls\nMSG tool : out1\nMSG tool : out2\nEND\n")),
    "{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"out1\"},{\"type\":\"tool_result\",\"tool_use_id\":\"call_8\",\"content\":\"out2\"")
    == True{} : Bool}

# LAW: the marked history reasoning is STRIPPED on the way back - a
# thinking block without its signature is rejected by the Messages
# API (probed), so no \"type\":\"thinking\" block ever rides an
# assistant message.
law anth_unsigned_thinking_stays_stripped:
  {Bool.not(String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
       W.parse_wire("MODEL default
MSG assistant : <think>2+2 = 4</think>\\n4
END
")),
       "type\":\"thinking\""))
    == True{} : Bool}

# LAW: a SIGNED marked reasoning replays its thinking block verbatim
# with the signature at block level - the reasoning continuity the
# Messages API needs across tool calls; the model no longer re-thinks
# after every tool result.
law anth_signed_thinking_replays:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
       W.parse_wire("MODEL default
MSG assistant : <think>2+2 = 4\\nBENDSIG::sig</think>\\n4
END
")),
       "{\"type\":\"thinking\",\"thinking\":\"2+2 = 4\",\"signature\":\"sig\"}")
    == True{} : Bool}

# LAW: EVERY signed span of one message replays as its own thinking
# block - a reply with two thinking blocks leaves no marker in the
# text (a span riding as plain text is refused as reasoning
# extraction, probed).
law anth_multi_span_replays:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
       W.parse_wire("MODEL default
MSG assistant : <think>a\\nBENDSIG::s1</think>\\n<think>b\\nBENDSIG::s2</think>\\nok
END
"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"assistant\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"a\",\"signature\":\"s1\"},{\"type\":\"thinking\",\"thinking\":\"b\",\"signature\":\"s2\"},{\"type\":\"text\",\"text\":\"ok\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: a long tool run keeps every message - 20 tool results still
# reach the body (the walk fuel covers the three hops of a tool message).
law anth_long_tool_run_keeps_messages:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : x\n  CALL 1 echo : hi\nMSG tool : out1\nMSG assistant : x\n  CALL 2 echo : hi\nMSG tool : out2\nMSG assistant : x\n  CALL 3 echo : hi\nMSG tool : out3\nMSG assistant : x\n  CALL 4 echo : hi\nMSG tool : out4\nMSG assistant : x\n  CALL 5 echo : hi\nMSG tool : out5\nMSG assistant : x\n  CALL 6 echo : hi\nMSG tool : out6\nMSG assistant : x\n  CALL 7 echo : hi\nMSG tool : out7\nMSG assistant : x\n  CALL 8 echo : hi\nMSG tool : out8\nMSG assistant : x\n  CALL 9 echo : hi\nMSG tool : out9\nMSG assistant : x\n  CALL 10 echo : hi\nMSG tool : out10\nMSG assistant : x\n  CALL 11 echo : hi\nMSG tool : out11\nMSG assistant : x\n  CALL 12 echo : hi\nMSG tool : out12\nMSG assistant : x\n  CALL 13 echo : hi\nMSG tool : out13\nMSG assistant : x\n  CALL 14 echo : hi\nMSG tool : out14\nMSG assistant : x\n  CALL 15 echo : hi\nMSG tool : out15\nMSG assistant : x\n  CALL 16 echo : hi\nMSG tool : out16\nMSG assistant : x\n  CALL 17 echo : hi\nMSG tool : out17\nMSG assistant : x\n  CALL 18 echo : hi\nMSG tool : out18\nMSG assistant : x\n  CALL 19 echo : hi\nMSG tool : out19\nMSG assistant : x\n  CALL 20 echo : hi\nMSG tool : out20\nEND\n")),
    "\"content\":\"out20\"")
    == True{} : Bool}

# LAW: a refusal (empty content, stop_reason "refusal") surfaces as a
# provider error - never an empty reply retried to the null limit.
law anth_refusal_maps:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[],\"stop_reason\":\"refusal\",\"stop_details\":{\"explanation\":\"blocked\"}}")
    == "ERROR provider refusal: blocked\nEND\n" : String}

# comment header preserved below
# LAW: Anthropic text blocks become separate lines (the GLM parity).
law anth_blocks_join_lines:
  {W.unescape_nl(Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"text\",\"text\":\"a\"},{\"type\":\"text\",\"text\":\"b\"}]}"))
    == "OK a\nb\nEND\n" : String}

# LAW: Anthropic thinking blocks surface in the reply wrapped in the
# think markers - the same transport the GLM adapter uses, so the TUI
# and the history are unchanged.
law anth_thinking_surfaces:
  {W.unescape_nl(Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"2+2 = 4\",\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}]}"))
    == "OK <think>2+2 = 4
BENDSIG::sig</think>
4
END
" : String}

# comment header preserved below
# LAW: tool_use blocks map onto the CALL reply lines the Core parses
# (1-based ids, the flat arg rehydrated from the input object).
law anth_tool_calls_map:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"}}]}")
    == "OK \nCALL 1 echo : hi\nEND\n" : String}

# LAW: a structured input rides as its JSON on the CALL line.
law anth_structured_call_maps:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"run_typescript\",\"input\":{\"code\":\"x()\"}}]}")
    == "OK \nCALL 1 run_typescript : {\"code\":\"x()\"}\nEND\n" : String}

# LAW: a 200 that carries an error body surfaces as an error.
law anth_error_body_maps:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}")
    == "ERROR provider 200: Overloaded\nEND\n" : String}

# LAW: a non-200 still maps to the flattened ERROR line with the status.
law anth_status_error_maps:
  {Api.reply_of_for(Api.Anthropic{}, 401, "{\"error\":\"bad key\"}")
    == "ERROR provider 401: bad key\nEND\n" : String}

# ---- tool names on the wire (BISE-293) ----
# Every family bise speaks (openai-chat: OpenAI, Mistral, OpenRouter...;
# anthropic) takes tool names matching ^[a-zA-Z0-9_-]{1,64}$ (OpenAI
# refused "self.compact" with a 400 on the user's first message). A
# name rides through W.wire_name on the way out (tools[] and the
# history's calls) and comes back through the request's own tools
# (W.name_in); the names bise sends today are already valid, so they
# ride unchanged.

# LAW: the live catalog's names are their own wire names.
law wire_name_keeps_the_catalog:
  {Bool.and(String.eq(W.wire_name("search_tool_functions"), "search_tool_functions"),
    Bool.and(String.eq(W.wire_name("bash"), "bash"),
     Bool.and(String.eq(W.wire_name("run_typescript"), "run_typescript"),
      Bool.and(String.eq(W.wire_name("skill"), "skill"),
       String.eq(W.wire_name("apply_patch"), "apply_patch"))))) == True{} : Bool}

# LAW: any other name is made valid: a dot (an MCP connector's
# "group.fn", the old self.* tools), a slash, a space, a non-ASCII char
# become "_", a long name is cut to 64 chars, an empty one is "_".
law wire_name_sanitizes:
  {Bool.and(String.eq(W.wire_name("self.compact"), "self_compact"),
    Bool.and(String.eq(W.wire_name("github.search_issues"), "github_search_issues"),
     Bool.and(String.eq(W.wire_name("my tool/v2"), "my_tool_v2"),
      Bool.and(String.eq(W.wire_name("café"), "caf_"),
       Bool.and(String.eq(W.wire_name(""), "_"),
        String.eq(W.wire_name("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
          "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")))))) == True{} : Bool}

# LAW: what comes out matches the pattern, and the pattern check
# refuses what the providers refuse.
law wire_name_matches_the_pattern:
  {Bool.and(W.name_ok(W.wire_name("self.reload")),
    Bool.and(W.name_ok(W.wire_name("linear.list-issues")),
     Bool.and(W.name_ok(W.wire_name("")),
      Bool.and(W.name_ok(W.wire_name("mcp.server.tool with spaces and ünicode.and.a.name.longer.than.sixty.four.chars")),
       Bool.and(Bool.not(W.name_ok("self.compact")),
        Bool.and(Bool.not(W.name_ok("")),
         Bool.not(W.name_ok("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"))))))))
    == True{} : Bool}

# LAW: the round trip: a call coming back under its wire name is named
# as the harness knows it; a valid name stays; a name no tool of the
# request has passes unchanged (the Core answers it "unknown tool").
law wire_names_round_trip:
  {W.reply_names_in(
      [W.WTool{"bash", ""}, W.WTool{"github.search_issues", ""}],
      "OK hi\nCALL 1 github_search_issues : {\"q\":\"a b\"}\nCALL 2 bash : ls\nCALL 3 self_compact : \nEND\n")
    == "OK hi\nCALL 1 github.search_issues : {\"q\":\"a b\"}\nCALL 2 bash : ls\nCALL 3 self_compact : \nEND\n"
    : String}

# LAW: both families send the wire names, in tools[] and in the
# history's calls: an old session's top-level self.compact call
# replays as self_compact.
law openai_body_sends_wire_names:
  {Bool.and(String.contains(Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "\"name\":\"github_search_issues\""),
    Bool.and(String.contains(Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "\"name\":\"self_compact\""),
     Bool.not(String.contains(Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "self.compact"))))
    == True{} : Bool}

law anthropic_body_sends_wire_names:
  {Bool.and(String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "\"name\":\"github_search_issues\""),
     String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "\"name\":\"self_compact\""))
    == True{} : Bool}

# ---- the OpenAI Responses family (BISE-147) ----
# OpenAI's gpt-6 models take tools with reasoning on /v1/responses only.
# Stateless like OpenCode and Codex: store false, the encrypted
# reasoning back (include), the whole conversation replayed with this
# family's own reasoning items; reasoning {effort, summary auto}.

law resp_family_name_and_endpoint:
  {Bool.and(String.eq(Api.api_name(Api.OpenAIResp{}), "openai-responses"),
    Bool.and(String.eq(Api.endpoint(Api.OpenAIResp{}), "/responses"),
      Api.streams(Api.OpenAIResp{}))) == True{} : Bool}

law resp_style_of_api:
  {Api.style_of_api("openai-responses") == Some{Api.OpenAIResp{}}
    : Maybe<&2, Api.Style>}

# LAW: the body: stateless, reasoning at the call's effort with its
# summary, the encrypted reasoning asked back; the system prompt is a
# developer message; the history's own reasoning replays (ENC1) as a
# reasoning item before its text and call, the call and its output
# pair on call_1; Anthropic's signed span is dropped (not replayed as
# text either); the tools are functions, strict off.
law resp_body_maps:
  {Api.api_body_for(Api.OpenAIResp{}, "gpt-6.1-sol", Api.MFacts{True{}, 0n, "max_tokens", "", "", "medium", False{}},
      W.parse_wire("MODEL default\nTOOL bash : Run a command.\nMSG system : Be brief.\nMSG user : hi\nMSG assistant : <think>plan\\nBENDSIG::oair:ENC1</think>\\nok\n  CALL 1 bash : ls\nMSG tool : a.txt\nMSG assistant : <think>old\\nBENDSIG::anthsig</think>\\nbye\nMSG user : again\nEND\n"))
    == "{\"model\":\"gpt-6.1-sol\",\"store\":false,\"reasoning\":{\"effort\":\"medium\",\"summary\":\"auto\"},\"include\":[\"reasoning.encrypted_content\"],\"input\":[{\"role\":\"developer\",\"content\":\"Be brief.\"},{\"role\":\"user\",\"content\":\"hi\"},{\"type\":\"reasoning\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"plan\"}],\"encrypted_content\":\"ENC1\"},{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"ok\"}]},{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"ls\\\"}\"},{\"type\":\"function_call_output\",\"call_id\":\"call_1\",\"output\":\"a.txt\"},{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"bye\"}]},{\"role\":\"user\",\"content\":\"again\"}],\"tools\":[{\"type\":\"function\",\"name\":\"bash\",\"description\":\"Run a command.\",\"parameters\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"},\"description\":{\"type\":\"string\",\"description\":\"One short line shown to the user while this runs: what you are doing, in the user's language. 8 words at most, starting lowercase, no final period. E.g. 'weighing the hero image', 'je lance les tests du signup'.\"}},\"required\":[\"arg\"]},\"strict\":false}],\"tool_choice\":\"auto\"}"
    : String}

# LAW: a model the catalog does not mark as reasoning sends no
# reasoning, no include; an output cap rides as max_output_tokens.
law resp_body_plain_model:
  {Api.api_body_for(Api.OpenAIResp{}, "m", Api.MFacts{False{}, 64n, "max_tokens", "", "", "", False{}},
      W.parse_wire("MODEL default\nMSG user : hi\nEND\n"))
    == "{\"model\":\"m\",\"store\":false,\"max_output_tokens\":64,\"input\":[{\"role\":\"user\",\"content\":\"hi\"}]}"
    : String}

# LAW: the Anthropic family never replays an OpenAI reasoning span
# (its signature is not Anthropic's).
law anthropic_drops_resp_reasoning:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nTOOL bash : Run a command.\nMSG system : Be brief.\nMSG user : hi\nMSG assistant : <think>plan\\nBENDSIG::oair:ENC1</think>\\nok\n  CALL 1 bash : ls\nMSG tool : a.txt\nMSG assistant : <think>old\\nBENDSIG::anthsig</think>\\nbye\nMSG user : again\nEND\n")), "oair:")
    == False{} : Bool}

# LAW: a whole response: the reasoning item is a think span signed
# "oair:<encrypted_content>", the message text follows, a
# function_call is a CALL line (its args flattened as for Chat).
law resp_reply_maps:
  {Rs.reply_of(200, "{\"id\":\"resp_1\",\"status\":\"completed\",\"error\":null,\"output\":[{\"type\":\"reasoning\",\"id\":\"rs_1\",\"encrypted_content\":\"ENC2\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"think\"}]},{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"hello\"}]},{\"type\":\"function_call\",\"call_id\":\"call_x\",\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"pwd\\\"}\"}]}")
    == "OK <think>think\\nBENDSIG::oair:ENC2</think>\\nhello\nCALL 1 bash : pwd\nEND\n" : String}

# LAW: a stream is read from its terminal event (the whole response
# rides in response.completed); the deltas before it are not needed.
law resp_stream_reads_completed:
  {Rs.reply_any("event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"hel\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"sequence_number\":9,\"response\":{\"error\":null,\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"hello\"}]}],\"usage\":{\"input_tokens\":12,\"input_tokens_details\":{\"cached_tokens\":8},\"output_tokens\":3}}}\n\n")
    == "OK hello\nEND\n" : String}

# LAW: ChatGPT's plan route (chatgpt.com/backend-api/codex/responses)
# ends its stream with a response.completed whose output is [] (status
# and usage only): the items come in the response.output_item.done
# events, in order, as Codex reads them (codex-api sse/responses.rs).
# The reply is read from those, the usage still from response.completed
# (issue 15: every plan turn read as an empty response).
law resp_stream_items_when_completed_empty:
  {Rs.reply_any("event: response.output_item.done
data: {\"type\":\"response.output_item.done\",\"sequence_number\":3,\"output_index\":0,\"item\":{\"type\":\"reasoning\",\"id\":\"rs_1\",\"encrypted_content\":\"ENC2\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"think\"}]}}

event: response.output_text.delta
data: {\"type\":\"response.output_text.delta\",\"delta\":\"hel\"}

event: response.output_item.done
data: {\"type\":\"response.output_item.done\",\"sequence_number\":7,\"output_index\":1,\"item\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"hello\"}]}}

event: response.output_item.done
data: {\"type\":\"response.output_item.done\",\"sequence_number\":9,\"output_index\":2,\"item\":{\"type\":\"function_call\",\"call_id\":\"call_x\",\"name\":\"bash\",\"namespace\":\"bise\",\"arguments\":\"{\\\"arg\\\":\\\"pwd\\\"}\"}}

event: response.completed
data: {\"type\":\"response.completed\",\"sequence_number\":10,\"response\":{\"id\":\"resp_1\",\"status\":\"completed\",\"error\":null,\"output\":[],\"usage\":{\"input_tokens\":12,\"output_tokens\":3}}}

")
    == "OK <think>think\\nBENDSIG::oair:ENC2</think>\\nhello
CALL 1 bash : pwd
END
" : String}

# LAW: an error event, or a stream cut before its end, is an error
# reply (the retry policy retries a 200 error).
law resp_stream_error:
  {Bool.and(String.eq(Rs.reply_any("event: error\ndata: {\"type\":\"error\",\"code\":\"server_error\",\"message\":\"boom\"}\n\n"),
      "ERROR provider 200: boom\nEND\n"),
    String.starts_with(Rs.reply_any("event: response.created\ndata: {\"type\":\"response.created\"}\n\n"),
      "ERROR provider 200: the stream ended")) == True{} : Bool}

# LAW: a refused request keeps OpenAI's own words (BISE-293).
law resp_status_error:
  {Rs.reply_of(400, "{\"error\":{\"message\":\"Invalid value.\",\"type\":\"invalid_request_error\"}}")
    == "ERROR provider 400: Invalid value.\nEND\n" : String}

# LAW: the usage: input_tokens counts the cached part already.
law resp_usage:
  {Api.usage_of(Api.OpenAIResp{},
      Rs.usage("data: {\"type\":\"response.completed\",\"response\":{\"output\":[],\"usage\":{\"input_tokens\":12,\"input_tokens_details\":{\"cached_tokens\":8},\"output_tokens\":3}}}\n"))
    == Api.Usage{12n, 3n, 8n, 0n} : Api.Usage}

# ---- the ChatGPT plan route (shape = "chatgpt-plan", docs/subscriptions-design.md) ----
# OpenAI's plan route (developers.openai.com/siwc, preview limitations)
# takes store false and stream true, refuses an output cap and the
# other fields below, a role "system" item, and function tools outside
# a namespace. The fixture: a reasoning model WITH an output cap in the
# catalog (the plan body must drop it), a system prompt, a tool, a call
# and its output in the history.

def plan.req() -> W.WReq:
  W.parse_wire("MODEL default
TOOL bash : Run a command.
MSG system : Be brief.
MSG user : hi
MSG assistant : <think>plan\\nBENDSIG::oair:ENC1</think>\\nok
  CALL 1 bash : ls
MSG tool : a.txt
MSG user : again
END
")

# what goes on the wire: the family's stream body of the plan body
def plan.body() -> String:
  Api.wire_body(Api.OpenAIResp{}, Api.api_body_for(Api.OpenAIResp{}, "gpt-6.1-sol",
    Api.MFacts{True{}, 4096n, "max_tokens", "", "", "medium", True{}}, plan.req()))

# LAW: the plan body, whole: streamed, not stored, no output cap though
# the catalog sets one, the system prompt a developer message, the tool
# inside the "bise" namespace, the replayed call carrying its namespace.
law resp_plan_body_maps:
  {plan.body()
    == "{\"stream\":true,\"model\":\"gpt-6.1-sol\",\"store\":false,\"reasoning\":{\"effort\":\"medium\",\"summary\":\"auto\"},\"include\":[\"reasoning.encrypted_content\"],\"input\":[{\"role\":\"developer\",\"content\":\"Be brief.\"},{\"role\":\"user\",\"content\":\"hi\"},{\"type\":\"reasoning\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"plan\"}],\"encrypted_content\":\"ENC1\"},{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"ok\"}]},{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"ls\\\"}\",\"namespace\":\"bise\"},{\"type\":\"function_call_output\",\"call_id\":\"call_1\",\"output\":\"a.txt\"},{\"role\":\"user\",\"content\":\"again\"}],\"tools\":[{\"type\":\"namespace\",\"name\":\"bise\",\"description\":\"bise's tools\",\"tools\":[{\"type\":\"function\",\"name\":\"bash\",\"description\":\"Run a command.\",\"parameters\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"},\"description\":{\"type\":\"string\",\"description\":\"One short line shown to the user while this runs: what you are doing, in the user's language. 8 words at most, starting lowercase, no final period. E.g. 'weighing the hero image', 'je lance les tests du signup'.\"}},\"required\":[\"arg\"]},\"strict\":false}]}],\"tool_choice\":\"auto\"}"
    : String}

# LAW: present: "stream": true first, "store": false.
law resp_plan_streams_unstored:
  {Bool.and(String.starts_with(plan.body(), "{\"stream\":true,"),
    String.contains(plan.body(), "\"store\":false,")) == True{} : Bool}

# LAW: absent: every field the plan route refuses, as a key.
law resp_plan_no_refused_fields:
  {List.any(&2, String, k => String.contains(plan.body(), k),
    ["\"max_output_tokens\":", "\"temperature\":", "\"top_p\":", "\"metadata\":",
      "\"user\":", "\"prompt_cache_retention\":", "\"safety_identifier\":",
      "\"truncation\":", "\"previous_response_id\":", "\"background\":",
      "\"conversation\":", "\"max_tool_calls\":", "\"top_logprobs\":",
      "\"moderation\":", "\"prompt\":"])
    == False{} : Bool}

# LAW: absent: a role "system" item (the system prompt is a developer
# message); present: "tools" opens on the "bise" namespace (the
# function tools sit in it: resp_plan_body_maps pins the whole list).
law resp_plan_no_system_item_no_bare_tool:
  {Bool.or(String.contains(plan.body(), "\"role\":\"system\""),
    Bool.not(String.contains(plan.body(),
      "],\"tools\":[{\"type\":\"namespace\",\"name\":\"bise\",")))
    == False{} : Bool}

# LAW: the namespace round trip: a call the plan route sends back
# inside the "bise" namespace is a CALL line with the tool's own name,
# and that name maps back to the harness tool of the request.
law resp_plan_namespaced_call_back:
  {W.reply_names_in(W.req_tools(plan.req()),
      Rs.reply_any("event: response.completed
data: {\"type\":\"response.completed\",\"response\":{\"error\":null,\"output\":[{\"type\":\"function_call\",\"call_id\":\"call_x\",\"name\":\"bash\",\"namespace\":\"bise\",\"arguments\":\"{\\\"arg\\\":\\\"pwd\\\"}\"}]}}

"))
    == "OK 
CALL 1 bash : pwd
END
" : String}

# LAW: without the shape, the body is today's: no namespace, no
# namespace on a replayed call, the input items and tools as before
# (resp_body_maps and resp_body_plain_model pin today's whole bodies).
law resp_no_shape_items_unchanged:
  for xs: List<&2, J.Json>
  {Rs.items_for(False{}, xs) == xs : List<&2, J.Json>}

law resp_no_shape_tools_unchanged:
  for ts: List<&2, W.WTool>
  {Rs.tools_for(False{}, ts) == Rs.tools_json(ts) : List<&2, J.Json>}

law resp_no_shape_no_namespace:
  {String.contains(Api.api_body_for(Api.OpenAIResp{}, "gpt-6.1-sol",
      Api.MFacts{True{}, 4096n, "max_tokens", "", "", "medium", False{}}, plan.req()),
    "namespace") == False{} : Bool}

# LAW: the shape comes from the models file: providers.<p>.shape =
# "chatgpt-plan" is the plan; absent, or another word, is not.
law plan_shape_from_models_file:
  {Bool.and(Api.facts_plan(Pv.facts(Cf.parse_toml("[providers.chatgpt]
api = \"openai-responses\"
shape = \"chatgpt-plan\"
"), "chatgpt", "chatgpt/gpt-6.1-sol", "")),
    Bool.not(Bool.or(Api.facts_plan(Pv.facts(Cf.parse_toml("[providers.openai]
api = \"openai-responses\"
"), "openai", "openai/gpt-6.1-sol", "")),
      Api.facts_plan(Pv.facts(Cf.parse_toml("[providers.chatgpt]
shape = \"chatgpt\"
"), "chatgpt", "chatgpt/gpt-6.1-sol", "")))))
    == True{} : Bool}

# ---- the plan's errors, one line each (design, final words §5) ----

# LAW: the usage limit mid-stream (response.failed, a 200) ends the
# turn on bise's line, no retry.
law plan_limit_mid_stream:
  {Pv.attempt_plan(True{}, 200,
      Api.error_code(Api.OpenAIResp{}, "event: response.output_text.delta
data: {\"type\":\"response.output_text.delta\",\"delta\":\"hel\"}

event: response.failed
data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"subscription_sharing_usage_limit_exceeded\",\"message\":\"limit\"},\"output\":[]}}

"),
      Pv.ARetry{0, "provider 200: limit"})
    == Pv.AFail{"your ChatGPT plan's limit for bise is reached. your usage is at chatgpt.com/settings/usage, or switch model with /model."}
    : Pv.Attempt}

# LAW: the usage limit before the stream (a 429 with OpenAI's error
# object): the same line, not a retry.
law plan_limit_429:
  {Pv.attempt_plan(True{}, 429,
      Api.error_code(Api.OpenAIResp{}, "{\"error\":{\"code\":\"subscription_sharing_usage_limit_exceeded\",\"message\":\"limit\",\"type\":\"usage_limit\"}}"),
      Pv.attempt_status(429, "", "ERROR provider 429: limit
END
"))
    == Pv.AFail{"your ChatGPT plan's limit for bise is reached. your usage is at chatgpt.com/settings/usage, or switch model with /model."}
    : Pv.Attempt}

# LAW: usage unavailable mid-stream (OpenAI: the usage could not be
# checked, retry later with a bounded backoff): a retry, its cause
# named, not a final line.
law plan_usage_unavailable_mid_stream:
  {Pv.attempt_plan(True{}, 200,
      Api.error_code(Api.OpenAIResp{}, "event: response.failed
data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"subscription_sharing_usage_unavailable\",\"message\":\"off\"},\"output\":[]}}

"), Pv.ARetry{0, "provider 200: off"})
    == Pv.ARetry{0, "ChatGPT could not check your plan's usage (subscription_sharing_usage_unavailable)"}
    : Pv.Attempt}

# LAW: usage unavailable before the stream (a 503): a retry that keeps
# the server's Retry-After.
law plan_usage_unavailable_503:
  {Pv.attempt_plan(True{}, 503,
      Api.error_code(Api.OpenAIResp{}, "{\"error\":{\"code\":\"subscription_sharing_usage_unavailable\",\"message\":\"off\"}}"),
      Pv.attempt_status(503, "5", "ERROR provider 503: off
END
"))
    == Pv.ARetry{5000, "ChatGPT could not check your plan's usage (subscription_sharing_usage_unavailable)"}
    : Pv.Attempt}

# LAW: once the retries are spent on an unchecked usage, the turn ends
# on the line alone (no "provider failed after N attempts:"); any other
# cause keeps today's text.
law plan_usage_unavailable_exhausted:
  {Bool.and(String.eq(Pv.exhausted_for(10n,
        "ChatGPT could not check your plan's usage (subscription_sharing_usage_unavailable)"),
      "ChatGPT couldn't check your plan's usage. try again in a moment, or switch model with /model."),
    String.eq(Pv.exhausted_for(10n, "timeout"), Pv.exhausted_text(10n, "timeout")))
    == True{} : Bool}

# LAW: plan use not allowed for this user or workspace
# (subscription_sharing_user_not_eligible, a 403): final, its line.
law plan_not_eligible:
  {Pv.attempt_plan(True{}, 403,
      Api.error_code(Api.OpenAIResp{}, "{\"error\":{\"code\":\"subscription_sharing_user_not_eligible\",\"message\":\"no\"}}"),
      Pv.AFail{"OpenAI refused the key (403). check it with /setup."})
    == Pv.AFail{"ChatGPT plan use is off for bise. turn it on in your ChatGPT settings, or pick another provider in /provider."}
    : Pv.Attempt}

# LAW: a 401 on a plan provider (OpenAI's {"detail": ...} admission
# body, or any other) is the expired sign-in, never "refused the key".
law plan_401_expired:
  {Pv.attempt_plan(True{}, 401,
      Api.error_code(Api.OpenAIResp{}, "{\"detail\":\"Unauthorized\"}"),
      Pv.AFail{"OpenAI refused the key (401). check it with /setup."})
    == Pv.AFail{"your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt."}
    : Pv.Attempt}

# LAW: the token command failing on a plan provider (signed out, the
# refresh refused) is the expired sign-in too; another provider's
# key_command keeps its line.
law plan_key_command_failed:
  {Bool.and(String.eq(Pv.key_why(True{}, True{}, "chatgpt", ""),
      "your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt."),
    String.eq(Pv.key_why(True{}, False{}, "corp", ""),
      "no corp key: its key_command failed or printed nothing (config.toml)"))
    == True{} : Bool}

# LAW: a plan provider's other attempts are unchanged (a completed
# stream, another error code, another status).
law plan_other_attempts_unchanged:
  for a: Pv.Attempt
  {Pv.attempt_plan(True{}, 400, "invalid_request_error", a) == a : Pv.Attempt}

# LAW: without the shape, no attempt changes, whatever the code.
law plan_no_shape_unchanged:
  for +s: U32
  for +c: String
  for a: Pv.Attempt
  {Pv.attempt_plan(False{}, s, c, a) == a : Pv.Attempt}

# LAW: a refusal names the provider actually used. A provider whose URL
# the user brings (base_url_env: foundry) is named by the host the call
# went to, a gateway behind it included (2026-10-30: "Anthropic (foundry
# proxy) refused..." for a call that went to a gateway); a provider with
# its own URL keeps its display name.
law refusal_names_the_host_of_a_brought_url:
  {[Pv.prov_label("[providers.foundry]
name = \"Anthropic (foundry proxy)\"
base_url_env = \"ANTHROPIC_FOUNDRY_BASE_URL\"
", "foundry", "https://gateway.example.test/anthropic/v1/messages"),
      Pv.prov_label("", "mistral", "https://api.mistral.ai/v1/chat/completions"),
      Pv.refusal(Pv.prov_label("[providers.foundry]
base_url_env = \"ANTHROPIC_FOUNDRY_BASE_URL\"
", "foundry", "https://gateway.example.test/anthropic/v1/messages"), 400,
        "{\"error\":{\"message\":\"messages: text content blocks must be non-empty\"}}", "")]
    == ["gateway.example.test", Pv.prov_display("", "mistral"),
      "gateway.example.test refused the conversation's history (400). gateway.example.test said: \"messages: text content blocks must be non-empty\""]
    : List<&2, String>}
