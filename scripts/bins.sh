#!/usr/bin/env bash
# bins.sh — the Bend native binaries (repl-live, repl-scripted, sb-core,
# harness-demo) and the V8 engine of a version (bend-jsrt, RELEASE build,
# BISE-133). They are NOT in git (BISE-114): each one is built from
# its sources into a cache keyed by the CONTENT of those sources, shared
# by every worktree and by versions.sh, then copied where it is run.
#
#   scripts/bins.sh [--src <dir>] <name>...      <dir>/<name> = the build of <dir>'s
#                                          sources (default <dir>: this repo)
#   scripts/bins.sh path [--src <dir>] <name>    build if needed, print the cache file
#   scripts/bins.sh key [--src <dir>] <name>     the cache key (hash of the sources)
#   scripts/bins.sh macos-target                 the oldest macOS every binary runs on
#   scripts/bins.sh minos <file>...              check each binary against it: exit 1
#                                          when one needs a newer macOS (or says
#                                          nothing); no-op off macOS
#
# A hit is a copy (~0.3 s); a miss is a bend compile (sb-core ~15 s,
# the REPLs 1-2 min, bend-jsrt a cargo build --release: ~1 min warm,
# ~5 min cold). Cache: $SB_BUILD_DIR/cache/<name>-<key> (default
# SB_BUILD_DIR=<bise dev dir>/build: ~/.local/state/switchboard/build
# until the state moves to ~/.bise, BISE-160); of each name, the 3 newest
# and those used in the last hour are kept (BISE-133; a hit touches its
# file). bend-jsrt: `path` and `key` only (a dev tree runs its debug
# build, run.sh keeps it fresh; a bend-jsrt at the root would win over
# it), its cargo target is $SB_BUILD_DIR/target-jsrt. A failed compile keeps an existing
# <dir>/<name> (no toolchain: still runnable). A binary the EDR ate is
# rebuilt the same way: run the script again. MACOSX_DEPLOYMENT_TARGET
# (BISE-164) comes from rust/.cargo/config.toml, the one place for it:
# exported for the compile (bend -o calls cc) and part of the key.
set -euo pipefail
REPO="$(cd "$(dirname "$0")/.." && pwd)"
# the same dirs as bise_home (rust/home): $BISE_HOME/dev, ~/.bise/dev once
# migrated, else today's ~/.local/state/switchboard (XDG_STATE_HOME unread)
if [ -n "${BISE_HOME:-}" ]; then STATE="$BISE_HOME/dev"
elif [ -e "$HOME/.bise/migrated.json" ]; then STATE="$HOME/.bise/dev"
else STATE="$HOME/.local/state/switchboard"; fi
BUILD="${SB_BUILD_DIR:-$STATE/build}"
CACHE="$BUILD/cache"
# the toolchains of the user's REAL home (the user database, never $HOME):
# the Rust tests run on a temp HOME (bise_home::test_home), where
# $HOME/.bend/bin is missing, `bend version` failed and the key of the
# same sources differed from a shell's (bins-key)
u="$(id -un)"; case "$u" in *[!A-Za-z0-9._-]*|"") TOOLS="$HOME" ;; *) eval "TOOLS=~$u" ;; esac
export PATH="$TOOLS/.cargo/bin:$PATH"
export PATH="$TOOLS/.bend/bin:$PATH"

say() { echo "bins: $*" >&2; }

# the oldest macOS the binaries run on (BISE-164): the one value, in
# rust/.cargo/config.toml (cargo reads it there); every build path takes
# it from here. The value of THIS repo, also for an old commit (--src).
macos_target() {
  local v
  v="$(sed -n 's/^MACOSX_DEPLOYMENT_TARGET *= *{ *value *= *"\([0-9.]*\)".*/\1/p' "$REPO/rust/.cargo/config.toml" 2>/dev/null)"
  [ -n "$v" ] || { say "no MACOSX_DEPLOYMENT_TARGET in $REPO/rust/.cargo/config.toml"; exit 2; }
  echo "$v"
}
export MACOSX_DEPLOYMENT_TARGET; MACOSX_DEPLOYMENT_TARGET="$(macos_target)"

