# Linux and Windows: the plan (ports)

Status: Linux in the releases (linux-nix: both arches built by
release.yml and listed in latest.json, NixOS through the flake; §7).
Windows: an assessment and a compile check (§6), no port. Later: a
static musl build (§8).

## 1. The short version

1. **Linux is close.** Bend ships linux-x64/arm64 builds, its C output
   already has `__linux__` branches, the V8 engine has prebuilt Linux
   libraries, and most of the Rust code already had Linux paths
   (`/proc` for the process table, `xdg-open`, `wl-paste`/`xclip`,
   OSC 52). The first build needed 4 small fixes (§3.2).
2. **One glibc floor: 2.34.** Built on Ubuntu 22.04 (glibc 2.35), every
   binary needs at most `GLIBC_2.34`: Ubuntu 22.04+, Debian 12+,
   RHEL/Rocky/Alma 9, Fedora 35+, Amazon Linux 2023, Arch, WSL2 Ubuntu.
   Not covered: Ubuntu 20.04, Debian 11, RHEL 8, Amazon Linux 2, Alpine
   (musl).
3. **Windows: WSL first.** The Linux build runs in WSL2 as is. A native
   port is blocked on the Bend runtime (POSIX-only C: `posix_spawnp`,
   `poll`, `pipe`, `mmap`, `pthread`, BSD sockets; Bend itself says
   "Linux, macOS or WSL"), then needs a new hub IPC, process control
   and installer on our side: months, not weeks.
4. Order: Linux release (CI + channel) → Linux tests in CI → WSL
   check + doc → Linux voice → native Windows only if users ask and Bend
   supports it.

## 2. Per component

| Component | Linux | Windows native | Effort (Linux / Windows) |
|---|---|---|---|
| Bend binaries (`repl-live`, `repl-scripted`, `sb-core`): `bend -o` = C + `clang -std=c11 -O3 -lpthread -lm` | works; Bend has linux-x64/arm64 releases (glibc). **clang ≥ 15 on arm64**: Ubuntu 22.04's clang 14 crashes on `sb-core` ("cannot scavenge register") | Bend does not support Windows. The runtime is POSIX (`posix_spawnp`, `waitpid`, `kill`, `pipe`, `poll`, `mmap`, `dlopen`, pthreads, BSD sockets; kqueue on macOS) | done / upstream Bend port (large) |
| `bend/vendor/http/effs/wire.c` (sockets, TLS by `dlopen` of OpenSSL) | works: `libssl.so.3` is in its list (OpenSSL 3: Ubuntu 22.04+, Debian 12+, RHEL 9) | Winsock + Schannel, or a bundled OpenSSL | none / medium |
| `bend-jsrt` (deno_core, rusty_v8) | works: prebuilt `librusty_v8` for x86_64/aarch64 gnu; links glibc dynamically (`libm.so.6`, `libc.so.6`) | rusty_v8 has `x86_64-pc-windows-msvc`; the crate compiles (§6) | none / small |
| `bise` Rust: TUI (crossterm, ratatui) | works | crossterm works on Windows | none / small |
| voice (cpal) | cpal links ALSA (`libasound.so.2`): bise did not start without it. **Now off on Linux** (a clear "not available on Linux yet") | cpal uses WASAPI | dlopen'd ALSA/PipeWire backend: medium / small |
| clipboard: `pbcopy` on local macOS, else OSC 52 | OSC 52 (most terminals); `wl-copy`/`xclip` would help on terminals that refuse it | OSC 52 in Windows Terminal | small / small |
| image paste: `osascript`, else `wl-paste`/`xclip` | works when one is installed | new (PowerShell `Get-Clipboard`) | none / small |
| Switchboard hub: Unix socket `hub.sock`, `sb` client | works | Rust std has no `UnixStream` on Windows: named pipes or `uds_windows`; every `std::os::unix` use (8 files) | none / medium |
| process control (`procs.rs`): `setsid`, sessions, `BISE_OWNERS` in the environment, snapshot by `ps -E` | works: `/proc/<pid>/{stat,environ}` path already there | Job Objects replace sessions; Toolhelp snapshot; no `pre_exec`, no signals | none / large |
| `ps -o command=` (daemon, repl, update, versions.sh) | procps `ps` has the same flags; not in every minimal image (Debian slim has none: hub still works, stale REPL detection degrades) | replace with sysinfo/Toolhelp | small / medium |
| terminal pane (`portable-pty`) | works | ConPTY through portable-pty | none / small |
| login-shell PATH (`tools_env.rs`, `$SHELL -i -l`, default `/bin/zsh`) | works; the default when `$SHELL` is unset should be `/bin/sh` | no login shell: the tools' PATH is the user's PATH | trivial / small |
| hub backup before a switch (`rsync -a`) | `rsync` is missing on minimal distros: the backup is skipped (returns None). Use `cp -a` or Rust | Rust copy | small / (same) |
| `bise doctor` (`sw_vers`, `sysctl`, `codesign`) | prints macOS-only checks as warnings; add `/etc/os-release`, glibc version, `libssl.so.3` | new checks | small / small |
| git worktrees | works | works (long paths, symlinks: `core.symlinks`) | none / small |
| install/update (`install.sh`, `bise update`) | **fixed**: `plutil` (JSON) → python3 or a sed fallback, `shasum` → `sha256sum` fallback; `~/.local/share/bise`, `~/.local/bin`, `.bashrc`/`.zshrc`/`.profile` | a PowerShell installer; the `sb -> bise` and `bend-harness` links need symlink rights: copies or `.cmd` shims; `current` pointer as a file, not a symlink | done / medium |
| data paths (`~/.bise`, `~/.local/share/bise`) | same | `%USERPROFILE%\.bise`, `%LOCALAPPDATA%\bise` | none / small |
| bash tool, `apply_patch`, `/tmp/bend-*-<port>` side files | work | needs Git Bash or a PowerShell tool; `%TEMP%` | none / large (the tool contract) |
| tests: tmux e2e, `gate.sh` (`sysctl`, APFS `cp -c`, `renice`) | tmux exists; the gate is macOS-shaped (clones, `minos`), would need a Linux mode to run in CI | no tmux | medium / large |

