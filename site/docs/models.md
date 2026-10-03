---
title: models and roles
description: which model does what in bise, and how to change each one.
---

bise uses models for a few different jobs. each job is a **role**, and each role has its own model. you pick one model at the first run; every other role follows it or picks a cheap one for you.

## the roles

| role | what it does | unset |
|---|---|---|
| `main` | your team lead: the agent you talk to | the model of the first run |
| `agents` | the agents main starts | same as main |
| `small` | small jobs: titles, summaries | the cheap model of main's provider |
| `voice` | listens when you talk (`ctrl+r`) | Mistral's or OpenAI's speech model, when you have that key |
| `classify` | the checker: in auto, decides which commands run and which ask you | Jev, through TypeSafe or OpenRouter when you have one of those keys, else your small jobs model ([more](approvals#the-checker)) |

`/models` shows them all, with what each one resolves to. pick a row to change it: role, then provider, then model, then effort.

## change a model

for the agent you're looking at, now:

```text
/model anthropic/claude-sonnet-5-5
```

`/model <model> default` also makes it the default for new sessions: main's role when you're in main's view, the agents' role in an agent's view.

from a terminal, for every new session:

```sh
bise config set main anthropic/claude-opus-5-5
bise config set agents openai/gpt-6.1-sol
bise config set small mistral/mistral-small-latest
bise config get main
```

or in `~/.bise/config.toml`:

```toml title="~/.bise/config.toml"
[roles]
main = "anthropic/claude-opus-5-5"      # your team lead
agents = "openai/gpt-6.1-sol"           # the agents main starts (unset: main's)
small = "mistral/mistral-small-latest"  # titles, summaries
voice = "mistral/voxtral-transcribe-3"  # listens when you talk

[roles.main]                            # the table form, to set the effort too
model = "anthropic/claude-opus-5-5"
effort = "high"
```

a role is either one line or a table, not both. an environment variable wins over the file for one session: `BISE_MODEL` (main), `BISE_AGENT_MODEL`, `BISE_SMALL_MODEL`, `BISE_VOICE_MODEL`, `BISE_CLASSIFY_MODEL`.

> note: older config files have `model`, `agent_model`, `small_model` and `[voice] model` at the top. they keep working. when bise writes a role, it moves it under `[roles]`.

## reasoning effort

for a model that reasons, `/reasoning` picks how hard it thinks:

```text
/reasoning high
```

Anthropic's models take `none`, `low`, `medium`, `high` and `max`; the others `none`, `low`, `medium` and `high`. unset, a model gets `high`. the effort shows on the divider next to the model.

## compaction

when a conversation grows close to the model's context window, bise compacts it: it replaces the older part with a summary, so the agent goes on. by default that happens at 80% of the window. to compact earlier:

```sh
bise config set compaction_threshold 450000   # tokens
bise config set compaction_threshold 45%      # a share of the window
```

the threshold is at most 80% of the window. `/compact` compacts the agent you're looking at now.

## what a model can do

each model in the list has a context window, a max output, and whether it reads images and reasons. a model bise doesn't list gets its provider's defaults. for a model behind your own gateway, you set these yourself: see [gateways and local models](gateways#model-limits).

```sh
bise models anthropic
```

```text
anthropic  Anthropic · anthropic · key: env ANTHROPIC_API_KEY
  anthropic/claude-opus-5-5      1M ctx  128k out vision reasoning $4/$20
  anthropic/claude-sonnet-5-5    1M ctx  128k out vision reasoning $2/$10
  anthropic/claude-haiku-4-5   200k ctx   64k out vision reasoning $1/$5
```

prices are list prices in USD per million tokens, input then output. bise shows a turn's cost when it knows the price.