# the minos of a Mach-O file (the highest of its archs; LC_VERSION_MIN_MACOSX
# for old ones), empty when it has none
minos_of() {
  otool -l "$1" 2>/dev/null | awk '/cmd LC_BUILD_VERSION|cmd LC_VERSION_MIN_MACOSX/ { w = 1 }
    w && $1 ~ /^(minos|version)$/ { print $2; w = 0 }' | sort -t. -k1,1n -k2,2n -k3,3n | tail -1
}
# check <file>...: every one runs on MACOSX_DEPLOYMENT_TARGET
check_minos() {
  [ "$(uname -s)" = Darwin ] || { say "minos: not macOS, nothing to check"; return 0; }
  local f m bad=0 t="$MACOSX_DEPLOYMENT_TARGET"
  for f in "$@"; do
    m="$(minos_of "$f")"
    if [ -z "$m" ]; then echo "FAIL minos ?    $f (missing, or no macOS version in it)"; bad=1
    elif [ "$(printf '%s\n%s\n' "$m" "$t" | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)" != "$t" ] \
         && [ "$m" != "$t" ]; then echo "FAIL minos $m > $t  $f"; bad=1
    else echo "ok   minos $m  $f"; fi
  done
  return $bad
}

# the Bend sources of <src>: bend/ since the root cleanup, the root
# before it (an old commit built by versions.sh)
bend_dir() {  # <src>
  if [ -d "$1/bend/runtime" ]; then echo bend/; fi
}

# <name> <src> -> "<main .bend> <source dirs...>"
recipe() {
  local b; b="$(bend_dir "$2")"
  case "$1" in
    repl-live) echo "${b}runtime/repl-live.bend ${b}runtime ${b}core ${b}vendor" ;;
    repl-scripted) echo "${b}runtime/repl.bend ${b}runtime ${b}core ${b}vendor" ;;
    harness-demo) echo "${b}runtime/demo.bend ${b}runtime ${b}core ${b}vendor" ;;
    sb-core) echo "${b}hub/main.bend ${b}hub ${b}vendor" ;;
    # a cargo build (release): the crate and its path dependencies
    bend-jsrt) echo "cargo rust/jsrt rust/images rust/home" ;;
    *) say "unknown binary: $1 (repl-live repl-scripted sb-core harness-demo bend-jsrt)"; exit 2 ;;
  esac
}

