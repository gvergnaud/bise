#!/usr/bin/env python3
"""Compaction keeps the summary, then only the user's newest messages
within a token budget (user: "il faut garder que les messages récents,
avec un nombre de tokens max en partant du plus récent").

Runs ./repl-scripted (its config keeps 40 estimated tokens of user
messages, runtime/main.bend cfg) through four compactions, each in a new
process resumed from the session file the last one saved (the checkpoint
path a restart takes):
  1. m1 m2 m3, /compact        keeps m2 m3
  2. m4 m5, /compact           keeps m4 m5 (m2 m3 gone)
  3. m6, /compact              keeps m5 m6
  4. /compact (nothing new)    keeps m5 m6, not more
After each one the saved context must be: ONE summary message first (an
earlier summary is never carried), then the kept user messages in
order, verbatim, within the budget (or the latest alone), the older
ones gone. The session log's compaction_done must carry the real
summary (what /log shows), with no separate summary event.

Run: python3 -u tests/compaction_e2e.py  (scripts/bins.sh repl-scripted)
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import session_ev as sev  # noqa: E402  (run_repl, ev_lines)

BUDGET = 40  # runtime/main.bend cfg: the scripted select budget
# the summary message's head: escaped in the session file, a real
# newline in the session log
SUMMARY = "Summary of the earlier conversation:\\n<summary>"
SUMMARY_EV = "Summary of the earlier conversation:\n<summary>"

# 60 chars each: 1 + 60/4 = 16 estimated tokens (core/estimate.bend),
# so a 40-token budget keeps two
MSGS = ["m%d: " % i + ("please handle request number %d now" % i).ljust(56, ".") for i in range(1, 7)]
assert all(len(m) == 60 for m in MSGS)

def fail(msg):
    sys.exit("FAIL compaction_e2e: " + msg)

def tokens(text):
    return 1 + len(text) // 4

def context(saved):
    """(injected, role, text) of each MSG line of a saved session."""
    out = []
    for line in saved.splitlines():
        if line.startswith("MSG "):
            head, text = line[4:].split(" : ", 1)
            inj, role = head.split(" ")
            out.append((inj == "True", role, text))
    return out

def check(n, saved, wire, want):
    ctx = context(saved)
    users = [t for inj, role, t in ctx if role == "user" and not inj]
    summaries = [t for inj, role, t in ctx if role == "user" and inj and t.startswith("Summary of the earlier")]
    if not ctx or not (ctx[0][0] and ctx[0][2].startswith(SUMMARY)):
        fail("compaction %d: the context does not open with the summary: %r" % (n, ctx[:1]))
    if len(summaries) != 1:
        fail("compaction %d: %d summaries in the context (they pile up)" % (n, len(summaries)))
    if users != want:
        fail("compaction %d: kept %r, want %r" % (n, users, want))
    if len(users) > 1 and sum(tokens(u) for u in users) > BUDGET:
        fail("compaction %d: kept %d tokens, budget %d" % (n, sum(tokens(u) for u in users), BUDGET))
    gone = [m for m in MSGS if m not in want and any(m in t for _, _, t in ctx)]
    if gone:
        fail("compaction %d: older messages still in the context: %r" % (n, gone))
    evs = sev.ev_lines(wire)
    done = [e for e in evs if e["type"] == "compaction_done"]
    if len(done) != 1 or not done[0]["data"]["summary"][0]["text"].startswith(SUMMARY_EV):
        fail("compaction %d: compaction_done does not carry the summary: %r" % (n, done))
    if [e for e in evs if e["type"] == "context_injected" and e["data"]["kind"] == "summary"]:
        fail("compaction %d: a separate summary event" % n)
    if len(done[0]["data"]["kept"]) != len(want):
        fail("compaction %d: compaction_done keeps %r, want %d" % (n, done[0]["data"]["kept"], len(want)))
    print("ok compaction %d: summary + %d kept (%d tokens <= %d)"
          % (n, len(users), sum(tokens(u) for u in users), BUDGET))

def main():
    if not os.path.exists(sev.REPL):
        fail("no ./repl-scripted: scripts/bins.sh repl-scripted")
    script = {m: [{"text": "done: " + m[:2]}] for m in MSGS}
    rounds = [
        (MSGS[0:3], [MSGS[1], MSGS[2]]),
        (MSGS[3:5], [MSGS[3], MSGS[4]]),
        (MSGS[5:6], [MSGS[4], MSGS[5]]),
        ([], [MSGS[4], MSGS[5]]),
    ]
    saved = None
    for n, (sends, want) in enumerate(rounds, 1):
        wire, saved = sev.run_repl(script, sends + ["/compact"], saved, restored_turn=False)
        check(n, saved, wire, want)
    print("PASS compaction_e2e")

if __name__ == "__main__":
    main()
