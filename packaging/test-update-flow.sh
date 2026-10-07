#!/usr/bin/env bash
# test-update-flow.sh — updating an installed bise moves its running hub
# (BISE-255), end to end in a CLEAN fake HOME from a file:// channel:
# install release 1, a hub with a task (its REPL answered by
# tests/fake_provider.py), publish release 2, `bise update`, then
# `/restart latest`: the hub runs release 2 and the task goes on; publish
# release 3, `bise update`, then launch `bise` again in the workspace:
# the hub moves to release 3 before the TUI attaches, the task goes on.
# Everything is under /tmp: the real ~/.bise, ~/.local and live hubs are
# never touched.
#
#   packaging/test-update-flow.sh
#
# The app of each release: a release `bise` from cargo, the Bend
# binaries from bins.sh's cache, a STUB bend-jsrt (like test-release.sh).

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE" && git rev-parse --show-toplevel)"
T=/tmp/pu-home WS=/tmp/pu-ws REL=/tmp/pu-rel PK=/tmp/pu-pack WORK=/tmp/pu-work

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $*"; }
ko()  { fail=$((fail + 1)); echo "  FAIL $*"; }
check() { local what="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$what"; else ko "$what"; fi; }
until_ok() { local n="$1" i=0; shift; while [ $i -lt "$n" ]; do "$@" >/dev/null 2>&1 && return 0; sleep 0.1; i=$((i + 1)); done; return 1; }

kill_all() {
  for p in $(ps -axo pid=,command= | grep -F "$T/.local/share/" | grep -v grep | awk '{print $1}'); do kill "$p" 2>/dev/null; done
  [ -n "${fpid:-}" ] && kill "$fpid" 2>/dev/null
  return 0
}
trap kill_all EXIT
kill_all
rm -rf "$T" "$WS" "$REL" "$PK" "$WORK"; mkdir -p "$T" "$PK/src" "$WORK"

# the scripted provider: the task's turns never leave the machine
FAKE_LOG="$WORK/fake.log" python3 -u "$REPO/tests/fake_provider.py" > "$WORK/fake.out" 2> "$WORK/fake.err" &
fpid=$!
until_ok 100 grep -q '^PORT' "$WORK/fake.out" || { echo "fake provider did not start"; exit 1; }
port="$(awk '/^PORT/ {print $2; exit}' "$WORK/fake.out")"
FAKE="BEND_PROVIDER_URL=http://127.0.0.1:$port/v1/chat/completions"

# a clean environment: fake HOME, system PATH only, the fake provider
E() { env -i HOME="$T" PATH=/usr/bin:/bin:/usr/sbin:/sbin SHELL=/bin/zsh TERM=dumb \
        USER="${USER:-me}" LANG=en_US.UTF-8 BISE_NO_UPDATE=1 MISTRAL_API_KEY=fake-key \
        BEND_MODEL=mistral-small-latest SB_ONBOARDING=off "$FAKE" "$@"; }

os="$(uname -s | tr '[:upper:]' '[:lower:]')"; arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
target="$os-$arch"

echo "== the app of the releases"
src="$PK/src/app"; mkdir -p "$src"
(cd "$REPO/rust" && cargo build --release -q -p bend-harness) || { echo "cargo build failed"; exit 1; }
cp "${CARGO_TARGET_DIR:-$REPO/rust/target}/release/bise" "$src/bise"
ln -s bise "$src/bend-harness"
for b in repl-live repl-scripted sb-core; do
  p="$("$REPO/scripts/bins.sh" path "$b" | tail -n 1)" && [ -x "$p" ] || { echo "bins.sh path $b failed"; exit 1; }
  cp "$p" "$src/$b"
