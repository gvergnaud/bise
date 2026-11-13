#!/usr/bin/env bash
# release_cut.sh: the checked part of a continuous release (docs/release-cadence.md), in two
# steps with the app's notes written between them. Never pushes, never publishes.
#   scripts/release_cut.sh gate <keep dir>
#       main's tip gate (scripts/nightly_gate.sh --tip, its own heavy-run slot); the gate's logs
#       copied to <keep dir> at once (other gates overwrite them). Green: prints 'green <sha>', the
#       sha of the green-shas.tsv line it wrote (main's tip when its slot opened, maybe newer than
#       the wake's): the release is cut on THAT sha. Exit 0 green, 1 red or no new green line.
#   scripts/release_cut.sh tag <sha> <desktop-vYYYY.M.N> <notes file>
#       scripts/desktop_tag.py in a clean detached worktree (RELEASE_TREE, default
#       ~/.cache/bise-release/tree: outside ~/.bise, ~/bise and ~/Library, which the app's checks
#       refuse or watch), no .pyc written; the tag signed through Secretive (SSH_AUTH_SOCK, as git
#       tag.gpgsign is on), never pushed. A rerun on the same sha reuses desktop-runs.tsv and
#       checks-verdicts.tsv and only tags. Exit: desktop_tag.py's.
#   scripts/release_cut.sh pass <sha> <desktop-vYYYY.M.N> <notes file>
#       both at once on a pinned sha (release-speed plan A step 7, architect m_63312, marketing
#       m_63313): lane A the tip gate of THAT sha (nightly_gate.sh --tip --sha), lane B the app
#       side (desktop_tag.py --checks-only in the clean tree, as 'tag'), each in heavy-run's
#       release lane (HEAVY_PASS_LANE: slot 6 for the second lane; one timing-bound step at a time
#       across both, and no new heavy step of one lane during the other's). Lane A red stops lane
#       B at once (its process group: its quiet lock goes with it). Both green: tip-green is
#       checked just before the tag, then 'tag' (it reads the verdicts lane B wrote and tags).
#       Under RELEASE_PASS_DISK_GB (40) free (a cold 2-lane pass writes ~30 GB), lane B starts
#       after lane A. Its logs, and one 'lane=A|B' row per lane in timings.tsv (start = its time
#       minus its seconds, the load at its start): <BISE_HOME>/run/nightly/pass-<tag>-<time>/.
#       Exit 0 tagged, 1 red or refused, 2 bad use.
# Tested by hand on a cut (it wraps the gates and their slots); its decisions are in
# scripts/release_slot.py (tests/release_slot_test.py).
set -uo pipefail
R="$(cd "$(dirname "$0")/.." && pwd)"
BH="${BISE_HOME:-$HOME/.bise}"
SECRETIVE="$HOME/Library/Containers/com.maxgoedjen.Secretive.SecretAgent/Data/socket.ssh"

