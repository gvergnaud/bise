#!/usr/bin/env bash
# test-nix.sh — install a linux tarball from build-dist.sh on NixOS (or
# Nix on any Linux) through this repo's flake, in a throwaway HOME, and
# check it runs: the path Cyril takes (docs/nixos.md), before a release
# exists. Run it on a clean NixOS (a VM, an OrbStack machine):
#
#   nix-shell -p git tmux python3 --run 'packaging/test-nix.sh <tarball>'
#
# The flake is copied with nix/sources.json pointing at the tarball
# (file://), then `nix profile install` from it: nix/package.nix's
# autoPatchelfHook and OpenSSL RUNPATH are what make a glibc build start
# without /lib64/ld-linux. Checks: no unresolved library, OpenSSL in the
# RUNPATH, --version (linux-<arch>), the store root is read-only and bise
# says who updates it (`bise update`), install.sh refuses NixOS without
# nix-ld and names the flake, a headless turn on tests/fake_provider.py,
# an HTTPS request reaches TLS (BISE_TEST_TLS=0 skips it: it needs the
# network; a fake key, a 401 is the pass), then the TUI in tmux: it
# starts, reaches the composer (key from the env), and runs a turn with a
# bash tool call; the hub's log has no read-only (EROFS) error; stop.
# Nothing touches the real HOME: everything is under $W.

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
tarball="${1:?usage: test-nix.sh <linux tarball>}"
tarball="$(cd "$(dirname "$tarball")" && pwd)/$(basename "$tarball")"
W="${BISE_NIX_TEST_DIR:-/tmp/bise-nix-test}"
H="$W/home" WS="$W/ws" F="$W/bise"  # F: the profile names the element after it, as github:gvergnaud/bise
NIX=(nix --extra-experimental-features "nix-command flakes")

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $*"; }
ko()  { fail=$((fail + 1)); echo "  FAIL $*"; }
check() { local what="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$what"; else ko "$what"; fi; }
has() { case "$1" in *"$2"*) return 0 ;; esac; return 1; }

for c in nix git tmux python3; do command -v "$c" >/dev/null || { echo "test-nix: needs $c (nix-shell -p git tmux python3)"; exit 2; }; done
tmux -L bisenix kill-server 2>/dev/null
rm -rf "$W"; mkdir -p "$H" "$WS" "$F/nix"

# a clean user: the throwaway HOME, the system profile's PATH + its own
# nix profile, no key in any file (the key comes from the env)
E() { env -i HOME="$H" USER="${USER:-me}" SHELL=/run/current-system/sw/bin/bash TERM=xterm-256color LANG=C.UTF-8 \
  PATH="$H/.nix-profile/bin:$H/.local/state/nix/profile/bin:/run/wrappers/bin:/run/current-system/sw/bin:$(dirname "$(command -v tmux)"):$(dirname "$(command -v git)")" \
  NIX_PATH="${NIX_PATH:-}" "$@"; }

echo "== the flake, with sources.json on this tarball"
system="$("${NIX[@]}" eval --impure --raw --expr builtins.currentSystem)"
case "$system" in x86_64-linux) target=linux-x86_64 ;; aarch64-linux) target=linux-arm64 ;; *) echo "test-nix: $system"; exit 2 ;; esac
case "$(basename "$tarball")" in *"-$target.tar.gz") ;; *) echo "test-nix: $(basename "$tarball") is not $target"; exit 2 ;; esac
cp "$REPO/flake.nix" "$REPO/default.nix" "$F/"
[ -f "$REPO/flake.lock" ] && cp "$REPO/flake.lock" "$F/"
cp "$REPO/nix/package.nix" "$F/nix/"
id="$(tar -xzOf "$tarball" "$(basename "$tarball" .tar.gz)/app/VERSION" | sed -n 's/^id=//p')"
sum="$(sha256sum "$tarball" | cut -d' ' -f1)"
printf '{\n  "version": "%s",\n  "id": "%s",\n  "built": "",\n  "%s": {"url": "file://%s", "sha256": "%s"}\n}\n' \
  "$id" "$id" "$system" "$tarball" "$sum" > "$F/nix/sources.json"
(cd "$F" && git init -q && git add -A) # a path: flake reads every file; git keeps it a clean tree
# fetchurl's build sandbox cannot read a local file: put it in the store
# first (the same fixed-output path fetchurl would make)
E "${NIX[@]}" store prefetch-file --hash-type sha256 "file://$tarball" >/dev/null 2>&1 \
  || { echo "test-nix: nix store prefetch-file failed"; exit 2; }

