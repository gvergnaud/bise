#!/usr/bin/env bash
# build-dist.sh — PROTOTYPE: build a self-contained, installable tarball
# of the harness + Switchboard for the current platform (tested on
# macOS arm64) from one git commit.
#
#   packaging/build-dist.sh [<rev>] [--out <dir>]
#
#   <rev>   a commit (default HEAD; uncommitted changes are NOT shipped)
#   --out   where the tarball lands (default /tmp/bise-dist)
#
# Output: <out>/bise-<id>-<os>-<arch>.tar.gz (+ .sha256), with
#   bise-<id>-<os>-<arch>/
#     install.sh           the installer (the same file curl | sh fetches)
#     app/                 the app root, exactly as versions.sh builds it:
#       bise                 Rust: TUI + Switchboard daemon + sb CLI; on
#                            macOS a link to bise.app/Contents/MacOS/bise
#       bise.app             macOS: the bundle dev.bise.cli around it, so
#                            one folder grant lasts across versions (step 5)
#       sb                   -> bise (the agents' command: bise called as sb)
#       repl-live            native Bend REPL (runtime/repl-live.bend)
#       repl-scripted        native Bend REPL, no provider (runtime/repl.bend)
#       sb-core              native Bend hub core (hub/main.bend)
#       tool-desc-*.txt prompt-*.txt   read at run time (relative paths)
#       bend-jsrt            the V8 engine (RELEASE build; a commit before
#                            BISE-114: at rust/jsrt/target/debug/bend-jsrt,
#                            the path its runtime expects)
#       VERSION              id, commit, subject, built, bend_hash, macos,
#                            target
#       LICENSE NOTICE THIRD_PARTY_NOTICES   Apache-2.0 + what we ship of
#                            others (the commit's; the tree's before them)
#
# Every binary must run on the macOS target (rust/.cargo/config.toml,
# BISE-164): checked before packing (./bins.sh minos).
#
# It reuses versions.sh (its caches: cargo target dir, Bend compiles per
# source hash), so a commit already built by /version costs seconds.
# Nothing is published: the tarball stays on disk.

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE" && git rev-parse --show-toplevel)"
# the same dir as bise_home (rust/home): $BISE_HOME/dev, else ~/.bise/dev
STATE="${BISE_HOME:-$HOME/.bise}/dev"
BUILD="${SB_BUILD_DIR:-$STATE/build}"
export PATH="$HOME/.cargo/bin:$HOME/.bend/bin:$PATH"
# never the dev identity nor its bise.app (dev-sign.sh is for this Mac's
# dev builds): a release makes its own bise.app (dev.bise.cli, step 5),
# signed with BISE_SIGN_ID. Without this, CI's first versions.sh made a
# keychain on the runner.
export SB_DEV_SIGN=0

rev=HEAD
out=/tmp/bise-dist
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) rev="$1" ;;
  esac
  shift
done

say() { echo "build-dist: $*" >&2; }

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
target="$os-$arch"

cd "$REPO"
commit="$(git rev-parse "$rev^{commit}")"
id="$(git rev-parse --short "$commit")"
name="bise-$id-$target"

# 1. the app root of this commit (cached by versions.sh)
say "versions.sh build $id..."
# (--also: repl-scripted compiles with the version's own Bend binaries,
# beside its cargo build, instead of after it)
vdir="$("$REPO/scripts/versions.sh" build "$id" --also repl-scripted)"
h="$(sed -n 's/^bend_hash=//p' "$vdir/VERSION")"

# 2. repl-scripted (versions.sh puts it in bins.sh's cache only, and only
#    when it builds the version): bins.sh's cache, same key as repl-live
#    (content hash of runtime/ core/ vendor/); a version built before
#    without it compiles it here
scripted="$BUILD/cache/repl-scripted-$h"
if [ ! -x "$scripted" ]; then
  wt="/tmp/bise-dist-src-$id-$$"
  git worktree add -q --detach "$wt" "$commit"
  trap 'git -C "$REPO" worktree remove --force "$wt" 2>/dev/null || true' EXIT
  scripted="$("$REPO/scripts/bins.sh" path --src "$wt" repl-scripted)"
  git worktree remove --force "$wt"; trap - EXIT
fi

# 3. the V8 engine, release build: the version's own (versions.sh builds
#    it from the commit's rust/jsrt since BISE-133, VERSION has jsrt=),
#    else (an older version dir: its engine is the debug one) the tree's,
#    from bins.sh's cache: warn when the commit's rust/jsrt differs.
if grep -q '^jsrt=release' "$vdir/VERSION"; then
  js="$vdir/bend-jsrt"
else
  js="$("$REPO/scripts/bins.sh" path bend-jsrt)"
  if ! git diff --quiet "$commit" -- rust/jsrt/src rust/jsrt/Cargo.toml rust/jsrt/Cargo.lock; then
    say "WARNING: rust/jsrt differs between $id and the working tree; the shipped engine is the tree's"
  fi
fi

