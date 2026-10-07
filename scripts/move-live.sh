#!/usr/bin/env bash
# move-live.sh — move the live Switchboard from one folder to another of
# the same repository (e.g. the worktree harness-switchboard -> the main
# checkout harness), keeping every agent's context: journal (threads,
# cards, task board), sessions, transcripts, versions.
#
# Run it from a PLAIN TERMINAL (not from an agent: the hub stops and
# every agent REPL is restarted).
#
#   scripts/move-live.sh --to <new folder> [--from <old folder>] [--branch <br>]
#                  [--no-push] [--no-tui] [--wait <s>] [--allow-dirty]
#   scripts/move-live.sh --remove-old <old folder>     # the last step, separate
#
# Steps of the move:
#  1. back up any uncommitted change of the old folder (diff + untracked
#     files, /tmp/sb-move-backup-<ts>/); refuse to go on if something else
#     than regenerable binaries is uncommitted (--allow-dirty: go on; it is
#     saved anyway);
#  2. merge what the old branch has that the new folder's branch has not
#     (a merge commit made and tested in a temporary worktree: cargo build
#     + switchboard/tui unit tests; the new branch then fast-forwards to
#     it; pushed unless --no-push; never forced);
#  3. build the managed version of the new folder's HEAD (versions.sh);
#  4. wait (up to --wait s, default 300) for the agents to be idle;
#  5. back up the hub state, stop the hub (its REPLs stop: their working
#     directory must change);
#  6. move the state to the new folder's key: journal (task workspace
#     paths rewritten old -> new), sessions, transcripts, wire logs,
#     versions; task git worktrees moved with `git worktree move`; the
#     old state dir is renamed <dir>.moved-<ts>;
#  7. start the hub on that version, workspace = the new folder: every
#     agent resumes its SAME session there; one cut mid-turn is told to
#     continue; each agent gets a note that the workspace moved;
#  8. open the TUI there.
# The old folder is only removed by --remove-old (git worktree remove).
#
# For tests on throwaway workspaces: BISE_HOME, SB_VERSION_DIR (use
# this version dir instead of building one), SB_MOVE_GATES (the gate
# command run in the merge worktree).
set -euo pipefail

say() { echo "move-live: $*" >&2; }
die() { say "$*"; exit 1; }

FROM="$(cd "$(dirname "$0")/.." && pwd)"
TO=""
BRANCH=""
push=1
tui=1
wait_s=300
allow_dirty=0
remove_old=""
while [ $# -gt 0 ]; do
  case "$1" in
    --to) TO="$2"; shift ;;
    --from) FROM="$2"; shift ;;
    --branch) BRANCH="$2"; shift ;;
    --no-push) push=0 ;;
    --no-tui) tui=0 ;;
    --wait) wait_s="$2"; shift ;;
    --allow-dirty) allow_dirty=1 ;;
    --remove-old) remove_old="$2"; shift ;;
    *) die "unknown argument $1" ;;
  esac
  shift
done

# the state dir of a workspace, from paths.rs (`bise switchboard
# --state-dir`): the first binary that knows the flag (versions built
# before it do not; they would open a TUI)
state_dir_of() {
  local b
  for b in "$@"; do
    if [ -x "$b" ] && grep -aq -- '--state-dir' "$b"; then
      "$b" switchboard --state-dir --workspace "$SD_WS" </dev/null
      return
    fi
  done
  return 1
}
# the workspace's own key (an empty SB_STATE_DIR is unset: the old and
# the new folder have two states)
state_of() {
  SB_STATE_DIR="" SD_WS="$1" state_dir_of \
    "$FROM/rust/target/debug/bise" "$FROM/rust/target/release/bise" \
    "$FROM/rust/target/debug/bend-harness" "$FROM/rust/target/release/bend-harness" \
    || die "no bise with --state-dir in $FROM (build it: cd rust && cargo build)"
}

TS="$(date +%Y%m%d-%H%M%S)"
BK="/tmp/sb-move-backup-$TS"

# files that are rebuilt, never work: their changes do not block
REGEN='^(repl-live|repl-scripted|sb-core|harness-demo|repl)$'

# save every uncommitted change of a folder under $BK/<label>
save_changes() {
  local dir="$1" label="$2"
  mkdir -p "$BK/$label"
  git -C "$dir" diff --binary HEAD > "$BK/$label/tracked.diff" || true
  git -C "$dir" ls-files --others --exclude-standard -z > "$BK/$label/untracked.list" || true
  if [ -s "$BK/$label/untracked.list" ]; then
    (cd "$dir" && tr '\0' '\n' < "$BK/$label/untracked.list" | tar -cf "$BK/$label/untracked.tar" -T - 2>/dev/null) || true
  fi
  git -C "$dir" status --porcelain > "$BK/$label/status.txt" || true
}

