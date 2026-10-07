"""bise ambient's core against a real hub (docs/ambient-app.md §3-§4).

A real hub (bise sbd, scripted model) on a throwaway repo, and
`bise ambient-core --workspace <ws>` as the app runs it: stdio JSON lines.
No mic, no speaker, no app: only the typed path (`send`), the task list,
a question card and its answer by digit, and stdin EOF.

Run: python3 -u tests/ambient_core_e2e.py
"""
import json
import os
import queue
import subprocess
import sys
import threading

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, ROOT, check  # noqa: E402


class Core:
    """The core as the app sees it: commands in, events out."""

    def __init__(self, E):
        self.err = open(os.path.join(E.tmp, "core.stderr"), "a")
        self.p = subprocess.Popen([EXE, "ambient-core", "--workspace", E.ws], cwd=ROOT, env=E.env,
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.err, text=True)
        self.evs = []
        self.lock = threading.Lock()
        self.q = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.p.stdout:
            try:
                v = json.loads(line)
            except ValueError:
                print("not JSON on stdout: %r" % line)
                continue
            with self.lock:
                self.evs.append(v)

    def cmd(self, **v):
        self.p.stdin.write(json.dumps(v) + "\n")
        self.p.stdin.flush()

    def last(self, ev):
        with self.lock:
            return next((e for e in reversed(self.evs) if e.get("ev") == ev), None)

    def mark(self):
        """A cursor: the events from now on (a wait 'since' it never takes an older state)."""
        with self.lock:
            return len(self.evs)

    def seen(self, pred, since=0):
        with self.lock:
            return any(pred(e) for e in self.evs[since:])

    def wait(self, pred, timeout=90, what="event", since=0):
        """pred true on an event after `since` (mark()); the timeout scales with the load (wait.until)."""
        def said():
            with self.lock:
                tail = [e for e in self.evs[since:] if e.get("ev") != "level"][-12:]
            return "%s; last events: %s" % (what, json.dumps(tail))
        wait.until(lambda: self.seen(pred, since), timeout, said, poll=0.05)


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    ok = True
    core = None
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        core = Core(E)
        core.wait(lambda e: e.get("ev") == "hub" and e.get("up") is True, 30, "hub up")
        core.wait(lambda e: e.get("ev") == "state", 30, "a state")
        st = core.last("state")
        check(all(a["name"] != "main" for a in st["agents"]), "main is not in the task list: %r" % st)
        check(st["cards"] == [], "no card yet: %r" % st)

        # typed: one input to main, then main's turn as phase/main events
        core.cmd(cmd="send", text="hello from the capsule")
        core.wait(lambda e: e.get("ev") == "sent", 30, "sent")
        sent = core.last("sent")
        check(sent["voice"] is False and sent["shot"] is False, "typed, no shot: %r" % sent)
        core.wait(lambda e: e.get("ev") == "main" and "hello from the capsule" in e.get("text", ""), 90, "main's answer")
        core.wait(lambda e: e.get("ev") == "phase" and e.get("phase") == "done", 90, "phase done")
        check(core.seen(lambda e: e.get("ev") == "phase" and e.get("phase") == "working"), "a working phase")
        check(any("hello from the capsule" in l for l in c.lines("main")), "the hub has the message in main's feed")
        check(any("[from the capsule: answer in one short line]" in l for l in c.lines("main")),
              "via capsule: main's input carries the hint its rule reads")
        check(not core.seen(lambda e: e.get("ev") == "phase" and e.get("phase") == "speaking"),
              "typed: main does not speak")

        # an agent: it shows in the task list
        c.wait_idle("main")
        with core.lock:
            mains_before = sum(1 for e in core.evs if e.get("ev") == "main")
        c.say('/new t1: {{bash: sb send main --expect-reply "keep the banner on mobile?"}}')
        core.wait(lambda e: e.get("ev") == "state" and any(a["name"] == "t1" for a in e["agents"]), 60, "t1 in the list")

        # a question card, answered by digit from the capsule
        msg = wait.until(lambda: next((l.split()[4] for l in c.lines("main") if l.startswith("sb msg-in : t1 m_") and "banner" in l), None),
                         90, "t1's question reached main")
        c.wait_idle("main", "t1")
        # main's turn on t1's message is not for the user: no words in the
        # capsule (ambient's review m_4672)
        with core.lock:
            mains_after = sum(1 for e in core.evs if e.get("ev") == "main")
        check(mains_after == mains_before, "an agent's message to main sends no main event (%d new)" % (mains_after - mains_before))
        c.say('[[bash: sb card --for %s "keep the banner on mobile?"]]' % msg)
        core.wait(lambda e: e.get("ev") == "state" and any(cd["kind"] == "question" for cd in e["cards"]), 60, "the card")
        card = [cd for cd in core.last("state")["cards"] if cd["kind"] == "question"][0]
        check(card["agent"] == "t1", "t1's card: %r" % card)
        c.wait_idle("main")
        at = core.mark()
        core.cmd(cmd="answer", card=card["id"], reply="yes")
        core.wait(lambda e: e.get("ev") == "state" and not e["cards"], 60, "the card closed", since=at)
        c.wait_line("t1", 'from="user"', 60)

        # main's own card with bare numbers ('1 a PR per task', pm's C
        # fail 41): fn + 2 from the capsule answers it, never refused
        c.wait_idle("main", "t1")
        c.say('''[[bash: sb card "$(printf 'how should agents ship code here?\\n1 a PR per task\\n2 straight to main')"]]''')
        core.wait(lambda e: e.get("ev") == "state" and any("ship code" in cd["text"] for cd in e["cards"]), 60, "main's card")
        ship = [cd for cd in core.last("state")["cards"] if "ship code" in cd["text"]][0]
        check([o["label"] for o in ship["options"]] == ["a PR per task", "straight to main"], "its options: %r" % ship)
        c.wait_idle("main")
        n_err = sum(1 for e in core.evs if e.get("ev") == "error")
        at = core.mark()
        core.cmd(cmd="answer", card=ship["id"], reply="2")
        core.wait(lambda e: e.get("ev") == "state" and not any(cd["id"] == ship["id"] for cd in e["cards"]), 60, "main's card closed by the digit", since=at)
        check(sum(1 for e in core.evs if e.get("ev") == "error") == n_err, "no refusal")
        c.wait_line("main", "straight to main", 60)

        # main's real shape (ambient m_6476): options inline at the end of
        # its question: the core's card keeps the question alone, the
        # options as its list, the label '? main needs you'
        c.wait_idle("main")
        c.say('''[[bash: sb card "Que fais-tu ? 1. regarde le diff 2. arrête-le 3. laisse-le finir"]]''')
        # the hub's card first (main's turn under load is the slow part), then the core's view of it
        c.wait(lambda: any("Que fais-tu" in (cd.get("text") or "") for cd in c.cards()), 120, "main's inline card in the hub")
        core.wait(lambda e: e.get("ev") == "state" and any(cd["text"] == "Que fais-tu ?" for cd in e["cards"]), 60, "main's inline card, its body alone")
        q = [cd for cd in core.last("state")["cards"] if cd["text"] == "Que fais-tu ?"][0]
        check([o["label"] for o in q["options"]] == ["regarde le diff", "arrête-le", "laisse-le finir"] and q.get("label") == "? main needs you", "its options and label: %r" % q)
        c.wait_idle("main")
        at = core.mark()
        core.cmd(cmd="answer", card=q["id"], reply="3")
        core.wait(lambda e: e.get("ev") == "state" and not any(cd["id"] == q["id"] for cd in e["cards"]), 60, "the inline card answered", since=at)
        c.wait_line("main", "laisse-le finir", 60)

        # fn space + tab (ambient-lead m_6513): main is asked to start an
        # agent, with the hub's start hint
        c.wait_idle("main")
        core.cmd(cmd="start", text="fix the login loop on Safari")
        c.wait_line("main", "start an agent for: fix the login loop on Safari", 60)
        c.wait(lambda: any("[from the capsule: start an agent; answer in one short line]" in l for l in c.lines("main")), 60, "the start hint")

        # bise's bookkeeping stays in the TUI (pm's C fail 36): main's
        # archive suggestion is a drop card for the TUI, never one in the
        # capsule, and while it is open a second drop never acts
        c.wait_idle("main", "t1")
        c.say('/new -w t2: {{bash: echo wip > wip.txt}}')
        c.wait_idle("t2")
        c.say("[[bash: sb drop t2]]")
        c.wait(lambda: any(x.get("kind") == "drop" and x.get("agent") == "t2" for x in c.cards()), 60, "the drop card in the TUI")
        drop = next(x for x in c.cards() if x.get("kind") == "drop")
        core.wait(lambda e: e.get("ev") == "state" and any(a["name"] == "t2" for a in e["agents"]), 30, "t2 in the core's list")
        wait.holds(lambda: not any(cd["id"] == drop["id"] for cd in core.last("state")["cards"]), 1,
                   lambda: "no drop card in the capsule: %r" % core.last("state")["cards"])
        c.wait_idle("main")
        c.say("[[bash: sb drop t2]]")
        c.wait_line("main", "has not answered card #%d" % drop["id"], 60)
        check(any(x["id"] == drop["id"] for x in c.cards()), "the card still asks him")

        # round 10 (identity10 #data): t1's preview and history through the
        # hub's history op, his words to it, live in its open panel
        c.wait_idle("main", "t1")
        core.cmd(cmd="agent_preview", agent="t1")
        core.wait(lambda e: e.get("ev") == "agent_preview" and e["agent"] == "t1", 30, "t1's preview")
        pv = core.last("agent_preview")
        check(isinstance(pv["actions"], list) and "waiting" in pv and "pages" in pv, "the preview's fields: %r" % pv)
        core.cmd(cmd="agent_history", agent="t1")
        core.wait(lambda e: e.get("ev") == "agent_history" and e["agent"] == "t1", 30, "t1's history")
        h = core.last("agent_history")
        kinds = [x["kind"] for x in h["entries"]]
        check(kinds and set(kinds) <= {"you", "agent", "tools", "card", "page", "report", "from-agent", "to_agent"}, "its entries: %r" % kinds)
        core.cmd(cmd="agent_send", agent="t1", text="e2e ping from the panel", mode="now")
        core.wait(lambda e: e.get("ev") == "sent" and e.get("agent") == "t1" and e.get("mode") == "now", 30, "sent to t1")
        c.wait_line("t1", "e2e ping from the panel", 60)
        core.wait(lambda e: e.get("ev") == "agent_entry" and e["agent"] == "t1" and e["entry"]["kind"] == "you"
                  and "e2e ping from the panel" in e["entry"]["text"], 30, "his message live in t1's panel")
        core.cmd(cmd="agent_unwatch", agent="t1")

        # the app goes: the core exits on stdin EOF
        core.p.stdin.close()
        rc = core.p.wait(timeout=10)
        check(rc == 0, "exit 0 on stdin EOF: %r" % rc)
        core = None
    except AssertionError as e:
        print("FAIL", e)
        ok = False
    finally:
        if core:
            core.p.kill()
        E.close()
    print("ambient_core_e2e:", "ok" if ok else "FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