## 3. What was proven on Linux (this task)

### 3.1 Runs

Build container: Ubuntu 22.04, Rust 1.97.1, Bend 2.0.32
(linux release tarball), clang 15, OrbStack on an M-series Mac. The
same scripts as CI on macOS: `build-dist.sh HEAD` (→ `versions.sh` →
`bins.sh`), then `test-install.sh` (clean HOME, install, `--version`,
login, scripted + live sessions, a full turn answered by
`tests/fake_provider.py`, a Switchboard hub started, `sb list`, stop,
reinstall, uninstall).

| Target | Build | test-install.sh |
|---|---|---|
| linux-arm64 (native) | 30 MB tarball, 94 MB installed; V8 engine ~1 min (prebuilt V8), each REPL ~35 s | 31/31 in the build container; 31/31 on a clean `debian:12-slim` (no ALSA, no rsync) and a clean `rockylinux:9` (glibc 2.34) |
| linux-x86_64 (Rosetta, §3.3) | 30 MB tarball; cargo + V8 engine slower under emulation (~10 min) | 31/31 in the build container |

`objdump -T`: every binary's highest symbol version is `GLIBC_2.34`.
`ldd`: `bise` → libc, libm, libgcc_s (no ALSA any more); the Bend
binaries → libc, libm; `bend-jsrt` → libc, libm, libgcc_s. OpenSSL is
`dlopen`ed at the first HTTPS request (`libssl.so.3`).

### 3.2 What had to change

- `rust/tui`: cpal is not a dependency on Linux (it linked
  `libasound.so.2`: on a server, a container or WSL without ALSA, bise
  would not start). Voice says "not available on Linux yet".
- `install.sh`: `plutil` (macOS) read latest.json and the GitHub API
  answer → `json_get` (plutil, else python3, else a sed on the flat
  latest.json); `shasum` → `sha256_of` (shasum, else `sha256sum`).
  Without it `curl | sh` from a release channel fails on Linux.
