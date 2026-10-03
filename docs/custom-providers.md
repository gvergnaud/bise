# Custom providers and LLM gateways

bise talks to any server that speaks one of three APIs: OpenAI's Chat
Completions, Anthropic's Messages, or OpenAI's Responses. So a gateway
like [LiteLLM](https://docs.litellm.ai/docs/simple_proxy), a company
proxy, vLLM, or any other OpenAI-compatible base URL works: you add it as
a provider in `~/.bise/config.toml`.

`bise models` prints which config file bise reads (last line).

## LiteLLM in three steps

1. Add the provider to `~/.bise/config.toml`:

   ```toml
   [providers.litellm]
   name = "LiteLLM"
   api = "openai-chat"
   base_url = "http://localhost:4000/v1"
   key_env = "LITELLM_API_KEY"
   small_model = "mistral-small"   # a cheap model, for titles and summaries

   [roles]
   main = "litellm/claude-sonnet-4-5"
   ```

   The part after `litellm/` is a `model_name` from your LiteLLM config,
   as is. Names with a `/` work too: `litellm/anthropic/claude-sonnet-4-5`
   (the provider is the part before the first `/`).

2. Give bise the key (your LiteLLM master key or virtual key). Either
   save it:

   ```sh
   bise login litellm
   ```

   It is saved in `~/.bise/auth.json`, only you can read it, and it wins
   over the environment. Or export the variable named by `key_env`:

   ```sh
   export LITELLM_API_KEY=sk-...
   ```

3. Check it:

   ```sh
   bise auth check litellm   # one tiny call: "✓ litellm/... answered"
   bise providers            # LiteLLM  ✓ ready · saved in bise · main uses it
   ```

Then start `bise`. `/model litellm/<model>` switches an agent to
another model, and `bise config set agents litellm/<model>` picks the
model of the agents main starts.

## The provider table

`[providers.<id>]`, every key optional but `base_url`:

| key | what it does | default |
| --- | --- | --- |
| `name` | the name bise shows | the id |
| `api` | the wire API: `openai-chat` (`/chat/completions`), `anthropic` (`/messages`), `openai-responses` (`/responses`) | `openai-chat` |
| `base_url` | the URL bise adds the path above to: keep the `/v1` | none |
| `key_env` | the env variable holding the key; `""` for a server that needs no key | none |
| `small_model` | a cheap model of this provider, for titles and summaries | none |
| `key_command` | a shell command that prints the key, run before every call (below) | none |
| `headers` | a table of static HTTP headers; model values override provider values | none |
| `headers_env` | an env variable holding extra headers (below) | none |
| `idle_timeout_sec` | how long bise waits for the server to send something, in seconds (below) | 90 |

The key is sent as `Authorization: Bearer <key>` (`openai-chat`,
`openai-responses`) or `x-api-key` (`anthropic`). With `key_env = ""`
and no `key_command`, bise sends no key: a local server.

## Model limits

A model bise doesn't know gets these defaults: 128k context, 16k output,
no images, no reasoning, tools on. Set the real ones per model, or for
the whole provider (in `[providers.<id>]`, same keys):

```toml
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

Context matters: bise compacts a conversation before it fills the
model's window, so a window set too large ends in an error from the
server instead.

## Gateways behind a login (key_command, headers_env)

Some gateways want a short-lived token or extra headers, the way Claude
Code uses `apiKeyHelper` and `ANTHROPIC_CUSTOM_HEADERS`:

```toml
[providers.gateway]
name = "Company gateway"
api = "anthropic"
base_url = "https://gateway.example.com/v1"
key_env = ""
key_command = "my-tool auth token"     # its output is the key, run before every call
headers_env = "GATEWAY_HEADERS"         # one "Name: value" per line
reasoning = true
```

```sh
export GATEWAY_HEADERS="x-team: platform"
```

The key from `key_command` is sent both as the API's own header and as
`Authorization: Bearer`. A command that fails or prints nothing stops
the call with one line naming it. Write `key_command` on one line, with
no `"` or `\` (use single quotes).

LiteLLM also serves the Anthropic API (`/v1/messages`), so the same
LiteLLM works with `api = "anthropic"` when you want Claude's thinking
blocks kept as they are.

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

## Local servers

Ollama (`ollama/<model>`) and LM Studio (`lmstudio/<model>`) are built
in. Another local server (vLLM, llama.cpp, ...):

```toml
[providers.vllm]
name = "vLLM"
base_url = "http://localhost:8000/v1"
key_env = ""
context = 32768
```

### A slow server: `idle_timeout_sec`

bise streams every reply, and waits at most 90 s for the server to
send something: the first byte, then each next piece. A local model on
a slow machine can take longer than that to read a long prompt before
its first word, and a gateway can hold the request while it queues.
Then bise retries, and the error says:

```
no answer from localhost:11434 in 90 s (timeout) — a slow model or gateway? raise idle_timeout_sec under [providers.ollama] in ~/.bise/config.toml
```

Raise it for that provider only (seconds, 1 to 86400):

```toml
[providers.ollama]
idle_timeout_sec = 600
```

It also works under one model (`[models."ollama/qwen3:32b"]`), and a
lower value makes a stuck gateway fail sooner. It is a limit on
silence, not on the whole reply: a long answer that keeps coming is
never cut. Unset, every provider keeps 90 s.

## Changing a built-in provider

The same tables change a built-in provider or model, key by key. For
example, OpenAI through a proxy:

```toml
[providers.openai]
base_url = "https://openai-proxy.example.com/v1"
```

The built-in list, with every key it takes, is
[`rust/catalog/models.toml`](../rust/catalog/models.toml); its header
explains each one.

## When something is wrong

- `bise models <id>` shows the provider as bise reads it, its models and
  where the key comes from. An unknown key or a wrong type in
  config.toml is listed there as a warning, not an error.
- `bise auth check <id>` makes one tiny call and says what failed.
- `unknown provider 'x'`: the provider part of the model name has no
  `[providers.x]` table.

## Coming from opencode

opencode's `config.json` maps like this:

| opencode `provider.<id>` | bise `[providers.<id>]` |
| --- | --- |
| `options.baseURL` | `base_url` |
| `options.apiKey` | `bise login <id>` (saved in `~/.bise/auth.json`) |
| `env` | `key_env` |
| `npm: "@ai-sdk/openai-compatible"` | `api = "openai-chat"` |
| `npm: "@ai-sdk/anthropic"` | `api = "anthropic"` |
| `models.<name>.limit.context` / `.output` | `[models."<id>/<name>"] context` / `max_output` |
| `whitelist` / `blacklist` | not needed: any `<id>/<model>` works |
| `options.chunkTimeout` / `headerTimeout` (ms) | `idle_timeout_sec` (seconds, one limit for both) |
| `options.timeout` (whole request) | none: bise never cuts a reply that keeps coming |

config.toml takes no key: it goes in `~/.bise/auth.json` (`bise login`),
in the environment (`key_env`), or comes from a command (`key_command`).
