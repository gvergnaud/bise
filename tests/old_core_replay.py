"""A version rollback after bise desktop S2 (architect m_8524 c.): an older
sb-core replays a journal that a newer one wrote, with the cross-hub kinds
(x_out, x_acked, x_failed, x_settled, x_in, x_replied), the timers'
acts (act_set, act_done), S10's follow and job_end, and hub-loop's
worktree_placed in it. It skips each of them (they are reported
as skipped, never misread) and rebuilds the rest of the state.

    python3 -u tests/old_core_replay.py [<base sha>]   (default: HEAD~1)

It first replays a journal this tree's own sb-core writes (a message, a
card, a timer, a watch, a held route: WRITE, real ms timestamps) with the
base's sb-core, which must bring them back and skip only new kinds.

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
    # sched-names (architect m_14532): a timer's name, on its every_set (an
    # older sb-core ignores the field) and by every_name (skipped)
    {"type": "every_set", "id": 1, "agent": "main", "by": "main", "text": "check the build", "next_ms": 9000,
     "at": 1000, "every_ms": 600000, "name": "build check"},
    {"type": "every_set", "id": 2, "agent": "main", "by": "main", "text": "read the log", "next_ms": 9000,
     "at": 1000, "every_ms": 600000, "held": True},
    {"type": "every_name", "id": 2, "name": "log read", "announce": True, "at": 1100},
    # msg-parts (architect m_17621): his messages with their parts (an
    # older sb-core ignores the key and keeps the message, its text whole):
    # a plain one, a queued one, one to another agent (a route; steered or
    # not is the delivery's, never the journal's). Before msg 7: a base
    # with St.hot/St.old (d1fe6f8d5) skips an id below next_msg.
    dict(msg(4, "user", "main"), msg=dict(msg(4, "user", "main")["msg"], plain=True,
         parts='[{"kind":"quote","from":"main","text":"q"},{"kind":"text","text":"m4"}]')),
    dict(msg(5, "user", "main"), msg=dict(msg(5, "user", "main")["msg"], plain=True, queued=True,
         parts='[{"kind":"text","text":"m5"}]')),
    dict(msg(6, "user", "perf"), msg=dict(msg(6, "user", "perf")["msg"], plain=True,
         parts='[{"kind":"paste","n":1,"text":"x"},{"kind":"text","text":"m6"}]')),
    # D1 (debt T7): a timer and a message with real ms timestamps (past
    # 2^32: the generated codec's laws can't write them as literals), the
    # timer in the generated writer's key order (`at` after its keys)
    {"type": "every_set", "id": 3, "agent": "main", "by": "user", "text": "check the deploy",
     "next_ms": 1760000600000, "every_ms": 600000, "until_ms": 1760086400000, "times": 5, "at": 1760000000400},
    dict(msg(7, "user", "main"), msg=dict(msg(7, "user", "main")["msg"], created_ms=1760000000700)),
    # event-wake: a watch of `sb wake` and its end (skipped: on a rollback
    # the watches are lost, the rest of the state is not)
    {"type": "wake_set", "watch": {"id": 1, "agent": "main", "spec": {"kind": "file", "path": "/t/rc"},
                                   "max_text": "still running", "set_at": 1000, "max_at": 3601000, "quiet": False}},
    {"type": "wake_set", "watch": {"id": 2, "agent": "main", "spec": {"kind": "bg", "slot": "/t/bg/0", "cmd": "make"},
                                   "max_text": "", "set_at": 1000, "max_at": 86401000, "quiet": True}},
    {"type": "wake_end", "id": 1, "why": "hit", "at": 1200},
    # hub-loop: the worktree the shell made for a placing task (skipped by
    # an older sb-core: the task keeps its placing workspace until its
    # next restart)
    {"type": "worktree_placed", "name": "perf", "ws": {"mode": "worktree", "path": "/w/wt/perf", "branch": "sb/perf",
                                                       "base_commit": "abc", "dropped": False}, "err": ""},
    # issue 32 3b: the end of a drop run off the loop (skipped by an older
    # sb-core: the task keeps its workspace as it was before the drop)
    {"type": "worktree_dropped", "name": "perf", "snapshot": None, "err": "fatal: could not remove the worktree"},
]


def is_new(e):
    """A kind an older sb-core may not know (it skips it, never misreads it)."""
    return e["type"].startswith(("x_", "act_", "wake_", "route_")) or e["type"] in ("follow", "job_end", "every_name", "worktree_placed", "worktree_dropped",
                                                                                 "card_withdrawn", "card_late")


NEW = [i for i, e in enumerate(FIXTURE) if is_new(e)]


class Core:
    """One sb-core on its link port."""

    def __init__(self, binary):
        s0 = socket.socket()
        s0.bind(("127.0.0.1", 0))
        port = s0.getsockname()[1]
        s0.close()
        self.proc = subprocess.Popen([binary], env=dict(os.environ, SB_CORE_PORT=str(port)),
                                     stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        banner = self.proc.stdout.readline()
        assert banner.startswith("sb-core on"), "banner: %r" % banner
        self.f = socket.create_connection(("127.0.0.1", port)).makefile("rw")
        self.call({"t": "init", "workspace": "/w"})

    def call(self, v):
        self.f.write(json.dumps(v) + "\n")
        self.f.flush()
        return json.loads(self.f.readline())

    def kill(self):
        self.proc.kill()


# D1 (architect m_23192): the journal a NEW core writes (this tree's
# sb-core, its journal effects), real ms timestamps
WRITE = [
    {"t": "boot"},
    {"t": "say", "focus": "main", "text": "ship the docs", "queued": False},
    {"t": "confirm_open", "agent": "main", "text": "drop docs?"},
    {"t": "every_set", "agent": "main", "by": "user", "text": "check the deploy", "next_ms": 1760000600000, "every_ms": 600000},
    {"t": "wake_set", "agent": "main", "spec": {"kind": "pid", "pid": 4242}, "max_text": "1h", "max_ms": 3600000, "quiet": False},
    {"t": "route_ask", "focus": "", "text": "@shop deploy", "via": "spotlight", "queued": False,
     "context": "", "projects": [{"id": "shop-1", "name": "shop"}]},
    # withdrawn-answer (architect m_30039): main withdraws its card 2, his
    # answer comes after: card_withdrawn, then main's message and card_late
    {"t": "req", "token": 1, "from": "main", "req": {"cmd": "card", "text": "ship v2?", "for": None}, "who": "outside", "sock": "agent"},
    {"t": "req", "token": 2, "from": "main", "req": {"cmd": "withdraw", "card": 2, "why": "moot"}, "who": "outside", "sock": "agent"},
    {"t": "answer", "card": 2, "text": "yes, ship it"},
    # name-free (architect m_31139): x archived, then a new x: the old one
    # is renamed x-2 (renamed), the new one created (task_created), kinds
    # every base knows; this tree's core gives the new x its own folder x.2
    {"t": "new", "base": "x", "brief": {"objective": "one", "context": ""}, "brief_text": "one", "objective": "one",
     "worktree": False, "with_changes": False},
    {"t": "drop", "name": "x", "force": True},
    {"t": "new", "base": "x", "brief": {"objective": "two", "context": ""}, "brief_text": "two", "objective": "two",
     "worktree": False, "with_changes": False},
]

# what this tree's sb-core says of the agents after WRITE (name-free)
AGENTS = [("main", "main"), ("x-2", "x"), ("x", "x.2")]


def agents_of(view):
    return [(a["name"], a.get("dir")) for a in view.get("view", view).get("agents", [])]


def written_by_this_tree(path):
    """This tree's sb-core runs WRITE; its journal lines go to `path`."""
    out = subprocess.run(["bash", "scripts/bins.sh", "path", "sb-core"], cwd=ROOT, capture_output=True, text=True)
    core = Core(out.stdout.strip().splitlines()[-1])
    lines = []
    try:
        for i, st in enumerate(WRITE):
            ans = core.call(dict(st, now=1760000000000 + i * 1000, git=False, ans=[]))
            lines += [x["ev"] for x in ans.get("fx", []) if x.get("fx") == "journal"]
        live = agents_of(core.call({"t": "view_all"}))
    finally:
        core.kill()
    with open(path, "w") as f:
        f.writelines(json.dumps(l) + "\n" for l in lines)
    kinds = [l["type"] for l in lines]
    assert kinds == ["message_sent", "card_opened", "every_set", "wake_set", "route_held", "act_set",
                     "card_opened", "card_closed", "card_withdrawn", "message_sent", "card_late",
                     "task_created", "main_note", "message_sent", "lifecycle", "message_state", "main_note",
                     "renamed", "task_created", "main_note", "message_sent"], kinds
    assert live == AGENTS, "the archived x is x-2 in its folder, the new x in x.2: %r" % live
    # the same journal replayed on this tree's sb-core: the same agents
    again = Core(out.stdout.strip().splitlines()[-1])
    try:
        again.call({"t": "replay_many", "evs": lines})
        back = agents_of(again.call({"t": "view_all"}))
    finally:
        again.kill()
    assert back == AGENTS, "a reused name replays to the same agents: %r" % back
    print("ok   name-free: a reused name (renamed x x-2, task_created x) replays to the same agents on this tree")
    return lines


