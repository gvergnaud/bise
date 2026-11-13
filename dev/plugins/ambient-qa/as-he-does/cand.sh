#!/bin/bash
# The candidate-only as-he-does run (ambient-lead m_20081): his own promoted app may be running, so
# nothing here is addressed by bundle id or by name (no desktop-try.sh, no osascript, no pkill). The
# candidate's own path is opened with --real's stand-in env (HOME/BISE_HOME under ~/.bise/gate,
# BISE_DESKTOP_OFFSCREEN=1, a localhost CDP port), and only its own pids, found by its executable
# path, are stopped or restarted (SIGTERM, then wait). His app's pids are logged before and after:
# unchanged is part of the proof. Gates as run.sh (A front app, B his Library, C TCC + a helper under
# the candidate's path), each a kill of the run's own pids at its first breach.
#   cand.sh [--smoke] <out dir> <candidate app>
# --smoke (fast-try p3, every try): one boot on its own world (~/.bise/gate/smoke, built fresh from
# realshape.py's SMOKE_SHAPE, CDP port 9342, never real-shape's world or port): the pill, the core
# coming up, one message to harness' main and the fake provider's answer, a clean quit; the same
# gates A/B/C, stop check and Library check as the full run. Exit 0 green, 1 a red step, 3 a breach.
set -u
CAND_ARGS=("$@")
SMOKE=; [ "${1:-}" = --smoke ] && { SMOKE=1; shift; }
O=${1:?out dir}; A=${2:?candidate app}; mkdir -p "$O"
# the RESOLVED path (ambient-lead m_20102): a path through a symlink and the processes carry
# the resolved one, so a lookup by the given path missed the main process and the stop left 4
# processes (724c0bb9's first run)
A=$(cd "$A" && pwd -P) || { echo "no $2"; exit 2; }
P=${AS_HE_DOES_PYTHON:-$HOME/.pyenv/versions/3.11.13/bin/python3}
HERE=$(cd "$(dirname "$0")" && pwd); T=$(cd "$HERE/../../../.." && pwd)
# the drivers are .ts, run by node's type stripping: flagged before node 22.18 (js-to-ts, architect
# m_40557), on each node line only, so the candidate app never inherits it. Goes with engines >=22.18.
TS_NODE="--experimental-strip-types --disable-warning=ExperimentalWarning"
RS="$P -B $HERE/../realshape.py"
# a world per heavy slot (fast-loop, his pick q-slow on loop-4min; review-gates m_36599): world 1 is
# ~/.bise/gate/real-shape (CDP 9341), world n ~/.bise/gate/real-shape-n (CDP 9341 + 10 (n - 1));
# the smoke keeps its own (~/.bise/gate/smoke, 9342). CAND_WORLD_N: the one this run took (below).
world() { if [ "$1" = 1 ]; then echo "$HOME/.bise/gate/real-shape"; else echo "$HOME/.bise/gate/real-shape-$1"; fi; }
WN=${CAND_WORLD_N:-1}
G=$(world "$WN"); [ -n "$SMOKE" ] && G=$HOME/.bise/gate/smoke
H=$G/w/h
# One run per world and one run per candidate path (main m_30102): two runs in the two heavy slots
# shared the world, and one's stop sweep (every pid naming $G/w) killed the other's hubs
# (desk-cut-13 m_30091, 2026-10-18 00:06); a smoke and a full run on the same app kill each other's
# app by path (review-gates m_29603). Each lock is a lockf on a file: <world>/locks/world.lock and
# ~/.bise/gate/locks/cand-<sha16 of the app path>.lock. A busy one: one line naming its holder,
# then a wait of up to CAND_LOCK_WAIT s (7200), then exit 2 with that line; never a sweep.
# The order is always the candidate's lock first, then a world's (no two runs can deadlock): a run
# holding its candidate polls its worlds every 2 s (lockf -t 0 each) until one is free or the wait
# ends. Its worlds (CAND_WORLDS, 3): a release (heavy-run's --prio lane, HEAVY_SLOT 'heavy5.lock …',
# or CAND_PRIO=1) any, world 1 first; any other run 2..CAND_WORLDS, so world 1 stays a release's and
# a release never waits behind two agents' runs; CAND_WORLDS=1: world 1 for everyone (the old way).
# A world whose CDP port another process listens on is skipped with a line, never shared.
# Tested in tests/cand_lock_test.py.
LOCKS=$HOME/.bise/gate/locks
CAND_LOCK=$LOCKS/cand-$(printf '%s' "$A" | shasum -a 256 | cut -c1-16)
NW=${CAND_WORLDS:-3}
holder() { cat "$1.holder" 2>/dev/null || echo "an unknown run"; }
busy() { ! /usr/bin/lockf -t 0 "$1.lock" /usr/bin/true 2>/dev/null; }
prio() { case "${HEAVY_SLOT:-}" in heavy5.lock*) return 0;; esac; [ -n "${CAND_PRIO:-}" ]; }
mine() { if [ -n "$SMOKE" ] || [ "$NW" -le 1 ]; then echo 1; elif prio; then seq 1 "$NW"; else seq 2 "$NW"; fi; }
wlock() { if [ -n "$SMOKE" ]; then echo "$G/locks/world"; else echo "$(world "$1")/locks/world"; fi; }
if [ -z "${CAND_LOCKED:-}" ]; then
  mkdir -p "$LOCKS"; for n in $(mine); do mkdir -p "$(dirname "$(wlock "$n")")"; done
  W=${CAND_LOCK_WAIT:-7200}
  busy "$CAND_LOCK" && echo "$(date +%T) cand.sh: $CAND_LOCK is held by $(holder "$CAND_LOCK"); waiting up to $W s (or stop that run: kill its pid)"
  free=; for n in $(mine); do busy "$(wlock "$n")" || { free=1; break; }; done
  [ -z "$free" ] && for n in $(mine); do echo "$(date +%T) cand.sh: $(wlock "$n") is held by $(holder "$(wlock "$n")"); waiting up to $W s (or stop that run: kill its pid)"; done
  CAND_DEADLINE=$(( $(date +%s) + W )) CAND_LOCKED=1 /usr/bin/lockf -t "$W" "$CAND_LOCK.lock" /bin/bash "$0" "${CAND_ARGS[@]}"
  rc=$?
  if [ $rc = 75 ]; then
    for l in "$CAND_LOCK" $(for n in $(mine); do wlock "$n"; done); do busy "$l" && echo "cand.sh: gave up after $W s: $l is still held by $(holder "$l"); wait for it, or stop that run by its pid (kill <pid>), never by name"; done
    exit 2
  fi
  exit $rc
