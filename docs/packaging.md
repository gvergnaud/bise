# Packaging and distribution — harness + Switchboard (future `bise`)

Status: design + prototype (`packaging/`). Nothing is
published. The command is still `bend-harness`; the scripts keep the name
in one variable (`CMD`) so the rename to `bise` is one line plus the data
dirs (see §8).

## 1. Recommendation (short)

1. **Ship native tarballs per target on a release channel, installed by
   `curl -fsSL …/install.sh | sh`** (the Claude Code / rustup model): no
   Node, no Rust, no Bend on the user's machine. The installed layout is
   the one `versions.sh` already uses (immutable version dirs + a pointer),
   so updates, `/version`, `/restart` and rollback reuse the Switchboard
   version switch.
2. **Sign with a Developer ID and notarize every macOS binary from day
   one.** This is a hard requirement, not polish: the prototype showed
   that the endpoint security agent (CrowdStrike) on a managed Mac deletes
   unsigned/ad-hoc binaries (§6).
3. Add a **Homebrew tap** next (same tarballs); an npm wrapper only if we
   need that reach later. Build on GitHub Actions, 4 targets (macOS
   arm64/x86_64, Linux x86_64/arm64); macOS arm64 first.

## 2. What runs at run time vs build time

Measured on commit df748eb (macOS arm64).

### Run time (must ship)

