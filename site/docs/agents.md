---
title: main and the agents
description: you talk to main. main starts one agent per task, the agents work in parallel, and what needs you waits in your inbox.
---

## how it works

1. you tell **main** what you want, in your own words. main is your team lead.
2. main turns it into tasks and starts one **agent** per task. each agent has a brief: the goal, the context, what not to do, and when it's done.
3. the agents work in parallel. they message each other and main when their work touches.
4. when an agent needs a decision only you can make, it lands in your **inbox**. the rest never interrupts you.
5. when a task is done, its agent lands the work in your repo (see [landing work](flow)) and reports to main.

you keep typing the whole time. the composer is never locked: `⏎` while main works steers its turn, and `tab` queues your message for after it.

## the screen

- **the feed**: the thread of the agent you're looking at, main by default.
- **the panel** on the right: every agent, a number, and its state. `∿` working, `?` needs you, `✓` done, `○` idle.
- **the divider** above the composer: who you talk to, its model, its effort and the approvals mode, for example `you → main · opus 5.5 · high · yolo`.
- **the inbox**, above the divider when something waits: approvals first, then questions, then merges.

## talk to an agent

- `⌥` + a number goes to that agent; `⌥0` back to main. `esc` also goes back to main.
- click an agent in the panel.
- `@agent …` from main's view sends it a message without leaving main. `@main …` from inside an agent.
- `ctrl+s` (or `cmd+k`, or `/switch`) finds an agent by name, archived ones too.

in an agent's view, what you type goes to that agent. main still sees it.

## the inbox

an inbox item is a question, an approval or a merge that waits for you. `ctrl+1` to `ctrl+9` opens the item with that number, or click it, or `/inbox`.

with an item open:

- `1`-`9` picks an option, or `↑↓` then `⏎`.
- type an answer and `⏎` (on an approval, typing means no, with your text as the note).
- `←→` moves to the other items. `ctrl+x` closes it without answering. `esc` goes back to your thread; the item keeps your draft.

`/answer N text` and `/close N [note]` do the same from the composer.

## the commands you use

| command | what it does |
|---|---|
| `/new [-w] [name:] objective` | start an agent yourself; `-w` gives it its own git worktree |
| `/agents` | list the agents and what they do |
| `/switch` | find an agent by name, archived ones too |
| `/interrupt` | stop the current turn of the agent in view |
| `/stop <agent>` | stop an agent's turn, and its hands on Chrome or an app, until you write to it |
| `/archive <agent>` | stop an agent and archive it, with its worktree |
| `/restore <agent>` | bring an archived agent back |
| `/archived` | show or hide the archived agents in the panel |
| `/isolate <agent>` | give an agent its own git worktree |
| `/rename <agent> <new-name>` | rename an agent |
| `/compact` | compact the conversation of the agent in view |
| `/quit` | quit; the agents keep running |

every command and key is in [keys and commands](keys).

## the agents keep running

quitting bise (`/quit`, or `ctrl+c` at idle) closes the window, not the work: the agents go on in the background. run `bise` in the same folder to come back. to stop everything in a folder:

```sh
bise switchboard --stop               # stop this folder's bise and its agents
bise switchboard --stop --keep-agents # stop bise, the agents go on
```

## past work

every agent's thread is kept, archived ones too. ask main about past work in your own words ("what did we do on the login last week?"): it searches every thread, from before any compaction too. `bise session show` prints a session's log from a terminal.

## worktrees

agents that touch the same files share your folder. main gives a bigger or riskier task its own git worktree, a separate checkout on its own branch, so it never steps on the others. `ψ` in the panel marks an agent alone in its worktree. the worktree is removed when its work has landed and its last agent leaves.
