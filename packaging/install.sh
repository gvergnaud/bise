#!/bin/sh
# install.sh — the installer of bise (the harness + Switchboard), BISE-170.
#
#   curl -fsSL <channel>/install.sh | sh          (the published form)
#   sh install.sh [--from <tarball|bundle dir>] [--dist-url <url>]
#                 [--prefix <dir>] [--bin-dir <dir>] [--no-modify-path] [--keep <n>]
#   sh install.sh --uninstall [--prefix <dir>] [--bin-dir <dir>] [--purge]
#   sh install.sh --dev [--repo <dir>] [--bin-dir <dir>]   (dev channel)
#   sh install.sh --uninstall --dev [--bin-dir <dir>]
#
# Where the build comes from, in order: --from; the bundle this file sits
# in (app/ next to it); the release channel: <url>/latest.json names the
# tarball of this Mac (darwin-arm64 or darwin-x86_64; an x86_64 shell
# under Rosetta gets arm64) and its sha256. The channel is ONE value:
# --dist-url, else $BISE_DIST_URL, else DIST_URL_DEFAULT below (stamped
# by make-release.sh in the install.sh it publishes). file:// works (the
# tests). The channel is recorded in $PREFIX/dist-url: `bise update`
# reads it.
#
# A private GitHub repo (BISE-217): a GitHub release asset that a plain
# download cannot read (404) comes from `gh release download` (the
# GitHub CLI, logged in with `gh auth login`), else the API with
# $GH_TOKEN or $GITHUB_TOKEN ($BISE_GITHUB_API: another API base).
#
# --dev (BISE-129): no bundle; $BIN_DIR/bise runs the version the hub of
# the dev repo (--repo, default: the repo this script is in) runs now:
# its versions.json 'current', what /restart and `sb restart` switch
# to. Every restart there updates `bise` everywhere.
#
# Layout:
#   $PREFIX/versions/<id>/   immutable app roots (the versions.sh layout)
#   $PREFIX/current -> versions/<id>
#   $PREFIX/bin/bise         the launcher: exec current/bise (the rest,
#                            --version, update, uninstall, is in bise)
#   $PREFIX/install.sh       the installer of the current version (uninstall)
#   $PREFIX/dist-url         the release channel it came from
#   $BIN_DIR/bise -> $PREFIX/bin/bise
#   $BIN_DIR/bend-harness -> $PREFIX/bin/bise   (the old name, one release)
# Defaults: PREFIX=~/.local/share/bise, BIN_DIR=~/.local/bin.
# Your data is never inside $PREFIX: ~/.bise (keys, config, hubs,
# sessions) and the older ~/.bend-harness, ~/.local/state/switchboard
# survive an uninstall unless --purge.

set -eu

CMD=bise                  # the command name (BISE-165)
OLD_CMD=bend-harness      # its old name: a second link, kept one release
# the release channel, stamped by make-release.sh ('' = none published)
DIST_URL_DEFAULT=''
DIST_URL="${BISE_DIST_URL:-$DIST_URL_DEFAULT}"
PREFIX="${BISE_PREFIX:-$HOME/.local/share/bise}"
OLD_PREFIX="$HOME/.local/share/bend-harness"   # before BISE-170
BIN_DIR="${BISE_BIN_DIR:-$HOME/.local/bin}"
FROM=""
MODIFY_PATH=1
KEEP=3
ACTION=install
PURGE=0
DEV=0
REPO=""
MARK="# added by the $CMD installer"
OLD_MARK="# added by the $OLD_CMD installer"   # before BISE-165: same PATH line

