#!/usr/bin/env python3
"""scripts/runtime_check.sh: a runtime whose viewer (or Sheets' view) loads the xlsx grid must
carry office-sheet.js beside it (xlsx-fix: the app's roots and versions/9552c21a8 shipped
without it, the viewer's import a 404), and every build that ships one runs the check.
Temp dirs only; < 1 s.

  python3 tests/runtime_check_test.py
"""
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CHECK = os.path.join(ROOT, "scripts", "runtime_check.sh")
fails = []


def check(ok, what):
    if not ok:
        fails.append(what)
        print("FAIL %s" % what)


def put(d, rel, text="x"):
    p = os.path.join(d, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w") as f:
        f.write(text)


def run(d):
    r = subprocess.run(["bash", CHECK, d], capture_output=True, text=True)
    return r.returncode, r.stderr


def root(grid_import=True, office=True, grid=True):
    d = tempfile.mkdtemp(prefix="rtcheck-")
    put(d, "kit/office/viewer.js", "import('./sheet.js')")
    put(d, "kit/office/sheet.js", 'import("/kit/office/office-sheet.js")' if grid_import else "// the old HTML table")
    if office:
        put(d, "office/dist/office.js")
    if grid:
        put(d, "office/dist/office-sheet.js")
    return d


rc, _ = run(root())
check(rc == 0, "a whole runtime passes")
rc, err = run(root(grid=False))
check(rc == 1 and "office/dist/office-sheet.js" in err, "a runtime whose sheet.js loads the grid, without it, fails and says so: %r" % err)
rc, err = run(root(office=False))
check(rc == 1 and "office/dist/office.js" in err, "a runtime with the viewer and no office.js fails: %r" % err)
rc, _ = run(root(grid_import=False, grid=False))
check(rc == 0, "an older runtime (sheet.js loads no grid) needs none")
d = root(grid=False)
put(d, "office/dist/office-sheet.js", "")
rc, _ = run(d)
check(rc == 1, "an empty grid bundle is no grid")

# Sheets' grid in its view folder (architect m_60401): the view loads it, the runtime carries it
GRID = "plugins/sheets/ai.mistral.vibe/views/grid"
def sheets(grid=True):
    d = root(grid_import=False, grid=False)
    put(d, GRID + "/book.js", 'import("./office-sheet.js")')
    if grid:
        put(d, GRID + "/office-sheet.js")
    return d
rc, _ = run(sheets())
check(rc == 0, "Sheets' view with its grid passes")
rc, err = run(sheets(grid=False))
check(rc == 1 and GRID + "/office-sheet.js" in err, "Sheets' view without its grid fails and says so: %r" % err)

# every build that ships a runtime runs it on what it made
for rel, needle in (("scripts/versions.sh", 'scripts/runtime_check.sh" "$tmp"'),
                    ("packaging/build-dist.sh", 'scripts/runtime_check.sh" "$app"'),
                    ("scripts/try_build.py", "scripts/runtime_check.sh %s"),
                    ("apps/desktop/electron-builder.mjs", "scripts/runtime_check.sh")):
    with open(os.path.join(ROOT, rel)) as f:
        check(needle in f.read(), "%s runs scripts/runtime_check.sh" % rel)

print("runtime_check: %s" % ("%d FAIL" % len(fails) if fails else "all pass"))
sys.exit(1 if fails else 0)
