"""A desktop app's hubs never run from inside its bundle (ambient-lead m_21726, architect m_21731).

His try refused its update twice: after the app quit, the hubs its core had started (and their
REPLs, plugin bridges and computer-use servers) still ran from 'bise try.app/Contents/Resources/bin/
bise', so the swap of the bundle had to wait for them. The core installs the bundle's runtime as
<prefix>/versions/<id> (update::hub_root) and now starts its hubs with that dir's bise
(update::hub_exe).

Here, on a throwaway home: a stand-in bundle (Contents/Resources/bin: this tree's bise, its runtime
and a VERSION), `bise ambient-core` run from it as the app runs it, its hub up and main's REPL
started by a message, then a NEW agent (t1). Then: the hub and both REPLs run from
versions/<id>/bise's dir, and once the core is gone no process of the hub's tree (sbd, repl-live,
plugins serve, computer-use) runs from the bundle. Last, app B, another build with its own id
(root-<sha12>, as desktop_try names a candidate): its new hub runs from versions/<B's id>, B's own
runtime, and A's dir is left as it was.

Run: python3 -u tests/hub_outside_app_e2e.py (after cd rust && cargo build)
"""
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from ambient_core_e2e import Core  # noqa: E402
from e2e import EXE, ROOT, check  # noqa: E402

# the runtime the desktop pack puts next to bise (electron-builder.mjs, BISE_PACK_ROOT)
RUNTIME = ("repl-live", "repl-scripted", "sb-core", "prompts", "kit", "plugins", "computer-use")
A, B = "a1" * 20, "b2" * 20
# a VERSION id names one build (desktop_try: root-<sha12> for a candidate); two builds under one id
# are an error (architect m_21930; the legacy 'root' id's second dir went in issue 31)
ID, ID_B = "root-" + A[:12], "root-" + B[:12]


def bundle(tmp, name, commit, vid):
    """A stand-in '<name>.app' with its runtime in Contents/Resources/bin: bise copied (its
    canonical path must be in the bundle), the runtime linked to this tree, a VERSION with that
    id and commit."""
    app = os.path.join(tmp, name + ".app")
    bin_dir = os.path.join(app, "Contents", "Resources", "bin")
    os.makedirs(bin_dir)
    shutil.copy2(EXE, os.path.join(bin_dir, "bise"))
    for n in RUNTIME:
        src = os.path.join(ROOT, n)
        if os.path.exists(src):
            os.symlink(src, os.path.join(bin_dir, n))
    with open(os.path.join(bin_dir, "VERSION"), "w") as f:
        f.write("id=%s%scommit=%s%s" % (vid, chr(10), commit, chr(10)))
    return app, bin_dir


def procs():
    """Every process: (pid, command)."""
    out = subprocess.run(["ps", "-axww", "-o", "pid=,command="], capture_output=True, text=True).stdout
    rows = []
    for line in out.splitlines():
        pid, _, cmd = line.strip().partition(" ")
        if pid.isdigit():
            rows.append((int(pid), cmd))
    return rows


def turns(c, name):
    """`name`'s finished turns on the hub (0 while one runs or before its first)."""
    a = c.agent(name) or {}
    return (a.get("turns") or 0) if a.get("status") == "idle" else 0


def hubs_in(versions):
    """The hubs that run from that versions dir (whatever their workspace)."""
    return [c for _, c in hub_pids(versions)]


def hub_pids(versions):
    """The hubs that run from that versions dir: (pid, command)."""
    return [(p, c) for p, c in procs() if " sbd --workspace " in c and c.startswith(versions)]


