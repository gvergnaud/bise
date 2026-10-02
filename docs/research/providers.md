# Research: every known AI provider, with a simple config

Arc 2. User request, verbatim: « Je voudrais que tu travailles sur le
support de tous les AI providers connus, je voudrais environ le même
support cross AI provider que ce que Open Code a. je veux que la config
soit simple. »

Read at HEAD `db19ec1`. Research only: no product code changed.
The `~/.bise/` layout is agreed with `research-portable` (see §3.4).

## TL;DR

- Today bise speaks to **2 endpoints, both hard-coded**: the Anthropic
  Messages API through the foundry proxy (streamed) and Mistral's
  Chat Completions (not streamed). The provider is guessed from the model
  name: it starts with `claude` → Anthropic, anything else → Mistral.
- OpenCode gets 75+ providers from 2 things: the **AI SDK** (one adapter
  per wire format) and the **models.dev catalog** (225 providers, 8 279
  models, with limits, capabilities and prices). 184 of the 225
  providers use the same adapter: OpenAI-compatible Chat Completions.
- So "all providers" is **4 wire formats + 1 catalog + 1 key store**,
  not 75 adapters: OpenAI Chat (and every compatible endpoint),
  Anthropic Messages, OpenAI Responses, Google Gemini. Azure, Vertex and
  Bedrock are these same formats with a different URL and auth.
- The config stays one line: `model = "anthropic/claude-sonnet-4-5"` in
  `~/.bise/config.toml`. The key comes from the env var the catalog
  names (`ANTHROPIC_API_KEY`) or from `bise login anthropic`
  (`~/.bise/auth.json`). Local Ollama / LM Studio work with no config.
- Wire mapping stays in Bend (where it is today, pinned by the laws).
  Rust does the catalog download, the key store and login, and the
  model picker in the TUI.
- Plan: 14 issues, BISE-140..153, about 80 h. The first one splits
  `core/api.bend` by family so that the families can then run in
  parallel worktrees.

---

## 1. What exists today (file:line)

### 1.1 The provider table: 2 providers, hard-coded