- `bise update`: the same `sha256sum` fallback.
- `test-install.sh`: `stat -f %Lp` → GNU `stat -c %a` on Linux; the
  `--version` check wants `<os>-<arch>`, not `darwin-<arch>`.
- Build machine: clang ≥ 15 (arm64). Not a code change: the CI job
  installs `clang-15` and puts it first on PATH as `clang`.

### 3.3 x86_64 and Rosetta

Bend's linux-x64 binary is a Bun executable that dies with "Illegal
instruction" under Rosetta (AVX), so on a Mac the x86_64 container
cannot run `bend`. The C that `bend -o x.c` prints is the same on both
arches (its `#if` are per OS, not per arch): it was emitted in the arm64
container, compiled with clang in the x86_64 one into `bins.sh`'s cache
(same keys), and the rest (cargo, V8, `build-dist.sh`,
`test-install.sh`) ran as on CI. A real x86_64 runner (CI) runs `bend`
directly.

## 4. Decisions for the user

1. **glibc floor: 2.34** (build on Ubuntu 22.04). Covers the distros of
   §1.2. Lower (2.31: Ubuntu 20.04, Debian 11) = build in an older
   image, and OpenSSL 1.1 there (wire.c wants `libssl.so.3`). Older
   than that is not worth it. **musl (Alpine, static)**: not now: Bend's
   release links glibc, rusty_v8 has no musl prebuilt; a static musl
   build would mean building V8 from source.
2. **Distros we say we support**: proposed Ubuntu 22.04/24.04, Debian
   12/13, Fedora (current), RHEL-likes 9, and WSL2 Ubuntu; others
   "should work" when glibc ≥ 2.34 and OpenSSL 3.
3. **Windows: WSL first** (recommended): no native port until Bend runs
   on Windows; document "install WSL2 + Ubuntu, then curl | sh". Native
   is a separate project (§6).
4. **Linux voice**: off for now. Later: load ALSA with `dlopen` (a small
   capture shim, no link-time dependency), or PipeWire.
5. **CI cost**: Linux runners are 1x (macOS is 10x); the two Linux jobs
   could run on every push to main, not only on tags.
6. **Signing on Linux**: none (sha256 in latest.json, as today). Later,
   if asked: minisign/cosign signatures of the tarballs.
7. The **LGPL question** (glibc in V8) stays open: on Linux, V8 links
   glibc dynamically (`libm.so.6` in `ldd bend-jsrt`), nothing of glibc
   is inside our binaries; the static question is the macOS/Windows one.

## 5. CI (proposal, branch `sb/ports`)

`release.yml` gains two matrix rows, `linux-x86_64` (`ubuntu-22.04`) and
`linux-arm64` (`ubuntu-22.04-arm`), same steps: pinned Bend (the linux
tarballs and their sha256), Rust, caches, the V8 engine, `build-dist.sh`,
`test-install.sh`, the artifact. Linux-only steps: apt `clang-15 zsh`
(test-install uses zsh) and `clang` → clang-15 on PATH. The draft
release job keeps `--targets` darwin-only until the user decides: the
linux archives are built and tested, uploaded as artifacts, not put in
latest.json. To ship them: add them to `ci-release.sh --targets`.

## 6. Windows

**WSL2** (Ubuntu 22.04+): the Linux tarball, the Linux install. What to
check there: the TUI in Windows Terminal (kitty keyboard protocol: not
supported, the fallback path), OSC 52 copy (supported), `xdg-open`
(needs `wslu`: `wslview`), the Windows clipboard for image paste
(`powershell.exe Get-Clipboard` via interop), and repositories on
`/mnt/c` (slow git: recommend the WSL file system). A day's work, most
of it docs.

**Native**, in the order it blocks:

1. Bend runtime on Windows (upstream Bend): process spawn, pipes,
   poll, mmap, pthreads → Win32; sockets → Winsock; our `wire.c` TLS →
   Schannel or a bundled OpenSSL. Nothing starts without it.
2. Hub IPC: `hub.sock` → named pipes (or AF_UNIX through `uds_windows`),
   client and daemon.
