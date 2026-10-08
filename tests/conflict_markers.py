#!/usr/bin/env python3
"""No merge or stash conflict markers in tracked files (ambient-lead m_12395: stash markers
landed on ambient-app in 2393001f and nothing caught them).

A marker is a whole line that starts with '<<<<<<< ' or '>>>>>>> ', or is exactly '======='
(git's three markers). Files that need such lines (a test fixture of a conflict) are listed in
ALLOWED, by path. Binary files are skipped. Fast: one `git grep` over the index.

  python3 tests/conflict_markers.py              exit 1 and the lines when any marker is found
  python3 tests/conflict_markers.py --self-test  the rule on synthetic lines

Run by tests/gate.sh quick and apps/desktop's `pnpm check`.
"""
import re
import subprocess
import sys
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MARKER = re.compile(r"^(<<<<<<< |=======$|>>>>>>> )")
# files whose conflict lines are their point (a fixture of a conflict); none today
ALLOWED = frozenset()


def is_marker(line):
    """The line (without its newline) is one of git's conflict markers. Pure."""
    return bool(MARKER.match(line.rstrip("\r")))


def found(root=ROOT):
    """[(path, line number, line)] of every marker in the tracked files, ALLOWED left out."""
    r = subprocess.run(["git", "-C", root, "grep", "-n", "-I", "-E", "--no-color", r"^(<<<<<<< |=======$|>>>>>>> )"],
                       capture_output=True, text=True)
    if r.returncode not in (0, 1):
        raise SystemExit("conflict-markers: git grep failed: " + r.stderr.strip())
    out = []
    for row in r.stdout.splitlines():
        path, n, line = row.split(":", 2)
        if path not in ALLOWED and is_marker(line):
            out.append((path, int(n), line))
    return out


def self_test():
    good = ["<<<<<<< Updated upstream", "=======", ">>>>>>> Stashed changes", ">>>>>>> sb/amb-tools", "=======\r"]
    bad = ["<<<<<<<", "======= ", "========", "  =======", "a <<<<<<< b", ">>>>>>>x", "== heading", "<<<<<<<< x"]
    wrong = [l for l in good if not is_marker(l)] + [l for l in bad if is_marker(l)]
    if wrong:
        raise SystemExit("conflict-markers self-test failed on: %r" % wrong)


def main():
    self_test()
    if "--self-test" in sys.argv:
        print("conflict-markers: self-test ok")
        return
    hits = found()
    if hits:
        for path, n, line in hits:
            print("%s:%d: %s" % (path, n, line))
        print("conflict-markers: %d conflict marker line(s) in tracked files: resolve them (a fixture that needs one goes in ALLOWED)" % len(hits))
        sys.exit(1)
    print("conflict-markers: none in tracked files")


if __name__ == "__main__":
    main()
