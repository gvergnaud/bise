#!/usr/bin/env bash
# The desktop app's full end-to-end (fast-try p7, architect m_29601): what only the real packed
# app shows, run at night and before a desktop release, never between Gabriel and a try (his try
# path: a candidate, the selected checks and the smoke; docs/desktop-try.md).
#   scripts/nightly_desktop.sh <out dir> [<sha>] [--until HH:MM|@<epoch>]
# <sha>: default HEAD of this tree. The nightly (scripts/nightly_gate.sh --in-slot) runs it as one
# night_step, in the slot it holds; desktop_try and desktop_dmg nest there at nice 10 (heavy_argv),
# never a second slot. Its steps, by value (a night cut short keeps the most useful part):
#   candidate  scripts/desktop-try.sh --candidate <sha>: the packed 'bise cand' app, then
#              scripts/tray_badge_check.sh on it (the tray's dot addon, signed and universal;
#              its red: desktop/tray-badge)
#   quick      every quick as-he-does row (no QUICK_ROWS), the first run (fresh home, the model
#              step) and one fake-voice ctrl+r take: cand.sh with AS_HE_DOES_ONLY=quick
#   land       apps/desktop pnpm build + pnpm test:land; at a release (NIGHTLY_DESKTOP_LAND_SPECS=checks,
#              set by scripts/desktop_tag.py) the build only: test:land's 3 specs are desktop checks
#              too, and desktop_tag runs every desktop check on that sha and judges them (release-speed
#              plan A step 4, architect m_62929: 1.7 min a pass, the same specs twice)
#   mcp-apps   tests/mcp_apps_hostile_e2e.py (this tree's bise built first): the hostile MCP App
#              through a throwaway hub's web host in Chromium, every probe blocked (docs/mcp-apps.md
#              C6, risk 1; owner mcp-apps). Its red, desktop/mcp-apps-hostile, is a leak:
#              tests/known_reds.py refuses it, so a tag never goes through it (architect m_50511)
#   full       the full as-he-does pass: cand.sh (first, cut, rows2, two restarts, second, the
#              hubs outside the bundle, the core with no env)
#   dmg        scripts/desktop-dmg.sh <sha>: that candidate repacked as 'bise', the DMG mounted
#              read-only and checked (never launched)
# Each cand.sh run is on its own throwaway world (~/.bise/gate/real-shape, fake provider and
# accounts) with its gates A/B/C; a breach is a red. A step is green, red, or none (no result:
# not run, with why). --until: nothing new starts at or after that local time, or that epoch (the
# nightly passes its own end, @<06:00's epoch>, so a run started late starts nothing); what didn't
# run is none, never green. On battery: nothing runs (none: on battery).
# The reds: each ok:false step of the cand.sh JSONs, named 'as-he-does/<mode>: <step name>' (digits
# as N: a card's number changes each run), plus the steps' own reds, split by tests/known_reds.py
# (known with its owner, else NEW). A known red is still red.
# Out: <out>/<step>.log, <out>/quick/ and <out>/full/ (cand.sh's dirs), <out>/summary.txt, and
# what a tag reads (scripts/desktop_tag.py): <out>/reds.txt (each red's exact name) and
# <out>/steps.tsv (each step's state). Each run's line in <BISE_HOME>/run/nightly/desktop-runs.tsv:
# <when> <sha> green|red|none <out dir>. Exit 0 green, 1 red ('FAIL <step>' lines), 2 no result; a second run in a row with no result is red (desktop-runs.tsv keeps each
# run's verdict; the summary names the last green run). The candidate dir it built is kept for a rerun
# (the newest 2; under 20 GB free it is removed at the end); one built before (another agent's,
# desktop_try says "is built") is kept.
# Tests: tests/nightly_desktop_test.py (fake desktop-try, cand.sh, dmg and pnpm: NIGHTLY_DESKTOP_*).
nightly_desktop() {
set -uo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
out="" sha="" until=""
while [ $# -gt 0 ]; do
  case "$1" in
    --until) until="${2:-}"; shift 2 ;;
    *) if [ -z "$out" ]; then out="$1"; elif [ -z "$sha" ]; then sha="$1"; else echo "usage: nightly_desktop.sh <out dir> [<sha>] [--until HH:MM|@<epoch>]" >&2; return 2; fi; shift ;;
  esac
