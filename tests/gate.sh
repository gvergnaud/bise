#!/usr/bin/env bash
# The agents' gate (docs/loop-speed.md), run in the
# foreground, never `sleep N; tail` in a loop:
#   gate.sh [quick]   what the tree's changes touch, ~5-10 s warm: clippy -D
#                     warnings (next to the tests, in $target/clippy), the
#                     tests of the changed crates and of the crates using them; if a .bend file of core/ hub/ vendor/
#                     or LAWS/PROOF changed: PROOF.bend (4 shards in parallel).
#                     The tests run bins.sh's cached sb-core of the tree's
#                     hub/ vendor/ itself (SB_CORE_BIN = the cache file, one
#                     compile per source hash for every worktree and the
#                     full gate; never a copy of its own: the EDR deletes
#                     fresh unsigned copies). The bend part runs next to cargo.
#   gate.sh full      everything: cargo build, run_all.sh (it puts ./repl-live
#                     ./repl-scripted ./sb-core in place with bins.sh: a copy
#                     from the cache, a compile when their sources changed;
#                     never commit them) with FUZZ_RUNS=2000
#                     (~60 s warm; e2e + tmux tests in parallel, SB_TEST_JOBS),
#                     then no binary built needs a macOS newer than the
#                     target (./bins.sh minos, BISE-164).
#   gate.sh new [name]
#                     start a task (name: default $SB_AGENT): its folder
#                     ~/.bise/worktrees/<project-id>/<name>/ holds a worktree
#                     of HEAD (<repo>/, e.g. harness/) and its own target
#                     (target/), an APFS clone (0 bytes, ~3 s) of the warm
#                     seed of the current deps (~/.bise/cache/gate-seed/,
#                     one at a time); prints the cd/export to run. No seed
#                     for these deps (Cargo.lock, a Cargo.toml,
#                     rust/.cargo/config.toml or rustc changed): builds one
#                     on top of the newest seed (the changed deps only;
#                     ~2 min cold) and keeps it for the next tasks.
#   gate.sh done [name]
#                     end a task: remove its folder, worktree and target
#                     (refused when the worktree has uncommitted changes or
#                     commits on no branch). The hub does it too at the
#                     task's /drop and, for an orphan, at its start (never
#                     with such work: it says so in main's thread).
#                     Inside an agent, new/done/quick/full tell the hub where
#                     it works (sb worktree <path>|none: the TUI's ψ, BISE-136).
#   gate.sh wait <bg .out file | pid>
#                     the bash tool put a gate in the background: block until
#                     it ends (at most 25 s), then show its end and exit code.
# "Changed" = the tree vs GATE_BASE (default HEAD), untracked files included.
# CARGO_TARGET_DIR is honoured (the e2e/tmux tests run its bise);
# the bend results are cached in $CARGO_TARGET_DIR/gate-cache.
# Sharing the machine (BISE-244, loop-speed.md §0): quick, full and new's
# seed build run under nice 10 (GATE_NICE) with CARGO_BUILD_JOBS = half the
# cores (unless set); one full gate at a time on the machine (a lock,
# <gate dir>/full-<uid>.lock; the others print and set "waiting for the
# gate"); one seed build per deps key (the other tasks wait, then clone it).
# The gate dir: ~/.bise/gate ($BISE_HOME/gate), writable in auto's sandbox
# (approvals-design.md §7) and short (the test hubs' sockets live under it:
# an agent's $TMPDIR is too deep for a unix socket path); /tmp/bise-gate-
# where there is no ~/.bise yet.
# Each quick/full run has its own TMPDIR <gate dir>/<pid>/: at its end,
# ctrl-c or kill included, it kills what it started (the test hubs, their
# sb-core and REPLs, the tmux tests' shells), and each run first kills what
# a dead run left.
set -uo pipefail
mode="${1:-quick}"
# BISE-136: inside an agent, tell the hub where it works (the TUI's ψ):
# new/quick/full in a private worktree say its path, done says none.
# Best effort and silent (outside an agent, or an older hub).
sb_place() { [ -n "${SB_AGENT:-}" ] && command -v sb >/dev/null 2>&1 && sb worktree "$1" >/dev/null 2>&1; return 0; }
sb_note() { [ -n "${SB_AGENT:-}" ] && command -v sb >/dev/null 2>&1 && sb status working --note "$1" >/dev/null 2>&1; return 0; }
# BISE-244: the hub, the TUIs and the other tasks stay responsive: this
# script and all it starts at nice 10, cargo on half the cores
share_machine() {
  renice "${GATE_NICE:-10}" -p $$ >/dev/null 2>&1
  local n; n="$(sysctl -n hw.ncpu 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"
  export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(( (n + 1) / 2 ))}"
}
proc_start() { ps -o lstart= -p "$1" 2>/dev/null | tr -s ' ' _; }
older_than_a_minute() { [ -n "$(find "$1" -maxdepth 0 -mmin +1 2>/dev/null)" ]; }
if [ "$mode" = wait ]; then
  arg="${2:?usage: gate.sh wait <bg .out file | pid>}"
  if [ -f "$arg" ]; then pid="$(cat "${arg%.out}.pid" 2>/dev/null)"; rcf="${arg%.out}.rc"; else pid="$arg"; rcf=; fi
  for _ in $(seq 25); do { [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; } || break; sleep 1; done
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then echo "still running (pid $pid): gate.sh wait $arg"; exit 0; fi
  [ -f "$arg" ] && tail -n 25 "$arg"
  [ -n "$rcf" ] && [ -f "$rcf" ] && echo "exit code: $(cat "$rcf")"
  exit 0
fi
if [ "$mode" = new ] || [ "$mode" = done ]; then
  # BISE-230: a task's folder <home>/worktrees/<project-id>/<name>/, like
  # the hub's own task worktrees: the worktree (<repo>/), its cargo target
  # (target/) and the task it belongs to (owner). The hub removes the
  # folder at the task's /drop and at its start (an orphan), never when
  # the worktree has uncommitted changes or commits on no branch.
  name="${2:-${SB_AGENT:-}}"
  [ -n "$name" ] || { echo "usage: gate.sh $mode <name>" >&2; exit 2; }
  # the repo of the cwd (gate.sh may run from a copy: bash <(git show HEAD:...)),
  # else this script's
  cd "$(git rev-parse --show-toplevel 2>/dev/null || echo "$(dirname "$0")/..")" || exit 1
  # the main worktree (the shared tree), even from a task's detached one
  main="$(git worktree list --porcelain | sed -n '1s/^worktree //p')"
  if [ -n "${BISE_HOME:-}" ]; then home="$BISE_HOME"
  elif [ -f "$HOME/.bise/migrated.json" ]; then home="$HOME/.bise"
  else home=""; fi
  wtroot="${home:-$HOME/.local/state/switchboard}/worktrees"
  seeds="${home:-$HOME/.bend-harness}/cache/gate-seed"
  # the project's id, as the hub computes it (switchboard::paths::workspace_id,
  # pinned by its test ids_match_the_python_copies)
  pid="$(python3 - "$main" <<'PY'
import os, sys
p = os.path.realpath(sys.argv[1]); h = 0xcbf29ce484222325
for b in p.encode(): h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
base = os.path.basename(p) or "root"
base = "".join(c if c.isascii() and (c.isalnum() or c in "-_") else "-" for c in base)[:32]
print("%s-%08x" % (base, h & 0xFFFFFFFF))
PY
)"
  dir="$wtroot/$pid/$name" repo="$(basename "$main")"
  [ "$repo" = target ] || [ "$repo" = owner ] && repo=repo
  wt="$dir/$repo" tgt="$dir/target"
  if [ "$mode" = done ]; then
    # the layout before BISE-230: /tmp/<name>-wt and /tmp/<name>-target
    [ -d "$dir" ] || { wt="/tmp/$name-wt"; tgt="/tmp/$name-target"; dir=; }
    if [ -e "$wt/.git" ]; then
      if [ -n "$(git -C "$wt" status --porcelain 2>/dev/null)" ]; then
        echo "$wt has uncommitted changes: commit them, or git -C $wt stash / checkout, then again"; exit 1
      fi
      mine="$(git -C "$wt" rev-list HEAD --not --branches --remotes 2>/dev/null | wc -l | tr -d ' ')"
      [ "$mine" = 0 ] || { echo "$wt has $mine commit(s) on no branch: put them on main (or a branch), then again"; exit 1; }
      git -C "$main" worktree remove --force "$wt"
    fi
    rm -rf "$tgt" ${dir:+"$dir"}; sb_place none; echo "removed ${dir:-$wt and $tgt}"; exit 0
  fi
  [ -e "$dir" ] && { echo "$dir exists: another name, or gate.sh done $name"; exit 1; }
  mkdir -p "$dir" && echo "${SB_AGENT:-$name}" >"$dir/owner" || exit 1
  git worktree add -q --detach "$wt" "$(git -C "$main" rev-parse HEAD)" || { rm -rf "$dir"; exit 1; }
  cd "$wt" || exit 1
  sb_place "$wt"
  export PATH="$HOME/.cargo/bin:$PATH"
  # the seed: a warm target (tests + clippy of every crate) for exactly
  # these deps, one at a time (<home>/cache/gate-seed/<key>, ~7 GB of
  # blocks every clone shares); the workspace crates recompile in the task
  # anyway (a new checkout's mtimes), incrementally
  key="$( { cat rust/Cargo.lock rust/.cargo/config.toml $(git ls-files 'rust/Cargo.toml' 'rust/*/Cargo.toml'); rustc -vV; } | shasum | cut -c1-12)"
  seed="$seeds/$key"
  mkdir -p "$seeds"
  # the seed of the old layout (/tmp/sb-seed-<key>): a clone, without the
  # older seed an earlier `mv` nested in it
  if [ ! -d "$seed" ] && [ -d "/tmp/sb-seed-$key" ] && mkdir "$seeds.lock" 2>/dev/null; then
    rm -rf "$seed.tmp" && cp -cR "/tmp/sb-seed-$key" "$seed.tmp" && rm -rf "$seed.tmp"/sb-seed-* && mv "$seed.tmp" "$seed"
    rmdir "$seeds.lock"
  fi
  # BISE-244: one seed build per key: the first task builds it
  # ($seed.building holds its pid), the others wait for it and clone it
  share_machine
  said=
  until [ -d "$seed" ] || mkdir "$seed.building" 2>/dev/null; do
    bp="$(cat "$seed.building/pid" 2>/dev/null)"
    if { [ -n "$bp" ] && ! kill -0 "$bp" 2>/dev/null; } || { [ -z "$bp" ] && older_than_a_minute "$seed.building"; }; then
      rm -rf "$seed.building"; continue
    fi
    [ -n "$said" ] || { echo "another task builds the seed for these deps (pid ${bp:-?}): waiting for it, then a clone"; sb_note "waiting for the seed build"; said=1; }
    sleep 3
  done
  [ -d "$seed.building" ] && [ ! -s "$seed.building/pid" ] && echo $$ >"$seed.building/pid"
  building() { [ "$(cat "$seed.building/pid" 2>/dev/null)" = $$ ]; }
  s=$SECONDS
  if [ -d "$seed" ]; then
    building && rm -rf "$seed.building"
    cp -cR "$seed" "$tgt" || exit 1
    echo "target: clone of the seed $seed ($((SECONDS - s))s)"
  else
    newest="$(ls -dt "$seeds"/* 2>/dev/null | grep -v '\.tmp$' | head -1)"
    [ -n "$newest" ] && cp -cR "$newest" "$tgt"
    echo "no seed for these deps: building one${newest:+ on top of $newest} (~2 min cold, once)"
    (cd rust && CARGO_TARGET_DIR="$tgt" cargo clippy --offline -q --workspace --all-targets --target-dir "$tgt/clippy") >/dev/null 2>&1 &
    (cd rust && CARGO_TARGET_DIR="$tgt" cargo test --offline -q --workspace --no-run) >/dev/null 2>&1; rc=$?
    wait $! || rc=1
    if [ $rc = 0 ] && mkdir "$seeds.lock" 2>/dev/null; then
      # one seed: the new one replaces the older ones (0 bytes: a clone);
      # never `mv` onto an existing seed (it would nest the new one in it)
      if [ ! -e "$seed" ]; then
        rm -rf "$seed.tmp" && cp -cR "$tgt" "$seed.tmp" && mv "$seed.tmp" "$seed"
      fi
      for o in "$seeds"/*; do [ "$o" = "$seed" ] || rm -rf "$o"; done
      rmdir "$seeds.lock"
    fi
    building && rm -rf "$seed.building"
    echo "target: built ($((SECONDS - s))s)$([ $rc = 0 ] || echo ', with errors: the gate shows them')"
  fi
  echo "now: cd $wt && export CARGO_TARGET_DIR=$tgt   (end: gate.sh done $name)"
  exit 0
fi
case "$mode" in quick|full) ;; *) echo "usage: gate.sh [quick|full|new [name]|done [name]|wait <file|pid>]" >&2; exit 2 ;; esac
cd "$(dirname "$0")/.."
root="$PWD"
# a linked worktree has a .git file (the shared checkout a directory)
[ -f "$root/.git" ] && sb_place "$root"
share_machine
# ---- BISE-244: what a run starts dies with it. Its TMPDIR, <gate dir>/<pid>
# (short: the hubs' sockets live under it), holds the tests' workspaces, so
# a test hub names it in its command line even orphaned; its sb-core and
# REPLs share its process group. The tmux server itself is never killed
# (the user's sessions may live in it): the test's shell in it is.
gate_procs() {  # <marker> <pid whose descendants go too | ""> <worktree binary prefixes... >: pids to kill
  local m="$1" me="$2"; shift 2
  # the marker goes through the environment: in awk's argv it would match awk
  ps -axww -o pid=,ppid=,pgid=,command= | GATE_M="$m" GATE_BINS="$*" awk -v me="$me" -v self=$$ '
    BEGIN { m = ENVIRON["GATE_M"]; bins = ENVIRON["GATE_BINS"] }
    { p = $1 + 0; pp[p] = $2 + 0; pg[p] = $3 + 0; c = $0; sub(/^ *[0-9]+ +[0-9]+ +[0-9]+ /, "", c); cmd[p] = c; ids[++n] = p }
    function tmux(c) { return c ~ /^([^ ]*\/)?tmux( |$)/ }
    function orphan_bin(p,   i, k, b) {  # an orphan (ppid 1) of the worktree s binaries
      if (pp[p] != 1 || bins == "") return 0
      k = split(bins, b, " "); for (i = 1; i <= k; i++) if (cmd[p] == b[i] || index(cmd[p], b[i] " ") == 1) return 1
      return 0
    }
    END {
      for (p = self; p > 1 && !(p in anc); p = pp[p]) anc[p] = 1
      # this snapshot: the ps, its subshell and the subshell s children (awk)
      for (i = 1; i <= n; i++) if (cmd[ids[i]] == "ps -axww -o pid=,ppid=,pgid=,command=") sub_sh = pp[ids[i]]
      for (i = 1; i <= n; i++) { p = ids[i]; if (p == sub_sh || pp[p] == sub_sh) anc[p] = 1 }
      for (i = 1; i <= n; i++) { p = ids[i]
        if (tmux(cmd[p])) continue
        if (index(cmd[p], m) || (me != "" && pp[p] == me) || orphan_bin(p)) { k[p] = 1; if (pg[p] != pg[self]) g[pg[p]] = 1 }
      }
      for (i = 1; i <= n; i++) { p = ids[i]; if (pg[p] in g) k[p] = 1 }
      do { ch = 0; for (i = 1; i <= n; i++) { p = ids[i]; if (!(p in k) && (pp[p] in k)) { k[p] = 1; ch = 1 } } } while (ch)
      for (p in k) if (!(p in anc) && !tmux(cmd[p])) print p
    }'
}
gate_kill() {  # <pids>: TERM, then KILL what is left half a second later
  [ -n "$1" ] || return 0
  kill -TERM $1 2>/dev/null; sleep 0.5
  for p in $1; do kill -0 "$p" 2>/dev/null && kill -KILL "$p" 2>/dev/null; done
  return 0
}
# in a task's worktree, its own binaries left alone by a dead parent
wt_bins=""
if [ -f "$root/.git" ]; then
  t="$(cd "${CARGO_TARGET_DIR:-rust/target}" 2>/dev/null && pwd -P)"
  wt_bins="$root/repl-live $root/repl-scripted $root/sb-core ${t:+$t/debug/bise}"
fi
# the leftovers of dead runs (a kill -9, a crash): their processes now; their
# folder (the logs of a red run) after a day
# the gate dir (above): runs <gpre><pid>, the lock, a red step's log
if [ -n "${BISE_HOME:-}" ] || [ -f "$HOME/.bise/migrated.json" ]; then
  gdir="${BISE_HOME:-$HOME/.bise}/gate" gpre="${BISE_HOME:-$HOME/.bise}/gate/"
else
  gdir=/tmp gpre=/tmp/bise-gate-
fi
mkdir -p "$gdir"
# what a run's processes name in their command line: its folder (a /tmp
# one without /tmp: python may show it as /private/tmp)
gate_mark() { case "$1" in /tmp/bise-gate-*) echo "/bise-gate-${1##*-}/" ;; *) echo "$1/" ;; esac; }
# a red or dead run's folder: only run_all's logs stay (sb-run-all.*)
gate_prune() {
  find "$1" -mindepth 1 -maxdepth 1 ! -name 'sb-run-all.*' ! -name .owner -exec rm -rf {} + 2>/dev/null
  for l in "$1"/sb-run-all.*; do
    [ -d "$l" ] && find "$l" -mindepth 1 -maxdepth 1 ! -name '*.log' -exec rm -rf {} + 2>/dev/null
  done
  return 0
}
for d in "$gpre"[0-9]* /tmp/bise-gate-[0-9]*; do
  [ -d "$d" ] || continue
  p="${d##*[-/]}"
  kill -0 "$p" 2>/dev/null && [ "$(proc_start "$p")" = "$(cat "$d/.owner" 2>/dev/null)" ] && continue
  gate_kill "$(gate_procs "$(gate_mark "$d")" "")"
  if [ -n "$(find "$d" -maxdepth 0 -mtime +0 2>/dev/null)" ]; then rm -rf "$d" 2>/dev/null; else gate_prune "$d"; fi
done
[ -n "$wt_bins" ] && gate_kill "$(gate_procs "/bise-gate-none/" "" $wt_bins)"
run="$gpre$$"
rm -rf "$run"; mkdir -p "$run" && proc_start $$ >"$run/.owner" || { echo "gate: cannot write $run"; exit 1; }
export TMPDIR="$run" BISE_GATE_RUN=$$
LOCK="${gpre}full-$(id -u).lock"
locked="" out=""
on_exit() {
  local rc=$?
  trap - EXIT INT TERM HUP
  kill $(jobs -p) 2>/dev/null
  gate_kill "$(gate_procs "$(gate_mark "$run")" $$ $wt_bins)"
  [ -n "$out" ] && rm -rf "$out"
  [ -n "$locked" ] && [ "$(cut -d' ' -f1 "$LOCK/owner" 2>/dev/null)" = $$ ] && rm -rf "$LOCK"
  # a red run keeps run_all's logs for a day, nothing else (the tests'
  # workspaces: ~100 MB a run)
  if [ $rc = 0 ]; then rm -rf "$run"; else gate_prune "$run"; fi
  exit $rc
}
trap on_exit EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
# one full gate at a time on the machine: a mkdir lock, its owner file
# "<pid> <start> <agent> <root>"; a dead owner's lock is taken over (one
# taker at a time: $LOCK.takeover)
lock_stale() {
  local o; o="$(cat "$LOCK/owner" 2>/dev/null)"
  [ -n "$o" ] || { older_than_a_minute "$LOCK"; return; }
  set -- $o
  ! kill -0 "$1" 2>/dev/null || [ "$(proc_start "$1")" != "$2" ]
}
gate_lock() {
  local t0=$SECONDS said="" opid ostart oagent oroot
  until mkdir "$LOCK" 2>/dev/null; do
    if lock_stale && mkdir "$LOCK.takeover" 2>/dev/null; then
      lock_stale && rm -rf "$LOCK"
      rmdir "$LOCK.takeover"; continue
    fi
    older_than_a_minute "$LOCK.takeover" && rmdir "$LOCK.takeover" 2>/dev/null
    if [ -z "$said" ]; then
      read -r opid ostart oagent oroot <"$LOCK/owner" 2>/dev/null
      echo "waiting for the gate: ${oagent:-another task}'s full gate runs (pid ${opid:-?}, ${oroot:-?}); one at a time"
      sb_note "waiting for the gate (${oagent:-another task}'s full gate)"
      said=1
    fi
    sleep 2
  done
  echo "$$ $(proc_start $$) ${SB_AGENT:-${USER:-?}} $root" >"$LOCK/owner"
  locked=1
  [ -z "$said" ] || { echo "got the gate after $((SECONDS - t0))s"; sb_note "full gate running"; }
}
[ "$mode" = full ] && gate_lock
export PATH="$HOME/.bend/bin:$HOME/.cargo/bin:$PATH"
unset SB_CORE_BIN
# the oldest macOS the binaries run on (rust/.cargo/config.toml, BISE-164):
# for the quick sb-core's cc too
export MACOSX_DEPLOYMENT_TARGET; MACOSX_DEPLOYMENT_TARGET="$(scripts/bins.sh macos-target)"
base="${GATE_BASE:-HEAD}"
changed="$( { git diff --name-only "$base"; git ls-files --others --exclude-standard; } | sort -u)"
hub_changed=0 bend_changed=0
printf '%s\n' "$changed" | grep -qE '^bend/(hub|vendor)/' && hub_changed=1
printf '%s\n' "$changed" | grep -qE '^bend/((core|hub|vendor)/.*|LAWS|PROOF)\.bend$' && bend_changed=1
# ./sb-core (not in git, BISE-114): the build of this tree's hub/ vendor/,
# from bins.sh's cache shared by every worktree (a copy; ~15 s on a miss).
# quick with a hub/ change: bins.sh compiles it in the background (below)
if [ $hub_changed = 0 ] || [ "$mode" = full ]; then
  scripts/bins.sh sb-core || { echo "FAIL sb-core build"; exit 1; }
fi
# the EDR of a company Mac (CrowdStrike) deletes a fresh unsigned binary
# when it is spawned (~0.2 s): one clear line instead of 30 red tests
# (a rerun compiles it again in bins.sh's cache, where it survived so far)
sbcore_gone() {  # <file>: say so and true when the sb-core that was built is gone
  [ -n "$1" ] && [ ! -e "$1" ] || return 1
  echo "FAIL sb-core vanished after it was built: an antivirus (EDR) may have removed it ($1)"
}

if [ "$mode" = full ]; then
  s=$SECONDS
  export FUZZ_RUNS="${FUZZ_RUNS:-2000}"
  tests/run_all.sh & wait $!; rc=$?
  [ $rc = 0 ] || sbcore_gone "$root/sb-core"
  # every binary built runs on the macOS target (BISE-164): the Bend ones,
  # bise, and the engine when this tree has one
  bins=(./repl-live ./repl-scripted ./sb-core "${CARGO_TARGET_DIR:-rust/target}/debug/bise")
  for f in ./harness-demo rust/jsrt/target/debug/bend-jsrt; do [ -e "$f" ] && bins+=("$f"); done
  if [ $rc = 0 ]; then scripts/bins.sh minos "${bins[@]}" || rc=1; fi
  [ $rc = 0 ] && echo "GATE full GREEN ($((SECONDS - s))s)" || echo "GATE full FAILED"
  exit $rc
fi

# ---- quick
t0=$SECONDS
cache="${CARGO_TARGET_DIR:-$root/rust/target}/gate-cache"
# the PROOF shards sit in a copy of bend/ in it and import it relatively
# (bend refuses an absolute import path with a dot, like ~/.bise/gate/...)
out="$(mktemp -d "${TMPDIR:-/tmp}/sbgateXXXXXX")"
mkdir -p "$cache"
fail() {  # <name> <log>: the failures, the log kept
  # the tests' sb-core gone: that line alone, not every test it failed
  if sbcore_gone "${SB_CORE_BIN:-}"; then
    cp "$2" "$gdir/sb-gate-$1.log"; echo "log of $1: $gdir/sb-gate-$1.log"; exit 1
  fi
  echo "FAIL $1"
  grep -E "^test .* FAILED|panicked|^error|^warning|^failures:|^Location|^Error" "$2" | head -30
  cp "$2" "$gdir/sb-gate-$1.log"; echo "log: $gdir/sb-gate-$1.log"; exit 1
}
hash_of() {  # <dirs/files...>: one hash of the .bend files' names and contents
  (cd "$root" && find "$@" -name '*.bend' -type f 2>/dev/null | sort | while read -r f; do echo "$f"; cat "$f"; done) | shasum | cut -c1-16
}
# the bend part, in the background (it takes longer than cargo)
proof_job() {
  local h; h="$(hash_of bend/core bend/hub bend/vendor bend/LAWS.bend bend/PROOF.bend)"
  [ -f "$cache/proof-ok-$h" ] && { echo "ok   PROOF (cached)"; return 0; }
  local d="$out/proof" n=4 s=$SECONDS i pids=()
  mkdir -p "$d"
  python3 tests/proof_shards.py "$root/bend" $n "$d" || { echo "FAIL PROOF (split)"; return 1; }
  for i in $(seq 0 $((n - 1))); do bend "$d/P$i.bend" --check-only >"$d/P$i.log" 2>&1 & pids+=($!); done
  wait "${pids[@]}"
  for i in $(seq 0 $((n - 1))); do
    grep -q "ALL PROOFS CHECK" "$d/P$i.log" && continue
    grep -qx "Error: $(cat "$d/P$i.expect") TODOs found." "$d/P$i.log" && continue
    echo "FAIL PROOF ($((SECONDS - s))s): shard $i of $n"; grep -v '^- ' "$d/P$i.log" | head -20
    echo "(rerun whole: bend bend/PROOF.bend)"; return 1
  done
  touch "$cache/proof-ok-$h"; echo "ok   PROOF ($((SECONDS - s))s, $n shards)"
}
# the sb-core of the tests: bins.sh's cache file of this tree's hub/ vendor/
# (keyed by their content), run in place, never copied. It used to be a
# -O1 build of its own in $cache (7 s instead of 10-16 s), so a hub change
# compiled sb-core twice (quick, then full), and that fresh copy is what
# the EDR deleted at its first spawn; the -O3 compile runs next to cargo's.
sbcore_job() {
  local s=$SECONDS f
  f="$(scripts/bins.sh path sb-core 2>"$out/sb-core.log")" && [ -x "$f" ] \
    || { echo "FAIL sb-core build"; tail -20 "$out/sb-core.log"; return 1; }
  sbcore_gone "$f" && return 1
  echo "$f" >"$out/sbcore.path"
  if grep -q compiling "$out/sb-core.log"; then echo "ok   sb-core ($((SECONDS - s))s, compiled into bins.sh's cache)"
  else echo "ok   sb-core (bins.sh's cache)"; fi
}
[ $bend_changed = 1 ] && { proof_job >"$out/proof.res" 2>&1; echo $? >"$out/proof.rc"; } &
rm -f "$cache"/sb-core-*  # the -O1 copies of the gate before
if [ $hub_changed = 1 ]; then
  # SB_CORE_BIN is set from $out/sbcore.path once it is built (below)
  { sbcore_job >"$out/sbcore.res" 2>&1; echo $? >"$out/sbcore.rc"; } &
  sbcore_pid=$!
else
  # a hit: ./sb-core was just put in place from it
  SB_CORE_BIN="$(scripts/bins.sh path sb-core 2>/dev/null)" && export SB_CORE_BIN || unset SB_CORE_BIN
fi
# the crates to test: changed ones and the crates depending on them
pkgs=""
add() { case " $pkgs " in *" $1 "*) ;; *) pkgs="$pkgs $1" ;; esac; }
for f in $changed; do
  case "$f" in
    rust/Cargo.toml|rust/Cargo.lock|rust/.cargo/*) add bise-session; add bise-home; add bise-catalog; add bend-plugins; add bend-images; add bend-tui; add switchboard; add bend-harness ;;
    rust/home/*) add bise-home; add bend-plugins; add bend-images; add bend-tui; add switchboard; add bise-catalog; add bend-harness ;;
    rust/catalog/*) add bise-catalog; add bend-harness ;;
    rust/computer-use/*) add bise-computer-use; add bend-harness ;;
    rust/plugins/*) add bend-plugins; add bend-tui; add bend-harness ;;
    rust/images/*) add bend-images; add bend-tui; add bend-harness ;;
    rust/tui/*) add bend-tui; add bend-harness ;;
    rust/vendor/crossterm/*) add bend-tui; add bend-harness; crossterm_changed=1 ;;
    rust/switchboard/*) add switchboard; add bend-harness ;;
    rust/harness/*) add bend-harness ;;
    rust/session/*) add bise-session; add switchboard; add bend-harness ;;
    bend/hub/*|bend/vendor/*) add switchboard ;;
  esac
done
step() {  # <name> <cmd...>: one line when green, the failures and the log when red
  local n=$1 s=$SECONDS; shift
  # in the background: a kill or ctrl-c reaches the trap at once
  "$@" >"$out/$n.log" 2>&1 & wait $!
  if [ $? = 0 ]; then echo "ok   $n ($((SECONDS - s))s)"; else fail "$n" "$out/$n.log"; fi
}
# no `cargo build`: clippy checks every target, the tests compile and link
# the changed crates; the binary itself is built by gate.sh full.
# clippy runs next to the tests in its own target dir ($target/clippy: its
# own cargo lock; ~300 MB, 30-40 s the first time)
target="${CARGO_TARGET_DIR:-$root/rust/target}"
{ s=$SECONDS
  if (cd rust && cargo clippy --offline -q --workspace --all-targets --target-dir "$target/clippy" -- -D warnings) >"$out/clippy.log" 2>&1
  then echo "ok   clippy ($((SECONDS - s))s)" >"$out/clippy.res"; echo 0 >"$out/clippy.rc"
  else echo 1 >"$out/clippy.rc"; fi; } &
clippy_pid=$!
if [ -n "$pkgs" ]; then
  args=""; for p in $pkgs; do args="$args -p $p"; done
  if [ -n "${sbcore_pid:-}" ]; then
    step test-build bash -c "cd rust && cargo test --offline -q --no-run $args"
    wait "$sbcore_pid"; cat "$out/sbcore.res"; [ "$(cat "$out/sbcore.rc")" = 0 ] || exit 1
    SB_CORE_BIN="$(cat "$out/sbcore.path")"; export SB_CORE_BIN
  fi
  step "test$(echo "$pkgs" | tr ' ' '_')" bash -c "cd rust && cargo test --offline -q $args"
  grep "test result" "$out"/test_*.log | grep -v " 0 passed" | sed 's/^.*test result/  test result/'
  # our crossterm patch (not a workspace member): its parser's tests
  [ "${crossterm_changed:-0}" = 1 ] && step test_crossterm bash -c "cd rust && cargo test --offline -q --manifest-path vendor/crossterm/Cargo.toml --lib event::sys::unix::parse"
else
  echo "no Rust or hub change vs $base: no Rust tests run"
fi
wait "$clippy_pid"
[ "$(cat "$out/clippy.rc")" = 0 ] || fail clippy "$out/clippy.log"
cat "$out/clippy.res"
if [ $bend_changed = 1 ]; then
  wait; cat "$out/proof.res"; [ "$(cat "$out/proof.rc")" = 0 ] || exit 1
fi
echo "GATE quick GREEN ($((SECONDS - t0))s):${pkgs:- no Rust tests}$([ $bend_changed = 1 ] && echo ' + PROOF') (full: gate.sh full)"