fi
if [ "$CAND_LOCKED" = 1 ]; then
  . "$HERE/ports.sh" || { echo "no $HERE/ports.sh"; exit 2; }
  skipped=" "
  while :; do
    for n in $(mine); do
      l=$(wlock "$n"); busy "$l" && continue
      p=$(cdp_world "$n"); [ -n "$SMOKE" ] && p=$CDP_SMOKE
      # a connect to it (lsof walks every process: 5 s and more at load 100+)
      if (exec 3<> "/dev/tcp/127.0.0.1/$p") 2> /dev/null; then
        case "$skipped" in *" $n "*) ;; *) echo "$(date +%T) cand.sh: world $n skipped: its CDP port $p is taken by another process"; skipped="$skipped$n ";; esac
        continue
      fi
      rm -f "$O/.world"
      CAND_LOCKED=2 CAND_WORLD_N=$n /usr/bin/lockf -t 0 "$l.lock" /bin/bash "$0" "${CAND_ARGS[@]}"
      rc=$?
      # 75 with no .world: lockf lost the race for that world (the run never started): the next one
      { [ $rc != 75 ] || [ -e "$O/.world" ]; } && exit $rc
    done
    [ "$(date +%s)" -ge "${CAND_DEADLINE:-0}" ] && break
    sleep 2
  done
  for n in $(mine); do l=$(wlock "$n"); busy "$l" && echo "cand.sh: gave up after ${CAND_LOCK_WAIT:-7200} s: $l is still held by $(holder "$l"); wait for it, or stop that run by its pid (kill <pid>), never by name"; done
  exit 2
