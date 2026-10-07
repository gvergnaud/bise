# bise on NixOS

bise's terminal app (the TUI, the hub, the agents) runs on NixOS,
x86_64 and aarch64. The desktop app does not (macOS only).

## Install

With flakes (`nix-command` and `flakes` enabled):

```sh
nix profile install github:gvergnaud/bise
```

Without flakes:

```sh
nix-env -if https://github.com/gvergnaud/bise/archive/main.tar.gz
```

In a NixOS or home-manager config, the flake's `packages.<system>.bise`
(or `overlays.default`, which adds `pkgs.bise`):

```nix
inputs.bise.url = "github:gvergnaud/bise";
# ...
environment.systemPackages = [ inputs.bise.packages.${pkgs.system}.bise ];
```

Try it without installing: `nix run github:gvergnaud/bise`.

`curl -fsSL https://bise.dev/install | sh` does not work on NixOS (the
build is for a usual Linux and NixOS has no `/lib64/ld-linux`): the
script stops and prints the command above. With `programs.nix-ld`
enabled it installs, but then add `openssl` to
`programs.nix-ld.libraries`.

## First run

You need `git` (bise works in git repos) and a model key. In a project
folder:

```sh
cd ~/code/some-repo
bise
```

bise asks for a key the first time (`bise login` stores one), or takes
it from the environment: `MISTRAL_API_KEY`, `ANTHROPIC_API_KEY`,
`OPENAI_API_KEY`... `bise doctor` checks the setup.

## Update

```sh
nix profile upgrade bise
```

Then run `bise` again in a folder where it runs: the running agents move
to the new version and keep going. `bise update` and `/restart latest`
say this too; they cannot write to the Nix store.

## What Linux does not have

These are macOS-only and are off on Linux, with one line where they
would show:

- the sandbox of `auto` approvals (macOS's `sandbox-exec`): on Linux,
  auto checks each command instead; main's feed says it once;
- voice mode: "not available on Linux yet";
- computer use (it drives macOS apps and the browser through macOS);
- the desktop app.

## How it works (for the maintainers)

`flake.nix` -> `nix/package.nix`: the release tarball of the GitHub
release (built on Ubuntu 22.04, glibc), with `autoPatchelfHook` (the
store's glibc and libgcc) and OpenSSL 3 in every binary's RUNPATH (the
Bend runtime `dlopen`s `libssl.so.3` for HTTPS). The app root is
`$out/lib/bise`, read-only: bise writes only in `~/.bise`.
`nix/sources.json` pins the release (URLs, sha256): the release's
`nix-sources.json` (`packaging/make-release.sh`), committed by
`packaging/publish-release.sh --publish`, so main's HEAD installs the
latest release (a tag's own commit carries the release before it).
`packaging/test-nix.sh <tarball>` checks the whole path on a NixOS
machine before a release exists.
