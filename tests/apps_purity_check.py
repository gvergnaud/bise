#!/usr/bin/env python3
"""No type-specific view code outside plugins/ (docs/plugin-apps.md A6b, architect m_60354).

A file type opens through its app (a plugin's view, picked by bend_plugins::opens);
the desktop host, the kit outside plugins/, the pages server and the TUI never know
one type from another. A text scan flags, it never guesses: every hit fails with its
file:line, and tests/apps_purity.txt is the only way past.

Two rules, on the code lines of git-tracked .ts/.tsx/.mts/.js/.mjs/.rs files (a comment
line is no hit; a generated file follows its source; plugins/ and vendored code are out):

- renderer: a document renderer outside plugins/: an import or a URL of univer,
  docx-preview, pdf.js, office/dist's view bundles or kit/office/*, and every file of
  kit/office/ and office/sdk/src/view/ (a renderer's own code), anywhere in the repo.
- switch: a branch on a file's type or kind in the host (SWITCH_ROOTS, tests left out):
  a compare or a `case` / match arm on a type word (`ext === 'pdf'`, `"xlsx" =>`), an
  ending test (`endsWith('.md')`), a list of two endings or more (`['png', 'jpg']`), an
  ending regex (`\\.(png|jpe?g)$`), an ending as a map key (`'.png': ...`), a `*_EXTS`
  table.

tests/apps_purity.txt: `<path> | <n> | <step> | <why>`, one per file, n its hits (exact:
more is refused, fewer too: lower n, or delete the line when the step lands). The step
is the change that removes the file's hits: a row of docs/plugin-apps.md's migration order
(`step 1`..`step 6`; a file the steps share names its last), `thumbs` (A6b's thumbnails,
icons and previews, apps-thumbs) or `gallery` (cards and filter by app, A6b, apps-gallery);
those only shrink, and the list is deleted with BUILTIN when the last type moves. `keep`:
a hit that is no file type (a log body kind, a code fence's tag), exact like the others,
its why saying what the word is there; a new keep line needs architect's sign, and every
run prints `N kept`. The check outlives the migration: at step 6 BUILTIN and the step lines
go, the check and its keep lines stay (architect m_60429).

    python3 tests/apps_purity_check.py              check the repo (gate.sh quick)
    python3 tests/apps_purity_check.py --list       every hit, file:line rule text
    python3 tests/apps_purity_check.py --self-test  the rules on made-up lines
"""
from __future__ import annotations

import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ALLOW = ROOT / "tests" / "apps_purity.txt"
CODE = (".ts", ".tsx", ".mts", ".js", ".mjs", ".rs")
OUT = ("plugins/", "bend/vendor/", "rust/vendor/", "node_modules/")
# the host: the window (main and renderer), the kit outside plugins/, the hub and its
# pages server, the TUI
SWITCH_ROOTS = ("apps/desktop/src/", "kit/", "rust/switchboard/src/", "rust/tui/src/")
# a renderer's own code, wherever it is imported from
RENDERER_HOMES = ("kit/office/", "office/sdk/src/view/")
STEPS = ("step 1", "step 2", "step 3", "step 4", "step 5", "step 6", "thumbs", "gallery", "keep")

# (`~kit/office/v<n>/`: bise's DOM-free office library views may import, never a renderer;
# architect m_60401)
RENDERER = re.compile(
    r"""['"`][^'"`]*?(@univerjs/|docx-preview|pdfjs-dist|(?<!~)kit/office/(?!v\d+/)|office-(?:view|pdf|sheet|md)\.js"""
    r"""|office/sdk/src/view)"""
)
# the endings of the types an app opens (docs/plugin-apps.md's fixtures/files and kin)
EXT = (r"png|jpe?g|gif|webp|heic|heif|avif|svg|bmp|tiff?|psd|pdf|md|markdown|txt|rtf|"
       r"mp4|mov|webm|m4v|mkv|avi|mp3|m4a|wav|3gp|aac|docx?|pptx?|xlsx?|csv|tsv|odt|ods|odp|html?")
