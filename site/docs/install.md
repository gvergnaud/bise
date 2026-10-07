---
title: install and first run
description: one command installs bise. the first run asks for one thing, the model, and only when it finds no key.
---

## install

```sh
curl -fsSL https://bise.dev/install | sh
```

the installer picks the build for your machine (a Mac with Apple silicon or Intel, Linux on x86_64 or arm64), checks its sha256, and puts it in `~/.local/share/bise`. the `bise` command goes in `~/.local/bin`. no sudo.

open a new terminal if `bise` is not found yet, or run `~/.local/bin/bise`. then check:

```sh
bise --version
```

> note: `~/.local/bin` must be on your `PATH`. the installer adds it to your shell's rc file unless you pass `--no-modify-path`.

### linux

the same command works on Linux, x86_64 and arm64. the build needs glibc 2.34 or newer:

| works | not yet |
|---|---|
| Ubuntu 22.04 and newer, Debian 12 and newer, Fedora, RHEL, Rocky and Alma 9, Amazon Linux 2023, Arch, WSL2 with Ubuntu 22.04+ | Alpine (musl), Ubuntu 20.04, Debian 11, RHEL 8 (older glibc) |

it needs `git`, and OpenSSL 3 (`libssl.so.3`, there by default on the systems above) for its HTTPS calls. `bise doctor` shows your distro and what is off on Linux.

on Linux, these are macOS only and stay off: the [sandbox](approvals) of `auto` approvals (auto checks each command instead, and says so once), [voice](voice), [computer use](computer-use) and the desktop app.

### nixos

NixOS has no `/lib64/ld-linux`, so the installer stops and points here. install with Nix instead:

```sh
nix profile install github:gvergnaud/bise                              # with flakes
nix-env -if https://github.com/gvergnaud/bise/archive/main.tar.gz      # without
```

in a NixOS or home-manager config: the input `github:gvergnaud/bise`, then `inputs.bise.packages.${pkgs.system}.bise` (or `overlays.default`, which adds `pkgs.bise`). update with `nix profile upgrade bise`, then run `bise` again in the folder: the agents move to the new version.

### let a coding agent do it

if you use Claude Code, Codex or another coding agent, paste this into it:

```text
read https://bise.dev/setup.md and set up bise for me
```

it installs bise, then runs `bise setup scan`, which lists what your machine already has (the API keys and where they are, your agent's model, its instructions file, skills and MCP servers) without ever printing a key. it shows you one short plan and waits for your yes before it writes anything.

## first run

go to a repo and start bise:

```sh
cd your-repo
bise
```

the first run has up to four short screens:

1. **hello**: any key goes on.
2. **the theme**: light or dark, picked from your terminal's background. `/theme` changes it later.
3. **the model**: only when bise found no key. you pick a provider, a model, and paste its key. bise checks it with one tiny call before it saves it. see [providers and keys](providers).
4. **how it works**: three lines about main, the agents and the inbox.

when bise finds keys in your environment (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, ...), it skips the model screen and uses them. `/welcome` replays these screens.

the model you pick becomes main's. the agents use the same one unless you change it, and the small jobs (titles, summaries) get a cheap model of the same provider. see [models and roles](models).

## what goes where

| path | what it holds |
|---|---|
| `~/.local/share/bise` | the app: one folder per version, `current` points to the one in use |
| `~/.local/bin/bise` | the command |
| `~/.bise/config.toml` | your [settings](config): models, approvals, voice |
| `~/.bise/auth.json` | your saved keys and sign-ins, readable only by you |
| `~/.bise/hubs/` | one folder per repo where bise runs: its agents and their threads |
| `~/.bise/sessions/` | the conversation logs |

`BISE_HOME` moves `~/.bise` somewhere else.

## Ghostty

in Ghostty, a few keys need one line each in Ghostty's config before they reach bise: `cmd+v` (paste an image), `cmd+f`, `cmd+k`, `cmd+a` and `cmd+↑↓`. `/setup` offers them, or:

```sh
bise setup ghostty --dry-run   # shows the change, writes nothing
bise setup ghostty             # adds the lines, after a backup of the file
```

## uninstall

```sh
bise uninstall           # removes the app, keeps ~/.bise
bise uninstall --purge   # also removes ~/.bise: your settings, keys and threads
```

> careful: `--purge` deletes your saved keys, sign-ins and every agent's thread.