# uncommitted paths that are not regenerable (tracked or untracked)
blocking_changes() {
  git -C "$1" status --porcelain | awk '{print $NF}' | grep -Ev "$REGEN" || true
}

# ------------------------------------------------------------------
if [ -n "$remove_old" ]; then
  OLD="$(cd "$remove_old" && pwd -P)"
  OLD_STATE="$(state_of "$OLD")"
  if [ -S "$OLD_STATE/hub.sock" ] && [ -f "$OLD_STATE/journal.jsonl" ]; then
    die "a hub state still lives at $OLD_STATE: move the live first"
  fi
  save_changes "$OLD" old-final
  b="$(blocking_changes "$OLD")"
  if [ -n "$b" ]; then
    say "uncommitted work in $OLD (saved in $BK/old-final):"
    echo "$b" >&2
    die "commit it (or apply the backup elsewhere) before removing the folder"
  fi
  branch="$(git -C "$OLD" branch --show-current)"
  git -C "$OLD" worktree remove --force "$OLD"
  say "removed the worktree $OLD (branch $branch kept; backup of its last state: $BK/old-final)"
  exit 0
fi

[ -n "$TO" ] || die "--to <new folder> is required"
OLD="$(cd "$FROM" && pwd -P)"
NEW="$(cd "$TO" && pwd -P)"
[ "$OLD" != "$NEW" ] || die "--from and --to are the same folder"
OLD_STATE="$(state_of "$OLD")"
NEW_STATE="$(state_of "$NEW")"
[ -f "$OLD_STATE/journal.jsonl" ] || die "no hub state for $OLD ($OLD_STATE)"
if [ -f "$NEW_STATE/journal.jsonl" ]; then
  die "$NEW already has a hub state ($NEW_STATE): refusing to overwrite it"
fi
common() { git -C "$1" rev-parse --path-format=absolute --git-common-dir; }
[ "$(common "$OLD")" = "$(common "$NEW")" ] || die "$OLD and $NEW are not folders of the same repository"
BRANCH="${BRANCH:-$(git -C "$OLD" branch --show-current)}"
NEW_BRANCH="$(git -C "$NEW" branch --show-current)"
mkdir -p "$BK"
say "from $OLD ($BRANCH) to $NEW ($NEW_BRANCH); backups in $BK"

# 1. uncommitted changes: saved, and blocking unless regenerable
save_changes "$OLD" old
save_changes "$NEW" new
b="$(blocking_changes "$OLD")"
if [ -n "$b" ] && [ "$allow_dirty" = 0 ]; then
  say "uncommitted work in $OLD (saved in $BK/old):"
  echo "$b" >&2
  die "commit it first (or --allow-dirty: it stays saved in $BK/old)"
fi
[ -z "$(git -C "$NEW" status --porcelain --untracked-files=no | awk '{print $NF}' | grep -Ev "$REGEN" || true)" ] \
  || die "$NEW has uncommitted changes to tracked files (saved in $BK/new): commit them first"

# 2. merge what the old branch has, tested before the new branch moves
if [ -n "$(git -C "$NEW" rev-list "$NEW_BRANCH..$BRANCH" 2>/dev/null)" ]; then
  n="$(git -C "$NEW" rev-list --count "$NEW_BRANCH..$BRANCH")"
  say "merging $n commit(s) of $BRANCH into $NEW_BRANCH (tested in a temporary worktree)"
  mwt="/tmp/sb-move-merge-$TS"
  git -C "$NEW" worktree add -q --detach "$mwt" "$NEW_BRANCH"
  cleanup_mwt() { git -C "$NEW" worktree remove --force "$mwt" 2>/dev/null || true; }
  trap cleanup_mwt EXIT
  git -C "$mwt" merge --no-ff -q "$BRANCH" -m "Merge branch '$BRANCH' into $NEW_BRANCH (move of the live Switchboard)" \
    || { git -C "$mwt" merge --abort || true; die "merge conflict: resolve it by hand in $NEW, then run again"; }
  say "gates: cargo build + switchboard/tui unit tests"
  # ./sb-core is not in git (BISE-114): the switchboard tests spawn it
  gates="${SB_MOVE_GATES:-scripts/bins.sh sb-core && cd rust && CARGO_TARGET_DIR=$STATE_ROOT/build/target-move cargo build -q -p bend-harness && CARGO_TARGET_DIR=$STATE_ROOT/build/target-move cargo test -q -p switchboard -p bend-tui >/dev/null}"
  (cd "$mwt" && bash -c "$gates") \
    || die "gates failed: $NEW_BRANCH not moved (the merge was only in $mwt)"
  merged="$(git -C "$mwt" rev-parse HEAD)"
  git -C "$NEW" merge --ff-only -q "$merged"
  cleanup_mwt; trap - EXIT
  say "$NEW_BRANCH is now $(git -C "$NEW" rev-parse --short HEAD)"
  if [ "$push" = 1 ]; then
    git -C "$NEW" push -q origin "$NEW_BRANCH" || say "push failed (not forced): push $NEW_BRANCH by hand"
  fi
