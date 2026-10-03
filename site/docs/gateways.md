---
title: gateways and local models
description: LiteLLM, a company proxy, vLLM, Ollama or LM Studio: any server that speaks OpenAI's or Anthropic's API works as a provider.
---

bise talks to any server that speaks one of three APIs: OpenAI's Chat Completions, Anthropic's Messages, or OpenAI's Responses. so a gateway like [LiteLLM](https://docs.litellm.ai/docs/simple_proxy), a company proxy, vLLM, or any OpenAI-compatible URL works. you add it as a provider in `~/.bise/config.toml`.

## LiteLLM in three steps

1. add the provider:

```toml title="~/.bise/config.toml"
[providers.litellm]
name = "LiteLLM"
api = "openai-chat"
base_url = "http://localhost:4000/v1"
key_env = "LITELLM_API_KEY"
small_model = "mistral-small"   # a cheap model, for titles and summaries

[roles]
main = "litellm/claude-sonnet-4-5"
```

the part after `litellm/` is a `model_name` from your LiteLLM config, as it is. names with a `/` work too: `litellm/anthropic/claude-sonnet-4-5`.

2. give bise the key (your LiteLLM master key or a virtual key). save it:

```sh
bise login litellm
```

or export the variable named by `key_env`:

```sh
export LITELLM_API_KEY=your-litellm-key
```

3. check it:

```sh
bise auth check litellm   # one tiny call
bise providers            # LiteLLM  ✓ ready
```

then start `bise`. `/model litellm/<model>` switches the agent you're looking at, and `bise config set agents litellm/<model>` sets the model of the agents main starts.

## the provider table

`[providers.<id>]`. every key is optional but `base_url`.

| key | what it does | unset |
|---|---|---|
| `name` | the name bise shows | the id |
| `api` | the wire API: `openai-chat` (`/chat/completions`), `anthropic` (`/messages`), `openai-responses` (`/responses`) | `openai-chat` |
| `base_url` | the URL bise adds the path above to: keep the `/v1` | none |
| `key_env` | the variable that holds the key; `""` for a server that needs none | none |
| `small_model` | a cheap model of this provider, for titles and summaries | none |
| `key_command` | a command that prints the key, run before every call | none |
| `headers` | a table of static HTTP headers; model values override provider values | none |
| `headers_env` | a variable that holds extra headers | none |
| `idle_timeout_sec` | how long bise waits for the server to send something, in seconds | 90 |

the key goes out as `Authorization: Bearer <key>` (`openai-chat`, `openai-responses`) or `x-api-key` (`anthropic`). with `key_env = ""` and no `key_command`, bise sends no key.

## model limits

a model bise doesn't know gets these defaults: 128k context, 16k output, no images, no reasoning, tools on. set the real ones per model, or for the whole provider (the same keys in `[providers.<id>]`):

```toml title="~/.bise/config.toml"
[models."litellm/claude-sonnet-4-5"]
context = 1000000
max_output = 64000
vision = true       # it reads images
reasoning = true    # it thinks; /reasoning picks the effort
tools = true

# optional, USD per million tokens: bise then shows the cost
input_price = 3.0
output_price = 15.0
```

the context matters: bise compacts a conversation before it fills the window. a window set too large ends in an error from the server instead.

## gateways behind a login

some gateways want a short-lived token or extra headers, the way Claude Code uses `apiKeyHelper` and `ANTHROPIC_CUSTOM_HEADERS`:

```toml title="~/.bise/config.toml"
[providers.gateway]
name = "Company gateway"
api = "anthropic"
base_url = "https://gateway.example.com/v1"
key_env = ""
key_command = "my-tool auth token"     # its output is the key, run before every call
headers_env = "GATEWAY_HEADERS"        # one "Name: value" per line
reasoning = true
```

```sh
export GATEWAY_HEADERS="x-team: platform"
```

the key from `key_command` goes out both as the API's own header and as `Authorization: Bearer`. a command that fails or prints nothing stops the call with one line that names it. write `key_command` on one line, with no `"` or `\` (single quotes work).

