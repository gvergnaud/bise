#!/usr/bin/env bash
# test-install.sh — install a tarball from build-dist.sh into a CLEAN
# fake HOME and check it runs. Everything happens under /tmp: the real
# ~/.bend-harness, ~/.bise, ~/.local/state/switchboard and live hubs are never touched.
#
#   packaging/test-install.sh <tarball>
#
# Checks: install (from the extracted bundle, like curl | sh would),
# the command on PATH in a new login shell, --version (this Mac's arch),
# login (key file), a single-agent session start (--headless, scripted
# and live), one headless turn answered by tests/fake_provider.py (the
# installed REPL calls a provider and answers; needs python3, nothing
# leaves the machine), the Switchboard hub start / sb list / stop on a
# throwaway git workspace, a reinstall (idempotent), then uninstall
# (data kept). Exit 0 when every check passed (CI runs it on each arch).
#
# BISE_CMD: the command to test (the CI workflow's one variable for the
# name, BISE-165); default: bise when the bundle ships it, else
# bend-harness.

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
tarball="${1:?usage: test-install.sh <tarball>}"
tarball="$(cd "$(dirname "$tarball")" && pwd)/$(basename "$tarball")"
T=/tmp/pk-home
WS=/tmp/pk-ws
DL=/tmp/pk-dl
WORK=/tmp/pk-work

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $*"; }
ko()  { fail=$((fail + 1)); echo "  FAIL $*"; }
check() { local what="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$what"; else ko "$what"; fi; }

# a clean environment: fake HOME, system PATH only, zsh as the shell
E() { env -i HOME="$T" PATH=/usr/bin:/bin:/usr/sbin:/sbin SHELL=/bin/zsh TERM=dumb \
        USER="${USER:-me}" LANG=en_US.UTF-8 "$@"; }

# leftovers of an earlier run (a hub of the fake HOME only)
for p in $(ps -axo pid=,command= | grep -F "$T/.local/share/" | grep -v grep | awk '{print $1}'); do kill "$p" 2>/dev/null; done
rm -rf "$T" "$WS" "$DL" "$WORK"; mkdir -p "$T" "$DL" "$WORK"

echo "== install"
tar -C "$DL" -xzf "$tarball"
bundle="$(ls -d "$DL"/*/)"
CMD="${BISE_CMD:-}"
if [ -z "$CMD" ]; then if [ -e "$bundle/app/bise" ]; then CMD=bise; else CMD=bend-harness; fi; fi
arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
os="$(uname -s | tr '[:upper:]' '[:lower:]')"
# a file's permission bits: BSD stat (macOS), GNU stat (Linux; its -f
# means the file system and prints before it fails: no `||` fallback)
mode_of() { if [ "$os" = darwin ]; then stat -f %Lp "$1"; else stat -c %a "$1"; fi; }
# the installer's PATH line: "# added by the <name> installer"
MARK='added by the [a-z-]* installer'
E sh "$bundle/install.sh" 2>&1 | sed 's/^/     /'
BIN="$T/.local/bin/$CMD"
check "launcher linked in ~/.local/bin" test -x "$BIN"
check "old name linked too (bend-harness)" test -x "$T/.local/bin/bend-harness"
check "PATH line in ~/.zshrc" grep -q "$MARK" "$T/.zshrc"
found="$(E /bin/zsh -ic "command -v $CMD" 2>/dev/null | tail -n 1)"
[ "$found" = "$BIN" ] && ok "new shell finds $CMD ($found)" || ko "new shell finds $CMD (got '$found')"

echo "== --version"
v="$(cd "$WORK" && E /bin/zsh -ic "$CMD --version" 2>&1 | tail -n 1)"
echo "     $v"
case "$v" in "$CMD "*"$os-$arch"*) ok "--version ($os-$arch)" ;; *) ko "--version: want '$CMD <id> ($os-$arch, ...)'" ;; esac

echo "== doctor's PATH line (BISE-270: the launcher names the version it runs)"
d="$(cd "$WORK" && E /bin/zsh -ic "$CMD doctor" 2>&1 | grep 'PATH' | head -n 1)"
echo "     $d"
case "$d" in *"is a launcher that runs this bise"*) ok "doctor: the launcher runs this bise" ;; *) ko "doctor's PATH line" ;; esac

