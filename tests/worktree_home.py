"""The task worktrees' home (BISE-230): <home>/worktrees/<project-id>/<task>/.

A real hub (bise sbd) with a throwaway BISE_HOME and no SB_STATE_DIR, like
the user's: a hub worktree of the old layout (<state>/worktrees/<task>)
moves at the hub's start and the task follows it; the start removes clean
orphan folders (and the old place's), keeps a dirty one and says so in
main's thread; a /drop removes the task's folders, the hub's worktree and
a gate.sh-like one (owner file).

Run: python3 -u tests/worktree_home.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from e2e import EXE, check, out, sh  # noqa: E402

OLD = time.time() - 3600  # an orphan older than the hub's grace (10 min)


def project_id(path):
    """switchboard::paths::workspace_id, as gate.sh computes it (pinned by
    paths::tests::ids_match_the_python_copies)."""
    p = os.path.realpath(path)
    h = e2e.fnv1a64(p.encode())
    base = "".join(c if c.isascii() and (c.isalnum() or c in "-_") else "-" for c in os.path.basename(p))[:32]
    return "%s-%08x" % (base, h & 0xFFFFFFFF)


def aged(*paths):
    for p in paths:
        os.utime(p, (OLD, OLD))


def orphan(E, root, name, dirty=False):
    """A folder like gate.sh new makes, for a task this hub never had."""
    d = os.path.join(root, name)
    os.makedirs(os.path.join(d, "target", "debug"))
    open(os.path.join(d, "target", "debug", "big"), "w").write("x")
    open(os.path.join(d, "owner"), "w").write(name + "\n")
    sh(E.ws, "git worktree add -q --detach %s HEAD" % os.path.join(d, "ws"))
    if dirty:
        open(os.path.join(d, "ws", "wip.txt"), "w").write("mine\n")
    aged(os.path.join(d, "owner"), d)
    return d


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    home = os.path.join(os.path.realpath(E.tmp), "bise")
    E.env.pop("SB_STATE_DIR")
    E.env["BISE_HOME"] = home
    pid = project_id(E.ws)
    E.state = os.path.join(home, "hubs", pid)
    root = os.path.join(home, "worktrees", pid)
    legacy = os.path.join(E.state, "worktrees")
    ok = True
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say("/new -w t1: {{bash: echo ready}}")
        c.wait(lambda: c.agent("t1") is not None, 60, "t1")
        c.wait_idle("t1")
        new = c.agent("t1")["path"]
        check(new == os.path.join(root, "t1", "ws"), "a hub worktree in its task folder: " + new)
        c.say('[[bash: sb spawn t2 --objective "{{bash: echo ready}}"]]')
        c.wait(lambda: c.agent("t2") is not None, 60, "t2")
        c.wait_idle("t2", "main")
        E.stop_hub()

        # the state of a hub before BISE-230: t1's worktree in <state>/worktrees
        os.makedirs(legacy)
        old = os.path.join(legacy, "t1")
        sh(E.ws, "git worktree move %s %s && rm -rf %s" % (new, old, os.path.dirname(new)))
        j = os.path.join(E.state, "journal.jsonl")
        text = open(j).read()
        open(j, "w").write(text.replace(new, old))
        # orphans: clean (removed), dirty (kept), and one in the old place
        gone = orphan(E, root, "gone")
        dirty = orphan(E, root, "dirty", dirty=True)
        sh(E.ws, "git worktree add -q --detach %s HEAD" % os.path.join(legacy, "old-clean"))
        aged(os.path.join(legacy, "old-clean"))
        # t2 works in a gate.sh-like folder of its own (owner = t2)
        mine = orphan(E, root, "t2-gate")
        open(os.path.join(mine, "owner"), "w").write("t2\n")

        c = E.start_hub()
        c.wait_status("t1", ["idle", "done", "blocked", "starting"], 60)
        moved = c.agent("t1")["path"]
        check(moved == new, "t1 follows its moved worktree: " + moved)
        check(os.path.isfile(os.path.join(moved, ".git")) and os.readlink(old) == new, "moved, a link left")
        check(new in out(E.ws, "git worktree list"), "git knows the new place")
        c.wait(lambda: not os.path.exists(gone), 30, "the clean orphan removed")
        c.wait(lambda: not os.path.exists(os.path.join(legacy, "old-clean")), 30, "the old place's orphan removed")
        c.wait_line("main", "not deleted: 1 uncommitted change", 30)
        check(os.path.exists(os.path.join(dirty, "ws", "wip.txt")), "the dirty orphan kept")
        check(os.path.exists(mine) and os.path.exists(new), "live tasks' folders kept")
        check("gone" not in out(E.ws, "git worktree list"), "git forgot the removed worktree")

        # an /archive: the task's folders go (the hub's worktree, a gate.sh one)
        c.wait_idle("t1", "t2", "main")
        c.say("/archive t1")
        c.wait_status("t1", "archived", 30)
        c.wait(lambda: not os.path.exists(os.path.dirname(new)), 30, "t1's folder removed")
        c.say("/archive t2")
        c.wait_status("t2", "archived", 30)
        c.wait(lambda: not os.path.exists(mine), 30, "t2's gate.sh folder removed")
        check(os.path.exists(dirty), "the dirty orphan still there")
        print("PASS worktree_home", flush=True)
    except Exception as e:
        ok = False
        print("FAIL worktree_home: %s" % e, flush=True)
        os.environ["SB_KEEP"] = "1"
    finally:
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