case "${1:-}" in
gate)
  [ $# = 2 ] || { sed -n '2,20p' "$0"; exit 2; }
  keep="$2"; mkdir -p "$keep"
  t0=$(date +%s)
  "$R/scripts/nightly_gate.sh" --tip; rc=$?
  log=$(ls -t "$BH"/gate/logs/full-* 2>/dev/null | head -1)
  [ -n "$log" ] && cp "$log" "$keep/"
  cp "$BH"/gate/sb-gate-*.log "$keep/" 2>/dev/null
  echo "tip gate rc=$rc, logs kept in $keep"
  [ $rc = 0 ] || exit 1
  line=$(tail -1 "$BH/gate/green-shas.tsv"); sha=$(cut -f1 <<<"$line"); when=$(cut -f2 <<<"$line")
  [ -n "$when" ] && [ "$when" -ge "$t0" ] || { echo "no new green line since the gate started"; exit 1; }
  echo "green $sha"
  ;;
tag|pass)
  [ $# = 4 ] || { sed -n '2,32p' "$0"; exit 2; }
  what="$1" sha="$2"; tag="$3"; notes="$(cd "$(dirname "$4")" && pwd)/$(basename "$4")"
  [ -s "$notes" ] || { echo "no notes in $4: write the app's notes first (the tag's body)"; exit 2; }
  if [ "$what" = pass ]; then
    sha="$(git -C "$R" rev-parse --verify -q "$sha^{commit}")" || { echo "no commit '$2'"; exit 2; }
    # the full check after a by-reuse tag went red: no cut until it is owned (marketing m_63313 c)
    if [ -s "$BH/run/nightly/release-red" ]; then
      echo "no cut: the full check after the last by-reuse tag is red, not owned yet ($BH/run/nightly/release-red):"
      cat "$BH/run/nightly/release-red"; exit 1
    fi
  fi
  tree="${RELEASE_TREE:-$HOME/.cache/bise-release/tree}"
  if [ ! -e "$tree/.git" ]; then
    mkdir -p "$(dirname "$tree")"
    git -C "$R" worktree add -q --detach "$tree" "$sha" || exit 2
  fi
  git -C "$tree" checkout -q --detach "$sha" || exit 2
  [ -z "$(git -C "$tree" status --porcelain)" ] || { echo "$tree is not clean"; exit 2; }
  find "$tree" -name __pycache__ -type d -prune -exec rm -rf {} +
  [ -S "$SECRETIVE" ] && export SSH_AUTH_SOCK="$SECRETIVE"
  tagger=(env PYTHONDONTWRITEBYTECODE=1 python3 scripts/desktop_tag.py "$tag" "$sha" --notes-file "$notes")
  if [ "$what" = pass ]; then
    keep="$BH/run/nightly/pass-$tag-$(date +%Y%m%d-%H%M%S)"; mkdir -p "$keep" || exit 2
    timings="${SB_TIMINGS:-$BH/gate/timings.tsv}"
    load() { /usr/sbin/sysctl -n vm.loadavg 2>/dev/null | awk '{print $2}'; }
    # one row per lane: <end> <sha> lane=<L> <rc> <secs> 0 <load at its start> release-pass
    row() { printf '%s\t%s\tlane=%s\t%s\t%s\t0\t%s\trelease-pass\n' "$(date +%s)" "$sha" "$1" "$2" "$(( $(date +%s) - $3 ))" "$4" >>"$timings"; }
    free=$(df -g "${HEAVY_DISK_VOLUME:-$HOME}" 2>/dev/null | awk 'NR == 2 {print $4}')
    floor="${RELEASE_PASS_DISK_GB:-40}"
    echo "release pass $tag on $sha: lane A the tip gate, lane B the app side; logs in $keep"
    # a recut one fix from a full tip run R (plan A step 8, scripts/release_reuse.py): lane A keeps
    # R's green rows where test_map vouches (gate.sh asks again in its tree), lane B R's green app
    # verdicts when the change reaches neither the app nor its checks
    rbase="$(cd "$R" && RELEASE_REUSE_REPO="$R" python3 -B scripts/release_reuse.py base "$sha" 2>/dev/null)"
    envA=() envB=()
    if [ -n "$rbase" ]; then
      envA=(GATE_REUSE_FROM="$rbase")
      app="$(cd "$R" && RELEASE_REUSE_REPO="$R" python3 -B scripts/release_reuse.py app "$rbase" "$sha" 2>&1)"
      case "$app" in "REUSE "*) envB=(DESKTOP_TAG_REUSE="$rbase") ;; esac
      echo "a recut of ${rbase:0:12}: lane A keeps its green rows where test_map vouches; lane B: $app"
    fi
    # each lane in its own process group (set -m): lane A red stops lane B's whole tree
    set -m
    ta=$(date +%s) la=$(load)
    env ${envA[@]+"${envA[@]}"} HEAVY_PASS_LANE=A "$R/scripts/nightly_gate.sh" --tip --sha "$sha" >"$keep/lane-A.out" 2>&1 & pa=$!
    ra=""
    if [ -n "$free" ] && [ "$free" -lt "$floor" ]; then
      echo "only $free GB free (a 2-lane pass wants $floor, RELEASE_PASS_DISK_GB): lane B starts after lane A"
      wait "$pa"; ra=$?; row A "$ra" "$ta" "$la"
    fi
    if [ -z "$ra" ] || [ "$ra" = 0 ]; then
      tb=$(date +%s) lb=$(load)
      (cd "$tree" && exec env ${envB[@]+"${envB[@]}"} HEAVY_PASS_LANE=B "${tagger[@]}" --checks-only) >"$keep/lane-B.out" 2>&1 & pb=$!
    fi
    if [ -z "$ra" ]; then wait "$pa"; ra=$?; row A "$ra" "$ta" "$la"; fi
    tl=$(tail -1 "$BH/run/nightly/tip-last" 2>/dev/null | cut -d' ' -f4)
    [ -n "$tl" ] && [ -f "$tl" ] && cp "$tl" "$keep/"
    if [ "$ra" != 0 ]; then
      echo "lane A (the tip gate of $sha) red, rc $ra: $keep/lane-A.out${tl:+, $tl}"
      if [ -n "${pb:-}" ] && kill -0 "$pb" 2>/dev/null; then
        # its quiet lock is a lockf of that group: it goes with it, so the next job isn't held
        kill -TERM -- "-$pb" 2>/dev/null; sleep 5; kill -KILL -- "-$pb" 2>/dev/null
        wait "$pb" 2>/dev/null; row B stopped "$tb" "$lb"
        echo "lane B stopped (lane A red): no tag"
      fi
      exit 1
    fi
    wait "$pb"; rb=$?; row B "$rb" "$tb" "$lb"
    set +m
    # once, after the pass's last quiet window (marketing m_63313 d): the moment main may lift a hold
    echo "heavy-run: timing-bound part done: release pass $tag, both lanes ended after $(( ($(date +%s) - ta) / 60 )) min (A rc $ra, B rc $rb)"
    [ "$rb" = 0 ] || { echo "lane B (the app side of $sha) red or refused, rc $rb: $keep/lane-B.out"; tail -5 "$keep/lane-B.out"; exit 1; }
  fi
  # the tag: tip-green checked just before it (desktop_tag.py), its verdicts read, not run again
  (cd "$tree" && "${tagger[@]}"); rc=$?
  # a tag that took a verdict of R: its full check (every test, every app verdict, none reused)
  # runs at once in the release lane, after it (architect m_63312, marketing m_63313 c)
  if [ "$what" = pass ] && [ $rc = 0 ] && [ -n "${rbase:-}" ]; then
    if awk -F'\t' -v s="$sha" '$1 == s && $6 ~ /^by-reuse=/ { f = 1 } END { exit !f }' "$BH/gate/green-shas.tsv" 2>/dev/null \
       || git -C "$tree" tag -l --format='%(contents)' "$tag" | grep -q ' by-reuse '; then
      nohup "$R/scripts/release_cut.sh" after "$sha" "$tag" >"$keep/after.out" 2>&1 &
      echo "a by-reuse tag: its full check runs now in the release lane (pid $!, $keep/after.out); a red writes $BH/run/nightly/release-red and blocks the next cut"
    fi
  fi
  exit $rc
  ;;
