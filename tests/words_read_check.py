#!/usr/bin/env python3
"""Every word of the catalog is read by a client (architect m_27078 c, m_58405): Rust reads it as
`<area>::<name>` (rust/, outside the catalog rust/proto/src/words), the desktop, the kit and the
plugins' views as `<area>.<name>` (apps/desktop/src, kit/, plugins/; architect m_61995), outside
SKIP_FILES. A word nothing reads is a dead key: delete it. In kit/ and plugins/ an area is read by
its own name only: an alias import or a destructured area is red (a read it would hide).

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
# a file that is no read, in any client: generated (the catalog's own output), a test or a check
SKIP_FILES = (".gen.", ".test.", ".check.")
# the JS readers besides the desktop (architect m_61995): the kit (@bise/view's pill, ...) and
# the plugins' views read '<area>.<name>' too
JS_READERS = ("kit", "plugins")
JS_EXTS = (".ts", ".tsx", ".js", ".mjs")
# a JS reader takes an area by its own name only: `import { viewkit as w }`, `const { ask } =
# viewkit` or `const w = viewkit` would hide its reads from this scan (architect m_61995)
ALIASED = re.compile(r"\b(?:import|export)\s*(?:type\s+)?\{([^}]*)\}\s*from\s*[\"'][^\"']*words[^\"']*[\"']")


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


def no_read(p):
    """A file SKIP_FILES says is no read."""
    return any(s in os.path.basename(p) for s in SKIP_FILES)


def js_readers(root):
    """{path: text} of every kit/ and plugins/ JS or TS file that may read a word."""
    out = {}
    for top in JS_READERS:
        for d, dirs, files in os.walk(os.path.join(root, top)):
            dirs[:] = [x for x in dirs if x not in SKIP_DIRS and not x.startswith(".")]
            for f in files:
                p = os.path.join(d, f)
                if f.startswith(".") or no_read(p) or not f.endswith(JS_EXTS):
                    continue
                try:
                    out[p] = open(p, encoding="utf-8").read()
                except (OSError, UnicodeDecodeError):
                    pass
    return out


def aliased(root, texts):
    """'<file>: <how>' for each area a JS reader takes under another name (ALIASED)."""
    areas = {k.split(".")[0] for k in keys(root)}
    out = []
    for p, t in sorted(texts.items()):
        rel = os.path.relpath(p, root)
        taken = set()
        for m in ALIASED.finditer(t):
            for part in m.group(1).split(","):
                name = part.split()
                if name and name[0] in areas:
                    taken.add(name[0])
                if len(name) == 3 and name[1] == "as" and name[0] in areas and name[2] != name[0]:
                    out.append("%s: %s" % (rel, part.strip()))
        # an area this file imported from a words module, then took apart or under a new name
        for a in sorted(taken):
            for m in re.finditer(r"(\{[^{}]*\}|\b(?:const|let|var)\s+[A-Za-z_$][\w$]*)\s*=\s*%s\b(?!\s*\.)" % a, t):
                out.append("%s: %s" % (rel, " ".join(m.group(0).split())))
    return out


def dead(root, js=None):
    """The keys no client reads."""
    catalog = os.path.join(root, "rust", "proto", "src", "words")
    rust = pairs(sources(os.path.join(root, "rust"), (".rs",), lambda p: p.startswith(catalog)), "::")
    ts = pairs(sources(os.path.join(root, "apps", "desktop", "src"), (".ts", ".tsx"), no_read), ".")
    ts |= pairs((js if js is not None else js_readers(root)).values(), ".")
    return [k for k in keys(root) if k not in rust and k not in ts]


def main(argv):
    root = argv[0] if argv else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    if not keys(root):
        print("words_read_check: no tests/words/*.gen.json under %s" % root)
        return 1
    js = js_readers(root)
    hidden = aliased(root, js)
    if hidden:
        print("words read under another name in kit/ or plugins/ (read them as <area>.<name>): %s" % hidden)
        return 1
    d = dead(root, js)
    if d:
        print("words no client reads (delete them, or read them as <area>::<name> / <area>.<name>): %s" % d)
        return 1
    print("words_read_check: ok (%d words, each read by a client)" % len(keys(root)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