done
cp -R "$REPO/prompts" "$src/prompts"
cp "$REPO"/LICENSE "$REPO"/NOTICE "$REPO"/THIRD_PARTY_NOTICES "$src/"
printf '#!/bin/sh\necho "bend-jsrt stub (test-update-flow.sh)" >&2; exit 1\n' > "$src/bend-jsrt"; chmod 755 "$src/bend-jsrt"
ID1="u1$(git -C "$REPO" rev-parse --short HEAD | cut -c1-5)"
printf 'id=%s\ncommit=%s\nsubject=x\nbuilt=x\nmacos=14.0\ntarget=%s\nchannel=test\n' \
  "$ID1" "$(git -C "$REPO" rev-parse HEAD)" "$target" > "$src/VERSION"
ID2="$ID1-r2" ID3="$ID1-r3"
pack() {
  local id="$1" built="$2" d="$PK/bise-$1-$target"
  rm -rf "$d"; mkdir -p "$d"
  cp -cR "$src" "$d/app" 2>/dev/null || cp -R "$src" "$d/app"
  { grep -vE '^(id|built|subject)=' "$src/VERSION"; echo "id=$id"; echo "built=$built"; echo "subject=release $id"; } > "$d/app/VERSION"
  cp "$HERE/install.sh" "$d/install.sh"
  tar -C "$PK" -czf "$PK/bise-$id-$target.tar.gz" "bise-$id-$target"
  echo "$PK/bise-$id-$target.tar.gz"
}
t1="$(pack "$ID1" 2026-01-01T00:00:01Z)" t2="$(pack "$ID2" 2026-01-02T00:00:00Z)" t3="$(pack "$ID3" 2026-01-03T00:00:00Z)"
publish() { "$HERE/make-release.sh" --out "$REL" --version "$2" "$1" >/dev/null 2>&1; }
publish "$t1" 0.0.1 && ok "release 1 ($ID1) published in file://$REL" || { ko "make-release.sh"; exit 1; }

echo "== install release 1"
(cd "$WORK" && E sh -c "curl -fsSL file://$REL/install.sh | sh") >/dev/null 2>&1
P="$T/.local/share/bise" BIN="$T/.local/bin/bise"
check "current -> versions/$ID1" test "$(readlink "$P/current")" = "versions/$ID1"

echo "== a hub on release 1, with a task"
mkdir -p "$WS" && (cd "$WS" && git init -q && echo x > README.md && git add README.md \
  && git -c user.name=t -c user.email=t@t -c commit.gpgsign=false commit -qm init)
(cd "$WS" && E "$BIN" sbd --workspace "$WS" </dev/null >/dev/null 2>"$WORK/hub.err" &)
state=""
hub_up() { state="$(ls -d "$T"/.bise/hubs/pu-ws-* 2>/dev/null | head -n 1)"; [ -n "$state" ] && [ -S "$state/hub.sock" ]; }
until_ok 150 hub_up && ok "hub started" || { ko "hub did not start"; tail -n 5 "$WORK/hub.err"; exit 1; }
root_is() { [ "$(cat "$state/hub.root" 2>/dev/null)" = "$(cd "$P/versions/$1" && pwd -P)" ]; }
check "hub runs versions/$ID1" root_is "$ID1"
# the agents' socket (agent.sock); a hub older than docs/issues/16 has only hub.sock
sbm() { s="$state/agent.sock"; [ -S "$s" ] || s="$state/hub.sock"; E SB_SOCKET="$s" SB_AGENT=main "$state/bin/sb" "$@" 2>&1; }
sbm spawn t1 --objective "remember zorglub-1 {{bash: echo first-turn}}" | sed 's/^/     /'
# the task's turns, as the fake provider saw them
turns() { grep -c '"agent": *"t1"' "$WORK/fake.log" 2>/dev/null || echo 0; }
t1_idle() { sbm list | grep -E '^t1 ' | grep -q idle; }
until_ok 300 t1_idle && ok "task t1 ran its first turn (idle)" || { ko "t1 not idle"; sbm list | sed 's/^/     /'; }
repl_on() { ps -axo command= | grep -F "$P/versions/$1/repl-live" | grep -v grep | grep -q .; }
check "t1's REPL runs release 1" repl_on "$ID1"