done
[ -n "$out" ] || { echo "usage: nightly_desktop.sh <out dir> [<sha>] [--until HH:MM|@<epoch>]" >&2; return 2; }
mkdir -p "$out" && out="$(cd "$out" && pwd)"
sha="$(git -C "$here" rev-parse --verify -q "${sha:-HEAD}^{commit}")" || { echo "no commit ${sha:-HEAD} in $here" >&2; return 2; }
bh="${BISE_HOME:-$HOME/.bise}"
try_sh="${NIGHTLY_DESKTOP_TRY:-$here/scripts/desktop-try.sh}"
hostile_sh="${NIGHTLY_DESKTOP_HOSTILE:-}"
dmg_sh="${NIGHTLY_DESKTOP_DMG:-$here/scripts/desktop-dmg.sh}"
badge_sh="${NIGHTLY_DESKTOP_BADGE:-$here/scripts/tray_badge_check.sh}"
cand_sh="${NIGHTLY_DESKTOP_CAND:-$here/dev/plugins/ambient-qa/as-he-does/cand.sh}"
pnpm="${NIGHTLY_DESKTOP_PNPM:-pnpm}"
land_specs="${NIGHTLY_DESKTOP_LAND_SPECS:-run}"
sum="$out/summary.txt"
: >"$sum"
reds=()
state_of=()
note() { echo "$*" | tee -a "$sum"; }
# <name> green|red|none <why>
record() { state_of+=("$1=$2"); note "STEP $1: $2${3:+ ($3)}"; [ "$2" = red ] && echo "FAIL $1"; return 0; }

# the hard end: an epoch, the next HH:MM from the start (a 23:00 start ends tomorrow's 08:00), or
# @<epoch> as is (the nightly's own end, already past when its slots came late: nothing starts)
# (NIGHTLY_DESKTOP_NOW: a file holding the epoch, the test's clock)
now() { if [ -n "${NIGHTLY_DESKTOP_NOW:-}" ]; then cat "$NIGHTLY_DESKTOP_NOW"; else date +%s; fi; }
end=""
case "$until" in
  "") ;;
  @*) end="${until#@}"; [[ "$end" =~ ^[0-9]+$ ]] || { echo "--until wants HH:MM or @<epoch>, got $until" >&2; return 2; }
      until="$(date -r "$end" +%H:%M)" ;;
  *) end="$(date -j -f "%Y-%m-%d %H:%M:%S" "$(date -r "$(now)" +%F) $until:00" +%s 2>/dev/null)" || { echo "--until wants HH:MM or @<epoch>, got $until" >&2; return 2; }
     # tomorrow's HH:MM by the calendar, not + 86400 (25 h on a DST night, 2026-10-25)
     [ "$end" -gt "$(now)" ] || end="$(date -j -f "%Y-%m-%d %H:%M:%S" "$(date -j -r "$(now)" -v+1d +%F) $until:00" +%s)" ;;
esac
late() { [ -n "$end" ] && [ "$(now)" -ge "$end" ]; }
on_battery() { [ -n "${NIGHTLY_DESKTOP_BATTERY:-}" ] && { [ "$NIGHTLY_DESKTOP_BATTERY" = 1 ]; return; }; /usr/bin/pmset -g batt 2>/dev/null | head -1 | grep -q "Battery Power"; }

note "nightly desktop: $sha in $here, $(date)${until:+, nothing new after $until}"
steps=(candidate quick land mcp-apps full dmg)
if on_battery; then
  for s in "${steps[@]}"; do record "$s" none "on battery"; done
  verdict; return $?
fi

