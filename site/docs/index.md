---
title: bise docs
description: bise is a terminal app for multi-agent coding. you talk to one agent, main, and it runs a team of agents for you.
---

bise runs in your terminal, one window per repo. you talk to **main**, your team lead. main turns what you ask into tasks and starts one **agent** per task. the agents work in parallel, talk to each other, and land their work in your repo. what needs you waits in your **inbox**.

these docs cover how to install bise, how to give it a model, and how to use and extend it.

## start here

1. [install bise](install) and run it in a repo.
2. give it a model: an API key from a [provider](providers)<!-- if subscriptions -->, or a plan you already pay for, through a [subscription](subscriptions)<!-- end -->.
3. ask main for something. read [main and the agents](agents) to see what happens next.

```sh
curl -fsSL https://bise.dev/install | sh
cd your-repo
bise
```

## what's in these docs

| page | what you find there |
|---|---|
| [providers and keys](providers) | the providers bise knows, where a key goes, `bise login` |
| [models and roles](models) | which model does what: main, agents, small jobs, voice |
| [gateways and local models](gateways) | LiteLLM, a company proxy, Ollama, LM Studio, vLLM |
| [main and the agents](agents) | how a task runs, the inbox, the commands you use |
| [approvals](approvals) | yolo and auto: what runs without asking |
| [landing work](flow) | how agents commit, features, pull requests |
| [voice](voice) | dictation and voice mode |
| [computer use](computer-use) | agents that drive Chrome and your Mac apps |
| [plugins and MCP](plugins) | skills, local and remote MCP servers, logins |
| [config.toml](config) | every setting in one place |
| [troubleshooting](troubleshooting) | `bise doctor`, logs, the usual fixes |

## for agents

every page is also plain Markdown: add `.md` to its address, for example `bise.dev/docs/providers.md`. to have a coding agent install and set up bise for you, give it [bise.dev/setup.md](https://bise.dev/setup.md).

## requirements

bise runs on macOS, on Apple silicon and Intel. you need git. linux is next.
