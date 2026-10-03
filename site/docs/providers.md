---
title: providers and keys
description: the model providers bise knows, where a key comes from, and how to add one.
---

bise talks to the model providers directly with your key. there is no bise account and no bise server in between: your prompts go from your Mac to the provider you picked.

<!-- if subscriptions -->two ways to pay for the models:

- **an API key** from a provider: this page.
- **a plan you already pay for**, like ChatGPT Plus or Pro, or a coding plan: see [subscriptions](subscriptions).
<!-- end -->
## add a key

the fastest way is the first run: when bise finds no key, it asks for one and checks it. after that, any time:

- in bise: `/provider`, then the provider, then paste the key.
- in a terminal:

```sh
bise login anthropic        # asks for the key, hidden
bise login                  # no provider: pick one from the list
```

`bise login` makes one tiny call with the key before it saves it, so a wrong key never gets saved. from a script, the key comes on stdin:

```sh
printf '%s' "$MY_KEY" | bise login openai --check
bise login openai --from ~/.env    # reads OPENAI_API_KEY=... from that file
```

| flag | what it does |
|---|---|
| `--check` | one tiny call first, saved only if it answers (for a script) |
| `--no-check` | save it without the call |
| `--model provider/model` | the model of that call (default: the one in use, else the provider's) |
| `--from FILE` | read the key from a `PROVIDER_API_KEY=...` line in a `.env` or shell file |

to remove a saved key: `bise logout anthropic`.

## where a key comes from

for each provider, bise takes the first key it finds, in this order:

1. `~/.bise/auth.json`: what `bise login`, `/provider` and the first run save. only you can read it (mode 0600).
2. the environment: the provider's variable, for example `ANTHROPIC_API_KEY`.
3. the old `.env` files of earlier versions: `~/.bend-harness/.env`, `~/.vibe/.env`.

so a key you saved in bise wins over the one in your shell. when your environment holds another key for the same provider, bise says once that it isn't used. `bise logout <provider>` goes back to the environment's key.

to see where each key comes from (never the key itself):

```sh
bise providers
```

and to test one with a tiny call, without saving anything:

```sh
bise auth check anthropic
```

> careful: `auth.json` holds your keys in clear text, readable only by you. don't commit it, paste it, or share it.

## the providers

`bise models` prints this list with each model's context, price and key state. the first run offers the first five; the others work with `bise login <id>` or their variable.

| provider | id | key variable | starts with | get a key |
|---|---|---|---|---|
| Anthropic | `anthropic` | `ANTHROPIC_API_KEY` | `anthropic/claude-opus-5-5` | [platform.claude.com](https://platform.claude.com/settings/keys) |
| OpenAI | `openai` | `OPENAI_API_KEY` | `openai/gpt-6-astra` | [platform.openai.com](https://platform.openai.com/api-keys) |
| Google AI Studio | `google` | `GEMINI_API_KEY` (or `GOOGLE_API_KEY`) | `google/gemini-3.8-flash` | [aistudio.google.com](https://aistudio.google.com/app/apikey) |
| Mistral | `mistral` | `MISTRAL_API_KEY` | `mistral/mistral-medium-latest` | [console.mistral.ai](https://console.mistral.ai/api-keys) |
| OpenRouter | `openrouter` | `OPENROUTER_API_KEY` | `openrouter/anthropic/claude-sonnet-5.5` | [openrouter.ai](https://openrouter.ai/settings/keys) |
| Groq | `groq` | `GROQ_API_KEY` | `groq/openai/gpt-oss-120b` | [console.groq.com](https://console.groq.com/keys) |
| xAI | `xai` | `XAI_API_KEY` | `xai/grok-4.7` | [console.x.ai](https://console.x.ai/team/default/api-keys) |
| DeepSeek | `deepseek` | `DEEPSEEK_API_KEY` | `deepseek/deepseek-v4-pro` | [platform.deepseek.com](https://platform.deepseek.com/api_keys) |
| Together AI | `together` | `TOGETHER_API_KEY` | `together/zai-org/GLM-5.3` | [api.together.ai](https://api.together.ai/settings/api-keys) |
| Fireworks AI | `fireworks` | `FIREWORKS_API_KEY` | `fireworks/accounts/fireworks/models/glm-5p3` | [app.fireworks.ai](https://app.fireworks.ai/settings/users/api-keys) |
| Cerebras | `cerebras` | `CEREBRAS_API_KEY` | `cerebras/gpt-oss-120b` | [cloud.cerebras.ai](https://cloud.cerebras.ai) |
| Ollama | `ollama` | none, local | `ollama/<model>` | [ollama.com](https://ollama.com) |
| LM Studio | `lmstudio` | none, local | `lmstudio/<model>` | [lmstudio.ai](https://lmstudio.ai) |

listed but not usable yet: Azure OpenAI, Google Vertex AI and Amazon Bedrock (they need a cloud login bise doesn't do yet). to reach them today, put a gateway in front: see [gateways and local models](gateways).

some keys only do one job:

| provider | id | key variable | what for |
|---|---|---|---|
| ElevenLabs | `elevenlabs` | `ELEVENLABS_API_KEY` | [voice](voice): speech to text |
| Deepgram | `deepgram` | `DEEPGRAM_API_KEY` | [voice](voice): speech to text |
| TypeSafe | `typesafe` | `TYPESAFE_API_KEY` | the checker of [auto mode](approvals) |

## one key is enough

with a single key, bise fills every role from that provider: main and the agents use the model you picked, the small jobs (titles, summaries) use the provider's cheap model, and voice uses the provider's speech model when it has one (Mistral, OpenAI). [models and roles](models) says how to change each one.

| provider | its cheap model, for small jobs |
|---|---|
| Anthropic | `claude-haiku-4-5` |
| OpenAI | `gpt-6-luna` |
| Google AI Studio | `gemini-3.5-flash-lite` |
| Mistral | `mistral-small-latest` |
| OpenRouter | `google/gemini-3.8-flash` |

## any model, listed or not

a model name is `provider/model`. any model the provider serves works, even when bise doesn't list it: it gets the provider's defaults (context, output, images). the provider is the part before the first `/`, so `openrouter/moonshotai/kimi-k3` is OpenRouter's `moonshotai/kimi-k3`.

```sh
bise models            # every provider and model bise knows
bise models openai     # only these
```

## what leaves your Mac

your messages, the files and command output the agents read, and the images you paste go to the provider of the model that works on them. nothing goes to bise. a few features send more, and each one says so the first time: [voice](voice) sends your audio to its speech provider, and [auto mode's](approvals) checker sends the commands it checks to its model.
