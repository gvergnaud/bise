#!/usr/bin/env bash
# test-release-gh.sh — the GitHub Releases channel (BISE-217) end to end,
# public and private, against a local stand-in of GitHub: a python server
# with the release download URLs (`/<o>/<r>/releases/latest/download/`),
# the API (`/api/repos/<o>/<r>/releases/latest`, assets answered with a
# redirect to another host, as GitHub does) and bise.dev/install; a stub
# `gh` for the GitHub CLI. All under /tmp with `env -i` and fake HOMEs.
#
#   packaging/test-release-gh.sh
#
# Public: `curl .../install | sh` and `bise update` read the plain URLs,
# no gh, no token. Private (the download URLs answer 404, like GitHub
# without auth): no gh and no token -> a clear 'gh auth login' error,
# nothing installed; gh not logged in -> the same; gh logged in -> install,
# `update --check`, `update --background` (the daily check's command);
# GH_TOKEN and no gh -> install and update through the API; a bad token
# -> the error. The app is this tree's debug `bise` with stub REPLs and
# engine (no hub runs here: test-release.sh covers the hub).
#
# CI is the one publisher (BISE-220): ci-release.sh (release.yml's release
# job) on stub archives of both arches -> check-release.py passes, and
# fails on a bad sha256, a missing arch, an unstamped install.sh, an extra
# file; a draft rerun replaces the files, a published release is refused.
# publish-release.sh against a stateful stub `gh` (releases, runs: `gh run
# watch` runs ci-release.sh, the "CI") and a throwaway git repo whose
# remote is github.com/o/r (push goes to a local bare repo): an unpushed
# commit is refused, --dry-run changes nothing, then tag + push + watch +
# draft, `--publish` flips it, the published files install and update;
# a failed run says so; --local lays out and checks the release.

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE" && git rev-parse --show-toplevel)"
W=/tmp/prg REL=/tmp/prg/rel PK=/tmp/prg/pack SRV=/tmp/prg/srv STUB=/tmp/prg/stub
TOKEN=tok-good

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $*"; }
ko()  { fail=$((fail + 1)); echo "  FAIL $*"; }
check() { local what="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$what"; else ko "$what"; fi; }
has() { case "$1" in *"$2"*) return 0 ;; esac; return 1; }

[ -f "$W/srv.pid" ] && kill "$(cat "$W/srv.pid")" 2>/dev/null
rm -rf "$W"; mkdir -p "$REL" "$PK" "$SRV" "$STUB/bin"

os="$(uname -s | tr '[:upper:]' '[:lower:]')"; arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
target="$os-$arch"

echo "== the app: this tree's bise, stub REPLs and engine"
(cd "$REPO/rust" && cargo build -q -p bend-harness) || { echo "cargo build failed"; exit 1; }
src="$PK/src"; mkdir -p "$src"
cp "${CARGO_TARGET_DIR:-$REPO/rust/target}/debug/bise" "$src/bise"
for b in repl-live repl-scripted sb-core bend-jsrt; do printf '#!/bin/sh\nexit 1\n' > "$src/$b"; chmod 755 "$src/$b"; done
pack() {  # <id> <built> [<target> <commit> <dir>]: a build-dist.sh-shaped archive of the app
  local tg="${3:-$target}" cm="${4:-$(git -C "$REPO" rev-parse HEAD)}" o="${5:-$PK}"
  local d="$o/bise-$1-$tg"
  rm -rf "$d"; mkdir -p "$d"; cp -cR "$src" "$d/app" 2>/dev/null || cp -R "$src" "$d/app"
  printf 'id=%s\ncommit=%s\nsubject=release %s\nbuilt=%s\nmacos=14.0\ntarget=%s\nchannel=test\n' \
    "$1" "$cm" "$1" "$2" "$tg" > "$d/app/VERSION"
  cp "$HERE/install.sh" "$d/install.sh"
  tar -C "$o" -czf "$o/bise-$1-$tg.tar.gz" "bise-$1-$tg"
  (cd "$o" && shasum -a 256 "bise-$1-$tg.tar.gz" > "bise-$1-$tg.tar.gz.sha256")
  rm -rf "$d"
  echo "$o/bise-$1-$tg.tar.gz"
}