| What | Where | Today |
|---|---|---|
| Alias | `runtime/provider-pure.bend:18-26` | `opus-5.5` → `claude-opus-5-5`, the only alias |
| Wire family | `runtime/provider-pure.bend:28-36` | `model_style`: name starts with `claude` → `Anthropic`, else `OpenAI` |
| URL | `runtime/provider-pure.bend:38-45` | Anthropic → a private proxy (`foundry`), `…/anthropic/v1/messages`; OpenAI → `https://api.mistral.ai/v1/chat/completions` |
| URL override | `runtime/provider-pure.bend:47-55` | `BEND_PROVIDER_URL` (used by the tests' fake provider) |
| Key env var | `runtime/provider-pure.bend:57-62` | `ANTHROPIC_FOUNDRY_API_KEY` or `MISTRAL_API_KEY` |
| The 2 families | `core/api.bend:1081-1083` | `type Style = OpenAI \| Anthropic` |
| Model choice | `runtime/provider.bend:613-620` | `BEND_MODEL` > `model` in config > `claude-opus-5-5` |
| The call | `runtime/provider.bend:622-630` | model → style → key env → call; empty key → "X is not set" (`:600-611`) |

So a model id like `gpt-5` goes to **Mistral's URL** with the Mistral
key. There is no base URL, no provider id, no way to add one without
editing Bend.

### 1.2 Wire format and streaming

- The Core never calls a provider. It prints a line-based request
  (`MODEL / TOOL / MSG / CALL / END`) and reads back
  `OK <text> / CALL <id> <name> : <args> / END` (`runtime/remote.bend:6-22`).
  `core/api.bend` parses that request (`parse_wire`, `:313`) and maps it
  to a JSON body per family (`api_body_for`, `:1631`), then maps the
  reply back (`reply_of_for`, `:1638`). This is a good seam: every new
  family is one more pair of pure functions.
- **OpenAI Chat body** (`core/api.bend:864-877`): `model`,
  `reasoning_effort: "high"` on **every** call (`:860-874`), `messages`,
  `tools`, `tool_choice: "auto"`. No `max_tokens`, no `stream`.
  Tool-call ids are `call_<n>` (`:318`). Reply: `choices[0].message`,
  content string or GLM-style block array with nested `thinking`
  (`:995-1020`). No `reasoning_content` (DeepSeek, Kimi, Qwen, GLM on
  their own APIs put the reasoning there): it is dropped.
- **Anthropic body** (`core/api.bend:1464-1489`): `system` block,
  `max_tokens: 32768` fixed, `thinking: {type: adaptive, display:
  summarized}` and `output_config.effort: high` fixed (`:1392-1397`),
  tools as `input_schema`, `self.*` tool names renamed `self_*` on the
  wire (`:1403-1437`). Thinking is replayed with its signature through a
  `BENDSIG::` line in the history (`:1070-1079`).
- **Headers** (`runtime/provider.bend:65-84`): OpenAI → `Authorization:
  Bearer`; Anthropic → `x-api-key`, `anthropic-version: 2023-06-01` and 4
  beta flags on every call (interleaved thinking, fine-grained tool
  streaming, prompt caching, 1M context).
- **Streaming**: only Anthropic streams (`runtime/provider.bend:413-428`,
  `sse.call :396`, `whole.call :405`). The SSE reader in
  `runtime/provider.bend:270-395` is generic (it collects raw text); the
  event fold is Anthropic-only (`core/anth-stream.bend:257`,
  `reply_any :344`). OpenAI-style calls wait for the whole reply with a
  600 s timeout (`runtime/provider-pure.bend:222`).
- **Retries** (`runtime/provider-pure.bend:190-389`): provider-neutral
  already (429, 408, 409, 425, 5xx, 529, error bodies in a 200,
  Retry-After). Keep as is.

### 1.3 Tokens, context, cost

- Usage line per call (`runtime/usage-pure.bend:20-34`): Anthropic
  (`input + cache_read + cache_write`) and OpenAI (`prompt_tokens`,
  `prompt_tokens_details.cached_tokens`). No cost.
- Context window: hard-coded in the TUI (`rust/tui/src/usage.rs:74-85`):
  `claude*` → 1M, `*glm*` → 200k, 3 Mistral prefixes → 128k, else
  unknown.
- Compaction threshold: one number, 800 000 estimated tokens by default
  (`runtime/settings-pure.bend:14-15`), estimate = bytes / 4
  (`core/estimate.bend:7`). **Bug for other models**: on a 128k or 200k
  model the provider refuses the request long before bise compacts.
- Output limit: 32 768 fixed for Anthropic, none for OpenAI.
- Prompt caching: the beta header is sent, but **no `cache_control`
  breakpoint** exists in the body (grep: none in `core/`). Any cache hit
  today comes from the proxy or the provider's automatic caching.

### 1.4 Images

- 2 shapes: `image_url` data URL for OpenAI, `image/base64` source for
  Anthropic (`core/image.bend:285-306`, `IStyle = IOai | IAnth`).
  No Gemini `inline_data`.
- No capability check before sending: the TUI only reacts after the
  provider refuses, by matching words in the error ("image", "vision")
  (`rust/tui/src/attach.rs:360-400`), then tells the user to use
  `/model`, **a command that does not exist** (`rust/tui/src/commands.rs:18-63`
  lists `/compact /interrupt /reload /plugins /status /clear /voice
  /help /shortcuts /quit`).

### 1.5 Model choice, onboarding, keys

- No `/model` command. The model changes only by editing
  `~/.bend-harness/config.toml` (`runtime/settings.bend:15-24`, template
  `runtime/settings-pure.bend:6-20`), `BEND_MODEL`, or
  `bend-harness --model NAME` (`rust/harness/src/main.rs:400`). The model
  is re-read on every call (`runtime/provider.bend:613`), so a config
  edit takes effect at the next call.
- Onboarding's model step (`rust/tui/src/onboarding.rs:102-175`,
  `:760-830`) knows 2 providers (`Provider::Claude | Mistral`), mirrors
  `model_style` by hand (`:127-134`), and only checks/saves a key: it does
  not change the model (it says "set model in
  ~/.bend-harness/config.toml", `:821`).
- Keys: environment, then `~/.bend-harness/.env`, then `~/.vibe/.env`
  (`rust/harness/src/main.rs:161-185`, `load_env_files`). A pasted key
  goes to `~/.bend-harness/.env` (`onboarding.rs:150-153`). No per-provider
  store, no login command.
- The config parser is a TOML subset that already supports `[section]`
  (`core/config.bend:1-13`). Good enough for provider tables.

### 1.6 Tests

- `tests/fake_provider.py`: a scripted
  **OpenAI Chat** server only (`e2e.py:51` points `BEND_PROVIDER_URL` at
  `/v1/chat/completions`). It answers whole JSON, never SSE.
- The laws pin the OpenAI mapping with GLM (`LAWS.bend:853-880`) and the
  Anthropic stream fold (`LAWS.bend` imports `core/anth-stream.bend`).
- No test for a real provider other than the live smoke on the user's
  own key.

---

## 2. What OpenCode does (checked 2025-09-29 on opencode.ai/docs and models.dev/api.json)

- **Providers** come from **models.dev** (`https://models.dev/api.json`,
  5.2 MB today): 225 providers, 8 279 models. Each provider has `id`,
  `env` (key env var names), `npm` (which AI SDK adapter), `api` (base
  URL for compatible ones). Each model has `limit.context`,
  `limit.output`, `tool_call`, `reasoning`, `reasoning_options`,
  `modalities.input` (image, pdf, audio), `attachment`, `temperature`,
  `cost` (input, output, cache_read, cache_write per 1M tokens).
  Adapter count across the 225 providers: `openai-compatible` 184,
  `anthropic` 8, `openai` 6, then 1-2 each (google, google-vertex,
  amazon-bedrock, azure, mistral, xai, groq, cerebras, togetherai,
  openrouter, cohere, perplexity…). Of the 8 279 models, 1 048 have no
  tool calling (useless for an agent: hide them).
- **Config**: `opencode.json(c)`; global `~/.config/opencode/`, then
  project `opencode.json`, merged. The core line is
  `"model": "anthropic/claude-sonnet-4-5"` (`provider_id/model_id`), plus
  `small_model` for titles. Per provider: `options.baseURL`,
  `options.apiKey` (`{env:VAR}` or `{file:path}`), `headers`,
  `timeout`, `whitelist`/`blacklist`, `models.<id>.options`
  (`reasoningEffort`, `thinking.budgetTokens`…), `models.<id>.limit`.
  A custom provider = `npm: "@ai-sdk/openai-compatible"` + `baseURL` +
  model ids.
- **Keys**: `/connect` (TUI) or `opencode auth login`, stored in
  `~/.local/share/opencode/auth.json`; `opencode auth list`. Env vars
  from models.dev work with no login. Bedrock / Vertex use the cloud
  SDK credential chain. Subscriptions by OAuth: GitHub Copilot, ChatGPT
  Plus/Pro, GitLab Duo (Claude Pro/Max through a plugin).
- **Local models**: LM Studio, Ollama, llama.cpp, Atomic Chat are
  OpenAI-compatible providers with a local `baseURL`; the model ids must
  match `GET /v1/models`, limits are set by hand.
- **Model picker**: `/models` lists the models of every provider that
  has a key. **Variants** (low/high reasoning) cycle with a key.
- **Per-provider quirks** (in OpenCode's `provider/transform.ts`; from
  its source, not re-checked today): Anthropic `cache_control` on the
  first 2 system blocks and the last 2 messages; Claude refuses empty
  text blocks, so they are removed; Mistral tool-call ids normalized to
  9 alphanumerics; Gemini tool schemas cleaned (no `$ref`, integer
  enums as strings); default temperature per model family; OpenAI
  Responses with `store: false` and `reasoning.encrypted_content` to
  replay reasoning; `max_tokens` capped by the catalog's `limit.output`.

---

## 3. Gaps, and the target design

### 3.1 Gaps (what blocks "all providers")

1. No provider id and no base URL: the family is guessed from the model
   name, the URL and the key are fixed (§1.1).
2. OpenAI Chat is not streamed: long replies from any compatible
   provider hit the same 255 s silence that forced Anthropic streaming.
3. OpenAI Chat quirks missing: `reasoning_content`, `max_tokens` vs
   `max_completion_tokens`, `reasoning_effort` sent to models that
   refuse it, `stream_options.include_usage`, tool-id format.
4. No OpenAI Responses API (GPT-5 / codex models give their best
   there, and some are Responses-only).
5. No Gemini format (images as `inline_data`, `thoughtSignature` replay
   required by Gemini 3 tool calls).
6. No catalog: context window, output limit, image support, reasoning
   support and price are guessed or absent (§1.3, §1.4). The 800k
   compaction threshold is wrong for most non-Claude models.
7. Anthropic direct (`api.anthropic.com`, `ANTHROPIC_API_KEY`) is not
   possible: only the foundry proxy. Thinking and effort are fixed for
   opus; older / smaller Claude models need `budget_tokens` or no
   thinking.
8. No `/model`, onboarding cannot pick a model, no key store per
   provider, no login command.
9. Reasoning replay is Anthropic-only (`BENDSIG::`). A mid-session switch
   to another family must drop foreign signed blocks cleanly.
10. Tests: the fake provider speaks one format, whole replies only.

### 3.2 The families (wire formats) to implement

| Family (`api =`) | Covers | Auth | Notes |
|---|---|---|---|
| `openai-chat` | OpenAI (old models), **every OpenAI-compatible endpoint**: OpenRouter, Groq, Together, Fireworks, DeepSeek, xAI, Mistral, Cerebras, DeepInfra, Z.ai, Moonshot, MiniMax, Nebius, Hugging Face, Vercel/Cloudflare gateways, Gemini's compat endpoint, Ollama, LM Studio, llama.cpp, vLLM | `Bearer` | Already exists; add SSE, quirks (§3.1.3) |
| `anthropic` | Anthropic, the foundry proxy, Vertex-Anthropic, Bedrock-Anthropic (invoke), and the "anthropic-compatible" endpoints (Z.ai, MiniMax, Kimi, DeepSeek offer one) | `x-api-key` or `Bearer` | Already exists; make thinking/limits per model, add `cache_control` |
| `openai-responses` | OpenAI GPT-5 / codex / o-series, Azure OpenAI | `Bearer` / `api-key` | New: `input` items, `reasoning.encrypted_content` replay, its own SSE events |
| `gemini` | Google AI Studio, Vertex Gemini | `x-goog-api-key` / OAuth token | New: `contents/parts`, `functionDeclarations`, `thoughtSignature`, `streamGenerateContent?alt=sse` |
| `bedrock-converse` | Bedrock non-Anthropic models | Bedrock API key (`AWS_BEARER_TOKEN_BEDROCK`) first; SigV4 later | Last, optional |

Cloud wrappers are only URL + auth over these: **Azure** = openai-chat
or openai-responses on `https://<resource>.openai.azure.com/openai/v1/`
with an `api-key` header. **Vertex** = gemini or anthropic on a
regional URL with a token from `gcloud auth print-access-token`.
**Bedrock** = anthropic (invoke) or converse, Bedrock API key.

Day 1 shortcut: Gemini works through its OpenAI-compatible endpoint
(`generativelanguage.googleapis.com/v1beta/openai/`) as soon as
`openai-chat` is solid; the native family comes after.

### 3.3 Where the code lives

**Wire mapping stays in Bend** (`core/` pure, `runtime/provider.bend`
IO), one module per family:
`core/wire.bend` (the line request, today in api.bend), `core/oai-chat.bend`,
`core/anthropic.bend` (+ `anth-stream.bend`), `core/oai-responses.bend`,
`core/gemini.bend`. Reasons: it is already there and pinned by the laws;
each family is pure (body in, reply lines out) so it is testable in
LAWS without the network; the Bend HTTP lib already streams.

**Rust does what Bend is bad at** (in `rust/harness`, shared with the
TUI): download and slim the models.dev catalog (5 MB of JSON), the key
store and `bise login`, the `/model` picker and onboarding, and later
request signing (the Bend libs have no SHA-256/HMAC/RSA: no SigV4, no
Google service-account JWT).

Not chosen: running the Vercel AI SDK inside `bend-jsrt` (deno_core).
It would give OpenCode's exact coverage, but it puts npm, a module
loader and network I/O inside the sandbox runtime, and a second agent
loop's worth of logic outside the laws. Worth revisiting only if the 4
families turn out to be too much work.

**Hand-over Rust → Bend**: the Bend runtime keeps re-reading
`config.toml` on each call (so `/model` is instant), and reads one small
per-provider catalog file written by Rust:
`~/.bise/cache/providers/<id>.toml` (family, base URL, env names, and
per model: context, output, tools, reasoning, image, prices). One file
read by path, no 5 MB parse in Bend. Keys: env var first (names from the
catalog), then `~/.bise/auth.json` (small JSON, `vendor/json.bend`).

**Neutral model inside**: the Core does not change. The reasoning
marker generalizes `BENDSIG::` to `BENDSIG:<family>::<opaque>` so each
family replays only its own signed blocks (Anthropic signature, Gemini
`thoughtSignature`, OpenAI `encrypted_content`) and turns foreign ones
into plain text or drops them.

### 3.4 The config (simple)

Layout agreed with `research-portable`: one root `~/.bise/`
(`BISE_HOME` overrides), `config.toml`, `auth.json` (0600),
`cache/`. Old `~/.bend-harness/config.toml` and `.env`, `~/.vibe/.env`
are read as fallbacks and migrated once.

The whole config for 95 % of users:

```toml
model = "anthropic/claude-sonnet-4-5"
```

The key: `ANTHROPIC_API_KEY` in the env (the catalog knows the name), or
`bise login anthropic` once (asks for the key, stores it in
`~/.bise/auth.json`). Local: `model = "ollama/qwen3-coder"` works with no
key and no table (bise probes `localhost:11434` / `:1234` / `:8080`).

Optional, only when needed:

```toml
model = "work/qwen3-coder-480b"
thinking = "high"                     # off | low | medium | high (per family mapping)

[provider.work]                       # any OpenAI-compatible endpoint
base_url = "https://llm.corp.example/v1"
key = "{env:CORP_LLM_KEY}"            # or {file:~/.secrets/k}; default: auth.json
api = "openai-chat"                   # default; openai-responses | anthropic | gemini
context = 128000                      # when the catalog does not know the model

[provider.anthropic]                  # override a known provider
base_url = "https://foundry-proxy.example.com/anthropic/v1"
key_env = "ANTHROPIC_FOUNDRY_API_KEY"
```

Precedence (unchanged rule): env (`BISE_MODEL`, alias `BEND_MODEL`) >
config > default. The default when nothing is set: onboarding asks; in
headless mode, the first provider with a key found, in a fixed order.
The user's current setup becomes a built-in provider `foundry` so
`model = "foundry/claude-opus-5-5"` (and the alias `opus-5.5`) keep
working.

Commands: `bise login [provider]`, `bise logout <provider>`,
`bise models [filter]` (the catalog, only providers with a key, only
models with tool calling), `/model` in the TUI (same list, writes the
`model` line).

---

## 4. Plan

Numbers BISE-140..153 (research-portable uses another range). Hours are
for one agent, gates included. "Own worktree" = can run in parallel in
its own git worktree.

| # | Title | Files it owns | Size | Order |
|---|---|---|---|---|
| BISE-140 | Split `core/api.bend` by family, no behavior change (`wire.bend`, `oai-chat.bend`, `anthropic.bend`; `api.bend` keeps the dispatch). LAWS unchanged and green | `core/api.bend`, new `core/wire.bend`, `core/oai-chat.bend`, `core/anthropic.bend`, `LAWS.bend` imports | 3 h | **first**; must wait for debt-ts (BISE-118) if it touches core |
| BISE-141 | Provider registry in Bend: `provider/model` ids, `[provider.x]` tables, `{env:}`/`{file:}`, family + base URL + key resolution, `foundry` built-in and `opus-5.5` alias; replaces `model_style`/`model_url`/`model_key_env` | `runtime/provider-pure.bend`, `runtime/provider.bend` (resolution part), `core/config.bend`, `runtime/settings*.bend`, LAWS section | 6 h | must wait for 140 |
| BISE-142 | Catalog in Rust: fetch models.dev daily (ETag), ship a snapshot in the build, write `~/.bise/cache/providers/<id>.toml`, drop tool-less models; `bise models` | new `rust/harness/src/catalog.rs`, `rust/harness/src/main.rs` (subcommand), snapshot file | 5 h | own worktree, now |
| BISE-143 | Key store + login: `~/.bise/auth.json` 0600, `bise login/logout/auth list`; `load_env_files` reads it (and the old files) | new `rust/harness/src/auth.rs`, `rust/harness/src/main.rs:161-185` | 3 h | own worktree, now (coordinate main.rs with 142: separate hunks) |
| BISE-144 | OpenAI Chat streaming: generic SSE fold for `chat.completion.chunk` (text, `tool_calls` deltas by index, `reasoning_content`, usage with `stream_options.include_usage`); `call.by_style` streams every family | new `core/oai-stream.bend`, `runtime/provider.bend` (`call.by_style`, `wire_body`), `runtime/usage-pure.bend` | 6 h | must wait for 140 (and 141 for provider.bend) |
| BISE-145 | OpenAI-compatible quirks, driven by the catalog: `reasoning_effort` only for reasoning models, `max_tokens`/`max_completion_tokens` from `limit.output`, `reasoning_content` in/out, tool-id normalization (Mistral), no images when the model has none, extra headers (OpenRouter `HTTP-Referer`/`X-Title`) | `core/oai-chat.bend`, `core/image.bend` | 5 h | must wait for 144 |
| BISE-146 | Anthropic direct and per model: `api.anthropic.com` + `ANTHROPIC_API_KEY`, `cache_control` breakpoints (system, tools, last 2 messages), thinking mode per model (adaptive / `budget_tokens` / off), `max_tokens` from the catalog, beta flags per model | `core/anthropic.bend`, `runtime/provider.bend` (headers only) | 4 h | must wait for 141; own worktree vs 144/145 |
| BISE-147 | OpenAI Responses family: body (`input` items, `function_call`/`function_call_output`), `store: false` + `include: [reasoning.encrypted_content]`, SSE events (`response.output_text.delta`, `response.function_call_arguments.delta`, `response.completed`), usage; `BENDSIG:<family>::` generalization | new `core/oai-responses.bend`, `core/api.bend` (dispatch), `core/anthropic.bend` (marker read) | 9 h | must wait for 144 |
| BISE-148 | Gemini native family: `contents/parts`, `systemInstruction`, `functionDeclarations` (schema cleaned), `functionCall`/`functionResponse`, `thoughtSignature` replay, `inline_data` images, `streamGenerateContent?alt=sse`, `usageMetadata` | new `core/gemini.bend`, `core/image.bend` (`IGem`), `runtime/usage-pure.bend` | 8 h | must wait for 144; own worktree vs 147 |
| BISE-149 | Cloud wrappers: Azure (v1 URL, `api-key`), Vertex (Gemini + Anthropic, token from `gcloud auth print-access-token`, cached 50 min), Bedrock (Bedrock API key, Anthropic invoke). SigV4 / service-account JWT out of scope | `runtime/provider.bend` (auth modes), `runtime/provider-pure.bend` | 6 h | must wait for 146, 147, 148 |
| BISE-150 | Model-aware harness: context window, output limit, image support and prices from the catalog; compaction threshold default = 80 % of the model's context (config still wins); cost in the usage line; TUI `context_window` and the no-vision line read the catalog (checked before sending) | `rust/tui/src/usage.rs`, `rust/tui/src/attach.rs`, `runtime/main.bend` (`cfg`), `runtime/usage-pure.bend` | 5 h | must wait for 142 (and 144 for usage-pure) |
| BISE-151 | `/model` picker + onboarding model step: lists providers with a key (env, auth.json, local probe) and their tool-capable models, fuzzy filter, writes `model = …`, offers `login` for a provider without key; replaces the 2-value `Provider` enum | `rust/tui/src/onboarding.rs`, `rust/tui/src/commands.rs`, new `rust/tui/src/model_picker.rs` | 8 h | must wait for 142, 143; must wait for debt-solo (TUI) |
| BISE-152 | Local models: probe Ollama `:11434`, LM Studio `:1234`, llama.cpp `:8080` (`GET /v1/models`), add them to the catalog cache, context from Ollama `/api/show` when known | `rust/harness/src/catalog.rs` (local part) | 3 h | must wait for 142, 145 |
| BISE-153 | Tests per family: the fake provider answers Anthropic SSE, OpenAI Chat SSE, Responses SSE, Gemini SSE (chosen by URL path); recorded real replies as LAWS fixtures; `live_providers.py` runs one tool-call turn per provider whose key is set | `tests/fake_provider.py`, new `tests/live_providers.py`, fixtures | 6 h | own worktree now for the fake server; each family adds its fixtures |
| — | Migration + docs (`~/.bend-harness` → `~/.bise`, `BEND_MODEL` alias, `docs/providers.md`) | with research-portable's migration issue | 3 h | last |

Total ≈ 80 h. Critical path: 140 → 141 → 144 → 147/148 → 149
(≈ 40 h). Parallel from day 1: 142, 143, 153. After 144: 145, 147,
148 in 3 worktrees (distinct files; only the `api.bend` dispatch lines
overlap, a 2-line merge).

What the user gets, by step:
- after 141 + 143: any OpenAI-compatible provider (OpenRouter, Groq,
  DeepSeek, Mistral, local…) with one config line — not streamed yet;
- after 144 + 145 + 146: the same, streamed and correct, plus Anthropic
  direct — ~200 of models.dev's 225 providers;
- after 147 + 148 + 149: OpenAI GPT-5 at its best, Gemini native,
  Azure, Vertex, Bedrock — OpenCode parity except OAuth subscriptions;
- 150 + 151: the model picker and correct limits, which make it feel
  like OpenCode.

---

## 5. Open decisions for the user

1. **Catalog source.** models.dev fetched daily plus a snapshot shipped
   in the build (proposed; like OpenCode), or only a curated built-in
   list (no network, fewer models, we update it by hand)?
2. **Subscriptions (OAuth).** OpenCode logs in with GitHub Copilot and
   ChatGPT Plus/Pro. Proposed: API keys only in this arc; OAuth later,
   provider by provider (Anthropic restricts using a Claude Pro/Max
   login in third-party tools). OK?
3. **Config format.** Keep TOML (`~/.bise/config.toml`, one line
   `model = "provider/model"`) — proposed — or JSON like OpenCode so
   their provider blocks can be pasted?
4. **One model or one per agent.** Proposed: one `model` for every
   agent in this arc; later `sb spawn --model` and a `[agents]` table
   (main on a strong model, tasks on a cheaper one). Or do you want
   per-agent models now?
5. **Cloud auth depth.** Proposed: Azure key, Vertex through `gcloud`,
   Bedrock through its API key. Full AWS SigV4 and Google
   service-account keys (needs signing in Rust) only if you need them.
   Needed now?

## Sources

- Code at `db19ec1`: files and lines cited above.
- https://opencode.ai/docs/providers/, https://opencode.ai/docs/config/,
  https://opencode.ai/docs/models/ (read 2025-09-29).
- https://models.dev/api.json (downloaded 2025-09-29: 225 providers,
  8 279 models, adapter counts computed from the `npm` field).