say() { printf '%s\n' "$CMD install: $*" >&2; }
die() { say "error: $*"; exit 1; }
# sha256 of a file: shasum (macOS, most Linux), else sha256sum (coreutils:
# a minimal Linux has no perl, so no shasum)
sha256_of() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  else sha256sum "$1" | cut -d' ' -f1; fi
}
# json_get <file> <a.b.c>: a string or number of a JSON file; plutil on
# macOS, python3 elsewhere, else (a bare Linux) the flat form
# make-release.sh writes for latest.json: one target per line
json_get() {
  if command -v plutil >/dev/null 2>&1; then plutil -extract "$2" raw -o - "$1" 2>/dev/null; return; fi
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import json,sys
v=json.load(open(sys.argv[1]))
for k in sys.argv[2].split("."): v=v[int(k)] if isinstance(v,list) else v[k]
print(v)' "$1" "$2" 2>/dev/null; return; fi
  case "$2" in
    targets.*.*) t="${2#targets.}"; k="${t##*.}"; t="${t%.*}"
      grep -F "\"$t\": {" "$1" | sed -n "s/.*\"$k\": *\"\{0,1\}\([^\",}]*\).*/\1/p" | head -n 1 ;;
    *) return 1 ;;
  esac
}

while [ $# -gt 0 ]; do
  case "$1" in
    --from) FROM="$2"; shift ;;
    --dist-url) DIST_URL="$2"; shift ;;
    --prefix) PREFIX="$2"; shift ;;
    --bin-dir) BIN_DIR="$2"; shift ;;
    --no-modify-path) MODIFY_PATH=0 ;;
    --keep) KEEP="$2"; shift ;;
    --uninstall) ACTION=uninstall ;;
    --purge) PURGE=1 ;;
    --dev) DEV=1 ;;
    --repo) REPO="$2"; shift ;;
    -h|--help) sed -n '2,39p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
  shift
done

# ---- the shell rc files the PATH line goes to ----
rc_files() {
  case "$(basename "${SHELL:-/bin/sh}")" in
    zsh) echo "${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash) echo "$HOME/.bashrc"; [ "$(uname -s)" = Darwin ] && echo "$HOME/.bash_profile" ;;
    fish) echo "$HOME/.config/fish/conf.d/$CMD.fish" ;;
    *) echo "$HOME/.profile" ;;
  esac
}

# the hubs running from this prefix (their app root is a version dir here)
running_hubs() {
  ps -axo pid=,command= 2>/dev/null | grep -F "$PREFIX/versions/" | grep -F " sbd " | grep -v grep || true
}

# ---- the dev channel (--dev): a launcher that follows a dev hub ----
# The dev state dir, the rule of versions.sh and bise_home: $BISE_HOME/dev,
# ~/.bise/dev once migrated, else ~/.local/state/switchboard.
dev_dir() {
  if [ -n "${BISE_HOME:-}" ]; then echo "$BISE_HOME/dev"
  elif [ -e "$HOME/.bise/migrated.json" ]; then echo "$HOME/.bise/dev"
  else echo "$HOME/.local/state/switchboard"; fi
}
if [ "$DEV" = 1 ]; then
  DEV_DIR="$(dev_dir)"
  LAUNCHER="$DEV_DIR/bin/$CMD"
  if [ "$ACTION" = uninstall ]; then
    [ -L "$BIN_DIR/$CMD" ] && [ "$(readlink "$BIN_DIR/$CMD")" = "$LAUNCHER" ] && rm -f "$BIN_DIR/$CMD"
    rm -f "$LAUNCHER"
    say "dev channel removed ($BIN_DIR/$CMD); versions, hubs and your data are kept"
    exit 0
  fi
  [ -n "$REPO" ] || REPO="$(cd "$(dirname "$0")" && git rev-parse --show-toplevel 2>/dev/null)" \
    || die "no repo: pass --repo <the dev repo>"
  REPO="$(cd "$REPO" 2>/dev/null && pwd -P)" || die "no such repo: $REPO"
  [ -x "$REPO/scripts/versions.sh" ] || [ -x "$REPO/versions.sh" ] || die "$REPO is not the bise dev repo (no scripts/versions.sh)"
  VERSIONS="${SB_VERSIONS_DIR:-$DEV_DIR/versions}"
  # a built version, newest first: to name the repo's hub (its id is a
  # hash of the path the binary computes: `switchboard --state-dir`)
  any=""
  for d in $(ls -t "$VERSIONS/" 2>/dev/null); do
    case "$d" in .*) continue ;; esac
    for b in bise bend-harness; do
      [ -x "$VERSIONS/$d/$b" ] && { any="$VERSIONS/$d/$b"; break 2; }
    done
  done
  [ -n "$any" ] || die "no version built in $VERSIONS: run '$REPO/scripts/versions.sh build' first"
  state="$(cd "$REPO" && env -u SB_STATE_DIR BISE_NO_MIGRATE=1 SB_LAUNCH_DIR="$REPO" \
    "$any" switchboard --state-dir --workspace "$REPO" 2>/dev/null)" || die "$any cannot name the hub of $REPO"
  HUB="$(basename "$state")"
  case "$HUB" in *-[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]) ;; *) die "odd hub id: $HUB" ;; esac
  mkdir -p "$DEV_DIR/bin"
  q() { printf '%s' "$1" | sed "s/'/'\\\\''/g"; }
  cat > "$LAUNCHER.tmp" <<EOF