echo "== a stand-in GitHub"
cat > "$W/srv.py" <<'EOF'
import http.server, json, os, sys
REL, SRV, TOKEN = sys.argv[1], sys.argv[2], sys.argv[3]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def send(self, code, body=b"", ctype="application/octet-stream", loc=None):
        self.send_response(code)
        if loc: self.send_header("Location", loc)
        self.send_header("Content-Type", ctype); self.send_header("Content-Length", str(len(body)))
        self.end_headers(); self.wfile.write(body)
    def file(self, name):
        p = os.path.join(REL, os.path.basename(name))
        if not os.path.isfile(p): return self.send(404, b"Not Found")
        self.send(200, open(p, "rb").read())
    def do_GET(self):
        auth = self.headers.get("Authorization", "")
        with open(os.path.join(SRV, "log"), "a") as f: f.write(f"{self.path} auth={'yes' if auth else 'no'}\n")
        private = open(os.path.join(SRV, "mode")).read().strip() == "private"
        p = self.path.split("?")[0].split("/")[1:]
        port = self.server.server_address[1]
        if p[:2] == ["site", "install"]: return self.file("install.sh")   # bise.dev/install
        if p[:5] == ["o", "r", "releases", "latest", "download"] and len(p) == 6:
            return self.send(404, b"Not Found") if private else self.file(p[5])
        if p[:1] == ["api"]:
            if auth != "Bearer " + TOKEN: return self.send(404, b'{"message":"Not Found"}', "application/json")
            if p[1:] == ["repos", "o", "r", "releases", "latest"]:
                assets = [{"name": n, "label": None, "url": f"http://127.0.0.1:{port}/api/repos/o/r/releases/assets/{n}"} for n in sorted(os.listdir(REL))]
                return self.send(200, json.dumps({"tag_name": "vX", "body": None, "assets": assets}).encode(), "application/json")
            if p[1:6] == ["repos", "o", "r", "releases", "assets"] and len(p) == 7:
                if self.headers.get("Accept") != "application/octet-stream": return self.send(200, b'{"name":"json"}', "application/json")
                return self.send(302, loc=f"http://localhost:{port}/s3/{p[6]}")
        if p[:1] == ["s3"]:   # another host: GitHub's storage refuses an Authorization header
            return self.send(400, b"auth sent to storage") if auth else self.file(p[1])
        self.send(404, b"Not Found")
s = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
open(os.path.join(SRV, "port"), "w").write(str(s.server_address[1]))
s.serve_forever()
EOF
echo public > "$SRV/mode"
python3 "$W/srv.py" "$REL" "$SRV" "$TOKEN" 2>"$W/srv.err" &
srv=$!; echo "$srv" > "$W/srv.pid"
for _ in $(seq 50); do [ -s "$SRV/port" ] && break; sleep 0.1; done
PORT="$(cat "$SRV/port" 2>/dev/null)"; [ -n "$PORT" ] || { echo "server did not start"; cat "$W/srv.err"; exit 1; }
trap 'kill "$srv" 2>/dev/null' EXIT
CH="http://127.0.0.1:$PORT/o/r/releases/latest/download" SITE="http://127.0.0.1:$PORT/site/install"
# the GitHub CLI: logged in when $STUB/authed exists (like `gh auth login`)
cat > "$STUB/bin/gh" <<EOF
#!/bin/sh
echo "gh \$*" >> "$SRV/gh.log"
[ -f "$STUB/authed" ] || { echo "To get started with GitHub CLI, please run:  gh auth login" >&2; exit 4; }
[ "\$1 \$2" = "release download" ] || exit 2
shift 2; name="" out="" repo=""
while [ \$# -gt 0 ]; do case "\$1" in -p) name="\$2"; shift ;; -O) out="\$2"; shift ;; -R) repo="\$2"; shift ;; esac; shift; done
case "\$repo" in */o/r) ;; *) echo "release not found" >&2; exit 1 ;; esac
cp "$REL/\$name" "\$out" 2>/dev/null || { echo "no assets match the file pattern" >&2; exit 1; }
EOF
chmod 755 "$STUB/bin/gh"
publish() { "$HERE/make-release.sh" --out "$REL" --url "$CH" --version "$2" "$1" >/dev/null 2>&1; }
t1="$(pack r1 2026-01-01T00:00:01Z)" t2="$(pack r2 2026-01-02T00:00:00Z)" t3="$(pack r3 2026-01-03T00:00:00Z)" t4="$(pack r4 2026-01-04T00:00:00Z)"
publish "$t1" 0.0.1 && ok "release r1 on the stand-in ($CH)" || { ko "make-release.sh"; exit 1; }