# the reds of one cand.sh dir: every ok:false step of its step JSONs
cand_reds() { # <mode> <dir>
  python3 -B - "$1" "$2" <<'PY'
import glob, json, os, re, sys
mode, d = sys.argv[1], sys.argv[2]
for f in sorted(glob.glob(os.path.join(d, "*.json"))):
    try:
        j = json.load(open(f))
    except (OSError, ValueError):
        continue
    if not isinstance(j, dict) or not isinstance(j.get("steps"), list):
        continue
    for s in j["steps"]:
        if isinstance(s, dict) and s.get("ok") is False:
            name = re.sub(r"\s+", " ", re.sub(r"\d+", "N", str(s.get("name", "?")))).strip()[:160]
            print("as-he-does/%s: %s" % (mode, name))
PY
}
# test:land's red checks, from land.mts' "known reds (tests/known_reds.txt): a (owner, since x), …"
# and "NEW reds (not in tests/known_reds.txt): b, c" lines: one name per line
land_reds() { # <land.log>
  python3 -B - "$1" <<'PY'
import re, sys
for line in open(sys.argv[1], errors="replace"):
    m = re.match(r"^(known reds \(tests/known_reds.txt\)|NEW reds \(not in tests/known_reds.txt\)): (.*)$", line.strip())
    if m:
        for part in re.sub(r"\([^)]*\)", "", m.group(2)).split(","):
            if part.strip():
                print(part.strip())
PY
}
# cand.sh <mode> <dir> [env...]: its rc, its BREACH, its red steps
as_he_does() {
  local mode=$1 d="$out/$1"; shift
  rm -rf "$d"
  # timing-bound: in the release's quiet window (heavy-run quiet, main m_63082), when installed
  local hq="$HOME/.bise/run/heavy-run" q=()
  [ -x "$hq" ] && [ -z "${NIGHTLY_DESKTOP_CAND:-}" ] && q=("$hq" quiet as-he-does --)
  ${q[@]+"${q[@]}"} env "$@" bash "$cand_sh" "$d" "$app" >"$out/$mode.log" 2>&1
  local rc=$? r n=0
  while IFS= read -r r; do [ -n "$r" ] && { reds+=("$r"); n=$((n + 1)); }; done < <(cand_reds "$mode" "$d")
  if [ -e "$d/BREACH" ]; then reds+=("as-he-does/$mode: gate breach"); record "$mode" red "gate breach: $(head -1 "$d/BREACH")"
  elif [ $rc != 0 ]; then reds+=("as-he-does/$mode: cand.sh exit $rc"); record "$mode" red "cand.sh exit $rc, $n red steps, $out/$mode.log"
  elif [ $n -gt 0 ]; then record "$mode" red "$n red steps"
  else record "$mode" green; fi
}

# apps/desktop's node_modules, once (a fresh nightly tree has none); pnpm 10 skips Electron's
# postinstall (tests/harness/README.md 'Electron'): the harness then takes the shared patched one,
# ~/.bise/dev/electron-dist/v<version>-lsui (night1: every test:land check 'no Electron at …')
deps() {
  [ -d "$here/apps/desktop/node_modules/@playwright/test" ] || ( cd "$here/apps/desktop" && "$pnpm" install --frozen-lockfile --prefer-offline ) || return 1
  local v shared
  if [ -z "${BISE_HARNESS_ELECTRON_DIST:-}" ] && [ ! -d "$here/apps/desktop/node_modules/electron/dist/Electron.app" ]; then
    v="$(jq -r .version "$here/apps/desktop/node_modules/electron/package.json" 2>/dev/null)"
    shared="$HOME/.bise/dev/electron-dist/v$v-lsui"
    [ -d "$shared/Electron.app" ] || { echo "no Electron: node_modules/electron/dist is empty and $shared has none"; return 1; }
    export BISE_HARNESS_ELECTRON_DIST="$shared"
  fi
}