echo "== nix profile install (the documented install, from this flake)"
if out="$(E "${NIX[@]}" profile install "path:$F" 2>&1)"; then ok "nix profile install"; else ko "nix profile install"; echo "$out" | tail -n 15 | sed 's/^/     /'; fi
BIN="$(E sh -c 'command -v bise')"
[ -n "$BIN" ] && ok "bise on PATH: $BIN" || { ko "no bise on PATH"; echo "== $pass passed, $fail failed"; exit 1; }
root="$(dirname "$(readlink -f "$BIN")")"
case "$root" in /nix/store/*/lib/bise) ok "app root in the store: $root" ;; *) ko "app root: $root" ;; esac

echo "== the binaries: libraries and OpenSSL"
missing=""
for f in "$root"/*; do
  [ -f "$f" ] && [ ! -L "$f" ] && head -c 4 "$f" | grep -q ELF || continue
  m="$(ldd "$f" 2>&1 | grep -F 'not found')" && missing="$missing $(basename "$f"):$m"
done
[ -z "$missing" ] && ok "every binary finds its libraries (ldd)" || ko "unresolved:$missing"
rp="$(E "${NIX[@]}" shell nixpkgs#patchelf -c patchelf --print-rpath "$root/repl-live" 2>/dev/null)"
ssl=""; for d in $(echo "$rp" | tr ':' ' '); do [ -e "$d/libssl.so.3" ] && ssl="$d"; done
[ -n "$ssl" ] && ok "repl-live's RUNPATH has libssl.so.3 (wire.c dlopens it): $ssl" || ko "no libssl.so.3 in repl-live's RUNPATH: $rp"
v="$(E "$BIN" --version 2>&1)"
has "$v" "$target" && ok "--version: $v" || ko "--version: $v"
check "the app root is read-only" sh -c "! touch '$root/.w' 2>/dev/null"
out="$(E "$BIN" update 2>&1)"
has "$out" "installed with Nix" && ok "bise update names Nix: $(echo "$out" | tail -n 1)" || ko "bise update: $out"

echo "== install.sh on NixOS without nix-ld"
out="$(E sh "$REPO/packaging/install.sh" --from "$tarball" 2>&1)"; rc=$?
[ $rc != 0 ] && has "$out" "nix profile install github:gvergnaud/bise" && ok "install.sh refuses and names the flake" || ko "install.sh ($rc): $out"

echo "== a headless turn (fake provider)"
fake() {  # <dir>: start tests/fake_provider.py, echo its port
  FAKE_LOG="$1/fake.log" python3 -u "$REPO/tests/fake_provider.py" > "$1/fake.out" 2> "$1/fake.err" &
  echo $! > "$1/fake.pid"
  local i=0; while [ $i -lt 100 ] && ! grep -q '^PORT ' "$1/fake.out" 2>/dev/null; do sleep 0.1; i=$((i + 1)); done
  sed -n 's/^PORT //p' "$1/fake.out"
}
D="$W/turn"; mkdir -p "$D" "$W/work"; port="$(fake "$D")"
msg="hello from nixos"
mkfifo "$D/in"
(cd "$W/work" && E BEND_PROVIDER_URL="http://127.0.0.1:$port/v1/chat/completions" MISTRAL_API_KEY=fake-key \
   "$BIN" --headless --model mistral-small-latest < "$D/in" > "$D/out" 2> "$D/err") &
pid=$!; exec 3> "$D/in"
i=0; while [ $i -lt 300 ] && ! grep -q '^READY' "$D/out" 2>/dev/null; do kill -0 $pid 2>/dev/null || break; sleep 0.1; i=$((i + 1)); done
rport="$(sed -n 's/^READY port=\([0-9]*\).*/\1/p' "$D/out")"
got=""
if [ -n "$rport" ] && exec 4<>"/dev/tcp/127.0.0.1/$rport"; then
  printf '%s\n' "$msg" >&4
  while IFS= read -r -t 60 line <&4; do case "$line" in *"ack: $msg"*) got=1 ;; "--- idle"*) break ;; esac; done
  exec 4<&-
fi
[ -n "$got" ] && ok "turn answered: ack: $msg" || { ko "headless turn"; tail -n 5 "$D/err" | sed 's/^/     /'; }
exec 3>&-; sleep 1; kill $pid "$(cat "$D/fake.pid")" 2>/dev/null; wait $pid 2>/dev/null

