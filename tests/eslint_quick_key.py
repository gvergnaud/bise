#!/usr/bin/env python3
"""The quick gate's eslint cache key: wipe the cache when exported types change.

`pnpm lint:cached` (quick gate only) runs eslint with --cache. Eslint's cache
keys each file on its own content, but typed rules (no-floating-promises,
no-unsafe-*) read the types of the files it imports: change a callee to return
a Promise and the unchanged caller's red would stay hidden in the cache. So,
before the lint, this emits the declarations of the three tsconfigs (an
incremental tsc, its state kept under the tree's own cache dir) and hashes them
with the lockfile, the tsconfigs and the eslint config. A key different from
the stored one wipes the eslint cache: that lint runs cold. A body-only edit
keeps the key, so the lint stays warm.

Fail safe: any emit error or crash makes the key "missing": the cache is wiped
and the lint runs cold. The gate's verdict never depends on this emit; it is
only a cache key.

A deleted source leaves its old .d.ts in the outDir (tsc -p doesn't clean it), so
a deletion alone doesn't move the key. No prune is needed: typecheck runs first
and fails any file that still imports it, a deleted file nobody imports changes
no lint result, and a rename emits a new .d.ts, which moves the key (architect).

Run from apps/desktop. TSC overrides the tsc binary (tests).
"""

from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path

CACHE = Path("node_modules/.cache/eslint-quick")
LINT = CACHE / "lint"
SIG = CACHE / "types.sig"
CONFIGS = {
    "node": "tsconfig.node.json",
    "web": "tsconfig.web.json",
    "electron": "tests/harness/tsconfig.electron.json",
}
INPUTS = ["pnpm-lock.yaml", "tsconfig.base.json", *CONFIGS.values(), "eslint.config.mjs"]


def emit(name: str, cfg: str, tsc: str) -> subprocess.Popen[bytes]:
    out = CACHE / "dts" / name
    info = CACHE / f"{name}.dts.tsbuildinfo"
    if not out.is_dir():
        # tsc -p trusts its buildinfo and would not re-emit what it thinks is there.
        info.unlink(missing_ok=True)
    return subprocess.Popen(
        [tsc, "-p", cfg, "--declaration", "--emitDeclarationOnly", "--noEmit", "false",
         "--incremental", "--outDir", str(out), "--tsBuildInfoFile", str(info)],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    )


def key(tsc: str) -> str | None:
    """The hash of the emitted declarations and the inputs, or None when the emit fails."""
    try:
        jobs = [(name, emit(name, cfg, tsc)) for name, cfg in CONFIGS.items()]
    except OSError as e:
        print(f"eslint_quick_key: tsc did not start ({e}): the lint runs cold", file=sys.stderr)
        return None
    failed = False
    for name, job in jobs:
        out, _ = job.communicate()
        if job.returncode != 0:
            failed = True
            tail = out.decode(errors="replace").strip().splitlines()[-3:]
            print(f"eslint_quick_key: the {name} declaration emit failed (rc {job.returncode}): "
                  "the lint runs cold" + "".join(f"\n  {t}" for t in tail), file=sys.stderr)
    if failed:
        return None
    h = hashlib.sha256()
    dts = CACHE / "dts"
    for f in sorted(p for p in dts.rglob("*") if p.is_file()):
        h.update(f"{f.relative_to(dts)}\0{hashlib.sha256(f.read_bytes()).hexdigest()}\n".encode())
    for name in INPUTS:
        p = Path(name)
        digest = hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else "absent"
        h.update(f"{name}\0{digest}\n".encode())
    return h.hexdigest()


def main() -> int:
    CACHE.mkdir(parents=True, exist_ok=True)
    tsc = os.environ.get("TSC") or "node_modules/.bin/tsc"
    try:
        new = key(tsc)
    except Exception as e:  # a crash is a missing key, never a red
        print(f"eslint_quick_key: {e!r}: the lint runs cold", file=sys.stderr)
        new = None
    old = SIG.read_text().strip() if SIG.is_file() else None
    if new is not None and new == old:
        return 0
    shutil.rmtree(LINT, ignore_errors=True)
    if new is None:
        SIG.unlink(missing_ok=True)
    else:
        SIG.write_text(new + "\n")
        if old is not None:
            print("eslint_quick_key: exported types changed: the lint runs cold", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