app="" cdir="" made=""
for s in "${steps[@]}"; do
  if late; then record "$s" none "past $until"; continue; fi
  case $s in
    candidate)
      bash "$try_sh" --candidate "$sha" >"$out/candidate.log" 2>&1; rc=$?
      j="$(tail -1 "$out/candidate.log")"
      app="$(printf '%s' "$j" | jq -r '.app // empty' 2>/dev/null)"
      cdir="$(printf '%s' "$j" | jq -r '.dir // empty' 2>/dev/null)"
      # built before this run (desktop_try's 'candidate <sha8> is built'): someone else's, kept
      grep -q "candidate ${sha:0:8} is built" "$out/candidate.log" || made="$cdir"
      if [ $rc = 0 ] && [ -d "$app" ]; then
        # the tray's wait-dot addon packed whole (else his bar falls back to the title dot)
        if why="$(bash "$badge_sh" "$app" 2>&1)"; then record candidate green "$app"
        else reds+=("desktop/tray-badge"); record candidate red "$why"; fi
      else reds+=("desktop/candidate"); record candidate red "desktop-try.sh --candidate exit $rc, $out/candidate.log"; app=""; fi ;;
    quick|full)
      if [ -z "$app" ]; then record "$s" none "no candidate"; continue; fi
      # the driver's @playwright/test (cand.sh's PW_FROM: this tree's apps/desktop)
      if ! deps >"$out/deps.log" 2>&1; then reds+=("desktop/deps"); record "$s" red "pnpm install failed, $out/deps.log"; continue; fi
      if [ $s = quick ]; then as_he_does quick AS_HE_DOES_ONLY=quick AS_HE_DOES_FIRSTRUN=1 AS_HE_DOES_FAKE_VOICE=1
      else as_he_does full; fi ;;
    land)
      if [ "$land_specs" = checks ]; then
        # one app build per sha (release-speed plan A step 5, architect m_63479): the candidate's vite
        # build of this sha in the same mode proves what this build-only check proves; its key
        # (desktop_try.build_key) must match this tree's, never a dirty one
        ck="$(jq -r '.build_key // empty' "$cdir/cand.json" 2>/dev/null)"
        if [ -n "$app" ] && [ -n "$ck" ] && [ "$ck" = "$(cd "$here" && python3 -B scripts/build_key.py "$sha" 2>/dev/null)" ]; then
          record land green "built once: the candidate's build of this sha ($ck); test:land's specs run in the release's desktop checks"
          continue
        fi
        { deps && ( cd "$here/apps/desktop" && "$pnpm" build ); } >"$out/land.log" 2>&1; rc=$?
        if [ $rc = 0 ]; then record land green "pnpm build; test:land's specs run in the release's desktop checks"
        else reds+=("desktop/build"); record land red "pnpm build exit $rc, $out/land.log"; fi
        continue
      fi
      { deps && ( cd "$here/apps/desktop" && "$pnpm" build && "$pnpm" test:land ); } >"$out/land.log" 2>&1; rc=$?
      if [ $rc = 0 ]; then record land green
      else
        # its reds by name (land.mts' known/NEW lines: each a desktop check's spec name), so a tag
        # can match them to known_reds; no such line (it didn't run its checks): one unknown red
        n=0
        while IFS= read -r r; do [ -n "$r" ] && { reds+=("$r"); n=$((n + 1)); }; done < <(land_reds "$out/land.log")
        [ $n = 0 ] && reds+=("desktop/test:land")
        record land red "exit $rc; $(grep -E '^(known reds|NEW reds)' "$out/land.log" | tr '\n' ' ')$out/land.log"
      fi ;;
    mcp-apps)
      # the hub's binaries of this tree (run_all's), then the check (NIGHTLY_DESKTOP_HOSTILE: the
      # test's fake, the whole step)
      if [ -n "$hostile_sh" ]; then bash "$hostile_sh" >"$out/mcp-apps.log" 2>&1
      else
        { deps && ( cd "$here" && scripts/bins.sh repl-live repl-scripted sb-core && scripts/bins.sh path bend-jsrt >/dev/null \
            && ( cd rust && cargo build --offline -q ) && python3 -u tests/mcp_apps_hostile_e2e.py ); } >"$out/mcp-apps.log" 2>&1
      fi
      rc=$?
      if [ $rc = 0 ]; then record mcp-apps green
      else reds+=("desktop/mcp-apps-hostile"); record mcp-apps red "exit $rc, $(grep -E '^FAIL' "$out/mcp-apps.log" | tail -1 | cut -c1-200), $out/mcp-apps.log"; fi ;;
    dmg)
      if [ -z "$app" ]; then record dmg none "no candidate"; continue; fi
      bash "$dmg_sh" "$sha" >"$out/dmg.log" 2>&1; rc=$?
      if [ $rc = 0 ]; then record dmg green "$(tail -1 "$out/dmg.log")"
      elif grep -q "no stable signing identity" "$out/dmg.log"; then record dmg none "no signing identity on this Mac"
      else reds+=("desktop/dmg"); record dmg red "desktop-dmg.sh exit $rc: $(grep 'bise dmg:' "$out/dmg.log" | tail -1)"; fi ;;
  esac
done