fi
echo "$WN" > "$O/.world"
for l in "$G/locks/world" "$CAND_LOCK"; do echo "pid $$ since $(date '+%F %T'), out $O, app $A, world $WN" > "$l.holder"; done
# the locks live as long as this process (lockf's children): any exit frees them, a breach's exit 3,
# a kill and a crash included (the kernel drops a dead owner's lock). CAND_LOCK_HOLD=<s>: the test's
# stand-in run, holding the locks that long and nothing else (tests/cand_lock_test.py)
[ -n "${CAND_LOCK_HOLD:-}" ] && { sleep "$CAND_LOCK_HOLD"; exit "${CAND_LOCK_HOLD_RC:-0}"; }
# AS_HE_DOES_FAKE_VOICE=1: the candidate's core hears the words written to $VOICE (fake.rs: its
# BISE_HOME is under ~/.bise/gate, never his), for dict1.ts' ctrl+r take
VOICE=$G/voice-cand.txt; FV=()
[ "${AS_HE_DOES_FAKE_VOICE:-}" = 1 ] && { : > "$VOICE"; FV=(--env "BISE_AMBIENT_FAKE_VOICE=$VOICE"); export AS_HE_DOES_VOICE=$VOICE; }
# the driver's f2a7 rows find the candidate's own ambient-core by this path (a SIGSTOP of its pid only)
export AS_HE_DOES_APP=$A
HIS="$HOME/.bise/dev/try-src/apps/desktop/dist/mac-arm64/bise try.app"
# its main executable from its own Info.plist (amb-mac fb2e544f: candidates are "bise cand.app")
EXE=$([ -f "$A/Contents/Info.plist" ] && /usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$A/Contents/Info.plist" 2>/dev/null)
MAIN="$A/Contents/MacOS/${EXE:-bise try}"
. "$HERE/ports.sh" || { echo "no $HERE/ports.sh"; exit 2; }
PORT_CDP=$(cdp_world "$WN"); [ -n "$SMOKE" ] && PORT_CDP=$CDP_SMOKE
# PW_FROM: a checkout whose apps/desktop has node_modules (@playwright/test, pngjs); the workspace's
# may have none (try-gate-25e5's first runs: every drive 'Cannot find module')
export PW_FROM=${PW_FROM:-$T/apps/desktop}
say() { echo "$(date +%T) $*"; }
case "$A" in "$HIS"|"$HIS/"*) echo "that's his promoted app, not a candidate"; exit 2;; esac
[ -x "$MAIN" ] || { echo "no $MAIN"; exit 2; }
case "$G" in "$HOME/.bise/gate/"*) ;; *) echo "the world must be under ~/.bise/gate"; exit 2;; esac
# its own bundle id (ambient-lead m_20669): with his app's id, LaunchServices can route a quit,
# activate or open between his app and ours (7e691aef's rerun: his quit took the candidate too)
CAND_ID=$([ -f "$A/Contents/Info.plist" ] && /usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$A/Contents/Info.plist" 2>/dev/null)
HIS_ID=$([ -f "$HIS/Contents/Info.plist" ] && /usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$HIS/Contents/Info.plist" 2>/dev/null)
[ -n "$CAND_ID" ] && [ "$CAND_ID" != "$HIS_ID" ] || { echo "the candidate's bundle id ($CAND_ID) is his app's ($HIS_ID): build it with its own id"; exit 2; }
# the end of the candidate and its world on EVERY exit of this run (app-quits): the app is opened
# by 'open' (launchd's child, never ours), so a kill of this script's tree never reached it, and
# three stopped 11.11 passes left 'bise cand' in his menu bar. reaper.py, detached and armed on
# this pid's exit (green, red, timeout, TERM or KILL), stops what is left, one line per process in
# $O/reaper.log and here. TERM/INT/HUP: our own watchers and fake provider go first, then the exit.
"$P" -B "$HERE/reaper.py" watch $$ "$A" "$G" "$O/reaper.log" || { echo "the reaper did not arm: no run without it"; exit 2; }
trap 'endrun; exit 143' TERM; trap 'endrun; exit 130' INT; trap 'endrun; exit 129' HUP
HISR=$(cd "$HIS" 2>/dev/null && pwd -P || echo "$HIS")
# pids by fixed strings on the process table (no regex on a path with spaces and dots)
# (the awk itself carries the string: skipped; ps shows it as 'awk -v p <path>', no '=': every
# 'stop left 1 + 1' of c28260a9's runs was this awk counting itself)
pidsunder() { ps -axo pid=,command= | awk -v p="$1" 'index($0, p) && index($0, "awk -v p") == 0 { print $1 }'; }
hispids() { { pidsunder "$HIS/Contents/"; pidsunder "$HISR/Contents/"; } | sort -nu | tr '\n' ' '; }
# the candidate's processes (they RUN from its path: the DMG's repack only names its helper,
# desktop-tag-37dd2ac2's dmg red) and its world's (they name $G/w): reaper.py's one matcher, for
# stopcand here and for the reaper (below) that stops them when this run ends any other way
REAP="$P -B $HERE/reaper.py"
ours() { $REAP list app "$A" "$G" | sort -nu; }
worldpids() { $REAP list world "$A" "$G"; }
mainpid() { ps -axo pid=,command= | awk -v m="$MAIN" '{ pid = $1; sub(/^ *[0-9]+ /, ""); if ($0 == m || index($0, m " ") == 1) { print pid; exit } }'; }
killpids() { for p in "$@"; do kill -"${SIG:-TERM}" "$p" 2>/dev/null; done; }
# hubs outside the app (amb-mac e043678b, ambient-lead m_21810 (a)): every hub, REPL and plugin bridge
# the candidate starts runs from <prefix>/versions/<id>/bise, never from inside the bundle; only its
# main, its Electron helpers and ambient-core do. After the app quits: nothing from the bundle, its
# hubs live on. Read only (ps); one step per check in $O/hubs-steps.jsonl, hubs-outside.json at the end.
hubstep() { jq -cn --arg n "$1" --argjson ok "$2" --arg i "$3" '{name: $n, ok: $ok, info: $i}' >> $O/hubs-steps.jsonl; }
psall() { ps -axww -o pid=,command= | awk -v p="$1" 'index($0, p) && index($0, "awk -v p") == 0' | cut -c1-400; }
hubsnow() { # <when>: while the app runs
  local b other hubs bad good repls
  b=$(psall "$A/Contents/")
  # 'bise pty' is the terminal panel's shell host (main's child, gone at quit), not a hub/REPL/bridge
  other=$(echo "$b" | grep -v '^$' | grep -vF "$A/Contents/MacOS/" | grep -vF "$A/Contents/Frameworks/" | grep -vF " ambient-core" | grep -vF "$A/Contents/Resources/bin/bise pty ")
  hubs=$(psall "$G/w" | grep -F ' sbd --workspace ')
  bad=$(echo "$hubs" | grep -F "$A/Contents/")
  good=$(echo "$hubs" | grep -F '/versions/' | grep -vF "$A/Contents/")
  repls=$(ps -axww -o pid=,command= | grep -F 'repl-live' | grep -v grep | cut -c1-300)
  { echo "== $1 $(date +%T)"; echo "-- from the bundle:"; echo "$b"; echo "-- world hubs:"; echo "$hubs"; echo "-- repl-live:"; echo "$repls"; } >> $O/hubs-outside.txt
  hubstep "$1: only main, its helpers and ambient-core run from the bundle (no hub, REPL or plugin bridge)" "$([ -z "$other" ] && echo true || echo false)" "$(echo "$b" | grep -vc '^$') from the bundle; others: ${other:-none}"
  hubstep "$1: the candidate's hubs run from versions/<id>/bise (at least one, none from the bundle)" "$([ -z "$bad" ] && [ -n "$good" ] && echo true || echo false)" "from versions: $(echo "$good" | grep -vc '^$'); from the bundle: $(echo "$bad" | grep -vc '^$'); exes: $(echo "$hubs" | sed -n 's/^ *[0-9]* \(.*\) sbd --workspace .*/\1/p' | sort | uniq -c | tr '
' ';')"
  hubstep "$1: no repl-live from the bundle" "$(echo "$repls" | grep -qF "$A/Contents/" && echo false || echo true)" "$(echo "$repls" | grep -vc '^$') repl-live in all; from the bundle: $(echo "$repls" | grep -cF "$A/Contents/")"
}
hubsafter() { # <when>: the app's processes just ended, the world's not yet stopped
  local b hubs
  b=$(psall "$A/Contents/"); hubs=$(psall "$G/w" | grep -F ' sbd --workspace ' | grep -F '/versions/')
  { echo "== $1 after quit $(date +%T)"; echo "-- from the bundle:"; echo "$b"; echo "-- world hubs from versions:"; echo "$hubs"; } >> $O/hubs-outside.txt
  hubstep "$1: after the app quit, 0 processes from the bundle while its hubs live on" "$([ -z "$b" ] && [ -n "$hubs" ] && echo true || echo false)" "from the bundle: $(echo "$b" | grep -vc '^$'); its hubs alive: $(echo "$hubs" | grep -vc '^$')"
}
# stop: SIGTERM to the candidate's main process (Electron: app.quit -> will-quit, as an Apple Event
# quit), wait for every process under its path to go, then SIGTERM, then SIGKILL what stays; then the
# world's own processes (its hubs), by pid
stopcand() {
  local m; m=$(mainpid)
  [ -n "$m" ] && kill -TERM "$m" 2>/dev/null
  for i in $(seq 60); do [ -z "$(ours)" ] && break; sleep 0.5; done
  # (a): the app is gone, its hubs not yet stopped
  [ -z "$(ours)" ] && hubsafter "stop ${STOPN:=0}" && STOPN=$((STOPN + 1))
  killpids $(ours); sleep 2; SIG=KILL killpids $(ours)
  local w; w=$(worldpids)
  [ -n "$w" ] && killpids $w; sleep 1
  w=$(worldpids); [ -n "$w" ] && { sleep 2; SIG=KILL killpids $w; sleep 1; }
  # a process seen in the count can be gone a moment later (c28260a9's runs: 1 + 1 left, nothing
  # by the time ps named them): counted again for up to 10 s, named at the first nonzero count
  local lc lw left=""
  for i in $(seq 20); do
    lc=$(ours | wc -l | tr -d ' '); lw=$(worldpids | wc -l | tr -d ' ')
    [ "$lc" = 0 ] && [ "$lw" = 0 ] && break
    [ -z "$left" ] && left=$(ps -o pid=,ppid=,stat=,etime=,command= -p "$(echo $(ours) $(worldpids) | tr ' ' ',')" 2>/dev/null | cut -c1-240)
    sleep 0.5
  done
  [ -n "$left" ] && echo "$left" > $O/left-$(date +%H%M%S).txt
  say "stopped: left under the candidate $lc, naming the world $lw"
  [ "$lc" = 0 ] && [ "$lw" = 0 ] || {
    # which ones (c28260a9's first run left 1 + 1 that were gone a minute later: say what they were)
    { echo "the stop left $lc + $lw processes (10 s after)"; echo "$left"; ps -o pid=,ppid=,stat=,etime=,command= -p "$(echo $(ours) $(worldpids) | tr ' ' ',')" 2>/dev/null | cut -c1-240; } > $O/BREACH
    cat $O/BREACH; return 1; }
}
opencand() {
  open -g -n -a "$A" --env HOME=$H --env BISE_HOME=$H/.bise --env BISE_DESKTOP_OFFSCREEN=1 ${FV[@]+"${FV[@]}"} \
    --args --remote-debugging-address=127.0.0.1 --remote-debugging-port=$PORT_CDP
  for i in $(seq 120); do [ -n "$(mainpid)" ] && break; sleep 0.25; done
  say "open: $A ($(grep commit= "$A/Contents/Resources/bin/VERSION")), pid $(mainpid), home $H/.bise"
}
endrun() { touch $O/stop; kill ${TPID:-} ${NPID:-} ${CPID:-} ${CCPID:-} 2>/dev/null; kill $(cat $G/fake/fake.pid 2>/dev/null) 2>/dev/null; }
breach() { [ -e $O/BREACH ] && { say "BREACH: $(cat $O/BREACH)"; stopcand; endrun; say "his app after: $(hispids)"; exit 3; }; }
# gate B with attribution (ambient-lead m_20102, architect m_64344): a change in the userData of one
# of HIS apps while that app runs is his app's write, logged with its owner (his-writes.txt), the run
# goes on; anything else = kill. The call is realshape.py lib_verdict (its test: tests/ambient_qa_test.py),
# in this order: a folder the candidate says is its own (its helpers' --user-data-dir, its BISE_HOME)
# inside one of his folders; a change elsewhere, or in the folder of an app of his that does not
# run; lsof (read only) on the candidate's pids, a backstop that sees open files at check time only.
# His apps: the promoted try app and his installed release (/Applications/bise.app, userData 'bise':
# 11.11 lane B stopped on its Dawn*/GPUCache and updates/route.json); each one's userData is
# Application Support/<its bundle name>.
HISREL=${AS_HE_DOES_HIS_RELEASE:-/Applications/bise.app}
CANDPREF="$HOME/Library/Preferences/$CAND_ID.plist"
# libverdict's args: --his <his app>::<its userData>::1 when it runs, ::0 when not; one per app
hisdirs() {
  local app; for app in "$HIS" "$HISREL"; do
    echo "--his"
    echo "$app::$HOME/Library/Application Support/$(basename "$app" .app)::$([ -n "$(pidsunder "$app/Contents/")" ] && echo 1 || echo 0)"
  done
}
libgate() {
  local out; out=$($RS libsnap --out $O/after-$1.json --diff $O/before.json) && return 0
  echo "$out"
  # (the folder's own line counts as his userData too: run 4 stopped on it)
  # the candidate's OWN preferences domain is allowed and logged (main m_20867: Chromium writes 3 keys
  # through cfprefsd at startup, under his real home whatever HOME is); deleted at the run's end
  echo "$out" | sed -n 's/^  //p' | grep -xF "$CANDPREF" | sed "s|^|$1 |" >> $O/cand-prefs.txt
  local pids; pids=$(sort -u $O/pids.txt 2>/dev/null | tr '\n' ',' | sed 's/,$//')
  # the candidate's processes: their command lines (its own folders) and the files they hold open
  : > $O/our-fds-$1.txt; : > $O/our-cmds-$1.txt
  [ -n "$pids" ] && lsof -n -P -Fn -p "$pids" 2>/dev/null | sed -n 's/^n//p' > $O/our-fds-$1.txt
  [ -n "$pids" ] && ps -o command= -p "$pids" > $O/our-cmds-$1.txt 2>/dev/null
  local args=() a; while IFS= read -r a; do args+=("$a"); done < <(hisdirs)
  local res; res=$(echo "$out" | sed -n 's/^  //p' | $RS libverdict --cand-pref "$CANDPREF" --fds $O/our-fds-$1.txt \
    --cmds $O/our-cmds-$1.txt --cand-path "$H/.bise" "${args[@]}")
  local v; v=$(echo "$res" | head -1)
  case "$v" in
    ok) say "library $1: only the candidate's own $CANDPREF (allowed, logged)"
        cp $O/after-$1.json $O/before.json; return 0;;
    his) say "library $1: only the userData of his apps that run changed, no candidate folder or fd there: his apps' write, logged in his-writes.txt"
        echo "$res" | sed -n 's/^his	//p' | sed "s|^|$1	|" | tee -a $O/his-writes.txt
        cp $O/after-$1.json $O/before.json; return 0;;
  esac
  { echo "library $1 ($v)"; echo "$res" | sed -n 's/^cand	/candidate folder: /p'; } > $O/BREACH; breach
}
# the candidate's own preferences domain, at the end of every mode (the quick one too: cand-e91f98d7
# run): through cfprefsd first (it caches and writes back); an emptied domain can leave its file as
# '{}' (run 40fc444d): only then rm it, and check after ~5 s that cfprefsd didn't write it back.
# cfprefsd itself is never touched (ambient-lead m_20883). Every candidate has the one id
# (dev.bise.desktop.cand): while a run on another world (or the smoke) holds its lock, its app may
# still write that domain, so the delete is left to the last run to end (logged)
candprefs() {
  local n; for n in $(seq 1 "$NW") smoke; do
    [ "$n" = "$WN" ] && [ -z "$SMOKE" ] && continue
    [ "$n" = smoke ] && [ -n "$SMOKE" ] && continue
    local l; if [ "$n" = smoke ]; then l=$HOME/.bise/gate/smoke/locks/world; else l=$(world "$n")/locks/world; fi
    [ -e "$l.lock" ] && busy "$l" && { say "cand prefs: left to the last run ($l is held by $(holder "$l"))"; grep 'cand prefs:' $O/run.out 2>/dev/null >> $O/cand-prefs.txt; return 0; }
  done
  defaults delete "$CAND_ID" > $O/cand-prefs-delete.txt 2>&1; echo "rc=$?" >> $O/cand-prefs-delete.txt
  if [ -e "$CANDPREF" ]; then say "cand prefs: 'defaults delete $CAND_ID' done, the file is still there ($(stat -f %z "$CANDPREF") bytes, $(plutil -p "$CANDPREF" 2>/dev/null | tr -d '[:space:]'))"
  else say "cand prefs: 'defaults delete $CAND_ID' done, the file is gone"; fi
  if [ -e "$CANDPREF" ] && [ "$(plutil -p "$CANDPREF" 2>/dev/null | tr -d '[:space:]')" = "{}" ]; then
    rm -f "$CANDPREF"; sleep 5
    if [ -e "$CANDPREF" ]; then say "cand prefs: the empty $CANDPREF came back after rm: left, logged"
    else say "cand prefs: empty after the delete: rm'd, still gone after 5 s"; fi
  fi
  grep 'cand prefs:' $O/run.out 2>/dev/null >> $O/cand-prefs.txt
}
# the candidate's userData in the world (userdata.ts standInPaths: HOME is the world's): window-frame.json
drive() { AS_HE_DOES_BISE_HOME=$H/.bise AS_HE_DOES_USERDATA="$H/Library/Application Support/$(basename "$A" .app)" AS_HE_DOES_TAG=${TAG:-} NODE_OPTIONS="$TS_NODE" node $HERE/driver.ts $PORT_CDP $O "$@" > $O/driver-$1${TAG:+-$TAG}.log 2>&1; echo $? > $O/driver-$1${TAG:+-$TAG}.rc; tail -4 $O/driver-$1${TAG:+-$TAG}.log; breach; }