# the task goes on: a new turn reaches the provider, with the first one in it
goes_on() {
  local n; n="$(turns)"
  sbm send t1 "and now $1 {{bash: echo $1}}" >/dev/null
  until_ok 300 sh -c "[ \$(grep -c '\"agent\": *\"t1\"' '$WORK/fake.log') -gt $n ]" || return 1
  tail -n 1 "$WORK/fake.log" | grep -q zorglub-1
}

echo "== release 2, bise update, /restart latest"
publish "$t2" 0.0.2 || ko "publish 2"
out="$(E "$BIN" update 2>&1)"; echo "$out" | sed 's/^/     /'
check "current -> $ID2" test "$(readlink "$P/current")" = "versions/$ID2"
echo "$out" | grep -q "launch bise again in its folder, or type /restart" && ok "bise update says what to do next" || ko "bise update's next step"
check "the hub still runs $ID1 (never switched silently)" root_is "$ID1"
sbm restart latest | sed 's/^/     /'
until_ok 300 root_is "$ID2" && ok "/restart latest: hub runs versions/$ID2" || ko "hub root: $(cat "$state/hub.root" 2>/dev/null)"
until_ok 100 hub_up
sbm list | grep -qE '^t1 ' && ok "t1 is still there" || ko "t1 gone after the switch"
goes_on second && ok "t1 goes on (a turn with its history)" || ko "t1 does not go on after /restart latest"
until_ok 300 repl_on "$ID2" && ok "t1's REPL moved to release 2" || ko "t1's REPL is not on $ID2"
# the switch is on probation (2 min): end its switcher so the next one runs
kill "$(cat "$state/switch.pid" 2>/dev/null)" 2>/dev/null; sleep 0.5

echo "== release 3, bise update, then launching bise again"
publish "$t3" 0.0.3 || ko "publish 3"
E "$BIN" update 2>&1 | sed 's/^/     /'
check "current -> $ID3" test "$(readlink "$P/current")" = "versions/$ID3"
check "the hub still runs $ID2" root_is "$ID2"
# not a terminal: the TUI's line mode, after the hub moved
(cd "$WS" && E "$BIN" </dev/null >"$WORK/tui.out" 2>"$WORK/tui.err" &)
until_ok 400 root_is "$ID3" && ok "launching bise moved the hub to versions/$ID3" || { ko "hub root: $(cat "$state/hub.root" 2>/dev/null)"; tail -n 5 "$WORK/tui.err"; }
grep -q "moving it to $ID3" "$WORK/tui.err" && ok "the launch said so" || { ko "the launch said nothing"; cat "$WORK/tui.err" | sed 's/^/     /'; }
until_ok 100 hub_up
sbm list | grep -qE '^t1 ' && ok "t1 is still there" || ko "t1 gone after the launch"
goes_on third && ok "t1 goes on (a turn with its history)" || ko "t1 does not go on after the launch"
until_ok 300 repl_on "$ID3" && ok "t1's REPL moved to release 3" || ko "t1's REPL is not on $ID3"
kill "$(cat "$state/switch.pid" 2>/dev/null)" 2>/dev/null; sleep 0.5
# launching again: nothing to move
(cd "$WS" && E "$BIN" </dev/null >"$WORK/tui2.out" 2>"$WORK/tui2.err" &)
sleep 2
! grep -q "moving it" "$WORK/tui2.err" && ok "a launch on the same version moves nothing" || ko "a second launch moved the hub again"

echo "== stop"
pid="$(cat "$state/hub.pid" 2>/dev/null)"
(cd "$WS" && E "$BIN" switchboard --stop --workspace "$WS") >/dev/null 2>&1
[ -n "$pid" ] && until_ok 50 sh -c "! kill -0 $pid 2>/dev/null" && ok "hub stopped" || ko "hub still running (pid '$pid')"
kill_all
echo "== $pass passed, $fail failed"
[ "$fail" = 0 ]