- OpenCode `packages/opencode/src/provider/transform.ts` (quirks list in
  §2, from memory of the source, not re-read today).

## 6. Decisions (user, 2026-09-29)

1. **Catalog: our own list, no third-party fetch.** bise ships a curated
   built-in model list (a TOML file in the repo, updated by us; no
   models.dev download at runtime). Any model name must also work
   without waiting for us: `model = "provider/any-model-name"` is
   accepted even when it is not in the list (sensible defaults), and the
   user can add or override a model in `~/.bise/config.toml` (e.g. a
   `[models."provider/name"]` table: context window, vision, reasoning).
   Custom providers (an OpenAI-compatible base URL + key env) the same way.
2. **API keys only in this arc**; subscriptions (OAuth) later, provider by
   provider, whatever is most convenient.
3. **TOML** (`~/.bise/config.toml`).
4. **One model for main, and optionally another one for the sub-agents**
   (e.g. `model = "…"` and `agent_model = "…"`; unset = same as main).
5. **Cloud auth, the simple way**: Azure key, Vertex through `gcloud`,
   Bedrock API key; no SigV4 / service-account signing for now.

## 7. BISE-142 as built: the catalog, the config, the hand-off to Bend

### 7.1 The built-in list

`rust/catalog/models.toml`, compiled into the binary (no network, no
third-party service). Edit it by hand; `cargo test -p bise-catalog`
checks it (every provider has a known family, no duplicate, no trailing
`/`). 17 providers: anthropic, foundry (the setup before BISE-142),
openai, google, mistral, openrouter, groq, xai, deepseek, together,
fireworks, cerebras, ollama, lmstudio (local, no key), azure, vertex,
bedrock (`needs = "BISE-149"`: listed, not usable yet), ~50 models.