# a clean environment: fake HOME, system PATH (+ the stub gh: E_GH=1)
E() {
  local p=/usr/bin:/bin:/usr/sbin:/sbin; [ "${E_GH:-0}" = 1 ] && p="$STUB/bin:$p"
  env -i HOME="$H" PATH="$p" SHELL=/bin/zsh TERM=dumb USER="${USER:-me}" LANG=en_US.UTF-8 BISE_NO_UPDATE=1 "$@"
}
install_line() { (cd "$W" && E sh -c "curl -fsSL '$SITE' | sh" 2>&1); }
cur() { readlink "$H/.local/share/bise/current" 2>/dev/null; }
B() { E "$H/.local/bin/bise" "$@" 2>&1; }

echo "== public: plain downloads, no gh, no token"
H=$W/home-pub; mkdir -p "$H"
out="$(install_line)"
check "curl bise.dev/install | sh installs r1" test "$(cur)" = versions/r1
check "the channel is recorded" test "$(cat "$H/.local/share/bise/dist-url")" = "$CH"
publish "$t2" 0.0.2
out="$(B update --check)"; has "$out" "r2 is available" && ok "update --check: r2 available" || ko "update --check: $out"
out="$(B update)"; check "bise update: current -> r2" test "$(cur)" = versions/r2
check "no gh, no API call" sh -c "[ ! -e '$SRV/gh.log' ] && ! grep -q '^/api' '$SRV/log'"

echo "== private: the download URLs answer 404"
echo private > "$SRV/mode"
H=$W/home-priv; mkdir -p "$H"
out="$(install_line)"
has "$out" "gh auth login" && ok "no gh, no token: the error says gh auth login" || ko "no gh: $out"
printf '%s\n' "$out" | tail -n 2 | sed 's/^/     /'
check "nothing installed" test ! -e "$H/.local/share/bise"
out="$(E_GH=1 install_line)"
has "$out" "gh auth login" && ok "gh not logged in: the same error" || ko "gh not logged in: $out"
check "gh was asked" grep -q 'release download -R 127.0.0.1:'"$PORT"'/o/r -p latest.json' "$SRV/gh.log"
touch "$STUB/authed"
out="$(E_GH=1 install_line)"
check "gh logged in: installs r2 (the latest)" test "$(cur)" = versions/r2
publish "$t3" 0.0.3
rm "$STUB/authed"
out="$(E_GH=1 B update --check)"
has "$out" "gh auth login" && ok "update --check, gh logged out: the error says gh auth login" || ko "update, logged out: $out"
printf '%s\n' "$out" | sed 's/^/     /'
touch "$STUB/authed"
out="$(E_GH=1 B update --check)"; has "$out" "r3 is available" && ok "update --check via gh: r3 available" || ko "update --check via gh: $out"
E_GH=1 E "$H/.local/bin/bise" update --background 2>"$W/bg.log"
check "update --background via gh: current -> r3" test "$(cur)" = versions/r3
check "... and it logs 'updated to r3'" grep -q 'updated to r3' "$W/bg.log"
rm "$STUB/authed"