#!/bin/sh
# $CMD launcher, dev channel (BISE-129), written by install.sh --dev.
# Runs the version the hub of REPO runs now: its versions.json 'current'
# (what /restart and 'sb restart' switch to; a hub that never switched:
# its hub.root), so a restart there updates '$CMD' everywhere. No hub
# yet: the newest built version.
# BISE_DEV_VERSION=<id|dir> runs another built version.
REPO='$(q "$REPO")'
HUB='$HUB'
EOF
  cat >> "$LAUNCHER.tmp" <<'EOF'
CMD="$(basename "$0")"
if [ -n "${BISE_HOME:-}" ]; then home="$BISE_HOME"; dev="$BISE_HOME/dev"
elif [ -e "$HOME/.bise/migrated.json" ]; then home="$HOME/.bise"; dev="$HOME/.bise/dev"
else home=""; dev="$HOME/.local/state/switchboard"; fi
versions="${SB_VERSIONS_DIR:-$dev/versions}"
runnable() { [ -x "$1/bise" ] || [ -x "$1/bend-harness" ]; }
pick() {
  if [ -n "${BISE_DEV_VERSION:-}" ]; then
    for v in "$BISE_DEV_VERSION" "$versions/$BISE_DEV_VERSION"; do
      runnable "$v" && { echo "$v"; return 0; }
    done
    echo "$CMD: BISE_DEV_VERSION=$BISE_DEV_VERSION: no such version in $versions" >&2; return 1
  fi
  # the hub's folder: ~/.bise/hubs, else the old place (a hub that has
  # not moved yet: it was running when ~/.bise was made)
  for d in ${home:+"$home/hubs/$HUB"} "$HOME/.local/state/switchboard/$HUB"; do
    [ -f "$d/versions.json" ] || continue
    for k in current good; do
      v="$(sed -n "s/.*\"$k\" *: *\"\([^\"]*\)\".*/\1/p" "$d/versions.json")"
      [ -n "$v" ] && runnable "$v" && { echo "$v"; return 0; }
    done
  done
  # a hub that never switched has no versions.json: the root it runs
  for d in ${home:+"$home/hubs/$HUB"} "$HOME/.local/state/switchboard/$HUB"; do
    v="$(cat "$d/hub.root" 2>/dev/null)"
    [ -n "$v" ] && runnable "$v" && { echo "$v"; return 0; }
  done
  for d in $(ls -t "$versions/" 2>/dev/null); do
    case "$d" in .*) continue ;; esac
    runnable "$versions/$d" && { echo "$versions/$d"; return 0; }
  done
  echo "$CMD: no version built in $versions; run '$REPO/scripts/versions.sh build'" >&2; return 1
}
root="$(pick)" || exit 1
root="$(cd "$root" && pwd -P)"
ver() { sed -n "s/^$1=//p" "$root/VERSION" 2>/dev/null; }
case "${1:-}" in
  --launcher-root) echo "$root"; exit 0 ;;
  --version|-V|version)
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"; arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
    echo "$CMD $(ver id) (${os}-${arch}, commit $(ver commit | cut -c1-12), built $(ver built))"
    echo "dev channel: the version the hub of $REPO runs ($root)"
    exit 0 ;;