# the end of a quick or smoke run: the gates' watchers stop, the fake provider and the world's last
# processes (by pid), the candidate's prefs domain, gate B's final diff, his app's pids again
wrapup() {
  touch $O/stop; wait $FPID $CCPID; kill $TPID $NPID $CPID 2>/dev/null
  kill $(cat $G/fake/fake.pid) 2>/dev/null
  w=$(ps -axo pid=,command= | grep -F "$G/w" | grep -v grep | awk '{print $1}'); [ -n "$w" ] && killpids $w
  candprefs
  libgate final
  say "his app after: $(hispids)" | tee -a $O/his.txt
}

T0=$(date +%s)
rm -f $O/BREACH $O/stop $O/pids.txt $O/hubs-steps.jsonl $O/hubs-outside.txt $O/smoke.json $O/smoke-verdict.json
say "his app before: $(hispids)" | tee $O/his.txt
$RS libsnap --out $O/before.json
# a world that did not build stops the run (ahd-rows): 95143c0dabcd's quick run went on with no
# shape.json, the candidate opened on an empty home's first run ('hi. i'm bise.') and every row from
# 'sidebar' on read as 18 app reds. Nothing is started yet: exit 2 with the reason, no gate to stop.
if [ -n "$SMOKE" ]; then
  $RS smoke-world --root $G/w > $O/world-build.log 2>&1; wrc=$?