```toml
default_model = "foundry/claude-opus-5-5"   # nothing configured

[aliases]
"opus-5.5" = "foundry/claude-opus-5-5"

[providers.anthropic]
name = "Anthropic"
api = "anthropic"          # openai-chat | anthropic | openai-responses | gemini | bedrock-converse
base_url = "https://api.anthropic.com/v1"
key_env = "ANTHROPIC_API_KEY"   # "" = no key (local)
needs = ""                      # "BISE-149" = not usable yet
context = 200000                # the defaults of its models
max_output = 32000
vision = true
reasoning = true

[models."anthropic/claude-sonnet-4-5"]
max_output = 64000              # only what differs from the provider
```

A model field not set comes from its provider, then from the defaults:
context 128000, max_output 16384, vision false, reasoning false, tools
true. A model may set its own `api` (a Responses-only model of a chat
provider).

### 7.2 The user's config (`config.toml`)

The same tables, merged key by key over the built-in list; the file
today is `$BEND_CONFIG`, else `~/.bend-harness/config.toml` (the file
runtime/settings.bend reads; `~/.bise/config.toml` once BISE-160's
`bise_home` is wired: one line in `main.rs`, `config_file()`).

```toml
model = "anthropic/claude-sonnet-4-5"     # main
agent_model = "groq/openai/gpt-oss-120b"  # sub-agents; unset = model

# a model the list does not know, or other limits for a known one
[models."anthropic/claude-sonnet-4-5"]
context = 1000000

[models."ollama/qwen3-coder:30b"]
context = 65536
reasoning = true

# a whole OpenAI-compatible provider
[providers.work]
base_url = "https://llm.corp.example/v1"
key_env = "CORP_LLM_KEY"
# api = "openai-chat" (default), context = 131072 (defaults of its models)

# a gateway in front of Anthropic, set up like Claude Code's
# ANTHROPIC_BASE_URL + ANTHROPIC_CUSTOM_HEADERS + apiKeyHelper
[providers.gateway]
api = "anthropic"
base_url = "https://gateway.corp.example/v1"
key_env = ""
reasoning = true
headers_env = "ANTHROPIC_CUSTOM_HEADERS"   # "Name: value" per line
key_command = "corp-tool auth token llm"   # run before every call

[aliases]
fast = "groq/openai/gpt-oss-120b"
```

