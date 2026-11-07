#!/usr/bin/env python3
"""The crates that read a changed file at compile time from outside their folder (review-gates
m_37101): include_str!/include_bytes!/include! of a path, plain or concat!(env!("CARGO_MANIFEST_DIR"),
"..."), and Path::new(env!("CARGO_MANIFEST_DIR")).join("...") (a folder a test reads). Read from the
source each run, never a list by hand, so a new include is covered. tests/gate_plan.py (the gate's plan) runs
this on the changed paths and maps each crate as a change in its folder (its tests and clippy).

  python3 tests/include_map.py <changed path>...   one line per crate hit: rust/<its folder>/(include)
  python3 tests/include_map.py --self-test
"""
import os
import re
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LIT = r'"((?:[^"\\]|\\.)*)"'
MDIR = r'env!\s*\(\s*"CARGO_MANIFEST_DIR"\s*\)'
INCLUDE = re.compile(r'include(?:_str|_bytes)?!\s*\(\s*(concat!\s*\(\s*' + MDIR + r'\s*,\s*)?' + LIT)
JOIN = re.compile(MDIR + r'\s*\)\s*\.join\s*\(\s*' + LIT)


def crate_dir(path, root):
    """The folder of the nearest Cargo.toml with a [package] above `path` (None: none)."""
    d = os.path.dirname(path)
    while d.startswith(root) and d != root:
        t = os.path.join(d, "Cargo.toml")
        if os.path.exists(t) and "[package]" in open(t, errors="replace").read():
            return d
        d = os.path.dirname(d)
    return None


def targets(root=ROOT):
    """[(target path relative to root, crate folder relative to root)] for every include in rust/."""
    out = []
    base = os.path.join(root, "rust")
    for d, dirs, files in os.walk(base):
        dirs[:] = [x for x in dirs if x != "target" and not x.startswith(".")]
        for f in files:
            if not f.endswith(".rs"):
                continue
            p = os.path.join(d, f)
            src = open(p, errors="replace").read()
            if "include" not in src and "CARGO_MANIFEST_DIR" not in src:
                continue
            c = crate_dir(p, root)
            if not c:
                continue
            found = [(m.group(2), bool(m.group(1))) for m in INCLUDE.finditer(src)]
            found += [(m.group(1), True) for m in JOIN.finditer(src)]
            for lit, from_crate in found:
                t = os.path.normpath(os.path.join(c if from_crate else d, lit.lstrip("/") if from_crate else lit))
                if t.startswith(root + os.sep):
                    out.append((os.path.relpath(t, root), os.path.relpath(c, root)))
    return out


def crates_for(changed, root=ROOT):
    """The crate folders (relative) reading one of `changed` (a file, or a file in a folder read)."""
    ts = targets(root)
    hit = []
    for p in changed:
        p = os.path.normpath(p.strip())
        for t, c in ts:
            if (p == t or p.startswith(t + os.sep)) and c not in hit:
                hit.append(c)
    return hit


def self_test():
    fails = []
    with tempfile.TemporaryDirectory() as t:
        t = os.path.realpath(t)
        os.makedirs(os.path.join(t, "rust", "a", "src", "m"))
        os.makedirs(os.path.join(t, "prompts"))
        open(os.path.join(t, "rust", "a", "Cargo.toml"), "w").write('[package]\nname = "a"\n')
        open(os.path.join(t, "rust", "a", "src", "m", "x.rs"), "w").write(
            'const P: &str = include_str!("../../../../prompts/p.txt");\n'
            'const B: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../kit/b.bin"));\n'
            'fn d() { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../kit/examples"); }\n')
        for p, want in (("prompts/p.txt", ["rust/a"]), ("kit/b.bin", ["rust/a"]), ("kit/examples/w.html", ["rust/a"]),
                        ("prompts/other.txt", []), ("kit/examples", ["rust/a"])):
            got = crates_for([p], t)
            if got != want:
                fails.append("synthetic %s: %r, want %r" % (p, got, want))
    # this repo, as review-gates named them: each reaches its crate
    for p, want in (("prompts/prompt-bise.txt", "rust/switchboard"), ("bend/laws/providers.bend", "rust/catalog"),
                    ("kit/examples/weekly-update.html", "rust/switchboard"),
                    ("prompts/skills-all/bise-pages/SKILL.md", "rust/switchboard")):
        if os.path.exists(os.path.join(ROOT, p)) and want not in crates_for([p]):
            fails.append("repo %s: %r, want %s" % (p, crates_for([p]), want))
    for f in fails:
        print("FAIL", f)
    print("include_map: %s" % ("ok" if not fails else "%d failed" % len(fails)))
    return 1 if fails else 0


if __name__ == "__main__":
    if sys.argv[1:2] == ["--self-test"]:
        sys.exit(self_test())
    for c in crates_for(sys.argv[1:]):
        print(c + "/(include)")
