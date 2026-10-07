#!/usr/bin/env bash
# versions.sh — the Switchboard versions cache.
#
# A version is an immutable app root built from one git commit (or from
# the working tree, uncommitted changes included):
#   $SB_VERSIONS_DIR/<id>/  bise (+ bend-harness -> bise, one release;
#                           sb -> bise, the agents' command),
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
#   scripts/versions.sh build [<rev>|--tree]   # build (cached), print the version dir
#   scripts/versions.sh id [<rev>|--tree]      # the id a build would get
#   scripts/versions.sh list                   # built versions, newest first
#   scripts/versions.sh prune [<n>]            # keep the n (3) newest versions and
#                                        # those a hub marks or runs (after
#                                        # each new build too)
#
# Defaults: SB_VERSIONS_DIR=~/.local/state/switchboard/versions; the
# cargo target dir and bins.sh's cache of the Bend binaries (one per
# source hash, a Bend compile is 1-2 min) live in
# SB_BUILD_DIR=~/.local/state/switchboard/build.
# Commits are built in a temporary git worktree under /tmp (removed after).

set -euo pipefail
cd "$(dirname "$0")/.."
REPO="$PWD"
# the same dirs as bise_home (rust/home): $BISE_HOME/dev, ~/.bise/dev once
# migrated, else today's ~/.local/state/switchboard (XDG_STATE_HOME unread)
if [ -n "${BISE_HOME:-}" ]; then STATE="$BISE_HOME/dev"
elif [ -e "$HOME/.bise/migrated.json" ]; then STATE="$HOME/.bise/dev"
else STATE="$HOME/.local/state/switchboard"; fi
VERSIONS="${SB_VERSIONS_DIR:-$STATE/versions}"
BUILD="${SB_BUILD_DIR:-$STATE/build}"
export PATH="$HOME/.cargo/bin:$HOME/.bend/bin:$PATH"
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

  # release: the TUI draws ~10x faster than the debug build
  say "cargo build --release $id..."
  # one cargo target dir per SOURCE: cargo decides freshness by mtime,
  # and a commit checked out in a worktree (fresh mtimes) would otherwise
  # leave artifacts newer than the tree's edited files - the next --tree
  # build would ship stale Rust code. Worktrees are always freshly checked
  # out, so they can share theirs.
  local target="$BUILD/target-commits"
  [ "$src" = "$REPO" ] && target="$BUILD/target-tree"
  (cd "$src/rust" && CARGO_TARGET_DIR="$target" cargo build -q --release -p bend-harness)
  # the command is bise since BISE-165: a commit before it builds
  # bend-harness (decided by its sources: the shared target dir may hold
  # both names); either way the version dir has bise, and a bend-harness
  # link for the switchers and scripts that still look for it
  local exe=bend-harness
  grep -q '^name = "bise"' "$src/rust/harness/Cargo.toml" && exe=bise
  cp "$target/release/$exe" "$tmp/bise"
  ln -s bise "$tmp/bend-harness"
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
  if [ -d "$src/computer-use/extension" ]; then
    mkdir -p "$tmp/computer-use"; cp -R "$src/computer-use/extension" "$tmp/computer-use/extension"
    rm -rf "$tmp/computer-use/extension/test"
  fi

  # the V8 engine: the RELEASE build of this source's rust/jsrt (BISE-133:
  # ~60 MB, the debug one was 110 MB, 93% of a version), from bins.sh's
  # cache (one per jsrt source hash): a hard link, so the versions of one
  # engine share it. $tmp/bend-jsrt: the harness passes it to the runtime
  # (BEND_JSRT_BIN); the same file at the old path too: a runtime before
  # BISE-114 (an old commit) runs rust/jsrt/target/debug/bend-jsrt
  local js jk; jk="$("$REPO/scripts/bins.sh" key --src "$src" bend-jsrt)"
  js="$(bin_path "$src" bend-jsrt)"
  ln -f "$js" "$tmp/bend-jsrt" 2>/dev/null || cp -c "$js" "$tmp/bend-jsrt" 2>/dev/null || cp "$js" "$tmp/bend-jsrt"
  ln -f "$tmp/bend-jsrt" "$tmp/rust/jsrt/target/debug/bend-jsrt"

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
  rm -rf "$vdir"; mv "$tmp" "$vdir"
  CLEAN_TMP=""
  say "version $id built: $vdir ($(du -sh "$vdir" | cut -f1))"
}

# the ids of the versions the hubs name: their marks (versions.json:
# current, good, previous, failed) and the root each one runs (hub.root),
# in the legacy state dir and in ~/.bise/hubs; one per line (the last
# component of every path in them). No `[ -f ] && ...` as the loop's last
# command: with pipefail a missing last file (always $STATE/*/hub.root in
# the bise layout) failed the pipe, set -e ended prune, and the build
# exited 1 after "built": the hub did not switch (bug-restart)
marks() {
  local h="${BISE_HOME:-$HOME/.bise}" f
  for f in "$HOME/.local/state/switchboard"/*/versions.json "$HOME/.local/state/switchboard"/*/hub.root \
           "$h"/hubs/*/versions.json "$h"/hubs/*/hub.root "$STATE"/*/versions.json "$STATE"/*/hub.root; do
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
  build) build "${2:---tree}" ;;
  prune) prune "${2:-}" ;;
  id) if [ "${2:---tree}" = "--tree" ]; then tree_id; else rev_id "$2"; fi ;;
  list)
    [ -d "$VERSIONS" ] || exit 0
    for d in $(ls -t "$VERSIONS"); do
      [ -f "$VERSIONS/$d/VERSION" ] || continue
      printf '%s\t%s\t%s\n' "$d" "$(sed -n 's/^built=//p' "$VERSIONS/$d/VERSION")" \
        "$(sed -n 's/^subject=//p' "$VERSIONS/$d/VERSION")"
    done ;;
  *) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