esac
# the user's folder: the workspace, and a single session's directory
# (never an inherited one: an agent's shell has the hub's)
export SB_LAUNCH_DIR="$PWD"
export BEND_WORKDIR="$PWD"
export BISE_APP_ROOT="$root"
[ -x "$root/bise" ] && exec "$root/bise" "$@"
# a version before BISE-163/165: bend-harness, its app root from the cwd
cd "$root" || exit 1
exec "$root/bend-harness" "$@"
EOF
  chmod 755 "$LAUNCHER.tmp"
  mv -f "$LAUNCHER.tmp" "$LAUNCHER"
  mkdir -p "$BIN_DIR"
  if [ -e "$BIN_DIR/$CMD" ] && [ ! -L "$BIN_DIR/$CMD" ]; then
    die "$BIN_DIR/$CMD exists and is not a link; remove it or pass --bin-dir"
  fi
  if [ -L "$BIN_DIR/$CMD" ] && [ "$(readlink "$BIN_DIR/$CMD")" != "$LAUNCHER" ]; then
    say "replacing $BIN_DIR/$CMD -> $(readlink "$BIN_DIR/$CMD")"
  fi
  ln -sfn "$LAUNCHER" "$BIN_DIR/$CMD"
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) say "$BIN_DIR is not on your PATH: add  export PATH=\"$BIN_DIR:\$PATH\"  to your shell's rc" ;;
  esac
  say "dev channel: $BIN_DIR/$CMD -> $LAUNCHER"
  say "it runs the version the hub of $REPO runs ($HUB), now: $("$LAUNCHER" --launcher-root)"
  exit 0
fi

# ---- uninstall ----
if [ "$ACTION" = uninstall ]; then
  hubs="$(running_hubs)"
  if [ -n "$hubs" ]; then
    say "stopping the Switchboard hubs of this install (agents too):"
    printf '%s\n' "$hubs" >&2
    printf '%s\n' "$hubs" | while read -r pid _; do kill "$pid" 2>/dev/null || true; done
  fi
  for c in "$CMD" "$OLD_CMD"; do [ -L "$BIN_DIR/$c" ] && rm -f "$BIN_DIR/$c"; done
  rm -f "$PREFIX/bin/$OLD_CMD"
  rm -rf "$PREFIX"
  for f in $(rc_files); do
    [ -f "$f" ] || continue
    if grep -qF -e "$MARK" -e "$OLD_MARK" "$f"; then
      { grep -vF -e "$MARK" -e "$OLD_MARK" "$f" || true; } > "$f.tmp.$$"
      mv "$f.tmp.$$" "$f"
      say "PATH line removed from $f"
    fi
  done
  if [ "$PURGE" = 1 ]; then
    rm -rf "${BISE_HOME:-$HOME/.bise}" "$HOME/.bend-harness" "$HOME/.local/state/switchboard"
    say "purged ~/.bise, ~/.bend-harness and ~/.local/state/switchboard (keys, config, hubs, sessions)"
  else
    say "kept your data: ~/.bise (and ~/.bend-harness, ~/.local/state/switchboard if any; --purge removes them)"
  fi
  say "uninstalled"
  exit 0
fi

# ---- install: find the bundle ----
os="$(uname -s | tr '[:upper:]' '[:lower:]')"; arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
# an x86_64 shell on an M-series Mac (Rosetta): the native build
if [ "$os" = darwin ] && [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null)" = 1 ]; then
  arch=arm64