else
  # rebuilt at every run's start (emptied first): a killed run's leftovers never reach the next
  $RS build --root $G/w > $O/world-build.log 2>&1; wrc=$?
fi
tail -8 $O/world-build.log
if [ $wrc != 0 ]; then
  say "the world did not build (rc $wrc, $O/world-build.log): no candidate opened, no row run"
  [ -n "$SMOKE" ] || [ -f "$HOME/.bise/gate/real-shape/shape.json" ] || say "no ~/.bise/gate/real-shape/shape.json: make it once with '$P $HERE/../realshape.py snapshot' (reads his state read-only)"
  exit 2
fi
# every model request in a line (the quote row reads it), in the world, never /tmp's default
export FAKE_LOG=$G/fake/requests.jsonl AS_HE_DOES_FAKE_LOG=$G/fake/requests.jsonl
# the files the driver's words carry, under the world (fx.ts, regress-25e5): an out dir in an
# agent's tmp (~/.bise/hubs/harness-<id>/...) names 'harness', and bise's thread routed them there
export AS_HE_DOES_FX=$G/fx
mkdir -p $G/fake; : > $FAKE_LOG; ($P -u $T/tests/fake_provider.py 0 > $G/fake/fake.out 2> $G/fake/fake.err & echo $! > $G/fake/fake.pid)
for i in $(seq 30); do grep -q PORT $G/fake/fake.out 2>/dev/null && break; sleep 0.3; done
PORT=$(awk '/PORT/{print $2}' $G/fake/fake.out); say "fake provider port $PORT"
cat >> $H/.bise/config.toml <<TOML
[roles]
main = "fake/mistral-small-latest"
classify = "off"