echo "== login (a key from stdin, BISE-170: no init in the launcher)"
(cd "$WORK" && printf 'test-key-not-real\n' | E "$BIN" login mistral) 2>&1 | sed 's/^/     /'
check "~/.bise/auth.json holds the key" grep -q 'test-key-not-real' "$T/.bise/auth.json"
[ "$(mode_of "$T/.bise/auth.json")" = 600 ] && ok "auth.json is mode 600" || ko "auth.json is mode 600"
# Apache-2.0 4(a)/(d): the license and the notices go with the binaries
cur="$T/.local/share/bise/current"
check "LICENSE (Apache-2.0) installed" grep -q 'Apache License' "$cur/LICENSE"
check "NOTICE installed" grep -q 'Gabriel Vergnaud' "$cur/NOTICE"
check "THIRD_PARTY_NOTICES installed" grep -q 'third-party notices' "$cur/THIRD_PARTY_NOTICES"
check "the launcher only execs current (no logic left in it)" sh -c "[ \$(grep -vc '^#' '$T/.local/share/bise/bin/$CMD') -le 5 ]"

# a headless session: READY on stdout, then close stdin to end it
# (NOMODEL_TURN=1: first one turn, which must say "no model yet")
session() {
  local label="$1"; shift
  local d; d="$(mktemp -d /tmp/pk-sess.XXXXXX)"
  mkfifo "$d/in"
  (cd "$WORK" && E "$BIN" --headless "$@" < "$d/in" > "$d/out" 2> "$d/err") &
  local pid=$!
  exec 3> "$d/in"
  local i=0
  while [ $i -lt 300 ] && ! grep -q '^READY' "$d/out" 2>/dev/null; do
    kill -0 $pid 2>/dev/null || break; sleep 0.1; i=$((i + 1))
  done
  if grep -q '^READY' "$d/out"; then
    ok "$label: $(grep '^READY' "$d/out" | cut -c1-90)…"
    [ -n "${NOMODEL_TURN:-}" ] && nomodel_turn "$label" "$(sed -n 's/^READY port=\([0-9]*\).*/\1/p' "$d/out")"
  else
    ko "$label: no READY"; sed 's/^/     /' "$d/err" | tail -n 5
  fi
  exec 3>&-
  i=0; while kill -0 $pid 2>/dev/null && [ $i -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
  kill -0 $pid 2>/dev/null && { ko "$label: still running after stdin closed"; kill $pid; } || ok "$label: exits when stdin closes"
  rm -rf "$d"
}
# a fresh install has no model (BISE-266): the session starts, and a
# turn answers with how to pick one, no provider call (BISE-280)
nomodel_turn() {
  local label="$1" port="$2" line got=""
  if [ -n "$port" ] && exec 4<>"/dev/tcp/127.0.0.1/$port"; then
    printf '%s\n' "hello" >&4
    while IFS= read -r -t 30 line <&4; do
      case "$line" in *"no model yet"*) got=1 ;; "--- idle"*) break ;; esac
    done
    exec 4<&-
  fi
  [ -n "$got" ] && ok "$label: a turn says 'no model yet'" || ko "$label: a turn did not say 'no model yet'"
}
echo "== single-agent session"
session "scripted session" --scripted
NOMODEL_TURN=1 session "live session (no model yet)"
check "sessions dir created in ~/.bise (BISE-161: a fresh HOME starts there)" test -d "$T/.bise/sessions"

# one turn through the installed REPL: the provider is the tests' fake
# (openai-chat), which answers "ack: <the message>"
echo "== a headless turn (fake provider)"
turn() {
  local d fpid port pid i line got="" msg="hello from test-install"
  d="$(mktemp -d /tmp/pk-turn.XXXXXX)"
  FAKE_LOG="$d/fake.log" python3 -u "$HERE/../tests/fake_provider.py" > "$d/fake.out" 2> "$d/fake.err" &
  fpid=$!
  # 30 s: a cold runner is slow; it exits early when python dies
  i=0; while [ $i -lt 300 ] && ! grep -q '^PORT ' "$d/fake.out" 2>/dev/null; do
    kill -0 $fpid 2>/dev/null || break; sleep 0.1; i=$((i + 1)); done
  port="$(sed -n 's/^PORT //p' "$d/fake.out")"
  if [ -z "$port" ]; then
    # the failure explains itself: which python, alive or not, its output
    kill -0 $fpid 2>/dev/null && state="still running after $((i / 10)) s" || state="exited"
    ko "fake provider did not start ($state; $(command -v python3): $(python3 --version 2>&1))"
    echo "     stdout:"; sed 's/^/       /' "$d/fake.out"
    echo "     stderr:"; sed 's/^/       /' "$d/fake.err"
    kill $fpid 2>/dev/null; wait $fpid 2>/dev/null; rm -rf "$d"; return
  fi
  mkfifo "$d/in"
  (cd "$WORK" && E BEND_PROVIDER_URL="http://127.0.0.1:$port/v1/chat/completions" \
     MISTRAL_API_KEY=fake-key "$BIN" --headless --model mistral-small-latest \
     < "$d/in" > "$d/out" 2> "$d/err") &
  pid=$!
  exec 3> "$d/in"
  i=0; while [ $i -lt 300 ] && ! grep -q '^READY' "$d/out" 2>/dev/null; do
    kill -0 $pid 2>/dev/null || break; sleep 0.1; i=$((i + 1)); done
  port="$(sed -n 's/^READY port=\([0-9]*\).*/\1/p' "$d/out")"
  if [ -n "$port" ] && exec 4<>"/dev/tcp/127.0.0.1/$port"; then
    printf '%s\n' "$msg" >&4
    while IFS= read -r -t 60 line <&4; do
      echo "$line" >> "$d/turn"
      case "$line" in *"ack: $msg"*) got=1 ;; "--- idle"*) break ;; esac
    done
    exec 4<&-
    [ -n "$got" ] && ok "turn answered: ack: $msg" || { ko "turn: no 'ack: $msg'"; tail -n 8 "$d/turn" 2>/dev/null | sed 's/^/     /'; }
    [ "$(wc -l < "$d/fake.log" 2>/dev/null | tr -d ' ')" = 1 ] && ok "the fake provider got one request" || ko "the fake provider got $(wc -l < "$d/fake.log" 2>/dev/null | tr -d ' ') requests (want 1)"
  else
    ko "turn: no READY"; tail -n 5 "$d/err" | sed 's/^/     /'
  fi
  exec 3>&-
  i=0; while kill -0 $pid 2>/dev/null && [ $i -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
  kill $pid $fpid 2>/dev/null; wait $pid $fpid 2>/dev/null
  rm -rf "$d"
}
turn