LiteLLM also serves Anthropic's API (`/v1/messages`), so the same LiteLLM works with `api = "anthropic"` when you want Claude's thinking blocks kept as they are.

## Headers in config.toml

Set non-secret routing headers directly in the provider table. This works
without a shell startup file or a modified launcher:

```toml
[providers.gateway]
api = "openai-chat"
base_url = "https://gateway.example.com/v1"
key_env = ""
key_command = "my-tool auth token"
headers = { source = "bise", "x-team" = "platform" }
```

A `[providers.gateway.headers]` table also works. Model-level `headers`
override the provider's headers by name. Names are case-insensitive.
`headers_env`, when configured, overrides both. Keep credentials in
`key_command`, `key_env`, or the key store.

If `headers_env` names an unset or blank variable, Bise stops before the
request and names the variable. Set it before starting Bise and restart
an existing hub, or use `headers` and remove `headers_env`. Invalid header
names, duplicate names, non-string values, and control characters are
reported as configuration warnings without printing header values.

## local models

Ollama (`ollama/<model>`, on `localhost:11434`) and LM Studio (`lmstudio/<model>`, on `localhost:1234`) are built in and need no key:

```sh
bise config set main ollama/qwen3:32b
```

another local server (vLLM, llama.cpp, ...):

```toml title="~/.bise/config.toml"
[providers.vllm]
name = "vLLM"
base_url = "http://localhost:8000/v1"
key_env = ""
context = 32768
```

### a slow server: idle_timeout_sec

bise streams every reply and waits at most 90 s for the server to send something: the first byte, then each next piece. a local model on a slow machine can take longer than that to read a long prompt, and a gateway can hold a request while it queues. then bise retries, and the error says:

```text
no answer from localhost:11434 in 90 s (timeout) — a slow model or gateway? raise idle_timeout_sec under [providers.ollama] in ~/.bise/config.toml
```

raise it for that provider only (seconds, 1 to 86400):

```toml title="~/.bise/config.toml"
[providers.ollama]
idle_timeout_sec = 600
```

it also works for one model (`[models."ollama/qwen3:32b"]`), and a lower value makes a stuck gateway fail sooner. it limits silence, not the whole reply: a long answer that keeps coming is never cut.

## change a built-in provider

the same tables change a built-in provider or model, key by key. OpenAI through a proxy:

```toml title="~/.bise/config.toml"
[providers.openai]
base_url = "https://openai-proxy.example.com/v1"
```

the built-in list, with every key it takes, is [models.toml](https://github.com/gvergnaud/bise/blob/main/rust/catalog/models.toml); its header explains each one.

## when something is wrong

- `bise models <id>` shows the provider as bise reads it, its models and where the key comes from. an unknown key or a wrong type in config.toml shows there as a warning.
- `bise auth check <id>` makes one tiny call and says what failed.
- `unknown provider 'x'`: the part before the first `/` of the model name has no `[providers.x]` table.

## coming from OpenCode

| OpenCode `provider.<id>` | bise `[providers.<id>]` |
|---|---|
| `options.baseURL` | `base_url` |
| `options.apiKey` | `bise login <id>` (saved in `~/.bise/auth.json`) |
| `env` | `key_env` |
| `npm: "@ai-sdk/openai-compatible"` | `api = "openai-chat"` |
| `npm: "@ai-sdk/anthropic"` | `api = "anthropic"` |
| `models.<name>.limit.context` / `.output` | `[models."<id>/<name>"] context` / `max_output` |
| `whitelist` / `blacklist` | not needed: any `<id>/<model>` works |
| `options.chunkTimeout` / `headerTimeout` (ms) | `idle_timeout_sec` (seconds, one limit for both) |
| `options.timeout` (the whole request) | none: bise never cuts a reply that keeps coming |

config.toml never holds a key: it goes in `~/.bise/auth.json` (`bise login`), in the environment (`key_env`), or comes from a command (`key_command`).