[providers.fake]
name = "fake"
api = "openai-chat"
base_url = "http://127.0.0.1:$PORT/v1"
key_env = "BISE_TRY_FAKE_KEY"
key_command = "echo fake-key"
small_model = "mistral-small-latest"
TOML
if [ -n "$SMOKE" ]; then
  # the smoke talks to the fake provider only (architect m_29576): every base_url of the world's
  # config on 127.0.0.1, main's role on fake/, and no provider key in the env a LaunchServices open
  # gives the app (launchctl's; this shell's own env never reaches it through open)
  CF=$H/.bise/config.toml
  realurl=$(grep -E '^[[:space:]]*base_url' "$CF" | grep -vE '"http://127\.0\.0\.1:[0-9]+/')
  realmain=$(grep -E '^[[:space:]]*main[[:space:]]*=' "$CF" | grep -vF '"fake/')
  realkey=$(for k in $( { env | sed -n 's/^\([A-Z0-9_]*_API_KEY\)=.*/\1/p'; echo ANTHROPIC_API_KEY OPENAI_API_KEY MISTRAL_API_KEY GEMINI_API_KEY GOOGLE_API_KEY OPENROUTER_API_KEY GROQ_API_KEY DEEPSEEK_API_KEY XAI_API_KEY; } | tr ' ' '\n' | sort -u); do
    [ -n "$(launchctl getenv "$k")" ] && echo "$k"; done)
  [ -n "$realurl$realmain$realkey" ] && { say "refused: the smoke world is not on the fake provider: ${realurl:+urls: $realurl; }${realmain:+main: $realmain; }${realkey:+launchd env: $realkey}"; kill $(cat $G/fake/fake.pid) 2>/dev/null; exit 2; }
  say "fake provider only: $CF on 127.0.0.1:$PORT, no provider key in launchd's env"
fi
# three saved rules, so settings › approvals has a list to walk (d0d45f92's rows; rules.rs's format)
[ -e $H/.bise/approvals.toml ] || cat > $H/.bise/approvals.toml <<TOML
[[allow]]
tool = "bash"
pattern = "cargo test *"

