#!/usr/bin/env python3
"""Profile sb-core on a journal: how long its boot replay, a Tick, a `say`
and `view_all` take, and where the time goes (macOS `sample`).

    core_profile.py <sb-core binary> <journal.jsonl> [--out <dir>] [--batch N]

Read-only: it starts its own sb-core on a free port, replays a COPY's
events into it (the journal file is only read) and kills it at the end.
Heavy (a big journal is ~30 s of CPU): run it under the heavy lock,
    /usr/bin/lockf -k ~/.bise/run/heavy.lock nice -n 10 scripts/core_profile.py ...
The sb-core binary: `scripts/bins.sh path sb-core` (this tree's).

It prints the timings and writes three samples to <dir> (default $TMPDIR):
sample-replay.txt, sample-tick.txt, sample-view.txt. Read each from its
'Sort by top of stack' list: Bend defs show as WL_FID_<FILE>_<DEF>
(e.g. WL_FID_MODEL_QUEUED_IN = model.bend queued_in). The method and the
patterns to look for: .agents/skills/bend-perf/SKILL.md.
"""
import argparse
import json
import os
import socket
import subprocess
import time

from core_lines import answer


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("binary")
    ap.add_argument("journal")
    ap.add_argument("--out", default=os.environ.get("TMPDIR", "."))
    ap.add_argument("--batch", type=int, default=500)
    a = ap.parse_args()

    s0 = socket.socket()
    s0.bind(("127.0.0.1", 0))
    port = s0.getsockname()[1]
    s0.close()
    core = subprocess.Popen([a.binary], env=dict(os.environ, SB_CORE_PORT=str(port)),
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    assert core.stdout.readline().startswith("sb-core on"), "no sb-core banner"
    f = socket.create_connection(("127.0.0.1", port)).makefile("rw")

    def call(v):
        t = time.time()
        f.write(json.dumps(v) + "\n")
        f.flush()
        line = answer(f.readline)
        return time.time() - t, len(line)

    def sample(name, secs):
        return subprocess.Popen(["/usr/bin/sample", str(core.pid), str(secs), "1", "-file", os.path.join(a.out, name)],
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    try:
        call({"t": "init", "workspace": "/w"})
        evs = [json.loads(l) for l in open(a.journal) if l.strip()]
        s = sample("sample-replay.txt", 20)
        t = time.time()
        for i in range(0, len(evs), a.batch):
            call({"t": "replay_many", "evs": evs[i:i + a.batch]})
        print("replay: %d events in %.1f s (batches of %d)" % (len(evs), time.time() - t, a.batch), flush=True)
        s.wait()

        s = sample("sample-tick.txt", 8)
        time.sleep(0.5)
        t, n, size = time.time(), 0, 0
        while time.time() - t < 6:
            _, size = call({"t": "tick", "now": 1_800_000_000_000 + n * 500, "git": True, "ans": []})
            n += 1
        print("tick: %.1f ms each (%d in 6 s), last answer %d bytes" % (6000.0 / max(n, 1), n, size), flush=True)
        s.wait()

        for k in range(3):
            dt, size = call({"t": "say", "focus": "main", "text": "profile %d" % k, "queued": True})
            print("say: %.1f ms, answer %d bytes" % (dt * 1000, size), flush=True)

        s = sample("sample-view.txt", 6)
        time.sleep(0.5)
        for _ in range(2):
            dt, size = call({"t": "view_all"})
            print("view_all: %.2f s, %d bytes" % (dt, size), flush=True)
        s.wait()
    finally:
        core.kill()


if __name__ == "__main__":
    main()