fi
target="$os-$arch"
# NixOS has no /lib64/ld-linux*: the glibc build would not start (nix-ld
# makes it work: NIX_LD set). The flake patches it for the store.
if [ "$os" = linux ] && [ -e /etc/NIXOS ] && [ -z "${NIX_LD:-}" ]; then
  die "NixOS: install bise with Nix instead: nix profile install github:gvergnaud/bise (flakes), or nix-env -if https://github.com/gvergnaud/bise/archive/main.tar.gz"
fi
# the bundle this file is in; never when piped (`curl | sh`: $0 is sh)
here=""
[ -f "$0" ] && here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/$CMD-install.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT INT TERM
DIST_URL="${DIST_URL%/}"

# ---- downloads: plain, else a private GitHub release asset ----
# the asset a release download URL names: <origin>/<owner>/<repo>/releases/
# latest/download/<name> or .../releases/download/<tag>/<name>
gh_part() {
  printf '%s\n' "$1" | sed -nE "s#^(https?://)([^/]+)/([^/]+)/([^/]+)/releases/(latest/download|download/([^/]+))/([^/]+)\$#$2#p"
}
# curl with its config (the token's header) on stdin: never in `ps`
curl_cfg() { printf '%s\n' "$1" | curl -fsSL --connect-timeout 15 --retry 2 -K - -o "$3" "$2"; }
fetch() {
  curl -fsSL --connect-timeout 15 --retry 2 "$1" -o "$2" 2>"$tmp/err" && return 0
  host="$(gh_part "$1" '\2')"
  [ -n "$host" ] || die "cannot read $1: $(cat "$tmp/err")"
  repo="$(gh_part "$1" '\3/\4')" tag="$(gh_part "$1" '\6')" name="$(gh_part "$1" '\7')"
  ghr="$repo"; [ "$host" = github.com ] || ghr="$host/$repo"
  if command -v gh >/dev/null 2>&1 \
    && GH_PROMPT_DISABLED=1 GH_NO_UPDATE_NOTIFIER=1 gh release download $tag -R "$ghr" -p "$name" --clobber -O "$2" </dev/null 2>"$tmp/err"; then
    return 0
  fi
  token="${GH_TOKEN:-${GITHUB_TOKEN:-}}"
  if [ -n "$token" ]; then
    api="${BISE_GITHUB_API:-}"
    [ -n "$api" ] || if [ "$host" = github.com ]; then api=https://api.github.com; else api="$(gh_part "$1" '\1\2')/api/v3"; fi
    rel="$api/repos/$repo/releases/latest"; [ -z "$tag" ] || rel="$api/repos/$repo/releases/tags/$tag"
    hdr="header = \"Authorization: Bearer $token\""
    if curl_cfg "$hdr" "$rel" "$tmp/release.json" 2>"$tmp/err"; then
      i=0 asset=""
      while n="$(json_get "$tmp/release.json" "assets.$i.name")"; do
        [ "$n" = "$name" ] && { asset="$(json_get "$tmp/release.json" "assets.$i.url")"; break; }
        i=$((i + 1))
      done
      [ -n "$asset" ] && curl_cfg "$hdr
header = \"Accept: application/octet-stream\"" "$asset" "$2" 2>"$tmp/err" && return 0
      [ -n "$asset" ] || echo "the release has no asset $name" > "$tmp/err"
    fi
  fi
  rm -f "$2"
  say "cannot read $1 ($(tail -n 1 "$tmp/err" 2>/dev/null))"
  die "if $repo is private: install the GitHub CLI (brew install gh), run 'gh auth login' with an account that can read it, then run this again (or set GH_TOKEN)"
}

