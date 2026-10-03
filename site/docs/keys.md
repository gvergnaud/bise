---
title: keys and commands
description: the keys and slash commands of bise. in the app, /help shows the essential ones and /shortcuts every one.
---

hold `ctrl` (or `⌥`) in bise to see that key's shortcuts where they act, in Ghostty and kitty. `/` in the composer lists the commands; `tab` completes, `⏎` runs.

## talking

| key | what it does |
|---|---|
| `⏎` | send to the agent in view; while it works, steer its turn |
| `tab` | while it works: queue your message for after its turn |
| `shift+⏎` | a new line (`alt+⏎` and `ctrl+j` too) |
| `@agent …` | a message to an agent without leaving main; `@main …` from inside an agent |
| `@` + a path | attach a file |
| `$` | a skill: the popup lists them |
| `ctrl+c` | interrupt the turn of the agent in view; again, or at idle, quit (the agents keep running) |
| `shift+tab` | switch the approvals mode: yolo / auto |
| `ctrl+r` | dictation; twice: [voice mode](voice#voice-mode) |
| `esc` | put the draft away (`↑` brings it back) |

## agents

| key | what it does |
|---|---|
| `⌥` + `0`…`9` | go to main (`0`) or to the agent with that number |
| `alt+↓` / `alt+↑` | select the next / previous agent; `⏎` enters it, `space` previews it |
| `ctrl+s` | find an agent by name (`cmd+k`, `/switch`) |
| `esc` | in an agent: back to main |

## inbox

| key | what it does |
|---|---|
| `ctrl+1`…`ctrl+9` | open the inbox item with that number (or click it) |
| `1`-`9` | an item open: pick an option |
| `←` / `→` | an item open: the previous / next one |
| `ctrl+x` | an item open: close it without answering |

## the feed

| key | what it does |
|---|---|
| `ctrl+o` | open or close everything folded |
| `ctrl+f` | find (`cmd+f` in Ghostty with its lines) |
| `ctrl+y` | copy a code block |
| `ctrl+l` | clear the display |
| `` ctrl+` `` | show or hide a terminal panel in the workspace (`ctrl+space` too) |

select text in the feed, then type: the selection goes into your message as a quote.

## commands

| command | what it does |
|---|---|
| `/help` | the commands and the essential keys |
| `/shortcuts` | every keyboard shortcut (also `/keys`) |
| `/inbox` | open what waits for you, the most blocking first |
| `/answer N text` | answer inbox item N |
| `/close N [note]` | close inbox item N without answering |
| `/new [-w] [name:] objective` | start an agent; `-w` in its own git worktree |
| `/agents` | list the agents and what they do |
| `/switch` | find an agent by name, archived ones too |
| `/rename <agent> <new-name>` | rename an agent |
| `/isolate <agent>` | give an agent its own git worktree |
| `/archive <agent>` | stop an agent and archive it, with its worktree |
| `/restore <agent>` | bring an archived agent back |
| `/archived` | show or hide the archived agents |
| `/interrupt` | interrupt the turn of the agent in view |
| `/stop <agent>` | stop an agent's turn and its hands on Chrome or an app, until you write to it |
| `/model [<model>] [default]` | the model of the agent in view; `default`: for new sessions too |
| `/models` | which model does what: main, agents, small jobs, voice, the checker |
| `/provider` | set up a provider's key, or change it |
| `/reasoning [<effort>]` | the reasoning effort of the agent in view |
| `/approvals [yolo\|auto]` | the mode, the checker and your saved rules; or switch the mode |
| `/compact` | compact the conversation of the agent in view |
| `/voice` | dictation and voice mode: the model, the voice, the language |
| `/computer-use [off\|uninstall]` | turn on and set up computer use; or turn it off |
| `/plugins [list\|enable\|disable\|login\|logout] [<name>]` | the agent plugins |
| `/theme [auto\|light\|dark]` | the theme; auto follows your terminal |
| `/setup` | check your terminal and repo again, and offer what would help |
| `/welcome` | replay the first run |
| `/update` | look for a new release now |
| `/restart` | reload bise; nothing is lost |
| `/quit` | quit; the agents keep running |
