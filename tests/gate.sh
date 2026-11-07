#!/usr/bin/env bash
# The agents' gate (docs/loop-speed.md), run in the
# foreground, never `sleep N; tail` in a loop:
#   gate.sh [quick]   the check before a land (docs/loop-speed.md §0 rule 11): fast
#                     steps only, never an e2e, tmux, Playwright, desktop
#                     harness or as-he-does step (those run on main: the tip
#                     gate, the nightly, a release's full --all).
#                     What the tree's changes touch (the plan: tests/gate_plan.py,
#                     paths in, steps out; this script runs it), ~5-10 s warm: clippy -D
#                     warnings (next to the tests, in $target/clippy), the
#                     tests of the changed crates and of the crates using them; if a .bend file of core/ hub/ vendor/
#                     or of laws/ proofs/ changed: PROOF, the bend/proofs/*.bend whose
#                     imports' closure holds it (gate_plan.py; full: every one), each
#                     cached by that closure's content (<gate dir>/proof-cache),
#                     in parallel (tests/proof_run.py, 4 jobs);
#                     no changed test hard-codes a UI word past its count in
#                     tests/word_literals.txt (tests/word_lint.py, issue 29).
#                     The gate's own tests (gate_sweep, gate_queue, desktop_pick,
#                     test_map, the ratchets' --self-test...) only when their
#                     sources changed (gate_plan.py SELFTESTS); full runs them all.
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
#   gate.sh full --all  every e2e/tmux test, whatever changed: a release
#                     (SB_AGENT release*: always), the nightly run. Without it
#                     the full gate runs the Rust part and the e2e/tmux tests
#                     tests/test_map.py picks for the tree's changes vs its
#                     merge-base with main (GATE_FULL_BASE); main's tip, shared
#                     code or an unknown path: all of them.
#                     A red test gets one rerun alone inside the gate
#                     (run_all.sh: FLAKY lines, <gate dir>/flaky.tsv).
#                     Green with every test on a clean tree (no diff, no
#                     untracked file): its sha goes to <gate dir>/green-shas.tsv
#                     (this file owns it: <sha> <end epoch> <secs> <agent>
#                     <log>). A full gate on a clean tree whose sha is there
#                     runs only word_lint (GATE_FORCE=1: all
#                     of it): a release on a sha that scripts/nightly_gate.sh
#                     --tip (main's tip, in a quiet slot) gated skips its own.
#   gate.sh rerun     after a full gate red only on e2e/tmux tests: those
#                     tests alone, one by one, on the same tree (HEAD + diff +
#                     untracked; <gate dir>/red-<state>), instead of a whole
#                     new full gate; green says which ones were flaky.
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
#   gate.sh seed-path print the seed to clone for the cwd repo's deps (this
#                     key's, else the newest; exit 1 when none): a desktop
#                     candidate's cargo target starts from it (try_build.py).
#   gate.sh wait <bg .out file | pid>
#                     the bash tool put a gate in the background: block until
#                     it ends (at most 25 s), then show its end and exit code.
#   gate.sh stop      stop the gate running in this checkout (and what it
#                     started), not another agent's: never pkill a gate by
#                     name, macOS pkill matches every agent's (repl-kill).
# "Changed" = the tree vs GATE_BASE (default HEAD), untracked files included.
# CARGO_TARGET_DIR is honoured (the e2e/tmux tests run its bise); unset,
# in a task worktree the hub made (<task dir>/<repo> with <task dir>/owner)
# with no rust/target yet, it is <task dir>/target, a clone of the seed.
# Raises the open-files limit to 10240 (a launchd job starts at 256).
# the bend results are cached in $CARGO_TARGET_DIR/gate-cache.
# Sharing the machine (BISE-244, loop-speed.md §0): quick, full and new's
# seed build run under nice 10 (GATE_NICE) with CARGO_BUILD_JOBS = half the
# cores (unless set); one full gate or rerun at a time on the machine (a lock,
# <gate dir>/full-<uid>.lock; the others print and set "waiting for the
# gate"); one seed build per deps key (the other tasks wait, then clone it).
# The gate dir: ~/.bise/gate ($BISE_HOME/gate), writable in auto's sandbox
# (approvals-design.md §7) and short (the test hubs' sockets live under it:
# an agent's $TMPDIR is too deep for a unix socket path).
# Each quick/full/rerun keeps its output (stdout and stderr, teed) in
# <gate dir>/logs/<mode>-<epoch>-<pid>.log for 7 days, first line "# gate
# <mode> agent=<SB_AGENT> sha=<HEAD> start=<epoch> root=<repo>", last "#
# gate end rc=<n> secs=<n>" (after its tees drained): the gate runs'
# record, scripts/loop_audit.py's one source of them (a task's $TMPDIR,
# where its own copy went, is deleted at its drop; a green run's folder
# at its end).
# Each quick/full run has its own TMPDIR <gate dir>/<pid>/: at its end,
# ctrl-c or kill included, it kills what it started (the test hubs, their
# sb-core and REPLs, the tmux tests' shells), and each run first kills what
# a dead run left.
set -uo pipefail
mode="${1:-quick}"
# run_all.sh tags each timings.tsv line with it: the audit counts a test's
# reds in gate runs only, not in an agent's own loops (gate-reds)
export SB_RUN_KIND="gate-$mode"
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
  # a launchd job (launchctl submit) starts with 256 open files: bend-tui's
  # tests then fail on UnixStream::pair (EMFILE) and the gate is red for
  # nothing (core-b, 2026-10-12)
  [ "$(ulimit -n)" != unlimited ] && [ "$(ulimit -n)" -lt 10240 ] && ulimit -n 10240 2>/dev/null
  return 0
}
# the bise home, and the warm seeds of gate.sh new (one per deps key)
bise_home() { echo "${BISE_HOME:-$HOME/.bise}"; }
seeds_dir() { echo "$(bise_home)/cache/gate-seed"; }
seed_key() {  # in the repo root: the deps' key (Cargo.lock, Cargo.tomls, cargo config, rustc)
  { cat rust/Cargo.lock rust/.cargo/config.toml $(git ls-files 'rust/Cargo.toml' 'rust/*/Cargo.toml'); rustc -vV; } | shasum | cut -c1-12
}
seed_from() {  # in the repo root: the seed to clone, this deps key's, else the newest; nothing when none
  local sd from; sd="$(seeds_dir)"; from="$sd/$(seed_key)"
  [ -d "$from" ] || from="$(ls -dt "$sd"/* 2>/dev/null | grep -vE '\.(tmp|building|lock)$' | head -1)"
  echo "$from"
}
own_crates() {  # in the repo root: the package names of the repo's own crates (every rust/**/Cargo.toml [package])
  git ls-files 'rust/Cargo.toml' 'rust/**/Cargo.toml' 2>/dev/null | while read -r f; do
    awk '/^\[/ { p = ($0 == "[package]") } p && /^name *=/ { gsub(/[" ]/, ""); sub(/^name=/, ""); print; exit }' "$f"
  done | sort -u
}
seed_strip() {  # in the repo root: a clone of a seed keeps the deps only
  # A seed built in tree A holds A's build of the repo's own crates, with A's
  # CARGO_MANIFEST_DIR baked in (env!): cargo reuses it in a tree whose
  # sources are older than the seed (the nightly tree: git rewrites only the
  # changed files), and once A is gone a test that reads a fixture through it
  # fails (bise-peer's agent_key, the desktop-v2026.10.21 tip gate, m_48661).
  # Without their fingerprints cargo rebuilds them here; the deps stay warm.
  # .fingerprint/<package>-<hash>, incremental/<crate_name>-<hash>: the
  # package name, '-' as '_'; a crate whose [lib] name differs (none today:
  # crossterm's is its own) needs that name read too (architect m_48675)
  local t="$1" re fp d
  re="$(own_crates | paste -sd'|' -)"
  [ -n "$re" ] && [ -d "$t" ] || return 0
  re="^($re|$(tr - _ <<<"$re"))-[0-9a-f]{16}$"
  find "$t" -maxdepth 4 -type d \( -name .fingerprint -o -name incremental \) 2>/dev/null | while read -r fp; do
    for d in "$fp"/*; do [[ "${d##*/}" =~ $re ]] && rm -rf "$d"; done
  done
  return 0
}
proc_start() { ps -o lstart= -p "$1" 2>/dev/null | tr -s ' ' _; }
# the gate dir: runs <gpre><pid> (each with .owner, its start, and .root, its
# checkout), the lock, a red step's log
gate_dirs() {
  gdir="${BISE_HOME:-$HOME/.bise}/gate" gpre="${BISE_HOME:-$HOME/.bise}/gate/"
}
# gate.sh stop: the gates running in checkout $1 (a run whose .root is it and
# whose pid still runs with the start in .owner) get a TERM; each one's exit
# kills what its run started. Never pkill a gate by name: every agent's gate
# matches (macOS pkill reads its options only before the pattern, so a -P or
# -u after "gate.sh quick" narrows nothing: repl-kill, 2026-10-24).
gate_stop() {
  local d p n=0
  gate_dirs
  for d in "$gpre"[0-9]*; do
    [ -d "$d" ] && [ "$(cat "$d/.root" 2>/dev/null)" = "$1" ] || continue
    p="${d##*[-/]}"
    kill -0 "$p" 2>/dev/null && [ "$(proc_start "$p")" = "$(cat "$d/.owner" 2>/dev/null)" ] || continue
    kill -TERM "$p" 2>/dev/null || continue
    n=$((n + 1)); echo "stopping the gate $p in $1"
    for _ in $(seq 50); do kill -0 "$p" 2>/dev/null || break; sleep 0.2; done
    kill -0 "$p" 2>/dev/null && echo "the gate $p still runs after 10 s"
  done
  [ $n -gt 0 ] || echo "no gate runs in $1"
  return 0
}
older_than_a_minute() { [ -n "$(find "$1" -maxdepth 0 -mmin +1 2>/dev/null)" ]; }
# ---- the plan: which steps run for the changed paths (quick, land, full's desktop part) is
# tests/gate_plan.py's, pure, with its test on fixtures (tests/gate_plan_test.py); this script
# only runs it. Sets the plan's shell variables: pkgs (the crates to test and clippy), the step
# flags (proto_ts office office_skills crossterm_changed kit mcpapps gallery cu aqa words
# bendw), hub_changed, bend_changed, core_gen, dwhy and drel (desktop_check, its related
# files), heavy (the heavy steps, '' for a light run), land_steps. A plan that fails: red.
gate_plan() {  # <changed path>...
  local sh
  sh="$(python3 -B "$root/tests/gate_plan.py" --sh -- "$@")" || { echo "FAIL gate_plan: tests/gate_plan.py failed"; return 1; }
  eval "$sh"
}
# the gate's own tests of a plan's array (selftests: quick's, by path; selftests_all: full's),
# from the repo root, one at a time: quiet when green, the test's output again when red
selftests_run() {  # <array name> <red line>
  local arr=() e n c s t=$SECONDS ran="" tab=$'\t'
  eval "arr=(\${$1[@]+\"\${$1[@]}\"})"
  # ${a[@]+...}: macOS /bin/bash 3.2 calls an empty array unbound under set -u
  for e in ${arr[@]+"${arr[@]}"}; do
    n="${e%%"$tab"*}" c="${e#*"$tab"}" s=$SECONDS
    (cd "$root" && eval "$c") >/dev/null 2>&1 || { (cd "$root" && eval "$c"); echo "FAIL $n"; echo "$2"; return 1; }
    ran="$ran $n($((SECONDS - s))s)"
  done
  echo "ok   gate self-tests ($((SECONDS - t))s):${ran:- none for these paths}"
}
# node as the desktop lanes find it (desktop_path): the hub's land queue has no shell setup
# Node runs the repo's .ts scripts by type stripping: flagged before node 22.18 (js-to-ts,
# architect m_40557: the runners set it, never a shebang). Goes when engines says >=22.18.
TS_NODE_OPTIONS="--experimental-strip-types --disable-warning=ExperimentalWarning"
# the checks, then their types (office/sdk owns typescript: skipped without its node_modules)
kit_check() {
  local f tsc="$root/office/sdk/node_modules/.bin/tsc"; desktop_path
  for f in "$root"/kit/check.ts "$root"/kit/*.check.ts; do echo "node ${f#"$root"/}"; NODE_OPTIONS="$TS_NODE_OPTIONS" node "$f" || return 1; done
  if [ -x "$tsc" ]; then
    echo "tsc -p kit/tsconfig.node.json"; "$tsc" -p "$root/kit/tsconfig.node.json" || return 1
    # the page scripts: their types, and the committed .js is what their .ts emits
    echo "tsc -p kit/tsconfig.json"; "$tsc" -p "$root/kit/tsconfig.json" || return 1
    # the artifact viewer's modules (kit/office): theirs, on office/dist's types
    echo "tsc -p kit/office/tsconfig.json"; "$tsc" -p "$root/kit/office/tsconfig.json" || return 1
    # bise.dev's own scripts (Vercel serves the committed .js)
    echo "tsc -p site/tsconfig.json"; "$tsc" -p "$root/site/tsconfig.json" || return 1
    echo "ts_emit kit/tsconfig.json kit/office/tsconfig.json site/tsconfig.json --check"
    NODE_OPTIONS="$TS_NODE_OPTIONS" node "$root/scripts/ts_emit.ts" "$root/kit/tsconfig.json" "$root/kit/office/tsconfig.json" "$root/site/tsconfig.json" --check || return 1
  else echo "skipped kit types: no office/sdk/node_modules (pnpm -C office/sdk install)"; fi
}
# the extension's unit tests (no browser), then the types of its tests and the bench
cu_check() {
  local tsc="$root/office/sdk/node_modules/.bin/tsc"; desktop_path
  echo "node --test computer-use/extension/test/unit.test.ts"
  NODE_OPTIONS="$TS_NODE_OPTIONS" node --test "$root/computer-use/extension/test/unit.test.ts" || return 1
  if [ -x "$tsc" ]; then echo "tsc -p computer-use/tsconfig.node.json"; "$tsc" -p "$root/computer-use/tsconfig.node.json" || return 1
    # the extension's own code: its types, and the committed .js is what its .ts emits
    echo "tsc -p computer-use/extension/tsconfig.json"; "$tsc" -p "$root/computer-use/extension/tsconfig.json" || return 1
    echo "ts_emit computer-use/extension/tsconfig.json --check"
    NODE_OPTIONS="$TS_NODE_OPTIONS" node "$root/scripts/ts_emit.ts" "$root/computer-use/extension/tsconfig.json" --check || return 1
  else echo "skipped computer-use types: no office/sdk/node_modules (pnpm -C office/sdk install)"; fi
}
# the as-he-does helpers' unit test, then the types of the driver and pagedrive (Playwright's
# types are apps/desktop's: skipped without its node_modules, like desktop_check)
aqa_check() {
  local tsc="$root/apps/desktop/node_modules/.bin/tsc"; desktop_path
  echo "node --test dev/plugins/ambient-qa/as-he-does/fx.test.ts"
  NODE_OPTIONS="$TS_NODE_OPTIONS" node --test "$root/dev/plugins/ambient-qa/as-he-does/fx.test.ts" || return 1
  if [ -x "$tsc" ]; then echo "tsc -p dev/plugins/ambient-qa/tsconfig.json"; "$tsc" -p "$root/dev/plugins/ambient-qa/tsconfig.json" || return 1
  else echo "skipped ambient-qa types: no apps/desktop/node_modules (pnpm -C apps/desktop install)"; fi
}
if [ "$mode" = wait ]; then
  arg="${2:?usage: gate.sh wait <bg .out file | pid>}"
  if [ -f "$arg" ]; then pid="$(cat "${arg%.out}.pid" 2>/dev/null)"; rcf="${arg%.out}.rc"; else pid="$arg"; rcf=; fi
  for _ in $(seq 25); do { [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; } || break; sleep 1; done
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then echo "still running (pid $pid): gate.sh wait $arg"; exit 0; fi
  [ -f "$arg" ] && tail -n 25 "$arg"
  [ -n "$rcf" ] && [ -f "$rcf" ] && echo "exit code: $(cat "$rcf")"
  exit 0
fi
# the desktop checks a change can break (tests/test_map.py desktop-pick, the nightly's coverage
# map; fast-try p2). <from>: default the try's (~/.bise/dev/build/desktop-try/try.stamp's head);
# <to>: default the working tree. Shadow mode until review-gates' go: it says and logs, everything
# still runs.
if [ "$mode" = desktop-pick ]; then
  cd "$(git rev-parse --show-toplevel 2>/dev/null || echo "$(dirname "$0")/..")" || exit 1
  from="${2:-}"
  if [ -z "$from" ]; then
    stamp="$(bise_home)"; stamp="${stamp:-$HOME/.bise}/dev/build/desktop-try/try.stamp"
    from="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["head"])' "$stamp" 2>/dev/null)" \
      || { echo "desktop-pick: no try stamp at $stamp: gate.sh desktop-pick <from> [<to>]"; exit 2; }
  fi
  exec python3 tests/test_map.py desktop-pick --from "$from" ${3:+--to "$3"} ${GATE_PICK_LOG:+--log "$GATE_PICK_LOG"}
fi
if [ "$mode" = seed-path ]; then
  # the seed a desktop candidate's cargo target starts from (scripts/desktop_try.py), for the
  # cwd's repo: one place for the seeds' dir and key
  cd "$(git rev-parse --show-toplevel)" || exit 1
  from="$(seed_from)"; [ -n "$from" ] || exit 1
  echo "$from"; exit 0
fi
if [ "$mode" = seed ]; then
  # bise.toml [worktree] setup: a new task worktree's rust/target starts as an
  # APFS clone of the warm seed (~3 s, 0 bytes) before any cargo runs there.
  # side-resize's first bare 'cargo test' made a cold 6 GB rust/target that
  # its quick gate then built on for 21 min (main m_48101; architect m_48124:
  # the repo's hook, not the hub). The hub runs setup inside worktree_create,
  # which daemon/place_work.rs calls off the hub's loop (hub-loop 3a): a slow
  # clone holds only that placement. A failed setup fails the spawn
  # (worktree.rs prepare returns Err), so this never fails: no seed, a clone
  # that fails, a target already there, CARGO_TARGET_DIR set: a line, exit 0.
  cd "$(git rev-parse --show-toplevel 2>/dev/null || echo "$(dirname "$0")/..")" || exit 0
  t=rust/target
  if [ -n "${CARGO_TARGET_DIR:-}" ] || [ -e "$t" ] || [ ! -d rust ]; then
    echo "seed: nothing to do (${CARGO_TARGET_DIR:+CARGO_TARGET_DIR set}${CARGO_TARGET_DIR:-$t there or no rust/})"; exit 0
  fi
  from="$(seed_from 2>/dev/null)"
  if [ -n "$from" ] && cp -cR "$from" "$t.tmp" 2>/dev/null && seed_strip "$t.tmp" && mv "$t.tmp" "$t"; then
    echo "seed: $t, a clone of the warm seed $(basename "$from") (its deps; this repo's crates build here)"
  else
    rm -rf "$t.tmp"; echo "seed: no warm seed to clone in $(seeds_dir): the first cargo here builds cold"
  fi
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
  wtroot="$(bise_home)/worktrees"
  seeds="$(seeds_dir)"
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
  key="$(seed_key)"
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
    seed_strip "$tgt"
    echo "target: clone of the seed $seed ($((SECONDS - s))s)"
  else
    newest="$(ls -dt "$seeds"/* 2>/dev/null | grep -v '\.tmp$' | head -1)"
    [ -n "$newest" ] && cp -cR "$newest" "$tgt" && seed_strip "$tgt"
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
# light: whether this tree's quick gate runs no heavy step (no cargo, PROOF, desktop, office or
# proto step: the plan's heavy, one answer for this and for a light run's own refusal below,
# review-gates m_37126). Exit 0 and 'light', else 1 and 'heavy: <why>'. heavy-run runs a light
# quick gate without a slot or a disk reserve (fast-loop: a quick gate waited p50 409 s for a
# slot, its own run 32-41 s of python checks)
if [ "$mode" = light ]; then
  cd "$(dirname "$0")/.." || exit 2
  root="$PWD"
  base="${GATE_BASE:-HEAD}"
  changed="$( { git diff --name-only "$base"; git ls-files --others --exclude-standard; } | sort -u)" || { echo "heavy: git"; exit 1; }
  gate_plan $changed || { echo "heavy: gate_plan"; exit 1; }
  if [ -z "$heavy" ]; then echo light; exit 0; fi
  echo "heavy:$heavy"; exit 1
fi
[ "$mode" = stop ] && { gate_stop "$(cd "$(dirname "$0")/.." && pwd)"; exit 0; }
case "$mode" in quick|full|rerun|land) ;; *) echo "usage: gate.sh [quick|full|rerun|land <base> <head>|light|new [name]|done [name]|wait <file|pid>|stop]" >&2; exit 2 ;; esac
# ---- every gate of an agent goes through heavy-run's queue, one gate a tree
# (main m_48101: at 21:55 his Mac sat at load 93/12 on battery; side-resize's
# and esc-sheet's quick gates ran tests/gate.sh by hand, outside heavy-run,
# cold cargo and vitest with no slot, no load hold, no disk reserve, and
# esc-sheet's tree ran two gates at once). Not for a gate under a live gate
# (a test's), nor a land check (the hub's land queue runs those one by one).
gate_self="$(cd "$(dirname "$0")" && pwd)/gate.sh"
gate_outer=""
[ -n "${BISE_GATE_RUN:-}" ] && [ "$BISE_GATE_RUN" != $$ ] && [ -n "${BISE_GATE_START:-}" ] \
  && [ "$(proc_start "$BISE_GATE_RUN")" = "$BISE_GATE_START" ] && gate_outer=1
if [ "$mode" != land ] && [ -z "$gate_outer" ] && [ -z "${GATE_NO_HEAVY:-}" ]; then
  # one gate a tree: a second one in the same tree refuses at once, naming the
  # first (it would build and test the same files twice at once)
  gate_tree="$(cd "$(dirname "$0")/.." && pwd -P)"
  if [ "${GATE_TREE_LOCKED:-}" != "$gate_tree" ]; then
    gl="${BISE_HOME:-$HOME/.bise}/gate/locks"; mkdir -p "$gl" 2>/dev/null
    tl="$gl/tree-$(printf %s "$gate_tree" | shasum | cut -c1-16).lock"
    # -k: the file stays (one inode for every taker; without it a release
    # deletes it and two gates can each lock their own new file). One take,
    # -t 0: a mark the run writes tells lockf's own 75 (taken) from the gate's
    # rc (architect m_48124: a probe, then a second take, raced)
    mk="$tl.started.$$"; rm -f "$mk"
    /usr/bin/lockf -k -t 0 "$tl" /bin/bash -c ': >"$2"; echo "pid $$ $3 since $(date "+%F %T")" >"$1.holder"; shift 3; export GATE_TREE_LOCKED="$1"; shift; exec /bin/bash "$@"' \
      _ "$tl" "$mk" "$mode" "$gate_tree" "$gate_self" "$@" 2>/dev/null
    rc=$?
    if [ ! -e "$mk" ]; then
      echo "gate: a gate already runs in this tree ($(cat "$tl.holder" 2>/dev/null || echo "$gate_tree")): wait for it (tests/gate.sh wait) or stop it (tests/gate.sh stop), then run again"
      echo "GATE $mode FAILED: one gate a tree"; exit 1
    fi
    rm -f "$mk"
    exit $rc
  fi
  # the queue: heavy-run gives the slot, holds it while the Mac is busy and on
  # a low battery, reserves the disk; a docs-only quick runs at once (its light lane)
  hr="${GATE_HEAVY_RUN:-$HOME/.bise/run/heavy-run}"
  if [ -z "${HEAVY_SLOT:-}" ] && [ -x "$hr" ]; then
    echo "gate: $mode goes through heavy-run's queue ($hr; GATE_NO_HEAVY=1 skips it)"
    exec "$hr" "$gate_self" "$@"
  fi
fi
cd "$(dirname "$0")/.."
root="$PWD"
# a linked worktree has a .git file (the shared checkout a directory); a
# land check runs in the hub's land queue, not for the agent
[ -f "$root/.git" ] && [ "$mode" != land ] && sb_place "$root"
# a land's steps, decided before the run's setup (its folder, the logs, the
# sweep of dead runs: seconds under load): a land with no step ends here
if [ "$mode" = land ]; then
  base="${2:-}" head="${3:-}"
  { [ -n "$base" ] && [ -n "$head" ]; } || { echo "usage: gate.sh land <base> <head>" >&2; exit 2; }
  [ "$(git rev-parse HEAD)" = "$(git rev-parse "$head")" ] || { echo "GATE land FAILED: tree (HEAD is not $head)"; exit 1; }
  changed="$(git diff --name-only "$base" "$head")" || { echo "GATE land FAILED: diff ($base..$head)"; exit 1; }
  gate_plan $changed || { echo "GATE land FAILED: gate_plan"; exit 1; }
  [ "$land_steps" = 1 ] || { echo "GATE land GREEN (0s): no step for these paths"; exit 0; }
fi
share_machine
# a task worktree the hub made (<task dir>/<repo>, its <task dir>/owner)
# with no CARGO_TARGET_DIR and no rust/target yet: its target is
# <task dir>/target, a clone of the warm seed (0 bytes, ~3 s), like
# gate.sh new's, instead of a cold rust/target (3-13 GB and 2-5 min a task;
# 5 of them filled the disk on 2026-10-11). The hub's task dirs are removed
# with the task, target included.
if [ -z "${CARGO_TARGET_DIR:-}" ] && [ -f "$root/.git" ] && [ -f "$(dirname "$root")/owner" ] && [ ! -d "$root/rust/target" ]; then
  tdir="$(dirname "$root")/target"
  if [ ! -d "$tdir" ]; then
    s=$SECONDS sd="$(seeds_dir)" k="$(seed_key)" from="$(seed_from)"
    if [ -n "$from" ] && cp -cR "$from" "$tdir.tmp" 2>/dev/null && (cd "$root" && seed_strip "$tdir.tmp") && mv "$tdir.tmp" "$tdir"; then
      echo "target: $tdir, a clone of the seed $(basename "$from")$([ "$from" = "$sd/$k" ] || echo ' (older deps: the changed ones rebuild)') ($((SECONDS - s))s)"
    else
      rm -rf "$tdir.tmp"; mkdir -p "$tdir"; echo "target: $tdir (no seed to clone: a cold build)"
    fi
  fi
  export CARGO_TARGET_DIR="$tdir"
fi
# ---- BISE-244: what a run starts dies with it. Its TMPDIR, <gate dir>/<pid>
# (short: the hubs' sockets live under it), holds the tests' workspaces, so
# a test hub names it in its command line even orphaned; its sb-core and
# REPLs share its process group. The tmux server itself is never killed
# (the user's sessions may live in it): the test's shell in it is.
gate_procs() {  # <marker> <pid whose descendants go too | ""> <worktree binary prefixes... >: pids to kill
  local m="$1" me="$2"; shift 2
  # the marker goes through the environment: in awk's argv it would match awk
  ps -axww -o pid=,ppid=,pgid=,command= | GATE_M="$m" GATE_BINS="$*" GATE_KEEP="${glog:-}" awk -v me="$me" -v self=$$ '
    BEGIN { m = ENVIRON["GATE_M"]; bins = ENVIRON["GATE_BINS"]; keep = ENVIRON["GATE_KEEP"] }
    { p = $1 + 0; pp[p] = $2 + 0; pg[p] = $3 + 0; c = $0; sub(/^ *[0-9]+ +[0-9]+ +[0-9]+ /, "", c); cmd[p] = c; ids[++n] = p }
    function tmux(c) { return c ~ /^([^ ]*\/)?tmux( |$)/ }
    function orphan_bin(p,   i, k, b) {  # an orphan (ppid 1) of the worktree s binaries
      if (pp[p] != 1 || bins == "") return 0
      k = split(bins, b, " "); for (i = 1; i <= k; i++) if (cmd[p] == b[i] || index(cmd[p], b[i] " ") == 1) return 1
      return 0
    }
    END {
      for (p = self; p > 1 && !(p in anc); p = pp[p]) anc[p] = 1
      # the groups of our callers (a test pool, run_all, an orphaned hub that
      # started this gate): a match in one of them dies alone, never with its
      # group, or a gate inside a test kills the pool around it (release-52
      # m_39422: run_all s xargs got a TERM while land_every_e2e ran)
      for (a in anc) ag[pg[a]] = 1
      # this snapshot: the ps, its subshell and the subshell s children (awk)
      for (i = 1; i <= n; i++) if (cmd[ids[i]] == "ps -axww -o pid=,ppid=,pgid=,command=") sub_sh = pp[ids[i]]
      for (i = 1; i <= n; i++) { p = ids[i]; if (p == sub_sh || pp[p] == sub_sh) anc[p] = 1 }
      for (i = 1; i <= n; i++) { p = ids[i]
        # the tees of the run log (gate dir/logs): they end with its output
        if (tmux(cmd[p]) || (keep != "" && index(cmd[p], keep))) continue
        if (index(cmd[p], m) || (me != "" && pp[p] == me) || orphan_bin(p)) { k[p] = 1; if (pg[p] != pg[self] && !(pg[p] in ag)) g[pg[p]] = 1 }
      }
      for (i = 1; i <= n; i++) { p = ids[i]; if (pg[p] in g) k[p] = 1 }
      do { ch = 0; for (i = 1; i <= n; i++) { p = ids[i]; if (!(p in k) && (pp[p] in k)) { k[p] = 1; ch = 1 } } } while (ch)
      for (p in k) if (!(p in anc) && !tmux(cmd[p]) && !(keep != "" && index(cmd[p], keep))) print p
    }'
}
gate_nested() {  # the pid of the live gate this one runs under, or nothing
  # (BISE_GATE_RUN and its start time, exported by every gate below)
  [ -n "${BISE_GATE_RUN:-}" ] && [ "$BISE_GATE_RUN" != $$ ] && [ -n "${BISE_GATE_START:-}" ] \
    && [ "$(proc_start "$BISE_GATE_RUN")" = "$BISE_GATE_START" ] && echo "$BISE_GATE_RUN"
  return 0
}
gate_kill() {  # <pids>: TERM, then KILL what is left half a second later
  [ -n "$1" ] || return 0
  kill -TERM $1 2>/dev/null; sleep 0.5
  for p in $1; do kill -0 "$p" 2>/dev/null && kill -KILL "$p" 2>/dev/null; done
  return 0
}
# in a task's worktree, its own binaries left alone by a dead parent
wt_bins=""
# a gate started under a live gate (a test's land check: land_every_e2e's
# worktree cases run gate.sh land under the tip gate's run_all) sweeps
# nothing of its caller's: the dead runs and the orphans of its binaries are
# the outer gate's to sweep. Its own run's processes still die with it.
nested="$(gate_nested)"
if [ -f "$root/.git" ] && [ -z "$nested" ]; then
  t="$(cd "${CARGO_TARGET_DIR:-rust/target}" 2>/dev/null && pwd -P)"
  wt_bins="$root/repl-live $root/repl-scripted $root/sb-core ${t:+$t/debug/bise}"
fi
# the leftovers of dead runs (a kill -9, a crash): their processes now; their
# folder (the logs of a red run) after a day
# the gate dir (above): runs <gpre><pid>, the lock, a red step's log
gate_dirs
mkdir -p "$gdir"
# what a run's processes name in their command line: its folder (a /tmp
# one without /tmp: python may show it as /private/tmp)
gate_mark() { echo "$1/"; }
# a red or dead run's folder: only run_all's logs stay (sb-run-all.*)
gate_prune() {
  find "$1" -mindepth 1 -maxdepth 1 ! -name 'sb-run-all.*' ! -name .owner -exec rm -rf {} + 2>/dev/null
  for l in "$1"/sb-run-all.*; do
    [ -d "$l" ] && find "$l" -mindepth 1 -maxdepth 1 ! -name '*.log' ! -name '*.keep' -exec rm -rf {} + 2>/dev/null
  done
  return 0
}
for d in "$gpre"[0-9]*; do
  [ -z "$nested" ] && [ -d "$d" ] || continue
  p="${d##*[-/]}"
  kill -0 "$p" 2>/dev/null && [ "$(proc_start "$p")" = "$(cat "$d/.owner" 2>/dev/null)" ] && continue
  gate_kill "$(gate_procs "$(gate_mark "$d")" "")"
  if [ -n "$(find "$d" -maxdepth 0 -mtime +0 2>/dev/null)" ]; then rm -rf "$d" 2>/dev/null; else gate_prune "$d"; fi
done
[ -n "$wt_bins" ] && gate_kill "$(gate_procs "/bise-gate-none/" "" $wt_bins)"
run="$gpre$$"
rm -rf "$run"; mkdir -p "$run" && proc_start $$ >"$run/.owner" && echo "$root" >"$run/.root" || { echo "gate: cannot write $run"; exit 1; }
echo "gate $mode (pid $$): stop it with tests/gate.sh stop, never pkill (it matches every agent's gate)"
export TMPDIR="$run" BISE_GATE_RUN=$$ BISE_GATE_START="$(cat "$run/.owner")"
# every quick/full/rerun keeps its output in <gate dir>/logs/ for 7 days,
# whatever the caller did with it (a task's tmp goes at its drop, a green
# run's folder at its end): the loop audit's record (scripts/loop_audit.py)
glogs="$gdir/logs" glog=""
if mkdir -p "$glogs" 2>/dev/null; then
  find "$glogs" -name '*.log' -mtime +7 -delete 2>/dev/null
  gstart="$(date +%s)" glog="$glogs/$mode-$gstart-$$.log"
  if echo "# gate $mode agent=${SB_AGENT:-} sha=$(git rev-parse --short HEAD 2>/dev/null || echo none) start=$gstart root=$root" >"$glog" 2>/dev/null; then
    exec > >(tee -a "$glog") 2> >(tee -a "$glog" >&2)
  else glog=""; fi
fi
# the Mac stays awake while the gate runs: -s (PreventSystemSleep, honoured
# on AC power only) on top of -i, which a DarkWake ignores (display off,
# the user away: 'Maintenance Sleep' under heavy-run's caffeinate -i, issue
# 26). -w $$: it ends with this gate. Out of the job table (a subshell's),
# and every wait below names its pid (gate_queue_test).
[ -x /usr/bin/caffeinate ] && ( /usr/bin/caffeinate -s -i -w $$ >/dev/null 2>&1 & )
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
  # the run log's last line, its rc: after the tees drained what we wrote
  if [ -n "$glog" ]; then
    exec >&- 2>&-
    for _ in $(seq 20); do pgrep -f "tee -a $glog" >/dev/null 2>&1 || break; sleep 0.1; done
    echo "# gate end rc=$rc secs=$(( $(date +%s) - gstart ))" >>"$glog" 2>/dev/null
  fi
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
# a rerun too: it runs e2e/tmux tests like a full gate's, and next to one
# (audit-blind's rerun beside its second full gate, 2026-10-14) both sets
# timed out at ~350 s (partial_e2e, repl_start_e2e, subscriptions_e2e)
# a full gate on a clean tree whose sha a full gate with every test passed
# (green-shas.tsv, written below): the static checks only, no lock wait
green_file="$gdir/green-shas.tsv"
clean_tree() { [ -z "$(git status --porcelain 2>/dev/null)" ]; }
if [ "$mode" = full ] && [ "${GATE_FORCE:-}" != 1 ] && clean_tree \
  && g="$(awk -F'\t' -v s="$(git rev-parse HEAD)" '$1 == s { l = $0 } END { print l }' "$green_file" 2>/dev/null)" && [ -n "$g" ]; then
  python3 tests/word_lint.py --all || { echo "GATE full FAILED"; exit 1; }
  IFS=$'\t' read -r _ gend gsecs gagent glogf <<<"$g"
  echo "GATE full GREEN (0s): $(git rev-parse --short HEAD) passed a full gate with every test at $(date -r "$gend" '+%F %T') (${gsecs}s, ${gagent:-?}, $glogf): word_lint only (GATE_FORCE=1 runs it all)"
  exit 0
fi
case "$mode" in full|rerun) gate_lock ;; esac
# bend: the one this tree pins (bend/VERSION, scripts/bend-pin.sh)
. scripts/bend-pin.sh
gate_bend_dir="$(bend_bin_dir "$PWD" "$(bend_tools_home)")" || { echo "GATE FAILED: no bend for bend/VERSION"; exit 1; }
export PATH="$gate_bend_dir:$HOME/.cargo/bin:$PATH"
# the oldest macOS the binaries run on (rust/.cargo/config.toml, BISE-164):
# for the quick sb-core's cc too
export MACOSX_DEPLOYMENT_TARGET; MACOSX_DEPLOYMENT_TARGET="$(scripts/bins.sh macos-target)"
# a land's base and paths: set above (main's tip before it to the rebased tip)
if [ "$mode" != land ]; then
  base="${GATE_BASE:-HEAD}"
  changed="$( { git diff --name-only "$base"; git ls-files --others --exclude-standard; } | sort -u)"
fi
# the steps for these paths (tests/gate_plan.py), said in the log
if [ "$mode" != land ]; then
  gate_plan $changed || { echo "GATE $mode FAILED"; exit 1; }
  echo "plan:${heavy:- light}$([ -n "$dwhy" ] && echo "; desktop: $dwhy${drel:+ (related:$([ -n "$drel" ] && echo " $drel"))}")"
fi
# a light run (heavy-run's light lane: HEAVY_SLOT 'light <pid>', no slot, no disk reserve) runs
# no heavy step, whatever changed since heavy-run asked `gate.sh light` (review-gates m_37126)
if [ "$mode" = quick ] && [ "${HEAVY_SLOT:-}" != "" ] && [ "${HEAVY_SLOT%% *}" = light ] && [ -n "$heavy" ]; then
  echo "FAIL heavy step in a light run:$heavy: rerun through heavy-run (HEAVY_NO_LIGHT=1)"; echo "GATE quick FAILED"; exit 1
fi
# ./sb-core (not in git, BISE-114): the build of this tree's hub/ vendor/,
# from bins.sh's cache shared by every worktree (a copy; ~15 s on a miss).
# quick with a hub/ change: bins.sh compiles it in the background (below)
if [ "$mode" != land ] && { [ $hub_changed = 0 ] || [ "$mode" = full ]; }; then
  scripts/bins.sh sb-core || { echo "FAIL sb-core build"; exit 1; }
fi
# the EDR of a company Mac (CrowdStrike) deletes a fresh unsigned binary
# when it is spawned (~0.2 s): one clear line instead of 30 red tests
# (a rerun compiles it again in bins.sh's cache, where it survived so far)
sbcore_gone() {  # <file>: say so and true when the sb-core that was built is gone
  [ -n "$1" ] && [ ! -e "$1" ] || return 1
  echo "FAIL sb-core vanished after it was built: an antivirus (EDR) may have removed it ($1)"
}

# bise desktop's own checks (architect's merge review B3): typecheck, eslint,
# vitest and its build (`pnpm check`, no Playwright), so a proto change
# that breaks apps/desktop is red here. Run only when tests/test_map.py
# desktop says the change reaches it (a desktop path or a seam it reads),
# and always with full --all. A tree without apps/desktop: nothing.
# Without pnpm: 'skipped' in quick, red in full (strict). Without
# node_modules: quick skips; full installs them from the lockfile
# (--frozen-lockfile, no Electron download: check needs no binary).
# a launchd job (the nightly, a gate run outside the bash tool's file cap)
# or the hub's land queue has no shell setup: its PATH lacks the node
# manager's bin. Its default node (fnm's, Volta's, pnpm's own home) has
# pnpm next to node
desktop_path() {
  local d
  command -v pnpm >/dev/null 2>&1 && return 0
  for d in "${FNM_DIR:-$HOME/.local/share/fnm}/aliases/default/bin" "$HOME/.volta/bin" "$HOME/Library/pnpm"; do
    if [ -x "$d/pnpm" ]; then export PATH="$d:$PATH"; return 0; fi
  done
}
desktop_deps() {  # [strict]: node_modules (installed when strict); what is missing on stdout
  local miss=""
  desktop_path
  if ! command -v pnpm >/dev/null 2>&1; then miss="no pnpm on PATH (nor fnm's default, Volta's or ~/Library/pnpm)"
  elif [ ! -d "$root/apps/desktop/node_modules" ]; then
    if [ "${1:-}" = strict ]; then
      (cd "$root/apps/desktop" && ELECTRON_SKIP_BINARY_DOWNLOAD=1 pnpm -s install --frozen-lockfile --prefer-offline) >"$run/desktop_install.log" 2>&1 \
        || miss="pnpm install failed ($(tail -1 "$run/desktop_install.log"))"
    else miss="no node_modules; run pnpm -C apps/desktop install"; fi
  fi
  echo "$miss"
}
# quick's vitest share (gate-reds, main m_33154): when every changed path the desktop counts is
# under apps/desktop/src/ (the plan's drel), the tests that import them (vitest related) and the
# ones that read src/ by path (the ds laws, the CSP, the words...: grep) rather than all ~250
# files (157-182 s at load 30, and its worker RPC times out there). drel empty: all of them (a
# config, a fixture, a seam outside the app, a deleted file). Full: all.
# quick's whole-app mode, in apps/desktop: the bims' world simulations a change reaches
quick_bims_laws() {
  # shellcheck disable=SC2086
  if [ "${dbims:-0}" = 1 ]; then pnpm -s exec vitest run --project bims-laws
  elif [ -n "${dfiles:-}" ]; then pnpm -s exec vitest related --run --passWithNoTests --project bims-laws $dfiles
  fi
}
desktop_check() {
  [ -d "$root/apps/desktop" ] || return 0
  local miss; desktop_path; miss="$(desktop_deps "${1:-}")"
  if [ -n "$miss" ]; then
    if [ "${1:-}" = strict ]; then echo "FAIL desktop_check: $miss"; return 1; fi
    echo "skipped desktop_check: $miss"; return 0
  fi
  local s=$SECONDS log="$run/desktop_check.log" rel=""
  # shellcheck disable=SC2086
  [ "${1:-}" = strict ] || rel="${drel:+ $drel}"
  if [ -n "$rel" ]; then
    # by path (fs reads, globs, a file URL: review-gates m_33177): only the ones that read a
    # changed file, or whose reads are unknown (the plan's dbypath, tests/gate_plan.py
    # bypath_reads; all 38 took 19 s each run); the full gate runs every one
    # the unit project; the bims' world simulations (project bims-laws, ~1000 s of cpu) only the
    # ones whose imports hold a changed file (vitest related: a bims file, or the shared code a
    # law imports; usually none), architect m_55052
    local drc ran projects="--project unit"
    # lint: the changed files only (eslint of the whole app took 527 s at load 30, of 3 files
    # 14 s, fast-loop m_46374); typecheck still covers every file, and the full gate lints all.
    # --no-warn-ignored: $rel holds .css/.webp/.html too, which eslint warns are ignored, and
    # --max-warnings 0 made a css-only change red (review-gates m_46660)
    # shellcheck disable=SC2086
    (cd "$root/apps/desktop" && python3 ../../tests/conflict_markers.py && pnpm -s typecheck && pnpm -s exec eslint --max-warnings 0 --no-warn-ignored $rel \
        && pnpm -s exec vitest related --run --passWithNoTests $projects $rel \
        && pnpm -s exec vitest related --run --passWithNoTests --project bims-laws $rel && echo "@@ by path" \
        && { [ -z "$dbypath" ] || pnpm -s exec vitest run --passWithNoTests $projects $dbypath; }) >"$log" 2>&1
    drc=$?
    # what it ran, said and kept (<gate dir>/vitest-related.jsonl: the audit's 'vitest-related miss')
    local tests_rx='(✓|❯|×|FAIL) +src/[^ ]+\.test\.tsx?' related
    ran="$(grep -oE "$tests_rx" "$log" | awk '{ print $NF }' | sort -u | tr '\n' ' ')"
    related="$(sed '/^@@ by path$/q' "$log" | grep -oE "$tests_rx" | awk '{ print $NF }' | sort -u | tr '\n' ' ')"
    echo "  vitest related to$rel: $(wc -w <<<"$related" | tr -d ' ') files ($related); by path: ${dbypath:-none}"
    # a change no test imports: said, with its modules (51 of 339 related quicks ran no test)
    local bare; bare="$(printf '%s\n' $rel | grep -E '\.tsx?$' | grep -vE '\.test\.tsx?$' | tr '\n' ' ')"
    [ $drc = 0 ] && [ -z "$related" ] && [ -n "$bare" ] && echo "WARN 0 tests ran for ${bare% }: no test imports it (a test next to it, or the full gate's run)"
    python3 -c 'import json, sys, time; print(json.dumps({"t": int(time.time()), "agent": sys.argv[1], "sha": sys.argv[2], "changed": sys.argv[3].split(), "ran": sys.argv[4].split(), "rc": int(sys.argv[5])}))' \
      "${SB_AGENT:-?}" "$(git -C "$root" rev-parse HEAD 2>/dev/null)" "$rel" "$ran" "$drc" >>"$gdir/vitest-related.jsonl" 2>/dev/null
    if [ $drc = 0 ]; then echo "ok   desktop_check ($((SECONDS - s))s, vitest: related + by path; full runs all)"; return 0; fi
  # the whole app: an agent's quick lints through its tree's eslint cache (check:cached,
  # node_modules/.cache/eslint-quick: a file re-linted when its content changed). Its
  # typed rules read other files, which the cache doesn't track: a type change can leave an
  # unchanged file's red hidden there, so every other gate (full, land, the tip gate, a
  # release) lints cold with `check` (main m_54506, architect)
  # (check:cached tests the unit project; then the bims' world simulations: all of them when a
  # bims file changed (the plan's dbims), else the ones whose imports hold a changed app file
  # (dfiles: vitest related, usually none)
  elif (cd "$root/apps/desktop" && pnpm -s "$([ "$mode" = quick ] && [ "${1:-}" != strict ] && echo check:cached || echo check)" \
      && { [ "$mode" != quick ] || [ "${1:-}" = strict ] || quick_bims_laws; }) >"$log" 2>&1
  then echo "ok   desktop_check ($((SECONDS - s))s)"; return 0; fi
  echo "FAIL desktop_check"
  grep -E "error|Error|FAIL|✗|×" "$log" | head -30
  # one name a tree's gate (the agent's), never shared: side-resize's red overwrote
  # fast-loop's under the one name (2026-10-27 22:31)
  cp "$log" "$gdir/sb-gate-desktop_check-${SB_AGENT:-$$}.log"; echo "log: $gdir/sb-gate-desktop_check-${SB_AGENT:-$$}.log"
  return 1
}
# ---- land (docs/issues/28): `gate.sh land <base> <head>`, run by `sb land`
# (bise.toml's [land] check) in the worktree, rebased on main's tip <base>,
# before main moves: only the fast steps that are always green on main
# (lines, clippy, generated proto TS, the desktop's types and build, the
# office SDK, the kit), only for the paths <base>..<head> touches
# (the plan). No e2e, tmux, PROOF or vitest: the quick/full gate and the
# nightly keep those. No known reds here. A land with no step ended above,
# before the run's setup.
# Red: "FAIL <step>", its error lines, then "GATE land FAILED: <step>"
# (land_check.rs reads those words).
if [ "$mode" = land ]; then
  t0=$SECONDS  # its steps: the plan above
  ran=""
  lstep() {  # <name> <cmd...>
    local n=$1 s=$SECONDS log="$run/land-$1.log"; shift
    "$@" >"$log" 2>&1 & wait $!
    if [ $? = 0 ]; then echo "ok   $n ($((SECONDS - s))s)"; ran="$ran $n"; return 0; fi
    echo "FAIL $n ($((SECONDS - s))s)"
    # its error lines (and rustc's --> places), else the log's tail
    local pat='(^|[^a-z_])(error|Error|ERROR|FAIL|failed|✗|×)'
    if grep -qE "$pat" "$log"; then grep -E "$pat|^ *--> " "$log" | grep -v '^error: could not compile' | head -20
    else tail -20 "$log"; fi
    cp "$log" "$gdir/sb-gate-land-$n.log" 2>/dev/null && echo "log: $gdir/sb-gate-land-$n.log"
    echo "GATE land FAILED: $n"; exit 1
  }
  desktop_land() {
    local miss; desktop_path; miss="$(desktop_deps strict)"
    [ -z "$miss" ] || { echo "error: $miss"; return 1; }
    # + pnpm check's other half, which only quick (with node_modules), full and the nightly
    # ran, so its reds reached main through lands (gate-reds: hero.css's spacing, two eslint
    # errors in bims/render on 10-19): the ds laws (src/ds/lint.test.ts: bare controls, class
    # rules, the spacing ratchet; ~4 s) and eslint on the land's own .ts/.tsx (~30 s at load 28)
    local lint; lint="$(printf '%s\n' $changed | sed -n 's#^apps/desktop/\(.*\.tsx\{0,1\}\)$#\1#p' \
      | while read -r f; do [ -f "$root/apps/desktop/$f" ] && echo "$f"; done)"
    cd "$root/apps/desktop" && pnpm -s typecheck && pnpm -s exec vitest run src/ds/lint.test.ts || return 1
    # shellcheck disable=SC2086
    # (--no-warn-ignored: a file eslint.config ignores, src/proto/*.gen.ts, is no warning)
    if [ -n "$lint" ]; then pnpm -s exec eslint --max-warnings 0 --no-warn-ignored $lint || return 1; fi
    pnpm -s build || return 1
    # pnpm build and a release ship no source map (only the nightly's coverage-map build has them)
    local maps; maps="$(find out -name '*.map' 2>/dev/null | head -3)"
    [ -z "$maps" ] || { echo "error: source maps in out/ ($maps): BISE_DESKTOP_SOURCEMAP is set?"; return 1; }
  }
  target="${CARGO_TARGET_DIR:-$root/rust/target}"
  [ "${words:-0}" = 1 ] && lstep word_lint env GATE_BASE="$base" python3 tests/word_lint.py $changed
  [ "${bendw:-0}" = 1 ] && lstep pick_gate bash -c 'python3 tests/pick_gate.py --self-test && python3 tests/pick_gate.py'
  # every shim registered with its end date (issue 31); an overdue one is
  # only a warning here (its owner gets the red at their quick gate)
  lstep shims bash -c 'python3 tests/shims_check.py --self-test && python3 tests/shims_check.py --land'
  # apply and replay_batch reach no need: a cold record is fetched before, never inside (issue 36)
  lstep cold_apply bash -c 'python3 tests/cold_apply_lint.py --self-test && python3 tests/cold_apply_lint.py'
  # sb-core's records by position only in bend/hub/gen/ (state-schema)
  lstep state_guard env GATE_BASE="$base" bash -c 'python3 tests/state_guard.py --self-test && python3 tests/state_guard.py'
  # quick's clippy, in its warm dir: the workspace, so both share every artifact
  [ -n "$pkgs" ] && lstep clippy bash -c "cd rust && cargo clippy --offline -q --workspace --all-targets --target-dir '$target/clippy' -- -D warnings"
  [ "${proto_ts:-0}" = 1 ] && lstep proto_ts bash -c "cd rust && cargo test --offline -q -p bise-proto --features ts --test ts"
  [ -n "$dwhy" ] && lstep desktop desktop_land
  # the desktop checks this land would pick, said and logged (shadow mode: never fails the land)
  [ -n "$dwhy" ] && { tests/desktop_pick_shadow.sh land "$base" "$head" || true; }
  [ "${office:-0}" = 1 ] && lstep office_check tests/office_check.sh strict
  [ "${kit:-0}" = 1 ] && lstep kit kit_check
  [ "${cu:-0}" = 1 ] && lstep computer_use cu_check
  [ "${aqa:-0}" = 1 ] && lstep ambient_qa aqa_check
  echo "GATE land GREEN ($((SECONDS - t0))s):${ran:- no step for these paths}"
  exit 0
fi
# the tree's state (HEAD, the diff, the untracked files): a red full gate
# leaves the e2e/tmux tests it failed in <gate dir>/red-<state>, for
# gate.sh rerun on exactly this tree
tree_state() {
  { git rev-parse HEAD; git diff HEAD; git ls-files --others --exclude-standard | while read -r f; do echo "$f"; cat "$f" 2>/dev/null; done; } | shasum | cut -c1-16
}
red="$gdir/red-$(tree_state)"
if [ "$mode" = rerun ]; then
  # the e2e/tmux tests the last full gate failed on this same tree, alone
  # (one by one): what the agents did by hand. Green = every test of the
  # full gate passed on this tree; the flaky ones are said and kept in
  # timings.tsv (rc 1 then 0) for the audit. A Rust red has no list: fix it.
  [ -s "$red" ] || { echo "no red full gate on this tree (its e2e/tmux part): run gate.sh full"; exit 2; }
  s=$SECONDS t="$(tr '\n' ' ' <"$red")"
  echo "rerun alone: $t"
  tests/run_all.sh --serial --only "$t" & wait $!; rc=$?
  if [ $rc = 0 ]; then
    rm -f "$red"; echo "GATE rerun GREEN ($((SECONDS - s))s): failed in the full gate, passed alone (flaky): $t"
  else echo "GATE rerun FAILED"; fi
  exit $rc
fi
if [ "$mode" = full ]; then
  s=$SECONDS
  # every test against tests/word_literals.txt (issue 29, ~1 s)
  python3 tests/word_lint.py --all || { echo "GATE full FAILED"; exit 1; }
  # every shim registered, none past its removal release (issue 31, ~0.1 s)
  python3 tests/shims_check.py || { echo "GATE full FAILED"; exit 1; }
  # sb-core's records by position only in bend/hub/gen/ (state-schema, ~0.1 s)
  python3 tests/state_guard.py || { echo "GATE full FAILED"; exit 1; }
  # no new hand-written JS (js-to-ts, ~0.1 s)
  python3 tests/js_guard.py || { echo "GATE full FAILED"; exit 1; }
  # every gate self-test, whatever changed (quick runs them by path; ~30 s); the ratchets
  python3 tests/sleep_check.py >/dev/null || { python3 tests/sleep_check.py; echo "GATE full FAILED"; exit 1; }
  python3 tests/conflict_markers.py || { echo "GATE full FAILED"; exit 1; }
  python3 tests/loop_blocking_check.py || { echo "GATE full FAILED"; exit 1; }
  python3 tests/pick_gate.py || { echo "GATE full FAILED"; exit 1; }
  selftests_run selftests_all "GATE full FAILED" || exit 1
  export FUZZ_RUNS="${FUZZ_RUNS:-2000}"
  rm -f "$red"; export SB_RED_FILE="$red"
  # the e2e/tmux tests this change can break (tests/test_map.py; the user's
  # yes, 2026-10-12): the change = this tree vs its merge-base with main.
  # Everything: --all, a release agent, the nightly run, no branch change
  # (main's tip), shared code or a path no rule knows. The Rust part always.
  pick_args=() picked="" dwhy="" fchanged=""
  if [ "${2:-}" = --all ] || [ "${GATE_ALL:-}" = 1 ]; then picked="ALL --all"
  elif case "${SB_AGENT:-}" in release*) true ;; *) false ;; esac; then picked="ALL a release's gate"
  elif ! fb="$(git merge-base HEAD "${GATE_FULL_BASE:-main}" 2>/dev/null)"; then picked="ALL no merge-base with ${GATE_FULL_BASE:-main}"
  else
    fchanged="$( { git diff --name-only "$fb"; git ls-files --others --exclude-standard; } | sort -u)"
    if [ -z "$fchanged" ]; then picked="ALL no change vs ${GATE_FULL_BASE:-main}"
    else
      picked="$(python3 tests/test_map.py select $fchanged 2>"$gdir/pick-$$.why")" || picked="ALL test_map.py failed"
      dwhy="$(gate_plan $fchanged && echo "$dwhy")" || dwhy="gate_plan failed"
    fi
  fi
  # the desktop's checks: with every test, or when the change reaches it
  if [ "${picked#ALL}" != "$picked" ] && [ -z "$fchanged" ]; then dwhy="${picked#ALL }"; fi
  [ -d apps/desktop ] && echo "desktop checks: ${dwhy:-not run, the change does not reach apps/desktop (tests/test_map.py desktop)}"
  case "$picked" in
    ALL*) echo "e2e/tmux tests: all (${picked#ALL })" ;;
    *) echo "e2e/tmux tests: $(echo $picked | wc -w | tr -d ' ') picked for this change (tests/test_map.py; all of them run nightly and in a release):"
       sed 's/^# /  /' "$gdir/pick-$$.why"; pick_args=(--tests "$picked") ;;
  esac
  rm -f "$gdir/pick-$$.why"
  # ${a[@]+...}: macOS /bin/bash 3.2 calls an empty array unbound under set -u
  tests/run_all.sh ${pick_args[@]+"${pick_args[@]}"} & wait $!; rc=$?
  # which reds are known (tests/known_reds.txt, their owner on it) and which are new
  [ $rc = 0 ] || { [ -s "$red" ] && python3 tests/known_reds.py split $(cat "$red"); }
  [ $rc = 0 ] || { [ -s "$red" ] && echo "only e2e/tmux tests failed: gate.sh rerun runs them alone on this tree"; }
  [ $rc = 0 ] || sbcore_gone "$root/sb-core"
  # apps/desktop/src/proto matches bise-proto every time (a generated file
  # changed by a merge or a rebase can't slip through; quick runs it on a change)
  if [ $rc = 0 ]; then (cd rust && cargo test --offline -q -p bise-proto --features ts --test ts) || { echo "FAIL proto_ts"; rc=1; }; fi
  # the committed office SDK bundle is what its sources build (docs/office.md)
  if [ $rc = 0 ]; then tests/office_check.sh strict || { echo "FAIL office_check"; rc=1; }; fi
  # the office skills' scripts run on the SDK of this tree (plugins/{sheets,slides,docs})
  if [ $rc = 0 ]; then python3 tests/office_skills_check.py strict || { echo "FAIL office_skills"; rc=1; }; fi
  # and the desktop itself builds and passes its checks on those types (B3)
  if [ $rc = 0 ] && [ -n "$dwhy" ]; then desktop_check strict || rc=1; fi
  # every binary built runs on the macOS target (BISE-164): the Bend ones,
  # bise, and the engine when this tree has one
  bins=(./repl-live ./repl-scripted ./sb-core "${CARGO_TARGET_DIR:-rust/target}/debug/bise")
  for f in ./harness-demo rust/jsrt/target/debug/bend-jsrt; do [ -e "$f" ] && bins+=("$f"); done
  if [ $rc = 0 ]; then scripts/bins.sh minos "${bins[@]}" || rc=1; fi
  [ $rc = 0 ] && echo "GATE full GREEN ($((SECONDS - s))s)$([ ${#pick_args[@]} = 0 ] || echo ": $(echo $picked | wc -w | tr -d ' ') e2e/tmux tests picked")" || echo "GATE full FAILED"
  # every test and the desktop's checks ran, on a clean tree: its sha is green
  if [ $rc = 0 ] && [ ${#pick_args[@]} = 0 ] && [ -n "$dwhy" ] && clean_tree; then
    printf '%s\t%s\t%s\t%s\t%s\n' "$(git rev-parse HEAD)" "$(date +%s)" "$((SECONDS - s))" "${SB_AGENT:-${USER:-?}}" "${glog:-?}" >>"$green_file"
    echo "green sha recorded: $(git rev-parse --short HEAD) ($green_file)"
  fi
  exit $rc
fi

# ---- quick
t0=$SECONDS
# no blind time.sleep in tests/ (docs/issues/10-tests-wait.md): a test
# waits for a state with tests/wait.py (its --self-test: with the gate's own tests below)
python3 "$root/tests/sleep_check.py" >/dev/null || { python3 "$root/tests/sleep_check.py"; echo "GATE quick FAILED"; exit 1; }
# the ratchets on the tree, always (a few tenths of a second each):
# nothing slow on the hub's loop past tests/loop_blocking.txt (c4-loop-never-waits)
python3 "$root/tests/loop_blocking_check.py" >/dev/null || { python3 "$root/tests/loop_blocking_check.py"; echo "GATE quick FAILED"; exit 1; }
# no merge or stash conflict marker in a tracked file (2393001f landed stash markers)
python3 "$root/tests/conflict_markers.py" >/dev/null || { python3 "$root/tests/conflict_markers.py"; echo "GATE quick FAILED"; exit 1; }
# every compatibility shim in tests/shims.txt with its removal release, none
# past it (tests/shims_check.py, issue 31; always: the release moves with tags)
python3 "$root/tests/shims_check.py" || { echo "GATE quick FAILED"; exit 1; }
# a known red whose test went green on its last 2 shas since: red in its owner's quick, a note in
# the others' (tests/known_reds.py --expire, c5-tip-per-land; ~1 s: git merge-base on a few shas)
python3 "$root/tests/known_reds.py" --expire || { echo "GATE quick FAILED: your known_reds line expired"; exit 1; }
# sb-core's records built or opened only in bend/hub/gen/ (tests/state_guard.py, state-schema)
python3 "$root/tests/state_guard.py" || { echo "GATE quick FAILED"; exit 1; }
# no new hand-written JS: generated, vendored or on tests/js_allowlist.txt (js-to-ts, ~0.1 s)
python3 "$root/tests/js_guard.py" || { echo "GATE quick FAILED"; exit 1; }
# no new Bool.pick walk that never stops; the list of old ones only shrinks; no
# def that calls itself twice with the same arguments (2^n: hub-tick) (tests/pick_gate.py)
python3 "$root/tests/pick_gate.py" || { echo "GATE quick FAILED"; exit 1; }
# the gate's own tests whose sources changed (the plan's selftests, tests/gate_plan.py SELFTESTS:
# the ratchets' --self-test, gate_plan_test, test_map, include_map, the desktop pick, the sweep,
# the queue, keep_logs, runtime_check, covmap); the full gate runs every one
selftests_run selftests "GATE quick FAILED" || exit 1
# no changed test hard-codes a UI word past its count in the shrink-only
# tests/word_literals.txt (tests/word_lint.py, issue 29: read it by key)
GATE_BASE="$base" python3 "$root/tests/word_lint.py" $changed || { echo "GATE quick FAILED"; exit 1; }
cache="${CARGO_TARGET_DIR:-$root/rust/target}/gate-cache"
# the proofs files are checked in a copy of bend/ in it (tests/proof_run.py:
# bend refuses an import path with a dot, like ~/.bise/gate/...)
out="$(mktemp -d "${TMPDIR:-/tmp}/sbgateXXXXXX")"
mkdir -p "$cache"
fail() {  # <name> <log>: the failures, the log kept
  # the tests' sb-core gone: that line alone, not every test it failed
  if sbcore_gone "${SB_CORE_BIN:-}"; then
    cp "$2" "$gdir/sb-gate-$1-${SB_AGENT:-$$}.log"; echo "log of $1: $gdir/sb-gate-$1-${SB_AGENT:-$$}.log"; exit 1
  fi
  echo "FAIL $1"
  grep -E "^test .* FAILED|panicked|^error|^warning|^failures:|^Location|^Error" "$2" | head -30
  cp "$2" "$gdir/sb-gate-$1-${SB_AGENT:-$$}.log"; echo "log: $gdir/sb-gate-$1-${SB_AGENT:-$$}.log"; exit 1
}
# the bend part, in the background (it takes longer than cargo)
proof_job() {
  # the plan's proofs files: those whose imports' closure holds a changed .bend (a hub file:
  # the hub's proofs, never models' or the runtime's; the full gate checks every one)
  [ -n "$proof_files" ] || { echo "ok   PROOF (0 files: no proofs file imports the changed .bend)"; return 0; }
  local d="$out/proof"
  mkdir -p "$d"
  # 4 at once, each cached by its closure's content and the bend version (the gate dir's
  # proof-cache, every worktree's and run_all's): one line per failing file, the last line
  # ok/FAIL with the wall and CPU time
  # shellcheck disable=SC2086
  python3 tests/proof_run.py "$root" 4 "$d" --cache "$gdir/proof-cache" $proof_files >"$d/run.log" 2>&1 \
    || { grep -v '^ok  ' "$d/run.log" | head -40; return 1; }
  tail -1 "$d/run.log"
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
proof_pid=$!
rm -f "$cache"/sb-core-*  # the -O1 copies of the gate before
if [ $hub_changed = 1 ]; then
  # SB_CORE_BIN is set from $out/sbcore.path once it is built (below)
  { sbcore_job >"$out/sbcore.res" 2>&1; echo $? >"$out/sbcore.rc"; } &
  sbcore_pid=$!
else
  # a hit: ./sb-core was just put in place from it
  SB_CORE_BIN="$(scripts/bins.sh path sb-core 2>/dev/null)" && export SB_CORE_BIN || unset SB_CORE_BIN
fi
step() {  # <name> <cmd...>: one line when green, the failures and the log when red
  local n=$1 s=$SECONDS; shift
  # in the background: a kill or ctrl-c reaches the trap at once
  "$@" >"$out/$n.log" 2>&1 & wait $!
  if [ $? = 0 ]; then echo "ok   $n ($((SECONDS - s))s)"; else fail "$n" "$out/$n.log"; fi
}
# sb-core's side of bise_proto::core is generated too (issue 25): a change to
# the schema, its generator or bend/hub/gen/ runs bise-proto's tests (core_gen:
# a stale or hand-edited gen file is red) and gen/laws.bend,
# gen/journal-laws.bend (decode∘encode, the tag table, the journal fixture),
# gen/state.bend, state-laws.bend and state-codec.bend (sb-core's records,
# state-schema) must check; a bend/ change still runs PROOF above (the plan's core_gen, its
# bise-proto in pkgs)
# no `cargo build`: clippy checks every target, the tests compile and link
# the changed crates; the binary itself is built by gate.sh full.
# clippy runs next to the tests in its own target dir ($target/clippy: its
# own cargo lock; ~300 MB, 30-40 s the first time)
# No Rust or hub change ($pkgs empty, the plan): no clippy either. It only
# re-read an unchanged workspace (1-3 s warm), but cold, on a target the disk
# reclaim deleted, it took 200-490 s (fast-loop: p50 83 s, p90 491 s of the
# quick gate's p50 282 s over 78 runs, 74 of them with no Rust change)
target="${CARGO_TARGET_DIR:-$root/rust/target}"
clippy_pid=""
if [ -n "$pkgs" ]; then
{ s=$SECONDS
  if (cd rust && cargo clippy --offline -q --workspace --all-targets --target-dir "$target/clippy" -- -D warnings) >"$out/clippy.log" 2>&1
  then echo "ok   clippy ($((SECONDS - s))s)" >"$out/clippy.res"; echo 0 >"$out/clippy.rc"
  else echo 1 >"$out/clippy.rc"; fi; } &
clippy_pid=$!
fi
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
# the generated codecs' laws (issue 25, mapped above)
[ "${core_gen:-0}" = 1 ] && step core_gen_laws bash -c 'for f in laws journal-laws state state-laws state-codec; do r=$(BEND_NO_TELEMETRY=1 bend bend/hub/gen/$f.bend 2>&1); echo "$f: $r"; grep -q "ALL PROOFS CHECK" <<<"$r" || exit 1; done'
# bise desktop's TS types (apps/desktop/src/proto) are bise-proto's, generated:
# a stale or hand-edited file is red (BISE_PROTO_BLESS=1 on that test rewrites them)
[ "${proto_ts:-0}" = 1 ] && step proto_ts bash -c "cd rust && cargo test --offline -q -p bise-proto --features ts --test ts"
# the office SDK: office/dist/office.js is what office/sdk/src builds, and its tests pass on it
[ "${office:-0}" = 1 ] && step office_check tests/office_check.sh
# the kit's node checks and their types (land had them, quick did not: js-to-ts)
[ "${kit:-0}" = 1 ] && step kit kit_check
# the MCP Apps fixtures and example plugins against the spec's shapes, their views' innerHTML escaped
[ "${mcpapps:-0}" = 1 ] && step mcp_apps_plugins python3 tests/fixtures/mcp-apps/check.py
# the gallery plugins' unit tests (plugins/*/test/*.test.mjs and the kit's, node:test, no deps)
[ "${gallery:-0}" = 1 ] && step gallery_plugins bash -c 'shopt -s nullglob; t=(plugins/.view-kit/test/*.test.mjs plugins/*/test/*.test.mjs); [ ${#t[@]} = 0 ] || node --test "${t[@]}"'
# the extension's unit tests and the computer-use scripts' types
[ "${cu:-0}" = 1 ] && step computer_use cu_check
# the as-he-does helpers' unit test and the driver's types
[ "${aqa:-0}" = 1 ] && step ambient_qa aqa_check
# the office skills' scripts, through bise office run (skipped with words when bise or bend-jsrt is not built)
[ "${office_skills:-0}" = 1 ] && step office_skills python3 tests/office_skills_check.py
# then the desktop's own checks, when the change reaches it (tests/test_map.py desktop)
if [ -n "$dwhy" ]; then desktop_check || exit 1; fi
if [ -n "$clippy_pid" ]; then
  wait "$clippy_pid"
  [ "$(cat "$out/clippy.rc")" = 0 ] || fail clippy "$out/clippy.log"
  cat "$out/clippy.res"
else
  echo "skipped clippy: no Rust or hub change vs $base"
fi
if [ $bend_changed = 1 ]; then
  # its pid, never a bare `wait`: bash 5.2's also waits on the log's
  # `exec > >(tee ...)`, which end only with the gate (gate_queue_test)
  wait "$proof_pid"; cat "$out/proof.res"; [ "$(cat "$out/proof.rc")" = 0 ] || exit 1
fi
echo "GATE quick GREEN ($((SECONDS - t0))s):${pkgs:- no Rust tests}$([ $bend_changed = 1 ] && echo ' + PROOF') (full: gate.sh full)"