[[allow]]
tool = "bash"
pattern = "pnpm check"

[[allow]]
tool = "bash"
pattern = "ls *"
TOML
# a GitHub of the world's own (e812463c's merge card, his #384): the harness project gets an origin
# on github.com (never pushed: pushurl disabled) and the world's fake gh (fake_gh.py over
# $GHW/prs-*.json) first on the PATH his login shell gives (the world's ~/.zprofile), so the
# candidate's hubs and the app's login env find it before any other gh; the driver writes the PR
# once its place exists (rowsE812). Nothing reaches his gh, his login or the network.
GHW=$G/w/gh-world; export AS_HE_DOES_GH_WORLD=$GHW
HP=$(jq -r '.projects[] | select(.name == "harness") | .path' $H/.bise/projects.json 2>/dev/null | head -1)
if [ -n "$HP" ] && git -C "$HP" rev-parse --git-dir > /dev/null 2>&1; then
  export AS_HE_DOES_HARNESS=$HP
  mkdir -p $GHW/bin
  git -C "$HP" remote remove origin 2>/dev/null
  git -C "$HP" remote add origin https://github.com/qa-world/harness.git
  git -C "$HP" config remote.origin.pushurl DISABLED-in-world
  echo '{"owner": "qa-world", "name": "harness", "prs": []}' > $GHW/prs-harness.json
  printf '#!/bin/sh\n# the world'"'"'s fake gh (dev/plugins/ambient-qa/fake_gh.py): never his gh, his login or the network\nexec %s %s %s "$@"\n' \
    "$P" "$HERE/../fake_gh.py" "$GHW" > $GHW/bin/gh; chmod 755 $GHW/bin/gh
  echo "export PATH=\"$GHW/bin:\$PATH\"" >> $H/.zprofile
  say "world GitHub: harness at $HP, origin $(git -C "$HP" remote get-url origin), gh $GHW/bin/gh"
else
  say "world GitHub: no harness repo in the world ($HP): the merge rows will say so"
fi
log stream --style compact --predicate 'process == "tccd"' > $O/tccd.log 2>&1 & TPID=$!
log stream --level debug --style ndjson --predicate 'process == "tccd" OR subsystem == "com.apple.TCC"' > $O/tcc-debug.ndjson 2>&1 & NPID=$!
touch $O/pids.txt
( while [ ! -e $O/stop ]; do
    o=$(ours 2>/dev/null); [ -n "$o" ] && echo "$o" >> $O/pids.txt
    h=$(for p in $o; do case "$(ps -o command= -p "$p")" in ("$A/Contents/MacOS/bise-mac-helper"*) echo "$p";; esac; done)
    [ -n "$h" ] && { echo "helper $h" > $O/BREACH; SIG=KILL killpids $(sort -u $O/pids.txt); }
    sleep 0.3
  done ) & CPID=$!
$RS tcc --log $O/tccd.log --pids $O/pids.txt --kill-pids --stop-file $O/stop --bundle "$A" --out $O & CCPID=$!
$RS front --out $O/front.jsonl --stop-file $O/stop --bundle "$A" --pids $O/pids.txt & FPID=$!
sleep 1

# first launch: the passes, this cut's rows, then the draft across two restarts (SIGTERM)
say "launch first"; opencand; breach; libgate first-open
if [ "${AS_HE_DOES_ONLY:-}" = quick ]; then
  # a follow-up pass (ambient-lead m_22430): the quick rows, then one real ctrl+r take with the
  # fake voice (AS_HE_DOES_FAKE_VOICE=1), then the usual stop and gates
  drive quick
  if [ -n "${FV:-}" ]; then
    AS_HE_DOES_VOICE=$VOICE NODE_OPTIONS="$TS_NODE" node $HERE/dict1.ts $PORT_CDP $O > $O/driver-dict1.log 2>&1; tail -4 $O/driver-dict1.log; breach
  fi
  L=$(find "$H" -name core.log -newer $O/before.json -not -path "*/.bise/hubs/*" -print0 2>/dev/null | xargs -0 ls -t 2>/dev/null | head -1)
  [ -n "$L" ] && cp "$L" $O/core-first.log
  stopcand; breach; libgate first-stop
  if [ "${AS_HE_DOES_FIRSTRUN:-}" = 1 ]; then
    # the first run's live key check (desk-cut-10, onboard-keys-2): the candidate again on a FRESH
    # home under the world (stopcand stops what names $G/w), no key, no prefs, every provider's
    # base_url on the world's fake provider, the keychain probe off, no browser or link opener;
    # the gates still run. firstrun.ts never goes past the model step.
    WH=$H; H=$G/w/first-$(date +%H%M%S)/h; mkdir -p $H/.bise
    for p in anthropic openai mistral chatgpt openrouter; do printf '[providers.%s]\nbase_url = "http://127.0.0.1:%s/v1"\n\n' $p $PORT; done > $H/.bise/config.toml
    FV=(--env BISE_DETECT_KEYCHAIN=0 --env BISE_OPEN=true --env BISE_BROWSER=true)
    say "first run: fresh home $H"; opencand; breach; libgate firstrun-open
    AS_HE_DOES_FIRST_HOME=$H/.bise NODE_OPTIONS="$TS_NODE" node $HERE/firstrun.ts $PORT_CDP $O > $O/driver-firstrun.log 2>&1; tail -4 $O/driver-firstrun.log; breach
    stopcand; breach; libgate firstrun-stop
    H=$WH
  fi
  wrapup
  say "done (quick)"; exit 0