echo "== private: GH_TOKEN, no gh (the API)"
H=$W/home-tok; mkdir -p "$H"
: > "$SRV/log"
out="$(cd "$W" && E GH_TOKEN="$TOKEN" BISE_GITHUB_API="http://127.0.0.1:$PORT/api" sh -c "curl -fsSL '$SITE' | sh" 2>&1)"
check "installs r3 through the API" test "$(cur)" = versions/r3
check "the asset came from the storage host, without the token" grep -q '^/s3/bise-r3-.* auth=no' "$SRV/log"
check "the token is never in a URL" sh -c "! grep -q '$TOKEN' '$SRV/log'"
publish "$t4" 0.0.4
out="$(E GH_TOKEN=bad BISE_GITHUB_API="http://127.0.0.1:$PORT/api" "$H/.local/bin/bise" update 2>&1)"
has "$out" "gh auth login" && ok "a bad token: the error says gh auth login" || ko "bad token: $out"
out="$(E GITHUB_TOKEN="$TOKEN" BISE_GITHUB_API="http://127.0.0.1:$PORT/api" "$H/.local/bin/bise" update 2>&1)"
check "GITHUB_TOKEN: bise update -> r4" test "$(cur)" = versions/r4

echo "== public again: the private installs update with no auth"
echo public > "$SRV/mode"
H=$W/home-priv
out="$(B update)"; check "the gh install updates to r4 by plain curl" test "$(cur)" = versions/r4

echo "== CI makes the release (ci-release.sh, release.yml's release job)"
CI=$W/ci; mkdir -p "$CI/dist"
HEADC="$(git -C "$REPO" rev-parse HEAD)"
pack r5 2026-01-05T00:00:00Z darwin-arm64 "$HEADC" "$CI/dist" >/dev/null
pack r5 2026-01-05T00:00:00Z darwin-x86_64 "$HEADC" "$CI/dist" >/dev/null
for tg in linux-x86_64 linux-arm64; do pack r5 2026-01-05T00:00:00Z $tg "$HEADC" "$CI/dist" >/dev/null; done
out="$("$HERE/ci-release.sh" v0.0.5 "$CI/dist" "$CI/out" --repo o/r --url "$CH" --commit "$HEADC" 2>"$CI/err")" \
  && ok "ci-release.sh: both arches, checked" || { ko "ci-release.sh"; sed 's/^/     /' "$CI/err"; }
check "the files install.sh and bise update read, and only them" test "$(ls "$CI/out" | tr '\n' ' ')" = \
  "bise-r5-darwin-arm64.tar.gz bise-r5-darwin-arm64.tar.gz.sha256 bise-r5-darwin-x86_64.tar.gz bise-r5-darwin-x86_64.tar.gz.sha256 bise-r5-linux-arm64.tar.gz bise-r5-linux-arm64.tar.gz.sha256 bise-r5-linux-x86_64.tar.gz bise-r5-linux-x86_64.tar.gz.sha256 install.sh latest.json nix-sources.json "
check "latest.json: version 0.0.5, id r5, this commit" python3 -c "import json,sys; m=json.load(open('$CI/out/latest.json')); sys.exit(not (m['version'], m['id'], m['commit']) == ('0.0.5', 'r5', '$HEADC'))"
chk() { "$HERE/check-release.py" "$1" --url "$CH" "${@:2}"; }
cp -R "$CI/out" "$CI/bad"; sed -i '' 's/"sha256": "\(.\)/"sha256": "0\1/' "$CI/bad/latest.json" 2>/dev/null || sed -i 's/"sha256": "\(.\)/"sha256": "0\1/' "$CI/bad/latest.json"
check "check-release: a bad sha256 fails" sh -c "! '$HERE/check-release.py' '$CI/bad' --url '$CH'"
rm -rf "$CI/bad"; cp -R "$CI/out" "$CI/bad"; cp "$HERE/install.sh" "$CI/bad/install.sh"
check "check-release: an unstamped install.sh fails" sh -c "! '$HERE/check-release.py' '$CI/bad' --url '$CH'"
rm -rf "$CI/bad"; cp -R "$CI/out" "$CI/bad"; touch "$CI/bad/extra.txt"
check "check-release: an extra file fails" sh -c "! '$HERE/check-release.py' '$CI/bad' --url '$CH'"
check "check-release: another channel fails" sh -c "! '$HERE/check-release.py' '$CI/out' --url https://example.com/x"
mkdir -p "$CI/one"; cp "$CI/dist"/bise-r5-darwin-arm64.* "$CI/one/"
check "ci-release.sh: one arch only fails" sh -c "! '$HERE/ci-release.sh' v0.0.5 '$CI/one' '$CI/o1' --repo o/r --url '$CH' 2>/dev/null"
echo 0 >> "$CI/one/bise-r5-darwin-arm64.tar.gz"
check "ci-release.sh: an archive that is not its build's fails" sh -c "! '$HERE/ci-release.sh' v0.0.5 '$CI/one' '$CI/o1' --repo o/r --url '$CH' --targets darwin-arm64 2>/dev/null"

