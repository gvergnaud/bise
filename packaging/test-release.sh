#!/usr/bin/env bash
# test-release.sh — the release channel end to end in a CLEAN fake HOME
# (BISE-173, local part), from a file:// channel: `curl | sh` install,
# launch, `bise update` (explicit, and the daily background check), a
# hub that sees the update and switches with `sb restart latest`, a bad
# checksum refused, uninstall. Everything is under /tmp: the real
# ~/.bise, ~/.local and live hubs are never touched.
#
#   packaging/test-release.sh [<tarball>]
#
# <tarball>: a build-dist.sh archive (release 1; 2 and 3 are the same
# app with another id and a later build date). Without one: a bundle
# assembled from this tree (a release `bise` from cargo, the Bend
# binaries from bins.sh's cache) with a STUB bend-jsrt (the V8 engine is
# not exercised here; test-install.sh runs a real archive).

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE" && git rev-parse --show-toplevel)"
T=/tmp/pr-home WS=/tmp/pr-ws REL=/tmp/pr-rel PK=/tmp/pr-pack WORK=/tmp/pr-work

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $*"; }
ko()  { fail=$((fail + 1)); echo "  FAIL $*"; }
check() { local what="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$what"; else ko "$what"; fi; }
# wait until a command succeeds (<= $1 tenths of a second)
until_ok() { local n="$1" i=0; shift; while [ $i -lt "$n" ]; do "$@" >/dev/null 2>&1 && return 0; sleep 0.1; i=$((i + 1)); done; return 1; }

# a clean environment: fake HOME, system PATH only, zsh as the shell
E() { env -i HOME="$T" PATH=/usr/bin:/bin:/usr/sbin:/sbin SHELL=/bin/zsh TERM=dumb \
        USER="${USER:-me}" LANG=en_US.UTF-8 "$@"; }

for p in $(ps -axo pid=,command= | grep -F "$T/.local/share/" | grep -v grep | awk '{print $1}'); do kill "$p" 2>/dev/null; done
rm -rf "$T" "$WS" "$REL" "$PK" "$WORK"; mkdir -p "$T" "$PK/src" "$WORK"

os="$(uname -s | tr '[:upper:]' '[:lower:]')"; arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
target="$os-$arch"

echo "== the app of release 1"
if [ $# -gt 0 ]; then
  tar -C "$PK/src" -xzf "$1" || { echo "cannot extract $1"; exit 1; }
  src="$(ls -d "$PK"/src/*/app)"
  ID1="$(sed -n 's/^id=//p' "$src/VERSION")"
else
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
  printf '#!/bin/sh\necho "bend-jsrt stub (test-release.sh)" >&2; exit 1\n' > "$src/bend-jsrt"; chmod 755 "$src/bend-jsrt"
  ID1="t1$(git -C "$REPO" rev-parse --short HEAD | cut -c1-5)"
  printf 'id=%s\ncommit=%s\nsubject=test-release %s\nbuilt=2026-01-01T00:00:01Z\nmacos=14.0\ntarget=%s\nchannel=test\n' \
    "$ID1" "$(git -C "$REPO" rev-parse HEAD)" "$ID1" "$target" > "$src/VERSION"
fi
ID2="$ID1-r2" ID3="$ID1-r3"
# a tarball of the app as version <id>, built at <date>
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
check "install.sh has the channel stamped" grep -q "^DIST_URL_DEFAULT='file://$REL'" "$REL/install.sh"
check "latest.json names this target" plutil -extract "targets.$target.sha256" raw -o - "$REL/latest.json"

echo "== curl | sh (file:// channel)"
(cd "$WORK" && E sh -c "curl -fsSL file://$REL/install.sh | sh") 2>&1 | sed 's/^/     /'
P="$T/.local/share/bise" BIN="$T/.local/bin/bise"
check "prefix ~/.local/share/bise" test -d "$P/versions/$ID1"
check "current -> versions/$ID1" test "$(readlink "$P/current")" = "versions/$ID1"
check "LICENSE, NOTICE, THIRD_PARTY_NOTICES installed" sh -c "grep -q 'Apache License' '$P/versions/$ID1/LICENSE' && grep -q 'Gabriel Vergnaud' '$P/versions/$ID1/NOTICE' && grep -q 'third-party notices' '$P/versions/$ID1/THIRD_PARTY_NOTICES'"
check "~/.local/bin/bise -> the launcher" test "$(readlink "$BIN")" = "$P/bin/bise"
check "the channel is recorded" test "$(cat "$P/dist-url")" = "file://$REL"
check "the launcher only execs current" sh -c "[ \$(grep -vc '^#' '$P/bin/bise') -le 5 ]"
check "PATH line in ~/.zshrc" grep -q 'added by the bise installer' "$T/.zshrc"
found="$(E /bin/zsh -ic 'command -v bise' 2>/dev/null | tail -n 1)"
[ "$found" = "$BIN" ] && ok "a new shell finds bise" || ko "a new shell finds bise (got '$found')"
v="$(cd "$WORK" && E "$BIN" --version 2>&1)"
case "$v" in "bise $ID1 ($target"*) ok "--version: $v" ;; *) ko "--version: $v" ;; esac
out="$(E BISE_NO_UPDATE=1 "$BIN" update --check 2>&1)"
case "$out" in *"$ID1 is up to date"*) ok "update --check: up to date" ;; *) ko "update --check: $out" ;; esac

echo "== a hub on release 1"
mkdir -p "$WS" && (cd "$WS" && git init -q && echo x > README.md && git add README.md \
  && git -c user.name=t -c user.email=t@t -c commit.gpgsign=false commit -qm init)
