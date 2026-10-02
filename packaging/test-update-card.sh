#!/usr/bin/env bash
# test-update-card.sh — the new-release item (update-card), end to end
# in a CLEAN fake HOME from a file:// channel, the TUI in tmux:
# install release 1 (0.0.1), a hub that checks the channel every 2 s,
# publish release 2 with what's new (make-release.sh --whats-new): ONE
# quiet item `bise v0.0.2 is out`, the notes, `you're on v0.0.1`; `2
# later`: gone, not back for 0.0.2; `/update`: back; publish a broken
# release 3 (a bad tarball): `1` fails in one line, the hub stays on
# release 1; fix release 3: `1` installs it and switches the hub onto it
# (the item closes on the new hub); `/update`: `you're on the latest
# bise, v0.0.3.` Captures of the item at 150 and 80 columns, with and
# without notes, the running note and the thread lines go to $OUT
# (default /tmp/uc-work/captures) for the designer.
# Everything is under /tmp: the real ~/.bise, ~/.local and live hubs are
# never touched.
#
#   packaging/test-update-card.sh
#
# The app of each release: a release `bise` from cargo, the Bend
# binaries from bins.sh's cache, a STUB bend-jsrt (like test-update-flow.sh).

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE" && git rev-parse --show-toplevel)"
T=/tmp/uc-home WS=/tmp/uc-ws REL=/tmp/uc-rel PK=/tmp/uc-pack WORK=/tmp/uc-work
OUT="${OUT:-$WORK/captures}"
TM="tmux -L uc-test"

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $*"; }
ko()  { fail=$((fail + 1)); echo "  FAIL $*"; }
check() { local what="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$what"; else ko "$what"; fi; }
until_ok() { local n="$1" i=0; shift; while [ $i -lt "$n" ]; do "$@" >/dev/null 2>&1 && return 0; sleep 0.1; i=$((i + 1)); done; return 1; }

kill_all() {
  $TM kill-server 2>/dev/null
  for p in $(ps -axo pid=,command= | grep -F "$T/.local/share/" | grep -v grep | awk '{print $1}'); do kill "$p" 2>/dev/null; done
  [ -n "${fpid:-}" ] && kill "$fpid" 2>/dev/null
  return 0
}
trap kill_all EXIT
kill_all
rm -rf "$T" "$WS" "$REL" "$PK" "$WORK"; mkdir -p "$T" "$PK/src" "$WORK" "$OUT"

FAKE_LOG="$WORK/fake.log" python3 -u "$REPO/tests/fake_provider.py" > "$WORK/fake.out" 2> "$WORK/fake.err" &
fpid=$!
until_ok 100 grep -q '^PORT' "$WORK/fake.out" || { echo "fake provider did not start"; exit 1; }
port="$(awk '/^PORT/ {print $2; exit}' "$WORK/fake.out")"
FAKE="BEND_PROVIDER_URL=http://127.0.0.1:$port/v1/chat/completions"

# a clean environment: fake HOME, system PATH only, the fake provider; the
# CLI never checks by itself (BISE_NO_UPDATE), the hub checks every 2 s
BASE_ENV=(HOME="$T" PATH=/usr/bin:/bin:/usr/sbin:/sbin SHELL=/bin/zsh USER="${USER:-me}" LANG=en_US.UTF-8
  MISTRAL_API_KEY=fake-key BEND_MODEL=mistral-small-latest SB_ONBOARDING=off "$FAKE")
E() { env -i "${BASE_ENV[@]}" TERM=dumb BISE_NO_UPDATE=1 "$@"; }
H() { env -i "${BASE_ENV[@]}" TERM=dumb BISE_RELEASE_CHECK_SECS=2 "$@"; }

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
printf '#!/bin/sh\necho "bend-jsrt stub (test-update-card.sh)" >&2; exit 1\n' > "$src/bend-jsrt"; chmod 755 "$src/bend-jsrt"
ID1="c1$(git -C "$REPO" rev-parse --short HEAD | cut -c1-5)"
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
publish() { "$HERE/make-release.sh" --out "$REL" --version "$2" ${3:+--whats-new "$3"} "$1" >/dev/null 2>&1; }
publish "$t1" 0.0.1 && ok "release 1 ($ID1) published in file://$REL" || { ko "make-release.sh"; exit 1; }