fi

# 3. the version the hub will run
if [ -n "${SB_VERSION_DIR:-}" ]; then
  vdir="$SB_VERSION_DIR"
else
  vs="$NEW/scripts/versions.sh"; [ -x "$vs" ] || vs="$NEW/versions.sh"
  vdir="$("$vs" build HEAD)"
fi
exe="$vdir/bise"   # BISE-165; a version built before: bend-harness
[ -x "$exe" ] || exe="$vdir/bend-harness"
[ -x "$exe" ] || die "no bise in $vdir"
say "version: $(basename "$vdir")"

sbc() { SB_SOCKET="$1/agent.sock" SB_AGENT=main "$exe" sb "${@:2}"; }
hub_pid() {
  local p; p="$(cat "$1/hub.pid" 2>/dev/null || true)"
  if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then echo "$p"; fi
}

# 4. let the running turns end (a turn cut anyway is resumed)
if [ -n "$(hub_pid "$OLD_STATE")" ]; then
  t0=$(date +%s)
  while :; do
    busy="$(sbc "$OLD_STATE" list 2>/dev/null | awk '$3=="working"{print $1}' | tr '\n' ' ')"
    [ -z "$busy" ] && break
    if [ $(( $(date +%s) - t0 )) -ge "$wait_s" ]; then
      say "still busy after ${wait_s}s: ${busy}— their turn resumes after the move"
      break
    fi
    say "waiting for the agents to be idle: $busy"
    sleep 5
  done
fi

# 5. backup, stop
# not -a (it implies -D): copying a socket fails with "mkstempsock: Invalid
# argument" past the 104-byte socket path limit (macOS's openrsync has no
# --no-specials). agents/*/tmp: scratch, not state.
rsync -rlptg --exclude /hub.sock --exclude '/agents/*/tmp/' "$OLD_STATE/" "$BK/state/" \
  || die "the state backup to $BK/state failed (rsync above); nothing was moved"
say "hub state backed up: $BK/state"
if [ -n "$(hub_pid "$OLD_STATE")" ]; then
  "$exe" switchboard --stop --workspace "$OLD" >/dev/null 2>&1 || true
  for _ in $(seq 1 40); do [ -z "$(hub_pid "$OLD_STATE")" ] && break; sleep 0.25; done
  [ -z "$(hub_pid "$OLD_STATE")" ] || die "the hub of $OLD did not stop"
