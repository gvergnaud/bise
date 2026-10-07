"""A version rollback after bise desktop S2 (architect m_8524 c.): an older
sb-core replays a journal that a newer one wrote, with the cross-hub kinds
(x_out, x_acked, x_failed, x_settled, x_in, x_replied), the timers'
acts (act_set, act_done) and S10's follow and job_end in it. It skips each of them (they are reported
as skipped, never misread) and rebuilds the rest of the state.

    python3 -u tests/old_core_replay.py [<base sha>]   (default: HEAD~1)

The base's sb-core comes from scripts/bins.sh in a throwaway worktree of
the base (cached by its sources' key). The other direction, a new sb-core
on an old journal, is every replay test of the current one.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)


def msg(i, frm, to):
    return {"type": "message_sent", "msg": {"id": i, "thread": i, "from": frm, "to": to, "reply_to": None,
            "expect_reply": True, "auto": False, "text": "m%d" % i, "created_ms": 1000 + i, "plain": False}}


FIXTURE = [
    msg(1, "user", "main"),
    {"type": "x_out", "out": {"xid": 1, "project": "shop-1", "kind": "input", "msg": 1, "text": "m1",
                              "state": "pending", "tries": 0}},
    {"type": "x_failed", "xid": 1},
    {"type": "act_set", "act": {"id": 1, "due_ms": 5000, "what": {"kind": "retry", "xid": 1}}},
    {"type": "act_done", "id": 1},
    {"type": "x_acked", "xid": 1},
    msg(2, "@shop", "main"),
    {"type": "x_settled", "xid": 1},
    {"type": "x_in", "in": {"hub": "home-1", "xid": 7, "msg": 3, "kind": "ask", "replied": False}},
    msg(3, "@bise", "main"),
    {"type": "x_replied", "hub": "home-1", "xid": 7},
    {"type": "message_settled", "id": 3},
    # bise desktop S10: a follow and its job's end; J: the follow names the
    # hub that asked, and the end goes back to it through the outbox
    {"type": "follow", "name": "perf", "on": True, "hub": "home-1"},
    {"type": "job_end", "name": "perf", "state": "done"},
    {"type": "x_out", "out": {"xid": 2, "project": "home-1", "kind": "job_end", "msg": 0,
                              "text": "{\"agent\":\"perf\",\"key\":9000,\"state\":\"done\"}", "state": "pending", "tries": 0}},
]
NEW = [i for i, e in enumerate(FIXTURE) if e["type"].startswith(("x_", "act_")) or e["type"] in ("follow", "job_end")]


def main():
    base = sys.argv[1] if len(sys.argv) > 1 else "HEAD~1"
    sha = subprocess.check_output(["git", "rev-parse", base], cwd=ROOT, text=True).strip()
    tmp = tempfile.mkdtemp(prefix="old-core-", dir=os.environ.get("TMPDIR"))
    wt = os.path.join(tmp, "base")
    ok = True
    core = None
    try:
        subprocess.check_call(["git", "worktree", "add", "-q", "--detach", wt, sha], cwd=ROOT)
        out = subprocess.run(["bash", "scripts/bins.sh", "path", "sb-core"], cwd=wt, capture_output=True, text=True)
        binary = out.stdout.strip().splitlines()[-1] if out.stdout.strip() else ""
        assert os.access(binary, os.X_OK), "the base's sb-core: %s %s" % (out.stdout, out.stderr)
        s0 = socket.socket()
        s0.bind(("127.0.0.1", 0))
        port = s0.getsockname()[1]
        s0.close()
        core = subprocess.Popen([binary], env=dict(os.environ, SB_CORE_PORT=str(port)),
                                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        banner = core.stdout.readline()
        assert banner.startswith("sb-core on"), "banner: %r" % banner
        conn = socket.create_connection(("127.0.0.1", port))
        f = conn.makefile("rw")

        def call(v):
            f.write(json.dumps(v) + "\n")
            f.flush()
            return json.loads(f.readline())

        call({"t": "init", "workspace": "/w"})
        out = call({"t": "replay_many", "evs": FIXTURE})
        skipped = out.get("skipped", [])
        assert skipped == NEW, "the base's sb-core skips exactly the new kinds: %r (want %r)" % (skipped, NEW)
        view = call({"t": "view_all"})
        assert core.poll() is None, "the base's sb-core died on the journal"
        text = json.dumps(view)
        assert all(("m%d" % i) in text for i in (1, 2, 3)), "the messages are rebuilt: %s" % text[:500]
        print("ok   old_core_replay: %s's sb-core skipped %d new lines, kept the messages" % (sha[:8], len(NEW)))
    except AssertionError as e:
        print("FAIL old_core_replay:", e)
        ok = False
    finally:
        if core:
            core.kill()
        subprocess.run(["git", "worktree", "remove", "--force", wt], cwd=ROOT, capture_output=True)
        shutil.rmtree(tmp, ignore_errors=True)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
