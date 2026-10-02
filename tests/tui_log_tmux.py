"""/log (BISE_DEV=1), read like a document (user: "ça manque d'espace entre
les messages", "c'est full width", "pas de shortcuts pour jump tout en
haut ou tout en bas", "la recherche ne permet pas de sauter d'un cas à
l'autre", "un code couleur pour les types de messages"). Two prompts to
main (fake provider: `ack: ...`), then /log: a blank row between entries;
each kind's label in its color (`› you` accent, `§ system` orange); the
key bar says `g top   G bottom`; `/ack` jumps live to the first match from
the cursor with `n of m`, n and N go round the ends (`back to the first`,
`back to the last`), esc in the field puts the cursor back; g shows the
first entry, G the last; at 200 columns nothing passes the measure (108);
NO_COLOR keeps the rail `▎`. Through the real binaries (tmux).

python3 -u tests/tui_log_tmux.py
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402

MEASURE = 108
# theme.rs: accent and syntax_number, light then dark
ACCENT = ("38;2;184;65;107", "38;2;244;166;176")
ORANGE = ("38;2;154;74;12", "38;2;240;178;122")
SHOTS = os.environ.get("LOG_SHOTS")


def send(t, text, ack):
    t.typed(text)
    t.keys("Enter")
    t.wait(ack, 60)
    t.wait_re(MAIN_IDLE, 60)


def shot(t, name):
    """Keep a capture for the designer (LOG_SHOTS=<dir>)."""
    if SHOTS:
        os.makedirs(SHOTS, exist_ok=True)
        with open(os.path.join(SHOTS, name + ".ansi"), "w") as f:
            f.write(t.screen(colors=True))
        with open(os.path.join(SHOTS, name + ".txt"), "w") as f:
            f.write(t.screen())


def open_log(t):
    t.typed("/log")
    t.keys("Enter")
    sc = t.wait("log of main")
    # the command popup may take the first ⏎
    if "full history" not in sc:
        t.keys("Enter")
        sc = t.wait("full history")
    return t.wait("esc close")


def colored(col, label, codes):
    """`label` is drawn in one of `codes` (an SGR fg)."""
    return any(re.search(r"\x1b\[[0-9;]*" + re.escape(c) + r"[0-9;]*m" + re.escape(label), col) for c in codes)


def main():
    theme = os.environ.get("BISE_THEME", "dark")
    with tui_session(150, 50, "BISE_DEV=1 BISE_THEME=" + theme) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        send(t, "first ping", "ack: first ping")
        send(t, "second ping", "ack: second ping")
        sc = open_log(t)
        shot(t, "history-150-" + theme)
        rows = sc.splitlines()
        # the key bar's words
        assert any(r.strip().startswith("/ search   f filter   g top   G bottom   tab what the model got") for r in rows), sc
        # a blank row above every header but the first
        heads = [y for y, r in enumerate(rows) if re.match(r"^\s*[›]?\s*#\d+\s", r)]
        assert len(heads) >= 4, sc
        for y in heads[1:]:
            assert rows[y - 1].strip() == "", (y, sc)
        # one color per kind
        col = t.screen(colors=True)
        assert colored(col, "› you", ACCENT), col[-4000:]
        # g: the top (the system prompt); G: the bottom (the last reply)
        t.keys("g")
        sc = t.wait("§ system")
        col = t.screen(colors=True)
        assert colored(col, "§ system", ORANGE), col[-4000:]
        t.keys("G")
        t.wait_re(r"›\s+#\d+.*:\* main\s+ack: second ping")
        # search: live to the first match from the cursor (round the end)
        t.keys("/")
        t.typed("ping")
        sc = t.wait_re(r"\d of \d")
        shot(t, "search-typing-" + theme)
        t.keys("Escape")
        # esc in the field: the cursor back on the last reply
        t.wait_re(r"›\s+#\d+.*:\* main\s+ack: second ping")
        t.keys("/")
        t.typed("ping")
        t.keys("Enter")
        sc = t.wait("n next   N previous   esc clear search")
        # the count, right of the key bar (the top bar says "request 1 of 1")
        m = re.search(r"(\d+) of (\d+)\s*$", [r for r in sc.splitlines() if "n next" in r][0])
        assert m, sc
        total = int(m.group(2))
        assert total >= 4, sc
        shot(t, "search-" + theme)
        # n to the last, then round to the first
        for _ in range(total - int(m.group(1))):
            t.keys("n")
        t.wait("%d of %d" % (total, total))
        t.keys("n")
        t.wait("back to the first · 1 of %d" % total)
        shot(t, "search-wrapped-" + theme)
        t.keys("N")
        t.wait("back to the last · %d of %d" % (total, total))
        # any key drops the note
        t.keys("N")
        sc = t.wait("%d of %d" % (total - 1, total))
        assert "back to" not in sc, sc
        t.keys("Escape")
        t.wait("g top   G bottom")
        t.keys("Escape")
        t.wait_gone("log of main")
    # wide: the entries stop at the measure
    with tui_session(200, 45, "BISE_DEV=1 BISE_THEME=" + theme) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        send(t, "a wide ping " + " ".join(["word"] * 60), "ack: a wide ping")
        open_log(t)
        t.keys("C-o")
        sc = t.wait("word word word")
        for r in sc.splitlines()[3:-2]:
            if "word" in r:
                # the screen's 1-column margin, then the measure
                assert len(r.rstrip()) <= 1 + MEASURE, (len(r.rstrip()), r)
    # 80 columns, and NO_COLOR: the rail stays, the labels tell
    with tui_session(80, 40, "BISE_DEV=1 NO_COLOR=1 BISE_THEME=" + theme) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        send(t, "first ping", "ack: first ping")
        sc = open_log(t)
        t.keys("C-o")
        sc = t.wait("  ▎ ")
        shot(t, "history-80-nocolor-" + theme)
        assert any(r.strip().startswith("/ search") and r.rstrip().endswith("esc close") for r in sc.splitlines()), sc
    print("PASS tui /log")


if __name__ == "__main__":
    run(main)