| File (app root) | From | Size | gz |
|---|---|---|---|
| `bend-harness` | Rust `rust/harness` (+ `tui`, `switchboard`), release | 5.5 MB | 2.1 MB |
| `repl-live` | `bend runtime/repl-live.bend -o` (native, links only libSystem) | 2.7 MB | 0.9 MB |
| `repl-scripted` | `bend runtime/repl.bend -o` (no provider; `--scripted`, smoke tests) | 2.5 MB | 0.9 MB |
| `sb-core` | `bend hub/main.bend -o` (the hub's decisions) | 2.2 MB | 0.9 MB |
| `rust/jsrt/target/debug/bend-jsrt` | `rust/jsrt` (deno_core/V8), **release** build at the debug path | 68 MB | 19 MB |
| `tool-desc-*.txt`, `prompt-*.txt` | repo root, read by relative path | 10 KB | |
| `VERSION` | id, commit, subject, built, bend_hash, target, channel | | |

Total: **77 MB installed, 25 MB tarball** — nothing stripped (§6).
V8 is 88 % of it. Stripping measured 60 MB / 23 MB: the tarball barely
changes, so no-strip costs little. (The versions dirs today hold the
110 MB *debug* jsrt: shipping the release one is already a big saving.)

**Not needed at run time**: the Bend compiler, cargo, the `.bend` sources,
`bend/laws/`, `bend/proofs/`. The Bend outputs are plain native executables.
Without sources, `/reload` restarts the REPL on the shipped binary (the
recompile step logs "source not found, keeping the binary") — fine.

System tools the code calls: `/bin/sh`, `git` (Switchboard: worktrees,
`/version`), `ps`, `kill`, `rsync` (version switch: not installed by
default on some Linux distros), `pbcopy` (clipboard, macOS only). The
installer warns when `git` is missing.

### Paths the code assumes

| Path | Who | Notes |
|---|---|---|
| app root = `BISE_APP_ROOT`, else the exe's dir when it holds `VERSION`, else (dev) the exe's dir and 3 parents, then a debug build's source tree; never the cwd | `harness/src/approot.rs` | C1 done (BISE-163) |
| relative `tool-desc-*.txt`, `prompt-*.txt`, `rust/jsrt/target/debug/bend-jsrt` | runtime (`tools.bend`, `repl-live.bend`, `main.bend`) | the binary `chdir`s to the app root in single-session mode |
| `sb-core` | `SB_CORE_BIN`, set from the app root by `sbd`; else `sb-core` next to the exe; `CARGO_MANIFEST_DIR/../../sb-core` in debug builds only | C4 done (BISE-163) |
| `~/.bend-harness/.env`, then `~/.vibe/.env` | keys (`MISTRAL_API_KEY`, `ANTHROPIC_FOUNDRY_API_KEY`) | real env wins |
| `~/.bend-harness/config.toml` | model, threshold (template written on first run) | |
| `~/.bend-harness/sessions/`, `tui.json`, `mcp-index.txt`, `skills-index.txt` | sessions, voice setting, connectors, skills | |
| `~/.agents/skills`, `~/.vibe/skills`, `$PWD/.agents/skills` | skills scan | `$PWD` is the app root in single-session mode (C2) |
| `~/.local/state/switchboard/<ws-id>/` (`SB_STATE_DIR`, `XDG_STATE_HOME`) | hub socket, journal, agents, worktrees, `bin/sb` (a link to the hub's `bise`: `sb` = `bise sb`) | |
| `~/.local/state/switchboard/{versions,build}` | `versions.sh`, `/version` | dev-only today (needs the repo) |
| `/tmp/bend-{prog,res,steer,interrupt}-<port>.*` | runtime side channels | world-readable `/tmp`: move under the state dir (C8) |
| `BEND_WORKDIR` | bash + apply_patch tools' dir | set per agent by the hub; **not set** in single-session mode (C2) |

### Provider — blocker for a public release

The default model is `claude-opus-5-5` through a private Anthropic
proxy (`foundry`, on a private network) with
`ANTHROPIC_FOUNDRY_API_KEY`. Outside that network nothing works by
default. The Mistral path (`api.mistral.ai/v1/chat/completions`, OpenAI mapping,
`MISTRAL_API_KEY`) is taken by any model name not starting with
`claude`. The prototype's `init` writes `model = "mistral-medium-latest"`
to `config.toml` and a live session announced that model (READY line).
The public default must change in code (C3) — and which model is a
product question (§10).

## 3. How others ship

| | Claude Code | Codex CLI | cargo-dist | Homebrew tap | plain curl \| sh + Releases |
|---|---|---|---|---|---|
| Install | `curl …/install.sh \| bash` (native), npm (legacy) | `npm i -g @openai/codex`, `brew install codex`, release tarballs | generates shell/PowerShell installers, brew formula, npm shim | `brew install org/tap/x` | one script |
| Artifact | one native binary per target | npm meta-package + per-platform optional deps holding the Rust binary | tarball per target | formula/cask pointing at tarballs | tarball per target |
| Layout | versions dir + `~/.local/bin` link | npm global | `~/.cargo/bin` or `$CARGO_HOME`-style | Cellar | ours |
| Update | background auto-update, `claude update` | npm/brew | `axoupdater` (opt-in) | `brew upgrade` | ours |
| Fit for us | **the model to copy** | npm needs Node: a poor fit for a 60 MB, multi-binary, non-JS app | assumes one cargo workspace; our artifacts come from 2 cargo workspaces + 3 Bend compiles → we'd fight it; borrow its conventions (receipts, installer flags) | good second channel, zero extra build | what the prototype is |

Codex's npm trick (one optional dependency per platform, `os`/`cpu`
fields) works, but it puts Node on the path of a tool that does not need
it and makes self-update a package-manager job — it conflicts with our
own version switch. Keep npm for later, as a thin shim that runs the
same installer.

## 4. Install layout (prototype = target design)

```
~/.local/share/bend-harness/          ($PREFIX; future ~/.local/share/bise)
  versions/<id>/                      immutable app roots (versions.sh layout)
  current -> versions/<id>
  bin/bend-harness                    launcher (sh)
  install.sh                          copy, for uninstall / reinstall
~/.local/bin/bend-harness -> $PREFIX/bin/bend-harness
~/.bend-harness/                      user data: keys, config, sessions (kept on uninstall)
~/.local/state/switchboard/           hubs (kept on uninstall)
```

The launcher resolves `current` **once** to the real version dir and
`exec`s that binary: the hub records a real dir in `hub.root`, and an
update that flips `current` never changes a running hub under it (same
rule as `versions.sh`: "rebuilding never changes a running system, only
an explicit switch does"). The installer keeps the last 3 versions, never
the current one nor one a running process uses.

The desktop app (`apps/desktop`, electron-builder) carries its own
runtime: `Contents/Resources/bin/` holds bise next to a version dir's
files (VERSION, repl-live, sb-core, bend-jsrt, prompts/, plugins/, kit/,
computer-use/extension), so `open bise.app` finds them with no env
(approot: VERSION next to the executable). They come from
`BISE_PACK_ROOT`, a pack-time input read only by the packaging scripts
(`scripts/desktop_try.py`, `scripts/desktop_app.py`: a snapshot made like
`versions.sh` makes a version dir); the pack fails without it. Hubs never
run from inside the `.app` (an app update renames it to `.old`, then
deletes it): the core installs that runtime once as
`$PREFIX/versions/<its VERSION id>` (`BISE_PREFIX`, else
`~/.local/share/bise`; update.rs `place`, the same code a release
install uses), without touching `current`, and starts hubs from there.
`BISE_APP_ROOT` is internal: an app never gets it.

## 5. The prototype

`packaging/`:

- **`build-dist.sh [<rev>] [--out dir]`** — calls `versions.sh build
  <rev>` (reuses its caches: a commit already built by `/version` costs
  seconds), adds `repl-scripted` (cached by the same Bend-source hash),
  the **release** V8 engine, drops `repo=` from `VERSION` (a dev path),
  adds `target=`/`channel=`, **never strips**, signs
  (`BISE_SIGN_ID` = a Developer ID → hardened runtime + timestamp; ad-hoc
  otherwise), waits 2 s and verifies every piece (a quarantine shows up
  as a missing file), runs `check-dist-pages.sh` on the app root (its hub
  on a throwaway HOME serves every stylesheet and script a page links,
  200 with its type: -31 and -32 shipped without `app/kit`), writes
  `bend-harness-<id>-<os>-<arch>.tar.gz` + `.sha256`. 20 s on a warm cache.
- **`install.sh`** — from an extracted bundle, `--from <tarball|dir>`,
  or (unpublished) `BISE_DIST_URL` download + sha256 check. Checks the
  target, clears the quarantine xattr, writes the version dir atomically
  (`.tmp` + `mv`), flips `current`, writes the launcher, links it into
  `~/.local/bin`, adds one marked PATH line to the shell's rc (zsh, bash,
  fish, sh; `--no-modify-path`), prunes old versions. `--uninstall`
  stops the hubs running from the prefix, removes prefix, link and PATH
  line, keeps user data unless `--purge`.
- **the launcher** — `--version` (from `VERSION`), `init` (key → 
  `~/.bend-harness/.env` mode 600, `config.toml` with a public model),
  first-run onboarding when a terminal session starts with no key
  anywhere, `uninstall`, `update` (stub), then sets `SB_LAUNCH_DIR` and
  `BEND_WORKDIR` to the user's folder and execs the binary.
- **the dev channel** (`install.sh --dev [--repo <dir>]`, BISE-129) — no
  bundle: `~/.local/bin/bise` -> `<dev dir>/bin/bise` (`~/.bise/dev`, or
  `~/.local/state/switchboard` before the move), a launcher that, at each
  run, runs the version the dev repo's hub runs: its `versions.json`
  `current` (what `/restart` and `sb restart` switch to), else its
  `hub.root`, else the newest built version. So a restart in the dev repo
  updates `bise` everywhere; `bise` in the dev repo attaches to its running
  hub. `BISE_DEV_VERSION=<id|dir>` runs another built version;
  `--launcher-root` prints the chosen dir (doctor's PATH line uses it);
  `--uninstall --dev` removes link and launcher only.
  `test-dev-install.sh <version dir> [<version dir>]` checks it in a fake
  HOME with throwaway hubs (tmux).
- **`test-install.sh <tarball>`** — installs into a clean fake HOME
  (`env -i HOME=/tmp/pk-home PATH=/usr/bin:/bin:/usr/sbin:/sbin
  SHELL=/bin/zsh`), never touching the real `~/.bend-harness`, state or
  live hubs.

**Result (commit df748eb, macOS arm64, unstripped + ad-hoc signed):
23/23 checks passed, no EDR deletion** — command
found by a new `zsh -i`; `--version` → `bend-harness df748eb
(darwin-arm64, commit df748eb7a635, built …)`; `init` (key file 600,
public model); single-agent sessions `--headless --scripted` and live
(READY, `model=mistral-medium-latest`), both exit when stdin closes;
Switchboard hub on a throwaway git workspace: started, `hub.root` = the
installed version dir, `sb list` through the agents' `bin/sb` link shows `main`,
main's `repl-live` runs from the version dir, `switchboard --stop` stops
hub and REPLs; reinstall is idempotent (one PATH line); uninstall
removes prefix, link and PATH line, keeps data.

Not tested: the interactive `init` prompt, the `curl | sh` download path,
a real model turn, TUI rendering, voice, Linux, x86_64.

## 6. Security software, signing, notarization — hard requirement

What happened during the prototype (2026-09-28, 16:24–16:27): a
**stripped** `sb-core` (ad-hoc signed or not) was deleted within a second
of being written, in every place (tarball extraction, a plain `cp` +
`strip`); the unstripped one was left alone. The user got CrowdStrike
alerts: files quarantined. The same class of problem already showed in
the old `release.sh` ("an EDR quarantine … must FAIL LOUDLY"). Consequences:

- **Never strip** (the prototype does not): unstripped + ad-hoc signed
  binaries passed the full test without an alert. Revisit stripping only
  once the binaries carry a Developer ID signature and are notarized.
- Run the packaging tests in CI (a GitHub macOS runner) or a VM by
  default; `test-install.sh` is written for that (it also works locally,
  unstripped).
- **Done (cli-sign, docs/issues/15-tcc-one-grant.md "Releases")**: the
  terminal release's darwin app root is `bise.app` (dev.bise.cli) + the
  `bise` link, signed by `scripts/dist_sign.py` (the signing core
  `scripts/mac_sign.py`, shared with the desktop's `desktop_sign.py`;
  entitlements in `packaging/entitlements/`), notarized in one submission
  (`scripts/notary.py`), bise.app and the helper stapled. The bullets
  below are the requirement it meets.
- **Every macOS binary is signed with a Developer ID Application
  certificate, hardened runtime, secure timestamp, then notarized**
  (`xcrun notarytool submit … --wait`, with an App Store Connect API
  key stored as CI secrets). Bare Mach-O files cannot be stapled; the
  ticket is checked online by Gatekeeper — or ship a signed `.pkg`
  later, which can be stapled. The build must not strip after signing.
- Entitlements under hardened runtime: `bend-jsrt` needs
  `com.apple.security.cs.allow-jit` (V8; maybe
  `allow-unsigned-executable-memory` — to verify); `bend-harness` needs
  `com.apple.security.device.audio-input` for voice; `repl-live` and
  `repl-scripted` need nothing: their TLS on macOS is the system's
  (Security.framework's SecureTransport, Keychain roots, `wire.c`), so
  library validation stays on and no Mac needs Homebrew's OpenSSL (a Mac
  without it made no HTTPS call before; -41's hardened runtime refused
  Homebrew's `libssl.3`). `BEND_LIBSSL` (an OpenSSL override) works in
  dev builds only; `BEND_TLS_CA` adds trust anchors (a PEM file).
  `test-install.sh`'s TLS turn and `tests/tls_trust.py` are the checks.
- **Microphone**: a CLI has no Info.plist; macOS asks for the permission
  on behalf of the *terminal app* (Terminal, iTerm2, Ghostty) — the
  prompt says "Ghostty would like to access the microphone". Document it
  in `init` (voice opt-in triggers the prompt right away, with an
  explanation), and handle "denied" with the System Settings path.
- `curl` does not set the quarantine xattr; a browser download does —
  the installer clears it (done), but signing + notarization is what
  makes Gatekeeper and the EDR accept the files.
- Why was only the stripped Bend binary flagged? Unknown: probably ML
  scoring of an unsigned, symbol-less binary with an odd layout. Ask the
  security team to allowlist our Team ID once we have one (§10).

## 7. Build matrix / CI (GitHub Actions)

Done for macOS (BISE-168): `.github/workflows/release.yml`, `darwin-arm64`
(macos-15) and `darwin-x86_64` (macos-15-intel), Developer ID signed and
notarized on a `v*` tag once the variable `RELEASE_SIGN` is `on` (else ad
hoc; release.yml's header), a
draft release made by make-release.sh on a `v*` tag (§11, BISE-220); the plan below still holds for
Linux (`linux-x86_64` ubuntu-24.04, `linux-arm64` ubuntu-24.04-arm) and
signing (BISE-169). Per job:

1. checkout the tag/commit; install a **pinned** Bend (`bend update`
   is `curl | sh`: pin the version/hash instead) and the Rust toolchain;
2. cache: cargo registry + target dirs, the Bend compile cache keyed by
   `bend_hash`, bins.sh's jsrt target `$SB_BUILD_DIR/target-jsrt`
   (deno_core downloads a prebuilt `librusty_v8` per target; the jsrt
   build is the long one; restored only when bins.sh's cache lacks the
   engine); saved on main only;
3. build = `build-dist.sh` (`versions.sh` under the hood, so CI and
   `/version` build the same thing);
4. macOS: sign (keychain from a base64 .p12 secret), notarize; Linux:
   decide glibc floor (build on the oldest supported distro or in a
   manylinux-like container) — Bend output links only libc; `cpal` needs
   ALSA (`libasound.so.2`) at run time → make voice a runtime-optional
   feature on Linux (dlopen) or a separate build flag;
5. smoke test = `test-install.sh` on the runner (clean HOME);
6. upload to a **draft** release; a final job writes the manifest
   `latest.json` (`{id, version, channel, targets: {darwin-arm64: {url,
   sha256, size}}}`) and publishes only after all targets pass.

Version names: today ids are short commits. Releases need an ordered
version (`0.3.0`, or a date `2026.09.28`) *plus* the commit; the version
dir can stay named by id, `VERSION` gains `version=`.

## 8. Update, onboarding, uninstall, rename

**Auto-update**, linked to the version system:

- `bise update`: fetch `latest.json` for the channel, download into
  `$PREFIX/versions/<id>` (sha256 + `codesign --verify`), flip `current`.
  Single sessions pick it up at next launch.
- A background check (launcher, at most once a day, never blocking the
  start; opt-out `BISE_NO_UPDATE=1` / config) downloads ahead of time
  and prints "bise 0.4 is ready — restarts next time".
- **Running hubs are never switched silently**: the hub announces the
  new version to `main`/the user; `/restart latest` (and `/version <id>`)
  switch with the existing `sbswitch` probation + automatic rollback.
  In an installed build, `/version` lists *released* versions from the
  manifest (plus the installed ones), instead of git commits; building
  from a repo stays a dev mode (when `VERSION` has `repo=`).

**First run (`bise init`)**, run automatically when an interactive
start finds no key: API key (hidden input, stored mode 600, validated by
one cheap API call), model choice, `git` check, voice opt-in (mic
prompt), shell completion, "try `bise` here or `bise switchboard` in a
project". Keeps reading `~/.vibe/.env` (Vibe users have nothing to do).

**Uninstall**: `bise uninstall [--purge]` (done in the prototype): stops
hubs of the install, removes prefix/link/PATH line; `--purge` removes
keys, sessions, hub state. Brew: `brew uninstall`.

**Rename to bise**: command `bise`; prefix `~/.local/share/bise`; user
data `~/.bise/` (was `~/.bend-harness/`), state `~/.local/state/bise/`
(was `…/switchboard/`); env vars `BISE_*` next to the old `BEND_*`/`SB_*`
for one release; on first run, move the old dirs (or read both). `sb`
stays the agents' command. In the scripts it is the `CMD=` variable and
the default paths.

## 9. Code changes needed (not made — only new files in this task)

- **C1** (done, BISE-163) `app_root()`: an installed binary must use its own dir, never a
  `repl-live` found in the cwd (running `bise` inside the dev repo would
  pick the dev tree's REPL). Prefer an explicit `BISE_APP_ROOT` (set by
  the launcher), else the exe dir when it holds `VERSION`, else today's
  lookup.
- **C2** single-session mode: set `BEND_WORKDIR` (and the skills scan's
  `$PWD`) to the launch folder before the `chdir` to the app root —
  today the bash tool of an installed session would run in the install
  dir. The launcher works around it by exporting `BEND_WORKDIR=$PWD`;
  the skills scan still sees the app root.
- **C3** default model/provider: no private endpoint as the built-in
  default; public default = a Mistral model; the foundry proxy only via
  config/env.
- **C4** (done, BISE-163) `core_bin()` fallback: drop `CARGO_MANIFEST_DIR` (a build-machine
  path) outside dev builds; look next to the exe.
- **C5** (`--version` done, BISE-165: `bise --version` reads `VERSION`) `--version` / `version` in the binary (reads `VERSION`, else the
  crate version + commit via `env!`), and `init`/`update`/`uninstall`
  subcommands (the launcher implements them today).
- **C6** `bend-jsrt`: look it up next to the exe (`bend-jsrt`), keep
  `rust/jsrt/target/debug/bend-jsrt` as the dev fallback; version it in
  `versions.sh` by a hash of `rust/jsrt` (today every version hard-links
  the live tree's *debug* engine, whatever the commit).
- **C7** `versions.sh`: also build `repl-scripted`; ship the release jsrt;
  add `version=`/`target=`; a `--dist` mode could replace
  `build-dist.sh` and `release.sh` (three build paths today:
  `run.sh`, `versions.sh`, `release.sh`).
- **C8** `/tmp/bend-*-<port>` side-channel files → a per-user dir
  (`$TMPDIR` on macOS is per-user; or the state dir): other local users
  can read programs and results in `/tmp` today.
- **C9** `/version` and `/restart latest` in installed mode: releases
  from the manifest instead of git + `versions.sh` (§8).
- **C10** Linux: `rsync` dependency of the switch (use `cp -R` or Rust),
  `pbcopy` (OSC 52 / `wl-copy`/`xclip`), ALSA for `cpal`.
- **C11** the remaining French user-facing messages (`introuvable`,
  `argument inconnu`) → English.
- **C12** ~~`release.sh` strips every binary~~: done, `release.sh` deleted
  (BISE-164); `build-dist.sh` is the one packaging path.

## 10. Questions for the user

1. **Open source or not?** The repo (`gvergnaud/bise`) license,
   and Bend's (`bendlang/bend`): can the binaries be redistributed? This
   decides public GitHub Releases vs a private bucket/CDN.
2. **Mistral green light**: is this a Mistral product (Mistral's Apple
   Developer ID, GitHub org, domain for `install.sh`, legal review), or a
   personal project? Who owns the signing certificate?
3. **Default model** for users outside Mistral: which Mistral model, and
   is the foundry/Claude path kept at all in public builds?
4. **Connectors / tools** (`mcp-index.txt`, `tools.*` in run_typescript):
   what ships to a public user, and what needs an account?
5. **Targets for v1**: macOS arm64 only, or all four from the start?
6. **Auto-update default**: on (Claude Code style) or ask at `init`?
   Channels (stable / nightly)?
7. **Security team**: allowlist request for our Team ID once signed; and
   may we run packaging tests on a dedicated VM/CI only (agreed after
   today's alerts)?
8. **Telemetry / crash reports**: none, opt-in, opt-out?
9. `bise` on Homebrew / npm: check the names are free before the rename
   (the pitch notes open checks).

## 11. The channel: GitHub Releases of gvergnaud/bise (BISE-217)

User decision: the releases are GitHub Releases of `gvergnaud/bise`; the
repo stays private for now (friends are collaborators), public later.

**Two release cycles from one main** (desktop-on-main, architect
m_19472). The desktop app's code (`apps/desktop`) lives on main, but it
releases apart from the terminal:

| | terminal (`bise`) | desktop app |
|---|---|---|
| tags | `vYYYY.M.D-N` (`v*`) | `desktop-vYYYY.M.N` |
| workflow | `release.yml` | `desktop-release.yml`, OFF until the repo variable `DESKTOP_RELEASE` is `on` |
| channel | `releases/latest/download/latest.json` | `releases/download/desktop-latest/desktop.json` (a rolling tag) |
| reader | `bise update`, the hub's hourly check | the desktop core (`app_update`), `release::parse_desktop` |

The rules (`tests/release_split_test.py`, `check-release.py`, the
`release.rs` tests): the terminal's app root and tarball never hold
`apps/`, `dev/`, `node_modules` or Electron; `latest.json` never has a
`desktop` key; a desktop release is a draft and never GitHub's 'latest'
(that would move every terminal's channel); `desktop.json` has `kind:
desktop` and names the terminal version of the runtime it carries
(`runtime_id`). A terminal release never waits on the desktop app, and
the desktop app ships only when the user says so.

**The desktop app's own updates** (desktop-dist v7, his picks). Two
rolling tags, same `desktop.json` shape (dist-ci's
`scripts/desktop_feed.py` writes it): `desktop-latest` (stable) and
`desktop-beta`. Every build goes to beta first and "promote" moves
`desktop-latest` to the same build, so beta is always stable or newer
and a beta app reads `desktop-beta` alone (his pref `desktop_channel`,
Settings › General's 'beta builds'). `"paused": true` offers nothing.
The app downloads in the background, checks the zip's sha256 and the
new app's signature against its own designated requirement (a Developer
ID app also needs Gatekeeper's yes), then installs at the first of: his
quit, a quiet moment (`apps/desktop/src/main/update/moment.ts`), his
click. `<name>.app.old` stays until the new app ran; the hubs never stop.
A hub newer than the app's runtime makes the core read the channel at
once. A self-signed test build refuses the first Developer ID one: it is
downloaded once by hand.

- **The channel** is the latest release's download URL,
  `https://github.com/gvergnaud/bise/releases/latest/download` (the
  Rust const `release::DIST_URL`, stamped into install.sh by
  make-release.sh). A new release is what every install reads next.
- **Release cycle** (BISE-220: CI is the one publisher):
  1. `packaging/publish-release.sh [vX.Y.Z]` (the
     user): tags a pushed commit (`--rev`, default HEAD), pushes the tag,
     watches the release.yml run (`gh run watch`), then shows the draft
     and checks its latest.json and install.sh.
  2. The tag's run of release.yml: builds darwin arm64 + x86_64,
     test-install.sh on a clean HOME, then `packaging/ci-release.sh`:
     make-release.sh (install.sh stamped with the channel, latest.json,
     tarballs + .sha256: the files install.sh and `bise update` read),
     `check-release.py`, a **draft** release. A rerun replaces a draft's
     files; a published release is never touched. The draft has
     darwin-arm64 + linux (fast-release: the desktop reads darwin-arm64);
     darwin-x86_64, ~2x slower, joins it when done (`ci-release.sh
     --attach`: its archive, then latest.json with the 4 targets, notes
     kept, even on a release published already). The run ends after it,
     so publish-release.sh's watch sees the 4 targets. Caches are saved
     by main's runs only (a push that changes rust/, bend/, packaging/,
     and a nightly run); tags restore them.
  3. `publish-release.sh vX.Y.Z --publish` (or `gh release edit vX.Y.Z
     -R gvergnaud/bise --draft=false`): the draft becomes the latest
     release. A draft is not "latest": no install or update sees it before.
  4. Linux (linux-nix): release.yml also builds `linux-x86_64` and
     `linux-arm64` on Ubuntu 22.04 (glibc 2.34 floor), so latest.json
     has 4 targets and install.sh picks `<os>-<arch>`. NixOS takes the
     same tarballs through the flake (`flake.nix`, `nix/package.nix`,
     docs/nixos.md): make-release.sh writes `nix-sources.json` (the
     tag's URLs + sha256, from latest.json; check-release.py checks they
     agree), and `--publish` commits it as `nix/sources.json` ("nix:
     sources for <version>"): the one writer, never by hand.
  Private repo: macOS minutes cost 10x, so the workflow runs on tags and
  by hand, not on every push. `--local` is the emergency path (CI down):
  today's local build (this Mac's arch only, `--add` another archive),
  make-release.sh, check-release.py, `gh release create` (published, or
  `--draft`).
- **What's new** (update-card): every release carries 3-5 plain lines
  for the users. Whoever cuts the release writes them in a file (one
  line each; blank lines and `#` comments dropped; what users can do now,
  no commit hashes) and passes it: `publish-release.sh vX.Y.Z --publish
  --whats-new <file>` (CI path: latest.json is re-uploaded with a
  `"notes"` list, and the lines go on top of the GitHub notes), or
  `--local ... --whats-new <file>` (make-release.sh `--whats-new`).
  Forgotten, or to fix them: `publish-release.sh vX.Y.Z --whats-new
  <file>` on the published release. An installed bise checks latest.json
  once an hour for the whole machine (`bise update --manifest`, one small
  GET, one hub at a time under a flock; `BISE_RELEASE_CHECK_SECS`; off
  with `BISE_NO_UPDATE=1`; never in bise's source tree). The offer is the
  machine's (update-flow, `bise_home::release_offer`): every hub plans it
  from the same files every 30 s. A newer release is ONE inbox card
  for the machine (update-dot, Gabriel m_46040, architect m_46059): each
  hub opens its own `update` card for it (place `release:<id>`, its
  changelog typed on the card: `Card.release`, the manifest's notes cut
  at 16 KB, and `Card.link` the release page) and the window lists one
  of them (bise's own when the home hub has one; grouped by that place
  for the list, the badge and the notifications: `shared/update.ts`).
  `1 update now · your agents keep running` answers that one hub: it
  installs the release once, its card closes at once (`updating`), then
  every project switches to it one at a time (probation, agents kept),
  and the app's own update installs with it and reopens the window where
  he is. `2 later` writes `update-later`: no card for that release
  again, in any project. Each hub closes its own card when the machine's
  offer leaves ready (`updating`, `later`, `update failed`, `updated`),
  so an answer anywhere clears every card within 30 s. While it runs,
  each hub says it on `hub/bise_update` (rows::BiseUpdate: installing,
  updating, failed); the desktop core folds every project's into the
  machine's one line (`core/machine_update`), drawn in the window's
  status pill and the TUI's status line. A project whose switch rolled
  back doesn't retry that release. `/update auto on` (Settings ›
  general, `Update automatically`, prefs `auto_update`, off by default)
  installs a ready release by itself. `/update` forgets `later`, checks
  now and opens the card. The desktop app's own
  updater still swaps the app alone when no newer bise waits.
- **Private repo**: a plain download of an asset answers 404. install.sh
  and `bise update` (the daily check, `/restart latest`) then ask
  `gh release download` (the GitHub CLI, `gh auth login`), else the API
  with `GH_TOKEN`/`GITHUB_TOKEN` (the asset's API URL with
  `Accept: application/octet-stream`; the token goes to curl on stdin,
  never in `ps`). Neither: an error that says to run `gh auth login`.
  `BISE_GITHUB_API` changes the API base (tests; GitHub Enterprise gets
  `<host>/api/v3`).
- **Public repo**: the plain downloads work; nothing to change.
- **Install line**: `curl -fsSL https://bise.dev/install | sh`: the site
  serves a copy of the stamped install.sh (`site/install.sh`;
  publish-release.sh says when it is stale). Private: `gh auth login`
  first. Without the site: `gh release download -R gvergnaud/bise -p
  install.sh -O - | sh`.
- **Test**: `packaging/test-release-gh.sh` (a python stand-in for GitHub
  and a stub `gh`, fake HOMEs, ~40 s): public, private without auth,
  with gh, with a token, a bad token; ci-release.sh on stub archives of
  both arches and check-release.py's failures; publish-release.sh
  against a stateful stub `gh` whose `run watch` runs ci-release.sh
  (tag, draft, `--publish`, a failed run, `--local`), then an install
  and an update from the published files.

## 12. The desktop app's release: signed, notarized, two channels (desktop-dist)

The page desktop-dist (his go). One workflow, `.github/workflows/desktop-release.yml`,
OFF until the repository variable `DESKTOP_RELEASE` is `on`.

- **The runtime**: a desktop release carries a published terminal release, the
  `v*` tag on its commit (or the input `runtime`): its darwin-arm64 archive,
  sha256 checked (`scripts/desktop-release.sh runtime`). Its commit is in the
  app's history and the seams (`rust/proto`, `rust/home`,
  `rust/tui/src/ambient`) are unchanged since, else refused (a dry run says it
  and goes on).
- **The build** (`scripts/desktop-release.sh build`, `scripts/desktop_release.py`):
  the native helper (macOS 14 floor), `pnpm build`, electron-builder `--dir`
  (ad hoc), then `scripts/desktop_sign.py`: every piece of code signed on its
  own, deepest first, never `--deep`, hardened runtime, a timestamp, the
  entitlements (`packaging/entitlements/desktop.plist`: JIT, the
  microphone, Apple Events) on the executables and the apps; before it,
  `cli_bundle` makes the runtime's `bin/bise.app` (dev.bise.cli, the
  terminal release's bundle, its `bise.plist`) with `bin/bise` a link into
  it, bend-jsrt gets `bend-jsrt.plist`, repl-live the desktop's default
  (`scripts/mac_sign.py`). Then
  notarytool (App Store Connect API key) and stapler on the app, the zip for the
  updater (`ditto`), the DMG (dmgbuild: the designer's window,
  `apps/desktop/resources/dmg/layout.json` and `background.png` + `@2x`),
  signed, notarized, stapled. Gates: `codesign --verify --deep --strict`, the
  runtime flag, the mounted DMG; signed: `spctl` on the app and the DMG,
  `stapler validate`, `syspolicy_check distribution`.
- **The tag**: only from `scripts/desktop_tag.py <desktop-vYYYY.M.N> [<sha>]
  [--notes-file <f>]`, in a clean checkout at that sha: the desktop app's
  full end-to-end (`scripts/nightly_desktop.sh`, docs/desktop-try.md) run
  on that exact sha (its last line in `~/.bise/run/nightly/desktop-runs.tsv`,
  else it runs then, in a heavy slot) with every step run and every red
  owned: in `tests/known_reds.txt` as it is at that sha, with its owner. A
  red outside the file refuses the tag, naming it. Then an annotated tag:
  the notes, then `nightly-desktop: green <sha> <when>` or
  `nightly-desktop: owned-reds <N> <sha> <when> (owners: …)`. It prints the
  run's verdict and the open known-reds count, and never pushes. The build
  job's first step (`desktop_tag.py check`) fails a lightweight tag or one
  without one of those lines for its own commit: 'cut desktop tags with
  scripts/desktop_tag.py'; it prints the line. The release body drops it.
- **Runs**: a `desktop-vYYYY.M.N` tag (or action `build`): signed + a DRAFT
  release (prerelease, never latest; its body the annotated tag's message).
  Action `dry-run`: ad hoc, no secret, no release, the files as a run
  artifact. Action `publish` (tag, beta|stable), `pause`, `resume` (channel):
  `scripts/desktop_publish.py`.
- **The feed** (`scripts/desktop_feed.py`, agreed with dist-updater and
  dist-site): `desktop.json` on two rolling releases, `desktop-beta` and
  `desktop-latest`, each with `bise-mac-arm64.dmg` (the site's static link)
  uploaded before the feed. A build goes to beta first; stable never ahead of
  beta; no channel goes back (N+1 from N's commit instead); `paused: true`
  stops every offer. `built` is the app's VERSION `built=` byte for byte.
  Every `gh release create/edit` says `--latest=false`: GitHub's latest is
  the terminal's channel.
- **Secrets** (environment `desktop-release`, his approval on each run):
  `MAC_CERT_P12_BASE64`, `MAC_CERT_PASSWORD`, `APPLE_API_KEY_P8`,
  `APPLE_API_KEY_ID`, `APPLE_API_ISSUER`, `APPLE_TEAM_ID`. The certificate
  lives in a throwaway keychain, the key in a 0600 file, both deleted at the
  end of the run.
- **Test**: `tests/desktop_release_test.py` (pure parts + an ad hoc signing of
  a fake bundle), `tests/release_split_test.py`.