fi
# no REPL may survive with the old working directory
for f in "$OLD_STATE"/agents/*/repl.pid; do
  [ -f "$f" ] || continue
  p="$(cat "$f")"
  if kill -0 "$p" 2>/dev/null && ps -p "$p" -o command= | grep -q repl-; then kill "$p" || true; fi
done

# 6. the state, at the new folder's key
mkdir -p "$NEW_STATE"
if [ -d "$OLD_STATE/worktrees" ]; then
  mkdir -p "$NEW_STATE/worktrees"
  for w in "$OLD_STATE"/worktrees/*; do
    [ -d "$w" ] || continue
    git -C "$NEW" worktree move "$w" "$NEW_STATE/worktrees/$(basename "$w")"
  done
fi
# sockets and FIFOs stay behind (-rlptg, not -a); the agents' tmp moves
rsync -rlptg --exclude hub.sock --exclude hub.pid --exclude switch.pid --exclude hub.root \
  --exclude bin/ --exclude worktrees/ --exclude 'agents/*/repl.json' --exclude 'agents/*/repl.pid' \
  "$OLD_STATE/" "$NEW_STATE/"
python3 - "$NEW_STATE/journal.jsonl" "$OLD" "$NEW" "$OLD_STATE" "$NEW_STATE" <<'PY'
import json, os, sys
path, old, new, old_st, new_st = sys.argv[1:6]
def fix(p):
    if p == old or p.startswith(old + "/"):
        return new + p[len(old):]
    if p == old_st or p.startswith(old_st + "/"):
        return new_st + p[len(old_st):]
    return p
def walk(v):
    # only the structured workspace paths: {"ws": {"path": ...}}
    if isinstance(v, dict):
        ws = v.get("ws")
        if isinstance(ws, dict) and isinstance(ws.get("path"), str):
            ws["path"] = fix(ws["path"])
        for x in v.values():
            walk(x)
    elif isinstance(v, list):
        for x in v:
            walk(x)
out, n = [], 0
for line in open(path):
    line = line.rstrip("\n")
    if not line:
        continue
    e = json.loads(line)
    before = json.dumps(e, sort_keys=True)
    walk(e)
    if json.dumps(e, sort_keys=True) != before:
        n += 1
        out.append(json.dumps(e, ensure_ascii=False, separators=(",", ":")))
    else:
        out.append(line)
tmp = path + ".tmp"
open(tmp, "w").write("\n".join(out) + "\n")
os.replace(tmp, path)
print("journal: %d event(s) rewritten to the new paths" % n, file=sys.stderr)
PY
# the system prompt saved in each session (its CFG line) names the
# working directory: the new one from now on. Only that line: the
# messages are history (and signed thinking blocks must stay intact).
python3 - "$OLD" "$NEW" "$OLD_STATE" "$NEW_STATE" "$NEW_STATE"/agents/*/session.txt <<'PY'
import os, sys
old, new, old_st, new_st = sys.argv[1:5]
n = 0
for path in sys.argv[5:]:
    if not os.path.isfile(path):
        continue
    lines = open(path, encoding="utf-8").read().split("\n")
    changed = False
    for i, l in enumerate(lines):
        if l.startswith("CFG ") and (old in l or old_st in l):
            lines[i] = l.replace(old_st, new_st).replace(old, new)
            changed = True
    if changed:
        n += 1
        tmp = path + ".tmp"
        open(tmp, "w", encoding="utf-8").write("\n".join(lines))
        os.replace(tmp, path)
print("sessions: %d system prompt(s) moved to the new paths" % n, file=sys.stderr)
PY
python3 - "$NEW_STATE/versions.json" "$vdir" <<'PY'
import json, os, sys
path, vdir = sys.argv[1:3]
try: st = json.load(open(path))
except Exception: st = {}
st.update(current=vdir, good=vdir, probation_until=0)
json.dump(st, open(path + ".tmp", "w")); os.replace(path + ".tmp", path)
PY
mv "$OLD_STATE" "$OLD_STATE.moved-$TS"
say "state moved: $NEW_STATE (the old one is now $OLD_STATE.moved-$TS)"

# 7. the hub on the new folder
# the live hub is nobody's: an agent's stop or /drop never kills it (BISE-243)
(cd "$vdir" && export BISE_OWNERS= && exec python3 -c 'import os,sys; os.setsid(); os.execv(sys.argv[1], sys.argv[1:])' \
   "$exe" sbd --workspace "$NEW" </dev/null >/dev/null 2>>"$NEW_STATE/hub.err" &)
for _ in $(seq 1 100); do
  [ -S "$NEW_STATE/hub.sock" ] && [ -n "$(hub_pid "$NEW_STATE")" ] && break
  sleep 0.1
done
[ -n "$(hub_pid "$NEW_STATE")" ] || die "the hub did not start (see $NEW_STATE/hub.err); state backup: $BK/state"
say "hub pid $(hub_pid "$NEW_STATE") on $NEW"
# the agents come back on their sessions; then each gets the note
for _ in $(seq 1 60); do
  sbc "$NEW_STATE" list 2>/dev/null | awk '{print $3}' | grep -q starting || break
  sleep 1
done
NOTE="The Switchboard workspace moved from $OLD to $NEW (same repository, branch $NEW_BRANCH; the old folder goes away). Your session is kept. Your working directory is now $NEW: use the new paths from now on."
python3 - "$NEW_STATE/hub.sock" "$NOTE" <<'PY'
import json, socket, sys, time
sock, note = sys.argv[1:3]
s = socket.socket(socket.AF_UNIX); s.connect(sock)
s.sendall(b'{"op":"hello"}\n' + (json.dumps({"op": "input", "focus": "main", "text": note}) + "\n").encode())
time.sleep(0.5); s.close()
PY
for name in $(sbc "$NEW_STATE" list 2>/dev/null | awk '$1!="main"{print $1}'); do
  sbc "$NEW_STATE" send "$name" "$NOTE" --mode queued >/dev/null 2>&1 || say "note to $name failed"
done
say "done: the live Switchboard runs on $NEW; old folder still there (remove it with: $0 --remove-old $OLD)"

# 8. the TUI there
if [ "$tui" = 1 ]; then
  cd "$vdir"
  exec "$exe" switchboard --workspace "$NEW"
fi