echo "== publish-release.sh: tag, CI's draft, publish (stub gh, throwaway repo)"
G=$W/git BARE=$W/bare.git GS=$W/ghs STUB2=$W/stub2
mkdir -p "$G/packaging" "$GS/rel" "$STUB2"
for f in publish-release.sh ci-release.sh check-release.py make-release.sh install.sh; do cp "$HERE/$f" "$G/packaging/"; done
gitq() { git -C "$G" -c user.name=t -c user.email=t@t -c commit.gpgsign=false -c tag.gpgsign=false "$@" >/dev/null 2>&1; }
gitq init -q -b main && gitq add -A && gitq commit -qm one
git init -q --bare "$BARE"
gitq remote add origin https://github.com/o/r.git && gitq remote set-url --push origin "$BARE" && gitq push origin main
C1="$(git -C "$G" rev-parse HEAD)"
gitq commit -q --allow-empty -m "not pushed"; C2="$(git -C "$G" rev-parse HEAD)"
mkdir -p "$W/cidist"
pack r6 2026-01-06T00:00:00Z darwin-arm64 "$C1" "$W/cidist" >/dev/null
pack r6 2026-01-06T00:00:00Z darwin-x86_64 "$C1" "$W/cidist" >/dev/null
for tg in linux-x86_64 linux-arm64; do pack r6 2026-01-06T00:00:00Z $tg "$C1" "$W/cidist" >/dev/null; done
# GitHub for o/r: releases in $GS/rel/<tag>/ (a `draft` flag file), tags
# and commits from the bare repo; `run watch` is CI: it runs ci-release.sh
cat > "$STUB2/gh" <<EOF
#!/usr/bin/env python3
import os, shutil, subprocess, sys
GS, BARE, DIST, CH, PK = "$GS", "$BARE", "$W/cidist", "$CH", "$G/packaging"
a = sys.argv[1:]
open(GS + "/log", "a").write(" ".join(a) + "\n")
def opt(k):
    return a[a.index(k) + 1] if k in a else None
def rel(t): return os.path.join(GS, "rel", t)
def git(*x): return subprocess.run(["git", "-C", BARE, *x], capture_output=True, text=True)
def files(t): return sorted(f for f in os.listdir(rel(t)) if f != "draft")
def die(m, c=1): print(m, file=sys.stderr); sys.exit(c)
def args_after(n):   # positionals after a[:n], until the first flag
    out = []
    for x in a[n:]:
        if x.startswith("-"): break
        out.append(x)
    return out
if a[:2] in (["auth", "status"], ["repo", "view"]): sys.exit(0)
if a[0] == "api":
    p = a[1].split("/")
    if p[3] == "commits":
        if git("cat-file", "-e", p[4] + "^{commit}").returncode: die("No commit found for SHA")
        print(p[4]); sys.exit(0)
    if p[3:6] == ["git", "ref", "tags"]:
        r = git("rev-parse", "-q", "--verify", "refs/tags/" + p[6] + "^{commit}")
        if r.returncode:   # as gh does: GitHub's 404 body on stdout, exit 1
            print('{"message":"Not Found","documentation_url":"https://docs.github.com/rest/git/refs#get-a-reference","status":"404"}', end="")
            die("gh: Not Found (HTTP 404)")
        print(r.stdout.strip()); sys.exit(0)
    die("stub: api " + a[1], 2)
