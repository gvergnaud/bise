"""scripts/hub_loop_held.py on a made-up hub.log: the share of up time the
loop was held and the blocks over 5 s outside a start.

  python3 tests/hub_loop_held_test.py
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "scripts"))
import hub_loop_held as h  # noqa: E402

LOG = """\
900 hub loop held 9.0 s on before any start (shell 9.0 s)
1000 hub start pid=1 workspace=/w
3000 hub loop held 2.0 s on input Tick: idle check 2.0 s (shell 2.0 s)
9000 hub loop busy 5 s on sb list from main (shell 5.0 s)
16000 hub loop free after 7 s on sb list from main (shell 7.0 s)
20000 boot: boot done (REPLs spawned)
30000 hub loop busy 5 s on state push (shell 5.0 s)
56000 hub loop busy 30 s on state push (shell 30.0 s)
60000 hub loop free after 34 s on state push (shell 34.0 s)
101000 a last line of the first run
200000 hub start pid=2 workspace=/w
201000 boot: boot done
230000 hub loop busy 5 s on repl line t1 (shell 5.0 s)
240000 hub loop busy 15 s on repl line t1 (shell 15.0 s)
"""


def test_up_time_holds_and_blocks():
    rs = h.runs(LOG.splitlines())
    assert [r["start"] for r in rs] == [1000, 200000], rs
    # run 1: 2 + 7 + 34 s held; run 2 ends busy: its last busy line, 15 s
    assert [len(r["holds"]) for r in rs] == [3, 1], rs
    s = h.summary(rs)
    assert s["up_h"] == round((100_000 + 40_000) / 3_600_000, 2), s
    assert s["held_h"] == round(58_000 / 3_600_000, 2), s
    assert s["held_pct"] == round(100 * 58_000 / 140_000, 2), s
    # the 7 s before boot done is a start's; 34 s and 15 s are blocks
    assert (s["blocks_over_5s"], s["in_starts_over_5s"], s["worst_ms"]) == (2, 1, 34_000), s
    assert s["worst"][0]["on"] == "state push", s


def test_the_window_cuts_up_time_and_holds():
    rs = h.runs(LOG.splitlines())
    s = h.summary(rs, since_ms=h.since("1m", 260_000))
    # from 200000: run 2 only, 40 s up, 15 s held
    assert (s["up_h"], s["held_pct"], s["blocks_over_5s"]) == (round(40_000 / 3_600_000, 2), 37.5, 1), s


if __name__ == "__main__":
    test_up_time_holds_and_blocks()
    test_the_window_cuts_up_time_and_holds()
    print("hub_loop_held_test: ok")
