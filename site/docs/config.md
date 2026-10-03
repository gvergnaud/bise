---
title: config.toml
description: every setting of bise in one file, ~/.bise/config.toml. most of them have a command, so you rarely edit it by hand.
---

bise reads `~/.bise/config.toml` (or `$BISE_HOME/config.toml`). you rarely need to open it: `/models`, `/voice`, `shift+tab` and `bise config set` write it for you, and keep the rest of the file as it was. `bise models` prints the path on its last line, and lists what it can't read as warnings.

> note: config.toml never holds a key. keys go in `~/.bise/auth.json` (`bise login`), in the environment, or come from a command. see [providers and keys](providers).

## a full example

```toml title="~/.bise/config.toml"
approvals = "auto"                        # yolo (default) or auto
compaction_threshold = "45%"              # tokens (450000) or a share of the window; at most 80%
project_doc_fallback_filenames = ["CLAUDE.md"]

[roles]
main = "anthropic/claude-opus-5-5"
agents = "openai/gpt-6.1-sol"
small = "mistral/mistral-small-latest"
voice = "mistral/voxtral-transcribe-3"

[roles.main]                              # instead of the main line, to set the effort
model = "anthropic/claude-opus-5-5"
effort = "high"

[voice]
language = "en"
vocabulary = ["bise", "config.toml"]
listen = "auto"

[providers.litellm]
api = "openai-chat"
base_url = "http://localhost:4000/v1"
key_env = "LITELLM_API_KEY"

[models."litellm/claude-sonnet-4-5"]
context = 1000000
vision = true
reasoning = true
```

## the top-level keys

| key | what it does | unset | command |
|---|---|---|---|
| `approvals` | `yolo`: every call runs. `auto`: safe calls run, risky ones ask you | `yolo` | `shift+tab`, `bise config set approvals auto` |
| `compaction_threshold` | when a conversation compacts: tokens, or a share of the window like `"45%"` | 80% of the window | `bise config set compaction_threshold 45%` |
| `project_doc_fallback_filenames` | files read where a folder has no AGENTS.md | none | `bise config set project_doc_fallback_filenames CLAUDE.md` |
| `project_doc_max_bytes` | the budget of the repo's instructions files | 32768 | |

## [roles]

which model does what. a role is a line (`main = "provider/model"`) or a table with `model` and `effort`. see [models and roles](models).

| role | env variable that wins | command |
|---|---|---|
| `main` | `BISE_MODEL` | `bise config set main <model>` |
| `agents` | `BISE_AGENT_MODEL` | `bise config set agents <model>` |
| `small` | `BISE_SMALL_MODEL` | `bise config set small <model>` |
| `voice` | `BISE_VOICE_MODEL` | `bise config set voice <model>` |
| `classify` | `BISE_CLASSIFY_MODEL` | `/models` |

## [voice]

dictation's `language` and `vocabulary`, and voice mode's `listen`, `tts_voice`, `speed`, `read_aloud` and `sounds`. see [voice](voice#settings).

## [providers.&lt;id&gt;] and [models."&lt;id&gt;/&lt;model&gt;"]

add a provider, or change a built-in one key by key: `name`, `api`, `base_url`, `key_env`, `small_model`, `key_command`, `headers`, `headers_env`, `idle_timeout_sec`, and a model's `context`, `max_output`, `vision`, `reasoning`, `tools` and prices. see [gateways and local models](gateways).

## the repo's settings

a few settings belong to one repo, in `.switchboard/config.toml` at its root:

```toml title="your-repo/.switchboard/config.toml"
[flow]
mode = "trunk"            # "pr" or "trunk"; unset: main asks you once
check = "make test"       # what every land runs first
push = true               # trunk: push the default branch after each land
```

see [landing work](flow).

## environment variables

| variable | what it does |
|---|---|
| `BISE_HOME` | moves `~/.bise` |
| `BISE_MODEL`, `BISE_AGENT_MODEL`, `BISE_SMALL_MODEL`, `BISE_VOICE_MODEL`, `BISE_CLASSIFY_MODEL` | a role's model for this session |
| `BISE_APPROVALS` | `yolo` or `auto` for this session, never written |
| `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, ... | a provider's key (see [providers and keys](providers#the-providers)) |