def zombies(parent):
    """`parent`'s children that exited and were never waited for: (pid, command)."""
    out = subprocess.run(["ps", "-axww", "-o", "pid=,ppid=,stat=,command="], capture_output=True, text=True).stdout
    rows = []
    for line in out.split(chr(10)):
        f = line.split(None, 3)
        if len(f) >= 3 and f[1] == str(parent) and f[2].startswith("Z"):
            rows.append((int(f[0]), f[3] if len(f) > 3 else ""))
    return rows


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    core = None
    app, bin_dir = bundle(E.tmp, "bise a", A, ID)
    inside = os.path.join(app, "Contents") + "/"
    app_b, bin_b = bundle(E.tmp, "bise b", B, ID_B)
    # the throwaway home's versions dir: every hub an app starts runs from there. The core starts
    # the home hub and the workspace's, and SB_STATE_DIR gives both one socket: whichever wins
    # holds it (the other one exits), so the laws below are about the one hub on that dir
    versions = [os.path.join(E.env["HOME"], ".local", "share", "bise", "versions") + "/"]
    try:
        core = Core(E, exe=os.path.join(bin_dir, "bise"), cwd=E.tmp, env=E.env)
        core.wait(lambda e: e.get("ev") == "hub" and e.get("up") is True, 60, "hub up")
        core.wait(lambda e: e.get("ev") == "state", 30, "a state")
        # main's REPL (and its plugin bridge) starts with a message, sent on the hub's socket: the
        # core's own send goes to its focused project, the home one here, not this workspace
        c = e2e.Client(os.path.join(E.state, "hub.sock"))
        c.say("hello")
        c.wait(lambda: turns(c, "main") >= 1, 120, "main's turn done")
        # a NEW agent: the hub starts a REPL from its own dir (ambient-lead m_21839: a dir without
        # repl-live kept old REPLs but could start none)
        c.say("/new t1: say hi")
        c.wait_status("t1", ["idle", "done", "blocked"], 120)

        wait.until(lambda: len(hubs_in(versions[0])) == 1, 30, "one hub on " + versions[0])
        exe = hubs_in(versions[0])[0].split(" sbd --workspace ")[0]
        dir_a = os.path.dirname(exe)
        check(exe.endswith(os.path.join("versions", ID, "bise")), "the hub runs from the installed runtime: %s" % exe)
        check(not exe.startswith(inside), "never from inside the bundle: %s" % exe)
        repls = [c for _, c in procs() if c.startswith(dir_a + "/repl-live")]
        check(len(repls) >= 2, "main's and t1's REPLs run from %s: %r" % (dir_a, repls))
        mine = [(p, c) for p, c in procs() if c.startswith(versions[0]) or c.startswith(inside)]
        check(all(not c.startswith(inside) for _, c in mine if "ambient-core" not in c),
              "only the core runs from the bundle: %r" % [m for m in mine if m[1].startswith(inside)])

        # one home hub, and none left a zombie under the core (ambient-lead m_21848: at boot his
        # core's home and project connections to the same folder both started one; the loser,
        # and every hub killed under it, stayed <defunct> children of the core)
        check(zombies(core.p.pid) == [], "no zombie under the core: %r" % zombies(core.p.pid))
        # his home hub killed under the core: reaped, and the core starts it again (fn talked to
        # nothing until the app restarted)
        # the hub on the socket is whichever won it (the home's or E.ws's, see above): kill that one
        at = core.mark()
        old = hub_pids(versions[0])[0][0]
        os.kill(old, 9)
        wait.until(lambda: all(p != old for p, _ in procs()), 20, "the killed hub reaped (never <defunct>)", poll=0.2)
        core.wait(lambda e: e.get("ev") == "hub" and e.get("up") is True, 90, "the home hub up again", since=at)
        wait.until(lambda: len(hub_pids(versions[0])) == 1, 30, "one hub on " + versions[0] + " again")
        again = hub_pids(versions[0])
        check(len(again) == 1 and again[0][0] != old, "one new home hub: %r" % again)
        check(zombies(core.p.pid) == [], "still no zombie under the core: %r" % zombies(core.p.pid))
        core.cmd(cmd="send", text="still there?")
        core.wait(lambda e: e.get("ev") == "sent", 60, "a send after the restart is taken", since=at)

        # the app quits: its core goes; the hub's tree stays, none of it in the bundle
        core.p.stdin.close()
        core.p.wait(timeout=20)
        core = None
        left = [(p, c) for p, c in procs() if c.startswith(inside)]
        check(left == [], "after the app quit nothing runs from the bundle: %r" % left)
        check(hubs_in(versions[0]) != [], "its hub lives on")

        # app B, another build: its new hub runs B's runtime, never A's dir
        e2e.stop_hub(os.path.join(E.state, "hub.sock"), False)
        wait.until(lambda: hubs_in(versions[0]) == [], 30, "A's hub gone")
        core = Core(E, exe=os.path.join(bin_b, "bise"), cwd=E.tmp, env=E.env, err="core-b.stderr")
        core.wait(lambda e: e.get("ev") == "hub" and e.get("up") is True, 60, "B's hub up")
        wait.until(lambda: len(hubs_in(versions[0])) == 1, 30, "one hub on " + versions[0])
        exe_b = hubs_in(versions[0])[0].split(" sbd --workspace ")[0]
        check(exe_b.endswith(os.path.join("versions", ID_B, "bise")), "B's hub runs B's own dir: %s" % exe_b)
        ver = open(os.path.join(os.path.dirname(exe_b), "VERSION")).read()
        check("commit=" + B in ver, "that dir is B's build: %r" % ver)
        check("commit=" + A in open(os.path.join(dir_a, "VERSION")).read(), "A's dir untouched")
        # a new hub counts its turns from 0 (the agents row's turns: this run's)
        c = e2e.Client(os.path.join(E.state, "hub.sock"))
        c.wait(lambda: c.agent("main") is not None, 30, "main on B's hub")
        check(turns(c, "main") == 0, "B's hub: no turn of main yet: %r" % c.agent("main"))
        c.say("hello again")
        c.wait(lambda: turns(c, "main") >= 1, 120, "main's turn on B")
        repls = [c for _, c in procs() if c.startswith(os.path.dirname(exe_b) + "/repl-live")]
        check(len(repls) >= 1, "main's REPL runs from B's dir: %r" % repls)
    finally:
        if core is not None:
            core.p.kill()
        e2e.stop_hub(os.path.join(E.state, "hub.sock"), False)
        wait.until(lambda: not any(c.startswith(E.tmp) or (versions[0] and c.startswith(versions[0])) for _, c in procs()), 30,
                   "the hub's tree gone", poll=0.2)
        E.close()
    print("hub_outside_app_e2e: ok")


if __name__ == "__main__":
    main()