# a type as a kind word (viewer_kind's, iconKindOf's, the gallery's)
# (never `image`, `office`, `doc` or `shot` alone: a pasted image, the bims' office, a
# ProseMirror doc, fn-talk's still)
KIND = r"shots|markdown|video|sheet|deck|slides|site"
WORD = rf"(?:\.?(?:{EXT})|{KIND})"
Q = r"""['"]"""
SWITCH = [
    ("compare", re.compile(rf"(?:[=!]==?)\s*{Q}{WORD}{Q}|{Q}{WORD}{Q}\s*(?:[=!]==?)")),
    ("case", re.compile(rf"\bcase\s+{Q}{WORD}{Q}\s*:")),
    # a Rust match arm (in TS, `'a' | 'b'` is a type, the compare or case on it is the hit)
    ("arm", re.compile(rf"{Q}{WORD}{Q}\s*(?:=>|\|\s*{Q})")),
    ("ending", re.compile(rf"(?:endsWith|ends_with)\(\s*{Q}\.(?:{EXT}){Q}", re.I)),
    ("list", re.compile(rf"{Q}\.?(?:{EXT}){Q}\s*,\s*{Q}\.?(?:{EXT}){Q}")),
    ("regex", re.compile(rf"\\\.\(?(?:\?:)?(?:{EXT})(?:\||\)|\$)")),
    ("key", re.compile(rf"{Q}\.(?:{EXT}){Q}\s*:")),
    ("table", re.compile(r"\b[A-Z][A-Z_]*_EXTS\b")),
]
# a doc comment's or a block comment's inner line
COMMENT_LINE = re.compile(r"^\s*(?://|\*(?!/)|#!)")
# a line that names a MIME type: an HTTP server's content-type by ending, the one place a
# server must know endings (it tells the browser, it never picks a viewer)
MIME = re.compile(r"""['"](?:text|image|video|audio|application|font)/[\w.+-]+""")


def is_test(path: str) -> bool:
    name = path.rsplit("/", 1)[-1]
    return (".test." in name or ".check." in name or name.endswith(("_tests.rs", "_test.rs", "tests.rs"))
            or "/tests/" in path or "_tests/" in path or "/test/" in path or "/__tests__/" in path)


def generated(text: str) -> bool:
    """A banner in the first 3 lines says the file is built from another (tests/js_guard.py's)."""
    first = "\n".join(text.splitlines()[:3]).lower()
    return "generated from" in first or "generated by" in first


def strip_comments(line: str, in_block: bool) -> tuple[str, bool]:
    """The line's code outside /* */ comments (one may span lines: `in_block` in and out) and
    after a `//` that starts a comment; quotes kept, so a glob's `**/*.ts` is no comment."""
    if not in_block and "/*" not in line and "//" not in line:
        return line, False
    out, i, quote = [], 0, ""
    while i < len(line):
        if in_block:
            end = line.find("*/", i)
            if end < 0:
                return "".join(out), True
            in_block, i = False, end + 2
            continue
        c = line[i]
        if quote:
            out.append(c)
            if c == "\\":
                out.append(line[i + 1:i + 2])
                i += 1
            elif c == quote:
                quote = ""
        elif c in "'\"`":
            quote = c
            out.append(c)
        elif line.startswith("/*", i):
            in_block, i = True, i + 2
            continue
        elif line.startswith("//", i) and (i == 0 or line[i - 1] in " \t;,{}()"):
            break
        else:
            out.append(c)
        i += 1
    return "".join(out), in_block


def code_lines(path: str, text: str):
    """(line number, code) of the file: comments out; a Rust file stops at its `#[cfg(test)]`."""
    in_block = False
    for n, line in enumerate(text.splitlines(), 1):
        if path.endswith(".rs") and line.strip() == "#[cfg(test)]":
            return
        if not in_block and COMMENT_LINE.match(line):
            continue
        code, in_block = strip_comments(line, in_block)
        if code.strip():
            yield n, code


def hits(path: str, text: str) -> list[tuple[int, str, str]]:
    """The file's hits: (line, rule, the line's text)."""
    if not path.endswith(CODE) or path.startswith(OUT) or "/node_modules/" in path or generated(text):
        return []
    out: list[tuple[int, str, str]] = []
    if path.startswith(RENDERER_HOMES):
        out.append((1, "renderer", "a renderer's own file outside plugins/"))
    switch = path.startswith(SWITCH_ROOTS) and not is_test(path)
    if not switch and not RENDERER.search(text):
        return out
    for n, line in code_lines(path, text):
        if RENDERER.search(line):
            out.append((n, "renderer", line.strip()))
        if switch:
            for rule, rx in SWITCH:
                if rule == "arm" and not path.endswith(".rs"):
                    continue
                if rx.search(line) and not MIME.search(line):
                    out.append((n, f"switch {rule}", line.strip()))
                    break
    return out


