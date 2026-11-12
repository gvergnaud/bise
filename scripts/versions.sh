#!/usr/bin/env bash
# versions.sh — the Switchboard versions cache.
#
# A version is an immutable app root built from one git commit (or from
# the working tree, uncommitted changes included):
#   $SB_VERSIONS_DIR/<id>/  bise (+ sb -> bise, the agents' command; with the dev
#                           identity, bise -> bise.app/Contents/MacOS/bise,
#                           a signed bundle: scripts/dev-sign.sh),
#                           repl-live, prompts/ (tool-desc-*.txt,
#                           prompt-*.txt; at the top before the root
#                           cleanup), sb-core (when bend/hub/ exists),
#                           bend-jsrt (release, + a hard link at the old path
#                           rust/jsrt/target/debug/bend-jsrt),
#                           VERSION (id, commit, subject, built, bend_hash,
#                           jsrt, macos: the oldest macOS it runs on)
# id = <short commit>, or <short commit>-dirty-<hash of the changes>.
# A hub runs FROM a version dir: rebuilding the tree never changes a
# running system, only an explicit switch does.
#
#   scripts/versions.sh build [<rev>|--tree] [--also <name>]...
#                                        # build (cached), print the version dir;
#                                        # --also: one more of bins.sh's binaries
#                                        # into its cache, compiled with the others
#                                        # (build-dist.sh: repl-scripted)
#   scripts/versions.sh id [<rev>|--tree]      # the id a build would get
#   scripts/versions.sh list                   # built versions, newest first
#   scripts/versions.sh prune [<n>]            # keep the n (3) newest versions and
#                                        # those a hub marks or runs (after
#                                        # each new build too)
#
# Defaults: SB_VERSIONS_DIR=~/.bise/dev/versions; the cargo target dir
# and bins.sh's cache of the Bend binaries (one per source hash, a Bend
# compile is 1-2 min) live in SB_BUILD_DIR=~/.bise/dev/build.
# Commits are built in a temporary git worktree under /tmp (removed after),
# into one cargo target (target-commits) whose unchanged crates are kept
# (scripts/cargo_mtimes.py); bins.sh's misses compile beside the cargo build.

set -euo pipefail
cd "$(dirname "$0")/.."
REPO="$PWD"
# the same dir as bise_home (rust/home): $BISE_HOME/dev, else ~/.bise/dev
STATE="${BISE_HOME:-$HOME/.bise}/dev"
VERSIONS="${SB_VERSIONS_DIR:-$STATE/versions}"
BUILD="${SB_BUILD_DIR:-$STATE/build}"
BINS_ALSO=""
# bend: no direct call here; bins.sh runs the one each --src pins
# (bend/VERSION, scripts/bend-pin.sh)
export PATH="$HOME/.cargo/bin:$PATH"
# the oldest macOS the binaries run on (BISE-164), this repo's value
# (rust/.cargo/config.toml) for every commit: an old one has no
# rust/.cargo/config.toml, cargo takes it from the environment
export MACOSX_DEPLOYMENT_TARGET; MACOSX_DEPLOYMENT_TARGET="$("$REPO/scripts/bins.sh" macos-target)"

say() { echo "versions: $*" >&2; }

# bin_path <src> <name>: bins.sh's cache file of <name> for <src>, built if
# needed; exits (loudly) when bins.sh fails or prints no executable file
# (bug-bins-path: an empty path gave `cp: : No such file or directory`)
bin_path() {
  local p; p="$("$REPO/scripts/bins.sh" path --src "$1" "$2")" || { say "bins.sh path $2 failed"; exit 1; }
  [ -n "$p" ] && [ -x "$p" ] || { say "bins.sh path $2: no binary ('$p')"; exit 1; }
  echo "$p"
}

# what to remove when the script exits, built or failed: a RETURN trap
# does not run when set -e exits, and the /tmp/sb-build-* worktrees leaked
CLEAN_TMP="" CLEAN_WT=""
cleanup() {
  if [ -n "$CLEAN_TMP" ]; then rm -rf "$CLEAN_TMP"; fi
  if [ -n "$CLEAN_WT" ]; then
    git worktree remove --force "$CLEAN_WT" 2>/dev/null || true
    git worktree prune 2>/dev/null || true
  fi
}
trap cleanup EXIT

# id of the working tree
tree_id() {
  local head dirty
  head="$(git rev-parse --short HEAD)"
  if [ -z "$(git status --porcelain)" ]; then echo "$head"; return; fi
  dirty="$( { git diff HEAD; git ls-files --others --exclude-standard -z \
              | xargs -0 shasum 2>/dev/null; } | shasum | cut -c1-8)"
  echo "$head-dirty-$dirty"
}