echo "== install release 1, a hub on it, the TUI in tmux"
(cd "$WORK" && E sh -c "curl -fsSL file://$REL/install.sh | sh") >/dev/null 2>&1
P="$T/.local/share/bise" BIN="$T/.local/bin/bise"
check "current -> versions/$ID1" test "$(readlink "$P/current")" = "versions/$ID1"
mkdir -p "$WS" && (cd "$WS" && git init -q && echo x > README.md && git add README.md \
  && git -c user.name=t -c user.email=t@t -c commit.gpgsign=false commit -qm init)
(cd "$WS" && H "$BIN" sbd --workspace "$WS" </dev/null >/dev/null 2>"$WORK/hub.err" &)
state=""
hub_up() { state="$(ls -d "$T"/.bise/hubs/uc-ws-* 2>/dev/null | head -n 1)"; [ -n "$state" ] && [ -S "$state/hub.sock" ]; }
until_ok 150 hub_up && ok "hub started" || { ko "hub did not start"; tail -n 5 "$WORK/hub.err"; exit 1; }
root_is() { [ "$(cat "$state/hub.root" 2>/dev/null)" = "$(cd "$P/versions/$1" && pwd -P)" ]; }
check "hub runs versions/$ID1" root_is "$ID1"
# the hub's first check: release 1 is the latest, its name is known
until_ok 100 grep -q "\"$ID1\"" "$T/.bise/cache/release-names.json" && ok "the hub's check read latest.json" || ko "no check: $(ls "$T/.bise/cache" 2>&1)"

tui() {  # <cols>
  $TM kill-server 2>/dev/null
  $TM new-session -d -s uc -x "$1" -y 40 "cd $WS && env -i ${BASE_ENV[*]} TERM=xterm-256color BISE_NO_UPDATE=1 BISE_CTRL_DIGITS=1 $BIN"
  until_ok 200 screen_has "bise"
}
screen() { $TM capture-pane -p -t uc 2>/dev/null; }
screen_has() { screen | grep -qF -- "$1"; }
wait_screen() { until_ok "${2:-200}" screen_has "$1"; }
snap() { screen > "$OUT/$1.txt"; }
# the item in the inbox box (its strip row), not old items folded in the thread
item_row() { screen_has "? bise · $1 is out"; }
no_item() { ! screen_has "waiting for you"; }
# the screen as one line: a long thread line wraps
flat_has() { screen | tr -d '│┃' | tr -s ' \n' '  ' | grep -qF -- "$1"; }
typed() { $TM send-keys -t uc -l -- "$1"; }
key() { $TM send-keys -t uc "$@"; }
CTRL1=$'\e[49;5u'
open_item() {  # ctrl+1 until the item view's key bar shows
  local i
  for i in 1 2 3; do
    key Escape; sleep 0.3; typed "$CTRL1"
    wait_screen "1-3 answer" 30 && return 0
  done
  return 1
}

tui 150
wait_screen "main" && ok "the TUI is up" || { ko "no TUI"; screen | tail -n 5; }
no_item && ok "no item while release 1 is the latest" || ko "an item already"

echo "== release 2 with what's new: one quiet item"
printf '# what is new in 0.0.2\n- the inbox keeps your place when an item closes\n\n- /update looks for a new release from any thread\n' > "$WORK/notes2.txt"
publish "$t2" 0.0.2 "$WORK/notes2.txt" || ko "publish 2"
grep -q '"notes": \["the inbox keeps your place when an item closes", "/update looks for a new release from any thread"\]' "$REL/latest.json" \
  && ok "latest.json carries the notes" || { ko "latest.json notes"; grep notes "$REL/latest.json"; }
until_ok 300 item_row v0.0.2 && ok "the item is in the inbox" || { ko "no item"; screen | tail -n 15; }
snap item-strip-150
check "the hub did not switch by itself" root_is "$ID1"
check "bise update did not run (current is still release 1)" test "$(readlink "$P/current")" = "versions/$ID1"
open_item && screen_has "you're on v0.0.1" && ok "the item: you're on v0.0.1" || { ko "the open item"; screen | tail -n 20; }
check "its notes" screen_has "the inbox keeps your place when an item closes"
check "1 update now" screen_has "update now · your agents keep running"
check "3 release notes" screen_has "release notes"
snap item-open-150

