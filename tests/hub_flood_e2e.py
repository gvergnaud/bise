#!/usr/bin/env python3
"""hub-fifo: a hub with 30 busy agents flooding lines still answers a
new client's `initialize` at once, and sends its state (hub/agents) at
most ~10 times a second.

The outage it guards (2026-10-09): his hub's loop, at 100% CPU, rebuilt
the whole snapshot for each REPL line and served messages in arrival
order, so a client's hello waited hours behind the lines: the TUI and the
desktop app showed 'no agents yet'. Now the clients' messages go ahead
(daemon/inbox.rs) and the state is throttled (daemon/state_gate.rs).

A throwaway hub on a temp app root (BISE_APP_ROOT: links to this tree)
whose `repl-live` is tests/fake_repl_flood.py; its journal is seeded with
30 tasks (e2e.seed_tasks), so main and the 30 REPLs start at the boot and
flood. Then, while they flood: three new clients, each initialize ->
its answer under 1 s; one client counts the hub/agents of 3 s.

Run: python3 -u tests/hub_flood_e2e.py  (needs a built bise)"""
import json
import os
import socket
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wait  # noqa: E402
from e2e import Env, ROOT, HERE, check, seed_tasks  # noqa: E402

TASKS = int(os.environ.get("FLOOD_TASKS", "30"))
# the backlog before the hellos: this many flood lines written
BACKLOG_LINES = 20000
HELLO_MAX_S = 1.0
STATE_WINDOW_S = 3.0


def app_root(tmp):
    """This tree's app root, its repl-live a flooding fake."""
    root = os.path.join(tmp, "app")
    os.makedirs(root)
    for name in os.listdir(ROOT):
        if name != "repl-live":
            os.symlink(os.path.join(ROOT, name), os.path.join(root, name))
    fake = os.path.join(root, "repl-live")
    with open(fake, "w") as f:
        f.write("#!/bin/sh\nexec %s -u %s\n" % (sys.executable, os.path.join(HERE, "fake_repl_flood.py")))
    os.chmod(fake, 0o755)
    return root


def started(state):
    """The agents whose REPL took the hub's connection (its wire log has
    its turn_started line)."""
    d = os.path.join(state, "agents")
    n = 0
    for a in os.listdir(d) if os.path.isdir(d) else []:
        w = os.path.join(d, a, "wire.log")
        if os.path.exists(w) and os.path.getsize(w) > 0:
            n += 1
    return n


def wire_lines(state):
    """The flood lines written so far, every agent (~ bytes / line)."""
    d = os.path.join(state, "agents")
    total = 0
    for a in os.listdir(d) if os.path.isdir(d) else []:
        w = os.path.join(d, a, "wire.log")
        if os.path.exists(w):
            total += os.path.getsize(w)
    return total // 30


def hello_s(sock_path, timeout=60):
    """A new client: seconds from its `initialize` to the hub's answer
    (its hub-wide state)."""
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(timeout)
    s.connect(sock_path)
    t0 = time.time()
    s.sendall((json.dumps({"jsonrpc": "2.0", "id": "flood", "method": "initialize",
                           "params": {"proto": 1, "client": {"name": "hub_flood_e2e"}}}) + "\n").encode())
    f = s.makefile("r")
    try:
        # a text match on the line's head, not a parse: the clock is for
        # the hub, not for this reader
        for line in f:
            if '"id":"flood"' in line[:120]:
                return time.time() - t0
    except socket.timeout:
        pass
    finally:
        s.close()
    return float("inf")


def main():
    E = Env()
    try:
        names = ["t%d" % i for i in range(TASKS)]
        seed_tasks(E.state, E.ws, names)
        E.env["BISE_APP_ROOT"] = app_root(E.tmp)
        E.env["FLOOD_RATE"] = "400"
        # the hub's own marks (client hello built / written): hub-timing.log
        E.env["SB_TIMING"] = os.path.join(E.tmp, "hub-timing.log")
        go = os.path.join(E.tmp, "flood-go")
        E.env["FLOOD_GO"] = go
        c = E.start_hub()
        wait.until(lambda: started(E.state) >= TASKS + 1, 120, "main and %d tasks started" % TASKS, poll=0.2)
        open(go, "w").close()
        # a backlog builds
        start = wire_lines(E.state)
        wait.until(lambda: wire_lines(E.state) - start >= BACKLOG_LINES, 60, "%d more flood lines" % BACKLOG_LINES, poll=0.2)
        sock = os.path.join(E.state, "hub.sock")
        took = [hello_s(sock) for _ in range(3)]
        print("initialize -> its answer while %d REPLs flood: %s" % (TASKS + 1, ", ".join("%.3f s" % t for t in took)))
        check(max(took) < HELLO_MAX_S, "an initialize is answered in under %.1f s while the REPLs flood: %s" % (HELLO_MAX_S, took))
        # a state broadcast sends hub/agents to every client: they count
        before = len(c.notes("hub/agents"))
        time.sleep(STATE_WINDOW_S)
        states = len(c.notes("hub/agents")) - before
        print("hub/agents in %.0f s of flood: %d" % (STATE_WINDOW_S, states))
        check(states <= STATE_WINDOW_S * 10 + 5, "the state goes at most ~10 times a second: %d in %.0f s" % (states, STATE_WINDOW_S))
        check(states >= 1, "the state still goes while the REPLs flood")
        print("PASS hub_flood_e2e")
    finally:
        E.close()


if __name__ == "__main__":
    main()
