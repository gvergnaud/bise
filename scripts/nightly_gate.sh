#!/usr/bin/env bash
# The nightly full gate: every e2e/tmux test on main's tip (gate.sh full
# --all), because the full gates of the day run only the tests their change
# can break (tests/test_map.py; docs/loop-speed.md §11).
#   scripts/nightly_gate.sh             run it now (through ~/.bise/run/heavy-run)
#   scripts/nightly_gate.sh --install [HH:MM]
#                                       a launchd job every day at HH:MM
#                                       (default 01:30; a Mac asleep then
#                                       runs it at its wake, past the
#                                       night's end: nothing runs)
#   scripts/nightly_gate.sh --uninstall
#   scripts/nightly_gate.sh --tip       main's tip, once per sha: the same
#                                       gate in the same tree, in one heavy
#                                       slot as a land check (heavy-run
#                                       --land, rank 1), never waiting for a
#                                       quiet machine (Gabriel 10:55: « merger
#                                       et tester après sur main »: the agents
#                                       land first, this gate tests main);
#                                       the sha tested is main's newest tip
#                                       when the slot comes. A green one
#                                       records the sha (gate.sh's
#                                       green-shas.tsv): a release cut needs
#                                       it. A red one writes tip-red
#                                       (scripts/tip_blame.py: the commits
#                                       since the last green, each one's
#                                       agent, the failing tests, the log;
#                                       its 'to:' line names who main
#                                       forwards it to, then main deletes
#                                       it). Nothing to do (the tip green or
#                                       tried): exits at once. Its results:
#                                       tip-<time>.log and tip-last (<sha>
#                                       green|red|infra <secs> <log>).
#   scripts/nightly_gate.sh --install-tip | --uninstall-tip
#                                       a launchd job that runs --tip when
#                                       main moves and every 10 min
# It waits for both heavy slots (~/.bise/run/heavy-run --alone: no agent's
# heavy job beside it; hours on a busy night; on battery it waits for AC),
# then, in the slot, checks main's tip out in its tree and runs the gate:
# the sha tested is main's tip when the slot came, and a tree removed while
# it waited is made again.
# Its tree: ~/.bise/run/nightly/tree/<repo> (a worktree; <repo>/../owner
# makes gate.sh full clone the warm seed into tree/target once). Never in
# the agents' worktrees folder: the hub's start sweep removes a clean
# folder there that no task owns (sweep.rs); it removed the nightly tree
# while the 2026-10-13 run waited for its slot.
# Its results, in ~/.bise/run/nightly/: <date>.log, `last` (one line: date,
# sha, green|red, seconds; `? infra` when the gate did not start), and `red`
# while main's tip is red (the sha, the failing tests, the log; or "did not
# start", an infra failure, no test ran), removed by the next green night.
# main keeps a watch on `red` (sb wake --on-file ~/.bise/run/nightly/red):
# it hears of a red night only.
# Tests: tests/nightly_gate_test.py (a fake repo, gate and heavy-run:
# NIGHTLY_HEAVY_RUN, BISE_HOME).
# The whole body is one function, read before it runs: main can land a new
# version of this file while a run waits for its slot.
nightly_gate() {
set -uo pipefail
LABEL=dev.bise.nightly-gate
here="$(cd "$(dirname "$0")/.." && pwd)"
main_tree="$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')"
out="${BISE_HOME:-$HOME/.bise}/run/nightly"
mkdir -p "$out"
plist="$HOME/Library/LaunchAgents/$LABEL.plist"
# node and pnpm for the desktop steps: fnm's default alias (a stable path; fnm's per-shell
# dirs are gone with their shell). launchd's PATH had neither: on 2026-10-18 desktop-checks
# and desktop-map exited 127 'node: No such file', desktop-full red on 'pnpm: command not found'
node_bin="${FNM_DIR:-$HOME/.local/share/fnm}/aliases/default/bin"
[ -d "$node_bin" ] && PATH="$node_bin:$PATH"
export PATH
launchd_path="$HOME/.bend/bin:$HOME/.cargo/bin:$node_bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

if [ "${1:-}" = --install ]; then
  at="${2:-01:30}" h="${at%%:*}" m="${at##*:}"
  mkdir -p "$(dirname "$plist")"
  cat >"$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key><array><string>/bin/bash</string><string>$main_tree/scripts/nightly_gate.sh</string></array>
  <key>StartCalendarInterval</key><dict><key>Hour</key><integer>$((10#$h))</integer><key>Minute</key><integer>$((10#$m))</integer></dict>
  <key>StandardOutPath</key><string>$out/launchd.out</string>
  <key>StandardErrorPath</key><string>$out/launchd.out</string>
  <key>EnvironmentVariables</key><dict><key>PATH</key><string>$launchd_path</string></dict>
</dict></plist>
EOF
  launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null
  launchctl bootstrap "gui/$(id -u)" "$plist" && echo "installed: $LABEL every day at $at ($plist); results in $out"
  exit $?
fi
if [ "${1:-}" = --uninstall ]; then
  launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null; rm -f "$plist"; echo "removed $LABEL"; exit 0
fi
TIP_LABEL=dev.bise.tip-gate
tip_plist="$HOME/Library/LaunchAgents/$TIP_LABEL.plist"
if [ "${1:-}" = --install-tip ]; then
  gd="$(git -C "$main_tree" rev-parse --absolute-git-dir)"
  mkdir -p "$(dirname "$tip_plist")"
  cat >"$tip_plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$TIP_LABEL</string>
  <key>ProgramArguments</key><array><string>/bin/bash</string><string>$main_tree/scripts/nightly_gate.sh</string><string>--tip</string></array>
  <key>WatchPaths</key><array><string>$gd/refs/heads/main</string><string>$gd/packed-refs</string></array>
  <key>StartInterval</key><integer>600</integer>
  <key>StandardOutPath</key><string>$out/tip-launchd.out</string>
  <key>StandardErrorPath</key><string>$out/tip-launchd.out</string>
  <key>EnvironmentVariables</key><dict><key>PATH</key><string>$launchd_path</string></dict>
</dict></plist>
EOF
  launchctl bootout "gui/$(id -u)/$TIP_LABEL" 2>/dev/null
  launchctl bootstrap "gui/$(id -u)" "$tip_plist" && echo "installed: $TIP_LABEL when main moves and every 10 min ($tip_plist); results in $out/tip-*"
  exit $?
fi
if [ "${1:-}" = --uninstall-tip ]; then
  launchctl bootout "gui/$(id -u)/$TIP_LABEL" 2>/dev/null; rm -f "$tip_plist"; echo "removed $TIP_LABEL"; exit 0
fi

ulimit -n 10240 2>/dev/null
unset SB_AGENT SB_SOCKET CARGO_TARGET_DIR

# the night's end (review-gates m_32494): NIGHTLY_END, an epoch set when the night is queued, the
# next NIGHTLY_UNTIL (default 06:00; empty: no end, a run by hand in the day) within 12 h; else
# (a Mac asleep through the night runs the job at its wake) that morning's, already past: nothing
# starts. In the slot nothing new starts at or after it: not the gate, not a step, not a batch of
# desktop checks (covmap --until), not a desktop-full step (nightly_desktop --until @<end>); the
# one running then ends, then the slots are free. NIGHTLY_NOW: a file holding an epoch, the
# tests' clock (covmap reads it too).
now() { if [ -n "${NIGHTLY_NOW:-}" ]; then cat "$NIGHTLY_NOW"; else date +%s; fi; }
[ -n "${NIGHTLY_NOW:-}" ] && export NIGHTLY_DESKTOP_NOW="$NIGHTLY_NOW"
night_until="${NIGHTLY_UNTIL-06:00}"
night_end() {
  local t e
  t="$(now)"
  e="$(date -j -f "%Y-%m-%d %H:%M:%S" "$(date -r "$t" +%F) $night_until:00" +%s 2>/dev/null)" || return 1
  # tomorrow's HH:MM by the calendar, never + 86400: on the night of a DST change (2026-10-25) a day
  # is 25 h, and + 86400 ended the night at 05:00 (the tip red on ca58673f6, run on 2026-10-24)
  local e2
  e2="$(date -j -f "%Y-%m-%d %H:%M:%S" "$(date -j -r "$t" -v+1d +%F) $night_until:00" +%s 2>/dev/null)" || return 1
  [ "$e" -le "$t" ] && [ $((e2 - t)) -le 43200 ] && e=$e2
  echo "$e"
}
late() { [ -n "${NIGHTLY_END:-}" ] && [ "$(now)" -ge "$NIGHTLY_END" ]; }
# the Mac's last wake before now, from pmset's log (no root): why a night ran late or not at all
# (NIGHTLY_PMSET_LOG: a file, the tests'). '2026-10-21 10:20:06 Wake from Hibernate [CDNVA]: due to ...'
last_wake() {
  { if [ -n "${NIGHTLY_PMSET_LOG:-}" ]; then cat "$NIGHTLY_PMSET_LOG"; else pmset -g log 2>/dev/null; fi; } |
    grep -E '^[0-9-]+ [0-9:]+ [+-][0-9]+ +(Wake|DarkWake) ' | tail -1 |
    sed -E 's/[[:space:]]+/ /g; s/^([0-9-]+ [0-9:]+) [+-][0-9]+ [A-Za-z]+ /\1 /' | cut -c1-160
}

if [ "${1:-}" = --in-slot ]; then
  # in the heavy slot: the tree at main's tip now, then the gate. A line
  # NOSTART <why> when the gate cannot start; LATE <why> when the slots came
  # past the night's end (nothing runs); START <date> <sha> when it does.
  repo="$(basename "$main_tree")"; case "$repo" in target|owner) repo=repo ;; esac
  tree="$out/tree" wt="$out/tree/$repo"
  cd / || exit 1
  if late; then echo "LATE the slots came at $(date -r "$(now)" +%H:%M), past the night's end $(date -r "$NIGHTLY_END" +%H:%M): nothing ran"; exit 3; fi
  tip="$(git -C "$main_tree" rev-parse --verify -q main)" || { echo "NOSTART no branch main in $main_tree"; exit 1; }
  if [ ! -e "$wt/.git" ]; then
    rm -rf "$wt"; git -C "$main_tree" worktree prune
    mkdir -p "$tree" && echo nightly >"$tree/owner" && git -C "$main_tree" worktree add -q --detach "$wt" "$tip" \
      || { echo "NOSTART cannot make the tree $wt"; exit 1; }
  fi
  git -C "$wt" checkout -q -f --detach "$tip" && git -C "$wt" clean -qfd \
    || { echo "NOSTART cannot check out main ($tip) in $wt"; exit 1; }
  [ -f "$wt/tests/gate.sh" ] || { echo "NOSTART no tests/gate.sh in $wt"; exit 1; }
  echo "START $(date) $(git -C "$wt" rev-parse --short HEAD)"
  slot_t0=$(date +%s)
  # caffeinate -is: the Mac never sleeps under the gate. On 2026-10-14
  # it took 'Maintenance Sleep' 5 times during the run (pmset -g log): each
  # test running across one ended at the wake, a timeout (5 reds; issue 26).
  # -i alone is not enough: at night the display is off, the Mac is in a
  # DarkWake, which ignores it; -s (PreventSystemSleep) holds on AC only
  cd "$wt" || { echo "NOSTART cannot enter $wt"; exit 1; }
  GATE_ALL=1 /usr/bin/caffeinate -is bash tests/gate.sh full --all
  rc=$?
  # the night's steps after the gate, in this tree and slot, by value (review-gates m_32494):
  # desktop-checks, desktop-map + its audit, desktop-full, core-record (fast-try p2, p7, p5).
  # night_step [--own <owner>] <name> <file> [<cmd>...]: skipped when <file> isn't in the tree;
  # at or past the night's end it never starts: "STEP <name>: none (past <end>)", no verdict,
  # never green; exit 0 green, 1 red: a "FAIL <name>" line (the red file names it) and the night
  # is red; 126 or 127 (its command not found or not runnable: a PATH without node) is red too,
  # "FAIL <name>: cannot run": a product step that never ran proves nothing (desktop-checks is the
  # one nightly run of the harness checks); 3 cut by the night's end: none; else no result.
  # --own: a tool of the gate, not the product: its red is a "STEP <name>: red (owner <owner>)"
  # line, its 126/127 no result, the night keeps the gate's verdict (review-gates m_29697 #1).
  # --past-end (after --own): it starts even past the night's end (the map of a cut run: minutes)
  end_at="${NIGHTLY_END:+$(date -r "$NIGHTLY_END" +%H:%M)}"
  night_step() {
    local own="" past=""
    [ "$1" = --own ] && { own=$2; shift 2; }
    [ "$1" = --past-end ] && { past=1; shift; }
    local n=$1 f=$2; shift 2
    [ -f "$f" ] || { echo "STEP $n: not in this tree, skipped"; return 0; }
    if [ -z "$past" ] && late; then echo "STEP $n: none (past the night's end $end_at, not started)"; return 3; fi
    echo "STEP $n: start $(date +%T)"
    /usr/bin/caffeinate -is "$@"; local r=$?
    case $r in
      0) echo "STEP $n: green" ;;
      1) if [ -n "$own" ]; then echo "STEP $n: red (owner $own; not a product red)"
         else echo "FAIL $n"; echo "STEP $n: red"; rc=1; fi ;;
      3) echo "STEP $n: none (cut by the night's end $end_at)" ;;
      126|127) if [ -n "$own" ]; then echo "STEP $n: no result (exit $r: cannot run)"
         else echo "FAIL $n: cannot run (exit $r)"; echo "STEP $n: red"; rc=1; fi ;;
      *) echo "STEP $n: no result (exit $r)" ;;
    esac
    return $r
  }
  # every harness check of the desktop, once, under V8 coverage: a red check is a product red
  # (FAIL <check>, the night red, review-gates m_29723); the night's end stops it between two
  # batches of specs: the specs it cut have no verdict and go in the map complete:false (picked
  # every time until a night runs them). Its cut nights in checks-runs.tsv (<date> <sha>
  # cut|ran <specs cut>): the second night in a row cut is red, "FAIL desktop-checks-cut"
  # (owner gate-nightly, the rule of desktop-runs.tsv). Then the coverage map from that run and
  # its audit, tools of the gate owned by try-select (fast-try p2): a refused map or a miss never
  # turns the night red (MISS lines, <gate dir>/desktop-pick-misses.tsv; the selection goes back
  # to shadow mode), and a missing map only makes the selection ALL
  day="$(date +%F)" gate_dir="${BISE_HOME:-$HOME/.bise}/gate"
  covmap=apps/desktop/tests/harness/coverage/covmap.mts
  # node runs the .ts itself (until engines >= 22.18 makes type stripping the default)
  ts_node="--experimental-strip-types --disable-warning=ExperimentalWarning"
  # a tip run skips the desktop checks, their map and audit, and core-record (review-gates
  # m_37134: the 04:31 tip run sat 5 h in desktop-checks, a slot and a 10 GB reserve held while
  # 5 agents waited for disk room): a tip gate is the gate per sha; the nightly runs these
  if [ -n "${TIP_RUN:-}" ]; then
    for n in desktop-checks desktop-map desktop-pick-audit; do echo "STEP $n: none (a tip run: the nightly runs it)"; done
  else
  rm -f "$gate_dir/desktop-cov-run/cut.txt"
  # the real-core checks' world (covmap brings it up) and core-record's run this tree's own bise
  # (scripts/tree_bise.sh: built in the gate's target, never the seed's clone a stopped gate left)
  if [ -f scripts/tree_bise.sh ] && [ -z "${QA_BISE:-}" ] && qb="$(bash scripts/tree_bise.sh)"; then
    export QA_BISE="$qb" AMBIENT_QA_APP_ROOT="$PWD"
  fi
  night_step desktop-checks "$covmap" env NODE_OPTIONS="$ts_node" node "$covmap" checks --gate-dir "$gate_dir" ${NIGHTLY_END:+--until "$NIGHTLY_END"}
  cr=$?
  if [ -f "$covmap" ]; then
    ncut=0
    if [ $cr = 3 ] && [ ! -s "$gate_dir/desktop-cov-run/cut.txt" ]; then ncut=all
    elif [ -s "$gate_dir/desktop-cov-run/cut.txt" ]; then ncut="$(wc -l <"$gate_dir/desktop-cov-run/cut.txt" | tr -d ' ')"; fi
    hist="$out/checks-runs.tsv"
    prev="$(tail -1 "$hist" 2>/dev/null | cut -f3)"
    printf '%s\t%s\t%s\t%s\n' "$day" "$(git rev-parse --short HEAD)" "$([ "$ncut" = 0 ] && echo ran || echo cut)" "$ncut" >>"$hist"
    # the checks' verdict on this exact sha, for a release (scripts/desktop_tag.py, review-gates
    # m_37177: the tip runs no desktop check, so a tag needs a night's or its own run of them):
    # checks-verdicts.tsv '<when> <sha> green|red|none <dir with reds.txt>'; cut is none
    cdir="$out/checks-$(date +%Y%m%d-%H%M%S)"
    mkdir -p "$cdir" && cp "$gate_dir/desktop-cov-run/reds.txt" "$cdir/reds.txt" 2>/dev/null
    case $cr in 0) cv=green ;; 1) cv=red ;; *) cv=none ;; esac
    printf '%s\t%s\t%s\t%s\n' "$(date +%Y-%m-%dT%H:%M:%S)" "$(git rev-parse HEAD)" "$cv" "$cdir" >>"$out/checks-verdicts.tsv"
    if [ "$ncut" != 0 ] && [ "$prev" = cut ]; then
      echo "FAIL desktop-checks-cut: the night's end cut the desktop checks two nights in a row ($ncut specs tonight; owner gate-nightly)"
      rc=1
    fi
  fi
  # a run the end cut still makes its map (review-gates m_32540): the specs that ran, the cut ones
  # complete:false; past the end too (no build, no app: minutes); none when no spec ran
  mapped=""
  if [ $cr = 3 ]; then
    [ -f "$gate_dir/desktop-cov-run/report.json" ] && mapped=--past-end
  else mapped=now; fi
  if [ -n "$mapped" ] && night_step --own try-select ${mapped#now} desktop-map "$covmap" env NODE_OPTIONS="$ts_node" node "$covmap" map --gate-dir "$gate_dir" --day "$day"; then
    night_step --own try-select desktop-pick-audit tests/desktop_pick_audit.py python3 -B tests/desktop_pick_audit.py --gate-dir "$gate_dir" --day "$day"
  fi
  fi
  # the desktop app's full end-to-end (fast-try p7): the packed candidate of this tip, every quick
  # as-he-does row with the first run, test:land, the full as-he-does pass, the DMG; a product
  # step. It runs even past the night's end, to start nothing and say so: its steps none, its
  # line in desktop-runs.tsv, where a second run in a row with no result is red
  # (scripts/nightly_desktop.sh, docs/desktop-try.md)
  # a tip run skips it (fast-loop): ~1h45 a sha would hold its slot past the next lands; the
  # nightly and a release's desktop_tag.py run it
  if [ -n "${TIP_RUN:-}" ] && [ -f scripts/nightly_desktop.sh ]; then
    echo "STEP desktop-full: none (a tip run: the nightly and a release run it)"
  elif [ -f scripts/nightly_desktop.sh ]; then
    echo "STEP desktop-full: start $(date +%T)"
    /usr/bin/caffeinate -is bash scripts/nightly_desktop.sh "$out/desktop-$day" ${NIGHTLY_END:+--until "@$NIGHTLY_END"}
    case $? in
      0) echo "STEP desktop-full: green" ;;
      1) echo "FAIL desktop-full"; echo "STEP desktop-full: red"; rc=1 ;;
      126|127) echo "FAIL desktop-full: cannot run"; echo "STEP desktop-full: red"; rc=1 ;;
      *) if late; then echo "STEP desktop-full: none (cut by the night's end $end_at)"
         else echo "STEP desktop-full: none (some steps not run)"; fi ;;
    esac
  else echo "STEP desktop-full: not in this tree, skipped"; fi
  # the recorded ambient-core of the QA world, typed and checked for invariants (feature-tests, p5)
  if [ -n "${TIP_RUN:-}" ]; then echo "STEP core-record: none (a tip run: the nightly runs it)"
  else night_step core-record scripts/core_record.sh bash scripts/core_record.sh; fi
  # heavy-run's release lane (heavy5.lock, --prio) runs beside an --alone job: a timing red of this
  # night may come from its load (review-gates m_35984); heavy.log says whether one ran meanwhile
  if awk -F'\t' -v t0="$slot_t0" -v now="$(date +%s)" '
      $4 != "heavy5.lock" { next }
      $3 == "-" { open[$1 FS $2] = $2; next }
      { delete open[$1 FS $2]; if ($2 <= now && $3 >= t0) busy = 1 }
      END { for (k in open) if (open[k] <= now) busy = 1; exit !busy }' "${HEAVY_LOG:-$HOME/.bise/run/heavy.log}" 2>/dev/null; then
    echo "LANE a release job ran in the lane during this night (heavy5.lock, heavy.log): a timing red may come from its load"
  fi
  exit $rc
