#!/usr/bin/env python3
"""A fake agent REPL that floods (hub_flood_e2e.py): the hub spawns it as
its app root's `repl-live`. It says it is up the way repl-live does (a
`harness-info steer=… interrupt=…` line and `REPL on …` on stdout, the
hub's repl.log), takes the hub's connection on BEND_REPL_PORT, starts a
turn and never ends it: a `tool #N bash : …` line after another in its
wire log (BEND_WIRE_LOG), each one an activity change for sb-core. It
exits when the hub closes the connection, or after FLOOD_SECS at most.

FLOOD_RATE: lines per second (default 400). FLOOD_GO: a file; the flood
waits for it (the test starts every REPL first: the hub starts them a few
at a time, and a start behind a flood would wait for it)."""
import os
import socket
import sys
import threading
import time

port = int(os.environ["BEND_REPL_PORT"])
wire = os.environ["BEND_WIRE_LOG"]
rate = int(os.environ.get("FLOOD_RATE", "400"))
life = float(os.environ.get("FLOOD_SECS", "120"))
go = os.environ.get("FLOOD_GO", "")
here = os.path.dirname(wire)
steer, interrupt = os.path.join(here, "steer.txt"), os.path.join(here, "interrupt.txt")
for p in (steer, interrupt):
    open(p, "a").close()

srv = socket.socket()
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", port))
srv.listen(1)
print("harness-info steer=%s interrupt=%s" % (steer, interrupt), flush=True)
print("REPL on 127.0.0.1:%d" % port, flush=True)
conn, _ = srv.accept()
gone = threading.Event()


def drain():
    # the hub's say/steer lines: read and dropped; EOF = the hub is gone
    while conn.recv(65536):
        pass
    gone.set()


threading.Thread(target=drain, daemon=True).start()
t0 = time.time()
batch = max(1, rate // 50)
n = 0
with open(wire, "a") as f:
    f.write("  obs: turn_started\n")
    f.flush()
    while not gone.is_set() and time.time() - t0 < life:
        # FLOOD_GO: no flood until that file exists (every REPL started)
        if not go or os.path.exists(go):
            lines = []
            for _ in range(batch):
                n += 1
                lines.append("tool #%d bash : echo flood %d\n" % (n, n))
            f.write("".join(lines))
            f.flush()
        time.sleep(0.02)
sys.exit(0)
