#!/usr/bin/env python3
"""Do two sb-core binaries give the same answers on a journal? The evidence of
a refactor that must not change behaviour (state-schema, bp-hot's same.py).

    core_same.py <sb-core A> <sb-core B> <journal.jsonl> [--batch N] [--views K]

Each binary gets its own sb-core on a free port: `init`, then the journal's
events in `replay_many` batches of N (default 500), and a `view_all` after
every K-th batch (default: 2 points, the middle batch and the last). Every
answer line of A is compared with B's, byte for byte; each view_all's sha256
is printed for both. Exit 0 when nothing differs, 1 otherwise (the first
differing step's two answers are printed, cut).

Read-only: the journal file is only read (use a copy). Heavy (a big journal
is minutes of CPU per binary): run it through ~/.bise/run/heavy-run.
"""
import argparse
import hashlib
import json
import os
import socket
import subprocess
import sys
import time


def run(binary, evs, batch, view_at):
    """Every answer line of one sb-core, in order: (step label, line)."""
    s0 = socket.socket()
    s0.bind(("127.0.0.1", 0))
    port = s0.getsockname()[1]
    s0.close()
    core = subprocess.Popen([binary], env=dict(os.environ, SB_CORE_PORT=str(port)),
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    out = []
    try:
        assert core.stdout.readline().startswith("sb-core on"), "%s: no sb-core banner" % binary
        f = socket.create_connection(("127.0.0.1", port)).makefile("rw")

        def call(label, v):
            f.write(json.dumps(v) + "\n")
            f.flush()
            out.append((label, f.readline()))

        def ask(v):
            f.write(json.dumps(v) + "\n")
            f.flush()
            return json.loads(f.readline())

        t = time.time()
        call("init", {"t": "init", "workspace": "/w"})
        for n, i in enumerate(range(0, len(evs), batch)):
            call("replay %d" % n, {"t": "replay_many", "evs": evs[i:i + batch]})
            if n in view_at:
                call("view_all %d" % n, {"t": "view_all"})
                out[-1] = (out[-1][0], whole_view(out[-1][1], ask))
        print("%s: %.1f s" % (os.path.basename(binary), time.time() - t), flush=True)
    finally:
        core.kill()
    return out


def whole_view(line, ask):
    """view_all's answer with every message whole (view-all-perf): a cold
    row (`whole` false, its msg without text and parts) gets its msg from
    `msg_bodies`, 256 ids a call, and `whole` goes (an older sb-core has
    none, and never a cold row: it is never asked, msg_bodies would be a
    step there). The same facts as the old full view_all, so an old and a
    new binary compare byte for byte."""
    a = json.loads(line)
    rows = a.get("view", {}).get("msgs", [])
    cold = [r["msg"]["id"] for r in rows if r.get("whole") is False]
    got = {}
    for i in range(0, len(cold), 256):
        b = ask({"t": "msg_bodies", "ids": cold[i:i + 256]})
        got.update({r["msg"]["id"]: r["msg"] for r in b["view"]["msgs"]})
    for r in rows:
        if r.get("whole") is False:
            r["msg"] = got[r["msg"]["id"]]
        r.pop("whole", None)
    return json.dumps(a, ensure_ascii=False, separators=(",", ":")) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("journal")
    ap.add_argument("--batch", type=int, default=500)
    ap.add_argument("--views", type=int, default=0, help="a view_all every K batches (0: the middle and the last)")
    a = ap.parse_args()
    evs = [json.loads(l) for l in open(a.journal) if l.strip()]
    nb = (len(evs) + a.batch - 1) // a.batch
    view_at = set(range(a.views - 1, nb, a.views)) if a.views > 0 else {nb // 2, nb - 1}
    ra = run(a.a, evs, a.batch, view_at)
    rb = run(a.b, evs, a.batch, view_at)
    differing = 0
    for (label, x), (_, y) in zip(ra, rb):
        if label.startswith("view_all"):
            print("%s sha256 A %s B %s" % (label, hashlib.sha256(x.encode()).hexdigest()[:12],
                                           hashlib.sha256(y.encode()).hexdigest()[:12]))
        if x != y:
            if differing == 0:
                print("first difference at %s:\n  A %s\n  B %s" % (label, x[:400].rstrip(), y[:400].rstrip()))
            differing += 1
    if len(ra) != len(rb):
        print("answers: A %d, B %d" % (len(ra), len(rb)))
        differing += 1
    print("events %d, steps %d, differing %d" % (len(evs), len(ra), differing))
    return 0 if differing == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
