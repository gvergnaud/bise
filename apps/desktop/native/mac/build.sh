#!/usr/bin/env bash
# build.sh — compile bise-mac-helper with swiftc and the Command Line Tools
# (no Xcode project). Electron's packaging puts the binary inside bise's
# app (Contents/MacOS), so macOS charges its permissions to the app.
#
#   build.sh --check        type-check every source, no binary
#   build.sh --test         the pure laws (keys, front rules, quiet, the lines against
#                           rust/proto/fixtures/helper_*.jsonl) through the swift
#                           interpreter (JIT in memory: no binary on disk)
#   build.sh <out path>     build the binary there and sign it (one stable path:
#                           the EDR deletes fresh unsigned binaries), and next to it
#                           tray-badge.node (badge/TrayBadge.m: the in-process N-API
#                           addon that draws the tray's wait dot, src/main/traybadge.ts),
#                           universal (arm64 + x86_64), signed the same way
#
# MACOSX_DEPLOYMENT_TARGET: from the caller, 14.0 at least (ScreenCaptureKit
# screenshots). HELPER_SIGN_ID: a codesigning identity
# (the app's); without it, ad hoc.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.0}"
ARCH="$(uname -m)"
SDK="$(xcrun --show-sdk-path)"
SOURCES=("$HERE"/src/*.swift)
FLAGS=(-swift-version 5 -target "$ARCH-apple-macos$TARGET" -sdk "$SDK"
       -framework AppKit -framework ScreenCaptureKit -framework AVFoundation
       -framework ApplicationServices -framework CoreAudio -framework CoreMediaIO
       -framework QuickLookThumbnailing)

case "${1:-}" in
  --check)
    swiftc -typecheck "${FLAGS[@]}" "${SOURCES[@]}"
    echo "bise-mac-helper: type-check ok (${#SOURCES[@]} files)"
    clang -fsyntax-only -fobjc-arc -isysroot "$SDK" -mmacosx-version-min="$TARGET" "$HERE/badge/TrayBadge.m"
    echo "tray-badge: syntax ok"
    ;;
  --test)
    t="$(mktemp -d "${TMPDIR:-/tmp}/helper-test.XXXXXX")"
    export HELPER_FIXTURES="$ROOT/rust/proto/fixtures"
    rc=0
    # the pure sources (a+b: concatenated) with their laws, one interpreter run each
    for pair in KeyMachine:KeyMachineTests FrontRules:FrontRulesTests Quiet:QuietTests \
                KeyMachine+FrontRules+Lines:LinesTests; do
      srcs="${pair%%:*}"; test="${pair#*:}"
      : > "$t/$test.swift"
      for s in ${srcs//+/ }; do cat "$HERE/src/$s.swift" >> "$t/$test.swift"; done
      cat "$HERE/test/$test.swift" >> "$t/$test.swift"
      swift "$t/$test.swift" || rc=1
    done
    rm -rf "$t"
    exit $rc
    ;;
  ""|-*)
    echo "usage: build.sh --check | --test | <out path>" >&2; exit 2 ;;
  *)
    OUT="$1"
    mkdir -p "$(dirname "$OUT")"
    # unlink first: a running copy keeps its inode, we never write into it
    rm -f "$OUT"
    swiftc -O "${FLAGS[@]}" -o "$OUT" "${SOURCES[@]}"
    codesign --force --sign "${HELPER_SIGN_ID:--}" --identifier dev.bise.mac-helper "$OUT" >/dev/null
    echo "bise-mac-helper: built $OUT"
    # N-API only: its few functions are resolved from the host at load, no headers, no node-gyp
    BADGE="$(dirname "$OUT")/tray-badge.node"
    rm -f "$BADGE"
    clang -bundle -fobjc-arc -O2 -arch arm64 -arch x86_64 -mmacosx-version-min="$TARGET" \
      -isysroot "$SDK" -undefined dynamic_lookup -framework AppKit -framework QuartzCore \
      -o "$BADGE" "$HERE/badge/TrayBadge.m"
    codesign --force --sign "${HELPER_SIGN_ID:--}" --identifier dev.bise.tray-badge "$BADGE" >/dev/null
    echo "tray-badge: built $BADGE"
    ;;
esac