echo "== Switchboard hub on a throwaway workspace"
mkdir -p "$WS" && (cd "$WS" && git init -q && echo x > README.md && git add README.md \
  && git -c user.name=t -c user.email=t@t -c commit.gpgsign=false commit -qm init)
(cd "$WS" && E "$BIN" sbd --workspace "$WS" </dev/null >/dev/null 2>"$DL/hub.err" &)
state=""; i=0
while [ $i -lt 150 ]; do
  state="$(ls -d "$T"/.bise/hubs/pk-ws-* 2>/dev/null | head -n 1)"
  [ -n "$state" ] && [ -S "$state/hub.sock" ] && break
  sleep 0.1; i=$((i + 1))
done
if [ -n "$state" ] && [ -S "$state/hub.sock" ]; then
  ok "hub started (state $state)"
  root="$(cat "$state/hub.root" 2>/dev/null)"
  case "$root" in "$(cd "$T" && pwd -P)/.local/share/bise/versions/"*) ok "hub runs from the installed version ($root)" ;; *) ko "hub root: $root" ;; esac
  sleep 2
  out="$(E SB_SOCKET="$state/agent.sock" SB_AGENT=main "$state/bin/sb" list 2>&1)"
  echo "$out" | sed 's/^/     /' | head -n 5
  echo "$out" | grep -q main && ok "sb list (through the agents' link) answers" || ko "sb list"
  if [ -L "$state/bin/sb" ]; then ok "bin/sb is a link to bise"; else ko "bin/sb is not a link"; fi
  ps -axo command= | grep -F "$root/repl-live" | grep -v grep >/dev/null && ok "main agent's repl-live runs from the version dir" || ko "main agent's repl-live not running"
  (cd "$WS" && E "$BIN" switchboard --stop --workspace "$WS") 2>&1 | sed 's/^/     /'
  i=0; while [ -e "$state/hub.sock" ] && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
  pid="$(cat "$state/hub.pid" 2>/dev/null)"
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then ko "hub still running after --stop"; else ok "hub stopped"; fi
  ps -axo command= | grep -F "$root/repl-live" | grep -v grep >/dev/null && ko "agent REPLs left behind" || ok "no agent REPL left"
else
  ko "hub did not start"; tail -n 5 "$DL/hub.err"
fi

echo "== reinstall (same version: idempotent)"
E sh "$bundle/install.sh" 2>&1 | sed 's/^/     /'
[ "$(grep -c "$MARK" "$T/.zshrc")" = 1 ] && ok "one PATH line only" || ko "PATH line duplicated"

echo "== uninstall"
E "$BIN" uninstall 2>&1 | sed 's/^/     /'
check "prefix removed" test ! -e "$T/.local/share/bise"
check "command link removed" test ! -e "$BIN"
check "old-name link removed" test ! -e "$T/.local/bin/bend-harness"
check "PATH line removed" sh -c "! grep -q '$MARK' '$T/.zshrc'"
check "user data kept (~/.bise/auth.json)" test -f "$T/.bise/auth.json"

echo "== $pass passed, $fail failed"
[ "$fail" = 0 ]