3. Process control: sessions and `setsid`/`pre_exec`/signals → Job
   Objects (kill a tree when an agent stops), Toolhelp instead of `ps`
   and `/proc`.
4. Tools: the bash tool assumes `/bin/sh` (Git Bash or a PowerShell
   tool), `/tmp` side files, paths with `\`.
5. Install: PowerShell installer, no symlinks for `sb`/`current`,
   `%LOCALAPPDATA%`, code signing (SmartScreen).
6. Tests: the tmux e2e tests do not exist there.

Compile check (§6.1) measures step 2-5 on the Rust side.

### 6.1 Rust compile check for Windows

`cargo check --workspace --target x86_64-pc-windows-gnu --keep-going`
(mingw, in the Linux container): the two leaf crates stop it.

- `bise-home` (5 errors, `migrate.rs`): `std::os::unix` permissions
  (`from_mode`) and symlinks.
- `bise-session` (13 errors, `writer.rs`, `blob.rs`, `recorder.rs`):
  `OpenOptions::mode`/`DirBuilder::mode` (files made 0600/0700) and
  `libc::flock` (the one-writer lock).

Every other crate depends on them, so cargo did not check further. The
Unix-only uses outside tests, counted by `rg` (`std::os::unix`, `libc`,
`extern "C"`, `pre_exec`, Unix sockets, `*Ext` traits): switchboard 44,
tui 20, catalog 10, home 7, session 5, harness 5, jsrt 1, images and
plugins 0. The leaf crates are a day (cfg'd modes, `fs2`-style locks);
switchboard is the real work (hub socket, sessions, signals, §6 items
2-3). The V8 engine was not checked for Windows: rusty_v8 has an MSVC
prebuilt, not a mingw one.

## 7. Order of work

1. Land the Linux fixes on main (done with this doc).
2. Review `sb/ports`, merge the CI rows; first Linux artifacts from CI.
3. Put linux targets in the release (`ci-release.sh --targets`), test
   `curl | sh` on a clean Ubuntu, Debian, Fedora (container).
4. Small Linux gaps: `$SHELL` default, `rsync` → Rust copy, `doctor`
   checks, `wl-copy`/`xclip` for copy.
5. WSL: test and document.
6. Linux tests in CI (cargo tests + a Linux mode of the gate).
7. Linux voice (dlopen ALSA).
8. Native Windows: only after Bend runs on Windows.

Done (linux-nix): steps 1-3 (release.yml builds and releases both
Linux arches; latest.json has them; install.sh picks them), the
`$SHELL` default and `doctor` part of step 4, and NixOS through a binary
flake (`flake.nix`, `nix/package.nix`, docs/nixos.md,
`packaging/test-nix.sh`).

## 8. Later: a static musl build (Alpine, older glibc)

Not now; what it would take, for Alpine and the distros under glibc
2.34 (Ubuntu 20.04, Debian 11, RHEL 8):

- **V8 is the blocker.** rusty_v8 ships no musl prebuilt: `bend-jsrt`
  would build V8 from source for `*-unknown-linux-musl` (hours per arch
  on CI, a GN/ninja toolchain, a big cache), or bise runs JS another way
  on musl.
- **TLS.** `wire.c` `dlopen`s `libssl.so.3`; a static binary cannot
  `dlopen` the system's libraries. It needs OpenSSL linked in statically
  (or another TLS library) and the CA bundle found at run time
  (`/etc/ssl/certs`, `SSL_CERT_FILE`).
- **Bend binaries.** The C that `bend -o x.c` prints is plain C11 +
  pthreads: `musl-gcc` or `zig cc -target <arch>-linux-musl -static`
  should compile it; to check (`poll`, `posix_spawnp`, `dlopen` uses).
- **Rust.** The `*-unknown-linux-musl` targets are fine for bise itself
  (cpal is already off on Linux).
- **Cheaper for older glibc only:** build on an older image (glibc 2.28
  or 2.31) with the same dynamic linking; then `wire.c` also needs
  OpenSSL 1.1 (`libssl.so.1.1`) as a fallback, and rusty_v8's prebuilt
  must link against that glibc (to check).