def allowlist(text: str) -> dict[str, tuple[int, str]]:
    """path -> (n, step); a bad line raises with its number."""
    out: dict[str, tuple[int, str]] = {}
    for i, line in enumerate(text.splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        parts = [p.strip() for p in line.split("|")]
        if len(parts) != 4 or not parts[1].isdigit() or parts[2] not in STEPS or not parts[3]:
            raise ValueError(f"tests/apps_purity.txt:{i}: want `<path> | <n> | <{'|'.join(STEPS)}> | <why>`: {line}")
        if parts[0] in out:
            raise ValueError(f"tests/apps_purity.txt:{i}: {parts[0]} twice")
        out[parts[0]] = (int(parts[1]), parts[2])
    return out


def judge(found: dict[str, list[tuple[int, str, str]]], allow: dict[str, tuple[int, str]]) -> list[str]:
    """What fails: a hit in a file off the list or past its n, a listed n above its hits."""
    bad = []
    for path, hs in sorted(found.items()):
        n = allow.get(path, (0, ""))[0]
        if len(hs) > n:
            for line, rule, txt in hs:
                bad.append(f"{path}:{line}: {rule}: {txt[:120]}")
            bad.append(f"  -> {path}: {len(hs)} hits, the list allows {n}: open the type through its app "
                       "(docs/plugin-apps.md A2-A4), never a branch on it here")
    for path, (n, step) in sorted(allow.items()):
        have = len(found.get(path, []))
        if have < n:
            bad.append(f"{path}: {have} hits, the list says {n} ({step}): lower its n in tests/apps_purity.txt, "
                       "or delete its line when it reaches 0")
    return bad


def self_test() -> None:
    def h(path, text):
        return [r for _, r, _ in hits(path, text)]

    # renderers: imports and URLs, anywhere outside plugins/; a renderer's home is a hit by itself
    assert h("apps/desktop/src/a.ts", 'import { Univer } from "@univerjs/core";') == ["renderer"]
    assert h("office/sdk/src/x.ts", 'export { renderAsync } from "docx-preview";') == ["renderer"]
    assert h("rust/switchboard/src/v.rs", '<script src=\\"/kit/office/viewer.js\\"></script>') == ["renderer"]
    assert h("scripts/a.ts", 'const ui = await import("/kit/office/office-sheet.js");') == ["renderer"]
    assert h("kit/office/frame.ts", "export const FRAME = 1;") == ["renderer"]
    assert h("plugins/sheets/view/a.ts", 'import { Univer } from "@univerjs/core";') == []
    assert h("apps/desktop/src/a.ts", "// drawn by kit/office/image.js") == []
    assert h("apps/desktop/src/a.tsx", "   the viewer itself (kit/office/image.js), its esc") == []
    assert h("apps/desktop/src/a.tsx", "{/* the player\n  goes full screen ('kit/office/video.js'): allow it */}\nok();") == []
    assert h("apps/desktop/src/a.ts", "const g = 'src/**/*.ts'; if (ext === 'pdf') x(); /* 'xlsx' */") == ["switch compare"]
    assert h("apps/desktop/src/a.ts", "x(); // if (ext === 'pdf')") == []
    assert h("apps/desktop/src/a.ts", "const u = 'http://a/b'; if (e === 'md') x();") == ["switch compare"]
    assert h("rust/switchboard/src/pages/f.rs", 'const KIT: &[&str] = &["~kit/view/", "~kit/office/v1/"];') == []
    assert h("kit/office/image.js", "// generated from image.ts by scripts/ts_emit.ts\nimport '/kit/office/zoom.js'") == []
    # switches: in the host, not in its tests
    assert h("apps/desktop/src/a.ts", "if (ext === 'pdf') return 1;") == ["switch compare"]
    assert h("apps/desktop/src/a.ts", "if ('.md' !== e) return 1;") == ["switch compare"]
    assert h("apps/desktop/src/a.ts", "  case 'xlsx':") == ["switch case"]
    assert h("apps/desktop/src/a.ts", "if (k === 'shots') return 1;") == ["switch compare"]
    assert h("apps/desktop/src/a.ts", "if (c.kind === 'image') x(); if (cam === 'office') y();") == []
    assert h("rust/switchboard/src/pages/s.rs", '        "docx" => Some("docx"),') == ["switch arm"]
    assert h("rust/tui/src/o.rs", '        "png" | "jpg" => 1,') == ["switch arm"]
    assert h("kit/a.ts", "if (name.endsWith('.md')) x();") == ["switch ending"]
    assert h("apps/desktop/src/a.ts", "const IMAGE = new Set(['png', 'jpg', 'gif']);") == ["switch list"]
    assert h("apps/desktop/src/a.ts", r"const RE = /\.(png|jpe?g)$/i;") == ["switch regex"]
    assert h("apps/desktop/src/a.ts", "  '.png': 1,") == ["switch key"]
    assert h("apps/desktop/src/a.ts", "  '.png': 'image/png',") == []
    assert h("rust/switchboard/src/a.rs", '"jpg" | "jpeg" => "image/jpeg",') == []
    assert h("apps/desktop/src/a.ts", "type K = 'sheet' | 'float';") == []
    assert h("rust/switchboard/src/a.rs", "_ if VIDEO_EXTS.contains(&e) => 1,") == ["switch table"]
    assert h("apps/desktop/src/a.test.ts", "if (ext === 'pdf') return 1;") == []
    assert h("rust/switchboard/src/a.rs", "fn a() {}\n#[cfg(test)]\nmod t { const A: &str = \"pdf\" => 1; }") == []
    assert h("rust/plugins/src/opens.rs", '"xlsx" => 1,') == []
    assert h("apps/desktop/src/a.ts", "if (k === 'pages') x(); const n = 'pdf';") == []
    assert h("apps/desktop/src/a.ts", "if (c.type === 'text') x();") == []
    assert h("rust/switchboard/src/a.rs", "// \"pdf\" => Some(\"pdf\")") == []
    # the list: exact counts, steps named, both ways
    allow = allowlist("# a comment\nk/a.ts | 2 | step 1 | image and markdown\nk/b.ts | 1 | thumbs | LOOKS\n")
    assert allow == {"k/a.ts": (2, "step 1"), "k/b.ts": (1, "thumbs")}, allow
    for bad_line in ("k/a.ts | 2 | soon | why", "k/a.ts | x | step 1 | why", "k/a.ts | 1 | step 1 |", "k/a.ts | 1"):
        try:
            allowlist(bad_line)
            raise AssertionError(bad_line)
        except ValueError:
            pass
    one = [(3, "switch compare", "x")]
    assert judge({"k/a.ts": one * 2, "k/b.ts": one}, allow) == []
    assert any("3 hits, the list allows 2" in b for b in judge({"k/a.ts": one * 3, "k/b.ts": one}, allow))
    assert any("lower its n" in b for b in judge({"k/a.ts": one, "k/b.ts": one}, allow))
    assert any("allows 0" in b for b in judge({"k/a.ts": one * 2, "k/b.ts": one, "k/c.ts": one}, allow))
    print("apps_purity_check self-test: ok")


def scan() -> dict[str, list[tuple[int, str, str]]]:
    listed = subprocess.run(["git", "ls-files", "-z", "--", *(f"*{e}" for e in CODE)], cwd=ROOT,
                            check=True, capture_output=True).stdout.decode().split("\0")
    found = {}
    for path in filter(None, listed):
        p = ROOT / path
        if path.startswith(OUT) or not p.is_file():
            continue
        hs = hits(path, p.read_text("utf-8", "replace"))
        if hs:
            found[path] = hs
    return found


def main() -> int:
    if "--self-test" in sys.argv:
        self_test()
        return 0
    found = scan()
    if "--list" in sys.argv:
        for path, hs in sorted(found.items()):
            for line, rule, txt in hs:
                print(f"{path}:{line}: {rule}: {txt[:140]}")
        print(f"{sum(map(len, found.values()))} hits in {len(found)} files; by rule:",
              dict(Counter(r for hs in found.values() for _, r, _ in hs)))
        return 0
    try:
        allow = allowlist(ALLOW.read_text())
    except ValueError as e:
        print(f"apps_purity_check: {e}")
        return 1
    bad = judge(found, allow)
    if bad:
        print("apps_purity_check: type-specific view code outside plugins/ (docs/plugin-apps.md A6b):")
        print("\n".join(bad))
        return 1
    kept = sum(n for n, step in allow.values() if step == "keep")
    print(f"apps_purity_check: ok ({sum(map(len, found.values()))} listed hits in {len(found)} files, "
          f"{kept} kept: no file type)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
