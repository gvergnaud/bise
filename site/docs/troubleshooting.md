---
title: troubleshooting
description: bise doctor checks your machine, the install, your keys and models in one command. start there.
---

## bise doctor

```sh
bise doctor
```

it checks this machine (macOS, or Linux: the distro and what is off there), the install, your keys, each role's model and the running bise, without the network and without printing a key. each line is `✓` (fine) or `?` (worth a look), and every `?` says how to fix it:

```text
checking your setup

✓ macOS       macOS 15.5 arm64
✓ bise        bise 3f2a91c0 (darwin-arm64) · ~/.local/share/bise/versions/3f2a91c0/bise
✓ home        ~/.bise (default): config.toml ✓, auth.json ✓, hubs/ ✓, sessions/ ✓
✓ git         git version 2.50.1
✓ github      gh logged in; this repo's PRs: github.com/you/your-repo
✓ keys        anthropic (auth.json), mistral (env MISTRAL_API_KEY)
✓ main        anthropic/claude-opus-5-5 · high
✓ agents      same as main
✓ small jobs  anthropic/claude-haiku-4-5 · titles, summaries
✓ voice       mistral/voxtral-transcribe-3 · language auto · listens when you talk
? PATH        `bise` is not on PATH
              fix: add ~/.local/bin to PATH (the installer does it; open a new terminal)

? 1 thing to check. it says how.
```

`bise doctor --verbose` says more on each line.

## common problems

### `bise: command not found`

`~/.local/bin` isn't on your `PATH` yet. open a new terminal, or run `~/.local/bin/bise`. the installer adds the line to your shell's rc file unless you passed `--no-modify-path`.

### a key doesn't work

```sh
bise providers               # where each key comes from
bise auth check anthropic    # one tiny call; says what failed
```

a key saved in bise (`~/.bise/auth.json`) wins over the one in your environment. if you changed the key in your shell, `bise logout <provider>` drops the saved one. when the check says the account has no credit, it links the provider's billing page.

### `unknown provider 'x'`

the part before the first `/` of a model name must be a provider bise knows, or one you added under `[providers.x]` in config.toml. `bise models` lists them. see [gateways and local models](gateways).

### no answer from a local model or a gateway

`no answer from localhost:11434 in 90 s (timeout)` means the server sent nothing for 90 s. a local model reading a long prompt can take longer: raise `idle_timeout_sec` for that provider. see [a slow server](gateways#a-slow-server-idle_timeout_sec).

### a key doesn't reach bise in Ghostty

`cmd+v` with an image, `cmd+f`, `cmd+k`, `cmd+a` and `cmd+↑↓` need one line each in Ghostty's config: `bise setup ghostty`, or `/setup`.

### a remote MCP server has no tools

`bise plugins` (or `/plugins`) shows each server's state. `needs a login`: run `bise plugins login <server>`. a header that uses an unset variable leaves the server out and names the variable.

### an agent seems stuck

look for `?` in the panel: it may wait for you in the inbox (`ctrl+1`). `/interrupt` in its view stops its turn, and a message starts the next one. `/stop <agent>` also takes its hands off Chrome and your apps.

### main keeps compacting or seems to hold stale context

bise deliberately has no full-context reset for main: main keeps the continuity around ongoing agents, processes and unanswered questions. `/compact` compacts the conversation of the agent in view; it does not start a fresh thread or erase the canonical history. `/clear` only clears the visible feed, so it does not reset model context either.

if main repeatedly reloads a large context and compacts again, capture `bise session show --context` and `bise doctor --verbose` before restarting, then include that evidence in an issue. this makes the context bise actually sent to the model inspectable without pretending a reset happened.

## logs

```sh
bise session show            # the newest session's transcript
bise session show <id>       # one session (a prefix of its id works)
bise session show --context  # what the model sees now
```

## still stuck

open an issue on [GitHub](https://github.com/gvergnaud/bise/issues) with the output of `bise doctor` and what you did. it never prints a key, so you can paste it as it is; check it holds no private path you'd rather keep.