(cd "$WS" && E BISE_NO_UPDATE=1 MISTRAL_API_KEY=fake "$BIN" sbd --workspace "$WS" </dev/null >/dev/null 2>"$WORK/hub.err" &)
state=""
hub_up() { state="$(ls -d "$T"/.bise/hubs/pr-ws-* 2>/dev/null | head -n 1)"; [ -n "$state" ] && [ -S "$state/hub.sock" ]; }
if until_ok 150 hub_up; then ok "hub started"; else ko "hub did not start"; tail -n 5 "$WORK/hub.err"; fi
root_is() { [ "$(cat "$state/hub.root" 2>/dev/null)" = "$(cd "$P/versions/$1" && pwd -P)" ]; }
check "hub runs versions/$ID1" root_is "$ID1"
sbm() { E SB_SOCKET="$state/hub.sock" SB_AGENT=main "$state/bin/sb" "$@" 2>&1; }
lst="$(sbm version)"
echo "$lst" | grep -q "●★ $ID1" && ok "/version lists the installed version (●★ $ID1)" || { ko "/version list"; echo "$lst" | sed 's/^/     /'; }

echo "== release 2: the daily background check installs it"
publish "$t2" 0.0.2 || ko "publish 2"
out="$(E "$BIN" update --check 2>&1)"
case "$out" in *"$ID2 is available"*) ok "update --check: $ID2 available" ;; *) ko "update --check: $out" ;; esac
# a session start with the check due (interval 0): detached, never waits
d="$(mktemp -d /tmp/pr-sess.XXXXXX)"; mkfifo "$d/in"
(cd "$WORK" && E BISE_UPDATE_INTERVAL=0 "$BIN" --headless --scripted < "$d/in" > "$d/out" 2> "$d/err") &
spid=$!; exec 3> "$d/in"
until_ok 300 grep -q '^READY' "$d/out" && ok "session starts (READY)" || { ko "session: no READY"; tail -n 3 "$d/err"; }
exec 3>&-; until_ok 100 sh -c "! kill -0 $spid"; kill $spid 2>/dev/null; rm -rf "$d"
cur_is() { [ "$(readlink "$P/current")" = "versions/$1" ]; }
until_ok 200 cur_is "$ID2" && ok "background check: current -> $ID2" || { ko "background check: current is $(readlink "$P/current")"; tail -n 3 "$T/.bise/cache/update.log"; }
check "update.log says so" grep -q "updated to $ID2" "$T/.bise/cache/update.log"
check "the hub still runs $ID1 (never switched silently)" root_is "$ID1"
lst="$(sbm version)"
echo "$lst" | grep -q " ★ $ID2" && ok "/version: $ID2 is current (★), $ID1 running" || { ko "/version after update"; echo "$lst" | sed 's/^/     /'; }

echo "== sb restart latest: the hub switches to $ID2"
sbm restart latest | sed 's/^/     /'
until_ok 300 root_is "$ID2" && ok "hub runs versions/$ID2" || ko "hub root: $(cat "$state/hub.root" 2>/dev/null)"

echo "== release 3: bise update"
publish "$t3" 0.0.3 || ko "publish 3"
out="$(E BISE_NO_UPDATE=1 "$BIN" update 2>&1)"; echo "$out" | sed 's/^/     /'
check "current -> $ID3" cur_is "$ID3"
v="$(E BISE_NO_UPDATE=1 "$BIN" --version 2>&1)"
case "$v" in "bise $ID3 "*) ok "--version: $ID3" ;; *) ko "--version: $v" ;; esac
check "the running $ID2 and the older $ID1 are kept (keep 3)" test -d "$P/versions/$ID1" -a -d "$P/versions/$ID2"

echo "== a bad checksum is refused"
sed -i '' "s/\"sha256\": \"[0-9a-f]*\"/\"sha256\": \"$(printf '0%.0s' $(seq 64))\"/" "$REL/latest.json"
sed -i '' "s/\"id\": \"$ID3\"/\"id\": \"$ID1-r4\"/g; s/\"built\": \"[^\"]*\"/\"built\": \"2026-01-04T00:00:00Z\"/g" "$REL/latest.json"
out="$(E BISE_NO_UPDATE=1 "$BIN" update 2>&1)"
case "$out" in *"checksum mismatch"*) ok "update refused: checksum mismatch" ;; *) ko "bad checksum: $out" ;; esac
check "current unchanged ($ID3)" cur_is "$ID3"
check "no download left behind" sh -c "! ls -a '$P' | grep -q '^\.update-'"

echo "== stop, uninstall"
# the switch to $ID2 is still on probation (2 min): a hub stopped now
# would be rolled back and restarted by the switcher; end it first
for p in $(ps -axo pid=,command= | grep -F "$T/.local/share/" | grep -F " sbswitch " | grep -v grep | awk '{print $1}'); do kill "$p" 2>/dev/null; done
pid="$(cat "$state/hub.pid" 2>/dev/null)"
(cd "$WS" && E "$BIN" switchboard --stop --workspace "$WS") 2>&1 | sed 's/^/     /'
[ -n "$pid" ] && until_ok 50 sh -c "! kill -0 $pid 2>/dev/null" && ok "hub stopped" || ko "hub still running (pid '$pid')"
E "$BIN" uninstall 2>&1 | sed 's/^/     /'
check "prefix removed" test ! -e "$P"
check "command link removed" test ! -e "$BIN"
check "PATH line removed" sh -c "! grep -q 'added by the bise installer' '$T/.zshrc'"
check "user data kept (~/.bise)" test -d "$T/.bise/hubs"

for p in $(ps -axo pid=,command= | grep -F "$T/.local/share/" | grep -v grep | awk '{print $1}'); do kill "$p" 2>/dev/null; done
echo "== $pass passed, $fail failed"
[ "$fail" = 0 ]