Rules:
- **Names.** `provider/model`, split at the first `/` (ids may hold `/`:
  `openrouter/anthropic/claude-sonnet-4.5`). A name with no `/`: an
  alias, else the old rule (`claude*` → `foundry/<name>`, anything else
  → `mistral/<name>`), so old configs keep working.
- **Any name works.** A listed model takes its fields; an unlisted one
  its provider's defaults; an unknown provider resolves too (no base
  URL): the provider call says "add a [providers.<p>] table". Nothing
  about a model stops bise from starting.
- **Precedence, per key: env > config > default.**
  `model` = `BISE_MODEL` > `BEND_MODEL` > config `model` > `default_model`;
  `agent_model` = `BISE_AGENT_MODEL` > config `agent_model` > the
  effective `model`. Empty env values are unset.
- **A gateway.** `headers_env` names a variable whose text is one
  `Name: value` per line (Claude Code's `ANTHROPIC_CUSTOM_HEADERS`
  form); each call reads it and sends those headers after the family's.
  `key_command` is a shell command (`/bin/sh -c`) run before every call,
  like Claude Code's `apiKeyHelper`: its stdout, trimmed, is the key (a
  short-lived token is never stale), sent as the family's header and
  as `Authorization: Bearer`; a non-zero exit or no output: no call, one
  line that names the key_command. It wins over `key_env` (set
  `key_env = ""`, so the first run asks no key). No `"` or `\` in it:
  the runtime's reader takes a value as written (single quotes work).
  Both per provider or per model, neither built in.
- **Bad entries are warnings.** An unknown key, a wrong type, a family
  that does not exist, a model name without `/`, `[provider.x]`
  (singular): ignored, listed by `bise models`. A config that is not
  valid TOML (the Bend reader accepts bare words): its tables are
  ignored, `model` / `agent_model` are still read.

`bise models [filter]` (`bend-harness models`): the model and
agent_model in use (with where they come from and whether they are
listed), then per provider: family, where its key comes from (§7.4:
env, `auth.json`, an old `.env` file; never the key), its models with
context / output / vision / reasoning, and `<provider>/<any other>`
with the defaults; the warnings last.

Rust API (`bise_catalog`, for BISE-150/151/152): `Setup::load(path)` /
`Setup::from_text(text, env)` → `setup.model`, `setup.agent_model`,
`setup.model_for("main" | "agent")`; `Catalog::resolve(name)` →
`Resolved { name, provider, id, api, base_url, key_env, needs, caps:
Caps { context, max_output, vision, reasoning, tools }, known }`;
`catalog.context_window(name)` (usage.rs / compaction, BISE-150).

### 7.3 The hand-off to Bend (BISE-141)

- Before it starts REPLs (`sbd`, the hub, and `--headless`), bise
  writes the merged catalog (built-in + config.toml) to
  `<cache>/models.toml` (`~/.bend-harness/cache/models.toml` today; the
  temp dir if that fails; atomic) and exports its path as
  **`BISE_MODELS_FILE`**. No file (an old binary): the runtime keeps its
  built-in foundry + mistral table. The hub writes it again at each REPL
  spawn and at each input and idle (`env_for_spawn` in
  `rust/harness/src/main.rs`): a config.toml or .env edit reaches the
  next call, no hub restart. The hub's own environment is the one it
  started with: a variable exported in a shell after that needs the
  hub's restart (`bise switchboard --stop`), or a .env file.
- Base URLs: config.toml's `base_url` > the provider's `base_url_env`
  variables (the environment, then the .env files: bise's, then
  ~/.vibe/.env) > the built-in one. foundry has `base_url_env =
  "ANTHROPIC_FOUNDRY_BASE_URL"` (Claude Code's name and form: an
  Anthropic-family URL without `/v1` gets it). An empty `base_url` is no
  `base_url`: the file never says `base_url = ""` (it names
  `base_url_env` instead), and the runtime reads `""` as none: the error
  says what to set, never a call to `/messages`.
- The hub sets **`BISE_ROLE=main|agent`** on each REPL
  (`rust/switchboard/src/daemon.rs`, the spawn's env).
- Format: the config's own tables, flat for `core/config.bend` (one
  `key = value` per line, strings quoted, ints and `true`/`false` bare, a
  key's path keeps the quotes: `models."openai/gpt-5".context`):
  `version = 1`, `default_model`, `[aliases]`, `[providers.<id>]` with
  every key (name, api, base_url, key_env, needs, context, max_output,
  vision, reasoning, tools), `[models."<p>/<id>"]` with only the keys
  that differ from the provider. ~300 lines today. The model choice is
  **not** in it.
- Per call, the runtime: name = the precedence of §7.2 (config read
  fresh, so an edited `model` line applies at the next call; the role
  from `BISE_ROLE`); alias / old rule for a bare name; provider = before
  the first `/`; each key = `models."<p>/<id>".<k>`, else
  `providers.<p>.<k>`; unknown provider, `needs` set, or a family not
  built yet: an error at call time. A new `[providers]` / `[models]`
  table in config.toml needs a restart (the file is written at start).

### 7.4 BISE-143 as built: API keys (`auth.json`, login)

API keys only (§6.2; OAuth later). Code: `rust/catalog/src/auth.rs`
(store + resolution, pure), `auth_cli.rs` (the commands), wired in
`rust/harness/src/main.rs` (`auth_paths`, `load_keys`). The command
name is `bise_catalog::CLI` (`bend-harness` until BISE-165: one line).

Commands (`bend-harness …`, later `bise …`):
- `login [provider]`: asks the key with the terminal echo off (no
  provider: a numbered list of the providers that take a key); without
  a terminal it reads the key from stdin (`printf %s "$K" | … login
  openai`). The key is trimmed; empty, or holding a space or a control
  character: refused, nothing saved. Says when the provider's env var
  is set (it wins) and that a running hub keeps the keys it started
  with. Unknown provider / one with no key (ollama): an error. Custom
  providers of config.toml (`key_env`) work.
- `logout [provider]` (no provider: the only stored one); says when
  another source still has a key.
- `auth list` (`auth`, `auth login|logout` too): per provider that
  takes a key, its `key_env` and where the key comes from (`env
  OPENAI_API_KEY`, `auth.json`, `~/.vibe/.env (MISTRAL_API_KEY)`, `-`);
  warns on entries for unknown providers, non-API entries, and an
  auth.json readable by others.
- `models` shows the same source per provider.

`auth.json` = `bise_home::Home::auth_file()` (`<root>/auth.json`:
`~/.bise` in the bise layout, `~/.bend-harness` in the legacy one),
the OpenCode layout `{"<provider>": {"type": "api", "key": "…"}}`;
other entries are kept as they are. Written atomically (temp file in
the same dir, 0600, renamed); its directory created 0700 when missing
(an existing one keeps its mode). A file that is not a JSON object is
an error without its content and is never overwritten.

**Resolution, per provider** (its `key_env` from the catalog, config
included):
1. the environment: `key_env`, then its aliases (`GEMINI_API_KEY` ←
   `GOOGLE_API_KEY`; `auth::ALIASES`); empty = unset;
2. `auth.json` (the provider id);
3. the old `.env` files (`Home::env_files()`: `<root>/.env`, in the
   bise layout `~/.bend-harness/.env`, then `~/.vibe/.env`), first file
   first, `key_env` then its aliases.

**Hand-off.** The Bend runtime reads `getenv(key_env)` on each call
(BISE-141). At start (sbd, `--headless`), `load_keys()` resolves every
provider and sets `key_env` in the process env when the key is not
already there under that name (an alias, auth.json, a .env file); then
the rest of the .env lines as before. The REPLs inherit it. No key is
written to any other file (not the models file, not the session) or
printed; `Found`'s Debug hides it.

**At each REPL spawn** (BISE-146 follow-up (a)): the hub resolves the
keys again (`keys_for_spawn` in `rust/harness/src/main.rs`, handed to
the daemon as `Opts::spawn_env`; pure part `Resolution::spawn_env`):
auth.json, the .env files and config.toml read again, the environment
the hub started with still first, but the variables `load_keys` set
itself do not count as "the environment". Each export is set on the
new REPL; a key_env the hub set at start whose key is gone now
(`logout`) is removed. So a `login` / `logout` reaches the next agent
(spawn, respawn, restart) without restarting the hub; a REPL already
running keeps the env it started with.

**Onboarding** (follow-up (b)): the model step
(`rust/tui/src/onboarding.rs`) reads the catalog (`Setup`: the model in
use and its provider; the providers that take a key and are usable),
finds the keys with `auth::Keys` (env, auth.json, the old .env files),
and saves a pasted key with `auth_cli::login` (auth.json, 0600; asks
before replacing a stored one). It no longer writes `<root>/.env`.

### 7.5 BISE-144 as built: OpenAI Chat streams

- Every `openai-chat` call streams (`Api.streams` is True for both
  families): the body gets `"stream":true` and
  `"stream_options":{"include_usage":true}` in front
  (`Os.stream_body`), the SSE reader of `runtime/provider.bend` reads
  it, `core/oai-stream.bend` folds the chunks into the message a whole
  reply carries (`Os.whole` gives the whole body) and `Oai.reply_ok`
  maps it: the same OK/CALL/END as a non-streamed reply.
- Fold: text pieces; Mistral content blocks (thinking/text);
  `reasoning_content` / `reasoning` kept in the message (not surfaced
  yet); `tool_calls` by index (interleaved; first id/name kept; no
  index = new call when it has an id or name); `finish_reason`; the last
  `usage` (Groq: `x_groq.usage`); an `error` chunk = `ERROR provider
  200: …` (retried); a JSON 200 = the whole mapping.
- Per model (`Api.MFacts`, from the models file): `reasoning_effort`
  only when `reasoning = true`; `max_output` as `max_completion_tokens`
  (provider `openai`) or `max_tokens`; no file = today's body.

### 7.6 BISE-146 as built: Anthropic direct and per model

- **Direct API.** `anthropic/<model>` calls `https://api.anthropic.com/v1/messages`
  with `x-api-key: $ANTHROPIC_API_KEY` and `anthropic-version: 2023-06-01`
  (the Anthropic family's headers, `core/api.bend`). The `foundry` proxy is
  unchanged apart from the cache breakpoints.
- **Catalog keys** (`rust/catalog`, Anthropic family only; model key over
  provider key, config.toml too): `thinking = "adaptive" | "budget" | "none"`
  (another word: a warning), `betas = "<anthropic-beta flags>"`. Built in:
  provider `anthropic` has `thinking = "budget"` and
  `betas = "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14"`
  (the 4.5 models refuse adaptive thinking; no 1M-context flag, it needs
  its own tier); `foundry` sets neither. They reach Bend through the
  models file and `Api.MFacts{…, thinking, betas}` (BISE-144's record).
- **Body** (`core/anthropic.bend`, `api_body_anth_for(model, out,
  thinking, reasoning, req)`):
  - `max_tokens` = the model's `max_output` (0 / no models file: 32768,
    the foundry proxy's);
  - thinking: `adaptive` = `{"type":"adaptive","display":"summarized"}` +
    `output_config: {"effort":"high"}` (today's opus-5.5 body); `budget` =
    `{"type":"enabled","budget_tokens": min(16000, max_tokens / 2)}`, none
    when that is under the API's 1024 minimum; `none` = no field. No word:
    adaptive when `reasoning = true`, else none (the built-in foundry entry
    of `runtime/provider-pure.bend` now says `reasoning = true`, so an old
    binary without the models file keeps today's body);
  - prompt caching, 4 `cache_control: {"type":"ephemeral"}` breakpoints
    (the API's maximum): the system block, the last tool, the last block of
    each of the two newest messages (a thinking block never gets one: the
    API refuses it).
- **Headers**: `Api.headers(st, betas, key)`: the model's `betas`, else the
  foundry list (unchanged bytes).
- **Checked live** (foundry, a bash round trip): opus-5.5, adaptive:
  call 1 `cache_read=0 cache_write=6253`, call 2 `cache_read=6253
  cache_write=73` (the usage line; the TUI reads `cache_read`); Haiku 4.5
  with `thinking = "budget"` (it refuses adaptive): thinking blocks, signed,
  replayed, cache read 5045 on call 2. The direct API itself: no key here;
  the family path is the fake provider's (provider_families.py part D).
- **Laws**: the Anthropic body laws carry the breakpoints; new:
  `anth_cache_breakpoints`, `anth_thinking_none_and_no_cache_on_thinking`,
  `anth_thinking_budget`, `anth_thinking_budget_too_small`,
  `anth_thinking_default_follows_reasoning`,
  `anth_thinking_default_reasons_adaptive`, `anth_thinking_words`,
  `family_headers_anthropic_betas`, `api_body_anth_follows_facts`.

### 7.6 BISE-150 as built: the model's limits and prices

- Threshold: `BEND_THRESHOLD` > config `compaction_threshold` (was
  `threshold` until BISE-300, no longer read) > 80 % of the context
  window of the model the REPL starts with (the models file: the
  model's `context`, else its provider's; nothing: 128000). BISE-300:
  a value is tokens (`450000`) or a share of the window (`"45%"`), and
  never goes above that 80 % (a number set for a 1M model stays safe
  on a 200k agent model). `runtime/provider-pure.bend` `window` /
  `threshold` / `thr_value`, laws `threshold_*` / `window_*`; Rust
  mirror `Catalog::default_threshold`, `compaction_threshold`.
  The config template no longer writes a threshold.
- harness-info and the usage line carry the full `provider/model` id.
- Prices: `input_price`, `output_price`, `cache_read_price`,
  `cache_write_price` (USD per 1M tokens) on models or providers;
  `Resolved.price`, `Price::cost`. Not in the hand-off (Bend does not
  use them).
- TUI (`rust/tui/src/models.rs`): gauge window, cost at the end of the
  usage line, and a message with images to a listed model with
  `vision = false` is stopped before sending (the no-vision line).

## 8. BISE-153 as built: the fake provider, fixtures, live tests

### 8.1 The fake provider (`tests/fake_provider.py`)

One server, four families; the URL path picks the family:

| path | family | streamed when | shapes from |
|---|---|---|---|
| `…/messages` | `anthropic` | `"stream": true` | docs.anthropic.com/en/docs/build-with-claude/streaming, /en/api/messages, /en/api/errors |
| `…/responses` | `openai-responses` | `"stream": true` | platform.openai.com/docs/api-reference/responses-streaming, /responses/object |
| `…/models/<m>:streamGenerateContent?alt=sse` (`:generateContent` whole; no `alt=sse`: a JSON array) | `gemini` | the path | ai.google.dev/api/generate-content, /gemini-api/docs/thought-signatures, /gemini-api/docs/troubleshooting |
| anything else (`…/chat/completions`) | `openai-chat` | `"stream": true` | platform.openai.com/docs/api-reference/chat-streaming, /docs/guides/error-codes; `reasoning_content`: api-docs.deepseek.com/guides/reasoning_model; mid-stream error: openrouter.ai/docs/api-reference/errors |

- Point the harness at it with a custom provider (BISE-141/142): in the
  models file (`BISE_MODELS_FILE`) or config.toml,
  `[providers.fake] api = "anthropic" base_url = "http://127.0.0.1:PORT/v1" key_env = ""`
  and `model = "fake/any"`; the URL is base_url + the family's
  endpoint. `BEND_PROVIDER_URL` still wins (the old tests: e2e, tmux).
- Not streamed = the whole JSON reply; the old openai-chat reply is
  unchanged (e2e and the tmux tests run on it).
- Streams are chunked HTTP/1.1 with one event per chunk. What each
  family's stream carries: Anthropic `message_start` / `ping` /
  `content_block_*` (thinking + `signature_delta`, text, `tool_use` with
  an empty first `input_json_delta`) / `message_delta` (stop_reason,
  usage) / `message_stop`. OpenAI Chat `chat.completion.chunk`: role
  first, `reasoning_content` deltas, content deltas, `tool_calls`
  deltas by `index` (id + name first, then argument pieces),
  finish_reason, then with `stream_options.include_usage` a `choices: []`
  usage chunk (and `usage: null` on the others), `data: [DONE]`.
  Responses: `response.created` / `in_progress`, per output item
  `output_item.added` … `.done` (reasoning summary deltas,
  `output_text.delta`, `function_call_arguments.delta`), every event with
  `sequence_number`, `response.completed` with usage;
  `encrypted_content` on the reasoning item when the request `include`s
  it. Gemini: `data:` chunks ended by CRLF CRLF, thought parts
  (`thought: true`), text parts, whole `functionCall` parts (the first
  with `thoughtSignature`), `finishReason` + `usageMetadata` last.
- The script is in the last real user message: `[[bash: CMD]]` (one call
  per request, as before), `[[think: TEXT]]` (reasoning on every reply),
  `[[error: 429|500|overloaded|stream [xN] [retry=S]]]` (the first N
  requests fail with the family's error body and Retry-After; `stream` =
  a 200 stream that breaks with the family's error event), `[[fixture:
  NAME]]` (the first request gets `tests/providers/<family>/NAME.sse`,
  `NAME.json` or `NAME.<status>.json` byte for byte). Requests of every
  family are read back into one neutral conversation (tool results in
  Anthropic user blocks, `function_call_output`, `functionResponse`),
  so the same script runs on all four.
- `$FAKE_LOG` lines keep `agent, last_user, user, reply, images` and add
  `family, path, stream, status, error, fixture`.
- In-process: `fake_provider.serve()` → `(server, port)`.

### 8.2 The fixtures (`tests/providers/<family>/`)

One folder per family (`anthropic`, `openai-chat`, `openai-responses`,
`gemini`), files named by what they are:

- `NAME.sse`: a streamed reply body, raw bytes as the API sent them;
- `NAME.json`: a whole reply body (Gemini: an object or the array);
- `NAME.<status>.json`: an error body with its HTTP status;
- `index.json`: per file, `source` (recorded: provider/model, date,
  how; or `fake_provider.py fixtures`) and `expect`: what the reply
  says, any of `text`, `reasoning`, `calls` (`[{name, args}]`), `error`
  (a string, or `true` = any error).

`tests/provider_families.py` (in run_all, ~5 s) folds every file with
`tests/provider_folds.py` (one reference fold per family, written from
the docs, independent of the renderers) and checks its `expect`; a file
missing from the index fails. It also checks the renderers against the
folds, the server's paths, markers and errors in each family's request
shape, and a real repl-live on the fake through `[providers.fake]`
(anthropic streamed with thinking and a retried 529; a config.toml model
switch to openai-chat; a gemini model fails cleanly until BISE-148).

Today: `fake-*` for every family (`fake_provider.py fixtures` writes
them: tool call streamed and whole, mid-stream error, 429, overloaded,
500; the test fails when one is stale); recorded:
`anthropic/foundry-tool-call.sse` (the foundry proxy),
`openai-chat/mistral-tool-call.sse` (Mistral sends the whole tool call
in one delta, in the same chunk as finish_reason and usage, plus a `p`
padding field) and `openai-chat/mistral-bad-key.401.json`
(`{"detail": …}`, not OpenAI's `{"error": …}`). No OpenAI, Anthropic
direct or Gemini key here: their folders hold only `fake-*` until
someone with a key runs `--record`.

### 8.3 How a family plugs in (BISE-144, 146, 147, 148)

1. Record real replies: `python3 tests/live_providers.py --record <row>`
   (rows: anthropic, foundry, openai, openai-responses, google, gemini,
   mistral, openrouter, groq, xai, deepseek, together, fireworks,
   cerebras) writes `<family>/<row>-tool-call.sse` and
   `<row>-bad-key.<status>.json` with their index entries. Add by hand
   the cases you need (reasoning, several calls, a refusal), each with
   its `expect`.
2. LAWS fixtures: `python3 tests/fake_provider.py bend <file>`
   prints the file as a Bend string literal; the law asserts the
   family's fold (`core/<family>-stream.bend`) gives the OK/CALL/END
   lines matching the file's `expect` (the same claim the Python fold
   checks). Prefer recorded files; a `fake-*` file only until one exists.
3. If the family streams something the fake does not send yet (a new
   event, a quirk), change its renderer in fake_provider.py, then
   `fake_provider.py fixtures`, and extend its reference fold in
   provider_folds.py when the docs say the fold must read it.
4. The harness test: in `provider_families.py` part D, the gemini (148)
   / responses (147) check turns from "fails cleanly" into a two-call
   bash turn like the anthropic one (add `[providers.fakeresp]`); 144
   makes the openai-chat turn streamed (the check prints `stream=`).
5. Live: `live_providers.py <row>` runs one real bash turn through
   repl-live on each row whose key is set (skipped otherwise; never in
   run_all), with the row's provider in a models file.
