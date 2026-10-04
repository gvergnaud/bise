"""Voice mode in a real terminal (tmux) on a throwaway hub with the fake
provider, with no mic, no sound and no network (BISE_VOICE_FAKE: a
recorded sentence plays as the mic, the listener hears a scripted line,
the voice is a silent tone on a device-less speaker): ctrl+r twice opens
voice mode, the sentence then the silence end the turn, main answers,
its answer is in the thread, the pane keeps your last question above
it (voice-lastq), esc ends voice mode with its line.

python3 -u tests/tui_voice_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, pane_rows, MAIN_IDLE  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
WAV = os.path.join(HERE, "..", "rust", "tui", "src", "voicemode", "testdata", "sentence.wav")


def main():
    env = "BISE_VOICE_FAKE=%s BISE_VOICE_FAKE_HEARD='voice-marker check the build'" % os.path.abspath(WAV)
    with tui_session(120, 40, env) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # ctrl+r twice: voice mode (one ctrl+r alone stays dictation)
        t.keys("C-r", "C-r")
        sc = t.wait("· voice mode ·")
        print("---- voice mode open ----\n" + sc)
        # the recorded sentence (~3 s), then silence: the turn goes
        sc = t.wait("ack: voice-marker check the build", timeout=30)
        print("---- the turn was sent, main answered ----\n" + sc)
        # voice-lastq: the pane keeps your last question above the answer
        sc = t.wait_re(r"│ +\S.* {3}voice-marker check the build")
        rows = pane_rows(sc.splitlines())
        q = next(i for i, r in enumerate(rows) if r.endswith("voice-marker check the build"))
        assert rows[q - 1].endswith("you"), rows
        assert rows[q + 3].endswith(":* main"), rows
        # esc: voice mode ends, its line in the thread (once main's voice
        # ended: listening again)
        t.wait("● listening", 30)
        t.keys("Escape")
        sc = t.wait("voice mode ended")
        assert "1 thing said" in sc, sc
        print("---- voice mode ended ----\n" + sc)


if __name__ == "__main__":
    run(main)