echo "== 2 later: gone, and not back for 0.0.2"
typed 2
until_ok 100 no_item && ok "later: the item closed" || ko "still there"
sleep 5
no_item && ok "not asked again for 0.0.2 (2 checks later)" || { ko "asked again"; screen | tail -n 12; }
check "later is kept" grep -qx "$ID2" "$T/.bise/cache/update-later"

echo "== /update: the item again"
typed "/update"; key Enter
until_ok 100 sh -c "$TM capture-pane -p -t uc | grep -qF '1-3 answer'" && screen_has "? bise v0.0.2 is out" \
  && ok "/update brings it back, open" || { ko "/update"; screen | tail -n 10; }

echo "== 80 columns, and an item without notes"
tui 80
wait_screen "is out" 200; open_item || ko "open the item"
snap item-open-80
key Escape
# release 3 published broken: no notes, and its tarball does not match
cp "$t3" "$WORK/good3.tar.gz"
publish "$t3" 0.0.3 || ko "publish 3"
printf 'garbage' >> "$REL/$(basename "$t3")"
until_ok 300 item_row v0.0.3 && ok "release 3 replaces the item" || { ko "no item for 0.0.3"; screen | tail -n 10; }
tui 150
wait_screen "is out" 200; open_item || ko "open the item"
screen | grep -A1 -F "? bise v0.0.3 is out" | tail -n 1 | grep -qF "you're on v0.0.1" && ok "no notes: no body" || { ko "no notes: no body"; screen | tail -n 15; }
snap item-no-notes-150

echo "== 1 on a broken release: one line, the running version stays"
typed 1
until_ok 300 flat_has "couldn't update to v0.0.3, you're still on v0.0.1: checksum mismatch for bise-$ID3-$target.tar.gz" && ok "the failure in one line" || { ko "no failure line"; screen | tail -n 15; }
! flat_has "file:///" && ok "the reason names the file, not its URL" || ko "a URL in the failure line"
snap update-failed-150
check "the hub still runs release 1" root_is "$ID1"

echo "== 1 on a good release: installed, switched"
cp "$WORK/good3.tar.gz" "$REL/$(basename "$t3")"
typed "/update"; key Enter
until_ok 100 sh -c "$TM capture-pane -p -t uc | grep -qF '? bise v0.0.3 is out'" && wait_screen "1-3 answer" 50 \
  || { ko "/update did not open the item after the fix"; screen | tail -n 10; }
typed 1
until_ok 50 screen_has "updating to v0.0.3" && { snap updating-150; ok "the running note"; } || ok "(the running note went by too fast to capture)"
until_ok 300 flat_has "updated to v0.0.3. switching now, your agents keep running." && ok "the success line" || { ko "no success line"; screen | tail -n 15; }
snap updated-150
until_ok 400 root_is "$ID3" && ok "the hub runs versions/$ID3" || ko "hub root: $(cat "$state/hub.root" 2>/dev/null)"
check "current -> $ID3" test "$(readlink "$P/current")" = "versions/$ID3"
until_ok 100 hub_up
kill "$(cat "$state/switch.pid" 2>/dev/null)" 2>/dev/null; sleep 0.5
tui 150
sleep 5
no_item && ok "the item closed on the new hub" || { ko "the item is still there"; screen | tail -n 10; }
typed "/update"; key Enter
until_ok 100 flat_has "you're on the latest bise, v0.0.3." && ok "/update: you're on the latest" || { ko "/update on the latest"; screen | tail -n 5; }
snap latest-150

echo "== stop"
pid="$(cat "$state/hub.pid" 2>/dev/null)"
(cd "$WS" && E "$BIN" switchboard --stop --workspace "$WS") >/dev/null 2>&1
[ -n "$pid" ] && until_ok 50 sh -c "! kill -0 $pid 2>/dev/null" && ok "hub stopped" || ko "hub still running (pid '$pid')"
kill_all
echo "captures: $OUT"
echo "== $pass passed, $fail failed"
[ "$fail" = 0 ]
