"""Client-protocol step 5's stub on a real hub (architect m_15183,
proto-lead m_15200): an older client's `{"op":"hello"}` gets the hub's exe
and reload id, then no burst and no older event, ever; the connection
stays for the released desktop cores' door (v2026.10.2-28's and -29's, the
release cut from main before client-protocol merges), one release: their
typed hello, their cmd lines, and their op lines replayed from
rust/proto/fixtures/released/core_door.jsonl (-28) and core_door_v2026.10.2-29.jsonl
(-29: one more cmd, tool_out, and an answer's files), each served by the typed arm
it maps to (rust/switchboard/src/daemon/rpc.rs DOOR_OPS): his words reach
the agent (input), the stop stops (interrupt), the timer ends
(every_stop); a line outside the table is refused, with one hub.log line.
The core's home connection (a typed hello without typed_only) gets the
older events that core reads (DOOR_EVENTS: its home state, ready, a page,
main's lines); nothing else ever gets an older event.

Run: python3 -u tests/older_door_e2e.py (after scripts/bins.sh)
"""
import json
import os
import socket
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402

EXE = e2e.EXE
FIXTURES = [os.path.join(e2e.ROOT, "rust", "proto", "fixtures", "released", f) for f in ("core_door.jsonl", "core_door_v2026.10.2-29.jsonl")]
# the older events a terminal of before client-protocol read
# (approvals, artifacts, versions: typed tags of the same name, not older)
OLDER = ("state", "line", "history", "ready", "page")
PAGE = '<section data-kit="prose" data-id="p1"><p>the week: dark mode shipped.</p></section>\n'



def check(cond, what):
    if not cond:
        raise AssertionError(what)


def released(path):
    """A released core's lines, by op (or cmd)."""
    out = {}
    for l in open(path):
        if l.strip():
            v = json.loads(l)
            out.setdefault(v.get("op") or "cmd:" + v["cmd"], []).append(v)
    return out


class Door:
    """A raw hub.sock connection that says `{"op":"hello"}` first."""

    def __init__(self, sock_path):
        self.s = socket.socket(socket.AF_UNIX)
        self.s.connect(sock_path)
        self.lines = []
        self.lock = threading.Lock()
        self.send({"op": "hello"})
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.s.makefile("r"):
            try:
                v = json.loads(line)
            except ValueError:
                continue
            with self.lock:
                self.lines.append(v)

    def send(self, v):
        self.s.sendall((json.dumps(v) + "\n").encode())

    def got(self):
        with self.lock:
            return list(self.lines)

    def wait(self, f, what, timeout=30):
        return wait.until(lambda: f(self.got()) or None, timeout, what)