if a[0] == "release":
    t = a[2] if len(a) > 2 else ""
    if a[1] == "create":
        if os.path.exists(rel(t)): die("a release with the same tag name already exists")
        if "--verify-tag" in a and git("rev-parse", "-q", "--verify", "refs/tags/" + t).returncode: die("tag not found")
        os.makedirs(rel(t))
        if "--draft" in a: open(rel(t) + "/draft", "w").close()
        for f in args_after(3): shutil.copy(f, rel(t))
        sys.exit(0)
    if not os.path.isdir(rel(t)): die("release not found")
    if a[1] == "view":
        j = opt("--json")
        if j == "isDraft": print("true" if os.path.exists(rel(t) + "/draft") else "false")
        elif j == "assets": print("\n".join(files(t)))
        else: print(("draft " if os.path.exists(rel(t) + "/draft") else "") + t + "\n" + "\n".join("asset: " + f for f in files(t)))
        sys.exit(0)
    if a[1] == "upload":
        for f in args_after(3): shutil.copy(f, rel(t))
        sys.exit(0)
    if a[1] == "delete-asset": os.remove(os.path.join(rel(t), a[3])); sys.exit(0)
    if a[1] == "download":
        d = opt("-D")
        os.makedirs(d, exist_ok=True)
        for i, x in enumerate(a):
            if x == "-p": shutil.copy(os.path.join(rel(t), a[i + 1]), d)
        sys.exit(0)
    if a[1] == "edit":
        if "--draft=false" in a and os.path.exists(rel(t) + "/draft"): os.remove(rel(t) + "/draft")
        sys.exit(0)
if a[:2] == ["run", "list"]:
    t = opt("--branch")
    if git("rev-parse", "-q", "--verify", "refs/tags/" + t).returncode == 0:
        open(GS + "/run-tag", "w").write(t); print(42)
    sys.exit(0)
if a[:2] == ["run", "watch"]:
    if os.path.exists(GS + "/ci-fails"): die("run 42 failed")
    t = open(GS + "/run-tag").read()
    sha = git("rev-parse", "refs/tags/" + t + "^{commit}").stdout.strip()
    r = subprocess.run([PK + "/ci-release.sh", t, DIST, GS + "/ci-out", "--repo", "o/r", "--url", CH, "--commit", sha, "--draft"], stdout=subprocess.DEVNULL, stderr=open(GS + "/ci.log", "a"))
    sys.exit(r.returncode)
die("stub: " + " ".join(a), 2)
EOF
chmod 755 "$STUB2/gh"
P() { env -i HOME="$W/home-rel" PATH="$STUB2:/usr/bin:/bin:/usr/sbin:/sbin" TMPDIR="$W/tmp/" \
  GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t \
  "$G/packaging/publish-release.sh" --repo o/r --url "$CH" "$@" 2>&1; }
mkdir -p "$W/home-rel" "$W/tmp"
tagged() { git -C "$BARE" rev-parse -q --verify "refs/tags/$1" >/dev/null 2>&1; }
out="$(P v0.0.6)"; has "$out" "is not on GitHub" && ok "an unpushed commit: refused" || ko "unpushed: $out"
check "... no tag pushed" sh -c "! git -C '$BARE' rev-parse -q --verify refs/tags/v0.0.6"
out="$(P v0.0.6 --rev "$C1" --add x)"; has "$out" "go with --local" && ok "--add without --local: refused" || ko "--add: $out"
out="$(P v0.0.6 --rev "$C1" --dry-run)"; has "$out" "would tag" && ok "--dry-run says what it would do" || ko "dry run: $out"
has "$out" "is on GitHub" && ko "... a 404 read as a tag on GitHub: $out" || ok "... a tag GitHub answers 404 for is not on GitHub"
check "... and pushes nothing" sh -c "! git -C '$BARE' rev-parse -q --verify refs/tags/v0.0.6"
out="$(P v0.0.6 --rev "$C1")"; printf '%s\n' "$out" | tail -n 3 | sed 's/^/     /'
check "the tag is pushed, on the commit" test "$(git -C "$BARE" rev-parse 'refs/tags/v0.0.6^{commit}' 2>/dev/null)" = "$C1"
check "the run was watched" grep -q '^run watch 42 ' "$GS/log"
check "CI made a draft of the release" test -f "$GS/rel/v0.0.6/draft"
has "$out" "check-release: ok, 0.0.6 (r6): darwin-arm64 darwin-x86_64 linux-arm64 linux-x86_64" && ok "the draft is checked (latest.json, install.sh, files)" || ko "draft check: $out"
has "$out" "not published" && ok "not published without --publish" || ko "publish hint: $out"
check "the draft's files" test "$(ls "$GS/rel/v0.0.6" | tr '\n' ' ')" = \
  "bise-r6-darwin-arm64.tar.gz bise-r6-darwin-arm64.tar.gz.sha256 bise-r6-darwin-x86_64.tar.gz bise-r6-darwin-x86_64.tar.gz.sha256 bise-r6-linux-arm64.tar.gz bise-r6-linux-arm64.tar.gz.sha256 bise-r6-linux-x86_64.tar.gz bise-r6-linux-x86_64.tar.gz.sha256 draft install.sh latest.json nix-sources.json "