after)
  # release_cut.sh after <sha> <tag>: the full check of a by-reuse tag: the tip gate with every
  # test (TIP_FULL: a by-reuse green line doesn't count) and the app side with no verdict reused
  # (DESKTOP_TAG_NO_REUSE), in its own tree (a next cut may use the other). A red: release-red,
  # which main forwards with the question 'release_halt.py halt <tag>?' (marketing m_63313 c);
  # release_cut.sh pass refuses to start while it is there (main deletes it once owned)
  [ $# = 3 ] || { sed -n '2,40p' "$0"; exit 2; }
  sha="$(git -C "$R" rev-parse --verify -q "$2^{commit}")" || { echo "no commit '$2'"; exit 2; }
  tag="$3" out="$BH/run/nightly"
  TIP_FULL=1 "$R/scripts/nightly_gate.sh" --tip --sha "$sha"; ra=$?
  rb=skipped
  if [ $ra = 0 ]; then
    atree="${RELEASE_AFTER_TREE:-$HOME/.cache/bise-release/after-tree}"
    if [ ! -e "$atree/.git" ]; then mkdir -p "$(dirname "$atree")"; git -C "$R" worktree add -q --detach "$atree" "$sha"; fi
    git -C "$atree" checkout -q -f --detach "$sha" && find "$atree" -name __pycache__ -type d -prune -exec rm -rf {} +
    (cd "$atree" && DESKTOP_TAG_NO_REUSE=1 PYTHONDONTWRITEBYTECODE=1 python3 scripts/desktop_tag.py "$tag" "$sha" --checks-only); rb=$?
  fi
  if [ $ra != 0 ] || [ "$rb" != 0 ]; then
    what="the tip gate (rc $ra; $(cut -d' ' -f4 "$out/tip-last" 2>/dev/null))"
    [ $ra = 0 ] && what="the app side (rc $rb)"
    { echo "release $tag (${sha:0:12}): the full check after its by-reuse tag is red: $what"
      echo "question for main: release_halt.py halt $tag? (the next cut waits until this is owned: delete this file once it is)"; } >"$out/release-red.tmp" \
      && mv "$out/release-red.tmp" "$out/release-red"
    cat "$out/release-red"; exit 1
  fi
  echo "the full check after $tag (${sha:0:12}) is green: every test and every app verdict, none reused"
  ;;
*)
  sed -n '2,20p' "$0"; exit 2 ;;
esac