def hub_log(E):
    p = os.path.join(E.state, "hub.log")
    return open(p).read() if os.path.exists(p) else ""


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: scripts/bins.sh")
    E = e2e.Env()
    E.env["SB_EVERY_MIN_MS"] = "1000"
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        sock = os.path.join(E.state, "hub.sock")
        project = c.project()
        rel, rel29 = (released(f) for f in FIXTURES)
        check(rel29["cmd:hello"] == rel["cmd:hello"] and rel29.keys() >= rel.keys(), "-29's lines are -28's and more: %r" % sorted(rel29.keys() - rel.keys()))

        # an older terminal: exe and reload, then nothing, ever
        term = Door(sock)
        first = term.wait(lambda g: g[:1], "the stub's hello")[0]
        check(set(first) == {"ev", "exe", "reload"} and first["ev"] == "hello" and first["exe"], "the stub's hello: %r" % first)

        # the -28 core's home connection: its typed hello without
        # typed_only, then the older events it reads (state, ready)
        home_hello, door_hello = rel["cmd:hello"]
        check("typed_only" not in home_hello and door_hello.get("typed_only"), "the fixture's two hellos: %r" % rel["cmd:hello"])
        home = Door(sock)
        home.wait(lambda g: g[:1], "the home's stub hello")
        home.send(home_hello)
        home.wait(lambda g: any(v.get("ev") == "welcome" for v in g), "welcome on the home connection")
        st = home.wait(lambda g: next((v for v in g if v.get("ev") == "state"), None), "the home state")
        check(any(a.get("name") == "main" for a in st.get("agents", [])), "main in the home state: %r" % st.get("agents"))
        home.wait(lambda g: any(v.get("ev") == "ready" for v in g), "ready on the home connection")

        # the -28 core's project door: its typed hello, then typed events only
        door = Door(sock)
        door.wait(lambda g: g[:1], "the door's stub hello")
        door.send(door_hello)
        door.wait(lambda g: any(v.get("ev") == "welcome" for v in g), "welcome on the door")
        door.wait(lambda g: any(v.get("ev") == "agents" for v in g), "agents on the door")

        # input -> command/run's arm: his words reach main; main's lines
        # reach the home connection (the capsule's feed)
        n = len(home.got())
        door.send({"op": "input", "focus": "main", "text": "door says hi"})
        c.wait_line("main", "door says hi", 60)
        home.wait(lambda g: any(v.get("ev") == "line" and v.get("agent") == "main" and "door says hi" in v.get("line", "") for v in g[n:]), "main's line on the home connection")
        c.wait_idle("main")

        # a page: its older page line on the home connection
        with open(os.path.join(E.tmp, "door-page.html"), "w") as f:
            f.write(PAGE)
        c.say('[[bash: sb page publish %s --id door-page --title "door page"]]' % os.path.join(E.tmp, "door-page.html"))
        home.wait(lambda g: any(v.get("ev") == "page" and v.get("id") == "door-page" for v in g), "the page on the home connection", 60)
        c.wait_idle("main")

        # interrupt -> turn/interrupt's arm: the stop stops a long turn
        c.say("[[bash: sleep 40]]")
        c.wait_status("main", "working", 30)
        t0 = time.time()
        door.send({"op": "interrupt", "agent": "main"})
        c.wait_status("main", "idle", 30)
        check(time.time() - t0 < 30, "the stop stopped the turn")

        # every_stop -> scheduled/stop's arm: the timer ends
        c.say('[[bash: sb every 10m "door timer"]]')
        items = c.wait(lambda: (c.hub.get("hub/scheduled") or {}).get("items"), 60, "a timer")
        tid = items[0]["id"]
        c.wait_idle("main")
        door.send({"op": "every_stop", "id": tid})
        c.wait(lambda: not (c.hub.get("hub/scheduled") or {}).get("items"), 30, "the timer ended")

        # page_voice -> page/voice's arm: a page the hub doesn't hold is
        # nothing (no refusal)
        for v in rel["page_voice"]:
            door.send(v)

        # -29's lines -28 hadn't: each reaches its typed arm (an answer or
        # its typed error, never 'unknown command')
        for v in rel29["cmd:tool_out"] + [a for a in rel29["cmd:answer"] if a.get("files")]:
            n = len(door.got())
            door.send(dict(v, project=project))
            got = door.wait(lambda g: next((x for x in g[n:] if x.get("ev") in ("tool_out", "error", "notice")), None), "%s answered" % v["cmd"])
            check("unknown command" not in got.get("text", ""), "-29's %s refused as unknown: %r" % (v["cmd"], got))

        # a line outside the table: refused, logged, its notice typed
        door.send({"op": "history", "agent": "main", "before": 10})
        door.wait(lambda g: any(v.get("ev") == "notice" and "history" in v.get("text", "") for v in g), "the refusal's notice")
        wait.until(lambda: "hub.sock refused the op \"history\"" in hub_log(E), 10, "the refusal in hub.log")
        check("page_voice" not in hub_log(E).split("hub.sock refused")[-1], "page_voice refused")

        # no older event outside the home connection: the older terminal,
        # the typed_only door, a JSON-RPC client
        for name, conn in (("terminal", term), ("door", door)):
            older = [v for v in conn.got() if v.get("ev") in OLDER]
            check(not older, "an older event on the %s: %r" % (name, older[:2]))
        check(term.got() == [first], "the older terminal got more than exe and reload: %r" % term.got()[1:3])
        with c.lock:
            check(not any("ev" in v for v in c.events), "an older event on a JSON-RPC client: %r" % [v for v in c.events if "ev" in v][:2])
        older = sorted({v["ev"] for v in home.got() if v.get("ev") in OLDER})
        check(set(older) <= {"state", "ready", "page", "line"}, "the home connection got an older kind outside DOOR_EVENTS: %r" % older)

        # stop_hub -> hub/stop's arm (that release's `bise stop`)
        stop = Door(sock)
        stop.wait(lambda g: g[:1], "the stop's stub hello")
        stop.send(rel["stop_hub"][0])
        E.hub.wait(timeout=20)
        E.hub = None
        wait.until(lambda: not os.path.exists(sock), 10, "the hub's socket gone")

        ok = True
        print("older_door_e2e: ok")
    finally:
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
