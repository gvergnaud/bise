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
# FLOOD_STOP: a file; once it exists no more lines (the REPL stays up)
stop = os.environ.get("FLOOD_STOP", "")
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
    # the hub's say/steer lines: read and dropped (logged with FLOOD_RECV_LOG);
    # EOF = the hub is gone. Its first line is its hello (BISE_REPL_TOKEN,
    # repl-core's serve_gate), dropped as repl-live does
    first = True
    while True:
        got = conn.recv(65536)
        if not got:
            break
        if first and os.environ.get("BISE_REPL_TOKEN") and got.startswith(b"hello "):
            got = got.split(b"\n", 1)[1] if b"\n" in got else b""
        first = False
        if recv_log and got:
            heard("say", got.decode("utf-8", "replace"))
    gone.set()


# FLOOD_RECV_DIR: a folder; each thing the hub sends this REPL (a `say` on
# its connection, a steer in its steer file) goes to <dir>/<agent dir>.log
# as `<ms>\t<say|steer>\t<text>` the moment it is read (send_latency_e2e.py)
recv_dir = os.environ.get("FLOOD_RECV_DIR", "")
recv_log = os.path.join(recv_dir, os.path.basename(here) + ".log") if recv_dir else ""
recv_lock = threading.Lock()


def heard(how, text):
    with recv_lock, open(recv_log, "a") as f:
        f.write("%d\t%s\t%s\n" % (int(time.time() * 1000), how, text.replace("\n", " ")))


def steers():
    # the steer file, read as it grows (every 5 ms)
    at = 0
    while not gone.is_set():
        try:
            size = os.path.getsize(steer)
        except OSError:
            size = at
        if size < at:
            at = 0
        if size > at:
            with open(steer, "rb") as f:
                f.seek(at)
                got = f.read()
            at += len(got)
            heard("steer", got.decode("utf-8", "replace"))
        time.sleep(0.005)


threading.Thread(target=drain, daemon=True).start()
if recv_log:
    threading.Thread(target=steers, daemon=True).start()
t0 = time.time()
# paced: `rate` lines a second from the flood's start, written every 20 ms;
# each line ends with its write time (` @<ms>`): the hub's transcript
# stamps it with its own time, the difference is the hub's lag
flood_t0 = None
n = 0
with open(wire, "a") as f:
    f.write("  obs: turn_started\n")
    f.flush()
    while not gone.is_set() and time.time() - t0 < life:
        # FLOOD_GO: no flood until that file exists (every REPL started)
        if (not go or os.path.exists(go)) and not (stop and os.path.exists(stop)):
            now = time.time()
            if flood_t0 is None:
                flood_t0 = now
            due = int(rate * (now - flood_t0)) + 1
            lines = []
            while n < due:
                n += 1
                lines.append("tool #%d bash : echo flood %d @%d\n" % (n, n, int(now * 1000)))
            f.write("".join(lines))
            f.flush()
        elif stop and os.path.exists(stop) and flood_t0 is not None:
            # once: a line that starts a new thread entry (a tool flood
            # stays one entry), so the agent's head moves at the end
            f.write("sb info : flood stopped @%d\n" % int(time.time() * 1000))
            f.flush()
            flood_t0 = None
        time.sleep(0.02)
sys.exit(0)
