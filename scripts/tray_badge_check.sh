#!/usr/bin/env bash
# tray_badge_check.sh <bise.app>: the packed app carries the tray's wait-dot addon whole
# (apps/desktop/native/mac/badge/TrayBadge.m, src/main/traybadge.ts): Contents/Resources/
# tray-badge.node, a universal (arm64 + x86_64) Mach-O bundle that exports N-API's
# napi_register_module_v1 and whose signature verifies. A packaging or signing break shows here
# (scripts/nightly_desktop.sh, after the candidate), not as a missing dot on his bar: the app
# would only fall back to the title dot. Exit 0 ok, 1 with the reason.
set -uo pipefail
app="${1:?usage: tray_badge_check.sh <bise.app>}"
f="$app/Contents/Resources/tray-badge.node"
[ -f "$f" ] || { echo "tray badge: no $f"; exit 1; }
archs="$(lipo -archs "$f" 2>/dev/null)"
for a in arm64 x86_64; do
  [[ " $archs " == *" $a "* ]] || { echo "tray badge: $f has no $a slice (archs: ${archs:-none})"; exit 1; }
done
nm -gU -arch arm64 "$f" 2>/dev/null | grep -q '_napi_register_module_v1$' || { echo "tray badge: $f exports no napi_register_module_v1"; exit 1; }
codesign --verify --strict "$f" 2>/dev/null || { echo "tray badge: $f: codesign --verify failed"; exit 1; }
echo "tray badge: ok ($archs)"