rev_id() { git rev-parse --short "$1^{commit}"; }

# build the source dir $1 into version $2 (subject/commit from $3)
build_from() {
  local src="$1" id="$2" rev="$3" vdir="$VERSIONS/$2"
  mkdir -p "$BUILD/cache" "$VERSIONS"
  local tmp="$vdir.tmp.$$"
  rm -rf "$tmp"; mkdir -p "$tmp/rust/jsrt/target/debug"
  CLEAN_TMP="$tmp"

  # the Bend binaries and the V8 engine that miss bins.sh's cache, compiled
  # all at once while cargo builds the command (a release's miss was 680 s
  # on CI's darwin-arm64, one after the other). A failure here is told
  # again, loudly, by bin_path below (it compiles the missing one alone)
  local want=(repl-live bend-jsrt) pf
  if [ -f "$src/bend/hub/main.bend" ] || [ -f "$src/hub/main.bend" ]; then want+=(sb-core); fi
  if [ -f "$src/plugins/sheets/src/sheet.ts" ] || [ -f "$src/office/sdk/src/view/sheet.ts" ]; then want+=(office-sheet); fi
  "$REPO/scripts/bins.sh" prefetch --src "$src" "${want[@]}" $BINS_ALSO &
  pf=$!

  # release: the TUI draws ~10x faster than the debug build
  say "cargo build --release $id..."
  # one cargo target dir per SOURCE: cargo decides freshness by mtime,
  # and a commit checked out in a worktree (fresh mtimes) would otherwise
  # leave artifacts newer than the tree's edited files - the next --tree
  # build would ship stale Rust code. Worktrees are always freshly checked
  # out, so they share theirs, and cargo_mtimes.py gives the files that did
  # not change since its last build their mtime back: only the changed
  # crates rebuild (a fresh worktree rebuilt all of them on each commit)
  local target="$BUILD/target-commits" cargo_ok=1
  if [ "$src" = "$REPO" ]; then
    (cd "$src/rust" && CARGO_TARGET_DIR="$BUILD/target-tree" cargo build -q --release -p bend-harness) || cargo_ok=0
    target="$BUILD/target-tree"
  else
    (cd "$src/rust" && CARGO_TARGET_DIR="$target" python3 -B "$REPO/scripts/cargo_mtimes.py" run --src "$src" --target "$target" -- cargo build -q --release -p bend-harness) || cargo_ok=0
  fi
  wait "$pf" || say "bins.sh prefetch failed: compiling one by one"
  [ "$cargo_ok" = 1 ] || { say "cargo build failed"; exit 1; }
  # the command is bise since BISE-165: a commit before it builds
  # bend-harness (decided by its sources: the shared target dir may hold
  # both names); either way the version dir has bise
  local exe=bend-harness
  grep -q '^name = "bise"' "$src/rust/harness/Cargo.toml" && exe=bise
  # bise = a link to bise.app/Contents/MacOS/bise (dev-sign.sh app: bundle
  # id dev.bise.bise, the stable 'bise ambient dev' identity): macOS keys a
  # Documents/Desktop grant on the bundle id and signature, the same for
  # every version, so the user grants it once (a plain bise is a new path
  # each version: asked again). Everything runs that one file (TUI, hub,
  # the agents' sb); bise_home::exe_home_dir finds <root> from it. Only a
  # source that has exe_home_dir (an older bise can't find its root from
  # the bundle), and only with the identity: else a plain bise, as before.
  # The Bend binaries and bend-jsrt come signed from bins.sh's cache.
  if grep -q 'pub fn exe_home_dir' "$src/rust/home/src/lib.rs" 2>/dev/null \
     && "$REPO/scripts/dev-sign.sh" app "$tmp/bise.app" "$target/release/$exe" "$id"; then
    ln -s bise.app/Contents/MacOS/bise "$tmp/bise"
  else
    cp "$target/release/$exe" "$tmp/bise"
  fi
  # sb: the agents' command, the same binary called by that name
  ln -s bise "$tmp/sb"

  # the Bend binaries: bins.sh's cache (one per source hash, shared with
  # run.sh and the gate; a Bend compile is 1-2 min). This repo's bins.sh
  # builds any commit, old ones included (their committed binaries, up to
  # BISE-114, are never used: a version always compiles its own sources)
  # (an assignment, not `cp "$(...)"`: set -e sees a failed bins.sh only
  # there; bin_path also refuses an empty or missing path)
  local h; h="$("$REPO/scripts/bins.sh" key --src "$src" repl-live)"
  local b; b="$(bin_path "$src" repl-live)"; cp "$b" "$tmp/repl-live"
  # sb-core: the hub's decisions in Bend (hub/*.bend), when the version has them
  if [ -f "$src/bend/hub/main.bend" ] || [ -f "$src/hub/main.bend" ]; then
    b="$(bin_path "$src" sb-core)"; cp "$b" "$tmp/sb-core"
  fi
  # the prompts and tool descriptions, read at run time from the app root:
  # in prompts/ since the root cleanup, at the top before it (the old
  # commit's REPL reads them there)
  if [ -d "$src/prompts" ]; then cp -R "$src/prompts" "$tmp/prompts"
  else cp "$src"/tool-desc-*.txt "$src"/prompt-*.txt "$tmp/"; fi
  # the built-in plugins (plugins/: computer use's tools and skill) and
  # computer use's browser extension, read at run time from the app root
  # (a version without plugins/ had no computer use: the try of d3391ca)
  if [ -d "$src/plugins" ]; then cp -R "$src/plugins" "$tmp/plugins"; fi
  # bise's pages kit (kit.js, notes.js, pearl.js, the CSS): the page
  # server serves /kit from <app root>/kit (a version without it served
  # pages with no kit); an older commit had it at apps/ambient/kit, where
  # its own page server reads it
  if [ -d "$src/kit" ]; then
    cp -R "$src/kit" "$tmp/kit"
  elif [ -d "$src/apps/ambient/kit" ]; then
    mkdir -p "$tmp/apps/ambient"; cp -R "$src/apps/ambient/kit" "$tmp/apps/ambient/kit"
  fi
  # the office SDK's bundles: `bise office run` hands bend-jsrt
  # <app root>/office/dist/office.js; the page server serves office.js to
  # the viewer at /kit/office/ and to the views at ~kit/office/v1/ (docs/office.md)
  if [ -d "$src/office/dist" ]; then mkdir -p "$tmp/office"; cp -R "$src/office/dist" "$tmp/office/dist"; fi
  # and the xlsx grid (Univer), built, never committed: bins.sh's cache (xlsx-univer), in
  # Sheets' view folder (plugins/ copied above); an older source tree kept it in office/dist
  local grid
  if [ -f "$src/plugins/sheets/src/sheet.ts" ]; then
    grid="$("$REPO/scripts/bins.sh" path --src "$src" office-sheet)" || { say "bins.sh path office-sheet failed"; exit 1; }
    cp "$grid" "$tmp/plugins/sheets/ai.mistral.vibe/views/grid/office-sheet.js"
    rm -f "$tmp/plugins/sheets/ai.mistral.vibe/views/grid/office-sheet.js.key"
  elif [ -f "$src/office/sdk/src/view/sheet.ts" ]; then
    grid="$("$REPO/scripts/bins.sh" path --src "$src" office-sheet)" || { say "bins.sh path office-sheet failed"; exit 1; }
    cp "$grid" "$tmp/office/dist/office-sheet.js"; rm -f "$tmp/office/dist/office-sheet.js.key"
  fi
  if [ -d "$src/computer-use/extension" ]; then
    mkdir -p "$tmp/computer-use"; cp -R "$src/computer-use/extension" "$tmp/computer-use/extension"
    rm -rf "$tmp/computer-use/extension/test"
  fi

  # the V8 engine: the RELEASE build of this source's rust/jsrt (BISE-133:
  # ~60 MB, the debug one was 110 MB, 93% of a version), from bins.sh's
  # cache (one per jsrt source hash): a hard link, so the versions of one
  # engine share it. $tmp/bend-jsrt: the harness passes it to the runtime
  # (BEND_JSRT_BIN)
  local js jk; jk="$("$REPO/scripts/bins.sh" key --src "$src" bend-jsrt)"
  js="$(bin_path "$src" bend-jsrt)"
  ln -f "$js" "$tmp/bend-jsrt" 2>/dev/null || cp -c "$js" "$tmp/bend-jsrt" 2>/dev/null || cp "$js" "$tmp/bend-jsrt"

  {
    echo "id=$id"
    echo "commit=$(git rev-parse "$rev")"
    echo "subject=$(git log -1 --format=%s "$rev")"
    echo "built=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "bend_hash=$h"
    echo "jsrt=release-$jk"
    echo "macos=$MACOSX_DEPLOYMENT_TARGET"
    echo "repo=$REPO"
  } > "$tmp/VERSION"
  # whole for the viewer (the xlsx grid above): never a version dir without it
  "$REPO/scripts/runtime_check.sh" "$tmp" || { say "version $id is not whole (runtime_check.sh)"; exit 1; }
  rm -rf "$vdir"; mv "$tmp" "$vdir"
  CLEAN_TMP=""
  say "version $id built: $vdir ($(du -sh "$vdir" | cut -f1))"
}

