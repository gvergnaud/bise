#!/usr/bin/env bash
# The shadow of the desktop checks' selection (fast-try p2, review-gates m_29412 #8): what
# tests/test_map.py desktop-pick would pick for this land or try, said in one line and logged to
# <gate dir>/desktop-pick.jsonl for the nightly audit. It never fails its caller and never holds
# it: at most DESKTOP_PICK_TIMEOUT seconds (default 20), exit 0 whatever happens. Everything still
# runs: the pick only starts skipping on review-gates' go (<gate dir>/desktop-pick.on).
#   tests/desktop_pick_shadow.sh land|try <from> [<to>]     (no <to>: the working tree)
# Tests: tests/desktop_pick_test.py (a crash, a hang: still exit 0).
kind="${1:-}" from="${2:-}" to="${3:-}"
[ -n "$kind" ] && [ -n "$from" ] || { echo "desktop-pick (shadow): usage: desktop_pick_shadow.sh land|try <from> [<to>]"; exit 0; }
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$(mktemp -t desktop-pick.XXXXXX)" || { echo "desktop-pick (shadow): no temp file"; exit 0; }
bound="${DESKTOP_PICK_TIMEOUT:-20}"
# The pick runs in its own process group (set -m), so the bound kills it and whatever it started.
# One timer measures the bound: a loop of `sleep 0.1` polls counted forks, not seconds, and under
# load (50+) each fork took seconds, so a 1 s bound held 25-44 s (pick-timeout, m_54546).
set -m
${DESKTOP_PICK_PY:-python3} "$root/tests/test_map.py" desktop-pick --from "$from" ${to:+--to "$to"} --log "$kind" >"$out" 2>/dev/null </dev/null &
pid=$!
# DESKTOP_PICK_SLEEP: the timer's clock (default sleep); its test passes one whose bound is already
# over, so it checks the kill without a wall-clock bound (6 quick gates red at load on 'secs < 10')
( ${DESKTOP_PICK_SLEEP:-sleep} "$bound" & s=$!; trap 'kill "$s" 2>/dev/null; exit 0' TERM; wait "$s"
  : >"$out.late"; kill -TERM -- -"$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null
  sleep 1; kill -KILL -- -"$pid" 2>/dev/null ) >/dev/null 2>&1 </dev/null &
timer=$!
set +m
wait "$pid"; rc=$?
kill -TERM -- -"$timer" 2>/dev/null || kill -TERM "$timer" 2>/dev/null
if [ -e "$out.late" ] && [ "$rc" -ne 0 ]; then
  echo "desktop-pick (shadow): no answer in $bound s, nothing logged"
elif [ "$rc" -eq 0 ]; then
  first="$(head -1 "$out")"
  case "$first" in
    ALL*) echo "desktop-pick (shadow, all still runs): ALL (${first#ALL })" ;;
    PICK*) echo "desktop-pick (shadow, all still runs): ${first#PICK } checks would run" ;;
    *) echo "desktop-pick (shadow): no answer" ;;
  esac
else
  echo "desktop-pick (shadow): tests/test_map.py desktop-pick failed, nothing logged"
fi
rm -f "$out" "$out.late"
exit 0
