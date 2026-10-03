---
title: updates
description: bise looks for a new release once a day and tells you in the inbox. you pick when it switches.
---

## how bise updates

once a day, when bise starts, it looks for a new release in the background. it never slows the start. when there is one, an item waits in your inbox: install it from there.

a running bise keeps its version until you switch, so an update never cuts an agent mid-task. `/restart` switches to the new version, and so does starting `bise` again in that folder. nothing is lost: the agents and their threads come back.

## by hand

in bise:

```text
/update
```

in a terminal:

```sh
bise update --check   # is there a newer release?
bise update           # download it, check its sha256, install it
bise --version
```

each version is installed in its own folder in `~/.local/share/bise/versions/`, and `current` points to the one in use. a few older versions are kept.

## turn the daily check off

```sh
export BISE_NO_UPDATE=1
```

`/update` and `bise update` still work.

## install the same version again

the installer is safe to run twice:

```sh
curl -fsSL https://bise.dev/install | sh
```
