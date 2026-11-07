#!/usr/bin/env bash
# bins.sh — the Bend native binaries (repl-live, repl-scripted, sb-core,
# harness-demo), the V8 engine of a version (bend-jsrt, RELEASE build,
# BISE-133), and the viewer's xlsx grid (office-sheet: Univer, ~7 MB of
# JS built by office/sdk/build.ts into office/dist/office-sheet.js,
# never committed, architect m_37322). They are NOT in git (BISE-114): each one is built from
# its sources into a cache keyed by the CONTENT of those sources, shared
# by every worktree and by versions.sh, then copied where it is run.
#
#   scripts/bins.sh [--src <dir>] <name>...      <dir>/<name> = the build of <dir>'s
#                                          sources (default <dir>: this repo)
#   scripts/bins.sh path [--src <dir>] <name>    build if needed, print the cache file
#   scripts/bins.sh key [--src <dir>] <name>     the cache key (hash of the sources)
#   scripts/bins.sh prefetch [--src <dir>] <name>...  build the missing ones into the
#                                          cache, all at once (one process each:
#                                          a miss of all four is the longest
#                                          compile, not their sum); exit 1 when
#                                          one fails
#   scripts/bins.sh macos-target                 the oldest macOS every binary runs on
#   scripts/bins.sh minos <file>...              check each binary against it: exit 1
#                                          when one needs a newer macOS (or says
#                                          nothing); no-op off macOS
#
# A hit is a copy (~0.3 s); a miss is a bend compile (sb-core ~15 s,
# the REPLs 1-2 min, bend-jsrt a cargo build --release: ~1 min warm,
# ~5 min cold). Cache: $SB_BUILD_DIR/cache/<name>-<key> (default
# SB_BUILD_DIR=<bise dev dir>/build: ~/.bise/dev/build); of each name, the 3 newest
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
# the same dir as bise_home (rust/home): $BISE_HOME/dev, else ~/.bise/dev
STATE="${BISE_HOME:-$HOME/.bise}/dev"
BUILD="${SB_BUILD_DIR:-$STATE/build}"
CACHE="$BUILD/cache"
# the toolchains of the user's REAL home (the user database, never $HOME):
# the Rust tests run on a temp HOME (bise_home::test_home), where
# $HOME/.bend/bin is missing, `bend version` failed and the key of the
# same sources differed from a shell's (bins-key)
u="$(id -un)"; case "$u" in *[!A-Za-z0-9._-]*|"") TOOLS="$HOME" ;; *) eval "TOOLS=~$u" ;; esac
export PATH="$TOOLS/.cargo/bin:$PATH"
# bend: the one <src> pins (bend/VERSION, scripts/bend-pin.sh), put on
# PATH below once <src> is known
. "$REPO/scripts/bend-pin.sh"

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
    # a node build (office/sdk/build.ts --only): its entry, the build, its node options and the lockfile
    office-sheet) echo "node office/sdk/src/view/sheet.ts office/sdk/build.ts office/sdk/.npmrc office/sdk/package.json office/sdk/pnpm-lock.yaml" ;;
    *) say "unknown binary: $1 (repl-live repl-scripted sb-core harness-demo bend-jsrt office-sheet)"; exit 2 ;;
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
  if [ "${r%% *}" = node ]; then node_key "$1" ${r#node }; return; fi
  local x; x="$(cc_extra "$2")"
  set -- "$1" $r
  # only the dirs <src> has (find exits 1 on a missing one: pipefail)
  local d dirs=(); for d in "${@:3}"; do if [ -e "$1/$d" ]; then dirs+=("$d"); fi; done
  # no bend: no key (a key of "bend ?" was another key of the same sources)
  local v; v="$(BEND_NO_TELEMETRY=1 bend version 2>/dev/null)" && [ -n "$v" ] \
    || { say "no key: \`bend version\` failed (bend not in $TOOLS/.bend/bin nor PATH)"; return 1; }
  (cd "$1" && { find "${dirs[@]}" -type f \( -name '*.bend' -o -name '*.c' -o -name '*.js' \) -print0 | sort -z | xargs -0 cat
                echo "MACOSX_DEPLOYMENT_TARGET=$MACOSX_DEPLOYMENT_TARGET"; echo "$v"
                # only when there are some: the other targets keep their keys
                if [ -n "$x" ]; then echo "CC_EXTRA=$x"; fi; } \
     | shasum | cut -c1-12)
}