if [ -z "$FROM" ]; then
  if [ -n "$here" ] && [ -f "$here/app/VERSION" ]; then
    FROM="$here"
  elif [ -n "$DIST_URL" ]; then
    say "reading $DIST_URL/latest.json"
    fetch "$DIST_URL/latest.json" "$tmp/latest.json"
    get() { json_get "$tmp/latest.json" "targets.$target.$1" || true; }
    url="$(get url)"; sum="$(get sha256)"
    [ -n "$url" ] || url="$(get file)"
    { [ -n "$url" ] && [ -n "$sum" ]; } || die "the release has no build for $target"
    case "$url" in *://*) ;; *) url="$DIST_URL/$url" ;; esac
    say "downloading $url"
    fetch "$url" "$tmp/dl.tar.gz"
    [ "$(sha256_of "$tmp/dl.tar.gz")" = "$sum" ] || die "checksum mismatch: $url"
    FROM="$tmp/dl.tar.gz"
  else
    die "no bundle and no release channel: pass --from <tarball|dir>, or --dist-url <url> (or BISE_DIST_URL)"
  fi
fi
if [ -f "$FROM" ]; then
  if [ -f "$FROM.sha256" ]; then
    [ "$(sha256_of "$FROM")" = "$(cut -d' ' -f1 "$FROM.sha256")" ] || die "checksum mismatch: $FROM"
  fi
  mkdir -p "$tmp/x"
  tar -C "$tmp/x" -xzf "$FROM" || die "cannot extract $FROM"
  FROM="$(find "$tmp/x" -mindepth 2 -maxdepth 2 -type d -name app | head -n 1 | xargs dirname)"
fi
app="$FROM/app"
# the command: bise, or bend-harness in a bundle built before BISE-165
[ -e "$app/bise" ] || [ -e "$app/bend-harness" ] \
  || die "incomplete bundle: app/bise missing in $FROM (a download cut short, or removed by security software)"
for f in repl-live sb-core VERSION; do
  [ -e "$app/$f" ] || die "incomplete bundle: app/$f missing in $FROM (a download cut short, or removed by security software)"
done
# the V8 engine: app/bend-jsrt, or its path before BISE-114
[ -e "$app/bend-jsrt" ] || [ -e "$app/rust/jsrt/target/debug/bend-jsrt" ] \
  || die "incomplete bundle: app/bend-jsrt missing in $FROM (a download cut short, or removed by security software)"

id="$(sed -n 's/^id=//p' "$app/VERSION")"
bundle_target="$(sed -n 's/^target=//p' "$app/VERSION")"
[ -z "$bundle_target" ] || [ "$bundle_target" = "$target" ] || die "this bundle is for $bundle_target, this Mac wants $target"
command -v git >/dev/null 2>&1 || say "warning: git not found (Switchboard needs it for worktrees and /version)"

# ---- the version dir: immutable, written once, then the pointer flips ----
mkdir -p "$PREFIX/versions" "$PREFIX/bin"
if [ -x "$PREFIX/versions/$id/bise" ] || [ -x "$PREFIX/versions/$id/bend-harness" ]; then
  say "version $id already installed"
else
  rm -rf "$PREFIX/versions/.$id.tmp"
  cp -R "$app" "$PREFIX/versions/.$id.tmp"
  # a browser download carries the quarantine flag; Gatekeeper would
  # block the unsigned binaries (curl does not set it)
  [ "$os" = darwin ] && xattr -dr com.apple.quarantine "$PREFIX/versions/.$id.tmp" 2>/dev/null || true
  mv "$PREFIX/versions/.$id.tmp" "$PREFIX/versions/$id"
fi
# (BSD mv onto a symlink to a dir moves INTO the dir: no rename trick,
# rm + ln; the launcher resolves 'current' once, at start)
rm -f "$PREFIX/current"
ln -s "versions/$id" "$PREFIX/current"
# the installer uninstall runs: the bundle's, else this file
if [ -f "$FROM/install.sh" ]; then cp "$FROM/install.sh" "$PREFIX/install.sh"
elif [ -n "$here" ]; then cp "$0" "$PREFIX/install.sh"; fi
# the channel `bise update` reads (an earlier install's is kept when none now)
if [ -n "$DIST_URL" ]; then printf '%s\n' "$DIST_URL" > "$PREFIX/dist-url"; fi

