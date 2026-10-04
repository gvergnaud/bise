"""BISE-276: the composer formats markdown as you type, every mark kept,
in a real terminal (tmux, legacy keys) against the fake provider. Ctrl+J
continues a list (`- ` then `- `), Tab indents the item, Ctrl+J on an
empty nested item steps out, on an empty item ends the list; in a ```ts
block plain ⏎ is a newline and the code is colored (the keyword and the
number in their syntax colors, the fence and the list markers dim);
after the closing fence ⏎ sends, and the message is exactly the text
typed (the fake provider's log).

python3 -u tests/tui_markdown_tmux.py
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, pane_rows, wait_until, MAIN_IDLE  # noqa: E402

# theme.rs, dark and light: (dim, syntax_keyword, syntax_number, text)
DIM = {(0xa3, 0x9c, 0x90), (0x6b, 0x64, 0x5a)}
KEYWORD = {(0xd7, 0xa6, 0xf0), (0x8a, 0x3f, 0xb0)}
NUMBER = {(0xf0, 0xb2, 0x7a), (0x9a, 0x4a, 0x0c)}
TEXT = {(0xec, 0xe6, 0xda), (0x1b, 0x19, 0x17)}

SGR = re.compile(r"\x1b\[([0-9;:]*)m")


def cells(row):
    """The visible chars of a captured row (capture-pane -e) with the
    foreground (r, g, b) each was drawn in (None: the default)."""
    out, fg, i = [], None, 0
    while i < len(row):
        m = SGR.match(row, i)
        if m:
            ps = [int(p) if p else 0 for p in re.split("[;:]", m.group(1) or "0")]
            k = 0
            while k < len(ps):
                if ps[k] == 0 or ps[k] == 39:
                    fg = None
                elif ps[k] == 38 and k + 4 < len(ps) + 0 and ps[k + 1] == 2:
                    fg = tuple(ps[k + 2:k + 5])
                    k += 4
                elif ps[k] in (38, 48) and k + 2 < len(ps) and ps[k + 1] == 5:
                    k += 2
                elif ps[k] == 48 and k + 1 < len(ps) and ps[k + 1] == 2:
                    k += 4
                k += 1
            i = m.end()
            continue
        if row[i] == "\x1b":  # another escape (OSC): skip to its end
            j = row.find("\x1b\\", i)
            i = j + 2 if j >= 0 else len(row)
            continue
        out.append((row[i], fg))
        i += 1
    return out


def fg_of(sc, text, at=0):
    """The foreground of `text`'s char `at` (its first) on the colored
    screen."""
    for row in sc.splitlines():
        cs = cells(row)
        plain = "".join(c for c, _ in cs)
        k = plain.find(text)
        if k >= 0:
            return cs[k + at][1]
    raise AssertionError("not on screen: %r" % text)


def composer(t):
    """The composer's rows, stripped (the blank bar rows around left out;
    the indentation shows in the message sent, checked at the end)."""
    return "\n".join(pane_rows(t.screen().rstrip("\n").splitlines())).strip("\n")


def wait_composer(t, text, timeout=5):
    text = "\n".join(x.strip() for x in text.split("\n"))
    wait_until(lambda: composer(t) == text, timeout,
               lambda: "composer %r, expected %r" % (composer(t), text), poll=0.1)


def typed(t, text):
    """Literal text, a leading `-` included (tmux would read it as a flag)."""
    t.keys("-l", "--", text)


def main():
    E = e2e.Env()
    with tui_session(120, 40, E=E) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # a list: ctrl+j continues it, tab nests, an empty item steps out
        # then ends the list
        typed(t, "- one")
        t.keys("C-j")
        wait_composer(t, "- one\n-")
        t.typed("two")
        t.keys("C-j")
        t.keys("Tab")
        wait_composer(t, "- one\n- two\n  -")
        t.typed("sub")
        t.keys("C-j")
        t.keys("C-j")
        wait_composer(t, "- one\n- two\n  - sub\n-")
        t.keys("C-j")
        wait_composer(t, "- one\n- two\n  - sub")
        # a ts block: plain Enter is a newline inside it
        t.typed("```ts")
        t.keys("Enter")
        t.typed("const x = 42")
        t.keys("Enter")
        t.typed("```")
        wait_composer(t, "- one\n- two\n  - sub\n```ts\nconst x = 42\n```")
        # the highlighter's colors on the drawn frame
        sc = t.wait_any([lambda s: fg_of(s, "const") in KEYWORD], 10, colors=True)[1]
        assert fg_of(sc, "const") in KEYWORD, fg_of(sc, "const")
        # the code's 42, not the divider's context (BISE-303: `42k · 4%`)
        assert fg_of(sc, "x = 42", 4) in NUMBER, fg_of(sc, "x = 42", 4)
        assert fg_of(sc, "```ts") in DIM, fg_of(sc, "```ts")
        assert fg_of(sc, "- one") in DIM, fg_of(sc, "- one")
        assert fg_of(sc, "one") in TEXT, fg_of(sc, "one")
        # after the closing fence Enter sends, the text exactly as typed
        t.keys("Enter")
        t.wait("ack: - one - two - sub", 60)
        want = "- one\n- two\n  - sub\n```ts\nconst x = 42\n```"

        def sent():
            if not os.path.exists(E.fake_log):
                return False
            for line in open(E.fake_log):
                r = json.loads(line)
                if want in [r.get("user")] + r.get("users", []):
                    return True
            return False
        wait_until(sent, 10, lambda: "the message %r in %s" % (want, open(E.fake_log).read()[-2000:]))
        print("PASS tui markdown")


if __name__ == "__main__":
    run(main)