# the ids of the versions the hubs name: their marks (versions.json:
# current, good, previous, failed) and the root each one runs (hub.root),
# in ~/.bise/hubs; one per line (the last component of every path in
# them). No `[ -f ] && ...` as the loop's last command: with pipefail a
# missing last file (always $STATE/*/hub.root) failed the pipe, set -e
# ended prune, and the build
# exited 1 after "built": the hub did not switch (bug-restart)
marks() {
  local h="${BISE_HOME:-$HOME/.bise}" f
  for f in "$h"/hubs/*/versions.json "$h"/hubs/*/hub.root "$STATE"/*/versions.json "$STATE"/*/hub.root; do
    if [ -f "$f" ]; then cat "$f"; echo; fi
  done | tr '",{}' '\n\n\n\n' | sed -n 's|^.*/\([^/]*\)/*$|\1|p'
}

# the built versions, newest first (by built=; in-progress *.tmp.* skipped)
by_age() {
  local d
  for d in "$VERSIONS"/*/; do
    d="${d%/}"; d="${d##*/}"
    case "$d" in *.tmp.*) continue ;; esac
    [ -f "$VERSIONS/$d/VERSION" ] || continue
    printf '%s %s\n' "$(sed -n 's/^built=//p' "$VERSIONS/$d/VERSION")" "$d"
  done | sort -r | cut -d' ' -f2
}