# ---- the launcher: exec the current version ----
q_prefix="$(printf '%s' "$PREFIX" | sed "s/'/'\\\\''/g")"
cat > "$PREFIX/bin/$CMD.tmp" <<EOF
#!/bin/sh
# $CMD launcher, written by install.sh: runs the CURRENT version from its
# real (immutable) dir, so an update that flips 'current' never changes
# a running hub under it. The rest (--version, update, uninstall) is $CMD's.
PREFIX='$q_prefix'
EOF
cat >> "$PREFIX/bin/$CMD.tmp" <<'EOF'
root="$(cd "$PREFIX/current" 2>/dev/null && pwd -P)" || { echo "$(basename "$0"): no version installed in $PREFIX" >&2; exit 1; }
# the version it runs, for `bise doctor` (like the dev launcher's)
[ "${1:-}" = --launcher-root ] && { echo "$root"; exit 0; }
# the user's folder (never one inherited from an agent's shell)
export SB_LAUNCH_DIR="$PWD"
exec "$root/bise" "$@"
EOF
chmod 755 "$PREFIX/bin/$CMD.tmp"
mv -f "$PREFIX/bin/$CMD.tmp" "$PREFIX/bin/$CMD"

mkdir -p "$BIN_DIR"
if [ -e "$BIN_DIR/$CMD" ] && [ ! -L "$BIN_DIR/$CMD" ]; then
  die "$BIN_DIR/$CMD exists and is not our link; remove it or pass --bin-dir"
fi
ln -sfn "$PREFIX/bin/$CMD" "$BIN_DIR/$CMD"
# the old name, for one release: the same launcher (it prints the name it
# was called by); an install before BISE-165 left a launcher file there
rm -f "$PREFIX/bin/$OLD_CMD"
if [ ! -e "$BIN_DIR/$OLD_CMD" ] || [ -L "$BIN_DIR/$OLD_CMD" ]; then
  ln -sfn "$PREFIX/bin/$CMD" "$BIN_DIR/$OLD_CMD"
fi

# ---- keep the last $KEEP versions, never the current one nor one a hub runs ----
in_use="$(ps -axo command= 2>/dev/null | grep -F "$PREFIX/versions/" | grep -v grep || true)"
n=0
for d in $(ls -t "$PREFIX/versions"); do
  case "$d" in .*) continue ;; esac
  n=$((n + 1))
  [ "$n" -le "$KEEP" ] && continue
  [ "$d" = "$id" ] && continue
  printf '%s' "$in_use" | grep -qF "$PREFIX/versions/$d/" && continue
  rm -rf "$PREFIX/versions/$d"
done

# ---- PATH ----
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    if [ "$MODIFY_PATH" = 1 ]; then
      for f in $(rc_files); do
        mkdir -p "$(dirname "$f")"
        grep -qF -e "$MARK" -e "$OLD_MARK" "$f" 2>/dev/null && continue
        case "$f" in
          *.fish) printf 'fish_add_path %s %s\n' "$BIN_DIR" "$MARK" >> "$f" ;;
          *) printf 'export PATH="%s:$PATH" %s\n' "$BIN_DIR" "$MARK" >> "$f" ;;
        esac
        say "added $BIN_DIR to PATH in $f (open a new terminal)"
      done
    else
      say "$BIN_DIR is not on your PATH: add it yourself"
    fi ;;
esac

say "installed $CMD $id in $PREFIX ($(du -sh "$PREFIX/versions/$id" | cut -f1))"
if [ "$PREFIX" != "$OLD_PREFIX" ] && [ -d "$OLD_PREFIX/versions" ]; then
  say "an older install is in $OLD_PREFIX (no longer used): remove it with  rm -rf '$OLD_PREFIX'"
fi
say "next: '$CMD' in a project folder (it asks for an API key the first time; '$CMD login' stores one)"
