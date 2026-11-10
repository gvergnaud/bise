#!/usr/bin/env python3
"""Every word of the catalog is read by a client (architect m_27078 c, m_58405): Rust reads it as
`<area>::<name>` (rust/, outside the catalog rust/proto/src/words), the desktop as `<area>.<name>`
(apps/desktop/src, outside the generated files and the tests). A word nothing reads is a dead key:
delete it.

The keys come from tests/words/*.gen.json (generated from the catalog, pinned to it by bise-proto's
ts test), so this is a text scan with no cargo: the quick gate's light lane runs it on a .rs, a
desktop .ts/.tsx or a words .gen.json change (4ff9ec771 dropped two reads in desktop TS only and
the Rust test that held this rule never ran in that quick).

    words_read_check.py [<repo root>]     rc 0, or 1 with the dead keys
"""
import glob
import json
import os
import re
import sys

SKIP_DIRS = {"target", "vendor", "node_modules", "out", "dist"}


def keys(root):
    """Every catalog key, '<area>.<name>', from tests/words/*.gen.json."""
    out = []
    for f in sorted(glob.glob(os.path.join(root, "tests", "words", "*.gen.json"))):
        out += [k for k in json.load(open(f, encoding="utf-8")) if k != "_"]
    return out


def sources(top, exts, skip):
    """The text of every file under `top` with one of `exts`, but `skip`'s and SKIP_DIRS'."""
    texts = []
    for d, dirs, files in os.walk(top):
        dirs[:] = [x for x in dirs if x not in SKIP_DIRS and not x.startswith(".") and not skip(os.path.join(d, x))]
        for f in files:
            p = os.path.join(d, f)
            if f.startswith(".") or skip(p) or not f.endswith(exts):
                continue
            try:
                texts.append(open(p, encoding="utf-8").read())
            except (OSError, UnicodeDecodeError):
                pass
    return texts


def pairs(texts, sep):
    """Every '<a><sep><b>' read whole in `texts`: each adjacent pair of a maximal name path
    (a::b::c reads a::b and b::c; a name character never touches either end), one pass."""
    out = set()
    path = re.compile(r"[A-Za-z0-9_]+(?:%s[A-Za-z0-9_]+)+" % re.escape(sep))
    for t in texts:
        for m in path.finditer(t):
            names = m.group(0).split(sep)
            out.update("%s.%s" % (a, b) for a, b in zip(names, names[1:]))
    return out


def dead(root):
    """The keys no client reads."""
    catalog = os.path.join(root, "rust", "proto", "src", "words")
    rust = pairs(sources(os.path.join(root, "rust"), (".rs",), lambda p: p.startswith(catalog)), "::")
    ts = pairs(sources(os.path.join(root, "apps", "desktop", "src"), (".ts", ".tsx"),
                       lambda p: os.path.basename(p).endswith(".gen.ts") or ".test." in os.path.basename(p)), ".")
    return [k for k in keys(root) if k not in rust and k not in ts]


def main(argv):
    root = argv[0] if argv else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    if not keys(root):
        print("words_read_check: no tests/words/*.gen.json under %s" % root)
        return 1
    d = dead(root)
    if d:
        print("words no client reads (delete them, or read them as <area>::<name> / <area>.<name>): %s" % d)
        return 1
    print("words_read_check: ok (%d words, each read by a client)" % len(keys(root)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
