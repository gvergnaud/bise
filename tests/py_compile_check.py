#!/usr/bin/env python3
"""Every tracked Python file compiles (main m_59170): 22cbd23c2 landed
tests/switch_intruder_e2e.py with a literal newline in a string (a SyntaxError at line 115), and
the quick gate never saw it, since an e2e file runs only after the land: the tip gate went red
and the 11.10 release waited.

The files: `git ls-files '*.py'` (tracked only: no venv, target or node_modules), minus SKIP
(architect m_59182: a deny-list, so a new folder of .py is covered without editing this file).
Compiled in memory (compile(), no .pyc written), with the python3 that runs the tests; about a
second. The quick gate's light lane runs it on any .py change.

    py_compile_check.py [<repo root>]     rc 0, or 1 with each file, line and error
"""
import os
import subprocess
import sys

# not compiled here, each with why
SKIP = (
    "docs/",  # docs/brand/readme/make.py uses 3.12-only f-string syntax; the python3 here is 3.9
)


def files(root):
    """The tracked .py files of root, minus SKIP, sorted (absolute paths)."""
    out = subprocess.run(["git", "ls-files", "-z", "*.py"], cwd=root, capture_output=True, text=True, check=True).stdout
    return sorted(os.path.join(root, f) for f in out.split("\0") if f and not f.startswith(SKIP))


def errors(root):
    """[(path relative to root, line, message)] of the files that do not compile."""
    bad = []
    for f in files(root):
        try:
            with open(f, "rb") as fh:
                compile(fh.read(), f, "exec", dont_inherit=True)
        except SyntaxError as e:
            bad.append((os.path.relpath(f, root), e.lineno or 0, e.msg))
        except ValueError as e:  # a null byte
            bad.append((os.path.relpath(f, root), 0, str(e)))
    return bad


def main():
    root = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), ".."))
    bad = errors(root)
    if bad:
        for f, line, msg in bad:
            print("py_compile_check: %s:%d: %s" % (f, line, msg))
        print("py_compile_check: %d file(s) do not compile (python %s)" % (len(bad), sys.version.split()[0]))
        return 1
    print("py_compile_check: ok (%d files)" % len(files(root)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
