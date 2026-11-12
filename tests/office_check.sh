#!/usr/bin/env bash
# The office SDK's check (docs/office.md, plan §9.1): office/dist/office.js
# is committed, so it must be what office/sdk/src builds. Typecheck, rebuild
# in memory and fail on a diff (a stale or hand-edited bundle), then the
# SDK's tests on that bundle.
#   tests/office_check.sh          no pnpm or node_modules: says "skipped", exit 0
#   tests/office_check.sh strict   installs node_modules from the lockfile; no pnpm: exit 1
# gate.sh quick runs it when office/ changed, gate.sh full every time.
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
sdk="$root/office/sdk"
[ -d "$sdk" ] || exit 0
strict="${1:-}"
miss=""
if ! command -v pnpm >/dev/null 2>&1; then
  for d in "${FNM_DIR:-$HOME/.local/share/fnm}/aliases/default/bin" "$HOME/.volta/bin" "$HOME/Library/pnpm"; do
    if [ -x "$d/pnpm" ]; then export PATH="$d:$PATH"; break; fi
  done
fi
if ! command -v pnpm >/dev/null 2>&1; then miss="no pnpm on PATH"
elif [ "$strict" = strict ]; then
  # every time, not only with no node_modules: a kept tree's (the tip gate's)
  # can predate a new dependency (Univer: TS2307 on 6a79da476); a no-op when current
  (cd "$sdk" && pnpm -s install --frozen-lockfile --prefer-offline) || miss="pnpm install failed in office/sdk"
elif [ ! -d "$sdk/node_modules" ]; then miss="no node_modules; run pnpm -C office/sdk install"
fi
if [ -n "$miss" ]; then
  if [ "$strict" = strict ]; then echo "FAIL office_check: $miss"; exit 1; fi
  echo "skipped office_check: $miss"; exit 0
fi
# the xlsx grid's bundle (office-sheet.js) is built, never committed (architect
# m_37322): it must be there and built from these sources (scripts/bins.sh writes
# the key it was built from beside it). Built here first, like the gate's sb-core
# (a fresh tree, the tip gate's, has none): from bins.sh's cache, a copy on a hit
"$root/scripts/bins.sh" office-sheet || { echo "FAIL office_check: scripts/bins.sh office-sheet failed"; exit 1; }
want="$("$root/scripts/bins.sh" key office-sheet 2>/dev/null)"
grid="plugins/sheets/ai.mistral.vibe/views/grid/office-sheet.js"
have="$(cat "$root/$grid.key" 2>/dev/null)"
if [ -z "$want" ] || [ ! -f "$root/$grid" ] || [ "$want" != "$have" ]; then
  echo "FAIL office_check: $grid is missing or stale for its sources: run scripts/bins.sh office-sheet"; exit 1
fi
cd "$sdk" && pnpm -s check
