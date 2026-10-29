---
title: troubleshooting
description: start with the check: settings › setup in bise desktop, bise doctor in bise terminal. each says what is off and how to fix it.
---

::: desktop settings › setup
## settings › setup

settings › setup checks again what bise needs: a model (a plan signed in, or a key), bise's home folder, and the three permissions of `fn` (microphone, accessibility, screen recording). each row says what is off and what it costs, with the button that fixes it: **sign in**, **allow**, or **open System Settings**. **check again** reads them all again. `/setup` in the composer opens it.

### fn does nothing

settings › setup shows which permission is missing. without Accessibility, `fn` works only in bise's own windows; without the microphone, it can't hear you. settings › voice and fn also has a switch for hold and for `fn fn`.

### no notifications

settings › general › macOS notifications says whether macOS allows them, with **turn on** or **open System Settings**. notifications also wait during calls, screen shares, full screen and focus mode: see settings › quiet moments.

### it doesn't update

bise updates itself only from Applications. settings › general says why an update didn't happen, and offers **move to Applications** when that is the reason. see [updates](updates).
:::

::: terminal bise doctor
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
✓ voice       mistral/voxtral-mini-transcribe-3 · language auto · listens when you talk
? PATH        `bise` is not on PATH
              fix: add ~/.local/bin to PATH (the installer does it; open a new terminal)

? 1 thing to check. it says how.
```

`bise doctor --verbose` says more on each line.
:::

## common problems

### `bise: command not found`

`~/.local/bin` isn't on your `PATH` yet. open a new terminal, or run `~/.local/bin/bise`. the installer adds the line to your shell's rc file unless you passed `--no-modify-path`.

### a key doesn't work

in bise desktop, settings › models says what the provider answered when it refused a key. from a terminal:

```sh
bise providers               # where each key comes from
bise auth check anthropic    # one tiny call; says what failed
```

a key saved in bise (`~/.bise/auth.json`) wins over the one in your environment. if you changed the key in your shell, `bise logout <provider>` drops the saved one. when the check says the account has no credit, it links the provider's billing page.

### `unknown provider 'x'`

the part before the first `/` of a model name must be a provider bise knows, or one you added under `[providers.x]` in config.toml. `bise models` lists them. see [gateways and local models](gateways).

### no answer from a local model or a gateway

`no answer from localhost:11434 in 90 s (timeout)` means the server sent nothing for 90 s. a local model reading a long prompt can take longer: raise `idle_timeout_sec` for that provider. see [a slow server](gateways#a-slow-server-idle_timeout_sec).

### a key doesn't reach bise terminal in Ghostty

`cmd+v` with an image, `cmd+f`, `cmd+k`, `cmd+a` and `cmd+↑↓` need one line each in Ghostty's config: `bise setup ghostty`, or `/setup`.

### a remote MCP server has no tools

settings › plugins in bise desktop, `bise plugins` or `/plugins` in bise terminal, show each server's state. `needs a login`: **log in** there, or `bise plugins login <server>`. a header that uses an unset variable leaves the server out and names the variable.

### an agent seems stuck

look for `?` next to it: it may wait for you in the inbox (`⌘I` in bise desktop, `ctrl+1` in bise terminal). stopping its turn (`⌘.`, or `/interrupt` in bise terminal) and writing to it starts the next one. `/stop <agent>` also takes its hands off Chrome and your apps.

### main keeps compacting or seems to hold stale context

bise deliberately has no full-context reset for main: main keeps the continuity around ongoing agents, processes and unanswered questions. `/compact` compacts the conversation of the agent in view; it does not start a fresh thread or erase the canonical history. `/clear` only clears the visible feed, so it does not reset model context either.

if main repeatedly reloads a large context and compacts again, inspect `bise session show --context` and `bise doctor --verbose` before restarting. these show the context bise actually sent to the model and the setup diagnostics; check for private information and redact sensitive content before sharing excerpts in an issue.

## logs

::: terminal bise session show
```sh
bise session show            # the newest session's transcript
bise session show <id>       # one session (a prefix of its id works)
bise session show --context  # what the model sees now
```
:::

`/debug` in either app's composer shows the raw session of the agent in view.

## still stuck

open an issue on [GitHub](https://github.com/gvergnaud/bise/issues) with what you did, and the output of `bise doctor` if you have bise terminal. it never prints a key, so you can paste it as it is; check it holds no private path you'd rather keep.