# the candidate this run built stays (fast-loop, his « 4 min max »): a rerun of named rows on it
# (scripts/desktop_rerun.py) opens it at once instead of a ~10 min rebuild. The nightly lists each
# candidate it built in <BISE_HOME>/run/nightly/cands.txt and prunes only from that list (review-gates
# m_35943: never another agent's, promote-lock's incident): the newest NIGHTLY_DESKTOP_KEEP_CANDS (2)
# stay, an older one goes unless a cand.sh run holds its lock or a process runs from it
# (cand_lock.users). Under NIGHTLY_DESKTOP_KEEP_GB (20) GB free the night's own goes too, as before
# (heavy-run waits under 15). NIGHTLY_DESKTOP_FREE_GB: the free GB the test fakes.
free_gb() { [ -n "${NIGHTLY_DESKTOP_FREE_GB:-}" ] && { echo "$NIGHTLY_DESKTOP_FREE_GB"; return; }; df -k "$bh" 2>/dev/null | awk 'NR==2 {print int($4 / 1048576)}'; }
cand_busy() {  # why a candidate dir must stay: a cand.sh run holds its lock, or a process runs from it
  local a l u; for a in "$1"/dist/*/*.app; do
    [ -d "$a" ] || continue
    l="$HOME/.bise/gate/locks/cand-$(printf '%s' "$(cd "$a" && pwd -P)" | shasum -a 256 | cut -c1-16).lock"
    # -k: the probe never unlinks the lock file (a cand.sh starting now keeps the same inode)
    [ -e "$l" ] && ! /usr/bin/lockf -k -t 0 "$l" /usr/bin/true 2>/dev/null && { echo "a cand.sh run holds it ($(cat "$l.holder" 2>/dev/null))"; return 0; }
  done
  u="$(python3 -B -c 'import sys; sys.path.insert(0, sys.argv[1]); import cand_lock; print(" ".join(str(p) for p, _ in cand_lock.users(sys.argv[2])))' "$here/scripts" "$1" 2>/dev/null)" || { echo "who runs from it is unknown (cand_lock.py failed)"; return 0; }
  [ -n "$u" ] && { echo "pid $u runs from it"; return 0; }
  return 1
}
if [ -n "$made" ] && [ -d "$made" ]; then
  built="$bh/run/nightly/cands.txt"; mkdir -p "$(dirname "$built")"
  if [ "$(free_gb)" -lt "${NIGHTLY_DESKTOP_KEEP_GB:-20}" ]; then
    rm -rf "$made" && git -C "$here" worktree prune 2>/dev/null
    note "removed the night's candidate $made ($(free_gb) GB free, under ${NIGHTLY_DESKTOP_KEEP_GB:-20})"
  else
    { grep -vxF "$made" "$built" 2>/dev/null; echo "$made"; } > "$built.tmp" && mv "$built.tmp" "$built"
    note "kept the night's candidate $made (a rerun opens it: scripts/desktop_rerun.py $out)"
    # the typed field disk_reclaim.py reads to keep it (architect m_61530: never the prose above)
    echo "kept_candidate=$(basename "$made")" >> "$sum"
  fi
  # the older ones the nightly built, newest last in cands.txt
  if [ -f "$built" ]; then
    keep=() n=0
    while IFS= read -r d; do
      [ -n "$d" ] && [ -d "$d" ] || continue
      n=$((n + 1))
      if [ $n -le "${NIGHTLY_DESKTOP_KEEP_CANDS:-2}" ]; then keep+=("$d"); continue; fi
      if why="$(cand_busy "$d")"; then keep+=("$d"); note "kept the older candidate $d: $why"; continue; fi
      rm -rf "$d" && note "removed the older candidate $d"
    done < <(tail -r "$built")
    printf '%s\n' "${keep[@]+"${keep[@]}"}" | tail -r | sed '/^$/d' > "$built.tmp" && mv "$built.tmp" "$built"
  fi
  git -C "$here" worktree prune 2>/dev/null
fi

[ ${#reds[@]} -gt 0 ] && python3 -B "$here/tests/known_reds.py" split "${reds[@]}" | tee -a "$sum"
# what a tag reads (scripts/desktop_tag.py): each red's exact name, each step's state
printf '%s\n' "${reds[@]+"${reds[@]}"}" | sed '/^$/d' >"$out/reds.txt"
printf '%s\n' "${state_of[@]}" | tr '=' '\t' >"$out/steps.tsv"
verdict
}

# the run's verdict, kept in desktop-runs.tsv (<date>T<time>, sha, green|red|none, out dir): two runs in a
# row with no result (battery, past --until) is a red of its own, owner gate-nightly (review-gates
# m_29770): the rows must not silently never run. The summary names the last green run.
verdict() {
  local v=green hist="$bh/run/nightly/desktop-runs.tsv" prev last
  case " ${state_of[*]} " in *=red*) v=red ;; *=none*) v=none ;; esac
  mkdir -p "$bh/run/nightly"
  prev="$(tail -1 "$hist" 2>/dev/null | cut -f3)"
  last="$(awk -F'\t' '$3 == "green" { l = $1 " on " $2 } END { print l }' "$hist" 2>/dev/null)"
  printf '%s\t%s\t%s\t%s\n' "$(date +%FT%T)" "$sha" "$v" "$out" >>"$hist"
  note "last green run: ${last:-none yet}"
  case $v in
    red) note "nightly desktop: red"; return 1 ;;
    none)
      if [ "$prev" = none ]; then
        record desktop-runs red "a second run in a row with no result: the end-to-end hasn't run since ${last:-ever} (owner gate-nightly)"
        note "nightly desktop: red (no result twice)"; return 1
      fi
      note "nightly desktop: no result for some steps"; return 2 ;;
  esac
  note "nightly desktop: green"
  return 0
}
nightly_desktop "$@"; exit $?