# 4. stage
stage="$(mktemp -d /tmp/bise-stage.XXXXXX)"
trap 'rm -rf "$stage"' EXIT
app="$stage/$name/app"
# where this commit's runtime looks for the engine: BEND_JSRT_BIN (set by
# the harness to app/bend-jsrt) since BISE-114, a fixed path before
jsrt_at=bend-jsrt
{ git show "$commit:bend/runtime/main.bend" 2>/dev/null || git show "$commit:runtime/main.bend"; } \
  | grep -c BEND_JSRT_BIN >/dev/null || jsrt_at=rust/jsrt/target/debug/bend-jsrt
mkdir -p "$app/$(dirname "$jsrt_at")"
# the command (BISE-165): bise, or the bend-harness of a version dir
# built before the rename (v2026.9.29), its real executable. No
# bend-harness link in the app any more: its one release has shipped.
if [ -e "$vdir/bise" ]; then cp "$vdir/bise" "$app/bise"; else cp "$vdir/bend-harness" "$app/bise"; fi
# sb: the agents' command, the same binary called by that name
ln -s bise "$app/sb"
for f in repl-live sb-core; do cp "$vdir/$f" "$app/$f"; done
cp "$scripted" "$app/repl-scripted"
# the prompts: prompts/ since the root cleanup, at the top of an older version
if [ -d "$vdir/prompts" ]; then cp -R "$vdir/prompts" "$app/prompts"; pr=prompts/
else cp "$vdir"/tool-desc-*.txt "$vdir"/prompt-*.txt "$app/"; pr=""; fi
# the built-in plugins and computer use's browser extension (versions.sh
# puts them in the version dir; read at run time from the app root)
if [ -d "$vdir/plugins" ]; then cp -R "$vdir/plugins" "$app/plugins"; fi
# the office SDK's bundle, for `bise office run` (docs/office.md)
if [ -d "$vdir/office/dist" ]; then mkdir -p "$app/office"; cp -R "$vdir/office/dist" "$app/office/dist"; fi
if [ -d "$vdir/computer-use/extension" ]; then mkdir -p "$app/computer-use"; cp -R "$vdir/computer-use/extension" "$app/computer-use/extension"; fi
# the pages kit (CSS, JS, fonts, the start-here page): the page server
# serves /kit from <app root>/kit. Without it (v2026.10.2-31, -32) every
# page and html artifact showed with no CSS. An older version dir has it
# at apps/ambient/kit, where its own page server reads it.
kit=""
if [ -d "$vdir/kit" ]; then cp -R "$vdir/kit" "$app/kit"; kit=kit
elif [ -d "$vdir/apps/ambient/kit" ]; then mkdir -p "$app/apps/ambient"; cp -R "$vdir/apps/ambient/kit" "$app/apps/ambient/kit"; fi
# whole for the viewer (office/dist with the xlsx grid, never committed): never a release without it
"$REPO/scripts/runtime_check.sh" "$app" || { say "$id is not whole for the viewer (scripts/runtime_check.sh): build its version again"; exit 1; }
cp "$js" "$app/$jsrt_at"
# VERSION: the dev repo path means nothing on the user's machine (the
# daemon would look for versions.sh there); the target says what it runs on
grep -v '^repo=' "$vdir/VERSION" > "$app/VERSION"
echo "target=$target" >> "$app/VERSION"
echo "channel=${BISE_CHANNEL:-dev}" >> "$app/VERSION"
# the license and the third-party notices (Apache-2.0 4(a)/(d)): the
# commit's; a commit before them gets the working tree's
for f in LICENSE NOTICE THIRD_PARTY_NOTICES; do
  git show "$commit:$f" > "$app/$f" 2>/dev/null || cp "$REPO/$f" "$app/$f"
done
cp "$HERE/install.sh" "$stage/$name/install.sh"
chmod +x "$stage/$name/install.sh"

# 5. the bundle (macOS): app/bise.app (dev.bise.cli) around the command,
#    app/bise a link into it (scripts/mac_sign.py bundle, the one builder:
#    the desktop app's runtime gets the same). macOS keys a folder grant on
#    a bundle's id + signature, a plain file's on its path: every version is
#    a new dir, so a plain bise asked again after each update (docs/issues/
#    15-tcc-one-grant.md). Signed below, after the helper (step 5c).
if [ "$os" = darwin ]; then
  vid="$(sed -n 's/^id=//p' "$app/VERSION")"
  mv "$app/bise" "$stage/bise.exe"
  python3 -B "$REPO/scripts/mac_sign.py" bundle "$app/bise.app" "$stage/bise.exe" \
    --id dev.bise.cli --version "${vid:-$id}" --link
  rm -f "$stage/bise.exe"
fi

