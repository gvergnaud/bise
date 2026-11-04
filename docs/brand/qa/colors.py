"""Every foreground / background color used in a pass's captures, with
the book §5 role it matches (or '??'), and the shots it appears in.

python3 colors.py shots/dark dark
"""
import collections
import glob
import os
import re
import sys

ROLES = {
    "dark": {"text": "ece6da", "dim": "a39c90", "faint": "857d72", "rule": "4a4540", "accent": "f4a6b0", "error": "ff5a52", "ok": "b9d99a",
             "kw": "d7a6f0", "str": "b9d99a", "comment": "857e74", "num": "f0b27a", "call": "8fc4f0", "type": "e8cf9a",
             "on_accent": "1b1917"},
    "light": {"text": "1b1917", "dim": "6b645a", "faint": "7d766c", "rule": "cfc8bd", "accent": "b8416b", "error": "b3261e", "ok": "3f7a2a",
              "kw": "8a3fb0", "str": "417228", "comment": "6c665b", "num": "9a4a0c", "call": "1f63a8", "type": "7a5c00",
              "on_accent": "ffffff"},
}
d, mode = sys.argv[1], sys.argv[2]
names = {}
for r, h in ROLES[mode].items():
    names.setdefault(h, []).append(r)
seen = collections.defaultdict(set)
for f in sorted(glob.glob(os.path.join(d, "*.ansi"))):
    for m in re.finditer(r"\x1b\[([0-9;:]*)m", open(f).read()):
        ps = m.group(1).split(";")
        for i, p in enumerate(ps):
            if p in ("38", "48") and i + 4 < len(ps) + 1 and ps[i + 1:i + 2] == ["2"]:
                h = "%02x%02x%02x" % tuple(int(x) for x in ps[i + 2:i + 5])
                seen[("fg" if p == "38" else "bg", h)].add(os.path.basename(f)[:2])
            elif p in ("38", "48") and ps[i + 1:i + 2] == ["5"]:
                seen[("fg" if p == "38" else "bg", "idx" + ps[i + 2])].add(os.path.basename(f)[:2])
for (k, h), fs in sorted(seen.items()):
    print("%s #%s %-18s in %s" % (k, h, "/".join(names.get(h, ["??"])), " ".join(sorted(fs))[:80]))