def stale_ids_skipped():
    """Issue 21 steps 3-4: this tree's sb-core keeps the messages split by
    id (law ids_ascending), so a boot replay skips a message_sent whose id
    is below next_msg (a reused id, an older id) and says where: the first
    message keeps its record, the later ones come back in send order."""
    out = subprocess.run(["bash", "scripts/bins.sh", "path", "sb-core"], cwd=ROOT, capture_output=True, text=True)
    core = Core(out.stdout.strip().splitlines()[-1])
    reused = dict(msg(2, "user", "main"), msg=dict(msg(2, "user", "main")["msg"], text="reused"))
    older = dict(msg(1, "user", "main"), msg=dict(msg(1, "user", "main")["msg"], text="older"))
    evs = [msg(1, "user", "main"), msg(2, "user", "main"), reused, older, msg(3, "user", "main")]
    try:
        out = core.call({"t": "replay_many", "evs": evs})
        assert out.get("skipped") == [2, 3], "the reused and the older id are skipped: %r" % out
        one = core.call({"t": "replay", "ev": older})
        text = json.dumps(one)
        assert "skipped" in text, "a single replay of an older id says so: %r" % one
        view = core.call({"t": "view_all"})
        msgs = view.get("view", view).get("msgs", [])
        got = [(m["msg"]["id"], m["msg"]["text"]) for m in msgs]
        assert got == [(1, "m1"), (2, "m2"), (3, "m3")], "every message once, in send order: %r" % got
    finally:
        core.kill()
    print("ok   stale ids: a reused id and an older id are skipped, the messages stay in send order")


