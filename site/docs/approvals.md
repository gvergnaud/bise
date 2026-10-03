---
title: approvals
description: two modes for every agent at once. yolo runs everything. auto runs what is clearly safe and asks you about the risky calls.
---

## the two modes

| mode | what runs without asking |
|---|---|
| `yolo` | everything. nothing asks, no exceptions. this is the default |
| `auto` | reads, edits in the repo, local git, `sb`, and the rules you saved. a checker model judges the rest, and the risky calls ask you |

the mode is one for main and every agent. it shows on the divider after the model, for example `you → main · opus 5.5 · high · yolo`.

`shift+tab` switches between them. bise remembers your pick in `~/.bise/config.toml`, for every repo, after a restart too. or:

```text
/approvals auto
```

```sh
bise config set approvals auto
```

`BISE_APPROVALS=auto bise` sets it for one session only, without writing the file.

## what auto runs at once

- **reads**, anywhere but the secret paths (your keys, `~/.ssh`, ...).
- **edits** in the roots: the repo you started bise in and below, `~/.bise` (but `auth.json`, `approvals.toml` and `hubs/`), and each agent's own temp folder.
- plain shell writes bise can read (`mkdir`, `rm`, `sed -i`, `cat > file`) inside the roots, like edits.
- **local git**: `add`, `commit`, `apply`. `checkout`, `reset`, `clean` and `restore` go to the checker.
- `sb`, the agents' own tool.
- the rules you saved with "always allow".

on macOS, the commands of auto run in a sandbox that lets them write only in the roots.

everything else goes to the **checker**, a small model that looks at the command and the task. most of what it sees runs. what may not be undoable, or doesn't look like part of the task, asks you.

## when an agent asks you

the question waits in your inbox:

```text
┃ ? api-v2 wants to run                                   1/3
┃   $ git push origin main --force
┃   it pushes to main and rewrites its history.
┃   1 allow   2 always allow git push here   3 no
┃   or type why not, then ⏎
```

- `1` allows it once.
- `2` allows it and every command like it in this repo from now on. the item names the rule it saves, like `cargo test *`.
- `3`, or type why not and `⏎`: the agent gets no, with your words.

the agent waits as long as it takes. switching to yolo leaves the waiting items open. a few calls always ask, with no "always" option, and say why: `it rewrites main. this one always asks.`

## your rules

`/approvals` shows the mode, the checker, and the rules you saved, and removes one. the rules live in `~/.bise/approvals.toml`, per repo (a repo's worktrees share them). an agent can't edit that file.

## the checker

the checker is a role, like main's model: `/models` changes it.

| checker | when | what leaves your Mac |
|---|---|---|
| Jev, by TypeSafe | the default when you have a TypeSafe or an OpenRouter key | the command, the script it runs, the start of the task, the paths |
| your small jobs model | no TypeSafe or OpenRouter key | the same, to that model's provider |
| a local model (Ollama, LM Studio) | you pick it in `/models` | nothing |
| off | `/models`, then off | nothing. auto then runs reads and edits, and every other command asks you |

the first time you switch to auto, bise says in one line which model checks and where it runs.

> note: auto reads the text of a command; it can't see what a program does once it runs. it stops mistakes and casual prompt injection. it is not a wall against a determined attacker.