n="$(grep -c '^run watch' "$GS/log")"
out="$(P v0.0.6 --publish)"; has "$out" "published v0.0.6" && ok "--publish: the draft is the latest release" || ko "publish: $out"
check "... gh release edit --draft=false" sh -c "grep -q '^release edit v0.0.6 -R o/r --draft=false --latest' '$GS/log' && [ ! -e '$GS/rel/v0.0.6/draft' ]"
check "... no second run watched" test "$(grep -c '^run watch' "$GS/log")" = "$n"
check "... the flake's nix/sources.json committed: r6, linux, the tag's URLs" sh -c "
  [ \"\$(git -C '$G' log -1 --format=%s)\" = 'nix: sources for 0.0.6' ] &&
  python3 -c \"import json,sys; n=json.load(open('$G/nix/sources.json')); sys.exit(not (n['id'] == 'r6' and n['aarch64-linux']['url'] == 'https://github.com/o/r/releases/download/v0.0.6/bise-r6-linux-arm64.tar.gz' and 'x86_64-linux' in n and 'darwin-arm64' not in n))\""
out="$(P v0.0.6 --publish)"; has "$out" "published already" && ok "again: already published, nothing done" || ko "again: $out"
touch "$GS/rel/v0.0.6/stale"
out="$(PATH="$STUB2:$PATH" "$HERE/ci-release.sh" v0.0.6 "$W/cidist" "$W/ci2" --repo o/r --url "$CH" --draft 2>&1)"
has "$out" "never rewrites" && ok "CI never rewrites a published release" || ko "rerun on published: $out"
check "... its files unchanged" test -e "$GS/rel/v0.0.6/stale"
rm "$GS/rel/v0.0.6/stale"
# the published files are the channel: install and update from them
cp "$GS/rel/v0.0.6"/* "$REL/"
H=$W/home-pub
out="$(B update)"; check "bise update from CI's release: current -> r6" test "$(cur)" = versions/r6
H=$W/home-ci; mkdir -p "$H"
out="$(install_line)"; check "curl .../install | sh from CI's install.sh installs r6" test "$(cur)" = versions/r6
# a rerun of CI on a draft replaces its files, drops a stale one
mkdir -p "$GS/rel/v0.0.6x"; touch "$GS/rel/v0.0.6x/draft" "$GS/rel/v0.0.6x/old.tar.gz"
gitq tag v0.0.6x "$C1" && gitq push origin v0.0.6x
out="$(PATH="$STUB2:$PATH" "$HERE/ci-release.sh" v0.0.6x "$W/cidist" "$W/ci3" --repo o/r --url "$CH" --draft 2>&1)" \
  && ok "CI rerun on a draft: files replaced, checked" || ko "rerun on draft: $out"
check "... the stale file is gone" test ! -e "$GS/rel/v0.0.6x/old.tar.gz"
touch "$GS/ci-fails"
out="$(P v0.0.7 --rev "$C1")"; has "$out" "the run failed" && ok "a failed CI run: said, nothing published" || ko "failed run: $out"
check "... no release" test ! -e "$GS/rel/v0.0.7"
rm "$GS/ci-fails"
out="$(P --local v0.0.8 --from "$W/cidist/bise-r6-darwin-arm64.tar.gz" --add "$W/cidist/bise-r6-darwin-x86_64.tar.gz" --dry-run)"
has "$out" "check-release: ok" && has "$out" "release create v0.0.8" && ok "--local: lays out, checks, prints the gh command" || ko "--local: $out"

echo "== $pass passed, $fail failed"
[ "$fail" = 0 ] && rm -rf "$W"
[ "$fail" = 0 ]