def replay_written(core, path):
    """The base's sb-core replays the new core's journal file: the message,
    the card, the timer and the watch come back; it skips only new kinds."""
    with open(path) as f:
        evs = [json.loads(l) for l in f if l.strip()]
    core.call({"t": "init", "workspace": "/w"})
    out = core.call({"t": "replay_many", "evs": evs})
    assert isinstance(out, dict) and "error" not in out and "err" not in out, "the replay of the new journal failed: %r" % out
    skipped = out.get("skipped", [])
    assert all(is_new(evs[i]) for i in skipped), "the base skips only new kinds: %r" % [evs[i]["type"] for i in skipped]
    view = core.call({"t": "view_all"})
    view = view.get("view", view)
    assert "ship the docs" in json.dumps(view.get("msgs", [])), "the message: %r" % view.get("msgs")
    # name-free: the base reads renamed and task_created as it always did
    names = [n for n, _ in agents_of(view)]
    assert names == [n for n, _ in AGENTS], "the reused name's agents: %r" % names
    assert "he answered #2 after you withdrew it" in json.dumps(view.get("msgs", [])), "his late answer: %r" % view.get("msgs")
    assert [c["id"] for c in view.get("cards", [])] == [1], "the card: %r" % view.get("cards")
    assert [t["id"] for t in view.get("timers", [])] == [1], "the timer: %r" % view.get("timers")
    assert [w["id"] for w in view.get("wakes", [])] == [1] or 3 in skipped, "the watch: %r" % view.get("wakes")
    print("ok   new journal: %d lines written by this tree's sb-core, %d skipped (%s), message, card, timer%s back"
          % (len(evs), len(skipped), ", ".join(evs[i]["type"] for i in skipped) or "none",
             "" if 3 in skipped else ", watch"))


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
        written_by_this_tree(os.path.join(tmp, "journal.jsonl"))
        stale_ids_skipped()
        replay_written(Core(binary), os.path.join(tmp, "journal.jsonl"))
        out = call({"t": "replay_many", "evs": FIXTURE})
        # the replay succeeds and no line it does not know fails it: a kind
        # newer than the base is skipped, a kind the base knows is applied
        # (a base that knows them all, e.g. main itself, skips none)
        assert isinstance(out, dict) and "error" not in out and "err" not in out, "the replay failed: %r" % out
        skipped = out.get("skipped", [])
        assert set(skipped) <= set(NEW), "the base's sb-core skips only the new kinds: %r (new: %r)" % (skipped, NEW)
        print("     skipped kinds: %s" % (", ".join(sorted({FIXTURE[i]["type"] for i in skipped})) or "none"))
        view = call({"t": "view_all"})
        assert core.poll() is None, "the base's sb-core died on the journal"
        text = json.dumps(view)
        assert all(("m%d" % i) in text for i in (1, 2, 3, 4, 5, 6, 7)), "the messages are rebuilt: %s" % text[:500]
        # sched-names: a timer with a name is rebuilt (an older base drops the name)
        timers = view.get("view", view).get("timers", [])
        assert sorted(t["id"] for t in timers) == [1, 2, 3], "the timers are rebuilt: %r" % timers
        print("ok   old_core_replay: %s's sb-core skipped %d of %d new lines, kept the messages" % (sha[:8], len(skipped), len(NEW)))
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