# the key: every .bend file under the source dirs and the .c/.js of
# their foreign effects (bend/vendor/http/effs), by content (never a
# date: a fresh checkout has fresh mtimes), and the macOS target (a
# binary built for another one is another binary), and the bend version
# (a new toolchain compiles the same sources to another binary: without
# it, the cache kept serving the old compiler's builds). Packages (0x…)
# are immutable.
key() {  # <src> <name>
  local r; r="$(recipe "$2" "$1")"
  if [ "${r%% *}" = cargo ]; then jsrt_key "$1" ${r#cargo }; return; fi
  set -- "$1" $r
  # only the dirs <src> has (find exits 1 on a missing one: pipefail)
  local d dirs=(); for d in "${@:3}"; do if [ -e "$1/$d" ]; then dirs+=("$d"); fi; done
  # no bend: no key (a key of "bend ?" was another key of the same sources)
  local v; v="$(BEND_NO_TELEMETRY=1 bend version 2>/dev/null)" && [ -n "$v" ] \
    || { say "no key: \`bend version\` failed (bend not in $TOOLS/.bend/bin nor PATH)"; return 1; }
  (cd "$1" && { find "${dirs[@]}" -type f \( -name '*.bend' -o -name '*.c' -o -name '*.js' \) -print0 | sort -z | xargs -0 cat
                echo "MACOSX_DEPLOYMENT_TARGET=$MACOSX_DEPLOYMENT_TARGET"; echo "$v"; } | shasum | cut -c1-12)
}

# the key of a cargo binary: every file (path and content) of its crate
# dirs that exist in <src> (an old commit has fewer), without target/,
# the macOS target and the release profile below
jsrt_key() {  # <src> <dir>...
  local src="$1"; shift
  (cd "$src" && { local d; for d in "$@"; do if [ -d "$d" ]; then echo "$d"; fi; done; } \
     | xargs -I{} find {} -path '*/target' -prune -o -type f -print | LC_ALL=C sort \
     | while read -r f; do printf '%s %s\n' "$(shasum < "$f" | cut -c1-40)" "$f"; done
   echo "MACOSX_DEPLOYMENT_TARGET=$MACOSX_DEPLOYMENT_TARGET $JSRT_PROFILE") | shasum | cut -c1-12
}
# the engine's release profile (BISE-133, measured): plain release 68.1 MB
# (a rebuild of the local crates 2 s), thin LTO 68.1 MB, fat LTO + 1
# codegen unit 63.5 MB (rebuild 38 s, cold ~4 min): a new engine is rare.
# Never stripped (-25 MB, but the EDR deletes stripped binaries,
# build-dist.sh step 5). The debug build of a dev tree: 110 MB.
JSRT_PROFILE="CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1"

# a file copy that shares the blocks when it can (APFS clone): the engine
# is ~60 MB
copy() { cp -c "$1" "$2" 2>/dev/null || cp "$1" "$2"; }

# compile <name> of <src> into <out>
compile() {  # <src> <name> <out>
  local src="$1" name="$2" out="$3" r; r="$(recipe "$name" "$src")"
  if [ "${r%% *}" != cargo ]; then
    (cd "$src" && bend "${r%% *}" -o "$out" >/dev/null); return
  fi
  # one target dir for every source: cargo decides freshness by mtime, and
  # a tree edited before a worktree was built would look fresh - the local
  # crates are always recompiled (the dependencies are kept)
  local t="$BUILD/target-jsrt"
  (cd "$src/rust/jsrt" && export CARGO_TARGET_DIR="$t" $JSRT_PROFILE \
    && { cargo clean -q --release -p bend-jsrt -p bend-images -p bise-home 2>/dev/null \
         || cargo clean -q --release -p bend-jsrt 2>/dev/null || true; } \
    && cargo build -q --release) && copy "$t/release/bend-jsrt" "$out"
}

# keep the 3 newest of <name>, and those used in the last hour (the
# agents' worktrees each have their own sources). Always returns 0: its
# loop used to end on `[ -n "" ] && rm` (a recent one kept), so under
# pipefail `bins.sh path` exited 1 before printing the path of a build
# (a cache miss), and versions.sh copied from "" (bug-bins-path)
prune() {  # <name>
  local old
  ls -t "$CACHE/$1-"* 2>/dev/null | grep -v '\.tmp\.' | tail -n +4 \
    | while read -r old; do
        if [ -n "$(find "$old" -mmin +60)" ]; then rm -f "$old"; fi
      done || true
}

# build <name> of <src> into the cache if absent; print the cache file
cached() {  # <src> <name>
  local src="$1" name="$2" k f
  # (|| return: under `if ! cached` set -e is off)
  k="$(key "$src" "$name")" || return 1; f="$CACHE/$name-$k"
  if [ ! -x "$f" ]; then
    mkdir -p "$CACHE"
    say "$name: compiling (sb-core ~15 s, a REPL 1-2 min, bend-jsrt 1-5 min)..."
    local s=$SECONDS
    compile "$src" "$name" "$f.tmp.$$" || { rm -f "$f.tmp.$$"; return 1; }
    mv "$f.tmp.$$" "$f"
    say "$name: built in $((SECONDS - s)) s ($f)"
    prune "$name"
  else
    touch "$f"
  fi
  echo "$f"
}

# <src>/<name> = the cache file (a new inode: a running binary keeps its own)
place() {  # <src> <name>
  local src="$1" name="$2" f out="$1/$2"
  if ! f="$(cached "$src" "$name")"; then
    if [ -x "$out" ]; then say "compiling $name failed: the existing $out is kept"; return 0; fi
    say "compiling $name failed"; return 1
  fi
  # <out>.key: the sources' key it was built from (the Rust tests refuse a
  # stale sb-core: switchboard/src/core_fresh.rs)
  printf '%s\n' "${f##*/"$name"-}" > "$out.key"
  cmp -s "$f" "$out" && return 0
  cp "$f" "$out.tmp.$$" && mv -f "$out.tmp.$$" "$out"
}

cmd=place
case "${1:-}" in
  macos-target) echo "$MACOSX_DEPLOYMENT_TARGET"; exit 0 ;;
  minos) shift; check_minos "$@"; exit ;;
esac
case "${1:-}" in path|key) cmd="$1"; shift ;; ""|-h|--help) sed -n '2,28p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;; esac
src="$REPO"
if [ "${1:-}" = --src ]; then src="$(cd "$2" && pwd)"; shift 2; fi
[ $# -gt 0 ] || { say "which binary? (repl-live repl-scripted sb-core harness-demo)"; exit 2; }
case "$cmd" in
  key) key "$src" "$1" ;;
  path) cached "$src" "$1" ;;
  place) for n in "$@"; do
           [ "$n" = bend-jsrt ] && { say "bend-jsrt: 'bins.sh path bend-jsrt' (a dev tree runs its debug build)"; exit 2; }
           place "$src" "$n"
         done ;;
esac
