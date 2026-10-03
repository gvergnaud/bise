---
title: the bise command
description: every subcommand of bise, for your terminal and your scripts. bise --help prints the same list.
---

## start

| command | what it does |
|---|---|
| `bise` | open bise in this folder: main and its agents |
| `bise switchboard --stop` | stop this folder's bise and its agents; `--keep-agents`: they go on |

## keys and models

| command | what it does |
|---|---|
| `bise login [provider]` | add a provider's key: checked with one tiny call, then saved ([more](providers#add-a-key)) |
| `bise logout [provider]` | remove it |
| `bise providers` | your providers: which are set up, where each key comes from (also `bise auth list`) |
| `bise auth check [provider]` | one tiny call with the key bise finds; saves nothing. `--model provider/model` |
| `bise models [filter]` | the models bise knows, and which have a key. `voice` as the filter: the speech models |
| `bise config get KEY` | read a setting: `main`, `agents`, `small`, `voice`, `approvals`, `compaction_threshold`, `project_doc_fallback_filenames` |
| `bise config set KEY VALUE` | write it in [config.toml](config) |

## setup

| command | what it does |
|---|---|
| `bise setup scan` | what this Mac has for bise: where keys are, tools, repos, Claude Code's and Codex's setup; never a key |
| `bise setup ghostty` | add the Ghostty lines for `cmd+v`/`f`/`k`/`a`/`↑↓`; `--dry-run` shows the change only |
| `bise plugins [list]` | agent plugins and what's wrong with them; `--json` |
| `bise plugins enable NAME` / `disable NAME` | turn a plugin on or off |
| `bise plugins import-mcp NAME [--dry-run] < servers.json` | Claude Code's or Codex's MCP servers as one plugin |
| `bise plugins login [SERVER]` / `logout SERVER` | log in to a remote MCP server in the browser, or forget it ([more](plugins#logging-in-oauth)) |
| `bise doctor` | check this Mac, the install, keys, models and running bise; `--verbose` ([more](troubleshooting)) |

## the install

| command | what it does |
|---|---|
| `bise update` | install the latest release; `--check` only says if there is one ([more](updates)) |
| `bise uninstall` | remove bise; `--purge` also removes `~/.bise`: settings, keys and threads |
| `bise --version` | this version |
| `bise session show [<id>]` | a session's log: the transcript; `--context` what the model sees now; `--raw` the log lines |

## for programs

| command | what it does |
|---|---|
| `bise --headless` | one session without the TUI: `--model NAME`, `--port N`, `--continue`, `--resume ID` (a prefix of the id works) |
| `bise sb <command>` | the agents' own tool (`sb` is a link to bise). `sb help` lists its commands |
