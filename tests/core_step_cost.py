#!/usr/bin/env python3
"""A step of sb-core costs the same with 200 or 1,000 archived agents
(chantier c4b-core-step-flat).

Its own sb-core (scripts/bins.sh path sb-core, or SB_CORE_BIN) on a free
port: main, LIVE live tasks and N archived ones, each of which ran (its
REPL ready, a turn, idle, exited, then archived: the runtime list keeps
it, as on his hub), then K ticks, K `say`s to a live task and K `sb list`
speaks steps. It counts the retired instructions of sb-core for each kind
(proc_pid_rusage, macOS), never wall time, so the machine's load does not
move it. For each kind: the cost per step at N = 1,000 must stay under
SLACK x its cost at N = 200 (a walk of every agent ever run: ~x5).

    python3 -u tests/core_step_cost.py [--n 200,1000] [--live 25] [--k 300] [--show]

--show prints the numbers (instructions per step) and skips the verdict.
"""
import argparse
import ctypes
import json
import os
import socket
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SLACK = 1.5
NOW = 1_800_000_000_000


def instructions(pid):
    """sb-core's retired instructions so far (rusage_info_v4.ri_instructions)."""
    lib = ctypes.CDLL("/usr/lib/libproc.dylib")
    buf = (ctypes.c_uint64 * 64)()
    if lib.proc_pid_rusage(pid, 4, ctypes.byref(buf)) != 0:
        raise OSError("proc_pid_rusage failed for %d" % pid)
    return buf[2 + 29]


class Core:
    def __init__(self, binary):
        s0 = socket.socket()
        s0.bind(("127.0.0.1", 0))
        port = s0.getsockname()[1]
        s0.close()
        self.p = subprocess.Popen([binary], env=dict(os.environ, SB_CORE_PORT=str(port)), stdin=subprocess.DEVNULL,
                                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        assert self.p.stdout.readline().startswith("sb-core on"), "no sb-core banner"
        self.f = socket.create_connection(("127.0.0.1", port)).makefile("rw")
        self.now = NOW

    def call(self, v):
        self.f.write(json.dumps(v) + "\n")
        self.f.flush()
        out = json.loads(self.f.readline())
        assert "error" not in out, "sb-core refused %s: %s" % (v.get("t"), out)
        return out

    def step(self, v):
        """one input at the hub's clock; a git need answered `false`"""
        self.now += 500
        ans = []
        while True:
            out = self.call(dict(v, now=self.now, git=True, ans=ans))
            if "need" not in out:
                return out
            ans.append(False)

    def close(self):
        self.p.kill()


def task(name, at):
    return {"type": "task_created", "name": name, "parent": "main", "at_ms": at,
            "ws": {"mode": "shared", "path": "/w", "branch": None, "base_commit": None, "dropped": False},
            "brief": {"objective": "a task of the cost bench", "context": ""}}


def ran(c, name):
    """its REPL connects, runs a turn and goes idle"""
    c.step({"t": "ready", "agent": name})
    c.step({"t": "turn_started", "agent": name})
    c.step({"t": "idle", "agent": name, "leftover": False, "held": False})


def world(binary, archived, live):
    c = Core(binary)
    c.call({"t": "init", "workspace": "/w"})
    gone = ["a%04d" % i for i in range(archived)]
    tasks = ["t%02d" % i for i in range(1, live + 1)]
    c.call({"t": "replay_many", "evs": [task(n, NOW - 10_000_000 + i) for i, n in enumerate(gone + tasks)]})
    c.step({"t": "ready", "agent": "main"})
    for n in gone:
        ran(c, n)
        c.step({"t": "exited", "agent": n, "crashed": False, "reason": "", "stalled": False, "loaded": True,
                "row": 0, "stopped": True})
    c.call({"t": "replay_many", "evs": [{"type": "lifecycle", "name": n, "lifecycle": "archived", "reason": "drop"}
                                        for n in gone]})
    for n in tasks:
        ran(c, n)
    return c, tasks


def per_step(c, k, make):
    i0 = instructions(c.p.pid)
    for i in range(k):
        c.step(make(i))
    return (instructions(c.p.pid) - i0) / k


def measure(binary, archived, live, k):
    c, tasks = world(binary, archived, live)
    try:
        return {
            "tick": per_step(c, k, lambda i: {"t": "tick"}),
            "say": per_step(c, k, lambda i: {"t": "say", "focus": tasks[i % len(tasks)], "text": "bench %d" % i,
                                              "queued": True}),
            "list": per_step(c, k, lambda i: {"t": "speaks", "token": 9000 + i, "from": "main", "cmd": "list",
                                               "who": "agent", "dir": "/w"}),
        }
    finally:
        c.close()


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", default="200,1000")
    ap.add_argument("--live", type=int, default=25)
    ap.add_argument("--k", type=int, default=300)
    ap.add_argument("--show", action="store_true")
    a = ap.parse_args(argv)
    if sys.platform != "darwin":
        print("SKIP core_step_cost: proc_pid_rusage counts instructions on macOS only")
        return 0
    binary = os.environ.get("SB_CORE_BIN") or subprocess.run(
        [os.path.join(ROOT, "scripts", "bins.sh"), "path", "sb-core"], capture_output=True, text=True, check=True).stdout.strip()
    ns = [int(x) for x in a.n.split(",")]
    got = {n: measure(binary, n, a.live, a.k) for n in ns}
    for n in ns:
        print("%5d archived: " % n + ", ".join("%s %.2fM" % (kind, v / 1e6) for kind, v in got[n].items())
              + " instructions per step")
    if a.show or len(ns) < 2:
        return 0
    lo, hi = got[ns[0]], got[ns[-1]]
    fails = ["%s: x%.1f from %d to %d archived (at most x%.1f)" % (kind, hi[kind] / lo[kind], ns[0], ns[-1], SLACK)
             for kind in lo if hi[kind] > SLACK * lo[kind]]
    for f in fails:
        print("FAIL core_step_cost " + f)
    if not fails:
        print("ok core_step_cost: a step costs the same with %d or %d archived agents" % (ns[0], ns[-1]))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