# the clang flags <name>'s C gets on top of bend's (scripts/cc-extra.sh),
# on stdout; none on every other target. linux-arm64 sb-core: clang 15
# (CI's, Ubuntu 22.04) dies on its C since Bend 2.0.39 ("Cannot scavenge
# register without an emergency spill slot", in the 101-argument musttail
# worklist function WL_FID_core_deliver_each_k766: clang < 19 has no
# preserve_none, so the arguments go on the stack), at every -O from -O1
# to -O3 (release run 42376828644). -O0 and clang 17/19 build it.
# FastISel (instead of SelectionDAG; the IR is still optimized at -O3)
# builds it: replay +20%, view_all +7% against clang 17 -O3. Drop it when
# CI's linux-arm64 clang is 17 or newer.
cc_extra() {  # <name>
  case "$(uname -s)-$(uname -m)-$1" in
    Linux-aarch64-sb-core) echo "-mllvm -fast-isel" ;;
  esac
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
# the key of a node build: each file's path and content (the grid's entry
# imports only types from the SDK: the SDK's own sources are not inputs)
node_key() {  # <src> <file>...
  local src="$1"; shift
  (cd "$src" && for f in "$@"; do if [ -f "$f" ]; then printf '%s %s\n' "$(shasum < "$f" | cut -c1-40)" "$f"; fi; done) | shasum | cut -c1-12
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
  if [ "${r%% *}" = node ]; then
    # its own node_modules from the lockfile, every time: a fresh worktree has
    # none, a kept one's can predate a new dependency (a no-op when current)
    (cd "$src/office/sdk" && pnpm -s install --frozen-lockfile --prefer-offline \
      && pnpm -s build --only "$name.js" --out "$out" >/dev/null && chmod +x "$out"); return
  fi
  if [ "${r%% *}" != cargo ]; then
    local x; x="$(cc_extra "$name")"
    if [ -z "$x" ]; then (cd "$src" && bend "${r%% *}" -o "$out" >/dev/null); return; fi
    (cd "$src" && BISE_CC="${CC:-clang}" BISE_CC_EXTRA="$x" CC="$REPO/scripts/cc-extra.sh" \
       bend "${r%% *}" -o "$out" >/dev/null); return
  fi
  # one target dir for every source: cargo decides freshness by mtime, and
  # a tree edited before a worktree was built would look fresh - the local
  # crates are always recompiled (the dependencies are kept)
  local t="$BUILD/target-jsrt"
  # one clean + build + copy at a time in that shared target: a gate's
  # `cargo clean -p bend-jsrt` deleted the binary a candidate was about to
  # copy (10.8's candidate red, review-gates m_33193). lockf (macOS),
  # flock (Linux CI), else unlocked as before
  mkdir -p "$BUILD"
  with_lock "$t.lock" /usr/bin/env CARGO_TARGET_DIR="$t" $JSRT_PROFILE /bin/bash -c '
    cd "$1/rust/jsrt" \
      && { cargo clean -q --release -p bend-jsrt -p bend-images -p bise-home 2>/dev/null \
           || cargo clean -q --release -p bend-jsrt 2>/dev/null || true; } \
      && cargo build -q --release \
      && { cp -c "$2/release/bend-jsrt" "$3" 2>/dev/null || cp "$2/release/bend-jsrt" "$3"; }' _ "$src" "$t" "$out"
}

# run <cmd...> holding <lock file> (kept: the next one waits on the same inode)
with_lock() {  # <lock file> <cmd...>
  local l="$1"; shift
  if [ -x /usr/bin/lockf ]; then /usr/bin/lockf -k "$l" "$@"
  elif command -v flock >/dev/null 2>&1; then flock "$l" "$@"
  else "$@"; fi
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
    # signed with bise's stable dev identity (dev-sign.sh): every copy of
    # it keeps one designated requirement, so macOS keeps its grants (a JS bundle: nothing to sign)
    if [ "$name" != office-sheet ]; then "$REPO/scripts/dev-sign.sh" sign "$name" "$f.tmp.$$"; fi
    mv "$f.tmp.$$" "$f"
    say "$name: built in $((SECONDS - s)) s ($f)"
    prune "$name"
  else
    # a build from before dev-sign.sh: signed now (a no-op once it is)
    if [ "$name" != office-sheet ]; then "$REPO/scripts/dev-sign.sh" sign "$name" "$f"; fi
    touch "$f"
  fi
  echo "$f"
}

# cached for every <name> at once, each in its own process: a release's
# miss compiled repl-live, sb-core, bend-jsrt and repl-scripted one after
# the other (680 s on CI's darwin-arm64, 1207 s on x86_64). Their tmp files
# are per name ($f.tmp.<pid>), the cargo one has its own target dir. Only
# the names that miss are started (a hit is a key: no process)
prefetch() {  # <src> <name>...
  local src="$1" n pids=() names=() bad=0 i k; shift
  for n in "$@"; do
    k="$(key "$src" "$n")" || return 1
    [ -x "$CACHE/$n-$k" ] && continue
    ( cached "$src" "$n" >/dev/null ) &
    pids+=($!); names+=("$n")
  done
  # ${a[@]+...}: macOS /bin/bash 3.2 calls an empty array unbound under set -u
  i=0
  for n in ${pids[@]+"${pids[@]}"}; do
    wait "$n" || { say "prefetch: ${names[$i]} failed"; bad=1; }
    i=$((i + 1))
  done
  return $bad
}

# <src>/<name> = the cache file (a new inode: a running binary keeps its own)
place() {  # <src> <name>
  local src="$1" name="$2" f out="$1/$2"
  # the grid's bundle goes where the page server reads it (pages/artifact BUNDLES)
  if [ "$name" = office-sheet ]; then out="$1/office/dist/office-sheet.js"; fi
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
case "${1:-}" in path|key|prefetch) cmd="$1"; shift ;; ""|-h|--help) sed -n '2,33p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;; esac
src="$REPO"
if [ "${1:-}" = --src ]; then src="$(cd "$2" && pwd)"; shift 2; fi
[ $# -gt 0 ] || { say "which binary? (repl-live repl-scripted sb-core harness-demo)"; exit 2; }
bend_dir_of_src="$(bend_bin_dir "$src" "$TOOLS")" || exit 1
export PATH="$bend_dir_of_src:$PATH"
case "$cmd" in
  key) key "$src" "$1" ;;
  path) cached "$src" "$1" ;;
  prefetch) prefetch "$src" "$@" ;;
  place) for n in "$@"; do
           [ "$n" = bend-jsrt ] && { say "bend-jsrt: 'bins.sh path bend-jsrt' (a dev tree runs its debug build)"; exit 2; }
           place "$src" "$n"
         done ;;
esac