# prune [<keep>]: keep the <keep> (default SB_KEEP_VERSIONS, 3) newest
# versions (by built=), and any version a hub marks (marks) or a running
# process runs from (its command line); remove the others (BISE-133).
# Here-strings, not `printf | grep -q`: grep -q quits at the first match,
# printf gets SIGPIPE on a big ps output (80 KB here), pipefail fails the
# test, and the version in use was removed
prune() {
  local keep="${1:-${SB_KEEP_VERSIONS:-3}}" d n=0 in_use m real
  [ -d "$VERSIONS" ] || return 0
  real="$(cd "$VERSIONS" && pwd -P)"
  in_use="$(ps -axo command= 2>/dev/null || true)"
  m="$(marks)"
  for d in $(by_age); do
    n=$((n + 1))
    [ "$n" -le "$keep" ] && continue
    grep -qxF -- "$d" <<<"$m" && continue
    grep -qF -e "$VERSIONS/$d/" -e "$real/$d/" <<<"$in_use" && continue
    say "prune: $d"
    rm -rf "${VERSIONS:?}/$d"
  done
}

# prune after a build: in its own process (set -e whole: inside a `||`
# list it is off), and its failure does not fail the build: the version
# is built, the hub must switch to it
prune_after_build() {
  "$REPO/scripts/versions.sh" prune || say "prune failed (every version kept)"
}

# (a version built before BISE-165 has bend-harness only: still valid)
built() { { [ -x "$VERSIONS/$1/bise" ] || [ -x "$VERSIONS/$1/bend-harness" ]; } && [ -x "$VERSIONS/$1/repl-live" ] && [ -f "$VERSIONS/$1/VERSION" ]; }

build() {
  local what="${1:---tree}" id
  if [ "$what" = "--tree" ]; then
    id="$(tree_id)"
    built "$id" || { build_from "$REPO" "$id" HEAD; prune_after_build; }
  else
    id="$(rev_id "$what")"
    if ! built "$id"; then
      local wt="/tmp/sb-build-$id-$$"
      git worktree add -q --detach "$wt" "$id"
      CLEAN_WT="$wt"
      build_from "$wt" "$id" "$id"
      prune_after_build
    fi
  fi
  echo "$VERSIONS/$id"
}

case "${1:-}" in
  build)
    shift; what=--tree BINS_ALSO=""
    while [ $# -gt 0 ]; do
      case "$1" in
        --also) BINS_ALSO="$BINS_ALSO $2"; shift ;;
        *) what="$1" ;;
      esac
      shift
    done
    build "$what" ;;
  prune) prune "${2:-}" ;;
  id) if [ "${2:---tree}" = "--tree" ]; then tree_id; else rev_id "$2"; fi ;;
  list)
    [ -d "$VERSIONS" ] || exit 0
    for d in $(ls -t "$VERSIONS"); do
      [ -f "$VERSIONS/$d/VERSION" ] || continue
      printf '%s\t%s\t%s\n' "$d" "$(sed -n 's/^built=//p' "$VERSIONS/$d/VERSION")" \
        "$(sed -n 's/^subject=//p' "$VERSIONS/$d/VERSION")"
    done ;;
  *) sed -n '2,35p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