# 5b. computer use's helper (macOS, docs/computer-use-design.md §5.2):
#     app/bise Computer Use.app from the commit's computer-use/macos-app,
#     built ad hoc by its bundle.sh (signed with the rest in 5c). Cached by
#     the folder's tree hash.
helper=""
if [ "$os" = darwin ] && git cat-file -e "$commit:computer-use/macos-app/Package.swift" 2>/dev/null; then
  hkey="$(git rev-parse "$commit:computer-use/macos-app")"
  hcache="$BUILD/cache/cu-helper-$hkey"
  if [ ! -d "$hcache/bise Computer Use.app" ]; then
    say "building the computer-use helper ($hkey)..."
    hsrc="$(mktemp -d /tmp/bise-cu-helper.XXXXXX)"
    git archive "$commit" computer-use/macos-app | tar -x -C "$hsrc"
    rm -rf "$hcache.tmp" && mkdir -p "$hcache.tmp"
    env -u BISE_SIGN_ID -u BISE_NOTARY_PROFILE "$hsrc/computer-use/macos-app/scripts/bundle.sh" --out "$hcache.tmp" >/dev/null \
      || { rm -rf "$hsrc" "$hcache.tmp"; say "the computer-use helper did not build"; exit 1; }
    rm -rf "$hsrc" "$hcache" && mv "$hcache.tmp" "$hcache"
  fi
  helper="$app/bise Computer Use.app"
  ditto "$hcache/bise Computer Use.app" "$helper"
fi

# 5c. sign, NEVER strip (scripts/dist_sign.py): every piece of code inside
#     out, hardened runtime, the entitlements of packaging/entitlements/
#     (bise.app: the mic, Apple Events; bend-jsrt: V8's JIT; repl-live,
#     repl-scripted: none, their TLS is macOS's own), each checked.
#     BISE_SIGN_ID (+ BISE_SIGN_KEYCHAIN): a Developer ID, whose designated
#     requirement (identifier + team, no hash) is the same for every
#     version; else ad hoc. BISE_NOTARIZE=1: one notarization of the whole
#     app root, the bundles stapled (bare binaries and the tarball can't
#     be). BISE_SIGN_REQUIRED=1 (release.yml on a v* tag or input sign): no
#     identity or no notary credentials fails here, never an ad hoc release.
#     Never strip: the endpoint security agent (CrowdStrike) on the dev Macs
#     deleted a stripped sb-core within a second of its creation; unstripped
#     binaries are left alone (docs/packaging.md §6).
if [ "$os" = darwin ]; then
  sargs=()
  [ "${BISE_SIGN_REQUIRED:-0}" = 1 ] && sargs+=(--required)
  [ "${BISE_NOTARIZE:-0}" = 1 ] && sargs+=(--notarize --work "$stage/notary")
  # ${a[@]+...}: macOS /bin/bash 3.2 calls an empty array unbound under set -u
  # (an unsigned build: v2026.10.2-39's darwin jobs stopped here)
  python3 -B "$REPO/scripts/dist_sign.py" "$app" ${sargs[@]+"${sargs[@]}"} \
    || { say "signing failed"; exit 1; }
  rm -rf "$stage/notary"
fi

# 6. verify: a missing piece must fail here, not on the user's machine
#    (wait a moment: a quarantine by security software is not instant)
sleep 2
for f in bise sb repl-live repl-scripted sb-core "$jsrt_at" \
         "${pr}tool-desc-bash.txt" "${pr}prompt-tool-use.txt" "${pr}prompt-tone.txt" VERSION \
         LICENSE NOTICE THIRD_PARTY_NOTICES; do
  [ -e "$app/$f" ] || { say "INCOMPLETE: app/$f missing"; exit 1; }
done
# ... and its pages serve their kit: the hub of this app root, started on
# a throwaway HOME, answers every stylesheet and script a page links with
# 200 and its type (a commit with kit/ at the root, the page server's: a
# release without app/kit fails here, whatever the version dir holds)
if git cat-file -e "$commit:kit/kit.css" 2>/dev/null; then
  [ -n "$kit" ] && [ -e "$app/kit/kit.css" ] || { say "INCOMPLETE: app/kit missing (the commit has kit/)"; exit 1; }
  "$HERE/check-dist-pages.sh" "$app" || { say "INCOMPLETE: the pages do not serve their kit"; exit 1; }
fi
# ... and no binary needs a macOS newer than the target (a version dir
# built before BISE-164 has Bend binaries for the build machine's macOS:
# remove it and run again)
"$REPO/scripts/bins.sh" minos "$app/bise" "$app/repl-live" "$app/repl-scripted" \
  "$app/sb-core" "$app/$jsrt_at" >&2 \
  || { say "a binary needs a newer macOS than $("$REPO/scripts/bins.sh" macos-target): rm -rf $vdir and run again"; exit 1; }
# ... and the helper is whole: signature, macOS target
if [ -n "$helper" ]; then
  codesign --verify --strict "$helper" || { say "INCOMPLETE: $helper: bad signature"; exit 1; }
  "$REPO/scripts/bins.sh" minos "$helper/Contents/MacOS/bise-computer-use" >&2 \
    || { say "the computer-use helper needs a newer macOS than the target"; exit 1; }
fi

# 7. pack
mkdir -p "$out"
tarball="$out/$name.tar.gz"
tar -C "$stage" -czf "$tarball.tmp" "$name"
mv "$tarball.tmp" "$tarball"
(cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
say "app root: $(du -sh "$app" | cut -f1), tarball: $(du -h "$tarball" | cut -f1)"
echo "$tarball"