fi

if [ "${1:-}" = --tip ]; then
  # a night that never started (review-gates m_37144: on 2026-10-22 the nightly job was disabled at
  # 01:00 and nothing said so): from 07:00, a day with no <date>.log gets its why once, in last and
  # red (main's watch). NIGHTLY_JOB_LOADED (tests): 1 or 0 instead of launchctl print
  nday="$(date -r "$(now)" +%F)"
  if [ "$(date -r "$(now)" +%H)" -ge 7 ] && [ ! -e "$out/$nday.log" ] && [ ! -e "$out/nostart-$nday" ]; then
    : >"$out/nostart-$nday"
    if [ "${NIGHTLY_JOB_LOADED:-$(launchctl print "gui/$(id -u)/$LABEL" >/dev/null 2>&1 && echo 1 || echo 0)}" = 0 ]; then
      why="the nightly job $LABEL is not loaded (disabled or removed; nightly_gate.sh --install puts it back)"
    else
      w="$(last_wake)"; why="its job is loaded but wrote no log${w:+ (the last wake in the pmset log: $w)}"
    fi
    echo "$nday ? nostart 0 $why" >"$out/last"
    { echo "nightly full gate did not start ($nday): $why"; echo "log: none ($out/$nday.log never written)"; } >"$out/red.tmp" && mv "$out/red.tmp" "$out/red"
  fi
  # main's tip as a land check (fast-loop, review-gates m_36198): once per sha, queued in
  # heavy-run's rank 1 whatever the load; the next move of main or tick tries again
  bh="${BISE_HOME:-$HOME/.bise}"
  tip="$(git -C "$main_tree" rev-parse --verify -q main)" || exit 0
  grep -q "^$tip	" "$bh/gate/green-shas.tsv" 2>/dev/null && exit 0
  [ "$(cut -d' ' -f1 "$out/tip-last" 2>/dev/null)" = "$tip" ] && exit 0
  # one tip run at a time, waiting ones included (lockf -t 0: a second one exits 75; the one in
  # line tests main's newest tip when its slot comes)
  [ -n "${TIP_LOCKED:-}" ] || TIP_LOCKED=1 exec /usr/bin/lockf -t 0 "$out/tip.lock" /bin/bash "$here/scripts/nightly_gate.sh" --tip
  log="$out/tip-$(date +%Y%m%d-%H%M%S).log" s=$SECONDS
  echo "QUEUED $(date): main $tip, a land check in heavy-run's queue" >"$log"
  # one heavy slot, ranked as a land check: an agent's heavy job may take the others
  # its wall cap (main m_37136: the 04:31 run held a slot 5 h): TIP_MAX_MIN (120) min, then
  # heavy-run stops it (rc 124, an infra state, no tip-red)
  TIP_RUN=1 HEAVY_MAX_MIN="${TIP_MAX_MIN:-120}" "${NIGHTLY_HEAVY_RUN:-$HOME/.bise/run/heavy-run}" --land /bin/bash "$here/scripts/nightly_gate.sh" --in-slot >>"$log" 2>&1
  rc=$?
  sha="$(awk '/^START /{print $NF; exit}' "$log")"
  if [ -z "$sha" ]; then state=infra; sha="$tip"
  else sha="$(git -C "$main_tree" rev-parse "$sha" 2>/dev/null || echo "$tip")"; [ $rc = 0 ] && state=green || state=red; fi
  echo "$sha $state $((SECONDS - s)) $log" >"$out/tip-last"
  # red: who landed since the last green, told through tip-red (main forwards it, main m_36219)
  j=(); for f in ${TIP_JOURNALS:-"$bh"/hubs/*/journal.jsonl}; do [ -f "$f" ] && j+=(--journal "$f"); done
  if [ "$state" = red ]; then
    python3 -B "$here/scripts/tip_blame.py" --repo "$main_tree" --tested "$sha" --green "$bh/gate/green-shas.tsv" \
      --log "$log" ${j[@]+"${j[@]}"} --out "$out/tip-red" || echo "tip_blame failed: no tip-red" >>"$log"
  fi
  # its flakes, green or red (run_all.sh's FLAKY lines: red, then green alone), each with its rate
  # over 14 days and its owner: flaky-open, which main forwards like tip-red (c5-tip-per-land)
  if [ "$state" != infra ] && grep -q '^FLAKY ' "$log"; then
    python3 -B "$here/scripts/tip_blame.py" --flaky "$bh/gate/timings.tsv" --repo "$main_tree" --tested "$sha" \
      --green "$bh/gate/green-shas.tsv" --log "$log" ${j[@]+"${j[@]}"} --out "$out/flaky-open" || echo "tip_blame failed: no flaky-open" >>"$log"
  fi
  find "$out" -name 'tip-*.log' -mtime +7 -delete 2>/dev/null
  exit $rc
fi

day="$(date +%F)" log="$out/$(date +%F).log" s=$SECONDS
# a night is never a tip run: TIP_RUN is the tip branch's word to its own slot run, and a test run
# under the tip gate (tests/nightly_gate_test.py) inherited it, so its fake nights skipped
# desktop-full (tip 156b45461)
unset TIP_RUN
if [ -n "$night_until" ]; then
  NIGHTLY_END="$(night_end)" || { echo "NIGHTLY_UNTIL wants HH:MM, got $night_until" >&2; exit 2; }
  # an epoch, or heavy-run's deadline (HEAVY_DEADLINE) and late() never fire (review-gates m_38064)
  [[ "$NIGHTLY_END" =~ ^[0-9]+$ ]] || { echo "nightly: the night's end is not an epoch: '$NIGHTLY_END'" >&2; exit 2; }
  export NIGHTLY_END
else
  # no end: never a caller's (2026-10-20: tests/nightly_gate_test.py, run by the night's own
  # gate, inherited that night's 06:00 and its 'no end' night started nothing)
  unset NIGHTLY_END
fi
# started past its end (review-gates m_37144: on 2026-10-21 the Mac hibernated on an empty battery
# at 01:30 and launchd ran the night at its 10:20 wake): a SKIP with its reason, never a slot taken
# for nothing nor a bare none; the second night in a row with nothing run is red
if late; then
  w="$(last_wake)"
  why="started at $(date -r "$(now)" +%H:%M), past the night's end $(date -r "$NIGHTLY_END" +%H:%M): ${w:+the Mac was asleep at the start time (pmset: $w)}${w:-why unknown (no wake in the pmset log)}"
  echo "SKIP $why" >"$log"
  prev="$(cut -d' ' -f3 "$out/last" 2>/dev/null)"
  echo "$day ? skip 0 $why" >"$out/last"
  case "$prev" in none|skip|nostart)
    { echo "nightly full gate: nothing ran two nights in a row ($day: $why)"; echo "log: $log"; } >"$out/red.tmp" && mv "$out/red.tmp" "$out/red"
    exit 1 ;;
  esac
  exit 3
fi
echo "QUEUED $(date): waiting for a heavy slot${NIGHTLY_END:+, nothing new starts after $(date -r "$NIGHTLY_END" '+%F %H:%M')}" >"$log"
# --alone: both heavy slots, never beside an agent's heavy job (2026-10-14:
# beside one, at load 8-9, 5 long tests timed out that pass alone; issue 26)
# HEAVY_DEADLINE: still queued at the night's end, the waiter gives up (review-gates m_38056: the
# night queued at 01:43 on 2026-10-23 still waited at 09:31, the cutoff never applied in the queue)
HEAVY_DEADLINE="${NIGHTLY_END:-}" "${NIGHTLY_HEAVY_RUN:-$HOME/.bise/run/heavy-run}" --alone /bin/bash "$here/scripts/nightly_gate.sh" --in-slot >>"$log" 2>&1
rc=$?
sha="$(awk '/^START /{print $NF; exit}' "$log")"
noslot="$(sed -n 's/^heavy-run: no slot by //p' "$log" | tail -1)"
if [ -z "$sha" ] && [ -n "$noslot" ]; then
  # no slot by the night's end: a skip, said with who held the slots; the second night in a row
  # with nothing run is red
  why="no slot by $noslot"
  echo "SKIP $why" >>"$log"
  prev="$(cut -d' ' -f3 "$out/last" 2>/dev/null)"
  echo "$day ? skip $((SECONDS - s)) $why" >"$out/last"
  case "$prev" in none|skip|nostart)
    { echo "nightly full gate: nothing ran two nights in a row ($day: $why)"; echo "log: $log"; } >"$out/red.tmp" && mv "$out/red.tmp" "$out/red"
    exit 1 ;;
  esac
  exit 3
fi
late_why="$(sed -n 's/^LATE //p' "$log" | tail -1)"
if [ -z "$sha" ] && [ -n "$late_why" ]; then
  # the slots came past the night's end: nothing ran, a night with no result (none), never green;
  # the second in a row is red (the rule of desktop-runs.tsv: the gate must not silently never run)
  prev="$(cut -d' ' -f3 "$out/last" 2>/dev/null)"
  echo "$day ? none $((SECONDS - s)) $late_why" >"$out/last"
  if [ "$prev" = none ] || [ "$prev" = skip ] || [ "$prev" = nostart ]; then
    { echo "nightly full gate: nothing ran two nights in a row ($day: $late_why)"
      echo "log: $log"; } >"$out/red.tmp" && mv "$out/red.tmp" "$out/red"
    exit 1
  fi
  exit 3
fi
if [ -z "$sha" ]; then
  # the gate did not start: an infra failure, no test ran
  why="$(sed -n 's/^NOSTART //p' "$log" | tail -1)"
  [ -n "$why" ] || why="$(grep -v 'already locked$' "$log" | tail -1) (exit $rc)"
  echo "$day ? infra $((SECONDS - s)) $why" >"$out/last"
  { echo "nightly full gate did not start ($day, an infra failure: no test ran): $why"
    echo "log: $log"; } >"$out/red.tmp" && mv "$out/red.tmp" "$out/red"
  [ $rc = 0 ] && rc=1
  exit $rc
fi
# the steps the night's end cut (none, never green), named after the verdict of what ran
cut="$(sed -nE 's/^STEP ([^:]*): none \((past|cut by) the night.s end.*/\1/p' "$log" | sort -u | paste -sd, -)"
if [ $rc = 0 ]; then
  echo "$day $sha green $((SECONDS - s))${cut:+ cut:$cut}" >"$out/last"; rm -f "$out/red"
  # the green nights, for the next red's blame (the lands since the last green night)
  [ -n "$sha" ] && printf '%s\t%s\n' "$(git -C "$main_tree" rev-parse "$sha" 2>/dev/null || echo "$sha")" "$(date +%s)" >>"$out/nightly-green.tsv"
else
  echo "$day $sha red $((SECONDS - s))${cut:+ cut:$cut}" >"$out/last"
  { echo "nightly full gate red on main $sha ($day, $((SECONDS - s)) s): $(grep -E '^FAIL ' "$log" | awk '{sub(/:$/, "", $2); print $2}' | tr '\n' ' ')"
    echo "log: $log"; } >"$out/red.tmp" && mv "$out/red.tmp" "$out/red"
  # who landed since the last green night (desktop-full runs only at night now: review-gates
  # m_36509): the same tip-red file main forwards, merged with one main hasn't sent yet
  if full="$(git -C "$main_tree" rev-parse "$sha" 2>/dev/null)"; then
    bh="${BISE_HOME:-$HOME/.bise}"
    j=(); for f in ${TIP_JOURNALS:-"$bh"/hubs/*/journal.jsonl}; do [ -f "$f" ] && j+=(--journal "$f"); done
    python3 -B "$here/scripts/tip_blame.py" --what "nightly" --repo "$main_tree" --tested "$full" --green "$out/nightly-green.tsv" \
      --log "$log" ${j[@]+"${j[@]}"} --out "$out/tip-red" || echo "tip_blame failed: no tip-red" >>"$log"
  fi
fi
# a night that ran: its flakes, green or red, in flaky-open (main forwards it, like tip-red)
if [ -n "$sha" ] && grep -q '^FLAKY ' "$log" && full="$(git -C "$main_tree" rev-parse "$sha" 2>/dev/null)"; then
  bh="${BISE_HOME:-$HOME/.bise}"
  j=(); for f in ${TIP_JOURNALS:-"$bh"/hubs/*/journal.jsonl}; do [ -f "$f" ] && j+=(--journal "$f"); done
  python3 -B "$here/scripts/tip_blame.py" --flaky "$bh/gate/timings.tsv" --what "nightly" --repo "$main_tree" --tested "$full" \
    --green "$out/nightly-green.tsv" --log "$log" ${j[@]+"${j[@]}"} --out "$out/flaky-open" || echo "tip_blame failed: no flaky-open" >>"$log"
fi
# old logs: two weeks
find "$out" -name '20*.log' -mtime +14 -delete 2>/dev/null
exit $rc
}
nightly_gate "$@"; exit $?
