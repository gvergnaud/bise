#!/usr/bin/env python3
"""approvals-tmp (docs/approvals-design.md §7.1): each agent's temp folder.

A real hub, real REPLs, the scripted provider (e2e.Env), a temp HOME.
A task `tt` runs, through its bash tool:
  1. `echo $TMPDIR $TMP $TEMP $TMUX_TMPDIR`: all four are its
     `<state>/agents/tt/tmp`;
  2. `mktemp` and python's `tempfile`: both land there;
  3. `tmux -L x`: its socket lands there (TMUX_TMPDIR);
  4. a job that outlives the sync window: its slot is `tmp/bg/0`.
A message while it works goes to its steer file, an interrupt to its
interrupt file: both in `run/`, like the bash wrappers. No harness file of
its REPL's port is in /tmp. Its role names the folder. /drop removes
`tmp/` (never `run/`); a hub start removes the `tmp/` of a gone agent.
"""
import glob
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import Env, check  # noqa: E402


def ports_in(run):
    """the REPL ports named by the harness files of a run/ folder"""
    out = set()
    for n in os.listdir(run):
        for pre in ("bend-steer-", "bend-interrupt-", "bend-sh-", "bend-plugins-start-", "bend-skills-scan-"):
            if n.startswith(pre):
                out.add(n[len(pre):].split("-")[0].split(".")[0])
    return out


def main():
    # /tmp keeps old runs' files and ports get reused: only this run's count
    started = time.time() - 1
    E = Env()
    home = os.path.join(E.tmp, "home")
    os.makedirs(home)
    # a temp HOME; a 3 s sync window (the 4th command goes to the background)
    E.env.update(HOME=home, XDG_STATE_HOME=os.path.join(home, "state"), BEND_BG_AFTER="3")
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        tmp = os.path.join(E.state, "agents", "tt", "tmp")
        run = os.path.join(E.state, "agents", "tt", "run")
        probe = os.path.join(E.ws, "probe.txt")
        cmds = [
            'echo "T=$TMPDIR|$TMP|$TEMP|$TMUX_TMPDIR" >> probe.txt',
            'mktemp >> probe.txt; python3 -c "import tempfile; print(tempfile.mkstemp()[1])" >> probe.txt',
            'tmux -L x new-session -d "sleep 30"; ls "$TMUX_TMPDIR"/tmux-*/ >> probe.txt; tmux -L x kill-server',
            'sleep 5; echo bg-done',
        ]
        brief = " ".join("{{bash: %s}}" % x for x in cmds)
        c.say("[[bash: sb spawn tt --objective '%s']]" % brief)
        c.wait(lambda: c.agent("tt") is not None, 60, "tt exists")
        c.wait(lambda: os.path.exists(os.path.join(tmp, "bg", "0.out")), 90, "the background job's slot in tmp/bg")
        c.wait_idle("tt")
        # a message while it works: the hub steers it (its steer file). The
        # script is the last user message: the steer comes in a turn of its own
        c.say("[[bash: sleep 2.5; echo slept]]", focus="tt")
        c.wait_status("tt", "working", 60)
        c.say("keep going", focus="tt")
        c.wait(lambda: glob.glob(os.path.join(run, "bend-steer-*.txt")), 30, "the steer file in run/")
        c.wait_idle("tt")
        c.interrupt("tt")
        c.wait(lambda: glob.glob(os.path.join(run, "bend-interrupt-*.txt")), 30, "the interrupt file in run/")
        lines = open(probe).read().split("\n")
        # TMUX_TMPDIR only when a socket fits under it (tools_env::tmux_fits)
        real = os.path.realpath(tmp)
        fits = len("%s/tmux-%d/" % (real, os.getuid())) + 16 <= 103
        tm = tmp if fits else ""
        check(lines[0] == "T=%s|%s|%s|%s" % (tmp, tmp, tmp, tm), "TMPDIR, TMP, TEMP, TMUX_TMPDIR: %r" % lines[0])
        check(lines[1].startswith(tmp + "/") and lines[2].startswith(tmp + "/"),
              "mktemp and tempfile land in tmp/: %r" % lines[1:3])
        check(lines[3] == "x" if fits else True, "tmux's socket is in tmp/: %r" % lines[3:])
        c.wait(lambda: "bg-done" in open(os.path.join(tmp, "bg", "0.out")).read(), 30, "the job's output")
        ports = ports_in(run)
        check(ports, "run/ holds the REPL's files: %r" % os.listdir(run))
        for p in ports:
            left = glob.glob("/tmp/bend-*-%s*" % p) + glob.glob("/tmp/bend-*-%s" % p)
            left = [f for f in set(left) if os.path.getmtime(f) >= started]
            check(not left, "no harness file of port %s in /tmp: %r" % (p, left))
        role = open(os.path.join(E.state, "agents", "tt", "role.md")).read()
        check("Your temp folder is `%s` (`$TMPDIR`)" % tmp in role, "the role names the folder")
        main_role = open(os.path.join(E.state, "agents", "main", "role.md")).read()
        check("Your temp folder is `%s`" % os.path.join(E.state, "agents", "main", "tmp") in main_role,
              "main's role names its folder")
        # /archive: tmp/ goes, run/ stays
        c.say("/archive tt")
        c.wait(lambda: c.agent("tt")["status"] == "archived"
               or any(n["kind"] == "confirm" for n in c.notices()), 30, "tt dropped or a confirmation")
        conf = [n for n in c.notices() if n["kind"] == "confirm"]
        if conf:
            c.confirm(conf[-1]["id"], True)
        c.wait_status("tt", "archived", 30)
        c.wait(lambda: not os.path.exists(tmp), 10, "the dropped task's tmp/ removed")
        check(os.path.isdir(run), "its run/ stays")
        # a hub start: the tmp/ of an agent that is gone is removed, main's kept
        gone = os.path.join(E.state, "agents", "ghost", "tmp")
        os.makedirs(gone)
        E.stop_hub()
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.wait(lambda: not os.path.exists(gone), 10, "a gone agent's tmp/ removed at the start")
        check(os.path.isdir(os.path.join(E.state, "agents", "main", "tmp")), "main's tmp/ kept")
        ok = True
        print("agent temp folders: ok")
    finally:
        if not ok:
            os.environ["SB_KEEP"] = "1"
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