if [ "${BISE_TEST_TLS:-1}" = 1 ]; then
  echo "== HTTPS: the store's OpenSSL (a fake key: 401 is the pass)"
  D="$W/tls"; mkdir -p "$D"; mkfifo "$D/in"
  (cd "$W/work" && E BEND_PROVIDER_URL="https://api.mistral.ai/v1/chat/completions" MISTRAL_API_KEY=fake-key \
     "$BIN" --headless --model mistral-small-latest < "$D/in" > "$D/out" 2> "$D/err") &
  pid=$!; exec 3> "$D/in"
  i=0; while [ $i -lt 300 ] && ! grep -q '^READY' "$D/out" 2>/dev/null; do kill -0 $pid 2>/dev/null || break; sleep 0.1; i=$((i + 1)); done
  rport="$(sed -n 's/^READY port=\([0-9]*\).*/\1/p' "$D/out")"; t=""
  if [ -n "$rport" ] && exec 4<>"/dev/tcp/127.0.0.1/$rport"; then
    printf 'hi\n' >&4
    while IFS= read -r -t 60 line <&4; do t="$t$line
"; case "$line" in "--- idle"*) break ;; esac; done
    exec 4<&-
  fi
  if has "$t" "401" || has "$t" "nauthorized"; then ok "TLS works: api.mistral.ai answered 401"
  else ko "TLS: $(printf '%s' "$t" | grep -iE 'tls|ssl|error|401' | head -n 3)"; fi
  exec 3>&-; sleep 1; kill $pid 2>/dev/null; wait $pid 2>/dev/null
fi

echo "== the TUI (tmux): start, composer, a turn with a tool call"
(cd "$WS" && git init -q && git config user.email t@t && git config user.name t && echo x > README && git add README && git commit -qm init)
D="$W/tui"; mkdir -p "$D"; port="$(fake "$D")"
mark="nix-tool-ok-$$"
T() { tmux -L bisenix "$@"; }
E tmux -L bisenix new-session -d -s b -x 150 -y 42 -c "$WS" \
  "env BEND_PROVIDER_URL=http://127.0.0.1:$port/v1/chat/completions BEND_MODEL=mistral-small-latest MISTRAL_API_KEY=fake-key SB_ONBOARDING=off BISE_APPROVALS=yolo bise; sleep 600"
screen() { T capture-pane -p -t b 2>/dev/null; }
waitfor() {  # <regex> <seconds>
  local i=0; while [ $i -lt $(($2 * 5)) ]; do screen | grep -Eq "$1" && return 0; sleep 0.2; i=$((i + 1)); done; return 1; }
if waitfor 'you → main|0 ○ main' 60; then ok "bise started, main's composer is there"; else ko "no composer"; screen | tail -n 15 | sed 's/^/     /'; fi
T send-keys -t b -l "[[bash: echo $mark]]"; sleep 0.3; T send-keys -t b Enter
if waitfor "done: .*$mark" 90; then ok "a turn with a bash tool call: $(screen | grep -o "done: .*$mark")"; else ko "no tool turn"; screen | tail -n 20 | sed 's/^/     /'; fi
grep -q "$mark" "$D/fake.log" 2>/dev/null && ok "the fake provider got the tool result" || ko "fake log has no $mark"
screen > "$W/tui-screen.txt"
hub="$(ls -d "$H"/.bise/hubs/ws-* 2>/dev/null | head -n 1)"
if [ -n "$hub" ]; then
  if grep -E 'EROFS|Read-only file system|ermission denied' "$hub"/*.log >/dev/null 2>&1; then
    ko "the hub log has a read-only/permission error: $(grep -hE 'EROFS|Read-only|ermission denied' "$hub"/*.log | head -n 2)"
  else ok "no read-only or permission error in the hub's logs"; fi
else ko "no hub state dir under $H/.bise/hubs"; fi
(cd "$WS" && E "$BIN" switchboard --stop --workspace "$WS") >/dev/null 2>&1
T kill-server 2>/dev/null; kill "$(cat "$D/fake.pid")" 2>/dev/null

echo "== uninstall"
E "${NIX[@]}" profile remove bise >/dev/null 2>&1
check "bise gone from the profile" test ! -e "$H/.nix-profile/bin/bise"

echo "== $pass passed, $fail failed (screen: $W/tui-screen.txt)"
[ "$fail" = 0 ]