fi
if [ "${AS_HE_DOES_ONLY:-}" = modes ]; then
  # a port's dev loop (asd-full): only the driver modes named in AS_HE_DOES_MODES, in order, on
  # one launch (e.g. 'cut rows2', 'cut probe'), then the usual stop and gates; never a verdict
  for m in ${AS_HE_DOES_MODES:-cut}; do drive $m; done
  stopcand; breach; libgate first-stop
  wrapup
  say "done (modes: ${AS_HE_DOES_MODES:-cut})"; exit 0
fi
if [ -n "$SMOKE" ]; then
  # the smoke (fast-try p3): the driver's prelude (pill, banner, sidebar), the core up, one message
  # and its answer; then the clean quit (stopcand: 0 left or a breach) and the closing gates
  AS_HE_DOES_ANSWER_MS=${AS_HE_DOES_ANSWER_MS:-90000} AS_HE_DOES_WATCHDOG_MS=${AS_HE_DOES_WATCHDOG_MS:-180000} drive smoke
  stopcand; breach; libgate smoke-stop
  wrapup
  # his app: the same pids before and after, checked (an automatic verdict, review-gates m_29603)
  his0=$(sed -n '1s/^[0-9:]* his app before: //p' $O/his.txt); his1=$(hispids)
  [ "$his0" = "$his1" ] || { echo "his app's pids changed: before '$his0', after '$his1'" > $O/BREACH; say "BREACH: $(cat $O/BREACH)"; exit 3; }
  # green = the driver ended by itself (rc 0) AND smoke.json holds every step of the pass, each ok:
  # a driver that died between steps leaves only its green steps written (review-gates m_29603)
  rc=$(cat $O/driver-smoke.rc 2>/dev/null)
  red=$( { [ "$rc" = 0 ] || echo "driver exit ${rc:-none}"
    [ -s $O/smoke.json ] || echo "no smoke.json"
    jq -r '.steps[] | select(.ok != true) | .name' $O/smoke.json 2>/dev/null
    for s in 'the status pill sits in the title bar; nothing moves under it (5 s from first draw)' \
      "no 'didn't start right' banner" 'sidebar' 'the core comes up: the core line goes, no ▲ line' \
      "a message to a project's main gets its answer"; do
      jq -e --arg s "$s" 'any(.steps[]; .name == $s)' $O/smoke.json > /dev/null 2>&1 || echo "missing: $s"
    done; } )
  t=$(( $(date +%s) - T0 ))
  jq -n --arg app "$A" --argjson s $t --arg red "$red" '{mode: "smoke", app: $app, seconds: $s, green: ($red == ""), red: ($red | split("\n") | map(select(. != "")))}' > $O/smoke-verdict.json
  if [ -n "$red" ]; then say "smoke RED in ${t} s: $(echo "$red" | tr '\n' ';')"; exit 1; fi
  say "smoke green in ${t} s"; exit 0
fi
drive first
drive cut
drive rows2
hubsnow "after rows2"
for c in pause atonce; do
  TAG=$c drive draft-type "draft kept across a restart ($c)"
  [ $c = pause ] && sleep 5
  m=$(mainpid)
  [ -z "$m" ] && { say "CANDIDATE GONE: its main process ended before the restart $c"; echo "restart-$c" >> $O/candidate-gone.txt; }
  say "restart $c (SIGTERM to ${m:-nothing}, then open)"; t0=$(date '+%Y-%m-%d %H:%M:%S')
  stopcand; breach; libgate restart-$c-stop
  opencand; breach; libgate restart-$c-open
  TAG=$c drive draft-check "draft kept across a restart ($c)"
done
L=$(find "$H" -name core.log -newer $O/before.json -not -path "*/.bise/hubs/*" -print0 2>/dev/null | xargs -0 ls -t 2>/dev/null | head -1)
if [ -n "$L" ]; then say "core.log at $L: $(wc -c < "$L") bytes, repl-live not found: $(grep -c 'repl-live not found' "$L")"; cp "$L" $O/core-first.log
else say "core.log: none under $H"; fi
# whether the world held a signed thought at all (else the BENDSIG row proves only the plain case)
say "signed thoughts in the world's hubs: $(grep -rl 'BENDSIG::' $H/.bise/hubs 2>/dev/null | wc -l | tr -d ' ') files"
say "hubs: $(ls $H/.bise/hubs/ | tr '\n' ' ')"
stopcand; breach; libgate first-stop

say "launch second"; opencand; breach; libgate second-open
drive second
stopcand; breach; libgate second-stop

touch $O/stop; wait $FPID $CCPID; kill $TPID $NPID $CPID 2>/dev/null
kill $(cat $G/fake/fake.pid) 2>/dev/null
say "realshape core with no env"
$RS core --bundle "$A" --out $O/core-noenv 2>&1 | tail -10
w=$(ps -axo pid=,command= | grep -F "$G/w" | grep -v grep | awk '{print $1}'); [ -n "$w" ] && killpids $w
candprefs
libgate final
[ -s $O/hubs-steps.jsonl ] && jq -s '{mode: "hubs-outside", steps: .}' $O/hubs-steps.jsonl > $O/hubs-outside.json
say "his app after: $(hispids)" | tee -a $O/his.txt
say "done"
