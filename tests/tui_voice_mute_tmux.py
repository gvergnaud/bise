"""Voice mode muted (voice-mute) in a real terminal (tmux), with the fakes
of tui_voice_tmux.py (no mic, no sound, no network): your turn is sent,
main answers and speaks; m mutes. Only your side greys (the header's
`○`, the status row's ` · ○ muted`, your lane under 30 rows, the keys'
`m unmute`); main's face keeps speaking in its colors, then smiles at
rest with `○ muted` alone; m again and you listen.

python3 -u tests/tui_voice_mute_tmux.py   (VOICE_MUTE_CAPTURES=<dir>: the
screens with their colors, for the designer)
"""
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, wait_until, MAIN_IDLE  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
WAV = os.path.join(HERE, "..", "rust", "tui", "src", "voicemode", "testdata", "sentence.wav")
# long enough that main's answer (`ack: …`) is said for several seconds
HEARD = ("voice-marker check the build and tell me which tests failed, why they failed, "
         "and what you would change first so that the whole suite is green again tonight")
KISS_COLS = 50  # the bar, the kiss (4 + 41 cells) and a margin, before the captions
SGR = re.compile(r"\x1b\[([0-9;]*)m")


def save(name, t):
    d = os.environ.get("VOICE_MUTE_CAPTURES")
    if d:
        os.makedirs(d, exist_ok=True)
        open(os.path.join(d, name + ".ans"), "w").write(t.screen(colors=True))
        open(os.path.join(d, name + ".txt"), "w").write(t.screen())


def face_colors(t):
    """The foreground colors of the drawn cells left of the captions, on
    the rows under the voice mode divider (the kiss)."""
    colors = set()
    lines = t.screen(colors=True).splitlines()
    plain = t.screen().splitlines()
    top = next(i for i, r in enumerate(plain) if "· voice mode" in r)
    for line in lines[top + 1:]:
        fg, col, i = "", 0, 0
        while i < len(line) and col < KISS_COLS:
            m = SGR.match(line, i)
            if m:
                codes = m.group(1)
                if codes in ("", "0") or codes.startswith("39"):
                    fg = ""
                for c in re.findall(r"38;[0-9;]+", codes):
                    fg = c
                i = m.end()
                continue
            ch = line[i]
            if col > 2 and not ch.isspace() and ch != "│":
                colors.add(fg)
            col += 1
            i += 1
    return colors


def speaking_then_muted(t, tag):
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    t.keys("C-r", "C-r")
    t.wait("· voice mode ·")
    # main answers: its voice starts
    t.wait("ack: voice-marker check the build", timeout=30)
    sc = t.wait_re(r"\bspeaking\b", timeout=10)
    save(tag + "-speaking-unmuted", t)
    unmuted = face_colors(t)
    t.keys("m")
    sc = t.wait("○ muted", timeout=5)
    return sc, unmuted


def main():
    env = "BISE_VOICE_FAKE=%s BISE_VOICE_FAKE_HEARD='%s'" % (os.path.abspath(WAV), HEARD)
    # the big pane (40 rows): the kiss
    with tui_session(120, 40, env) as t:
        sc, unmuted = speaking_then_muted(t, "big")
        assert "speaking · ○ muted" in sc, sc
        assert "○ voice mode" in sc and "● voice mode" not in sc, sc
        assert "m unmute" in sc, sc
        save("big-speaking-muted", t)
        muted = face_colors(t)
        assert muted and muted <= unmuted | {""}, "the face keeps its colors muted: %r vs %r" % (muted, unmuted)
        print("---- muted while main speaks ----\n" + sc)
        # its voice ends: the smile at rest, `○ muted` alone
        sc = t.wait_re(r"(?<!· )○ muted", timeout=40)
        # after the end-of-turn kiss: the face back in its resting colors
        wait_until(lambda: "speaking" not in t.screen() and face_colors(t) <= unmuted | {""}, 10,
                   lambda: "the smile at rest: %r vs %r" % (face_colors(t), unmuted))
        sc = t.screen()
        assert "speaking" not in sc, sc
        save("big-rest-muted", t)
        assert face_colors(t) <= unmuted | {""}, "the smile at rest in its colors"
        print("---- muted, main at rest ----\n" + sc)
        # m: you listen again
        t.keys("m")
        sc = t.wait("● listening", timeout=5)
        assert "○ muted" not in sc and "● voice mode" in sc, sc
        save("big-rest-unmuted", t)
        print("---- unmuted ----\n" + sc)
    # under 30 rows: the lanes, yours greys, main's speaks
    with tui_session(120, 29, env) as t:
        sc, _ = speaking_then_muted(t, "lanes")
        rows = sc.splitlines()
        you = next(r for r in rows if "○ muted" in r)
        agent = rows[rows.index(you) + 1]
        assert re.search(r"\byou\b", you), rows
        assert "speaking" in agent and "muted" not in agent, rows
        save("lanes-speaking-muted", t)
        print("---- lanes: muted while main speaks ----\n" + sc)


if __name__ == "__main__":
    run(main)
